//! The admin **span projection**: an in-process read model built *solely* by
//! consuming the server's span lifecycle (`admin-span-projection`).
//!
//! It maintains three things, all derived from spans and nothing else:
//! - a **live open-span registry** (current groups/games/rounds/waves + the queue),
//!   the source for the group inspector and the privileged reveal;
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
use crate::observability::balance_metrics::{self, Accumulator, MetricValue};
use crate::observability::lifecycle::{SpanConsumer, SpanEvent, SpanEventKind};
use crate::observability::span_schema::{self, SPAN_SCHEMA_VERSION, attr, span};

/// Recent completed games retained for replay (oldest evicted past this).
const REPLAY_CAPACITY: usize = 64;
/// Capacity of the live activity broadcast channel (SSE feed).
const LIVE_CHANNEL_CAPACITY: usize = 512;
/// Default margin a wave may exceed its timer budget before a group is flagged
/// stuck. **Needs playtesting** against real wave-timer budgets (design Open Q).
const DEFAULT_STUCK_WAVE_MARGIN_MS: u64 = 15_000;

/// One currently-open span tracked by the registry.
struct OpenSpan {
    name: &'static str,
    attributes: BTreeMap<String, String>,
    started: Instant,
    /// The enclosing `group.lifetime` span id (itself, if this *is* the group).
    group_id: Option<u64>,
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
    group_code: Option<String>,
    spans: Vec<CompletedSpan>,
}

/// A completed game retained in the bounded replay buffer.
#[derive(Clone, Serialize)]
pub struct ReplayGame {
    /// The game's id (from the `game.id` attribute).
    pub game_id: String,
    /// The group the game ran in, if known.
    pub group_code: Option<String>,
    /// Total game duration in milliseconds.
    pub duration_ms: u64,
    /// The game's descendant spans in completion order (rounds → waves →
    /// commits/spell-casts → resolve → depile/score), enough to replay it wave
    /// by wave including the spell casts and the depile reveal.
    pub spans: Vec<CompletedSpan>,
}

/// A short index entry for the replay list.
#[derive(Clone, Serialize)]
pub struct ReplaySummary {
    /// The game's id.
    pub game_id: String,
    /// The group the game ran in, if known.
    pub group_code: Option<String>,
    /// Total game duration in milliseconds.
    pub duration_ms: u64,
    /// Number of retained spans (a rough size signal).
    pub span_count: usize,
}

/// The Principle IV balance figures, all derived from the **unsampled**
/// in-process aggregates (never a sampled trace): every metric is evaluated from
/// its `boom-balance-metrics` definition over the folded
/// [`Accumulator`] — identical to what balance studies evaluate.
#[derive(Clone, Serialize)]
pub struct BalanceFigures {
    /// Completed games observed.
    pub games: u64,
    /// Completed rounds observed.
    pub rounds: u64,
    /// Every v2 balance metric, evaluated from the shared definitions: id,
    /// value (absent until its population exists), and the
    /// `[needs playtesting]` target band where one is seeded.
    pub metrics: Vec<MetricValue>,
    /// Per-spell cast rates (casts per round, by spell kind).
    pub per_spell_cast_rates: Vec<(String, f64)>,
    /// The span-schema version the projection was built against.
    pub schema_version: u32,
}

/// A live fleet summary.
#[derive(Clone, Serialize)]
pub struct FleetSummary {
    /// Live groups (open `group.lifetime` spans).
    pub groups: usize,
    /// In-flight games (open `game` spans).
    pub games: usize,
    /// Players waiting in the auto-match queue (open `lobby.wait` spans).
    pub queue_depth: usize,
    /// Groups currently flagged stuck/anomalous.
    pub stuck_groups: usize,
    /// The current balance figures.
    pub balance: BalanceFigures,
}

/// A live group as the inspector shows it, derived from the group's open spans.
#[derive(Clone, Serialize)]
pub struct GroupView {
    /// Registry key (the group's `group.lifetime` span id).
    pub group_id: u64,
    /// Invite code, if the span carried one.
    pub group_code: Option<String>,
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
    /// Group age in milliseconds.
    pub age_ms: u64,
    /// Age of the currently-open wave, if any.
    pub last_wave_age_ms: Option<u64>,
    /// Whether the group is flagged stuck/anomalous.
    pub stuck: bool,
    /// Why it is flagged, if it is.
    pub stuck_reason: Option<String>,
}

/// One seated player's hands, read from an open `hand` span (secret in-process).
#[derive(Clone, Serialize)]
pub struct HandReveal {
    /// The player's id.
    pub player_id: String,
    /// The (in-process-only) pantry (ingredient) hand contents.
    pub pantry: String,
    /// The (in-process-only) spell (grimoire) hand contents.
    pub spells: String,
}

/// A committed-but-unrevealed wave play, read from an open `commit` span (open
/// from the moment the hidden commit is accepted until the wave resolves).
#[derive(Clone, Serialize)]
pub struct CommitReveal {
    /// The committing player's id.
    pub player_id: String,
    /// The committed card's identity (secret until the depile).
    pub card: String,
    /// The commit's Vote colour — the card's colour, or `colorless` (secret
    /// until the depile).
    pub vote_color: String,
}

/// The privileged hidden-state reveal for a live group, from its open spans.
#[derive(Clone, Serialize)]
pub struct Reveal {
    /// The group's invite code.
    pub group_code: String,
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
    /// Active spell effects (secret): unfired primed Actives and a pending
    /// Quench shield.
    pub active_effects: Option<String>,
    /// Each seated player's pantry and spell hands (secret).
    pub hands: Vec<HandReveal>,
    /// Committed-but-unrevealed wave plays from open `commit` spans (secret).
    pub committed: Vec<CommitReveal>,
}

/// The outcome of a reveal request.
#[derive(Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RevealOutcome {
    /// No live group with that code.
    NoSuchGroup,
    /// The group is live but between rounds (no open `round` span).
    NoRoundInProgress {
        /// The group's code.
        group_code: String,
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
    /// The group this event belongs to, if any.
    pub group_code: Option<String>,
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
    /// Unsampled rolling counters — the `boom-balance-metrics` accumulator the
    /// shared definitions evaluate over.
    agg: Accumulator,
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

    /// Override the stuck-group wave margin (tests use a tiny margin).
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
        let group_code = {
            let mut st = self.state.write().expect("projection lock");
            let (mut group_id, mut game_id) = match ev.parent_id.and_then(|p| st.open.get(&p)) {
                Some(parent) => (parent.group_id, parent.game_id),
                None => (None, None),
            };
            if ev.name == span::GROUP_LIFETIME {
                group_id = Some(ev.id);
            }
            if ev.name == span::GAME {
                game_id = Some(ev.id);
            }
            let group_code = if ev.name == span::GROUP_LIFETIME {
                ev.attributes.get(attr::GROUP_CODE).cloned()
            } else {
                group_code_of(&st, group_id)
            };
            if ev.name == span::GAME {
                st.games.insert(
                    ev.id,
                    GameAccumulator {
                        group_code: group_code.clone(),
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
                    group_id,
                    game_id,
                },
            );
            group_code
        };
        self.broadcast_live("start", &ev, group_code);
    }

    fn on_update(&self, ev: SpanEvent) {
        let group_code = {
            let mut st = self.state.write().expect("projection lock");
            let group_id = match st.open.get_mut(&ev.id) {
                Some(open) => {
                    open.attributes = ev.attributes.clone();
                    open.group_id
                }
                None => return,
            };
            group_code_of(&st, group_id)
        };
        self.broadcast_live("update", &ev, group_code);
    }

    fn on_end(&self, ev: SpanEvent) {
        let group_code = {
            let mut st = self.state.write().expect("projection lock");
            let Some(open) = st.open.remove(&ev.id) else {
                return;
            };
            let dur_ms = open.started.elapsed().as_millis() as u64;
            let group_code = group_code_of(&st, open.group_id)
                .or(open.attributes.get(attr::GROUP_CODE).cloned());
            // Fold the completed span into the rolling aggregates via the shared
            // span→event seam, so the figures the dashboard reads come from the
            // same definitions the balance studies evaluate.
            if let Some(event) = balance_metrics::event_from_span(ev.name, &ev.attributes, dur_ms) {
                st.agg.record(&event);
            }

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
                    group_code: acc.group_code,
                    duration_ms: dur_ms,
                    spans: acc.spans,
                };
                if st.replay.len() >= REPLAY_CAPACITY {
                    st.replay.pop_front();
                }
                st.replay.push_back(replay);
            }
            group_code
        };
        self.broadcast_live("end", &ev, group_code);
    }

    fn broadcast_live(&self, kind: &'static str, ev: &SpanEvent, group_code: Option<String>) {
        // The feed is served only over the authenticated admin channel, so it may
        // carry the span's attributes as-is (no player connection ever sees it).
        let _ = self.live.send(LiveEvent {
            kind,
            span: ev.name.to_string(),
            group_code,
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
        let groups = self.group_views_locked(&st);
        FleetSummary {
            groups: count_open(&st, span::GROUP_LIFETIME),
            games: count_open(&st, span::GAME),
            queue_depth: count_open(&st, span::LOBBY_WAIT),
            stuck_groups: groups.iter().filter(|r| r.stuck).count(),
            balance: balance_from(&st.agg),
        }
    }

    /// The live group list, derived from the open-span registry.
    pub fn groups(&self) -> Vec<GroupView> {
        let st = self.state.read().expect("projection lock");
        self.group_views_locked(&st)
    }

    /// One group's live view by invite code.
    pub fn group(&self, code: &str) -> Option<GroupView> {
        let st = self.state.read().expect("projection lock");
        self.group_views_locked(&st)
            .into_iter()
            .find(|r| r.group_code.as_deref() == Some(code))
    }

    /// The current auto-match queue depth.
    pub fn queue_depth(&self) -> usize {
        let st = self.state.read().expect("projection lock");
        count_open(&st, span::LOBBY_WAIT)
    }

    /// The privileged hidden-state reveal for a live group, from its open spans.
    pub fn reveal(&self, code: &str) -> RevealOutcome {
        let st = self.state.read().expect("projection lock");
        let Some(group_id) = st
            .open
            .iter()
            .find(|(_, s)| {
                s.name == span::GROUP_LIFETIME
                    && s.attributes.get(attr::GROUP_CODE).map(String::as_str) == Some(code)
            })
            .map(|(id, _)| *id)
        else {
            return RevealOutcome::NoSuchGroup;
        };

        let descendants: Vec<&OpenSpan> = st
            .open
            .values()
            .filter(|s| s.group_id == Some(group_id))
            .collect();

        let round = descendants.iter().find(|s| s.name == span::ROUND);
        let Some(round) = round else {
            return RevealOutcome::NoRoundInProgress {
                group_code: code.to_string(),
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
                pantry: s
                    .attributes
                    .get(attr::HAND_PANTRY)
                    .cloned()
                    .unwrap_or_default(),
                spells: s
                    .attributes
                    .get(attr::HAND_SPELLS)
                    .cloned()
                    .unwrap_or_default(),
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
                vote_color: s
                    .attributes
                    .get(attr::VOTE_COLOR)
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect();

        RevealOutcome::Revealed(Reveal {
            group_code: code.to_string(),
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
            active_effects: round.attributes.get(attr::EFFECTS_ACTIVE).cloned(),
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
                group_code: g.group_code.clone(),
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
    /// span lifetime — and is tied to the stuck-group age check by the caller.
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

    fn group_views_locked(&self, st: &State) -> Vec<GroupView> {
        st.open
            .iter()
            .filter(|(_, s)| s.name == span::GROUP_LIFETIME)
            .map(|(id, group)| {
                let descendants: Vec<&OpenSpan> = st
                    .open
                    .values()
                    .filter(|s| s.group_id == Some(*id) && s.name != span::GROUP_LIFETIME)
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

                GroupView {
                    group_id: *id,
                    group_code: group.attributes.get(attr::GROUP_CODE).cloned(),
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
                    age_ms: group.started.elapsed().as_millis() as u64,
                    last_wave_age_ms,
                    stuck,
                    stuck_reason,
                }
            })
            .collect()
    }

    /// A group is stuck if its open wave has outlived its timer budget by the margin.
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

/// Resolve a group's invite code from its open `group.lifetime` span.
fn group_code_of(st: &State, group_id: Option<u64>) -> Option<String> {
    group_id
        .and_then(|id| st.open.get(&id))
        .and_then(|r| r.attributes.get(attr::GROUP_CODE).cloned())
}

/// The balance figures: every metric evaluated from its `boom-balance-metrics`
/// definition over the folded accumulator.
fn balance_from(agg: &Accumulator) -> BalanceFigures {
    BalanceFigures {
        games: agg.games,
        rounds: agg.rounds,
        metrics: balance_metrics::evaluate_all(agg),
        per_spell_cast_rates: agg.per_spell_cast_rates(),
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
    fn live_registry_adds_and_removes_groups() {
        let p = AdminProjection::new();
        start(
            &p,
            1,
            span::GROUP_LIFETIME,
            None,
            &[(attr::GROUP_CODE, "ABCD")],
        );
        assert_eq!(p.groups().len(), 1);
        assert_eq!(p.groups()[0].group_code.as_deref(), Some("ABCD"));
        assert_eq!(p.groups()[0].phase, "lobby");

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
        let view = &p.groups()[0];
        assert_eq!(view.phase, "wave");
        assert_eq!(view.round_number, Some(2));
        assert_eq!(view.wave_number, Some(1));
        assert_eq!(view.players, Some(4));

        end(&p, 4, span::WAVE, Some(3), &[(attr::WAVE_NUMBER, "1")]);
        assert_eq!(p.groups()[0].phase, "round");

        end(
            &p,
            1,
            span::GROUP_LIFETIME,
            None,
            &[(attr::GROUP_CODE, "ABCD")],
        );
        assert!(
            p.groups().is_empty(),
            "a finished group leaves the live view"
        );
    }

    /// Read one evaluated metric out of the balance figures.
    fn metric_value(b: &BalanceFigures, id: &str) -> Option<f64> {
        b.metrics
            .iter()
            .find(|m| m.id == id)
            .unwrap_or_else(|| panic!("metric {id} is defined"))
            .value
    }

    #[test]
    fn unsampled_aggregates_count_every_completed_round() {
        let p = AdminProjection::new();
        // 10 rounds complete; 3 boomed. Even if export sampling kept only a few,
        // the projection sees 100% because it consumes the lifecycle stream upstream.
        for i in 0..10u64 {
            let boomed = if i < 3 { "true" } else { "false" };
            start(&p, 100 + i, span::ROUND, None, &[(attr::ROUND_NUMBER, "1")]);
            end(
                &p,
                100 + i,
                span::ROUND,
                None,
                &[(attr::ROUND_BOOMED, boomed)],
            );
        }
        let b = p.balance();
        assert_eq!(b.rounds, 10);
        assert_eq!(
            metric_value(&b, "boom_rate"),
            Some(0.3),
            "boom rate should be 3/10"
        );
    }

    /// The projection's rolling aggregates agree with a direct evaluation of the
    /// `boom-balance-metrics` definitions over the same synthetic span stream —
    /// the single-source-of-definitions guarantee.
    #[test]
    fn aggregates_match_a_direct_definition_evaluation() {
        type SpanFixture<'a> = (u64, &'a str, &'a [(&'a str, &'a str)]);
        let p = AdminProjection::new();
        let mut direct = balance_metrics::Accumulator::default();
        let stream: &[SpanFixture] = &[
            (
                1,
                span::WAVE,
                &[(attr::WAVE_COMMITS, "3"), (attr::WAVE_PASSES, "1")],
            ),
            (2, span::SPELL_CAST, &[(attr::SPELL_KIND, "Peek")]),
            (
                3,
                span::ROUND,
                &[(attr::ROUND_BOOMED, "true"), (attr::ROUND_FROZEN, "false")],
            ),
            (
                4,
                span::SCORE,
                &[(attr::ROUND_BOOMED, "true"), (attr::DETONATORS, "a,b")],
            ),
            (5, span::RECONNECT, &[]),
            (6, span::GAME, &[]),
        ];
        for (id, name, attrs) in stream {
            start(&p, *id, name, None, &[]);
            end(&p, *id, name, None, attrs);
            let map: BTreeMap<String, String> = attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            // Durations differ (the projection times real span lifetimes), so
            // compare the duration-free metrics below.
            if let Some(ev) = balance_metrics::event_from_span(name, &map, 0) {
                direct.record(&ev);
            }
        }
        let b = p.balance();
        for id in [
            "boom_rate",
            "freeze_rate",
            "detonators_per_boom",
            "fold_rate",
            "wave_depth",
            "spell_cast_rate",
            "wave_timeout_rate",
            "reconnection_rate",
        ] {
            assert_eq!(
                metric_value(&b, id),
                balance_metrics::definition(id).unwrap().evaluate(&direct),
                "{id}: projection aggregate diverged from the definition"
            );
        }
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
            span::GROUP_LIFETIME,
            None,
            &[(attr::GROUP_CODE, "WXYZ")],
        );
        start(&p, 2, span::GAME, Some(1), &[(attr::GAME_ID, "gg")]);
        start(&p, 3, span::ROUND, Some(2), &[(attr::ROUND_NUMBER, "1")]);
        start(&p, 4, span::WAVE, Some(3), &[(attr::WAVE_NUMBER, "1")]);
        // The wave's leaves: a hidden commit, a spell cast, and the resolve.
        start(&p, 5, span::COMMIT, Some(4), &[(attr::PLAYER_ID, "p1")]);
        end(
            &p,
            5,
            span::COMMIT,
            Some(4),
            &[
                (attr::PLAYER_ID, "p1"),
                (attr::COMMITTED_CARD, "Ruby(v2,p1)"),
                (attr::VOTE_COLOR, "Ruby"),
            ],
        );
        start(
            &p,
            6,
            span::SPELL_CAST,
            Some(4),
            &[(attr::SPELL_KIND, "Peek")],
        );
        end(
            &p,
            6,
            span::SPELL_CAST,
            Some(4),
            &[(attr::SPELL_KIND, "Peek")],
        );
        start(&p, 7, span::RESOLVE, Some(4), &[]);
        end(
            &p,
            7,
            span::RESOLVE,
            Some(4),
            &[(attr::POT_CARD_COUNT, "1")],
        );
        end(&p, 4, span::WAVE, Some(3), &[(attr::WAVE_NUMBER, "1")]);
        // The round's depile (the boiling point is public at this point) + score.
        start(
            &p,
            8,
            span::DEPILE,
            Some(3),
            &[
                (attr::BOILING_POINT, "37"),
                (attr::REVEALS, "p1:Ruby(v2,p1)@w1!"),
            ],
        );
        end(
            &p,
            8,
            span::DEPILE,
            Some(3),
            &[
                (attr::BOILING_POINT, "37"),
                (attr::REVEALS, "p1:Ruby(v2,p1)@w1!"),
            ],
        );
        end(
            &p,
            3,
            span::ROUND,
            Some(2),
            &[(attr::ROUND_NUMBER, "1"), (attr::ROUND_BOOMED, "true")],
        );
        end(&p, 2, span::GAME, Some(1), &[(attr::GAME_ID, "gg")]);

        let game = p.replay("gg").expect("game retained");
        assert_eq!(game.group_code.as_deref(), Some("WXYZ"));
        // The preserved tree carries the v2 leaves for wave-by-wave replay…
        for name in ["round", "wave", "commit", "spell.cast", "resolve", "depile"] {
            assert!(
                game.spans.iter().any(|s| s.name == name),
                "replay tree should preserve {name} spans"
            );
        }
        // …in completion order: leaves before their wave, the wave before the
        // round, the depile between the wave and the round close.
        let pos = |name: &str| game.spans.iter().position(|s| s.name == name).unwrap();
        assert!(pos("commit") < pos("wave"));
        assert!(pos("spell.cast") < pos("wave"));
        assert!(pos("resolve") < pos("wave"));
        assert!(pos("wave") < pos("depile"));
        assert!(pos("depile") < pos("round"));
        // The depile entry preserves the public reveal (boiling point included).
        let depile = game.spans.iter().find(|s| s.name == "depile").unwrap();
        assert_eq!(
            depile
                .attributes
                .get(attr::BOILING_POINT)
                .map(String::as_str),
            Some("37")
        );
    }

    #[test]
    fn reveal_reads_open_round_attributes() {
        let p = AdminProjection::new();
        start(
            &p,
            1,
            span::GROUP_LIFETIME,
            None,
            &[(attr::GROUP_CODE, "RVL1")],
        );
        start(&p, 2, span::GAME, Some(1), &[(attr::GAME_ID, "g")]);
        // No round yet → "no round in progress".
        match p.reveal("RVL1") {
            RevealOutcome::NoRoundInProgress { group_code } => assert_eq!(group_code, "RVL1"),
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
                (attr::BOILING_POINT, "37"),
                (attr::MODIFIERS, "ThinIce"),
            ],
        );
        start(
            &p,
            4,
            span::HAND,
            Some(3),
            &[
                (attr::PLAYER_ID, "p1"),
                (attr::HAND_PANTRY, "Ruby(v2,p1) Moss(v0,p2)"),
                (attr::HAND_SPELLS, "Peek Surge"),
            ],
        );
        // An open wave with a committed-but-unrevealed play.
        start(&p, 5, span::WAVE, Some(3), &[(attr::WAVE_NUMBER, "1")]);
        start(
            &p,
            6,
            span::COMMIT,
            Some(5),
            &[
                (attr::PLAYER_ID, "p1"),
                (attr::COMMITTED_CARD, "Ruby(v2,p1)"),
                (attr::VOTE_COLOR, "colorless"),
            ],
        );
        // A live update records running volatility and the active spell effects
        // on the open round span.
        p.on_event(event(
            SpanEventKind::Update,
            3,
            span::ROUND,
            Some(2),
            &[
                (attr::ROUND_NUMBER, "2"),
                (attr::BOILING_POINT, "37"),
                (attr::MODIFIERS, "ThinIce"),
                (attr::VOLATILITY_TOTAL, "7"),
                (attr::EFFECTS_ACTIVE, "Hex(p2→p1),Quench(next-wave)"),
            ],
        ));

        match p.reveal("RVL1") {
            RevealOutcome::Revealed(r) => {
                assert_eq!(r.boiling_point.as_deref(), Some("37"));
                assert_eq!(r.volatility_total.as_deref(), Some("7"));
                assert_eq!(r.modifiers.as_deref(), Some("ThinIce"));
                assert_eq!(
                    r.active_effects.as_deref(),
                    Some("Hex(p2→p1),Quench(next-wave)")
                );
                assert_eq!(r.hands.len(), 1);
                assert_eq!(r.hands[0].pantry, "Ruby(v2,p1) Moss(v0,p2)");
                assert_eq!(r.hands[0].spells, "Peek Surge");
                assert_eq!(r.committed.len(), 1);
                assert_eq!(r.committed[0].card, "Ruby(v2,p1)");
                assert_eq!(r.committed[0].vote_color, "colorless");
            }
            _ => panic!("expected a reveal"),
        }
        assert!(matches!(p.reveal("NOPE"), RevealOutcome::NoSuchGroup));
    }

    #[test]
    fn unknown_span_is_ignored() {
        let p = AdminProjection::new();
        start(&p, 1, "totally.unknown.span", None, &[("x", "y")]);
        assert_eq!(p.open_span_count(), 0, "unknown spans are not tracked");
        // And it does not break subsequent known spans.
        start(
            &p,
            2,
            span::GROUP_LIFETIME,
            None,
            &[(attr::GROUP_CODE, "OK")],
        );
        assert_eq!(p.groups().len(), 1);
    }

    /// The planned-but-unimplemented pre-game spans (`brewer.pick`, `draft`) ride
    /// the same ignore-unknown tolerance: a projection at schema v2 ignores them
    /// without error until their content changes land (no schema bump).
    #[test]
    fn planned_spans_are_tolerated_until_their_changes_land() {
        use crate::observability::span_schema::planned;
        let p = AdminProjection::new();
        start(
            &p,
            1,
            span::GROUP_LIFETIME,
            None,
            &[(attr::GROUP_CODE, "PLAN")],
        );
        start(&p, 2, span::GAME, Some(1), &[(attr::GAME_ID, "g")]);
        for (id, name) in [(3u64, planned::BREWER_PICK), (4, planned::DRAFT)] {
            start(&p, id, name, Some(2), &[(attr::PLAYER_ID, "p1")]);
            end(&p, id, name, Some(2), &[(attr::PLAYER_ID, "p1")]);
        }
        // The planned spans were ignored; the known tree is unaffected.
        assert_eq!(p.groups().len(), 1);
        assert_eq!(p.groups()[0].phase, "game");
        let b = p.balance();
        assert_eq!(b.rounds, 0);
        assert_eq!(b.games, 0, "an ignored span folds into no aggregate");
    }

    #[test]
    fn stuck_group_flagged_when_wave_overruns() {
        let p = AdminProjection::new().with_stuck_wave_margin_ms(0);
        start(
            &p,
            1,
            span::GROUP_LIFETIME,
            None,
            &[(attr::GROUP_CODE, "STK1")],
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
        let view = &p.groups()[0];
        assert!(
            view.stuck,
            "an over-age wave should flag the group as stuck"
        );
        assert!(view.stuck_reason.is_some());
    }

    #[test]
    fn reaps_open_spans_whose_end_was_missed() {
        let p = AdminProjection::new();
        start(
            &p,
            1,
            span::GROUP_LIFETIME,
            None,
            &[(attr::GROUP_CODE, "LEAK")],
        );
        assert_eq!(p.open_span_count(), 1);
        std::thread::sleep(Duration::from_millis(10));
        // A short max-age reaps the leaked span (its End was never delivered).
        assert_eq!(p.reap_stale(Duration::from_millis(5)), 1);
        assert!(
            p.groups().is_empty(),
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
