//! The in-room networked game loop: drives the (synchronous, tested) engine over
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
    Contribution, DepileEntry, PlayerPublic, PlayerScore, ScoringOutcome,
};
use boiling_point_protocol::vocab::{Color, ModifierKind};
use boiling_point_protocol::{ClientMessage, PlayerId, RoomCode, ServerMessage};

use crate::config::{ContentConfig, ROUND_COUNT};
use crate::content::ContentRegistry;
use crate::game::card::Card;
use crate::game::deck::Deck;
use crate::game::modifiers::ActiveModifiers;
use crate::game::round::{Round, RoundEnd, WaveChoice, WaveInput};
use crate::game::scoring::{ScoringContext, explosion, score_safe};
use crate::game::state::Hand;
use crate::game::{DeathmatchResult, run_deathmatch};
use crate::lobby::room::RoomCommand;

/// A seated player as the game loop needs them: identity, colour, and the
/// outbound channel to reach them.
pub struct SeatInfo {
    /// Player id.
    pub id: PlayerId,
    /// Display name.
    pub name: String,
    /// Assigned colour.
    pub color: Color,
    /// Outbound channel to this player's connection.
    pub out: mpsc::Sender<ServerMessage>,
}

/// What one wave's collection yielded.
struct WaveCollection {
    committed: Vec<(PlayerId, Card)>,
    passers: Vec<PlayerId>,
    emptied: Vec<PlayerId>,
    reconnected: Vec<PlayerId>,
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
        })
        .collect()
}

async fn broadcast(players: &[SeatInfo], msg: ServerMessage) {
    for p in players {
        let _ = p.out.send(msg.clone()).await;
    }
}

async fn send_to(players: &[SeatInfo], id: PlayerId, msg: ServerMessage) {
    if let Some(p) = players.iter().find(|s| s.id == id) {
        let _ = p.out.send(msg).await;
    }
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

/// Run one full game to completion for the given seats. Owns the room's command
/// receiver for the duration so it can collect commits within wave timers.
pub async fn run_game(
    registry: &ContentRegistry,
    config: &ContentConfig,
    room_code: RoomCode,
    mut players: Vec<SeatInfo>,
    rx: &mut mpsc::Receiver<RoomCommand>,
    palette: &HashSet<u16>,
    seed: u64,
) {
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
    tracing::info!(players = players.len(), "game started");

    // `game` span (span_schema::span::GAME) — child of the caller's room.lifetime
    // span. Held open for the whole game; the deck seed rides as a secret attribute
    // (in-process only, redacted at export). Field names match `span_schema::attr`.
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
            hands.get_mut(id).unwrap().add(drawn);
            if reshuffled {
                broadcast(&players, ServerMessage::DeckReshuffled).await;
            }
        }
        for p in &players {
            let _ = p
                .out
                .send(ServerMessage::YourHand {
                    cards: hands[&p.id].views(),
                })
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

        // `round` span — child of the game span; held open for the whole round.
        // boiling_point/volatility_total are secret (in-process only); round.number
        // and round.exploded are public live-registry keys/outcome.
        let round_span = game_span.in_scope(|| {
            tracing::info_span!(
                "round",
                round.number = round_number as u64,
                boiling_point = effective_bp as i64,
                volatility_total = tracing::field::Empty,
                round.exploded = tracing::field::Empty,
            )
        });

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
                    wave.timer_ms = timer_ms
                )
            });
            broadcast(
                &players,
                ServerMessage::WaveOpened {
                    round_number,
                    wave_number: wave_no,
                    timer_ms,
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
            // Reconnected players resume for future rounds and get a private snapshot.
            for player in &collection.reconnected {
                // `reconnect` span — child of the game span; player.id is public.
                let _reconnect =
                    game_span.in_scope(|| tracing::info_span!("reconnect", player.id = %player.0));
                gone.remove(player);
                let snapshot = ServerMessage::StateSnapshot {
                    room_code: room_code.clone(),
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
            )
        });
        if exploded {
            let result = explosion(round.pot(), &ctx);
            score_span.record("pot.value", result.pot_value as i64);
            for (player, delta) in &result.deltas {
                *scores.get_mut(player).unwrap() += delta;
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
                *scores.get_mut(player).unwrap() += delta;
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
        let tied: Vec<(PlayerId, Hand)> =
            leaders.iter().map(|id| (*id, hands[id].clone())).collect();
        let mut shed_lowest = |_p: PlayerId, hand: &Hand| {
            hand.views()
                .iter()
                .min_by_key(|c| c.view.volatility)
                .unwrap()
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
    tracing::info!(?winners, "game over");
    broadcast(
        &players,
        ServerMessage::GameOver {
            final_scores: scores_vec(&scores, &ids),
            winners,
        },
    )
    .await;
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
    rx: &mut mpsc::Receiver<RoomCommand>,
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
    while !acting.iter().all(|p| locked.contains(p)) {
        tokio::select! {
            _ = &mut sleep => break,
            maybe = rx.recv() => {
                match maybe {
                    None => break,
                    Some(RoomCommand::Action { player, msg }) => {
                        let active = acting.contains(&player) && !gone.contains(&player);
                        match msg {
                            ClientMessage::CommitCard { card }
                                if active
                                    && hands.get(&player).is_some_and(|h| h.contains(card)) =>
                            {
                                choice.insert(player, WaveChoice::Play(card));
                            }
                            ClientMessage::CommitPass if active => {
                                choice.insert(player, WaveChoice::Pass);
                            }
                            ClientMessage::LockIn if active => {
                                locked.insert(player);
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
                            _ => {}
                        }
                    }
                    Some(RoomCommand::Leave { player }) => {
                        gone.insert(player);
                        if acting.contains(&player) {
                            choice.insert(player, WaveChoice::Pass);
                            locked.insert(player);
                        }
                    }
                    Some(RoomCommand::Join { player, out, .. }) => {
                        // A reconnect: reattach the returning player's channel.
                        // The snapshot is sent by the caller once the wave settles.
                        if let Some(seat) = players.iter_mut().find(|s| s.id == player) {
                            seat.out = out;
                            reconnected.push(player);
                        } else {
                            let _ = out
                                .send(ServerMessage::Error {
                                    code: boiling_point_protocol::server::ErrorCode::WrongPhase,
                                    message: "game already in progress".into(),
                                })
                                .await;
                        }
                    }
                    // Force-start is meaningless mid-game; an operator kill closes
                    // the current commit window (the lobby loop owns full teardown).
                    Some(RoomCommand::ForceStart) => {}
                    Some(RoomCommand::Shutdown) => break,
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
    }
}
