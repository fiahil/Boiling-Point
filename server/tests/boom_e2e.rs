//! End-to-end "triggering a boom" (boom2-observability task 4.4): a
//! **deterministic seeded game**, driven server-side and fully headless by
//! scripted bot clients, must surface the boom through the whole observability
//! pipeline — the v2 boom spans appear in the projection's replay tree and the
//! boom-rate balance metric reports exactly the engine's ground truth.
//!
//! Determinism: the engine is seeded, so the same seed + the same bot policy
//! (commit the first hand card, else pass) yields the same rounds, booms, and
//! detonators every run. The ground truth is computed by running the **sync**
//! engine first; the async game loop must then produce telemetry that matches
//! it exactly — deterministic games yield deterministic assertions.
//!
//! Runs in its own integration-test binary (its own process) so its
//! process-global span subscriber and projection are isolated.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tracing::Instrument as _;
use uuid::Uuid;

use boiling_point_protocol::frame::PendingDecision;
use boiling_point_protocol::vocab::{Brewer, Color, GrimoireBucket, Recipe};
use boiling_point_protocol::{CardId, ClientMessage, GroupCode, PlayerId, ServerMessage};

use boiling_point_server::admin::AdminProjection;
use boiling_point_server::config::ContentConfig;
use boiling_point_server::game::round::{WaveAction, WaveChoice};
use boiling_point_server::game::runner::Game;
use boiling_point_server::game::state::{Hand, Player};
use boiling_point_server::lobby::GroupCommand;
use boiling_point_server::observability;
use boiling_point_server::session::{self, SeatInfo};

/// The fixed scenario seed. With the first-card bot policy this seed booms —
/// the property the scenario exists to observe.
const SEED: u64 = 42;

/// Seat ids shared by the sync ground truth and the async game.
fn player_ids() -> Vec<(PlayerId, Color)> {
    Color::PLAYER_COLORS
        .into_iter()
        .enumerate()
        .map(|(i, color)| (PlayerId(Uuid::from_u128(i as u128 + 1)), color))
        .collect()
}

/// The deterministic bot policy, sync form: play the first hand ingredient,
/// else pass.
fn play_first_else_pass(_player: PlayerId, hand: &Hand) -> WaveChoice {
    match hand.ingredients().first() {
        Some(first) => WaveChoice {
            action: WaveAction::Play {
                card: first.id,
                colorless: false,
            },
            spell: None,
            second_spell: None,
        },
        None => WaveChoice::pass(),
    }
}

/// The same policy as a headless async bot client: commit the first hand card
/// (else pass) and lock in, until `GameOver`.
async fn bot(
    player: PlayerId,
    tx: mpsc::Sender<GroupCommand>,
    mut out: mpsc::Receiver<ServerMessage>,
) -> bool {
    let mut hand: Vec<CardId> = Vec::new();
    let mut passed = false;
    while let Some(msg) = out.recv().await {
        match msg {
            ServerMessage::YourHand { ingredients, .. } => {
                hand = ingredients.iter().map(|c| c.id).collect();
            }
            // Pick the first offered Brewer — the auto-pick rule, so the sync
            // ground truth reproduces the assignments.
            ServerMessage::DecisionFrame {
                decision: PendingDecision::BrewerPick { options },
                ..
            } => {
                let _ = tx
                    .send(GroupCommand::Action {
                        player,
                        msg: ClientMessage::PickBrewer { brewer: options[0] },
                    })
                    .await;
            }
            // Submit the frame's suggested quick-pick recipe — the straggler
            // default, so the sync ground truth reproduces the realized decks.
            ServerMessage::DecisionFrame {
                decision: PendingDecision::ApothecaryDraft { suggested, .. },
                ..
            } => {
                let _ = tx
                    .send(GroupCommand::Action {
                        player,
                        msg: ClientMessage::SubmitRecipe { recipe: suggested },
                    })
                    .await;
            }
            ServerMessage::WaveOpened { wave_number, .. } => {
                if wave_number == 1 {
                    passed = false;
                }
                if passed {
                    continue;
                }
                let action = match hand.first() {
                    Some(&card) => {
                        hand.remove(0);
                        ClientMessage::CommitIngredient {
                            card,
                            colorless: false,
                        }
                    }
                    None => {
                        passed = true;
                        ClientMessage::CommitPass
                    }
                };
                let _ = tx
                    .send(GroupCommand::Action {
                        player,
                        msg: action,
                    })
                    .await;
                let _ = tx
                    .send(GroupCommand::Action {
                        player,
                        msg: ClientMessage::LockIn,
                    })
                    .await;
            }
            ServerMessage::WaveResolved {
                passed: passers, ..
            } if passers.contains(&player) => passed = true,
            ServerMessage::GameOver { .. } => return true,
            _ => {}
        }
    }
    false
}

/// Wait until `f` returns true or the deadline passes (the lifecycle consumer
/// is fed asynchronously on a drain thread).
async fn wait_until(mut f: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if f() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    f()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_seeded_boom_appears_in_spans_and_the_boom_rate_metric() {
    observability::init("127.0.0.1:0".parse().expect("metrics addr"), None);
    let projection = Arc::new(AdminProjection::new());
    observability::lifecycle::register_consumer(projection.clone());

    let cfg = ContentConfig::from_toml(include_str!("../content.toml")).unwrap();
    let registry = cfg.build_registry().unwrap();

    // ---- Ground truth: the seeded sync engine decides what must be observed.
    let sync_players: Vec<Player> = player_ids()
        .into_iter()
        .map(|(id, color)| Player {
            id,
            color,
            display_name: format!("p{id}", id = id.0),
        })
        .collect();
    // The bots pick the first of each dealt pair and submit the suggested
    // quick-pick recipe — the deterministic deal + auto-pick + straggler
    // defaults — so the ground truth carries the same brewers and decks.
    let brewers: std::collections::HashMap<PlayerId, Brewer> = sync_players
        .iter()
        .zip(session::deal_brewer_pairs(SEED, sync_players.len()))
        .map(|(p, pair)| (p.id, session::auto_pick(&pair)))
        .collect();
    let recipes: std::collections::HashMap<PlayerId, Recipe> = sync_players
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let excluded =
                boiling_point_server::game::brewers::excluded_buckets(brewers.get(&p.id).copied());
            let options: Vec<GrimoireBucket> = GrimoireBucket::ALL
                .into_iter()
                .filter(|b| !excluded.contains(b))
                .collect();
            (p.id, session::suggested_recipe(SEED, i, &options))
        })
        .collect();
    let mut sync_game = Game::with_recipes(&registry, &cfg, sync_players, brewers, recipes, SEED);
    let mut decider = play_first_else_pass;
    let truth = sync_game.play_out(&mut decider);
    let expected_rounds = truth.rounds.len() as u64;
    let expected_booms = truth.rounds.iter().filter(|r| r.exploded).count() as u64;
    assert!(
        expected_booms >= 1,
        "the scenario seed must boom — pick another seed"
    );

    // ---- Drive the real async game loop with the same seed, under a synthetic
    // group.lifetime span (the projection's registry key).
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<GroupCommand>(512);
    let mut seats = Vec::new();
    let mut bots = Vec::new();
    for (i, (id, color)) in player_ids().into_iter().enumerate() {
        let (out_tx, out_rx) = mpsc::channel::<ServerMessage>(512);
        seats.push(SeatInfo {
            id,
            name: format!("bot{i}"),
            color,
            guest: false,
            out: out_tx,
        });
        bots.push(tokio::spawn(bot(id, cmd_tx.clone(), out_rx)));
    }
    drop(cmd_tx);
    let palette: HashSet<u16> = HashSet::new();
    let accounts = boiling_point_server::lobby::accounts::AccountStore::new();
    let ratings = boiling_point_server::rating::RatingStore::default();
    let group_span = tracing::info_span!("group.lifetime", group.code = "BOOM-E2E");
    let _ = tokio::time::timeout(
        Duration::from_secs(60),
        session::run_game(
            &registry,
            &cfg,
            GroupCode("BOOM-E2E".into()),
            seats,
            &mut cmd_rx,
            &palette,
            SEED,
            None,
            &accounts,
            &ratings,
        )
        .instrument(group_span),
    )
    .await
    .expect("the seeded game completes");
    for b in bots {
        assert!(b.await.unwrap(), "every bot saw GameOver");
    }

    // ---- The boom-rate metric reports exactly the ground truth, from the
    // shared boom-balance-metrics definition over the unsampled aggregates.
    assert!(
        wait_until(|| projection.balance().games >= 1).await,
        "the completed game should fold into the aggregates"
    );
    let balance = projection.balance();
    assert_eq!(balance.rounds, expected_rounds);
    let boom_rate = balance
        .metrics
        .iter()
        .find(|m| m.id == "boom_rate")
        .expect("boom_rate is defined")
        .value
        .expect("boom_rate has a population");
    assert_eq!(
        boom_rate,
        expected_booms as f64 / expected_rounds as f64,
        "boom rate must equal the engine's deterministic ground truth"
    );

    // ---- The boom spans appear in the retained replay tree: boomed rounds,
    // detonator-carrying score spans, the fatal resolve, and the depile's
    // crossing index — exactly as many as the engine booms.
    let replays = projection.replay_list();
    assert_eq!(
        replays.len(),
        1,
        "the completed game is retained for replay"
    );
    let game = projection
        .replay(&replays[0].game_id)
        .expect("replay retained");
    let count = |pred: &dyn Fn(&boiling_point_server::admin::projection::CompletedSpan) -> bool| {
        game.spans.iter().filter(|s| pred(s)).count() as u64
    };
    assert_eq!(
        count(&|s| s.name == "round"
            && s.attributes.get("round.boomed").map(String::as_str) == Some("true")),
        expected_booms,
        "boomed round spans must match the ground truth"
    );
    assert_eq!(
        count(&|s| s.name == "score" && s.attributes.contains_key("detonators")),
        expected_booms,
        "every boom's score span carries the detonator split"
    );
    assert_eq!(
        count(&|s| s.name == "resolve" && s.attributes.contains_key("detonators")),
        expected_booms,
        "every boom's fatal wave resolve span carries the detonator split"
    );
    assert_eq!(
        count(&|s| s.name == "depile" && s.attributes.contains_key("crossing_index")),
        expected_booms,
        "every boom's depile records where the climb crossed the boiling point"
    );
    // The detonator split matches the engine's fatal-wave sort, round by round.
    let mut expected_detonators: Vec<String> = truth
        .rounds
        .iter()
        .filter(|r| r.exploded)
        .map(|r| {
            r.detonators
                .iter()
                .map(|p| p.0.to_string())
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect();
    let mut observed: Vec<String> = game
        .spans
        .iter()
        .filter(|s| s.name == "score")
        .filter_map(|s| s.attributes.get("detonators").cloned())
        .collect();
    expected_detonators.sort();
    observed.sort();
    assert_eq!(
        observed, expected_detonators,
        "the observed detonator splits must equal the deterministic ground truth"
    );
}
