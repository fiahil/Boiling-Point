//! The auto-match queue: players enqueue and are assembled into tables of four.
//!
//! Enqueuing is decoupled from group assignment: a waiting player parks on a
//! oneshot channel until the fourth player arrives, at which point a group is
//! created, all four are joined, and each is handed the group's command channel.

use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};

use boiling_point_protocol::{PlayerId, ServerMessage};

use super::group::GroupCommand;
use super::registry::GroupRegistry;

/// Table size for matchmaking.
const GROUP_SIZE: usize = 4;

/// A player waiting in the queue, with the channel to wake them once matched.
struct Pending {
    player: PlayerId,
    name: String,
    session_token: String,
    out: mpsc::Sender<ServerMessage>,
    notify: oneshot::Sender<mpsc::Sender<GroupCommand>>,
    /// `lobby.wait` span, open while the player waits; dropped (closed) once they
    /// are matched, so the live count of open `lobby.wait` spans is the queue depth.
    _wait_span: tracing::Span,
}

/// The shared auto-match queue.
pub struct MatchQueue {
    groups: Arc<GroupRegistry>,
    waiting: Mutex<Vec<Pending>>,
}

impl MatchQueue {
    /// Create a queue that assembles groups via `groups`.
    pub fn new(groups: Arc<GroupRegistry>) -> Self {
        MatchQueue {
            groups,
            waiting: Mutex::new(Vec::new()),
        }
    }

    /// Enqueue a player. When this call completes the table of four (if it does),
    /// it creates a group, joins all four, and wakes each via their `notify`
    /// channel; otherwise the caller simply awaits its own `notify`.
    pub async fn enqueue(
        &self,
        player: PlayerId,
        name: String,
        session_token: String,
        out: mpsc::Sender<ServerMessage>,
        notify: oneshot::Sender<mpsc::Sender<GroupCommand>>,
    ) {
        // Take a full group under the lock (never held across an await).
        let group = {
            let mut waiting = self.waiting.lock().expect("queue mutex");
            waiting.push(Pending {
                player,
                name,
                session_token,
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
            let (_code, group_tx) = self.groups.create();
            for pending in group {
                let _ = group_tx
                    .send(GroupCommand::Join {
                        player: pending.player,
                        name: pending.name,
                        session_token: pending.session_token,
                        out: pending.out,
                    })
                    .await;
                let _ = pending.notify.send(group_tx.clone());
            }
        }
    }

    /// Number of players currently waiting (for metrics/tests).
    pub fn waiting(&self) -> usize {
        self.waiting.lock().expect("queue mutex").len()
    }
}
