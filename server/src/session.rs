//! The in-group networked game loop: drives the (synchronous, tested) engine over
//! the wire for one full game.
//!
//! For each round it draws spells and deals ingredient hands (private
//! `YourHand`), reveals a modifier from round 2, then runs waves: it tops every
//! active hand up to the floor, broadcasts `WaveOpened` with the timer budget,
//! collects hidden commits (ingredient-or-pass plus up to one spell) until the
//! timer expires or every active player has locked in, resolves the wave through
//! the engine, and broadcasts the public outcome (never card identities; Instant
//! spell activations are public, primed Actives stay silent). Each round ends
//! with the volatility-sorted depile — the boiling point revealed every round —
//! and scoring; a tie after the final round is settled by a Deathmatch.
//!
//! Resilience: a disconnected player auto-passes while absent (the seat is held
//! for the game); a reconnecting player reattaches their channel and receives a
//! private [`ServerMessage::StateSnapshot`] scoped to what they may know.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use chrono::Utc;
use sqlx::PgPool;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::observability::balance_metrics::BalanceEvent;
use crate::observability::metric;
use crate::observability::span_schema::SPAN_SCHEMA_VERSION;

use boiling_point_protocol::frame::{
    CastableSpell, PendingDecision, PlayableIngredient, TargetOptions,
};
use boiling_point_protocol::server::{
    Audience, Contribution, ErrorCode, Outbound, PlayerPublic, PlayerScore, ScoringOutcome,
};
use boiling_point_protocol::vocab::{Color, SpellTarget, TargetKind};
use boiling_point_protocol::{ClientMessage, GroupCode, PlayerId, ServerMessage};

use crate::config::{ContentConfig, ROUND_COUNT};
use crate::content::ContentRegistry;
use crate::game::round::{RoundEnd, SpellChoice, WaveAction, WaveChoice};
use crate::game::runner::{Game, RoundLog, RoundScoring, build_completed_game, depile_entries};
use crate::game::state::{Hand, Player};
use crate::lobby::group::GroupCommand;
use crate::persistence::persist_game;
use crate::replay::{RecordedInput, TimedInput, encode_replay};

/// A seated player as the game loop needs them: identity, colour, and the
/// outbound channel to reach them.
pub struct SeatInfo {
    /// Player id.
    pub id: PlayerId,
    /// Display name.
    pub name: String,
    /// Assigned colour.
    pub color: Color,
    /// Whether this seat is a matchmaking guest (not a group member).
    pub guest: bool,
    /// Outbound channel to this player's connection.
    pub out: mpsc::Sender<ServerMessage>,
}

/// What a finished game hands back to the persistent group so it can return to its
/// lobby: the final seats (with any mid-game reconnections' refreshed channels) and
/// the set of players who left/abandoned and never came back.
pub struct GameEnd {
    /// The seats as they stood at `GameOver` — reconnected players carry their
    /// refreshed `out` channel here.
    pub players: Vec<SeatInfo>,
    /// Players who disconnected (or left) and did not reconnect before the game
    /// ended; the group drops their seats when it returns to the lobby.
    pub gone: HashSet<PlayerId>,
    /// The game's winner(s) — more than one only for Deathmatch co-champions. The
    /// group folds these into its standings.
    pub winners: Vec<PlayerId>,
}

/// What one wave's collection yielded.
struct WaveCollection {
    /// Each acting player's chosen action this wave (ingredient-or-pass plus the
    /// optional spell). Validated against hands at commit time; the engine
    /// ([`Game::resolve_wave`]) removes the committed cards — collection itself
    /// never mutates a hand.
    choices: HashMap<PlayerId, WaveChoice>,
    /// Players who reconnected during the commit window (they resume next round).
    reconnected: Vec<PlayerId>,
    /// Whether the commit window closed on its timer rather than every active
    /// player locking in (feeds the `wave.timed_out` span attribute / timeout rate).
    timed_out: bool,
}

/// A compact, in-process-only rendering of one ingredient for span attributes —
/// read by the privileged reveal, never exported.
fn fmt_ingredient(c: &crate::game::card::Ingredient) -> String {
    format!("{:?}(v{},p{})", c.color, c.volatility, c.points)
}

/// A compact rendering of a pantry hand for the `hand.pantry` secret attribute.
fn fmt_pantry(hand: &Hand) -> String {
    hand.ingredients()
        .iter()
        .map(fmt_ingredient)
        .collect::<Vec<_>>()
        .join(" ")
}

/// A compact rendering of a spell hand for the `hand.spells` secret attribute.
fn fmt_spells(hand: &Hand) -> String {
    hand.spells()
        .iter()
        .map(|s| format!("{:?}", s.kind))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Refresh every open `hand` span's secret pantry/spell attributes from the
/// engine's live hands, so the privileged reveal always reads current state
/// (hands change at every top-up, commit, cast, and Forage draw).
fn record_hands(game: &Game<'_>, hand_spans: &[(PlayerId, tracing::Span)]) {
    for (id, span) in hand_spans {
        if let Some(hand) = game.hand(*id) {
            span.record("hand.pantry", fmt_pantry(hand).as_str());
            span.record("hand.spells", fmt_spells(hand).as_str());
        }
    }
}

/// Surface the open round's active spell effects (unfired primed Actives and a
/// pending Quench) on the round span's secret `effects.active` attribute.
fn record_active_effects(game: &Game<'_>, round_span: &tracing::Span) {
    let (primed, quench) = game.active_effects();
    let mut effects: Vec<String> = primed
        .iter()
        .map(|(caster, kind, target)| match target {
            Some(t) => format!("{kind:?}({}→{})", caster.0, t.0),
            None => format!("{kind:?}({})", caster.0),
        })
        .collect();
    if quench {
        effects.push("Quench(next-wave)".to_string());
    }
    round_span.record("effects.active", effects.join(",").as_str());
}

/// Public table view, marking disconnected (gone) players as not connected.
fn public_players(players: &[SeatInfo], gone: &HashSet<PlayerId>) -> Vec<PlayerPublic> {
    players
        .iter()
        .map(|s| PlayerPublic {
            id: s.id,
            display_name: s.name.clone(),
            color: s.color,
            connected: !gone.contains(&s.id),
            guest: s.guest,
        })
        .collect()
}

/// The single server→client egress point: deliver an already-routed [`Outbound`]
/// to its audience. Routing all sends through here makes the secret boundary
/// load-bearing — the `is_private_only()` debug-assert in
/// [`ServerMessage::broadcast`] fires if a private-only message is ever broadcast.
async fn dispatch(players: &[SeatInfo], out: Outbound) {
    match out.audience {
        Audience::Broadcast => {
            for p in players {
                let _ = p.out.send(out.message.clone()).await;
            }
        }
        Audience::Private(id) => {
            if let Some(p) = players.iter().find(|s| s.id == id) {
                let _ = p.out.send(out.message).await;
            }
        }
    }
}

/// Broadcast a public message to every seat. Constructs the [`Outbound`] via
/// [`ServerMessage::broadcast`], so broadcasting a private-only message trips the
/// debug-assert instead of leaking.
async fn broadcast(players: &[SeatInfo], msg: ServerMessage) {
    dispatch(players, msg.broadcast()).await;
}

/// Send a message privately to one seat, routed through [`Outbound`].
async fn send_to(players: &[SeatInfo], id: PlayerId, msg: ServerMessage) {
    dispatch(players, msg.to(id)).await;
}

fn scores_vec(scores: &HashMap<PlayerId, i32>, order: &[PlayerId]) -> Vec<PlayerScore> {
    order
        .iter()
        .map(|id| PlayerScore {
            player: *id,
            score: scores[id],
        })
        .collect()
}

/// Enumerate one player's complete legal action set for the current wave — the
/// decision-frame contract (`boom-decision-frame`): exact in both directions
/// against the validation this module and the engine apply. Plays mirror the
/// hand-membership check ([`Hand::contains_ingredient`]); the pass is always
/// legal for an active player; casts mirror [`target_shape_ok`] (the engine's
/// `target_valid` re-checks identically), enumerating every legal target.
/// `allow_spell` is false once the player's one allowed spell this wave is
/// spent — the refreshed frame then offers no further casts.
fn wave_commit_frame(
    player: PlayerId,
    hand: &Hand,
    seated: &[PlayerId],
    allow_spell: bool,
) -> PendingDecision {
    let playable: Vec<PlayableIngredient> = hand
        .ingredient_views()
        .into_iter()
        .map(|ingredient| PlayableIngredient {
            ingredient,
            colorless_allowed: true,
        })
        .collect();
    let spells: Vec<CastableSpell> = if allow_spell {
        hand.spell_views()
            .into_iter()
            .map(|s| CastableSpell {
                spell: s.id,
                kind: s.kind,
                targets: match s.kind.target_kind() {
                    TargetKind::None => TargetOptions::None,
                    TargetKind::Player => TargetOptions::Players {
                        players: seated.iter().copied().filter(|p| *p != player).collect(),
                    },
                    TargetKind::Color => TargetOptions::Colors {
                        colors: Color::PLAYER_COLORS.to_vec(),
                    },
                },
            })
            .collect()
    } else {
        Vec::new()
    };
    PendingDecision::WaveCommit {
        playable,
        can_pass: true,
        spells,
    }
}

/// Send `player` their current private hand (ingredients + spells).
async fn send_hand(players: &[SeatInfo], game: &Game<'_>, player: PlayerId) {
    let (ingredients, spells) = game
        .hand(player)
        .map(|h| (h.ingredient_views(), h.spell_views()))
        .unwrap_or_default();
    send_to(
        players,
        player,
        ServerMessage::YourHand {
            ingredients,
            spells,
        },
    )
    .await;
}

/// Run one full game to completion for the given seats. Owns the group's command
/// receiver for the duration so it can collect commits within wave timers. When
/// `pool` is `Some`, the completed game and its replay are persisted at `GameOver`.
#[allow(clippy::too_many_arguments)]
pub async fn run_game(
    registry: &ContentRegistry,
    config: &ContentConfig,
    group_code: GroupCode,
    mut players: Vec<SeatInfo>,
    rx: &mut mpsc::Receiver<GroupCommand>,
    palette: &HashSet<u16>,
    seed: u64,
    pool: Option<&PgPool>,
) -> GameEnd {
    let ids: Vec<PlayerId> = players.iter().map(|p| p.id).collect();
    let mut gone: HashSet<PlayerId> = HashSet::new();

    // The single orchestration core: `run_game` owns no game state of its own. It
    // drives a `Game` — the same engine `Game::play_out` is tested against — through
    // its `begin_round` / `top_up_active` / `resolve_wave` / `settle_round` /
    // `break_tie` steps, adding only the wire I/O (collect commits within a timer,
    // broadcast the public outcome) and the observability spans. The hands, decks,
    // scores, modifiers, RNG, round bookkeeping, per-player/per-round analytics, and
    // the replay action log all live in `Game`, so the shipping path cannot drift
    // from the tested one and the post-game write feeds straight off the engine.
    let game_players: Vec<Player> = players
        .iter()
        .map(|s| Player {
            id: s.id,
            color: s.color,
            display_name: s.name.clone(),
        })
        .collect();
    let mut game = Game::new(registry, config, game_players, seed);

    metric::game_started();
    let game_start = std::time::Instant::now();
    let started_at = Utc::now();
    // Every raw in-game input players send, stamped ms-since-game-start, for the
    // replay payload (observational only — the engine reconstructs from the
    // deterministic seed + action log, not from this).
    let mut input_log: Vec<TimedInput> = Vec::new();
    tracing::info!(players = players.len(), "game started");

    // `game` span (span_schema::span::GAME) — child of the caller's group.lifetime
    // span. Held open for the whole game; the deck seed rides as a sensitive
    // attribute (admin-only via the reveal, never on the player wire). Field names
    // match `span_schema::attr`.
    let game_id = Uuid::new_v4();
    let game_span = tracing::info_span!(
        "game",
        game.id = %game_id,
        players.count = players.len(),
        schema.version = SPAN_SCHEMA_VERSION,
        deck_seed = seed,
    );

    for round_number in 1..=ROUND_COUNT {
        // Open the round through the engine (modifier draw, spell draw + top-up,
        // hidden boiling point, active set excluding the disconnected), then
        // announce it on the wire.
        let opening = game.begin_round(round_number, &gone);
        let effective_bp = opening.effective_boiling_point;
        if let Some(kind) = opening.modifier {
            broadcast(
                &players,
                ServerMessage::ModifierRevealed {
                    modifier: kind,
                    round_number,
                },
            )
            .await;
        }
        for id in &ids {
            send_hand(&players, &game, *id).await;
        }

        let round_start = std::time::Instant::now();

        // `round` span — child of the game span; held open for the whole round.
        // boiling_point/volatility_total/effects.active are secret (in-process
        // only); round.number, the boom/freeze outcome, and modifiers are public
        // live-registry keys/outcomes (clients already see each ModifierRevealed).
        let mods_str = game
            .active_modifiers()
            .iter()
            .map(|m| format!("{m:?}"))
            .collect::<Vec<_>>()
            .join(",");
        let round_span = game_span.in_scope(|| {
            tracing::info_span!(
                "round",
                round.number = round_number as u64,
                boiling_point = effective_bp as i64,
                volatility_total = tracing::field::Empty,
                effects.active = tracing::field::Empty,
                round.boomed = tracing::field::Empty,
                round.frozen = tracing::field::Empty,
                modifiers = %mods_str,
            )
        });

        // `hand` spans — one per seated player, child of the round, held open for
        // the whole round so the privileged reveal can read each player's pantry
        // and spell hands from a live span. Both ride as secret attributes
        // (in-process only), refreshed whenever the engine's hands change.
        let hand_spans: Vec<(PlayerId, tracing::Span)> = ids
            .iter()
            .map(|id| {
                let span = round_span.in_scope(|| {
                    tracing::info_span!(
                        "hand",
                        player.id = %id.0,
                        hand.pantry = tracing::field::Empty,
                        hand.spells = tracing::field::Empty,
                    )
                });
                (*id, span)
            })
            .collect();
        record_hands(&game, &hand_spans);

        // The round-ending wave's `resolve` span is held past the wave loop so the
        // settlement can record the pot value P and the detonator split on it.
        let mut fatal_resolve: Option<tracing::Span> = None;
        let mut first_wave = true;
        while game.round_is_open() {
            let wave_no = game.wave_number();
            // The start-of-wave ingredient top-up (idempotent for wave 1, whose
            // deal happened at round open) — then refresh each active player's
            // private hand so they pick from true state.
            let topped = game.top_up_active();
            if !first_wave {
                for id in &topped {
                    send_hand(&players, &game, *id).await;
                }
            }
            first_wave = false;
            record_hands(&game, &hand_spans);

            let acting: Vec<PlayerId> = game.active().to_vec();
            let timer_ms = if wave_no == 1 {
                config.timing.wave1_ms
            } else {
                config.timing.wave_ms
            };
            // `wave` span — child of the round; held open for the whole commit
            // window so the live registry shows the in-flight wave.
            let wave_start = std::time::Instant::now();
            let wave_span = round_span.in_scope(|| {
                tracing::info_span!(
                    "wave",
                    wave.number = wave_no as u64,
                    wave.timer_ms = timer_ms,
                    wave.timed_out = tracing::field::Empty,
                    wave.commits = tracing::field::Empty,
                    wave.passes = tracing::field::Empty,
                )
            });
            // Phase-advance invalidation (`boom-decision-frame`): anything still
            // queued at this point was sent against a frame that has already been
            // resolved — reject those actions (StaleFrame, no state change)
            // before the new wave opens and fresh frames go out.
            let stale_reconnected = reject_stale(
                rx,
                &mut players,
                &mut gone,
                palette,
                game_start,
                &mut input_log,
            )
            .await;
            broadcast(
                &players,
                ServerMessage::WaveOpened {
                    round_number,
                    wave_number: wave_no,
                    timer_ms,
                    // Only one active player left ⇒ this is their single final wave.
                    final_wave: acting.len() == 1,
                },
            )
            .await;
            // Each connected acting player owes this wave's commit: send them the
            // enumerated legal action set (a disconnected seat auto-passes, so it
            // owes nothing).
            for player in &acting {
                if gone.contains(player) {
                    continue;
                }
                if let Some(hand) = game.hand(*player) {
                    send_to(
                        &players,
                        *player,
                        ServerMessage::DecisionFrame {
                            round_number,
                            wave_number: wave_no,
                            timer_ms: Some(timer_ms),
                            decision: wave_commit_frame(*player, hand, &ids, true),
                        },
                    )
                    .await;
                }
            }

            let collection = collect_wave(
                rx,
                &mut players,
                &acting,
                game.hands(),
                &mut gone,
                palette,
                round_number,
                wave_no,
                timer_ms,
                game_start,
                &mut input_log,
                &wave_span,
            )
            .await;
            let wave_timed_out = collection.timed_out;
            // Reconnected players (during the stale drain or the commit window)
            // resume for future rounds and get a private snapshot.
            for player in stale_reconnected.iter().chain(&collection.reconnected) {
                // `reconnect` span — child of the game span; player.id is public.
                let _reconnect =
                    game_span.in_scope(|| tracing::info_span!("reconnect", player.id = %player.0));
                metric::record(&BalanceEvent::PlayerReconnected);
                gone.remove(player);
                let (your_ingredients, your_spells) = game
                    .hand(*player)
                    .map(|h| (h.ingredient_views(), h.spell_views()))
                    .unwrap_or_default();
                let snapshot = ServerMessage::StateSnapshot {
                    group_code: group_code.clone(),
                    your_player_id: *player,
                    round_number,
                    players: public_players(&players, &gone),
                    scores: scores_vec(game.scores(), &ids),
                    active_modifiers: game.active_modifiers().to_vec(),
                    contributions: to_contributions(game.contributions(&ids)),
                    your_ingredients,
                    your_spells,
                };
                send_to(&players, *player, snapshot).await;
                tracing::info!(player = %player.0, "player reconnected");
            }

            // `resolve` span — child of the wave; pot.card_count is public. The engine
            // validates choices against hands, removes the committed cards, applies the
            // wave, draws Forage spells, and records the per-wave action log (the
            // deterministic replay input) on the shared `Game`. The round-ending
            // (fatal) wave's resolve span is held open so the settlement can record
            // the pot value P and the detonator split on it.
            let resolve_span = wave_span.in_scope(|| {
                tracing::info_span!(
                    "resolve",
                    pot.card_count = tracing::field::Empty,
                    pot.value = tracing::field::Empty,
                    detonators = tracing::field::Empty,
                )
            });
            let resolution = resolve_span.in_scope(|| game.resolve_wave(&collection.choices));
            resolve_span.record("pot.card_count", resolution.pot_card_count as u64);
            if resolution.ended == Some(RoundEnd::Exploded) {
                fatal_resolve = Some(resolve_span);
            } else {
                drop(resolve_span);
            }

            let played: Vec<PlayerId> = resolution.committed.iter().map(|(p, _, _)| *p).collect();

            // Surface the wave outcome, the live running volatility, and the active
            // spell effects on the open spans (Update lifecycle events), so the
            // reveal shows current state. wave.commits/wave.passes are public (the
            // WaveResolved broadcast carries who played and who passed); they also
            // feed the fold-rate metric fold.
            wave_span.record("wave.timed_out", wave_timed_out);
            wave_span.record("wave.commits", played.len() as u64);
            wave_span.record("wave.passes", resolution.passers.len() as u64);
            round_span.record("volatility_total", resolution.pot_volatility as i64);
            record_active_effects(&game, &round_span);
            record_hands(&game, &hand_spans);
            metric::record(&BalanceEvent::WaveResolved {
                timed_out: wave_timed_out,
                commits: played.len() as u64,
                folds: resolution.passers.len() as u64,
                duration_ms: wave_start.elapsed().as_millis() as u64,
            });

            // Visible-when-activated: Instant casts are public (caster + spell +
            // any colour target), in resolution order — each one a `spell.cast`
            // leaf span under the wave. Primed Actives stay silent.
            for (caster, spell, color_target) in &resolution.casts {
                let cast_span = wave_span.in_scope(|| {
                    tracing::info_span!(
                        "spell.cast",
                        player.id = %caster.0,
                        spell.kind = ?spell,
                        spell.target = tracing::field::Empty,
                    )
                });
                if let Some(color) = color_target {
                    cast_span.record("spell.target", format!("{color:?}").as_str());
                }
                drop(cast_span);
                metric::record(&BalanceEvent::SpellCast {
                    kind: format!("{spell:?}"),
                });
                broadcast(
                    &players,
                    ServerMessage::SpellCast {
                        player: *caster,
                        spell: *spell,
                        color_target: *color_target,
                    },
                )
                .await;
            }
            for (owner, ingredient, colorless) in &resolution.exposed {
                broadcast(
                    &players,
                    ServerMessage::Exposed {
                        player: *owner,
                        ingredient: *ingredient,
                        colorless: *colorless,
                    },
                )
                .await;
            }
            for (caster, dominant, lead) in &resolution.assays {
                send_to(
                    &players,
                    *caster,
                    ServerMessage::AssayResult {
                        dominant: *dominant,
                        lead: *lead,
                    },
                )
                .await;
            }
            for peeker in &resolution.peeked {
                send_to(
                    &players,
                    *peeker,
                    ServerMessage::PeekResult {
                        boiling_point: effective_bp.max(0) as u8,
                    },
                )
                .await;
            }
            // A Forage grew an owner's spell hand: re-send each affected owner a
            // private `YourHand` so the owning client tracks its true hand.
            for player in &resolution.hand_changed {
                send_hand(&players, &game, *player).await;
            }

            broadcast(
                &players,
                ServerMessage::WaveResolved {
                    played,
                    passed: resolution.passers,
                    cauldron_card_count: resolution.pot_card_count,
                    contributions: to_contributions(game.contributions(&ids)),
                },
            )
            .await;
        }

        // Settle the round through the engine (depile + scoring + analytics), then
        // broadcast the public outcome. The engine has already folded the deltas into
        // the cumulative scores and logged the round.
        let settlement = game.settle_round();
        let depile = settlement.depile;
        let boomed = matches!(settlement.scoring, RoundScoring::Exploded(_));
        // A freeze: the round settled with an empty pot (everyone passed).
        let frozen = !boomed && depile.reveals.is_empty();
        // Round outcome onto the round span: volatility_total is secret (the final
        // running volatility); the boom/freeze outcome is public after the depile.
        round_span.record(
            "volatility_total",
            depile
                .reveals
                .last()
                .map(|i| i.running_volatility)
                .unwrap_or(0) as i64,
        );
        round_span.record("round.boomed", boomed);
        round_span.record("round.frozen", frozen);

        // `depile` span — child of the round: the volatility-sorted reveal (the
        // fatal-wave sort). Everything on it is public at this point — the boiling
        // point is revealed EVERY round (boom and safe), the near-miss payoff; a
        // `!` marks an entry liable for the boom, a `~` one played colorless.
        let reveals_str = depile
            .reveals
            .iter()
            .map(|item| {
                format!(
                    "{}:{}@w{}{}{}",
                    item.player.0,
                    fmt_ingredient(&item.ingredient),
                    item.wave_number,
                    if item.colorless { "~" } else { "" },
                    if item.liable { "!" } else { "" },
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let depile_span = round_span.in_scope(|| {
            tracing::info_span!(
                "depile",
                boiling_point = depile.boiling_point as i64,
                reveals = %reveals_str,
                crossing_index = tracing::field::Empty,
            )
        });
        if let Some(idx) = depile.crossing_index {
            depile_span.record("crossing_index", idx as u64);
        }
        drop(depile_span);
        broadcast(
            &players,
            ServerMessage::Depile {
                reveals: depile_entries(&depile),
                exploded: boomed,
                boiling_point: depile.boiling_point,
                crossing_index: depile.crossing_index,
            },
        )
        .await;

        // `score` span — child of the round; the boom outcome, the pot value P,
        // and the detonator split are all public at settlement.
        let score_span = round_span.in_scope(|| {
            tracing::info_span!(
                "score",
                round.boomed = boomed,
                pot.value = tracing::field::Empty,
                detonators = tracing::field::Empty,
            )
        });
        match settlement.scoring {
            RoundScoring::Exploded(result) => {
                let detonators_csv = result
                    .detonators
                    .iter()
                    .map(|p| p.0.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                score_span.record("pot.value", result.pot_value as i64);
                score_span.record("detonators", detonators_csv.as_str());
                // The fatal wave's resolve span carries the pot value P and the
                // detonator split, completing it before it closes.
                if let Some(resolve) = fatal_resolve.take() {
                    resolve.record("pot.value", result.pot_value as i64);
                    resolve.record("detonators", detonators_csv.as_str());
                }
                metric::record(&BalanceEvent::Boom {
                    detonators: result.detonators.len() as u64,
                });
                broadcast(
                    &players,
                    ServerMessage::Explosion {
                        pot_value: result.pot_value,
                        detonators: result.detonators,
                        deltas: result
                            .deltas
                            .iter()
                            .map(|(p, d)| PlayerScore {
                                player: *p,
                                score: *d,
                            })
                            .collect(),
                        fired: result.fired,
                    },
                )
                .await;
            }
            RoundScoring::Safe(result) => {
                let outcome = if result.winners.len() == 1 {
                    ScoringOutcome::Domination {
                        winner: result.winners[0],
                    }
                } else {
                    ScoringOutcome::Split {
                        colors: result.winners.clone(),
                    }
                };
                score_span.record("pot.value", result.pot_value as i64);
                broadcast(
                    &players,
                    ServerMessage::RoundScored {
                        color_points: result.color_points,
                        outcome,
                        awards: result
                            .awards
                            .iter()
                            .map(|(p, s)| PlayerScore {
                                player: *p,
                                score: *s,
                            })
                            .collect(),
                        fired: result.fired,
                    },
                )
                .await;
            }
        }
        broadcast(
            &players,
            ServerMessage::ScoreUpdate {
                scores: scores_vec(game.scores(), &ids),
            },
        )
        .await;
        drop(score_span);

        metric::record(&BalanceEvent::RoundSettled {
            boomed,
            frozen,
            duration_ms: round_start.elapsed().as_millis() as u64,
        });
    }

    // Game over — break a tie for the lead with a Deathmatch (the engine's tiebreak
    // core; `run_game` only announces it on the wire first).
    let leaders = game.leaders();
    let winners = if leaders.len() > 1 {
        broadcast(
            &players,
            ServerMessage::DeathmatchStarted {
                participants: leaders.clone(),
            },
        )
        .await;
        game.break_tie(&leaders)
    } else {
        leaders
    };

    metric::record(&BalanceEvent::GameCompleted {
        duration_ms: game_start.elapsed().as_millis() as u64,
    });
    tracing::info!(?winners, "game over");
    broadcast(
        &players,
        ServerMessage::GameOver {
            final_scores: scores_vec(game.scores(), &ids),
            winners: winners.clone(),
        },
    )
    .await;

    // Post-game completion write: one consolidated game_replays row, fed straight
    // off the engine's analytics + action log + the recorded raw-input log. Skipped
    // cleanly when no database is configured.
    persist_completed_game(
        pool,
        config,
        game_id,
        started_at,
        &players,
        game.scores(),
        game.cards_played(),
        game.round_logs(),
        game.action_log(),
        &winners,
        &input_log,
        seed,
    )
    .await;

    // Hand the final seats (with reconnections' refreshed channels), the
    // still-absent players, and the winners back to the persistent group, which
    // returns the survivors to its lobby and folds the result into standings.
    GameEnd {
        players,
        gone,
        winners,
    }
}

/// Map the engine's per-player pot-contribution counts onto the public wire signal.
fn to_contributions(counts: Vec<(PlayerId, u8)>) -> Vec<Contribution> {
    counts
        .into_iter()
        .map(|(player, count)| Contribution { player, count })
        .collect()
}

/// The post-game completion write (the only DB write the server performs).
/// Encodes the replay payload (seed + action log + recorded raw inputs), builds
/// the consolidated [`CompletedGame`](crate::persistence::CompletedGame), and
/// persists it in [`persist_game`]'s single transaction (the `db.write` span).
/// With no database configured this is a clean no-op — persistence is optional
/// infrastructure, never a precondition for play.
///
/// Because the row *is* the replay, an (unexpected) replay-encode failure skips
/// the whole write rather than persisting a payload-less stub.
#[allow(clippy::too_many_arguments)]
async fn persist_completed_game(
    pool: Option<&PgPool>,
    config: &ContentConfig,
    game_id: Uuid,
    started_at: chrono::DateTime<Utc>,
    players: &[SeatInfo],
    scores: &HashMap<PlayerId, i32>,
    cards_played: &HashMap<PlayerId, u32>,
    rounds: &[RoundLog],
    action_log: &[WaveChoice],
    winners: &[PlayerId],
    input_log: &[TimedInput],
    seed: u64,
) {
    let Some(pool) = pool else {
        tracing::trace!(game.id = %game_id, "no database configured; completion write skipped");
        return;
    };
    let replay = match encode_replay(
        game_id,
        seed,
        config,
        players.iter().map(|p| (p.id, p.color, p.name.clone())),
        action_log,
        input_log,
    ) {
        Ok(replay) => replay,
        Err(e) => {
            tracing::error!(game.id = %game_id, error = %e, "failed to encode replay; skipping completion write");
            return;
        }
    };
    let completed = build_completed_game(
        players.iter().map(|p| (p.id, p.color, p.name.clone())),
        scores,
        cards_played,
        rounds,
        winners,
        game_id,
        started_at,
        Utc::now(),
        replay,
    );
    if let Err(e) = persist_game(pool, &completed).await {
        tracing::error!(game.id = %game_id, error = %e, "failed to persist completed game");
    }
}

/// Map an in-game client message to its recorded replay form. Returns `None` for
/// messages that are not in-game player inputs (heartbeats, lobby/entry messages).
fn recorded_input(msg: &ClientMessage) -> Option<RecordedInput> {
    match msg {
        ClientMessage::CommitIngredient { card, colorless } => {
            Some(RecordedInput::CommitIngredient {
                card: *card,
                colorless: *colorless,
            })
        }
        ClientMessage::CastSpell { spell, target } => Some(RecordedInput::CastSpell {
            spell: *spell,
            target: *target,
        }),
        ClientMessage::CommitPass => Some(RecordedInput::CommitPass),
        ClientMessage::LockIn => Some(RecordedInput::LockIn),
        ClientMessage::Emote { emote } => Some(RecordedInput::Emote { emote: *emote }),
        _ => None,
    }
}

/// Reject everything still queued from before the current wave opened — the
/// stale-frame rule (`boom-decision-frame`): a submission against a frame whose
/// decision has already been resolved gets [`ErrorCode::StaleFrame`] and changes
/// no state; the auto-resolved outcome stands. Liveness traffic is still
/// serviced (heartbeats answered, palette emotes broadcast), departures are
/// folded into `gone`, and reconnects reattach their channel (returned so the
/// caller sends their snapshot exactly as for mid-wave reconnects). Raw inputs
/// are recorded before rejection, like everything else players send.
async fn reject_stale(
    rx: &mut mpsc::Receiver<GroupCommand>,
    players: &mut [SeatInfo],
    gone: &mut HashSet<PlayerId>,
    palette: &HashSet<u16>,
    game_start: std::time::Instant,
    input_log: &mut Vec<TimedInput>,
) -> Vec<PlayerId> {
    let mut reconnected: Vec<PlayerId> = Vec::new();
    while let Ok(cmd) = rx.try_recv() {
        match cmd {
            GroupCommand::Action { player, msg } => {
                if let Some(input) = recorded_input(&msg) {
                    input_log.push(TimedInput {
                        player,
                        at_ms: game_start.elapsed().as_millis() as u32,
                        input,
                    });
                }
                match msg {
                    ClientMessage::CommitIngredient { .. }
                    | ClientMessage::CastSpell { .. }
                    | ClientMessage::CommitPass
                    | ClientMessage::LockIn => {
                        send_to(
                            players,
                            player,
                            ServerMessage::Error {
                                code: ErrorCode::StaleFrame,
                                message: "that decision has already been resolved".into(),
                            },
                        )
                        .await;
                    }
                    ClientMessage::Heartbeat => {
                        send_to(players, player, ServerMessage::Heartbeat).await;
                    }
                    ClientMessage::Emote { emote } if palette.contains(&emote.0) => {
                        broadcast(
                            players,
                            ServerMessage::EmoteBroadcast {
                                from: player,
                                emote,
                            },
                        )
                        .await;
                    }
                    ClientMessage::Emote { .. } => {
                        send_to(
                            players,
                            player,
                            ServerMessage::Error {
                                code: ErrorCode::InvalidEmote,
                                message: "unknown emote".into(),
                            },
                        )
                        .await;
                    }
                    // Entry and group-lobby messages are never valid mid-game.
                    _ => {
                        send_to(
                            players,
                            player,
                            ServerMessage::Error {
                                code: ErrorCode::WrongPhase,
                                message: "not a valid action during a wave".into(),
                            },
                        )
                        .await;
                    }
                }
            }
            GroupCommand::Leave { player } => {
                gone.insert(player);
            }
            GroupCommand::Join { player, out, .. } => {
                if let Some(seat) = players.iter_mut().find(|s| s.id == player) {
                    seat.out = out;
                    reconnected.push(player);
                } else {
                    let _ = out
                        .send(ServerMessage::Error {
                            code: ErrorCode::WrongPhase,
                            message: "game already in progress".into(),
                        })
                        .await;
                }
            }
            GroupCommand::ForceStart => {}
            GroupCommand::Shutdown => break,
        }
    }
    reconnected
}

/// A player's pending choice as a wave collects: the ingredient-or-pass slot may
/// be revised until lock-in; the spell slot is one-shot (a second cast is
/// rejected — at most one spell resolves per player per wave).
#[derive(Default)]
struct Pending {
    action: Option<WaveAction>,
    spell: Option<SpellChoice>,
}

/// Validate a spell's target shape on the wire (the engine re-validates).
fn target_shape_ok(
    kind: boiling_point_protocol::vocab::SpellKind,
    target: Option<SpellTarget>,
    caster: PlayerId,
    seated: &[PlayerId],
) -> bool {
    match kind.target_kind() {
        TargetKind::None => target.is_none(),
        TargetKind::Player => matches!(
            target,
            Some(SpellTarget::Player { player })
                if player != caster && seated.contains(&player)
        ),
        TargetKind::Color => matches!(
            target,
            Some(SpellTarget::Color { color }) if color != Color::Wild
        ),
    }
}

/// Collect one wave's hidden commits until the timer expires or every active
/// player has locked in. Heartbeats and emotes are serviced live; a disconnect
/// (`Leave`) auto-passes the player for the rest of the game.
///
/// Every raw in-game input (commit/cast/pass/lock-in/emote) is appended to
/// `input_log` as it arrives — *before* validation, so rejected/off-palette
/// attempts are captured too — stamped with `game_start.elapsed()` for the
/// replay payload.
///
/// Each accepted (hidden) commit opens a `commit` leaf span under `wave_span`,
/// carrying the card identity and its Vote colour as secret attributes, so the
/// privileged reveal can show committed-but-unrevealed plays while the wave is
/// open. A revised commit updates its span; a revision to pass closes it. All
/// remaining commit spans close when collection returns (the wave resolves).
#[allow(clippy::too_many_arguments)]
async fn collect_wave(
    rx: &mut mpsc::Receiver<GroupCommand>,
    players: &mut [SeatInfo],
    acting: &[PlayerId],
    hands: &HashMap<PlayerId, Hand>,
    gone: &mut HashSet<PlayerId>,
    palette: &HashSet<u16>,
    round_number: u8,
    wave_number: u8,
    timer_ms: u32,
    game_start: std::time::Instant,
    input_log: &mut Vec<TimedInput>,
    wave_span: &tracing::Span,
) -> WaveCollection {
    let seated: Vec<PlayerId> = players.iter().map(|s| s.id).collect();
    // When the commit window closes — for the refreshed frame's remaining budget.
    let deadline = std::time::Instant::now() + Duration::from_millis(timer_ms as u64);
    let mut pending: HashMap<PlayerId, Pending> = HashMap::new();
    let mut locked: HashSet<PlayerId> = HashSet::new();
    let mut reconnected: Vec<PlayerId> = Vec::new();
    let mut commit_spans: HashMap<PlayerId, tracing::Span> = HashMap::new();
    // Disconnected players auto-pass and are considered locked in.
    for p in acting {
        if gone.contains(p) {
            pending.entry(*p).or_default().action = Some(WaveAction::Pass);
            locked.insert(*p);
        }
    }

    let sleep = tokio::time::sleep(Duration::from_millis(timer_ms as u64));
    tokio::pin!(sleep);
    let mut timed_out = false;
    while !acting.iter().all(|p| locked.contains(p)) {
        tokio::select! {
            _ = &mut sleep => { timed_out = true; break; }
            maybe = rx.recv() => {
                match maybe {
                    None => break,
                    Some(GroupCommand::Action { player, msg }) => {
                        let active = acting.contains(&player) && !gone.contains(&player);
                        // Record the raw input (everything sent, incl. rejected
                        // attempts) before validating it.
                        if let Some(input) = recorded_input(&msg) {
                            input_log.push(TimedInput {
                                player,
                                at_ms: game_start.elapsed().as_millis() as u32,
                                input,
                            });
                        }
                        match msg {
                            ClientMessage::CommitIngredient { card, colorless } if active => {
                                // §I: a card the player doesn't hold is an invalid
                                // action, not a silent drop. The reply carries only the
                                // reason — never pot/volatility/boiling-point state — so
                                // it cannot weaken blind volatility.
                                let held = hands
                                    .get(&player)
                                    .and_then(|h| h.ingredients().iter().find(|c| c.id == card))
                                    .copied();
                                if let Some(ingredient) = held {
                                    pending.entry(player).or_default().action =
                                        Some(WaveAction::Play { card, colorless });
                                    // Open (or revise) the player's hidden `commit`
                                    // span: card identity and Vote colour are secret
                                    // until the depile.
                                    let card_str = fmt_ingredient(&ingredient);
                                    let vote = if colorless {
                                        "colorless".to_string()
                                    } else {
                                        format!("{:?}", ingredient.color)
                                    };
                                    match commit_spans.get(&player) {
                                        Some(span) => {
                                            span.record("committed_card", card_str.as_str());
                                            span.record("vote.color", vote.as_str());
                                        }
                                        None => {
                                            let span = wave_span.in_scope(|| {
                                                tracing::info_span!(
                                                    "commit",
                                                    player.id = %player.0,
                                                    committed_card = %card_str,
                                                    vote.color = %vote,
                                                )
                                            });
                                            commit_spans.insert(player, span);
                                        }
                                    }
                                } else {
                                    send_to(
                                        players,
                                        player,
                                        ServerMessage::Error {
                                            code: ErrorCode::NotYourCard,
                                            message: "that ingredient is not in your hand".into(),
                                        },
                                    )
                                    .await;
                                }
                            }
                            ClientMessage::CastSpell { spell, target } if active => {
                                let entry = pending.entry(player).or_default();
                                if entry.spell.is_some() {
                                    // At most one spell resolves per player per wave.
                                    send_to(
                                        players,
                                        player,
                                        ServerMessage::Error {
                                            code: ErrorCode::SpellLimit,
                                            message: "you already cast a spell this wave".into(),
                                        },
                                    )
                                    .await;
                                } else {
                                    let kind = hands
                                        .get(&player)
                                        .and_then(|h| h.spells().iter().find(|s| s.id == spell))
                                        .map(|s| s.kind);
                                    match kind {
                                        None => {
                                            send_to(
                                                players,
                                                player,
                                                ServerMessage::Error {
                                                    code: ErrorCode::NotYourSpell,
                                                    message: "that spell is not in your grimoire hand".into(),
                                                },
                                            )
                                            .await;
                                        }
                                        Some(kind) if !target_shape_ok(kind, target, player, &seated) => {
                                            send_to(
                                                players,
                                                player,
                                                ServerMessage::Error {
                                                    code: ErrorCode::InvalidTarget,
                                                    message: "that spell cannot take that target".into(),
                                                },
                                            )
                                            .await;
                                        }
                                        Some(_) => {
                                            pending.entry(player).or_default().spell =
                                                Some(SpellChoice { spell, target });
                                            // The one allowed cast this wave is now
                                            // spent: refresh the caster's frame so the
                                            // legal set they hold offers no further
                                            // spells (frame exactness on refresh).
                                            if let Some(hand) = hands.get(&player) {
                                                let remaining = deadline
                                                    .saturating_duration_since(
                                                        std::time::Instant::now(),
                                                    )
                                                    .as_millis()
                                                    as u32;
                                                send_to(
                                                    players,
                                                    player,
                                                    ServerMessage::DecisionFrame {
                                                        round_number,
                                                        wave_number,
                                                        timer_ms: Some(remaining),
                                                        decision: wave_commit_frame(
                                                            player, hand, &seated, false,
                                                        ),
                                                    },
                                                )
                                                .await;
                                            }
                                        }
                                    }
                                }
                            }
                            ClientMessage::CommitPass if active => {
                                pending.entry(player).or_default().action = Some(WaveAction::Pass);
                                // A revision to pass closes any open commit span: the
                                // reveal must not show a play that no longer stands.
                                commit_spans.remove(&player);
                            }
                            ClientMessage::LockIn if active => {
                                locked.insert(player);
                            }
                            // A commit/cast/pass/lock-in from a player who has already
                            // passed, timed out, or is otherwise not acting this round:
                            // reply LockedOut rather than drop it (§I). No state changes.
                            ClientMessage::CommitIngredient { .. }
                            | ClientMessage::CastSpell { .. }
                            | ClientMessage::CommitPass
                            | ClientMessage::LockIn => {
                                send_to(
                                    players,
                                    player,
                                    ServerMessage::Error {
                                        code: ErrorCode::LockedOut,
                                        message: "you are locked out of this round".into(),
                                    },
                                )
                                .await;
                            }
                            ClientMessage::Heartbeat => {
                                send_to(players, player, ServerMessage::Heartbeat).await;
                            }
                            ClientMessage::Emote { emote } if palette.contains(&emote.0) => {
                                broadcast(
                                    players,
                                    ServerMessage::EmoteBroadcast { from: player, emote },
                                )
                                .await;
                            }
                            // An off-palette emote is rejected exactly as in the lobby.
                            ClientMessage::Emote { .. } => {
                                send_to(
                                    players,
                                    player,
                                    ServerMessage::Error {
                                        code: ErrorCode::InvalidEmote,
                                        message: "unknown emote".into(),
                                    },
                                )
                                .await;
                            }
                            // Entry and group-lobby messages (create/join/enqueue,
                            // play-again, fill, leave) are never valid mid-game:
                            // reply WrongPhase, never silently drop (§I).
                            ClientMessage::CreateGroup { .. }
                            | ClientMessage::JoinGroup { .. }
                            | ClientMessage::EnqueueMatch { .. }
                            | ClientMessage::PlayAgain
                            | ClientMessage::FillGroup
                            | ClientMessage::CancelFill
                            | ClientMessage::LeaveGroup => {
                                send_to(
                                    players,
                                    player,
                                    ServerMessage::Error {
                                        code: ErrorCode::WrongPhase,
                                        message: "not a valid action during a wave".into(),
                                    },
                                )
                                .await;
                            }
                        }
                    }
                    Some(GroupCommand::Leave { player }) => {
                        gone.insert(player);
                        if acting.contains(&player) {
                            let entry = pending.entry(player).or_default();
                            entry.action = Some(WaveAction::Pass);
                            locked.insert(player);
                            commit_spans.remove(&player);
                        }
                    }
                    Some(GroupCommand::Join { player, out, .. }) => {
                        // A reconnect: reattach the returning player's channel.
                        // The snapshot is sent by the caller once the wave settles.
                        if let Some(seat) = players.iter_mut().find(|s| s.id == player) {
                            seat.out = out;
                            reconnected.push(player);
                        } else {
                            // An unseated joiner mid-game: a private Error sent direct
                            // to their channel (they are not in `players`, so this
                            // can't route through `send_to`).
                            let _ = out
                                .send(ServerMessage::Error {
                                    code: ErrorCode::WrongPhase,
                                    message: "game already in progress".into(),
                                })
                                .await;
                        }
                    }
                    // Force-start is meaningless mid-game; an operator kill closes
                    // the current commit window (the lobby loop owns full teardown).
                    Some(GroupCommand::ForceStart) => {}
                    Some(GroupCommand::Shutdown) => break,
                }
            }
        }
    }

    // The collected intents are handed to the engine, which validates them against
    // hands again, removes the committed cards, and normalises invalid inputs.
    // A player with a spell but no action by close auto-passes — the spell still
    // resolves (a spell never keeps a passed player in).
    let choices: HashMap<PlayerId, WaveChoice> = pending
        .into_iter()
        .map(|(player, p)| {
            (
                player,
                WaveChoice {
                    action: p.action.unwrap_or(WaveAction::Pass),
                    spell: p.spell,
                },
            )
        })
        .collect();
    WaveCollection {
        choices,
        reconnected,
        timed_out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ContentConfig;
    use crate::game::card::Ingredient;
    use boiling_point_protocol::server::PlayerScore;
    use boiling_point_protocol::vocab::SpellKind;
    use boiling_point_protocol::{CardId, EmoteId};

    /// A seated player wired to a fresh outbound channel; the returned receiver
    /// collects what the loop sends them.
    fn seat(n: u128, color: Color) -> (SeatInfo, mpsc::Receiver<ServerMessage>) {
        let (tx, rx) = mpsc::channel(64);
        (
            SeatInfo {
                id: PlayerId(Uuid::from_u128(n)),
                name: format!("p{n}"),
                color,
                guest: false,
                out: tx,
            },
            rx,
        )
    }

    fn ing(id: u32, color: Color, vol: u8, pts: u8) -> Ingredient {
        Ingredient {
            id: CardId(id),
            color,
            volatility: vol,
            points: pts,
        }
    }

    fn drain(rx: &mut mpsc::Receiver<ServerMessage>) -> Vec<ServerMessage> {
        let mut out = Vec::new();
        while let Ok(m) = rx.try_recv() {
            out.push(m);
        }
        out
    }

    // ---- invalid in-wave actions get an error, never a silent drop (§I) ----

    #[tokio::test]
    async fn bad_commit_replies_not_your_card_and_changes_no_state() {
        let (s1, mut rx1) = seat(1, Color::Ruby);
        let id1 = s1.id;
        let mut players = vec![s1];
        let mut hands: HashMap<PlayerId, Hand> = HashMap::new();
        let mut h = Hand::new();
        h.add_ingredients([ing(10, Color::Ruby, 2, 1)]);
        hands.insert(id1, h);
        let mut gone = HashSet::new();
        let palette: HashSet<u16> = HashSet::new();

        let (cmd_tx, mut cmd_rx) = mpsc::channel(8);
        cmd_tx
            .send(GroupCommand::Action {
                player: id1,
                msg: ClientMessage::CommitIngredient {
                    card: CardId(99),
                    colorless: false,
                },
            })
            .await
            .unwrap();
        drop(cmd_tx); // wave closes once the queued action is drained

        let collection = collect_wave(
            &mut cmd_rx,
            &mut players,
            &[id1],
            &hands,
            &mut gone,
            &palette,
            1,
            1,
            5_000,
            std::time::Instant::now(),
            &mut Vec::new(),
            &tracing::Span::none(),
        )
        .await;

        // No state change: an unheld card records no choice (the engine would treat
        // the absent choice as a pass), and the real card stays in hand untouched —
        // collection never mutates a hand.
        assert!(
            collection.choices.is_empty(),
            "an unheld card must not record a choice"
        );
        assert!(hands[&id1].contains_ingredient(CardId(10)));

        // The only reply is the NotYourCard error (reason only — no hidden state).
        let msgs = drain(&mut rx1);
        assert!(
            matches!(
                msgs.as_slice(),
                [ServerMessage::Error {
                    code: ErrorCode::NotYourCard,
                    ..
                }]
            ),
            "expected one NotYourCard error, got {msgs:?}"
        );
    }

    #[tokio::test]
    async fn second_spell_in_a_wave_is_rejected_with_spell_limit() {
        let (s1, mut rx1) = seat(1, Color::Ruby);
        let id1 = s1.id;
        let mut players = vec![s1];
        let mut hands: HashMap<PlayerId, Hand> = HashMap::new();
        let mut h = Hand::new();
        h.add_ingredients([ing(10, Color::Ruby, 2, 1)]);
        h.add_spells([
            crate::game::card::Spell {
                id: CardId(20),
                kind: SpellKind::Peek,
            },
            crate::game::card::Spell {
                id: CardId(21),
                kind: SpellKind::Surge,
            },
        ]);
        hands.insert(id1, h);
        let mut gone = HashSet::new();
        let palette: HashSet<u16> = HashSet::new();

        let (cmd_tx, mut cmd_rx) = mpsc::channel(8);
        for spell in [CardId(20), CardId(21)] {
            cmd_tx
                .send(GroupCommand::Action {
                    player: id1,
                    msg: ClientMessage::CastSpell {
                        spell,
                        target: None,
                    },
                })
                .await
                .unwrap();
        }
        drop(cmd_tx);

        let collection = collect_wave(
            &mut cmd_rx,
            &mut players,
            &[id1],
            &hands,
            &mut gone,
            &palette,
            1,
            1,
            5_000,
            std::time::Instant::now(),
            &mut Vec::new(),
            &tracing::Span::none(),
        )
        .await;

        // Only the first cast is kept; the second got a SpellLimit error.
        let choice = collection.choices.get(&id1).expect("a choice was recorded");
        assert_eq!(
            choice.spell,
            Some(SpellChoice {
                spell: CardId(20),
                target: None
            })
        );
        // No ingredient was committed → the action normalises to a pass, but the
        // spell still rides with it (pass + spell is legal; the spell never keeps
        // the passed player in).
        assert_eq!(choice.action, WaveAction::Pass);

        // The accepted first cast spends the spell slot (the refreshed frame
        // offers no further casts); the second cast gets the SpellLimit error.
        let msgs = drain(&mut rx1);
        assert!(
            matches!(
                msgs.as_slice(),
                [
                    ServerMessage::DecisionFrame { .. },
                    ServerMessage::Error {
                        code: ErrorCode::SpellLimit,
                        ..
                    }
                ]
            ),
            "expected a refreshed frame then one SpellLimit error, got {msgs:?}"
        );
    }

    #[tokio::test]
    async fn illegal_spell_targets_are_rejected() {
        let (s1, mut rx1) = seat(1, Color::Ruby);
        let (s2, _rx2) = seat(2, Color::Sapphire);
        let id1 = s1.id;
        let mut players = vec![s1, s2];
        let mut hands: HashMap<PlayerId, Hand> = HashMap::new();
        let mut h = Hand::new();
        h.add_spells([
            crate::game::card::Spell {
                id: CardId(30),
                kind: SpellKind::Hex,
            },
            crate::game::card::Spell {
                id: CardId(31),
                kind: SpellKind::Sour,
            },
        ]);
        hands.insert(id1, h);
        let mut gone = HashSet::new();
        let palette: HashSet<u16> = HashSet::new();

        let (cmd_tx, mut cmd_rx) = mpsc::channel(8);
        // Hex aimed at self: illegal.
        cmd_tx
            .send(GroupCommand::Action {
                player: id1,
                msg: ClientMessage::CastSpell {
                    spell: CardId(30),
                    target: Some(SpellTarget::Player { player: id1 }),
                },
            })
            .await
            .unwrap();
        // Sour aimed at Wild: illegal.
        cmd_tx
            .send(GroupCommand::Action {
                player: id1,
                msg: ClientMessage::CastSpell {
                    spell: CardId(31),
                    target: Some(SpellTarget::Color { color: Color::Wild }),
                },
            })
            .await
            .unwrap();
        drop(cmd_tx);

        let collection = collect_wave(
            &mut cmd_rx,
            &mut players,
            &[id1],
            &hands,
            &mut gone,
            &palette,
            1,
            1,
            5_000,
            std::time::Instant::now(),
            &mut Vec::new(),
            &tracing::Span::none(),
        )
        .await;

        assert!(
            collection.choices.get(&id1).and_then(|c| c.spell).is_none(),
            "no illegal cast may be recorded"
        );
        let msgs = drain(&mut rx1);
        assert_eq!(msgs.len(), 2);
        assert!(msgs.iter().all(|m| matches!(
            m,
            ServerMessage::Error {
                code: ErrorCode::InvalidTarget,
                ..
            }
        )));
    }

    #[tokio::test]
    async fn action_from_locked_out_player_replies_locked_out_and_changes_no_state() {
        let (s1, _rx1) = seat(1, Color::Ruby);
        let (s2, mut rx2) = seat(2, Color::Sapphire);
        let id1 = s1.id;
        let id2 = s2.id;
        let mut players = vec![s1, s2];
        let mut hands: HashMap<PlayerId, Hand> = HashMap::new();
        hands.insert(id1, Hand::new());
        let mut h2 = Hand::new();
        h2.add_ingredients([ing(20, Color::Sapphire, 1, 1)]);
        hands.insert(id2, h2);
        let mut gone = HashSet::new();
        let palette: HashSet<u16> = HashSet::new();

        // id2 is NOT in the acting set (already passed / locked out this round).
        let (cmd_tx, mut cmd_rx) = mpsc::channel(8);
        cmd_tx
            .send(GroupCommand::Action {
                player: id2,
                msg: ClientMessage::CommitPass,
            })
            .await
            .unwrap();
        drop(cmd_tx);

        let collection = collect_wave(
            &mut cmd_rx,
            &mut players,
            &[id1],
            &hands,
            &mut gone,
            &palette,
            1,
            1,
            5_000,
            std::time::Instant::now(),
            &mut Vec::new(),
            &tracing::Span::none(),
        )
        .await;

        // id2 takes no part in this wave's bookkeeping: its locked-out action records
        // no choice, and its hand is untouched.
        assert!(!collection.choices.contains_key(&id2));
        assert!(hands[&id2].contains_ingredient(CardId(20)));

        let msgs = drain(&mut rx2);
        assert!(
            matches!(
                msgs.as_slice(),
                [ServerMessage::Error {
                    code: ErrorCode::LockedOut,
                    ..
                }]
            ),
            "expected one LockedOut error, got {msgs:?}"
        );
    }

    #[tokio::test]
    async fn off_palette_emote_replies_invalid_emote_in_wave() {
        let (s1, mut rx1) = seat(1, Color::Ruby);
        let id1 = s1.id;
        let mut players = vec![s1];
        let mut hands: HashMap<PlayerId, Hand> = HashMap::new();
        hands.insert(id1, Hand::new());
        let mut gone = HashSet::new();
        let palette: HashSet<u16> = HashSet::from([1u16]);

        let (cmd_tx, mut cmd_rx) = mpsc::channel(8);
        // A palette emote is broadcast; an off-palette one is rejected — matching
        // the lobby, resolving the lobby-vs-wave inconsistency.
        cmd_tx
            .send(GroupCommand::Action {
                player: id1,
                msg: ClientMessage::Emote { emote: EmoteId(1) },
            })
            .await
            .unwrap();
        cmd_tx
            .send(GroupCommand::Action {
                player: id1,
                msg: ClientMessage::Emote {
                    emote: EmoteId(999),
                },
            })
            .await
            .unwrap();
        drop(cmd_tx);

        let mut input_log: Vec<TimedInput> = Vec::new();
        let _ = collect_wave(
            &mut cmd_rx,
            &mut players,
            &[id1],
            &hands,
            &mut gone,
            &palette,
            1,
            1,
            5_000,
            std::time::Instant::now(),
            &mut input_log,
            &tracing::Span::none(),
        )
        .await;

        let msgs = drain(&mut rx1);
        assert!(
            matches!(
                msgs.as_slice(),
                [
                    ServerMessage::EmoteBroadcast {
                        emote: EmoteId(1),
                        ..
                    },
                    ServerMessage::Error {
                        code: ErrorCode::InvalidEmote,
                        ..
                    }
                ]
            ),
            "expected a valid broadcast then an InvalidEmote error, got {msgs:?}"
        );

        // The raw-input log captured *everything sent* — both the valid emote and
        // the off-palette one that was rejected — in arrival order, with
        // non-decreasing timestamps.
        assert_eq!(
            input_log
                .iter()
                .map(|t| t.input.clone())
                .collect::<Vec<_>>(),
            vec![
                RecordedInput::Emote { emote: EmoteId(1) },
                RecordedInput::Emote {
                    emote: EmoteId(999)
                },
            ],
            "the recorder must capture both emotes (incl. the rejected one)"
        );
        assert!(input_log.iter().all(|t| t.player == id1));
        assert!(input_log.windows(2).all(|w| w[0].at_ms <= w[1].at_ms));
    }

    // ---- decision frames (`boom-decision-frame`): exactness + staleness ----

    /// Frame exactness both ways at the validation seam: every action the frame
    /// enumerates passes the checks `collect_wave` applies (hand membership +
    /// target shape), and every action that would pass those checks is
    /// enumerated. Non-enumerated probes fail both.
    #[test]
    fn wave_commit_frame_is_exact_against_validation() {
        let me = PlayerId(Uuid::from_u128(1));
        let seated: Vec<PlayerId> = (1..=4u128).map(|n| PlayerId(Uuid::from_u128(n))).collect();
        let mut hand = Hand::new();
        hand.add_ingredients([ing(10, Color::Ruby, 2, 1), ing(11, Color::Wild, 5, 0)]);
        hand.add_spells([
            crate::game::card::Spell {
                id: CardId(20),
                kind: SpellKind::Peek,
            },
            crate::game::card::Spell {
                id: CardId(21),
                kind: SpellKind::Hex,
            },
            crate::game::card::Spell {
                id: CardId(22),
                kind: SpellKind::Sour,
            },
        ]);

        let frame = wave_commit_frame(me, &hand, &seated, true);

        // Forward: everything enumerated validates.
        let PendingDecision::WaveCommit {
            playable,
            can_pass,
            spells,
        } = &frame;
        assert!(*can_pass, "an active player may always pass");
        for p in playable {
            assert!(hand.contains_ingredient(p.ingredient.id));
        }
        for s in spells {
            assert!(hand.contains_spell(s.spell));
            match &s.targets {
                TargetOptions::None => {
                    assert!(target_shape_ok(s.kind, None, me, &seated));
                }
                TargetOptions::Players { players } => {
                    for t in players {
                        assert!(target_shape_ok(
                            s.kind,
                            Some(SpellTarget::Player { player: *t }),
                            me,
                            &seated
                        ));
                    }
                }
                TargetOptions::Colors { colors } => {
                    for c in colors {
                        assert!(target_shape_ok(
                            s.kind,
                            Some(SpellTarget::Color { color: *c }),
                            me,
                            &seated
                        ));
                    }
                }
            }
        }

        // Reverse: everything that validates is enumerated.
        for c in hand.ingredients() {
            assert!(frame.permits_play(c.id, false) && frame.permits_play(c.id, true));
        }
        for s in hand.spells() {
            match s.kind.target_kind() {
                TargetKind::None => assert!(frame.permits_cast(s.id, None)),
                TargetKind::Player => {
                    for t in seated.iter().filter(|t| **t != me) {
                        assert!(frame.permits_cast(s.id, Some(SpellTarget::Player { player: *t })));
                    }
                }
                TargetKind::Color => {
                    for c in Color::PLAYER_COLORS {
                        assert!(frame.permits_cast(s.id, Some(SpellTarget::Color { color: c })));
                    }
                }
            }
        }

        // Probes: non-enumerated actions are absent from the frame AND fail the
        // validation, in agreement.
        assert!(!frame.permits_play(CardId(99), false));
        assert!(!hand.contains_ingredient(CardId(99)));
        let self_hex = Some(SpellTarget::Player { player: me });
        assert!(!frame.permits_cast(CardId(21), self_hex));
        assert!(!target_shape_ok(SpellKind::Hex, self_hex, me, &seated));
        let wild_sour = Some(SpellTarget::Color { color: Color::Wild });
        assert!(!frame.permits_cast(CardId(22), wild_sour));
        assert!(!target_shape_ok(SpellKind::Sour, wild_sour, me, &seated));
        assert!(
            !frame.permits_cast(CardId(21), None),
            "Hex requires a target"
        );

        // A spent spell slot empties the cast set; plays and the pass remain.
        let refreshed = wave_commit_frame(me, &hand, &seated, false);
        assert!(!refreshed.permits_cast(CardId(20), None));
        assert!(refreshed.permits_play(CardId(10), false));
        assert!(refreshed.permits_pass());
    }

    /// An accepted cast spends the wave's one spell slot: the caster receives a
    /// refreshed frame whose legal set offers no further casts (spec: "Illegal
    /// options are absent").
    #[tokio::test]
    async fn accepted_cast_refreshes_the_frame_without_spells() {
        let (s1, mut rx1) = seat(1, Color::Ruby);
        let id1 = s1.id;
        let mut players = vec![s1];
        let mut hands: HashMap<PlayerId, Hand> = HashMap::new();
        let mut h = Hand::new();
        h.add_ingredients([ing(10, Color::Ruby, 2, 1)]);
        h.add_spells([crate::game::card::Spell {
            id: CardId(20),
            kind: SpellKind::Peek,
        }]);
        hands.insert(id1, h);
        let mut gone = HashSet::new();
        let palette: HashSet<u16> = HashSet::new();

        let (cmd_tx, mut cmd_rx) = mpsc::channel(8);
        cmd_tx
            .send(GroupCommand::Action {
                player: id1,
                msg: ClientMessage::CastSpell {
                    spell: CardId(20),
                    target: None,
                },
            })
            .await
            .unwrap();
        drop(cmd_tx);

        let collection = collect_wave(
            &mut cmd_rx,
            &mut players,
            &[id1],
            &hands,
            &mut gone,
            &palette,
            1,
            1,
            5_000,
            std::time::Instant::now(),
            &mut Vec::new(),
        )
        .await;
        assert_eq!(
            collection.choices.get(&id1).and_then(|c| c.spell),
            Some(SpellChoice {
                spell: CardId(20),
                target: None
            })
        );

        let msgs = drain(&mut rx1);
        match msgs.as_slice() {
            [
                ServerMessage::DecisionFrame {
                    round_number: 1,
                    wave_number: 1,
                    timer_ms: Some(_),
                    decision,
                },
            ] => {
                assert!(
                    !decision.permits_cast(CardId(20), None),
                    "the refreshed frame must offer no further casts"
                );
                assert!(decision.permits_play(CardId(10), false));
                assert!(decision.permits_pass());
            }
            other => panic!("expected one refreshed DecisionFrame, got {other:?}"),
        }
    }

    /// Submissions queued after a frame's decision resolved are rejected with
    /// `StaleFrame` and change no state; liveness traffic is still serviced and
    /// departures are folded in.
    #[tokio::test]
    async fn stale_actions_are_rejected_with_stale_frame_and_no_state_change() {
        let (s1, mut rx1) = seat(1, Color::Ruby);
        let (s2, _rx2) = seat(2, Color::Sapphire);
        let id1 = s1.id;
        let id2 = s2.id;
        let mut players = vec![s1, s2];
        let mut gone = HashSet::new();
        let palette: HashSet<u16> = HashSet::new();

        let (cmd_tx, mut cmd_rx) = mpsc::channel(16);
        for msg in [
            ClientMessage::CommitIngredient {
                card: CardId(10),
                colorless: false,
            },
            ClientMessage::CastSpell {
                spell: CardId(20),
                target: None,
            },
            ClientMessage::CommitPass,
            ClientMessage::LockIn,
            ClientMessage::Heartbeat,
        ] {
            cmd_tx
                .send(GroupCommand::Action { player: id1, msg })
                .await
                .unwrap();
        }
        cmd_tx
            .send(GroupCommand::Leave { player: id2 })
            .await
            .unwrap();

        let mut input_log: Vec<TimedInput> = Vec::new();
        let reconnected = reject_stale(
            &mut cmd_rx,
            &mut players,
            &mut gone,
            &palette,
            std::time::Instant::now(),
            &mut input_log,
        )
        .await;

        assert!(reconnected.is_empty());
        assert!(
            gone.contains(&id2),
            "a drain-time leave is folded into gone"
        );
        // Four stale rejections, then the serviced heartbeat — in order.
        let msgs = drain(&mut rx1);
        assert_eq!(msgs.len(), 5, "got {msgs:?}");
        for m in &msgs[..4] {
            assert!(
                matches!(
                    m,
                    ServerMessage::Error {
                        code: ErrorCode::StaleFrame,
                        ..
                    }
                ),
                "expected StaleFrame, got {m:?}"
            );
        }
        assert!(matches!(msgs[4], ServerMessage::Heartbeat));
        // The raw-input log still captured the rejected attempts (not the heartbeat).
        assert_eq!(input_log.len(), 4);
    }

    // ---- the shipping (async) path is the tested engine path ----
    //
    // `run_game` drives the same orchestration core as `Game::play_out`, so a fixed
    // seed produces identical final scores (the parity test). A no-panic stress test
    // over many seeds keeps the live loop honest.

    /// A scripted in-process client: plays its first hand ingredient each wave
    /// (as a Vote), locking in so waves close as soon as every acting player has.
    /// Returns the final scores it observed at `GameOver`.
    async fn client_loop(
        id: PlayerId,
        mut rx: mpsc::Receiver<ServerMessage>,
        cmd_tx: mpsc::Sender<GroupCommand>,
    ) -> Option<Vec<PlayerScore>> {
        let mut hand: Vec<CardId> = Vec::new();
        let mut passed = false;
        while let Some(msg) = rx.recv().await {
            match msg {
                ServerMessage::YourHand { ingredients, .. } => {
                    hand = ingredients.iter().map(|c| c.id).collect();
                }
                ServerMessage::WaveOpened { wave_number, .. } => {
                    if wave_number == 1 {
                        passed = false;
                    }
                    if passed {
                        continue;
                    }
                    let action = match hand.first() {
                        Some(&c) => {
                            hand.remove(0);
                            ClientMessage::CommitIngredient {
                                card: c,
                                colorless: false,
                            }
                        }
                        None => {
                            passed = true;
                            ClientMessage::CommitPass
                        }
                    };
                    let _ = cmd_tx
                        .send(GroupCommand::Action {
                            player: id,
                            msg: action,
                        })
                        .await;
                    let _ = cmd_tx
                        .send(GroupCommand::Action {
                            player: id,
                            msg: ClientMessage::LockIn,
                        })
                        .await;
                }
                ServerMessage::WaveResolved {
                    passed: passers, ..
                } if passers.contains(&id) => passed = true,
                ServerMessage::GameOver { final_scores, .. } => return Some(final_scores),
                _ => {}
            }
        }
        None
    }

    /// Drive a full four-player game through the real async `run_game`, returning
    /// the agreed-upon final scores.
    async fn play_async_game(seed: u64, wave_ms: u32) -> Vec<PlayerScore> {
        let mut cfg = ContentConfig::from_toml(include_str!("../content.toml")).unwrap();
        cfg.timing.wave1_ms = wave_ms;
        cfg.timing.wave_ms = wave_ms;
        let registry = cfg.build_registry().unwrap();

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<GroupCommand>(512);
        let mut seats = Vec::new();
        let mut clients = Vec::new();
        for (i, color) in Color::PLAYER_COLORS.into_iter().enumerate() {
            let (out_tx, out_rx) = mpsc::channel::<ServerMessage>(512);
            let id = PlayerId(Uuid::from_u128(i as u128 + 1));
            seats.push(SeatInfo {
                id,
                name: format!("p{i}"),
                color,
                guest: false,
                out: out_tx,
            });
            clients.push(client_loop(id, out_rx, cmd_tx.clone()));
        }
        drop(cmd_tx); // once every client returns at GameOver, run_game's rx closes
        let palette: HashSet<u16> = HashSet::new();

        let mut it = clients.into_iter();
        let (c0, c1, c2, c3) = (
            it.next().unwrap(),
            it.next().unwrap(),
            it.next().unwrap(),
            it.next().unwrap(),
        );
        let game = run_game(
            &registry,
            &cfg,
            GroupCode("BREW-TEST".into()),
            seats,
            &mut cmd_rx,
            &palette,
            seed,
            None,
        );
        let (_g, r0, r1, r2, r3) = tokio::join!(game, c0, c1, c2, c3);

        let results = [r0, r1, r2, r3];
        let scores: Vec<&Vec<PlayerScore>> = results.iter().flatten().collect();
        assert_eq!(scores.len(), 4, "every client observed GameOver");
        for s in &scores[1..] {
            assert_eq!(*s, scores[0], "clients disagree on the final scores");
        }
        scores[0].clone()
    }

    /// The sync-engine analogue of the scripted `client_loop`: play the first
    /// ingredient of the live hand as a Vote, else pass.
    fn play_first_else_pass() -> impl FnMut(PlayerId, &Hand) -> WaveChoice {
        |_player, hand| match hand.ingredients().first() {
            Some(first) => WaveChoice {
                action: WaveAction::Play {
                    card: first.id,
                    colorless: false,
                },
                spell: None,
            },
            None => WaveChoice::pass(),
        }
    }

    /// The four sync-runner players, mirroring `play_async_game`'s seat ids/colours
    /// so `color_owner` and scoring line up across the two paths.
    fn sync_players() -> Vec<crate::game::state::Player> {
        Color::PLAYER_COLORS
            .into_iter()
            .enumerate()
            .map(|(i, color)| crate::game::state::Player {
                id: PlayerId(Uuid::from_u128(i as u128 + 1)),
                color,
                display_name: format!("p{i}"),
            })
            .collect()
    }

    #[tokio::test]
    async fn async_path_matches_sync_runner_for_fixed_seeds() {
        use crate::game::runner::Game;

        // The single orchestration core means the shipping (async) loop and the tested
        // sync engine must agree on the final scores for any fixed seed. This is
        // the safety net for the convergence: a real divergence fails loudly here.
        let cfg = ContentConfig::from_toml(include_str!("../content.toml")).unwrap();
        let registry = cfg.build_registry().unwrap();

        for seed in [0xC0FFEE_u64, 0x1234, 7, 42, 0xBEEF] {
            let async_scores: HashMap<PlayerId, i32> =
                tokio::time::timeout(Duration::from_secs(30), play_async_game(seed, 2_000))
                    .await
                    .unwrap_or_else(|_| panic!("async game for seed {seed:#x} completed"))
                    .into_iter()
                    .map(|s| (s.player, s.score))
                    .collect();

            let mut game = Game::new(&registry, &cfg, sync_players(), seed);
            let mut decider = play_first_else_pass();
            let sync_scores = game.play_out(&mut decider).scores;

            assert_eq!(
                async_scores, sync_scores,
                "the async loop diverged from the tested sync engine for seed {seed:#x}"
            );
        }
    }

    #[tokio::test]
    async fn async_path_completes_across_many_seeds_without_panicking() {
        // Parity with the sync runner's stress test: the async path reaches GameOver
        // for many seeds without tripping an invariant.
        for seed in 0..12u64 {
            let scores = tokio::time::timeout(Duration::from_secs(20), play_async_game(seed, 120))
                .await
                .unwrap_or_else(|_| panic!("seed {seed} did not complete"));
            assert_eq!(scores.len(), 4, "seed {seed} produced four final scores");
        }
    }
}
