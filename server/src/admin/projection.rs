//! The admin **span projection**: an in-process read model built *solely* by
//! consuming the server's span lifecycle (`admin-span-projection`).
//!
//! It maintains three things, all derived from spans and nothing else:
//! - a **live open-span registry** (current rooms/games/rounds/waves + the queue),
//!   the source for the room inspector and the privileged reveal;
//! - **unsampled rolling aggregates** folded from completed spans (the balance
//!   figures), computed upstream of any export sampling so they reflect 100% of
//!   completed rounds/games;
//! - a **bounded replay buffer** of recent completed games (wave-by-wave).
//!
//! It is **read-only by construction**: it holds no handle to game state, only the
//! owned [`SpanEvent`]s the lifecycle hook hands it, and a slow projection drops
//! events rather than backpressuring the game (the drop happens in the lifecycle
//! channel; see [`crate::observability::lifecycle`]).

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::RwLock;
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::sync::broadcast;

use crate::config::ROUND_COUNT;
use crate::observability::lifecycle::{SpanConsumer, SpanEvent, SpanEventKind};
use crate::observability::span_schema::{self, SPAN_SCHEMA_VERSION, attr, span};

/// Recent completed games retained for replay (oldest evicted past this).
const REPLAY_CAPACITY: usize = 64;
/// Capacity of the live activity broadcast channel (SSE feed).
const LIVE_CHANNEL_CAPACITY: usize = 512;
/// Default margin a wave may exceed its timer budget before a room is flagged
/// stuck. **Needs playtesting** against real wave-timer budgets (design Open Q).
const DEFAULT_STUCK_WAVE_MARGIN_MS: u64 = 15_000;

/// One currently-open span tracked by the registry.
struct OpenSpan {
    name: &'static str,
    attributes: BTreeMap<String, String>,
    started: Instant,
    /// The enclosing `room.lifetime` span id (itself, if this *is* the room).
    room_id: Option<u64>,
    /// The enclosing `game` span id (itself, if this *is* the game).
    game_id: Option<u64>,
}

/// A completed span retained for replay, with its place in the tree and duration.
#[derive(Clone, Serialize)]
pub struct CompletedSpan {
    /// Span id (lets the replay viewer rebuild the tree).
    pub id: u64,
    /// Parent span id within the game.
    pub parent_id: Option<u64>,
    /// Span name (`round`, `wave`, `commit`, `resolve`, `score`, …).
    pub name: String,
    /// The span's final attributes (including secrets, held in-process only).
    pub attributes: BTreeMap<String, String>,
    /// How long the span was open, in milliseconds.
    pub duration_ms: u64,
}

/// Per-game accumulation of completed child spans, kept until the game span ends.
struct GameAccumulator {
    room_code: Option<String>,
    spans: Vec<CompletedSpan>,
}

/// A completed game retained in the bounded replay buffer.
#[derive(Clone, Serialize)]
pub struct ReplayGame {
    /// The game's id (from the `game.id` attribute).
    pub game_id: String,
    /// The room the game ran in, if known.
    pub room_code: Option<String>,
    /// Total game duration in milliseconds.
    pub duration_ms: u64,
    /// The game's descendant spans in completion order (rounds → waves → commits
    /// → resolve → score), enough to replay it wave by wave.
    pub spans: Vec<CompletedSpan>,
}

/// A short index entry for the replay list.
#[derive(Clone, Serialize)]
pub struct ReplaySummary {
    /// The game's id.
    pub game_id: String,
    /// The room the game ran in, if known.
    pub room_code: Option<String>,
    /// Total game duration in milliseconds.
    pub duration_ms: u64,
    /// Number of retained spans (a rough size signal).
    pub span_count: usize,
}

/// Raw unsampled counters folded from completed spans.
#[derive(Default, Clone)]
struct Aggregates {
    games: u64,
    rounds: u64,
    explosions: u64,
    waves: u64,
    wave_timeouts: u64,
    commits: u64,
    reconnects: u64,
    dominations: u64,
    splits: u64,
    total_round_duration_ms: u64,
    total_game_duration_ms: u64,
}

/// The Principle IV balance figures, all derived from the **unsampled** in-process
/// aggregates (never a sampled trace). Rates are fractions in `[0, 1]`.
#[derive(Clone, Serialize)]
pub struct BalanceFigures {
    /// Completed games observed.
    pub games: u64,
    /// Completed rounds observed.
    pub rounds: u64,
    /// Explosions / rounds (compare against the ~0.30–0.40 design target).
    pub explosion_rate: f64,
    /// Mean round duration in milliseconds.
    pub avg_round_duration_ms: f64,
    /// Mean game duration in milliseconds.
    pub avg_game_duration_ms: f64,
    /// Mean cards committed per round (commits / rounds).
    pub cards_per_round: f64,
    /// Waves that closed on the timer / total waves.
    pub wave_timeout_rate: f64,
    /// Reconnections / game.
    pub reconnection_rate: f64,
    /// Single-colour dominations / decided (non-explosion) rounds — a
    /// dominant-strategy signal.
    pub dominant_color_rate: f64,
    /// The span-schema version the projection was built against.
    pub schema_version: u32,
}

/// A live fleet summary.
#[derive(Clone, Serialize)]
pub struct FleetSummary {
    /// Live rooms (open `room.lifetime` spans).
    pub rooms: usize,
    /// In-flight games (open `game` spans).
    pub games: usize,
    /// Players waiting in the auto-match queue (open `lobby.wait` spans).
    pub queue_depth: usize,
    /// Rooms currently flagged stuck/anomalous.
    pub stuck_rooms: usize,
    /// The current balance figures.
    pub balance: BalanceFigures,
}

/// A live room as the inspector shows it, derived from the room's open spans.
#[derive(Clone, Serialize)]
pub struct RoomView {
    /// Registry key (the room's `room.lifetime` span id).
    pub room_id: u64,
    /// Invite code, if the span carried one.
    pub room_code: Option<String>,
    /// Current phase: `lobby`, `game`, `round`, or `wave` (deepest open child).
    pub phase: String,
    /// Current round number, if a round is open.
    pub round_number: Option<u64>,
    /// Rounds per game (from config).
    pub round_total: u8,
    /// Current wave number, if a wave is open.
    pub wave_number: Option<u64>,
    /// Seated players, if a game is open.
    pub players: Option<u64>,
    /// Room age in milliseconds.
    pub age_ms: u64,
    /// Age of the currently-open wave, if any.
    pub last_wave_age_ms: Option<u64>,
    /// Whether the room is flagged stuck/anomalous.
    pub stuck: bool,
    /// Why it is flagged, if it is.
    pub stuck_reason: Option<String>,
}

/// One seated player's hand, read from an open `hand` span (secret in-process).
#[derive(Clone, Serialize)]
pub struct HandReveal {
    /// The player's id.
    pub player_id: String,
    /// The (in-process-only) hand contents.
    pub hand: String,
}

/// A committed card observed on an open `commit` span (usually empty — commits are
/// momentary, revealed publicly at the depile anyway).
#[derive(Clone, Serialize)]
pub struct CommitReveal {
    /// The committing player's id.
    pub player_id: String,
    /// The committed card's identity (secret until resolution).
    pub card: String,
}

/// The privileged hidden-state reveal for a live room, from its open spans.
#[derive(Clone, Serialize)]
pub struct Reveal {
    /// The room's invite code.
    pub room_code: String,
    /// The open round's number.
    pub round_number: Option<u64>,
    /// The open wave's number.
    pub wave_number: Option<u64>,
    /// The round's (secret) boiling point.
    pub boiling_point: Option<String>,
    /// The current (secret) running cauldron volatility.
    pub volatility_total: Option<String>,
    /// The round's active modifiers (public).
    pub modifiers: Option<String>,
    /// Each seated player's hand (secret).
    pub hands: Vec<HandReveal>,
    /// Cards committed on any still-open `commit` span (secret).
    pub committed: Vec<CommitReveal>,
}

/// The outcome of a reveal request.
#[derive(Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RevealOutcome {
    /// No live room with that code.
    NoSuchRoom,
    /// The room is live but between rounds (no open `round` span).
    NoRoundInProgress {
        /// The room's code.
        room_code: String,
    },
    /// Hidden state revealed.
    Revealed(Reveal),
}

/// An activity-feed event pushed to live (SSE) subscribers over the authenticated
/// admin channel. It carries the span's attributes (including sensitive game state);
/// the feed never reaches a player connection.
#[derive(Clone, Serialize)]
pub struct LiveEvent {
    /// `start`, `update`, or `end`.
    pub kind: &'static str,
    /// The span name.
    pub span: String,
    /// The room this event belongs to, if any.
    pub room_code: Option<String>,
    /// The span's attributes.
    pub attributes: BTreeMap<String, String>,
}

/// The mutable projection state, guarded by a single `RwLock`.
#[derive(Default)]
struct State {
    /// All currently-open spans, by id.
    open: HashMap<u64, OpenSpan>,
    /// In-progress per-game accumulators (game span id → accumulator).
    games: HashMap<u64, GameAccumulator>,
    /// Bounded buffer of recent completed games.
    replay: VecDeque<ReplayGame>,
    /// Unsampled rolling counters.
    agg: Aggregates,
}

/// The span projection. Register it as the lifecycle consumer *and* share it with
/// the admin API (`Arc<AdminProjection>`).
pub struct AdminProjection {
    state: RwLock<State>,
    live: broadcast::Sender<LiveEvent>,
    stuck_wave_margin_ms: u64,
}

impl Default for AdminProjection {
    fn default() -> Self {
        Self::new()
    }
}

impl AdminProjection {
    /// A fresh, empty projection.
    pub fn new() -> Self {
        let (live, _) = broadcast::channel(LIVE_CHANNEL_CAPACITY);
        Self {
            state: RwLock::new(State::default()),
            live,
            stuck_wave_margin_ms: DEFAULT_STUCK_WAVE_MARGIN_MS,
        }
    }

    /// Override the stuck-room wave margin (tests use a tiny margin).
    pub fn with_stuck_wave_margin_ms(mut self, margin_ms: u64) -> Self {
        self.stuck_wave_margin_ms = margin_ms;
        self
    }

    /// Subscribe to the live activity feed.
    pub fn subscribe(&self) -> broadcast::Receiver<LiveEvent> {
        self.live.subscribe()
    }

    // ---- span lifecycle handling (the only writers of state) ----

    fn on_start(&self, ev: SpanEvent) {
        if !span_schema::is_known_span(ev.name) {
            return; // schema tolerance: ignore unrecognized spans
        }
        let room_code = {
            let mut st = self.state.write().expect("projection lock");
            let (mut room_id, mut game_id) = match ev.parent_id.and_then(|p| st.open.get(&p)) {
                Some(parent) => (parent.room_id, parent.game_id),
                None => (None, None),
            };
            if ev.name == span::ROOM_LIFETIME {
                room_id = Some(ev.id);
            }
            if ev.name == span::GAME {
                game_id = Some(ev.id);
            }
            let room_code = if ev.name == span::ROOM_LIFETIME {
                ev.attributes.get(attr::ROOM_CODE).cloned()
            } else {
                room_code_of(&st, room_id)
            };
            if ev.name == span::GAME {
                st.games.insert(
                    ev.id,
                    GameAccumulator {
                        room_code: room_code.clone(),
                        spans: Vec::new(),
                    },
                );
            }
            st.open.insert(
                ev.id,
                OpenSpan {
                    name: ev.name,
                    attributes: ev.attributes.clone(),
                    started: Instant::now(),
                    room_id,
                    game_id,
                },
            );
            room_code
        };
        self.broadcast_live("start", &ev, room_code);
    }

    fn on_update(&self, ev: SpanEvent) {
        let room_code = {
            let mut st = self.state.write().expect("projection lock");
            let room_id = match st.open.get_mut(&ev.id) {
                Some(open) => {
                    open.attributes = ev.attributes.clone();
                    open.room_id
                }
                None => return,
            };
            room_code_of(&st, room_id)
        };
        self.broadcast_live("update", &ev, room_code);
    }

    fn on_end(&self, ev: SpanEvent) {
        let room_code = {
            let mut st = self.state.write().expect("projection lock");
            let Some(open) = st.open.remove(&ev.id) else {
                return;
            };
            let dur_ms = open.started.elapsed().as_millis() as u64;
            let room_code =
                room_code_of(&st, open.room_id).or(open.attributes.get(attr::ROOM_CODE).cloned());
            fold_aggregates(&mut st.agg, ev.name, &ev.attributes, dur_ms);

            // Accumulate completed children for the game's replay record.
            if ev.name != span::GAME
                && let Some(gid) = open.game_id
                && let Some(acc) = st.games.get_mut(&gid)
            {
                acc.spans.push(CompletedSpan {
                    id: ev.id,
                    parent_id: ev.parent_id,
                    name: ev.name.to_string(),
                    attributes: ev.attributes.clone(),
                    duration_ms: dur_ms,
                });
            }

            // Finalize the game into the bounded replay buffer.
            if ev.name == span::GAME
                && let Some(acc) = st.games.remove(&ev.id)
            {
                let replay = ReplayGame {
                    game_id: ev
                        .attributes
                        .get(attr::GAME_ID)
                        .cloned()
                        .unwrap_or_else(|| ev.id.to_string()),
                    room_code: acc.room_code,
                    duration_ms: dur_ms,
                    spans: acc.spans,
                };
                if st.replay.len() >= REPLAY_CAPACITY {
                    st.replay.pop_front();
                }
                st.replay.push_back(replay);
            }
            room_code
        };
        self.broadcast_live("end", &ev, room_code);
    }

    fn broadcast_live(&self, kind: &'static str, ev: &SpanEvent, room_code: Option<String>) {
        // The feed is served only over the authenticated admin channel, so it may
        // carry the span's attributes as-is (no player connection ever sees it).
        let _ = self.live.send(LiveEvent {
            kind,
            span: ev.name.to_string(),
            room_code,
            attributes: ev.attributes.clone(),
        });
    }

    // ---- read surfaces (the admin API calls these) ----

    /// The current balance figures from the unsampled aggregates.
    pub fn balance(&self) -> BalanceFigures {
        let st = self.state.read().expect("projection lock");
        balance_from(&st.agg)
    }

    /// A fleet summary: live counts + the balance figures.
    pub fn fleet(&self) -> FleetSummary {
        let st = self.state.read().expect("projection lock");
        let rooms = self.room_views_locked(&st);
        FleetSummary {
            rooms: count_open(&st, span::ROOM_LIFETIME),
            games: count_open(&st, span::GAME),
            queue_depth: count_open(&st, span::LOBBY_WAIT),
            stuck_rooms: rooms.iter().filter(|r| r.stuck).count(),
            balance: balance_from(&st.agg),
        }
    }

    /// The live room list, derived from the open-span registry.
    pub fn rooms(&self) -> Vec<RoomView> {
        let st = self.state.read().expect("projection lock");
        self.room_views_locked(&st)
    }

    /// One room's live view by invite code.
    pub fn room(&self, code: &str) -> Option<RoomView> {
        let st = self.state.read().expect("projection lock");
        self.room_views_locked(&st)
            .into_iter()
            .find(|r| r.room_code.as_deref() == Some(code))
    }

    /// The current auto-match queue depth.
    pub fn queue_depth(&self) -> usize {
        let st = self.state.read().expect("projection lock");
        count_open(&st, span::LOBBY_WAIT)
    }

    /// The privileged hidden-state reveal for a live room, from its open spans.
    pub fn reveal(&self, code: &str) -> RevealOutcome {
        let st = self.state.read().expect("projection lock");
        let Some(room_id) = st
            .open
            .iter()
            .find(|(_, s)| {
                s.name == span::ROOM_LIFETIME
                    && s.attributes.get(attr::ROOM_CODE).map(String::as_str) == Some(code)
            })
            .map(|(id, _)| *id)
        else {
            return RevealOutcome::NoSuchRoom;
        };

        let descendants: Vec<&OpenSpan> = st
            .open
            .values()
            .filter(|s| s.room_id == Some(room_id))
            .collect();

        let round = descendants.iter().find(|s| s.name == span::ROUND);
        let Some(round) = round else {
            return RevealOutcome::NoRoundInProgress {
                room_code: code.to_string(),
            };
        };
        let wave = descendants.iter().find(|s| s.name == span::WAVE);

        let hands = descendants
            .iter()
            .filter(|s| s.name == span::HAND)
            .map(|s| HandReveal {
                player_id: s
                    .attributes
                    .get(attr::PLAYER_ID)
                    .cloned()
                    .unwrap_or_default(),
                hand: s.attributes.get(attr::HAND).cloned().unwrap_or_default(),
            })
            .collect();
        let committed = descendants
            .iter()
            .filter(|s| s.name == span::COMMIT)
            .map(|s| CommitReveal {
                player_id: s
                    .attributes
                    .get(attr::PLAYER_ID)
                    .cloned()
                    .unwrap_or_default(),
                card: s
                    .attributes
                    .get(attr::COMMITTED_CARD)
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect();

        RevealOutcome::Revealed(Reveal {
            room_code: code.to_string(),
            round_number: round
                .attributes
                .get(attr::ROUND_NUMBER)
                .and_then(|v| v.parse().ok()),
            wave_number: wave.and_then(|w| {
                w.attributes
                    .get(attr::WAVE_NUMBER)
                    .and_then(|v| v.parse().ok())
            }),
            boiling_point: round.attributes.get(attr::BOILING_POINT).cloned(),
            volatility_total: round.attributes.get(attr::VOLATILITY_TOTAL).cloned(),
            modifiers: round.attributes.get(attr::MODIFIERS).cloned(),
            hands,
            committed,
        })
    }

    /// The replay index (recent completed games, newest last).
    pub fn replay_list(&self) -> Vec<ReplaySummary> {
        let st = self.state.read().expect("projection lock");
        st.replay
            .iter()
            .map(|g| ReplaySummary {
                game_id: g.game_id.clone(),
                room_code: g.room_code.clone(),
                duration_ms: g.duration_ms,
                span_count: g.spans.len(),
            })
            .collect()
    }

    /// A single completed game by id, or `None` if it was evicted / never retained.
    pub fn replay(&self, game_id: &str) -> Option<ReplayGame> {
        let st = self.state.read().expect("projection lock");
        st.replay.iter().find(|g| g.game_id == game_id).cloned()
    }

    /// Number of open spans (a read-only invariant check for tests).
    pub fn open_span_count(&self) -> usize {
        self.state.read().expect("projection lock").open.len()
    }

    /// Reap open-span registry entries whose span-end was evidently missed: any
    /// span open longer than `max_age` is dropped (and its game accumulator with
    /// it), bounding registry memory if an `End` event is ever lost. Returns the
    /// number reaped. `max_age` should be generous — larger than any legitimate
    /// span lifetime — and is tied to the stuck-room age check by the caller.
    pub fn reap_stale(&self, max_age: Duration) -> usize {
        let mut st = self.state.write().expect("projection lock");
        let stale: Vec<u64> = st
            .open
            .iter()
            .filter(|(_, s)| s.started.elapsed() > max_age)
            .map(|(id, _)| *id)
            .collect();
        for id in &stale {
            st.open.remove(id);
            st.games.remove(id); // harmless no-op unless the reaped span was a game
        }
        stale.len()
    }

    // ---- helpers ----

    fn room_views_locked(&self, st: &State) -> Vec<RoomView> {
        st.open
            .iter()
            .filter(|(_, s)| s.name == span::ROOM_LIFETIME)
            .map(|(id, room)| {
                let descendants: Vec<&OpenSpan> = st
                    .open
                    .values()
                    .filter(|s| s.room_id == Some(*id) && s.name != span::ROOM_LIFETIME)
                    .collect();
                let game = descendants.iter().find(|s| s.name == span::GAME);
                let round = descendants.iter().find(|s| s.name == span::ROUND);
                let wave = descendants.iter().find(|s| s.name == span::WAVE);

                let phase = if wave.is_some() {
                    "wave"
                } else if round.is_some() {
                    "round"
                } else if game.is_some() {
                    "game"
                } else {
                    "lobby"
                };
                let last_wave_age_ms = wave.map(|w| w.started.elapsed().as_millis() as u64);
                let (stuck, stuck_reason) = self.stuck_check(wave, last_wave_age_ms);

                RoomView {
                    room_id: *id,
                    room_code: room.attributes.get(attr::ROOM_CODE).cloned(),
                    phase: phase.to_string(),
                    round_number: round.and_then(|r| {
                        r.attributes
                            .get(attr::ROUND_NUMBER)
                            .and_then(|v| v.parse().ok())
                    }),
                    round_total: ROUND_COUNT,
                    wave_number: wave.and_then(|w| {
                        w.attributes
                            .get(attr::WAVE_NUMBER)
                            .and_then(|v| v.parse().ok())
                    }),
                    players: game.and_then(|g| {
                        g.attributes
                            .get(attr::PLAYERS_COUNT)
                            .and_then(|v| v.parse().ok())
                    }),
                    age_ms: room.started.elapsed().as_millis() as u64,
                    last_wave_age_ms,
                    stuck,
                    stuck_reason,
                }
            })
            .collect()
    }

    /// A room is stuck if its open wave has outlived its timer budget by the margin.
    fn stuck_check(
        &self,
        wave: Option<&&OpenSpan>,
        wave_age_ms: Option<u64>,
    ) -> (bool, Option<String>) {
        let (Some(wave), Some(age)) = (wave, wave_age_ms) else {
            return (false, None);
        };
        let timer_ms: u64 = wave
            .attributes
            .get(attr::WAVE_TIMER_MS)
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if age > timer_ms + self.stuck_wave_margin_ms {
            (
                true,
                Some(format!(
                    "wave open {age}ms, exceeds timer {timer_ms}ms + margin {}ms",
                    self.stuck_wave_margin_ms
                )),
            )
        } else {
            (false, None)
        }
    }
}

impl SpanConsumer for AdminProjection {
    fn on_event(&self, event: SpanEvent) {
        match event.kind {
            SpanEventKind::Start => self.on_start(event),
            SpanEventKind::Update => self.on_update(event),
            SpanEventKind::End => self.on_end(event),
        }
    }
}

/// Count open spans of a given name.
fn count_open(st: &State, name: &str) -> usize {
    st.open.values().filter(|s| s.name == name).count()
}

/// Resolve a room's invite code from its open `room.lifetime` span.
fn room_code_of(st: &State, room_id: Option<u64>) -> Option<String> {
    room_id
        .and_then(|id| st.open.get(&id))
        .and_then(|r| r.attributes.get(attr::ROOM_CODE).cloned())
}

/// Fold one completed span into the rolling aggregates.
fn fold_aggregates(
    agg: &mut Aggregates,
    name: &str,
    attrs: &BTreeMap<String, String>,
    dur_ms: u64,
) {
    let is = |key: &str, val: &str| attrs.get(key).map(String::as_str) == Some(val);
    match name {
        span::ROUND => {
            agg.rounds += 1;
            agg.total_round_duration_ms += dur_ms;
            if is(attr::ROUND_EXPLODED, "true") {
                agg.explosions += 1;
            }
        }
        span::WAVE => {
            agg.waves += 1;
            if is(attr::WAVE_TIMED_OUT, "true") {
                agg.wave_timeouts += 1;
            }
        }
        span::COMMIT => agg.commits += 1,
        span::RECONNECT => agg.reconnects += 1,
        span::GAME => {
            agg.games += 1;
            agg.total_game_duration_ms += dur_ms;
        }
        span::SCORE => match attrs.get(attr::DOMINANT_COLOR).map(String::as_str) {
            Some("none") | None => {}
            Some("split") => agg.splits += 1,
            Some(_) => agg.dominations += 1,
        },
        _ => {}
    }
}

/// Compute the balance figures from the raw counters (guards divide-by-zero).
fn balance_from(agg: &Aggregates) -> BalanceFigures {
    let div = |n: u64, d: u64| if d == 0 { 0.0 } else { n as f64 / d as f64 };
    let decided = agg.dominations + agg.splits;
    BalanceFigures {
        games: agg.games,
        rounds: agg.rounds,
        explosion_rate: div(agg.explosions, agg.rounds),
        avg_round_duration_ms: div(agg.total_round_duration_ms, agg.rounds),
        avg_game_duration_ms: div(agg.total_game_duration_ms, agg.games),
        cards_per_round: div(agg.commits, agg.rounds),
        wave_timeout_rate: div(agg.wave_timeouts, agg.waves),
        reconnection_rate: div(agg.reconnects, agg.games),
        dominant_color_rate: div(agg.dominations, decided),
        schema_version: SPAN_SCHEMA_VERSION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Build a span event with the given attributes.
    fn event(
        kind: SpanEventKind,
        id: u64,
        name: &'static str,
        parent_id: Option<u64>,
        attrs: &[(&str, &str)],
    ) -> SpanEvent {
        let attributes: BTreeMap<String, String> = attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        SpanEvent {
            kind,
            id,
            name,
            parent_id,
            attributes,
        }
    }

    fn start(
        p: &AdminProjection,
        id: u64,
        name: &'static str,
        parent: Option<u64>,
        attrs: &[(&str, &str)],
    ) {
        p.on_event(event(SpanEventKind::Start, id, name, parent, attrs));
    }
    fn end(
        p: &AdminProjection,
        id: u64,
        name: &'static str,
        parent: Option<u64>,
        attrs: &[(&str, &str)],
    ) {
        p.on_event(event(SpanEventKind::End, id, name, parent, attrs));
    }

    #[test]
    fn live_registry_adds_and_removes_rooms() {
        let p = AdminProjection::new();
        start(
            &p,
            1,
            span::ROOM_LIFETIME,
            None,
            &[(attr::ROOM_CODE, "ABCD")],
        );
        assert_eq!(p.rooms().len(), 1);
        assert_eq!(p.rooms()[0].room_code.as_deref(), Some("ABCD"));
        assert_eq!(p.rooms()[0].phase, "lobby");

        // A game → round → wave nests; phase follows the deepest open child.
        start(
            &p,
            2,
            span::GAME,
            Some(1),
            &[(attr::GAME_ID, "g1"), (attr::PLAYERS_COUNT, "4")],
        );
        start(&p, 3, span::ROUND, Some(2), &[(attr::ROUND_NUMBER, "2")]);
        start(
            &p,
            4,
            span::WAVE,
            Some(3),
            &[(attr::WAVE_NUMBER, "1"), (attr::WAVE_TIMER_MS, "30000")],
        );
        let view = &p.rooms()[0];
        assert_eq!(view.phase, "wave");
        assert_eq!(view.round_number, Some(2));
        assert_eq!(view.wave_number, Some(1));
        assert_eq!(view.players, Some(4));

        end(&p, 4, span::WAVE, Some(3), &[(attr::WAVE_NUMBER, "1")]);
        assert_eq!(p.rooms()[0].phase, "round");

        end(
            &p,
            1,
            span::ROOM_LIFETIME,
            None,
            &[(attr::ROOM_CODE, "ABCD")],
        );
        assert!(p.rooms().is_empty(), "a finished room leaves the live view");
    }

    #[test]
    fn unsampled_aggregates_count_every_completed_round() {
        let p = AdminProjection::new();
        // 10 rounds complete; 3 exploded. Even if export sampling kept only a few,
        // the projection sees 100% because it consumes the lifecycle stream upstream.
        for i in 0..10u64 {
            let exploded = if i < 3 { "true" } else { "false" };
            start(&p, 100 + i, span::ROUND, None, &[(attr::ROUND_NUMBER, "1")]);
            end(
                &p,
                100 + i,
                span::ROUND,
                None,
                &[(attr::ROUND_EXPLODED, exploded)],
            );
        }
        let b = p.balance();
        assert_eq!(b.rounds, 10);
        assert!(
            (b.explosion_rate - 0.3).abs() < 1e-9,
            "explosion rate should be 3/10"
        );
    }

    #[test]
    fn replay_buffer_is_bounded_and_evicts_oldest() {
        let p = AdminProjection::new();
        let total = REPLAY_CAPACITY + 5;
        for i in 0..total as u64 {
            let id = format!("g{i}");
            start(&p, 1000 + i, span::GAME, None, &[(attr::GAME_ID, &id)]);
            end(&p, 1000 + i, span::GAME, None, &[(attr::GAME_ID, &id)]);
        }
        let list = p.replay_list();
        assert_eq!(list.len(), REPLAY_CAPACITY, "buffer stays bounded");
        // Oldest (g0..g4) evicted; newest retained.
        assert!(p.replay("g0").is_none(), "oldest game evicted");
        assert!(
            p.replay(&format!("g{}", total - 1)).is_some(),
            "newest game retained"
        );
    }

    #[test]
    fn completed_game_is_replayable_wave_by_wave() {
        let p = AdminProjection::new();
        start(
            &p,
            1,
            span::ROOM_LIFETIME,
            None,
            &[(attr::ROOM_CODE, "WXYZ")],
        );
        start(&p, 2, span::GAME, Some(1), &[(attr::GAME_ID, "gg")]);
        start(&p, 3, span::ROUND, Some(2), &[(attr::ROUND_NUMBER, "1")]);
        start(&p, 4, span::WAVE, Some(3), &[(attr::WAVE_NUMBER, "1")]);
        end(&p, 4, span::WAVE, Some(3), &[(attr::WAVE_NUMBER, "1")]);
        end(
            &p,
            3,
            span::ROUND,
            Some(2),
            &[(attr::ROUND_NUMBER, "1"), (attr::ROUND_EXPLODED, "true")],
        );
        end(&p, 2, span::GAME, Some(1), &[(attr::GAME_ID, "gg")]);

        let game = p.replay("gg").expect("game retained");
        assert_eq!(game.room_code.as_deref(), Some("WXYZ"));
        assert!(game.spans.iter().any(|s| s.name == "wave"));
        assert!(game.spans.iter().any(|s| s.name == "round"));
    }

    #[test]
    fn reveal_reads_open_round_attributes() {
        let p = AdminProjection::new();
        start(
            &p,
            1,
            span::ROOM_LIFETIME,
            None,
            &[(attr::ROOM_CODE, "RVL1")],
        );
        start(&p, 2, span::GAME, Some(1), &[(attr::GAME_ID, "g")]);
        // No round yet → "no round in progress".
        match p.reveal("RVL1") {
            RevealOutcome::NoRoundInProgress { room_code } => assert_eq!(room_code, "RVL1"),
            other => panic!(
                "expected NoRoundInProgress, got {:?}",
                serde_json::to_string(&other)
            ),
        }
        start(
            &p,
            3,
            span::ROUND,
            Some(2),
            &[
                (attr::ROUND_NUMBER, "2"),
                (attr::BOILING_POINT, "11"),
                (attr::MODIFIERS, "ThinIce"),
            ],
        );
        start(
            &p,
            4,
            span::HAND,
            Some(3),
            &[(attr::PLAYER_ID, "p1"), (attr::HAND, "Ruby(v2,p1)")],
        );
        // A live update records running volatility on the open round span.
        p.on_event(event(
            SpanEventKind::Update,
            3,
            span::ROUND,
            Some(2),
            &[
                (attr::ROUND_NUMBER, "2"),
                (attr::BOILING_POINT, "11"),
                (attr::MODIFIERS, "ThinIce"),
                (attr::VOLATILITY_TOTAL, "7"),
            ],
        ));

        match p.reveal("RVL1") {
            RevealOutcome::Revealed(r) => {
                assert_eq!(r.boiling_point.as_deref(), Some("11"));
                assert_eq!(r.volatility_total.as_deref(), Some("7"));
                assert_eq!(r.modifiers.as_deref(), Some("ThinIce"));
                assert_eq!(r.hands.len(), 1);
                assert_eq!(r.hands[0].hand, "Ruby(v2,p1)");
            }
            _ => panic!("expected a reveal"),
        }
        assert!(matches!(p.reveal("NOPE"), RevealOutcome::NoSuchRoom));
    }

    #[test]
    fn unknown_span_is_ignored() {
        let p = AdminProjection::new();
        start(&p, 1, "totally.unknown.span", None, &[("x", "y")]);
        assert_eq!(p.open_span_count(), 0, "unknown spans are not tracked");
        // And it does not break subsequent known spans.
        start(&p, 2, span::ROOM_LIFETIME, None, &[(attr::ROOM_CODE, "OK")]);
        assert_eq!(p.rooms().len(), 1);
    }

    #[test]
    fn stuck_room_flagged_when_wave_overruns() {
        let p = AdminProjection::new().with_stuck_wave_margin_ms(0);
        start(
            &p,
            1,
            span::ROOM_LIFETIME,
            None,
            &[(attr::ROOM_CODE, "STK1")],
        );
        start(&p, 2, span::GAME, Some(1), &[(attr::GAME_ID, "g")]);
        start(&p, 3, span::ROUND, Some(2), &[(attr::ROUND_NUMBER, "1")]);
        start(
            &p,
            4,
            span::WAVE,
            Some(3),
            &[(attr::WAVE_NUMBER, "1"), (attr::WAVE_TIMER_MS, "0")],
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
        let view = &p.rooms()[0];
        assert!(view.stuck, "an over-age wave should flag the room as stuck");
        assert!(view.stuck_reason.is_some());
    }

    #[test]
    fn reaps_open_spans_whose_end_was_missed() {
        let p = AdminProjection::new();
        start(
            &p,
            1,
            span::ROOM_LIFETIME,
            None,
            &[(attr::ROOM_CODE, "LEAK")],
        );
        assert_eq!(p.open_span_count(), 1);
        std::thread::sleep(Duration::from_millis(10));
        // A short max-age reaps the leaked span (its End was never delivered).
        assert_eq!(p.reap_stale(Duration::from_millis(5)), 1);
        assert!(
            p.rooms().is_empty(),
            "a span whose end was missed is reaped"
        );
    }

    #[test]
    fn queue_depth_counts_open_lobby_wait_spans() {
        let p = AdminProjection::new();
        start(&p, 1, span::LOBBY_WAIT, None, &[(attr::PLAYER_ID, "a")]);
        start(&p, 2, span::LOBBY_WAIT, None, &[(attr::PLAYER_ID, "b")]);
        assert_eq!(p.queue_depth(), 2);
        end(&p, 1, span::LOBBY_WAIT, None, &[]);
        assert_eq!(p.queue_depth(), 1);
    }
}
