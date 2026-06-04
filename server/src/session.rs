//! The in-group networked game loop: drives the (synchronous, tested) engine over
//! the wire for one full game.
//!
//! For each round it deals refill-to-5 hands (private `YourHand`), reveals a
//! modifier from round 2, then runs waves: it broadcasts `WaveOpened` with the
//! timer budget, collects hidden commits until the timer expires or every active
//! player has locked in, resolves the wave through the engine, and broadcasts the
//! public outcome (never card identities). Effects stay silent except the
//! Peek/Expose/Recall tells. Each round ends with a depile and scoring; a tie
//! after the final round is settled by a Deathmatch.
//!
//! Resilience: a disconnected player auto-passes while absent (the seat is held
//! for the game); a reconnecting player reattaches their channel and receives a
//! private [`ServerMessage::StateSnapshot`] scoped to what they may know.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::observability::span_schema::SPAN_SCHEMA_VERSION;

use boiling_point_protocol::server::{
    Audience, Contribution, DepileEntry, ErrorCode, Outbound, PlayerPublic, PlayerScore,
    ScoringOutcome,
};
use boiling_point_protocol::vocab::{Color, ModifierKind};
use boiling_point_protocol::{ClientMessage, GroupCode, PlayerId, ServerMessage};

use crate::config::{ContentConfig, ROUND_COUNT};
use crate::content::ContentRegistry;
use crate::game::card::Card;
use crate::game::deck::Deck;
use crate::game::modifiers::ActiveModifiers;
use crate::game::round::{Round, RoundEnd, WaveChoice, WaveInput};
use crate::game::scoring::{ScoringContext, explosion, score_safe};
use crate::game::state::Hand;
use crate::game::{DeathmatchResult, run_deathmatch};
use crate::lobby::group::GroupCommand;

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
    committed: Vec<(PlayerId, Card)>,
    passers: Vec<PlayerId>,
    emptied: Vec<PlayerId>,
    reconnected: Vec<PlayerId>,
    /// Whether the commit window closed on its timer rather than every active
    /// player locking in (feeds the `wave.timed_out` span attribute / timeout rate).
    timed_out: bool,
}

/// A compact, in-process-only rendering of a hand for the `hand` span's secret
/// attribute — read by the privileged reveal, never exported.
fn fmt_hand(hand: &Hand) -> String {
    hand.views()
        .iter()
        .map(|c| {
            let eff = c.view.effect.map(|e| format!(":{e:?}")).unwrap_or_default();
            format!(
                "{:?}(v{},p{}){}",
                c.view.color, c.view.volatility, c.view.points, eff
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
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

/// Run one full game to completion for the given seats. Owns the group's command
/// receiver for the duration so it can collect commits within wave timers.
pub async fn run_game(
    registry: &ContentRegistry,
    config: &ContentConfig,
    group_code: GroupCode,
    mut players: Vec<SeatInfo>,
    rx: &mut mpsc::Receiver<GroupCommand>,
    palette: &HashSet<u16>,
    seed: u64,
) -> GameEnd {
    let ids: Vec<PlayerId> = players.iter().map(|p| p.id).collect();
    let color_owner: HashMap<Color, PlayerId> = players.iter().map(|p| (p.color, p.id)).collect();
    let mut hands: HashMap<PlayerId, Hand> = ids.iter().map(|id| (*id, Hand::new())).collect();
    let mut scores: HashMap<PlayerId, i32> = ids.iter().map(|id| (*id, 0)).collect();
    let mut deck = Deck::build(registry, seed);
    let mut modifiers = ActiveModifiers::new();
    let mut rng = StdRng::seed_from_u64(seed ^ 0xBEEF_F00D);
    let mut modifier_pile: Vec<ModifierKind> = registry
        .modifier_pool()
        .into_iter()
        .flat_map(|(kind, copies)| std::iter::repeat_n(kind, copies as usize))
        .collect();
    modifier_pile.shuffle(&mut rng);
    let mut gone: HashSet<PlayerId> = HashSet::new();

    crate::observability::metric::game_started();
    let game_start = std::time::Instant::now();
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
        if round_number >= 2
            && let Some(kind) = modifier_pile.pop()
        {
            modifiers.push(kind);
            broadcast(
                &players,
                ServerMessage::ModifierRevealed {
                    modifier: kind,
                    round_number,
                },
            )
            .await;
        }

        // Refill hands and send each player their private hand.
        for id in &ids {
            let len = hands[id].len();
            let (drawn, reshuffled) = deck.refill(len);
            hands
                .get_mut(id)
                .expect("invariant: every seated player has a hand")
                .add(drawn);
            if reshuffled {
                crate::observability::metric::deck_reshuffled();
                broadcast(&players, ServerMessage::DeckReshuffled).await;
            }
        }
        for id in &ids {
            send_to(
                &players,
                *id,
                ServerMessage::YourHand {
                    cards: hands[id].views(),
                },
            )
            .await;
        }

        let base = rng.gen_range(config.boiling_point.min..=config.boiling_point.max);
        let effective_bp = modifiers.effective_boiling_point(base, registry);
        let start_vol = modifiers.start_volatility(registry);
        let active: Vec<PlayerId> = ids
            .iter()
            .copied()
            .filter(|id| !hands[id].is_empty() && !gone.contains(id))
            .collect();
        let mut round = Round::start(active, effective_bp, start_vol);
        let mut wave_no: u8 = 1;
        let round_start = std::time::Instant::now();

        // `round` span — child of the game span; held open for the whole round.
        // boiling_point/volatility_total are secret (in-process only); round.number,
        // round.exploded, and modifiers are public live-registry keys/outcome. The
        // active modifiers ride as a public attribute (clients already see them).
        let mods_str = modifiers
            .kinds()
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
                round.exploded = tracing::field::Empty,
                modifiers = %mods_str,
            )
        });

        // `hand` spans — one per seated player, child of the round, held open for
        // the whole round so the privileged reveal can read each hand from a live
        // span. The hand contents ride as a secret attribute (in-process only).
        let _hand_spans: Vec<tracing::Span> = ids
            .iter()
            .map(|id| {
                let hand = fmt_hand(&hands[id]);
                round_span.in_scope(|| tracing::info_span!("hand", player.id = %id.0, hand = %hand))
            })
            .collect();

        while round.is_open() {
            let acting: Vec<PlayerId> = round.active().to_vec();
            let timer_ms = if wave_no == 1 {
                config.timing.wave1_ms
            } else {
                config.timing.wave_ms
            };
            // `wave` span — child of the round; held open for the whole commit
            // window so the live registry shows the in-flight wave.
            let wave_span = round_span.in_scope(|| {
                tracing::info_span!(
                    "wave",
                    wave.number = wave_no as u64,
                    wave.timer_ms = timer_ms,
                    wave.timed_out = tracing::field::Empty,
                )
            });
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

            let collection = collect_wave(
                rx,
                &mut players,
                &acting,
                &mut hands,
                &mut gone,
                palette,
                timer_ms,
            )
            .await;
            let wave_timed_out = collection.timed_out;
            // Reconnected players resume for future rounds and get a private snapshot.
            for player in &collection.reconnected {
                // `reconnect` span — child of the game span; player.id is public.
                let _reconnect =
                    game_span.in_scope(|| tracing::info_span!("reconnect", player.id = %player.0));
                crate::observability::metric::player_reconnected();
                gone.remove(player);
                let snapshot = ServerMessage::StateSnapshot {
                    group_code: group_code.clone(),
                    your_player_id: *player,
                    round_number,
                    players: public_players(&players, &gone),
                    scores: scores_vec(&scores, &ids),
                    active_modifiers: modifiers.kinds().to_vec(),
                    contributions: contributions(&round, &ids),
                    your_hand: hands.get(player).map(|h| h.views()).unwrap_or_default(),
                };
                send_to(&players, *player, snapshot).await;
                tracing::info!(player = %player.0, "player reconnected");
            }
            let WaveCollection {
                committed,
                passers,
                emptied,
                ..
            } = collection;
            let played: Vec<PlayerId> = committed.iter().map(|(p, _)| *p).collect();

            // `commit` leaf spans (one per committed card) — children of the wave.
            // The committed card identity rides as a secret attribute (in-process
            // only); it is never broadcast until public resolution.
            wave_span.in_scope(|| {
                for (player, card) in &committed {
                    let _commit =
                        tracing::info_span!("commit", player.id = %player.0, committed_card = ?card);
                }
            });

            // `resolve` span — child of the wave; pot.card_count is public.
            let resolve_span = wave_span.in_scope(|| {
                tracing::info_span!("resolve", pot.card_count = tracing::field::Empty)
            });
            let report = round.apply_wave(
                registry,
                WaveInput {
                    committed,
                    passers: passers.clone(),
                    emptied,
                    recalls: HashMap::new(),
                },
            );
            resolve_span.record("pot.card_count", round.pot().card_count() as u64);
            drop(resolve_span);
            // Surface the wave outcome and the live running volatility on the open
            // spans (an Update lifecycle event), so the reveal shows current state.
            wave_span.record("wave.timed_out", wave_timed_out);
            round_span.record("volatility_total", round.pot().volatility as i64);
            crate::observability::metric::wave_resolved(wave_timed_out);
            crate::observability::metric::cards_committed(played.len() as u64);
            for (player, card) in report.outcome.recalled {
                if let Some(hand) = hands.get_mut(&player) {
                    hand.add([card]);
                }
            }
            if !report.outcome.peeked.is_empty() {
                broadcast(&players, ServerMessage::SomeonePeeked).await;
                for peeker in &report.outcome.peeked {
                    send_to(
                        &players,
                        *peeker,
                        ServerMessage::PeekResult {
                            boiling_point: effective_bp.max(0) as u8,
                        },
                    )
                    .await;
                }
            }
            for card in report.outcome.exposed {
                broadcast(&players, ServerMessage::Exposed { card }).await;
            }

            let contributions = contributions(&round, &ids);
            broadcast(
                &players,
                ServerMessage::WaveResolved {
                    played,
                    passed: passers,
                    cauldron_card_count: round.pot().card_count() as u8,
                    contributions,
                },
            )
            .await;
            wave_no += 1;
        }

        // Depile (boiling point revealed only on explosion).
        let exploded = round.ended() == Some(RoundEnd::Exploded);
        let depile = round.depile();
        // Round outcome onto the round span: volatility_total is secret (the final
        // running volatility); round.exploded is public.
        round_span.record(
            "volatility_total",
            depile
                .reveals
                .last()
                .map(|i| i.running_volatility)
                .unwrap_or(0) as i64,
        );
        round_span.record("round.exploded", exploded);
        broadcast(
            &players,
            ServerMessage::Depile {
                reveals: depile
                    .reveals
                    .iter()
                    .map(|item| DepileEntry {
                        player: item.player,
                        card: item.card.view(),
                        running_volatility: item.running_volatility.max(0) as u8,
                    })
                    .collect(),
                exploded,
                boiling_point: exploded.then_some(depile.boiling_point),
                crossing_index: if exploded {
                    depile.crossing_index
                } else {
                    None
                },
            },
        )
        .await;

        // Score the round and broadcast the result.
        let shielded = round.shielded().clone();
        let ctx = ScoringContext {
            modifiers: &modifiers,
            registry,
            color_owner: &color_owner,
            shielded: &shielded,
            all_players: &ids,
        };
        // `score` span — child of the round; round.exploded and pot.value are public.
        let score_span = round_span.in_scope(|| {
            tracing::info_span!(
                "score",
                round.exploded = exploded,
                pot.value = tracing::field::Empty,
                dominant_color = tracing::field::Empty,
            )
        });
        if exploded {
            let result = explosion(round.pot(), &ctx);
            score_span.record("pot.value", result.pot_value as i64);
            // An explosion has no scoring colour winner.
            score_span.record("dominant_color", "none");
            for (player, delta) in &result.deltas {
                *scores.entry(*player).or_insert(0) += delta;
            }
            broadcast(
                &players,
                ServerMessage::Explosion {
                    pot_value: result.pot_value,
                    deltas: result
                        .deltas
                        .iter()
                        .map(|(p, d)| PlayerScore {
                            player: *p,
                            score: *d,
                        })
                        .collect(),
                    shielded: result.shielded,
                },
            )
            .await;
        } else {
            let result = score_safe(round.pot(), &ctx);
            for (player, delta) in &result.awards {
                *scores.entry(*player).or_insert(0) += delta;
            }
            let outcome = if result.winners.len() == 1 {
                ScoringOutcome::Domination {
                    winner: result.winners[0],
                }
            } else {
                ScoringOutcome::Split {
                    colors: result.winners.clone(),
                }
            };
            // Public dominant-strategy signal for the balance dashboard: the single
            // dominating colour, or `split` when several colours tied.
            score_span.record(
                "dominant_color",
                match result.winners.as_slice() {
                    [only] => format!("{only:?}"),
                    _ => "split".to_string(),
                }
                .as_str(),
            );
            crate::observability::metric::round_decided(result.winners.len() == 1);
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
                },
            )
            .await;
        }
        broadcast(
            &players,
            ServerMessage::ScoreUpdate {
                scores: scores_vec(&scores, &ids),
            },
        )
        .await;
        drop(score_span);

        crate::observability::metric::round_resolved(exploded);
        crate::observability::metric::round_duration(round_start.elapsed().as_secs_f64());

        // Spent pot cards return to the discard for future reshuffles.
        let spent: Vec<_> = round.pot().cards.iter().map(|pc| pc.card).collect();
        deck.discard_cards(spent);
    }

    // Game over — break a tie for the lead with a Deathmatch.
    let best = scores.values().copied().max().unwrap_or(0);
    let leaders: Vec<PlayerId> = ids
        .iter()
        .copied()
        .filter(|id| scores[id] == best)
        .collect();
    let winners = if leaders.len() > 1 {
        broadcast(
            &players,
            ServerMessage::DeathmatchStarted {
                participants: leaders.clone(),
            },
        )
        .await;
        let tied: Vec<(PlayerId, Hand)> =
            leaders.iter().map(|id| (*id, hands[id].clone())).collect();
        let mut shed_lowest = |_p: PlayerId, hand: &Hand| {
            hand.views()
                .iter()
                .min_by_key(|c| c.view.volatility)
                .expect("invariant: deathmatch sheds only from a non-empty hand")
                .id
        };
        match run_deathmatch(
            registry,
            tied,
            config.boiling_point.min,
            config.boiling_point.max,
            &mut shed_lowest,
            seed ^ 0xD3A7,
        ) {
            DeathmatchResult::Champion(p) => vec![p],
            DeathmatchResult::CoChampions(ps) if !ps.is_empty() => ps,
            DeathmatchResult::CoChampions(_) => leaders.clone(),
        }
    } else {
        leaders
    };

    crate::observability::metric::game_completed();
    crate::observability::metric::game_duration(game_start.elapsed().as_secs_f64());
    tracing::info!(?winners, "game over");
    broadcast(
        &players,
        ServerMessage::GameOver {
            final_scores: scores_vec(&scores, &ids),
            winners: winners.clone(),
        },
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

/// Per-player contributed-card counts in the current pot (the public signal).
fn contributions(round: &Round, ids: &[PlayerId]) -> Vec<Contribution> {
    ids.iter()
        .map(|id| Contribution {
            player: *id,
            count: round
                .pot()
                .cards
                .iter()
                .filter(|pc| pc.player == *id)
                .count() as u8,
        })
        .collect()
}

/// Collect one wave's hidden commits until the timer expires or every active
/// player has locked in. Heartbeats and emotes are serviced live; a disconnect
/// (`Leave`) auto-passes the player for the rest of the game.
async fn collect_wave(
    rx: &mut mpsc::Receiver<GroupCommand>,
    players: &mut [SeatInfo],
    acting: &[PlayerId],
    hands: &mut HashMap<PlayerId, Hand>,
    gone: &mut HashSet<PlayerId>,
    palette: &HashSet<u16>,
    timer_ms: u32,
) -> WaveCollection {
    let mut choice: HashMap<PlayerId, WaveChoice> = HashMap::new();
    let mut locked: HashSet<PlayerId> = HashSet::new();
    let mut reconnected: Vec<PlayerId> = Vec::new();
    // Disconnected players auto-pass and are considered locked in.
    for p in acting {
        if gone.contains(p) {
            choice.insert(*p, WaveChoice::Pass);
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
                        match msg {
                            ClientMessage::CommitCard { card } if active => {
                                // §I: a card the player doesn't hold is an invalid
                                // action, not a silent drop. The reply carries only the
                                // reason — never pot/volatility/boiling-point state — so
                                // it cannot weaken blind volatility.
                                if hands.get(&player).is_some_and(|h| h.contains(card)) {
                                    choice.insert(player, WaveChoice::Play(card));
                                } else {
                                    send_to(
                                        players,
                                        player,
                                        ServerMessage::Error {
                                            code: ErrorCode::NotYourCard,
                                            message: "that card is not in your hand".into(),
                                        },
                                    )
                                    .await;
                                }
                            }
                            ClientMessage::CommitPass if active => {
                                choice.insert(player, WaveChoice::Pass);
                            }
                            ClientMessage::LockIn if active => {
                                locked.insert(player);
                            }
                            // A commit/pass/lock-in from a player who has already passed,
                            // timed out, or is otherwise not acting this round: reply
                            // LockedOut rather than drop it (§I). No state changes.
                            ClientMessage::CommitCard { .. }
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
                            // An off-palette emote is rejected exactly as in the lobby,
                            // resolving the lobby-vs-wave inconsistency (F1/§I).
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
                            // Entry messages (create/join/enqueue) are never valid
                            // mid-game: reply WrongPhase, never silently drop.
                            ClientMessage::CreateRoom { .. }
                            | ClientMessage::JoinRoom { .. }
                            | ClientMessage::EnqueueMatch { .. } => {
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
                            choice.insert(player, WaveChoice::Pass);
                            locked.insert(player);
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

    let mut committed = Vec::new();
    let mut passers = Vec::new();
    let mut emptied = Vec::new();
    for player in acting {
        match choice.get(player) {
            Some(WaveChoice::Play(card_id)) => {
                if let Some(card) = hands.get_mut(player).and_then(|h| h.take(*card_id)) {
                    if hands[player].is_empty() {
                        emptied.push(*player);
                    }
                    committed.push((*player, card));
                } else {
                    passers.push(*player);
                }
            }
            _ => passers.push(*player),
        }
    }
    WaveCollection {
        committed,
        passers,
        emptied,
        reconnected,
        timed_out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ContentConfig;
    use crate::game::card::Card;
    use boiling_point_protocol::server::PlayerScore;
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
                out: tx,
            },
            rx,
        )
    }

    fn card(id: u32, color: Color, vol: u8, pts: u8) -> Card {
        Card {
            id: CardId(id),
            color,
            volatility: vol,
            points: pts,
            effect: None,
        }
    }

    fn drain(rx: &mut mpsc::Receiver<ServerMessage>) -> Vec<ServerMessage> {
        let mut out = Vec::new();
        while let Ok(m) = rx.try_recv() {
            out.push(m);
        }
        out
    }

    // ---- F1: invalid in-wave actions get an error, never a silent drop (§I) ----

    #[tokio::test]
    async fn bad_commit_card_replies_not_your_card_and_changes_no_state() {
        let (s1, mut rx1) = seat(1, Color::Ruby);
        let id1 = s1.id;
        let mut players = vec![s1];
        let mut hands: HashMap<PlayerId, Hand> = HashMap::new();
        let mut h = Hand::new();
        h.add([card(10, Color::Ruby, 2, 1)]);
        hands.insert(id1, h);
        let mut gone = HashSet::new();
        let palette: HashSet<u16> = HashSet::new();

        let (cmd_tx, mut cmd_rx) = mpsc::channel(8);
        cmd_tx
            .send(RoomCommand::Action {
                player: id1,
                msg: ClientMessage::CommitCard { card: CardId(99) },
            })
            .await
            .unwrap();
        drop(cmd_tx); // wave closes once the queued action is drained

        let collection = collect_wave(
            &mut cmd_rx,
            &mut players,
            &[id1],
            &mut hands,
            &mut gone,
            &palette,
            5_000,
        )
        .await;

        // No state change: nothing committed, the player falls through to a pass,
        // and the real card stays in hand.
        assert!(collection.committed.is_empty());
        assert_eq!(collection.passers, vec![id1]);
        assert!(hands[&id1].contains(CardId(10)));
        assert_eq!(hands[&id1].len(), 1);

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
    async fn action_from_locked_out_player_replies_locked_out_and_changes_no_state() {
        let (s1, _rx1) = seat(1, Color::Ruby);
        let (s2, mut rx2) = seat(2, Color::Sapphire);
        let id1 = s1.id;
        let id2 = s2.id;
        let mut players = vec![s1, s2];
        let mut hands: HashMap<PlayerId, Hand> = HashMap::new();
        hands.insert(id1, Hand::new());
        let mut h2 = Hand::new();
        h2.add([card(20, Color::Sapphire, 1, 1)]);
        hands.insert(id2, h2);
        let mut gone = HashSet::new();
        let palette: HashSet<u16> = HashSet::new();

        // id2 is NOT in the acting set (already passed / locked out this round).
        let (cmd_tx, mut cmd_rx) = mpsc::channel(8);
        cmd_tx
            .send(RoomCommand::Action {
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
            &mut hands,
            &mut gone,
            &palette,
            5_000,
        )
        .await;

        // id2 takes no part in this wave's bookkeeping; its hand is untouched.
        assert!(collection.committed.iter().all(|(p, _)| *p != id2));
        assert!(!collection.passers.contains(&id2));
        assert!(hands[&id2].contains(CardId(20)));

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
            .send(RoomCommand::Action {
                player: id1,
                msg: ClientMessage::Emote { emote: EmoteId(1) },
            })
            .await
            .unwrap();
        cmd_tx
            .send(RoomCommand::Action {
                player: id1,
                msg: ClientMessage::Emote {
                    emote: EmoteId(999),
                },
            })
            .await
            .unwrap();
        drop(cmd_tx);

        let _ = collect_wave(
            &mut cmd_rx,
            &mut players,
            &[id1],
            &mut hands,
            &mut gone,
            &palette,
            5_000,
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
    }

    // ---- F2: give the shipping (async) path determinism + stress coverage ----
    //
    // Full convergence onto the sync engine core (a network-backed `Decider`) is
    // deferred to a follow-up (see the proposal/design risk plan); these tests give
    // the async `run_game` the same determinism + no-panic stress guarantees the
    // sync runner is tested for (`runner.rs`).

    /// A scripted in-process client: plays its hand in order, one card per wave,
    /// locking in so waves close as soon as every acting player has. Returns the
    /// final scores it observed at `GameOver`.
    async fn client_loop(
        id: PlayerId,
        mut rx: mpsc::Receiver<ServerMessage>,
        cmd_tx: mpsc::Sender<RoomCommand>,
    ) -> Option<Vec<PlayerScore>> {
        let mut hand: Vec<CardId> = Vec::new();
        let mut idx = 0usize;
        while let Some(msg) = rx.recv().await {
            match msg {
                ServerMessage::YourHand { cards } => {
                    hand = cards.iter().map(|c| c.id).collect();
                    idx = 0;
                }
                ServerMessage::WaveOpened { .. } => {
                    let action = if idx < hand.len() {
                        let c = hand[idx];
                        idx += 1;
                        ClientMessage::CommitCard { card: c }
                    } else {
                        ClientMessage::CommitPass
                    };
                    let _ = cmd_tx
                        .send(RoomCommand::Action {
                            player: id,
                            msg: action,
                        })
                        .await;
                    let _ = cmd_tx
                        .send(RoomCommand::Action {
                            player: id,
                            msg: ClientMessage::LockIn,
                        })
                        .await;
                }
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

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<RoomCommand>(512);
        let mut seats = Vec::new();
        let mut clients = Vec::new();
        for (i, color) in Color::PLAYER_COLORS.into_iter().enumerate() {
            let (out_tx, out_rx) = mpsc::channel::<ServerMessage>(512);
            let id = PlayerId(Uuid::from_u128(i as u128 + 1));
            seats.push(SeatInfo {
                id,
                name: format!("p{i}"),
                color,
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
            RoomCode("BREW-TEST".into()),
            seats,
            &mut cmd_rx,
            &palette,
            seed,
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

    #[tokio::test]
    async fn async_path_is_deterministic_for_a_fixed_seed() {
        // The shipping loop must be reproducible under a fixed seed, matching the
        // determinism guarantee the sync runner is tested for.
        let a = tokio::time::timeout(Duration::from_secs(20), play_async_game(0xA11CE, 1_000))
            .await
            .expect("async game completed");
        let b = tokio::time::timeout(Duration::from_secs(20), play_async_game(0xA11CE, 1_000))
            .await
            .expect("async game completed");
        assert_eq!(a, b);
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
