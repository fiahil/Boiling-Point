//! The auto-match queue: an **anchor-and-fill** matcher.
//!
//! Two intents share one solo pool. A **solo** player enqueues to find any table;
//! a partial **group** (an *anchor*) opens for fill via [`MatchQueue::open_fill`].
//! Waiting solos backfill an anchor's empty seats as **guests** (first-come); solos
//! with no anchor to fill still assemble into a fresh group of four (all members).
//! A waiting player parks on a oneshot channel until placed, then is handed the
//! group's command channel.

use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};

use boiling_point_protocol::{GroupCode, PlayerId, ServerMessage};

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

/// A partial group open for fill: solos placed into it join as guests.
struct Anchor {
    code: GroupCode,
    tx: mpsc::Sender<GroupCommand>,
    /// Remaining guest seats to fill before the table is full.
    needed: usize,
}

/// The queue's protected state: the solo pool and the open fill anchors.
#[derive(Default)]
struct Inner {
    solos: Vec<Pending>,
    anchors: Vec<Anchor>,
}

/// The shared auto-match queue.
pub struct MatchQueue {
    groups: Arc<GroupRegistry>,
    inner: Mutex<Inner>,
}

impl MatchQueue {
    /// Create a queue that assembles groups via `groups`.
    pub fn new(groups: Arc<GroupRegistry>) -> Self {
        MatchQueue {
            groups,
            inner: Mutex::new(Inner::default()),
        }
    }

    /// Enqueue a solo player. If a group is open for fill, the player backfills it
    /// as a guest; otherwise they join the solo pool and, once four solos are
    /// waiting, a fresh group (all members) is formed. Sends happen outside the
    /// lock.
    pub async fn enqueue(
        &self,
        player: PlayerId,
        name: String,
        session_token: String,
        out: mpsc::Sender<ServerMessage>,
        notify: oneshot::Sender<mpsc::Sender<GroupCommand>>,
    ) {
        enum Action {
            /// Backfill an anchor group as a guest.
            Guest(mpsc::Sender<GroupCommand>, Pending),
            /// Form a fresh group of four members.
            Fresh(Vec<Pending>),
            /// Park and wait.
            Park,
        }

        let action = {
            let mut inner = self.inner.lock().expect("queue mutex");
            let pending = Pending {
                player,
                name,
                session_token,
                out,
                notify,
                _wait_span: tracing::info_span!("lobby.wait", player.id = %player.0),
            };
            // Prefer to fill a waiting anchor group.
            if let Some(i) = inner.anchors.iter().position(|a| a.needed > 0) {
                inner.anchors[i].needed -= 1;
                let tx = inner.anchors[i].tx.clone();
                if inner.anchors[i].needed == 0 {
                    inner.anchors.remove(i);
                }
                Action::Guest(tx, pending)
            } else {
                inner.solos.push(pending);
                if inner.solos.len() >= GROUP_SIZE {
                    Action::Fresh(inner.solos.drain(0..GROUP_SIZE).collect())
                } else {
                    Action::Park
                }
            }
        };

        match action {
            Action::Guest(tx, pending) => place_guest(&tx, pending).await,
            Action::Fresh(four) => {
                let (_code, tx) = self.groups.create();
                for pending in four {
                    place_member(&tx, pending).await;
                }
            }
            Action::Park => {}
        }
    }

    /// Open `code` for fill, needing `needed` more players. Any solos already
    /// waiting backfill it immediately (as guests); the rest of the need is
    /// registered so later solos fill it. Sends happen outside the lock.
    pub async fn open_fill(&self, code: GroupCode, tx: mpsc::Sender<GroupCommand>, needed: usize) {
        let to_place: Vec<Pending> = {
            let mut inner = self.inner.lock().expect("queue mutex");
            let take = needed.min(inner.solos.len());
            let pulled: Vec<Pending> = inner.solos.drain(0..take).collect();
            let remaining = needed - take;
            // Drop any stale prior registration for this code, then register the
            // remaining need (if any).
            inner.anchors.retain(|a| a.code != code);
            if remaining > 0 {
                inner.anchors.push(Anchor {
                    code,
                    tx: tx.clone(),
                    needed: remaining,
                });
            }
            pulled
        };
        for pending in to_place {
            place_guest(&tx, pending).await;
        }
    }

    /// Stop filling `code` (it started a game or cancelled the search).
    pub fn close_fill(&self, code: &GroupCode) {
        self.inner
            .lock()
            .expect("queue mutex")
            .anchors
            .retain(|a| &a.code != code);
    }

    /// Number of solo players currently waiting (for metrics/tests).
    pub fn waiting(&self) -> usize {
        self.inner.lock().expect("queue mutex").solos.len()
    }
}

/// Join a pending player into a group as a member, then wake them with the channel.
async fn place_member(tx: &mpsc::Sender<GroupCommand>, pending: Pending) {
    join_and_notify(tx, pending, false).await;
}

/// Join a pending player into a group as a guest, then wake them with the channel.
async fn place_guest(tx: &mpsc::Sender<GroupCommand>, pending: Pending) {
    join_and_notify(tx, pending, true).await;
}

async fn join_and_notify(tx: &mpsc::Sender<GroupCommand>, pending: Pending, guest: bool) {
    let _ = tx
        .send(GroupCommand::Join {
            player: pending.player,
            name: pending.name,
            session_token: pending.session_token,
            guest,
            out: pending.out,
        })
        .await;
    let _ = pending.notify.send(tx.clone());
}
