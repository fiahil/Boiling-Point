//! The room task: a single async owner of one room's state.
//!
//! Each connection talks to the room only through [`RoomCommand`]s on an mpsc
//! channel; the room pushes [`ServerMessage`]s back out through each seat's own
//! mpsc sender. This keeps the room the sole writer of its state (no locks). The
//! room serves the lobby (join, leave, heartbeat, emotes), auto-starts at four
//! and hands off to the in-room game loop (`session::run_game`), enforces an idle
//! timeout, and deregisters itself from the registry when it ends.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use boiling_point_protocol::server::{ErrorCode, PlayerPublic};
use boiling_point_protocol::vocab::Color;
use boiling_point_protocol::{ClientMessage, PlayerId, RoomCode, ServerMessage};

use crate::config::{ContentConfig, ROUND_COUNT};
use crate::content::ContentRegistry;
use crate::session::{self, SeatInfo};

use super::registry::RoomRegistry;

/// Exactly four players to a table.
const TABLE_SIZE: usize = 4;
/// A room sitting in the lobby this long without starting is destroyed.
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// A command delivered to a room's task by a connection.
pub enum RoomCommand {
    /// A player joins, providing the outbound channel to reach them.
    Join {
        /// The joining player's id.
        player: PlayerId,
        /// Their display name.
        name: String,
        /// Channel the room uses to send this player messages.
        out: mpsc::Sender<ServerMessage>,
    },
    /// A player's connection dropped.
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
    /// Admin command-plane: tear this room down (kill an idle or stuck room). The
    /// room's task ends, deregisters, and its `room.lifetime` span closes.
    Shutdown,
}

/// A handle to a running room: its code and the channel to command it.
#[derive(Clone)]
pub struct RoomHandle {
    /// The room's invite code.
    pub code: RoomCode,
    /// Channel to send the room commands.
    pub tx: mpsc::Sender<RoomCommand>,
}

/// One seated player within a room.
struct Seat {
    id: PlayerId,
    name: String,
    color: Color,
    out: mpsc::Sender<ServerMessage>,
}

/// Spawn a room task and return a handle to it.
pub fn spawn(
    code: RoomCode,
    rooms: Arc<RoomRegistry>,
    registry: Arc<ContentRegistry>,
    config: Arc<ContentConfig>,
    emote_palette: Arc<HashSet<u16>>,
) -> RoomHandle {
    let (tx, rx) = mpsc::channel(64);
    tokio::spawn(run(
        code.clone(),
        rx,
        rooms,
        registry,
        config,
        emote_palette,
    ));
    RoomHandle { code, tx }
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

/// Announce the game and drive it to completion for the currently seated players.
/// Shared by the auto-start (table full) and operator force-start paths.
async fn start_game(
    code: &RoomCode,
    seats: &[Seat],
    rx: &mut mpsc::Receiver<RoomCommand>,
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
    session::run_game(
        registry.as_ref(),
        config.as_ref(),
        code.clone(),
        seat_infos,
        rx,
        emote_palette.as_ref(),
        seed,
    )
    .await;
}

// `room.lifetime` (span_schema::span::ROOM_LIFETIME) — the outermost span; the
// live open-span registry keys off it. Instrumenting the whole task makes every
// child span (game → round → wave …) nest under this room. The field name matches
// `span_schema::attr::ROOM_CODE`.
#[tracing::instrument(name = "room.lifetime", skip_all, fields(room.code = %code.0))]
async fn run(
    code: RoomCode,
    mut rx: mpsc::Receiver<RoomCommand>,
    rooms: Arc<RoomRegistry>,
    registry: Arc<ContentRegistry>,
    config: Arc<ContentConfig>,
    emote_palette: Arc<HashSet<u16>>,
) {
    let mut seats: Vec<Seat> = Vec::new();

    // Lobby loop: serve commands until the game starts, everyone leaves, or the
    // room sits idle past the timeout.
    let idle = tokio::time::sleep(IDLE_TIMEOUT);
    tokio::pin!(idle);
    loop {
        let cmd = tokio::select! {
            _ = &mut idle => {
                tracing::info!(code = %code.0, "room idle timeout — destroying");
                break;
            }
            cmd = rx.recv() => match cmd {
                Some(c) => c,
                None => break,
            },
        };
        match cmd {
            RoomCommand::Join { player, name, out } => {
                if seats.len() >= TABLE_SIZE {
                    let _ = out
                        .send(ServerMessage::Error {
                            code: ErrorCode::WrongPhase,
                            message: "room is full or already started".into(),
                        })
                        .await;
                    continue;
                }
                let color = Color::PLAYER_COLORS[seats.len()];
                seats.push(Seat {
                    id: player,
                    name,
                    color,
                    out: out.clone(),
                });
                let _ = out
                    .send(ServerMessage::RoomJoined {
                        room_code: code.clone(),
                        your_player_id: player,
                        your_color: color,
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

                if seats.len() == TABLE_SIZE {
                    // Drive the whole game over the wire, then end the room.
                    start_game(&code, &seats, &mut rx, &registry, &config, &emote_palette).await;
                    break;
                }
            }
            RoomCommand::Leave { player } => {
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
                    break; // last player gone — room task ends
                }
            }
            RoomCommand::Action { player, msg } => match msg {
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
                // Gameplay actions are handled once the in-room game loop is wired.
                _ => {}
            },
            RoomCommand::ForceStart => {
                if !seats.is_empty() {
                    start_game(&code, &seats, &mut rx, &registry, &config, &emote_palette).await;
                    break;
                }
            }
            RoomCommand::Shutdown => {
                tracing::info!(code = %code.0, "room killed by operator");
                break;
            }
        }
    }
    rooms.remove(&code);
    crate::observability::metric::room_closed();
    tracing::debug!(code = %code.0, "room closed");
}
