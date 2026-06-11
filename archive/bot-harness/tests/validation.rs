//! End-to-end validation of the harness (tasks 7.1/7.2).
//!
//! These exercise the public batch API against the embedded default config: that
//! a seeded in-process batch is bit-for-bit reproducible, that the WebSocket
//! backend genuinely plays to `GameOver` over the real wire, and that a skewed
//! win distribution is flagged as a degenerate-strategy candidate.

use boiling_point_bot_harness::bot::RoundObservation;
use boiling_point_bot_harness::report::{Smell, Thresholds};
use boiling_point_bot_harness::runner::{
    BatchParams, GameRecord, SeatRecord, TransportKind, run_batch,
};
use boiling_point_bot_harness::stats::BatchStats;
use boiling_point_server::config::ContentConfig;

use boiling_point_protocol::PlayerId;
use boiling_point_protocol::vocab::{Color, ModifierKind};
use uuid::Uuid;

/// The checked-in default content config (same one the server embeds).
const DEFAULT_CONFIG: &str = include_str!("../../server/content.toml");

fn config() -> ContentConfig {
    ContentConfig::from_toml(DEFAULT_CONFIG).expect("parse default config")
}

fn params(games: u64, seed: u64, transport: TransportKind) -> BatchParams {
    BatchParams {
        games,
        seed,
        transport,
        strategy_names: vec![
            "cautious".into(),
            "aggressor".into(),
            "diplomat".into(),
            "random".into(),
        ],
    }
}

/// 7.1 — the same seed reproduces identical per-game outcomes AND identical
/// aggregated statistics on re-run.
#[tokio::test(flavor = "current_thread")]
async fn seeded_batch_is_reproducible() {
    let cfg = config();
    let p = params(50, 2026, TransportKind::InProcess);

    let run_a = run_batch(&cfg, &p).await.expect("batch a");
    let run_b = run_batch(&cfg, &p).await.expect("batch b");

    // Identical game outcomes, round for round.
    assert_eq!(run_a, run_b, "the same seed must reproduce every game");
    // Identical aggregated statistics.
    assert_eq!(
        BatchStats::aggregate(&run_a),
        BatchStats::aggregate(&run_b),
        "the same seed must reproduce the report"
    );
    // Sanity: the batch actually played complete games.
    assert_eq!(run_a.len(), 50);
    assert!(run_a.iter().all(|g| g.completed && g.rounds.len() == 5));
}

/// A different seed produces a different run (the seed actually matters).
#[tokio::test(flavor = "current_thread")]
async fn different_seed_diverges() {
    let cfg = config();
    let a = run_batch(&cfg, &params(50, 1, TransportKind::InProcess))
        .await
        .expect("a");
    let b = run_batch(&cfg, &params(50, 2, TransportKind::InProcess))
        .await
        .expect("b");
    assert_ne!(a, b, "distinct seeds should not coincide");
}

/// 7.2 — an intentionally skewed win distribution is flagged as a
/// degenerate-strategy candidate, with the supporting numbers, through the real
/// aggregate → detect pipeline.
#[test]
fn skewed_strategy_is_flagged_as_degenerate() {
    // Synthesize 100 games where "aggressor" wins almost every one — exactly the
    // degeneracy the harness must catch.
    let seats = |winner_strategy: &str| {
        vec![
            SeatRecord {
                player_id: PlayerId(Uuid::from_u128(1)),
                color: Color::Ruby,
                strategy: "cautious".into(),
            },
            SeatRecord {
                player_id: PlayerId(Uuid::from_u128(2)),
                color: Color::Sapphire,
                strategy: "aggressor".into(),
            },
            SeatRecord {
                player_id: PlayerId(Uuid::from_u128(3)),
                color: Color::Emerald,
                strategy: "diplomat".into(),
            },
            SeatRecord {
                player_id: PlayerId(Uuid::from_u128(4)),
                color: Color::Amethyst,
                strategy: "random".into(),
            },
        ]
        .into_iter()
        .find(|s| s.strategy == winner_strategy)
        .map(|s| s.player_id)
        .unwrap()
    };

    let round = |exploded: bool| RoundObservation {
        round_number: 1,
        exploded,
        pot_value: 8,
        cards_in_pot: 6,
        waves: 2,
        modifier: Some(ModifierKind::ThinIce),
    };

    let records: Vec<GameRecord> = (0..100u64)
        .map(|i| {
            // 90 of 100 wins go to the aggressor.
            let winner = if i < 90 {
                seats("aggressor")
            } else {
                seats("random")
            };
            GameRecord {
                index: i,
                seats: vec![
                    SeatRecord {
                        player_id: PlayerId(Uuid::from_u128(1)),
                        color: Color::Ruby,
                        strategy: "cautious".into(),
                    },
                    SeatRecord {
                        player_id: PlayerId(Uuid::from_u128(2)),
                        color: Color::Sapphire,
                        strategy: "aggressor".into(),
                    },
                    SeatRecord {
                        player_id: PlayerId(Uuid::from_u128(3)),
                        color: Color::Emerald,
                        strategy: "diplomat".into(),
                    },
                    SeatRecord {
                        player_id: PlayerId(Uuid::from_u128(4)),
                        color: Color::Amethyst,
                        strategy: "random".into(),
                    },
                ],
                // A healthy ~35% explosion rate so the explosion smell does NOT fire,
                // isolating the strategy-dominance signal.
                rounds: vec![round(i % 3 == 0)],
                winners: vec![winner],
                completed: true,
            }
        })
        .collect();

    let stats = BatchStats::aggregate(&records);
    let smells = Smell::detect(&stats, &Thresholds::default());

    let dominant = smells
        .iter()
        .find(|s| s.kind == "dominant_strategy")
        .expect("a dominant strategy must be flagged");
    assert!(
        dominant.detail.contains("aggressor"),
        "the flag must name the dominant strategy: {}",
        dominant.detail
    );
    assert!(
        dominant.detail.contains("90"),
        "the flag must carry the supporting numbers: {}",
        dominant.detail
    );
}

/// The WebSocket backend plays real games over the wire to completion (a smaller
/// honesty-check batch — not seed-reproducible, see runner docs).
#[tokio::test(flavor = "current_thread")]
async fn websocket_backend_completes_games() {
    let cfg = config();
    let records = run_batch(&cfg, &params(2, 0, TransportKind::WebSocket))
        .await
        .expect("ws batch");
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .all(|g| g.completed && g.rounds.len() == 5 && !g.winners.is_empty())
    );
}
