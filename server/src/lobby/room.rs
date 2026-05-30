//! The room task: a single async owner of one room's state.
//!
//! Each connection talks to the room only through [`RoomCommand`]s on an mpsc
//! channel; the room pushes [`ServerMessage`]s back out through each seat's own
//! mpsc sender. This keeps the room the sole writer of its state (no locks). For
//! this milestone the room handles the lobby (join, leave, auto-start at four,
//! heartbeat, emotes); the in-room game loop is wired in a later task.

use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::mpsc;

use boiling_point_protocol::server::{ErrorCode, PlayerPublic};
use boiling_point_protocol::vocab::Color;
use boiling_point_protocol::{ClientMessage, PlayerId, RoomCode, ServerMessage};

use crate::config::ROUND_COUNT;

/// Exactly four players to a table.
const TABLE_SIZE: usize = 4;

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
pub fn spawn(code: RoomCode, emote_palette: Arc<HashSet<u16>>) -> RoomHandle {
    let (tx, rx) = mpsc::channel(64);
    tokio::spawn(run(code.clone(), rx, emote_palette));
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

async fn run(
    code: RoomCode,
    mut rx: mpsc::Receiver<RoomCommand>,
    emote_palette: Arc<HashSet<u16>>,
) {
    let mut seats: Vec<Seat> = Vec::new();
    let mut started = false;

    while let Some(cmd) = rx.recv().await {
        match cmd {
            RoomCommand::Join { player, name, out } => {
                if started || seats.len() >= TABLE_SIZE {
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
                    started = true;
                    broadcast(
                        &seats,
                        ServerMessage::GameStarting {
                            players: public(&seats),
                            round_count: ROUND_COUNT,
                        },
                    )
                    .await;
                    // TODO: drive the in-room game loop (later task).
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
        }
    }
    let _ = code; // retained for logging/metrics once observability lands
}
