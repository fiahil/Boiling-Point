//! Frame-driven end-to-end validation (`boom-decision-frame`, tasks 1.2–1.4):
//! scripted clients that act ONLY on decision frames play complete games through
//! the real `session::run_game` loop. Property-style across seeds and waves:
//! every action submitted is copied verbatim from the current frame and must
//! never draw an error; a probing seat's non-enumerated submissions must always
//! be rejected; and no frame may carry a secret.

use std::collections::HashSet;

use tokio::sync::mpsc;
use uuid::Uuid;

use boiling_point_protocol::frame::{CastableSpell, PendingDecision, TargetOptions};
use boiling_point_protocol::server::ErrorCode;
use boiling_point_protocol::vocab::{Color, SpellTarget};
use boiling_point_protocol::{CardId, ClientMessage, GroupCode, PlayerId, ServerMessage, codec};

use boiling_point_server::config::ContentConfig;
use boiling_point_server::lobby::group::GroupCommand;
use boiling_point_server::session::{SeatInfo, run_game};

/// What one frame-driven client observed over a full game.
#[derive(Debug, Default)]
struct FrameBotReport {
    /// Decision frames received.
    frames: usize,
    /// Spells this client cast from its frames.
    casts: usize,
    /// Unexpected errors (anything not accounted for by a deliberate probe).
    unexpected_errors: Vec<String>,
    /// Rejections received for deliberate non-enumerated probes.
    probe_rejections: usize,
    /// Deliberate non-enumerated probes sent.
    probes_sent: usize,
    /// Whether the game reached GameOver.
    completed: bool,
}

/// Drive one seat purely off its decision frames: each frame, pick an action
/// from the enumerated set (rotating through plays/pass and through castable
/// spells so the whole set gets sampled across waves), submit it verbatim, and
/// lock in. When `probe` is set, also submit one non-enumerated action per wave
/// first and demand its rejection.
async fn frame_bot(
    me: PlayerId,
    mut rx: mpsc::Receiver<ServerMessage>,
    cmd_tx: mpsc::Sender<GroupCommand>,
    probe: bool,
) -> FrameBotReport {
    let mut report = FrameBotReport::default();
    let mut hand_ids: Vec<CardId> = Vec::new();
    let mut acted: Option<(u8, u8)> = None; // the (round, wave) already answered
    let mut decision_count: usize = 0;
    let mut locked_out = false;

    let send = |msg: ClientMessage| {
        let cmd_tx = cmd_tx.clone();
        async move {
            let _ = cmd_tx.send(GroupCommand::Action { player: me, msg }).await;
        }
    };

    while let Some(msg) = rx.recv().await {
        match msg {
            ServerMessage::YourHand { ingredients, .. } => {
                hand_ids = ingredients.iter().map(|c| c.id).collect();
            }
            ServerMessage::WaveOpened { wave_number: 1, .. } => {
                locked_out = false;
            }
            ServerMessage::WaveResolved { passed, .. } if passed.contains(&me) => {
                locked_out = true;
            }
            ServerMessage::DecisionFrame {
                round_number,
                wave_number,
                decision,
                ..
            } => {
                report.frames += 1;
                // No secret may ride a frame.
                let json = codec::encode_json(&decision).expect("frame encodes");
                assert!(
                    !json.contains("boiling_point"),
                    "a decision frame leaked the boiling point: {json}"
                );
                // The pre-game brewer pick: answer with the first offered
                // option (no probing — the wave frames cover that surface).
                if let PendingDecision::BrewerPick { options } = &decision {
                    send(ClientMessage::PickBrewer { brewer: options[0] }).await;
                    continue;
                }
                // The pre-game draft: the probe seat first submits a
                // duplicate-bucket recipe (never enumerated — must be
                // rejected), then everyone submits the suggested quick-pick
                // verbatim.
                if let PendingDecision::ApothecaryDraft { suggested, .. } = &decision {
                    if probe {
                        report.probes_sent += 1;
                        let mut bad = suggested.clone();
                        bad.pantry[1] = bad.pantry[0];
                        assert!(
                            !decision.permits_recipe(&bad),
                            "a duplicate-bucket recipe must not be enumerated"
                        );
                        send(ClientMessage::SubmitRecipe { recipe: bad }).await;
                    }
                    send(ClientMessage::SubmitRecipe {
                        recipe: suggested.clone(),
                    })
                    .await;
                    continue;
                }
                // Frame/hand agreement: the playable set IS the hand.
                let PendingDecision::WaveCommit {
                    playable,
                    can_pass,
                    spells,
                    ..
                } = &decision
                else {
                    panic!("an in-round frame is a wave commit");
                };
                let listed: Vec<CardId> = playable.iter().map(|p| p.ingredient.id).collect();
                assert_eq!(
                    listed, hand_ids,
                    "the frame's playable set must be exactly the hand"
                );
                assert!(can_pass);

                // A refresh for an already-answered wave never re-acts (a
                // pass + cast legitimately draws one: the accepted cast
                // refreshes the frame even though the pass locked us out).
                if acted == Some((round_number, wave_number)) {
                    continue;
                }
                // A fresh frame is only owed by a player who may still act.
                assert!(!locked_out, "a locked-out player received a frame");
                acted = Some((round_number, wave_number));
                decision_count += 1;

                if probe {
                    // A non-enumerated card must be rejected (NotYourCard).
                    report.probes_sent += 1;
                    send(ClientMessage::CommitIngredient {
                        card: CardId(9_999_999),
                        colorless: false,
                    })
                    .await;
                }

                // Rotate through the enumerated actions across decisions so the
                // whole legal set gets sampled; index len ⇒ the pass option.
                let pick = decision_count % (playable.len() + 1);
                if pick < playable.len() {
                    let p = &playable[pick];
                    send(ClientMessage::CommitIngredient {
                        card: p.ingredient.id,
                        colorless: p.colorless_allowed && decision_count.is_multiple_of(3),
                    })
                    .await;
                } else {
                    send(ClientMessage::CommitPass).await;
                    locked_out = true;
                }

                // Cast an enumerated spell on a rotation (≤1 per wave by rule).
                if !spells.is_empty() && decision_count.is_multiple_of(2) {
                    let s: &CastableSpell = &spells[decision_count % spells.len()];
                    let target = match &s.targets {
                        TargetOptions::None => None,
                        TargetOptions::Players { players } => Some(SpellTarget::Player {
                            player: players[decision_count % players.len()],
                        }),
                        TargetOptions::Colors { colors } => Some(SpellTarget::Color {
                            color: colors[decision_count % colors.len()],
                        }),
                    };
                    send(ClientMessage::CastSpell {
                        spell: s.spell,
                        target,
                    })
                    .await;
                    report.casts += 1;
                }
                send(ClientMessage::LockIn).await;
            }
            ServerMessage::Error { code, message } => {
                // NotYourCard rejects the wave probe; InvalidTarget the
                // duplicate-recipe draft probe.
                if probe && matches!(code, ErrorCode::NotYourCard | ErrorCode::InvalidTarget) {
                    report.probe_rejections += 1;
                } else {
                    report
                        .unexpected_errors
                        .push(format!("{code:?}: {message}"));
                }
            }
            ServerMessage::GameOver { .. } => {
                report.completed = true;
                break;
            }
            _ => {}
        }
    }
    report
}

/// Run one full seeded game with four frame-driven seats (seat 0 probes).
async fn run_frame_driven_game(seed: u64) -> Vec<FrameBotReport> {
    let mut cfg = ContentConfig::from_toml(include_str!("../content.toml")).unwrap();
    cfg.timing.wave1_ms = 3_000;
    cfg.timing.wave_ms = 3_000;
    cfg.timing.brewer_pick_ms = 3_000;
    cfg.timing.draft_ms = 3_000;
    let registry = cfg.build_registry().unwrap();

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<GroupCommand>(512);
    let mut seats = Vec::new();
    let mut bots = Vec::new();
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
        bots.push(frame_bot(id, out_rx, cmd_tx.clone(), i == 0));
    }
    drop(cmd_tx);

    let mut it = bots.into_iter();
    let (b0, b1, b2, b3) = (
        it.next().unwrap(),
        it.next().unwrap(),
        it.next().unwrap(),
        it.next().unwrap(),
    );
    let palette: HashSet<u16> = HashSet::new();
    let game = run_game(
        &registry,
        &cfg,
        GroupCode("FRAME-TEST".into()),
        seats,
        &mut cmd_rx,
        &palette,
        seed,
        None,
    );
    let (_end, r0, r1, r2, r3) = tokio::time::timeout(std::time::Duration::from_secs(60), async {
        tokio::join!(game, b0, b1, b2, b3)
    })
    .await
    .unwrap_or_else(|_| panic!("frame-driven game for seed {seed} timed out"));
    vec![r0, r1, r2, r3]
}

/// Everything copied verbatim from a frame validates (no errors, ever); every
/// non-enumerated probe is rejected; frames flow every wave; games complete.
#[tokio::test]
async fn frame_enumerated_actions_always_validate_and_probes_never_do() {
    let mut total_casts = 0usize;
    let mut total_frames = 0usize;
    for seed in [7u64, 42, 0xC0FFEE, 1234, 98765] {
        let reports = run_frame_driven_game(seed).await;
        for (i, r) in reports.iter().enumerate() {
            assert!(r.completed, "seat {i} (seed {seed}) saw GameOver");
            assert!(
                r.unexpected_errors.is_empty(),
                "seat {i} (seed {seed}): frame-enumerated submissions drew errors: {:?}",
                r.unexpected_errors
            );
            assert!(r.frames > 0, "seat {i} (seed {seed}) received frames");
            total_frames += r.frames;
            total_casts += r.casts;
        }
        // The probe seat's every non-enumerated submission was rejected.
        assert!(reports[0].probes_sent > 0);
        assert_eq!(
            reports[0].probe_rejections, reports[0].probes_sent,
            "seed {seed}: every non-enumerated probe must be rejected"
        );
    }
    assert!(total_frames > 0);
    assert!(
        total_casts > 0,
        "the rotation must exercise the spell-cast surface"
    );
}

/// The per-game choices a deterministic frame-driven table makes are a function
/// of the seed: re-running the same seed yields the same number of frames and
/// casts at every seat (the frame contract adds no nondeterminism).
#[tokio::test]
async fn frame_driven_games_are_stable_under_a_seed() {
    let a = run_frame_driven_game(2026).await;
    let b = run_frame_driven_game(2026).await;
    for (x, y) in a.iter().zip(b.iter()) {
        assert_eq!(x.frames, y.frames);
        assert_eq!(x.casts, y.casts);
    }
}

/// A second probe shape, end to end: casting a spell the frame does not list
/// (an unheld spell) is rejected with `NotYourSpell` and the game proceeds
/// unaffected to GameOver.
#[tokio::test]
async fn non_enumerated_spell_probes_are_rejected() {
    let mut cfg = ContentConfig::from_toml(include_str!("../content.toml")).unwrap();
    cfg.timing.wave1_ms = 1_500;
    cfg.timing.wave_ms = 1_500;
    cfg.timing.brewer_pick_ms = 1_500;
    cfg.timing.draft_ms = 1_500;
    let registry = cfg.build_registry().unwrap();

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<GroupCommand>(512);
    let mut seats = Vec::new();
    let mut outs = Vec::new();
    for (i, color) in Color::PLAYER_COLORS.into_iter().enumerate() {
        let (out_tx, out_rx) = mpsc::channel::<ServerMessage>(512);
        seats.push(SeatInfo {
            id: PlayerId(Uuid::from_u128(i as u128 + 1)),
            name: format!("p{i}"),
            color,
            guest: false,
            out: out_tx,
        });
        outs.push(out_rx);
    }
    let mut rx0 = outs.remove(0);
    let me = PlayerId(Uuid::from_u128(1));

    let palette: HashSet<u16> = HashSet::new();
    let game = run_game(
        &registry,
        &cfg,
        GroupCode("PROBE-TEST".into()),
        seats,
        &mut cmd_rx,
        &palette,
        99,
        None,
    );

    // Seats 1–3 pass every frame; seat 0 probes each frame with an unheld
    // spell before passing, and tallies its errors.
    let mut passers = Vec::new();
    for (i, mut rx) in outs.into_iter().enumerate() {
        let cmd_tx = cmd_tx.clone();
        let id = PlayerId(Uuid::from_u128(i as u128 + 2));
        passers.push(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    ServerMessage::DecisionFrame {
                        decision: PendingDecision::BrewerPick { options },
                        ..
                    } => {
                        let _ = cmd_tx
                            .send(GroupCommand::Action {
                                player: id,
                                msg: ClientMessage::PickBrewer { brewer: options[0] },
                            })
                            .await;
                    }
                    ServerMessage::DecisionFrame {
                        decision: PendingDecision::ApothecaryDraft { suggested, .. },
                        ..
                    } => {
                        let _ = cmd_tx
                            .send(GroupCommand::Action {
                                player: id,
                                msg: ClientMessage::SubmitRecipe { recipe: suggested },
                            })
                            .await;
                    }
                    ServerMessage::DecisionFrame { .. } => {
                        let _ = cmd_tx
                            .send(GroupCommand::Action {
                                player: id,
                                msg: ClientMessage::CommitPass,
                            })
                            .await;
                        let _ = cmd_tx
                            .send(GroupCommand::Action {
                                player: id,
                                msg: ClientMessage::LockIn,
                            })
                            .await;
                    }
                    ServerMessage::GameOver { .. } => break,
                    _ => {}
                }
            }
        });
    }
    let prober = {
        let cmd_tx = cmd_tx.clone();
        async move {
            let mut errors: Vec<ErrorCode> = Vec::new();
            let mut probes = 0usize;
            while let Some(msg) = rx0.recv().await {
                match msg {
                    ServerMessage::DecisionFrame {
                        decision: PendingDecision::BrewerPick { options },
                        ..
                    } => {
                        let _ = cmd_tx
                            .send(GroupCommand::Action {
                                player: me,
                                msg: ClientMessage::PickBrewer { brewer: options[0] },
                            })
                            .await;
                    }
                    ServerMessage::DecisionFrame {
                        decision: PendingDecision::ApothecaryDraft { suggested, .. },
                        ..
                    } => {
                        let _ = cmd_tx
                            .send(GroupCommand::Action {
                                player: me,
                                msg: ClientMessage::SubmitRecipe { recipe: suggested },
                            })
                            .await;
                    }
                    ServerMessage::DecisionFrame { decision, .. } => {
                        assert!(
                            !decision.permits_cast(CardId(9_999_999), None),
                            "the bogus spell must not be enumerated"
                        );
                        probes += 1;
                        let _ = cmd_tx
                            .send(GroupCommand::Action {
                                player: me,
                                msg: ClientMessage::CastSpell {
                                    spell: CardId(9_999_999),
                                    target: None,
                                },
                            })
                            .await;
                        let _ = cmd_tx
                            .send(GroupCommand::Action {
                                player: me,
                                msg: ClientMessage::CommitPass,
                            })
                            .await;
                        let _ = cmd_tx
                            .send(GroupCommand::Action {
                                player: me,
                                msg: ClientMessage::LockIn,
                            })
                            .await;
                    }
                    ServerMessage::Error { code, .. } => errors.push(code),
                    ServerMessage::GameOver { .. } => break,
                    _ => {}
                }
            }
            (probes, errors)
        }
    };
    drop(cmd_tx);

    let mut it = passers.into_iter();
    let (p1, p2, p3) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
    let (_end, (probes, errors), _, _, _) =
        tokio::time::timeout(std::time::Duration::from_secs(60), async {
            tokio::join!(game, prober, p1, p2, p3)
        })
        .await
        .expect("probe game completed");

    assert!(probes > 0, "the prober saw at least one frame");
    assert_eq!(
        errors.len(),
        probes,
        "every probe drew exactly one rejection: {errors:?}"
    );
    assert!(
        errors.iter().all(|c| matches!(c, ErrorCode::NotYourSpell)),
        "unheld-spell probes are rejected as NotYourSpell: {errors:?}"
    );
}
