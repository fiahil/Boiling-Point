//! The auto-match queue: an **anchor-and-fill** matcher.
//!
//! Two intents share one solo pool. A **solo** player enqueues to find any table;
//! a partial **group** (an *anchor*) opens for fill via [`MatchQueue::open_fill`].
//! Waiting solos backfill an anchor's empty seats as **guests** (first-come); solos
//! with no anchor to fill still assemble into a fresh group of four (all members).
//! A waiting player parks on a oneshot channel until placed, then is handed the
//! group's command channel.
//!
//! The *ordering* of who fills which seat / who assembles together is delegated
//! to a swappable [`MatchPolicy`] (`boom2-identity`, [design D3]): the default
//! [`FirstCome`] reproduces v1 exactly, while the production [`SkillBased`]
//! policy groups similar ratings and falls back to first-come for unrated play.
//! The queue resolves each solo's rating from the shared account + rating stores
//! before consulting the policy, so the policy stays a pure ordering decision.

use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};

use boiling_point_protocol::{GroupCode, PlayerId, ServerMessage};

use super::accounts::AccountStore;
use super::group::GroupCommand;
use super::policy::{Candidate, FirstCome, MatchPolicy};
use super::registry::GroupRegistry;
use crate::rating::RatingStore;

/// Table size for matchmaking.
const GROUP_SIZE: usize = 4;

/// A player waiting in the queue, with the channel to wake them once matched.
struct Pending {
    player: PlayerId,
    name: String,
    session_token: String,
    /// The player's conservative rating if they signed in to a rated account,
    /// else `None` (anonymous). Resolved once at enqueue; drives the policy.
    rating: Option<i32>,
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
    /// The searching group's members' mean rating, or `None` if unrated; lets
    /// the skill policy pull a similarly-rated guest.
    rating: Option<i32>,
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
    /// The ordering policy (default [`FirstCome`]).
    policy: Arc<dyn MatchPolicy>,
    /// Account + rating stores, to resolve a queued player's rating. Default
    /// (empty) stores leave every solo unrated, so the default queue is
    /// first-come regardless of policy.
    accounts: Arc<AccountStore>,
    ratings: Arc<RatingStore>,
}

impl MatchQueue {
    /// Create a queue that assembles groups via `groups`, with the v1 first-come
    /// policy and no identity stores (every solo unrated). The default used by
    /// tests and any deployment that hasn't enabled skill-based matchmaking.
    pub fn new(groups: Arc<GroupRegistry>) -> Self {
        MatchQueue {
            groups,
            inner: Mutex::new(Inner::default()),
            policy: Arc::new(FirstCome),
            accounts: Arc::new(AccountStore::new()),
            ratings: Arc::new(RatingStore::default()),
        }
    }

    /// Create a queue with a specific matching policy and the shared identity
    /// stores it reads ratings from. Production wires
    /// [`SkillBased`](super::policy::SkillBased) plus the server's account and
    /// rating stores so rated solos are grouped by skill.
    pub fn with_identity(
        groups: Arc<GroupRegistry>,
        policy: Arc<dyn MatchPolicy>,
        accounts: Arc<AccountStore>,
        ratings: Arc<RatingStore>,
    ) -> Self {
        MatchQueue {
            groups,
            inner: Mutex::new(Inner::default()),
            policy,
            accounts,
            ratings,
        }
    }

    /// The active policy's label (metrics/tests).
    pub fn policy_name(&self) -> &'static str {
        self.policy.name()
    }

    /// A queued player's conservative rating, if they have an account.
    fn rating_of(&self, player: PlayerId) -> Option<i32> {
        self.accounts
            .account_for_player(player)
            .map(|a| self.ratings.view(a.id).display)
    }

    /// Enqueue a solo player. If a group is open for fill, the policy picks which
    /// waiting solo backfills it as a guest; otherwise the player joins the solo
    /// pool and, once four are waiting, the policy picks four to form a fresh
    /// group (all members). Sends happen outside the lock.
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

        let rating = self.rating_of(player);
        let action = {
            let mut inner = self.inner.lock().expect("queue mutex");
            inner.solos.push(Pending {
                player,
                name,
                session_token,
                rating,
                out,
                notify,
                _wait_span: tracing::info_span!("lobby.wait", player.id = %player.0),
            });
            // Prefer to fill the oldest waiting anchor: the policy chooses which
            // queued solo (best fit for that anchor) takes the seat.
            if let Some(ai) = inner.anchors.iter().position(|a| a.needed > 0) {
                let anchor_rating = inner.anchors[ai].rating;
                let candidates = candidates(&inner.solos);
                let pick = self.policy.pick_for_anchor(&candidates, anchor_rating);
                let pending = inner.solos.remove(pick);
                inner.anchors[ai].needed -= 1;
                let tx = inner.anchors[ai].tx.clone();
                if inner.anchors[ai].needed == 0 {
                    inner.anchors.remove(ai);
                }
                Action::Guest(tx, pending)
            } else if inner.solos.len() >= GROUP_SIZE {
                let candidates = candidates(&inner.solos);
                let mut idxs = self.policy.pick_table(&candidates);
                // Remove in descending index order so earlier removals don't
                // shift the indices still to be removed.
                idxs.sort_unstable_by(|a, b| b.cmp(a));
                let four = idxs.into_iter().map(|i| inner.solos.remove(i)).collect();
                Action::Fresh(four)
            } else {
                Action::Park
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

    /// Open `code` for fill, needing `needed` more players, with the searching
    /// group's mean `anchor_rating` (or `None` if unrated). Any solos already
    /// waiting backfill it immediately — the policy picks the best fits — and the
    /// rest of the need is registered so later solos fill it. Sends happen
    /// outside the lock.
    pub async fn open_fill(
        &self,
        code: GroupCode,
        tx: mpsc::Sender<GroupCommand>,
        needed: usize,
        anchor_rating: Option<i32>,
    ) {
        let to_place: Vec<Pending> = {
            let mut inner = self.inner.lock().expect("queue mutex");
            let take = needed.min(inner.solos.len());
            let mut pulled = Vec::with_capacity(take);
            for _ in 0..take {
                if inner.solos.is_empty() {
                    break;
                }
                let candidates = candidates(&inner.solos);
                let pick = self.policy.pick_for_anchor(&candidates, anchor_rating);
                pulled.push(inner.solos.remove(pick));
            }
            let remaining = needed - pulled.len();
            // Drop any stale prior registration for this code, then register the
            // remaining need (if any).
            inner.anchors.retain(|a| a.code != code);
            if remaining > 0 {
                inner.anchors.push(Anchor {
                    code,
                    tx: tx.clone(),
                    needed: remaining,
                    rating: anchor_rating,
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

/// The policy's view of the current solo pool, in pool order.
fn candidates(solos: &[Pending]) -> Vec<Candidate> {
    solos
        .iter()
        .map(|p| Candidate { rating: p.rating })
        .collect()
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
