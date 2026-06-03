//! The group task: a single async owner of one group's state.
//!
//! Each connection talks to the group only through [`GroupCommand`]s on an mpsc
//! channel; the group pushes [`ServerMessage`]s back out through each seat's own
//! mpsc sender. This keeps the group the sole writer of its state (no locks).
//!
//! A **group persists across games** (group-model D2/D3): it serves the lobby
//! (join, leave, heartbeat, emotes), starts a game once it holds 4 ready players,
//! drives it via `session::run_game`, then returns the survivors to the lobby — the
//! group keeps its code and roster between games. Players opt back in with
//! [`ClientMessage::PlayAgain`]; a fresh game starts when 4 are ready again. The
//! group is destroyed only when it empties, sits idle past the timeout, or an
//! operator kills it, at which point it deregisters from the registry.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::Instant;

use boiling_point_protocol::server::{ErrorCode, PlayerPublic};
use boiling_point_protocol::vocab::Color;
use boiling_point_protocol::{ClientMessage, GroupCode, PlayerId, ServerMessage};

use crate::config::{ContentConfig, ROUND_COUNT};
use crate::content::ContentRegistry;
use crate::session::{self, GameEnd, SeatInfo};

use super::registry::GroupRegistry;

/// Exactly four players to a table.
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
    /// Whether this seat has opted in to (the next) game. Set on join and on
    /// `PlayAgain`; cleared after each game so play-again is an explicit opt-in.
    ready: bool,
}

/// Spawn a group task and return a handle to it.
pub fn spawn(
    code: GroupCode,
    groups: Arc<GroupRegistry>,
    registry: Arc<ContentRegistry>,
    config: Arc<ContentConfig>,
    emote_palette: Arc<HashSet<u16>>,
) -> GroupHandle {
    let (tx, rx) = mpsc::channel(64);
    tokio::spawn(run(
        code.clone(),
        rx,
        groups,
        registry,
        config,
        emote_palette,
    ));
    GroupHandle { code, tx }
}

fn public(seats: &[Seat]) -> Vec<PlayerPublic> {
    seats
        .iter()
        .map(|s| PlayerPublic {
            id: s.id,
            display_name: s.name.clone(),
            color: s.color,
            connected: true,
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

/// Announce and drive one full game to completion for the currently seated
/// players, then return the survivors to the lobby: any player who left/abandoned
/// and never reconnected is dropped, reconnected players keep their refreshed
/// channel, and every remaining seat is reset to not-ready (play-again is an
/// explicit opt-in). Shared by the auto-start and operator force-start paths.
async fn run_one_game(
    code: &GroupCode,
    seats: &mut Vec<Seat>,
    rx: &mut mpsc::Receiver<GroupCommand>,
    registry: &Arc<ContentRegistry>,
    config: &Arc<ContentConfig>,
    emote_palette: &Arc<HashSet<u16>>,
) {
    broadcast(
        seats,
        ServerMessage::GameStarting {
            players: public(seats),
            round_count: ROUND_COUNT,
        },
    )
    .await;
    let seat_infos: Vec<SeatInfo> = seats
        .iter()
        .map(|s| SeatInfo {
            id: s.id,
            name: s.name.clone(),
            color: s.color,
            out: s.out.clone(),
        })
        .collect();
    let seed: u64 = rand::random();
    let GameEnd { players, gone } = session::run_game(
        registry.as_ref(),
        config.as_ref(),
        code.clone(),
        seat_infos,
        rx,
        emote_palette.as_ref(),
        seed,
    )
    .await;
    // Rebuild the roster from the game's final seats (reconnects refreshed their
    // `out` channel mid-game), dropping anyone still gone, and clear ready flags.
    *seats = players
        .into_iter()
        .filter(|p| !gone.contains(&p.id))
        .map(|p| Seat {
            id: p.id,
            name: p.name,
            color: p.color,
            out: p.out,
            ready: false,
        })
        .collect();
}

// `group.lifetime` (span_schema::span::GROUP_LIFETIME) — the outermost span; the
// live open-span registry keys off it. Instrumenting the whole task makes every
// child span (game → round → wave …) nest under this group, and a persistent group
// nests several `game` spans over its life. The field name matches
// `span_schema::attr::GROUP_CODE`.
#[tracing::instrument(name = "group.lifetime", skip_all, fields(group.code = %code.0))]
async fn run(
    code: GroupCode,
    mut rx: mpsc::Receiver<GroupCommand>,
    groups: Arc<GroupRegistry>,
    registry: Arc<ContentRegistry>,
    config: Arc<ContentConfig>,
    emote_palette: Arc<HashSet<u16>>,
) {
    let mut seats: Vec<Seat> = Vec::new();

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
                seats.push(Seat {
                    id: player,
                    name,
                    color,
                    out: out.clone(),
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
                        &mut rx,
                        &registry,
                        &config,
                        &emote_palette,
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
                // game starts once all four seats have opted in.
                ClientMessage::PlayAgain => {
                    if let Some(seat) = seats.iter_mut().find(|s| s.id == player) {
                        seat.ready = true;
                    }
                    if ready_to_start(&seats) {
                        run_one_game(
                            &code,
                            &mut seats,
                            &mut rx,
                            &registry,
                            &config,
                            &emote_palette,
                        )
                        .await;
                        idle.as_mut().reset(Instant::now() + IDLE_TIMEOUT);
                        if seats.is_empty() {
                            break;
                        }
                    }
                }
                // Gameplay actions outside a game are no-ops (the in-game loop owns them).
                _ => {}
            },
            GroupCommand::ForceStart => {
                if !seats.is_empty() {
                    run_one_game(
                        &code,
                        &mut seats,
                        &mut rx,
                        &registry,
                        &config,
                        &emote_palette,
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
    groups.remove(&code);
    crate::observability::metric::group_closed();
    tracing::debug!(code = %code.0, "group closed");
}
