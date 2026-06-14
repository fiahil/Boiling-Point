//! The group task: a single async owner of one group's state.
//!
//! Each connection talks to the group only through [`GroupCommand`]s on an mpsc
//! channel; the group pushes [`ServerMessage`]s back out through each seat's own
//! mpsc sender. This keeps the group the sole writer of its state (no locks).
//!
//! A **group persists across games** (group-model): it serves the lobby, starts a
//! game once it holds 4 ready players, drives it via `session::run_game`, then
//! returns the survivors to the lobby. A group distinguishes **members** (joined by
//! invite/quick-match — persistent, carry standings) from **guests** (placed by
//! matchmaking fill — present for one game, dropped at `GameOver`). A partial group
//! can `FillGroup` to matchmake for the rest, showing a "looking for a 4th…" state.
//! Each group keeps a live, in-memory win tally (per member + a guest aggregate).
//! The group is destroyed only when it empties, sits idle past the timeout, or an
//! operator kills it, at which point it deregisters from the registry.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::mpsc;
use tokio::time::Instant;

use boiling_point_protocol::server::{ErrorCode, MemberStanding, PlayerPublic};
use boiling_point_protocol::vocab::Color;
use boiling_point_protocol::{ClientMessage, GroupCode, PlayerId, ServerMessage};

use crate::config::{ContentConfig, ROUND_COUNT};
use crate::content::ContentRegistry;
use crate::rating::RatingStore;
use crate::session::{self, GameEnd, SeatInfo};

use super::accounts::AccountStore;
use super::registry::GroupRegistry;

/// Exactly four players to a table. Also the **member cap**: a group holds at most
/// this many members, so it never needs more than fill to reach a full table.
const TABLE_SIZE: usize = 4;
/// A group sitting in its lobby this long without starting a game is destroyed
/// (resets after every game and on any lobby activity).
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// A command delivered to a group's task by a connection.
pub enum GroupCommand {
    /// A player joins (or reconnects), providing the outbound channel to reach them.
    Join {
        /// The joining player's id.
        player: PlayerId,
        /// Their display name.
        name: String,
        /// The connection's session token, echoed back in `GroupJoined` so the
        /// client can persist and replay it.
        session_token: String,
        /// Whether this player is a matchmaking **guest** (placed by fill) rather
        /// than a member (invite/quick-match join).
        guest: bool,
        /// Channel the group uses to send this player messages.
        out: mpsc::Sender<ServerMessage>,
    },
    /// A player's connection dropped, or they explicitly left the group.
    Leave {
        /// The departing player.
        player: PlayerId,
    },
    /// A gameplay/table action from a seated player.
    Action {
        /// The acting player.
        player: PlayerId,
        /// The client message.
        msg: ClientMessage,
    },
    /// Admin command-plane: start the game now with the currently seated players,
    /// without waiting for the table to fill.
    ForceStart,
    /// Admin command-plane: tear this group down (kill an idle or stuck group). The
    /// group's task ends, deregisters, and its `group.lifetime` span closes.
    Shutdown,
}

/// A handle to a running group: its code and the channel to command it.
#[derive(Clone)]
pub struct GroupHandle {
    /// The group's invite code.
    pub code: GroupCode,
    /// Channel to send the group commands.
    pub tx: mpsc::Sender<GroupCommand>,
}

/// One seated player within a group.
struct Seat {
    id: PlayerId,
    name: String,
    color: Color,
    out: mpsc::Sender<ServerMessage>,
    /// A matchmaking guest (one game) rather than a group member.
    guest: bool,
    /// Whether this seat has opted in to (the next) game. Set on join and on
    /// `PlayAgain`; cleared after each game so play-again is an explicit opt-in.
    ready: bool,
}

/// A single member's win record within the group's live standings.
#[derive(Default, Clone, Copy)]
struct MemberRecord {
    games_played: u32,
    wins: u32,
}

/// The group's live, in-memory standings: per-member games/wins, plus an aggregate
/// guest line so guest results don't vanish. Dies with the group (never persisted).
#[derive(Default)]
struct Standings {
    members: HashMap<PlayerId, MemberRecord>,
    guest_games: u32,
    guest_wins: u32,
}

impl Standings {
    /// Fold a finished game into the tally: `roster` is `(player, is_guest)` for
    /// everyone who played, `winners` the game's champion(s).
    fn record_game(&mut self, roster: &[(PlayerId, bool)], winners: &[PlayerId]) {
        let had_guest = roster.iter().any(|(_, guest)| *guest);
        for (id, guest) in roster {
            if !guest {
                self.members.entry(*id).or_default().games_played += 1;
            }
        }
        if had_guest {
            self.guest_games += 1;
        }
        for w in winners {
            match roster.iter().find(|(id, _)| id == w) {
                Some((_, true)) => self.guest_wins += 1,
                Some((_, false)) => self.members.entry(*w).or_default().wins += 1,
                None => {}
            }
        }
    }

    /// Whether nothing has been tallied yet (a fresh group with no games played).
    fn is_empty(&self) -> bool {
        self.guest_games == 0 && self.members.values().all(|r| r.games_played == 0)
    }

    /// The standings message for the group's current members.
    fn message(&self, seats: &[Seat]) -> ServerMessage {
        let members = seats
            .iter()
            .filter(|s| !s.guest)
            .map(|s| {
                let rec = self.members.get(&s.id).copied().unwrap_or_default();
                MemberStanding {
                    player: s.id,
                    games_played: rec.games_played,
                    wins: rec.wins,
                }
            })
            .collect();
        ServerMessage::StandingsUpdate {
            members,
            guest_games: self.guest_games,
            guest_wins: self.guest_wins,
        }
    }
}

/// Spawn a group task and return a handle to it. `pool` is the optional
/// persistence pool, threaded to each game's post-game write; `accounts`/
/// `ratings` are the shared identity stores, for the post-game rating update and
/// the fill anchor-rating lookup.
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    code: GroupCode,
    groups: Arc<GroupRegistry>,
    registry: Arc<ContentRegistry>,
    config: Arc<ContentConfig>,
    emote_palette: Arc<HashSet<u16>>,
    pool: Option<PgPool>,
    accounts: Arc<AccountStore>,
    ratings: Arc<RatingStore>,
) -> GroupHandle {
    let (tx, rx) = mpsc::channel(64);
    tokio::spawn(run(
        code.clone(),
        tx.clone(),
        rx,
        groups,
        registry,
        config,
        emote_palette,
        pool,
        accounts,
        ratings,
    ));
    GroupHandle { code, tx }
}

/// The searching group's anchor rating: the mean conservative rating of its
/// present **members** (guests don't count toward the group's skill), or `None`
/// if no member has a rated account.
fn anchor_rating(seats: &[Seat], accounts: &AccountStore, ratings: &RatingStore) -> Option<i32> {
    let rated: Vec<i32> = seats
        .iter()
        .filter(|s| !s.guest)
        .filter_map(|s| {
            accounts
                .account_for_player(s.id)
                .map(|a| ratings.view(a.id).display)
        })
        .collect();
    if rated.is_empty() {
        None
    } else {
        Some(rated.iter().sum::<i32>() / rated.len() as i32)
    }
}

fn public(seats: &[Seat]) -> Vec<PlayerPublic> {
    seats
        .iter()
        .map(|s| PlayerPublic {
            id: s.id,
            display_name: s.name.clone(),
            color: s.color,
            connected: true,
            guest: s.guest,
        })
        .collect()
}

async fn broadcast(seats: &[Seat], msg: ServerMessage) {
    for seat in seats {
        let _ = seat.out.send(msg.clone()).await;
    }
}

async fn broadcast_except(seats: &[Seat], except: PlayerId, msg: ServerMessage) {
    for seat in seats.iter().filter(|s| s.id != except) {
        let _ = seat.out.send(msg.clone()).await;
    }
}

/// The first table colour not already taken by a seated player.
fn first_free_color(seats: &[Seat]) -> Color {
    Color::PLAYER_COLORS
        .into_iter()
        .find(|c| !seats.iter().any(|s| s.color == *c))
        .unwrap_or(Color::PLAYER_COLORS[0])
}

/// A game starts when the table is full and every seat has opted in.
fn ready_to_start(seats: &[Seat]) -> bool {
    seats.len() == TABLE_SIZE && seats.iter().all(|s| s.ready)
}

/// Announce and drive one full game to completion, fold the result into standings,
/// then return the **members** to the lobby: guests are dropped (one game only),
/// players who left/abandoned and never reconnected are dropped, reconnected
/// members keep their refreshed channel, and every remaining seat is reset to
/// not-ready (play-again is an explicit opt-in). Any in-progress fill search is
/// closed first. Broadcasts the updated standings to the surviving members.
#[allow(clippy::too_many_arguments)]
async fn run_one_game(
    code: &GroupCode,
    seats: &mut Vec<Seat>,
    standings: &mut Standings,
    searching: &mut bool,
    groups: &Arc<GroupRegistry>,
    rx: &mut mpsc::Receiver<GroupCommand>,
    registry: &Arc<ContentRegistry>,
    config: &Arc<ContentConfig>,
    emote_palette: &Arc<HashSet<u16>>,
    pool: Option<&PgPool>,
    accounts: &AccountStore,
    ratings: &RatingStore,
) {
    if *searching {
        groups.close_fill(code);
        *searching = false;
    }
    broadcast(
        seats,
        ServerMessage::GameStarting {
            players: public(seats),
            round_count: ROUND_COUNT,
        },
    )
    .await;
    let roster: Vec<(PlayerId, bool)> = seats.iter().map(|s| (s.id, s.guest)).collect();
    let seat_infos: Vec<SeatInfo> = seats
        .iter()
        .map(|s| SeatInfo {
            id: s.id,
            name: s.name.clone(),
            color: s.color,
            guest: s.guest,
            out: s.out.clone(),
        })
        .collect();
    let seed: u64 = groups.next_game_seed();
    let GameEnd {
        players,
        gone,
        winners,
    } = session::run_game(
        registry.as_ref(),
        config.as_ref(),
        code.clone(),
        seat_infos,
        rx,
        emote_palette.as_ref(),
        seed,
        pool,
        accounts,
        ratings,
    )
    .await;
    standings.record_game(&roster, &winners);
    // Rebuild the roster from the game's final seats (reconnects refreshed their
    // `out` channel mid-game), dropping guests and anyone still gone, and clear
    // ready flags.
    *seats = players
        .into_iter()
        .filter(|p| !p.guest && !gone.contains(&p.id))
        .map(|p| Seat {
            id: p.id,
            name: p.name,
            color: p.color,
            out: p.out,
            guest: false,
            ready: false,
        })
        .collect();
    broadcast(seats, standings.message(seats)).await;
}

// `group.lifetime` (span_schema::span::GROUP_LIFETIME) — the outermost span; the
// live open-span registry keys off it. Instrumenting the whole task makes every
// child span (game → round → wave …) nest under this group, and a persistent group
// nests several `game` spans over its life. The field name matches
// `span_schema::attr::GROUP_CODE`.
#[tracing::instrument(name = "group.lifetime", skip_all, fields(group.code = %code.0))]
#[allow(clippy::too_many_arguments)]
async fn run(
    code: GroupCode,
    self_tx: mpsc::Sender<GroupCommand>,
    mut rx: mpsc::Receiver<GroupCommand>,
    groups: Arc<GroupRegistry>,
    registry: Arc<ContentRegistry>,
    config: Arc<ContentConfig>,
    emote_palette: Arc<HashSet<u16>>,
    pool: Option<PgPool>,
    accounts: Arc<AccountStore>,
    ratings: Arc<RatingStore>,
) {
    let mut seats: Vec<Seat> = Vec::new();
    let mut standings = Standings::default();
    let mut searching = false;

    // Persistent lobby loop: serve commands, run games when the table is ready, and
    // return to the lobby afterwards. Ends only when the group empties, sits idle
    // past the timeout, or an operator kills it.
    let idle = tokio::time::sleep(IDLE_TIMEOUT);
    tokio::pin!(idle);
    loop {
        let cmd = tokio::select! {
            _ = &mut idle => {
                tracing::info!(code = %code.0, "group idle timeout — destroying");
                break;
            }
            cmd = rx.recv() => match cmd {
                Some(c) => c,
                None => break,
            },
        };
        // Any lobby activity resets the idle window (it gauges a *stalled* lobby).
        idle.as_mut().reset(Instant::now() + IDLE_TIMEOUT);

        match cmd {
            GroupCommand::Join {
                player,
                name,
                session_token,
                guest,
                out,
            } => {
                // Reconnect into the lobby: a returning player reattaches their
                // channel rather than taking a second seat.
                if let Some(seat) = seats.iter_mut().find(|s| s.id == player) {
                    seat.out = out.clone();
                    let color = seat.color;
                    let _ = out
                        .send(ServerMessage::GroupJoined {
                            group_code: code.clone(),
                            your_player_id: player,
                            your_color: color,
                            session_token,
                            players: public(&seats),
                        })
                        .await;
                    if !standings.is_empty() {
                        let _ = out.send(standings.message(&seats)).await;
                    }
                    broadcast_except(
                        &seats,
                        player,
                        ServerMessage::PlayerConnectionChanged {
                            player,
                            connected: true,
                        },
                    )
                    .await;
                    continue;
                }
                if seats.len() >= TABLE_SIZE {
                    let _ = out
                        .send(ServerMessage::Error {
                            code: ErrorCode::WrongPhase,
                            message: "group is full".into(),
                        })
                        .await;
                    continue;
                }
                let color = first_free_color(&seats);
                // A guest is ready immediately (they joined to play this game); a
                // member readies on join (and re-readies via PlayAgain after a game).
                seats.push(Seat {
                    id: player,
                    name,
                    color,
                    out: out.clone(),
                    guest,
                    ready: true,
                });
                let _ = out
                    .send(ServerMessage::GroupJoined {
                        group_code: code.clone(),
                        your_player_id: player,
                        your_color: color,
                        session_token,
                        players: public(&seats),
                    })
                    .await;
                if !standings.is_empty() {
                    let _ = out.send(standings.message(&seats)).await;
                }
                broadcast_except(
                    &seats,
                    player,
                    ServerMessage::PlayerConnectionChanged {
                        player,
                        connected: true,
                    },
                )
                .await;

                if ready_to_start(&seats) {
                    run_one_game(
                        &code,
                        &mut seats,
                        &mut standings,
                        &mut searching,
                        &groups,
                        &mut rx,
                        &registry,
                        &config,
                        &emote_palette,
                        pool.as_ref(),
                        &accounts,
                        &ratings,
                    )
                    .await;
                    idle.as_mut().reset(Instant::now() + IDLE_TIMEOUT);
                    if seats.is_empty() {
                        break;
                    }
                }
            }
            GroupCommand::Leave { player } => {
                seats.retain(|s| s.id != player);
                broadcast(
                    &seats,
                    ServerMessage::PlayerConnectionChanged {
                        player,
                        connected: false,
                    },
                )
                .await;
                if seats.is_empty() {
                    break; // last player gone — group task ends
                }
            }
            GroupCommand::Action { player, msg } => match msg {
                ClientMessage::Heartbeat => {
                    if let Some(seat) = seats.iter().find(|s| s.id == player) {
                        let _ = seat.out.send(ServerMessage::Heartbeat).await;
                    }
                }
                ClientMessage::Emote { emote } => {
                    if emote_palette.contains(&emote.0) {
                        broadcast(
                            &seats,
                            ServerMessage::EmoteBroadcast {
                                from: player,
                                emote,
                            },
                        )
                        .await;
                    } else if let Some(seat) = seats.iter().find(|s| s.id == player) {
                        let _ = seat
                            .out
                            .send(ServerMessage::Error {
                                code: ErrorCode::InvalidEmote,
                                message: "unknown emote".into(),
                            })
                            .await;
                    }
                }
                // Post-game opt-in: the seat readies up for another game; a fresh
                // game starts once the table is full and all seats have opted in.
                ClientMessage::PlayAgain => {
                    if let Some(seat) = seats.iter_mut().find(|s| s.id == player) {
                        seat.ready = true;
                    }
                    if ready_to_start(&seats) {
                        run_one_game(
                            &code,
                            &mut seats,
                            &mut standings,
                            &mut searching,
                            &groups,
                            &mut rx,
                            &registry,
                            &config,
                            &emote_palette,
                            pool.as_ref(),
                            &accounts,
                            &ratings,
                        )
                        .await;
                        idle.as_mut().reset(Instant::now() + IDLE_TIMEOUT);
                        if seats.is_empty() {
                            break;
                        }
                    }
                }
                // A member asks matchmaking to fill the empty seats with guests.
                ClientMessage::FillGroup => {
                    if seats.iter().any(|s| s.id == player)
                        && seats.len() < TABLE_SIZE
                        && !searching
                    {
                        let needed = TABLE_SIZE - seats.len();
                        let rating = anchor_rating(&seats, &accounts, &ratings);
                        groups
                            .open_fill(code.clone(), self_tx.clone(), needed, rating)
                            .await;
                        searching = true;
                        broadcast(
                            &seats,
                            ServerMessage::GroupSearching {
                                needed: needed as u8,
                            },
                        )
                        .await;
                    } else if let Some(seat) = seats.iter().find(|s| s.id == player) {
                        let _ = seat
                            .out
                            .send(ServerMessage::Error {
                                code: ErrorCode::WrongPhase,
                                message: "group is full or already searching".into(),
                            })
                            .await;
                    }
                }
                // Stop an in-progress fill search; back to the idle lobby.
                ClientMessage::CancelFill if searching => {
                    groups.close_fill(&code);
                    searching = false;
                    broadcast(&seats, ServerMessage::GroupSearching { needed: 0 }).await;
                }
                // Other gameplay actions outside a game (incl. a no-op CancelFill
                // when not searching) are ignored.
                _ => {}
            },
            GroupCommand::ForceStart => {
                if !seats.is_empty() {
                    run_one_game(
                        &code,
                        &mut seats,
                        &mut standings,
                        &mut searching,
                        &groups,
                        &mut rx,
                        &registry,
                        &config,
                        &emote_palette,
                        pool.as_ref(),
                        &accounts,
                        &ratings,
                    )
                    .await;
                    idle.as_mut().reset(Instant::now() + IDLE_TIMEOUT);
                    if seats.is_empty() {
                        break;
                    }
                }
            }
            GroupCommand::Shutdown => {
                tracing::info!(code = %code.0, "group killed by operator");
                break;
            }
        }
    }
    if searching {
        groups.close_fill(&code);
    }
    groups.remove(&code);
    crate::observability::metric::group_closed();
    tracing::debug!(code = %code.0, "group closed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn pid(n: u128) -> PlayerId {
        PlayerId(Uuid::from_u128(n))
    }

    /// A member win and a guest win land in the right standings buckets, and
    /// games-played counts members only (group-fill-and-standings tasks.md 7.3).
    #[test]
    fn standings_split_member_and_guest_results() {
        let (a, b, c, guest) = (pid(1), pid(2), pid(3), pid(99));
        let roster = [(a, false), (b, false), (c, false), (guest, true)];
        let mut s = Standings::default();
        // Game 1: member `a` wins.
        s.record_game(&roster, &[a]);
        // Game 2: the guest wins.
        s.record_game(&roster, &[guest]);

        assert_eq!(s.members[&a].games_played, 2);
        assert_eq!(s.members[&a].wins, 1, "member win counted for the member");
        assert_eq!(s.members[&b].games_played, 2);
        assert_eq!(s.members[&b].wins, 0);
        assert_eq!(s.guest_games, 2, "both games included a guest");
        assert_eq!(
            s.guest_wins, 1,
            "the guest win rolled into the guest bucket"
        );
        assert!(
            !s.members.contains_key(&guest),
            "a guest never gets a per-member entry"
        );
    }

    /// Deathmatch co-champions each earn a win.
    #[test]
    fn standings_co_champions_each_win() {
        let (a, b) = (pid(1), pid(2));
        let mut s = Standings::default();
        s.record_game(&[(a, false), (b, false)], &[a, b]);
        assert_eq!(s.members[&a].wins, 1);
        assert_eq!(s.members[&b].wins, 1);
    }
}
