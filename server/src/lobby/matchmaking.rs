//! The auto-match queue: players enqueue and are assembled into tables of four.
//!
//! Enqueuing is decoupled from room assignment: a waiting player parks on a
//! oneshot channel until the fourth player arrives, at which point a room is
//! created, all four are joined, and each is handed the room's command channel.

use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};

use boiling_point_protocol::{PlayerId, ServerMessage};

use super::registry::RoomRegistry;
use super::room::RoomCommand;

/// Table size for matchmaking.
const GROUP_SIZE: usize = 4;

/// A player waiting in the queue, with the channel to wake them once matched.
struct Pending {
    player: PlayerId,
    name: String,
    out: mpsc::Sender<ServerMessage>,
    notify: oneshot::Sender<mpsc::Sender<RoomCommand>>,
    /// `lobby.wait` span, open while the player waits; dropped (closed) once they
    /// are matched, so the live count of open `lobby.wait` spans is the queue depth.
    _wait_span: tracing::Span,
}

/// The shared auto-match queue.
pub struct MatchQueue {
    rooms: Arc<RoomRegistry>,
    waiting: Mutex<Vec<Pending>>,
}

impl MatchQueue {
    /// Create a queue that assembles rooms via `rooms`.
    pub fn new(rooms: Arc<RoomRegistry>) -> Self {
        MatchQueue {
            rooms,
            waiting: Mutex::new(Vec::new()),
        }
    }

    /// Enqueue a player. When this call completes the table of four (if it does),
    /// it creates a room, joins all four, and wakes each via their `notify`
    /// channel; otherwise the caller simply awaits its own `notify`.
    pub async fn enqueue(
        &self,
        player: PlayerId,
        name: String,
        out: mpsc::Sender<ServerMessage>,
        notify: oneshot::Sender<mpsc::Sender<RoomCommand>>,
    ) {
        // Take a full group under the lock (never held across an await).
        let group = {
            let mut waiting = self.waiting.lock().expect("queue mutex");
            waiting.push(Pending {
                player,
                name,
                out,
                notify,
                _wait_span: tracing::info_span!("lobby.wait", player.id = %player.0),
            });
            if waiting.len() >= GROUP_SIZE {
                Some(waiting.drain(0..GROUP_SIZE).collect::<Vec<_>>())
            } else {
                None
            }
        };

        if let Some(group) = group {
            let (_code, room_tx) = self.rooms.create();
            for pending in group {
                let _ = room_tx
                    .send(RoomCommand::Join {
                        player: pending.player,
                        name: pending.name,
                        out: pending.out,
                    })
                    .await;
                let _ = pending.notify.send(room_tx.clone());
            }
        }
    }

    /// Number of players currently waiting (for metrics/tests).
    pub fn waiting(&self) -> usize {
        self.waiting.lock().expect("queue mutex").len()
    }
}
