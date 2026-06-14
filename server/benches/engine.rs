//! Criterion micro-benchmarks over the v2 engine hot paths (change
//! `boom2-benchmarking`, capability `server-benchmarks`): deck realization, wave
//! resolution, explosion resolution/depile, and modifier stacking.
//!
//! **Seeded and deterministic (spec):** every workload is built from fixed seeds
//! and fixed scenarios, so run-to-run variance reflects only the environment,
//! never the input — the precondition for reading these as trends.
//!
//! **Read as trends, not single runs (D2/R5):** rerun wall-clock variance on this
//! project is 6–12%, so the group sets a `noise_threshold` of 0.10 with extended
//! measurement/warm-up windows and a fixed sample size; a regression is a
//! *sustained level shift across consecutive `main` merges* in the dashboard, never
//! one run's delta. CI passes `--noplot` (plots are dead weight in a headless job;
//! the per-bench `estimates.json` the dashboard reads is written regardless).
//!
//! These never gate (D1): the bench job is observational; only bench *compilation*
//! rides the ordinary build gate.

// A bench crate exposes no API; the `criterion_group!`/`criterion_main!` macros
// emit `pub` items we cannot annotate, so the workspace `missing_docs` warn-lint
// is silenced here rather than left to fail the `-D warnings` gate.
#![allow(missing_docs)]

use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

use boiling_point_protocol::vocab::{Color, GrimoireBucket, ModifierKind, PantryBucket, SpellKind};
use boiling_point_protocol::{CardId, PlayerId};
use boiling_point_server::config::{ContentConfig, DEFAULT_CONTENT_TOML};
use boiling_point_server::content::ContentRegistry;
use boiling_point_server::content::spell::SpellValues;
use boiling_point_server::game::realizer::{realize_grimoire, realize_pantry};
use boiling_point_server::game::round::{Round, WaveInput};
use boiling_point_server::game::{ActiveModifiers, Ingredient};
use uuid::Uuid;

/// The six cauldron modifiers, stacked together to exercise the worst-case
/// compose (every offset/multiplier/parity rule fires at once).
const ALL_MODIFIERS: [ModifierKind; 6] = [
    ModifierKind::Residue,
    ModifierKind::ThinIce,
    ModifierKind::DeepCauldron,
    ModifierKind::BountifulBrew,
    ModifierKind::DoubleStakes,
    ModifierKind::Reversal,
];

/// The embedded default content, the fixed universe every bench realizes against.
fn registry() -> ContentRegistry {
    ContentConfig::from_toml(DEFAULT_CONTENT_TOML)
        .expect("embedded content.toml parses")
        .build_registry()
        .expect("embedded content.toml builds")
}

/// A stable player id from a small integer (no RNG — the seat set is fixed).
fn pid(n: u128) -> PlayerId {
    PlayerId(Uuid::from_u128(n))
}

/// A fixed-shape ingredient: the workload's only knobs are id and volatility.
fn ing(id: u32, volatility: u8) -> Ingredient {
    Ingredient {
        id: CardId(id),
        color: Color::Ruby,
        volatility,
        points: 1,
    }
}

/// One wave's committed cards plus its passers — the unit `apply_wave` consumes.
type Wave = (Vec<(PlayerId, Ingredient, bool)>, Vec<PlayerId>);

/// A four-seat round that settles safely after several commit waves: the common
/// no-boom hot path through `apply_wave` (snapshot, land, resolve, close).
fn safe_round_waves() -> (Vec<PlayerId>, Vec<Wave>) {
    let players: Vec<PlayerId> = (1..=4).map(pid).collect();
    let mut next_id = 1u32;
    let mut waves: Vec<Wave> = Vec::new();
    // Five waves of four low-volatility commits stay well under a high boiling
    // point (no explosion), then a final all-pass wave settles the round.
    for _ in 0..5 {
        let committed = players
            .iter()
            .map(|p| {
                let card = ing(next_id, 1);
                next_id += 1;
                (*p, card, false)
            })
            .collect();
        waves.push((committed, Vec::new()));
    }
    waves.push((Vec::new(), players.clone()));
    (players, waves)
}

/// A round driven to an explosion on its first wave: three heavy commits tip the
/// pot past a low boiling point, leaving a fatal-wave liability sort to depile.
fn exploded_round() -> Round {
    let players: Vec<PlayerId> = (1..=3).map(pid).collect();
    let values = SpellValues {
        dampen: 3,
        surge: 3,
        cap_max: 3,
        hex_bonus: 5,
        harvest_bonus: 3,
        forage_draws: 2,
    };
    let mut round = Round::start(players.clone(), 8, 0);
    round.apply_wave(
        &values,
        WaveInput {
            committed: vec![
                (players[0], ing(1, 3), false),
                (players[1], ing(2, 3), false),
                (players[2], ing(3, 3), false),
            ],
            spells: Vec::new(),
            passers: Vec::new(),
            exhausted: Vec::new(),
        },
    );
    debug_assert!(round.ended().is_some(), "the scenario must explode");
    round
}

/// The whole engine hot-path suite, one `BenchmarkGroup` so every bench lands at
/// the predictable `target/criterion/engine/<name>/new/estimates.json` the
/// dashboard's `collect` walks.
fn bench_engine(c: &mut Criterion) {
    let reg = registry();
    let values = SpellValues {
        dampen: 3,
        surge: 3,
        cap_max: 3,
        hex_bonus: 5,
        harvest_bonus: 3,
        forage_draws: 2,
    };

    let mut group = c.benchmark_group("engine");

    // 1. Deck realization (Apothecary): recipe buckets → fixed-size, capped,
    //    colour-anchored decks. Pantry and grimoire are separate hot paths.
    let pantry_buckets = [
        PantryBucket::Sage,
        PantryBucket::Nightshade,
        PantryBucket::Wisp,
    ];
    group.bench_function("deck_realization_pantry", |b| {
        b.iter(|| {
            let mut next_id = 0u32;
            black_box(realize_pantry(
                black_box(&reg),
                black_box(&pantry_buckets),
                Color::Ruby,
                &mut next_id,
                0xB01,
            ))
        })
    });

    let grimoire_buckets = [GrimoireBucket::Eyebright, GrimoireBucket::Ironbark];
    let reserves = [SpellKind::Peek];
    group.bench_function("deck_realization_grimoire", |b| {
        b.iter(|| {
            let mut next_id = 0u32;
            black_box(realize_grimoire(
                black_box(&reg),
                black_box(&grimoire_buckets),
                black_box(&reserves),
                &[],
                &mut next_id,
                0xB02,
            ))
        })
    });

    // 2. Wave resolution: the full multi-wave loop of a round that settles
    //    safely. `apply_wave` consumes its input, so a fresh round + fresh
    //    inputs are batched in per iteration (cheap setup, excluded from timing).
    let (players, waves) = safe_round_waves();
    group.bench_function("wave_resolution", |b| {
        b.iter_batched(
            || (Round::start(players.clone(), 100, 0), waves.clone()),
            |(mut round, waves)| {
                for (committed, passers) in waves {
                    round.apply_wave(
                        &values,
                        WaveInput {
                            committed,
                            spells: Vec::new(),
                            passers,
                            exhausted: Vec::new(),
                        },
                    );
                }
                black_box(round.ended())
            },
            BatchSize::SmallInput,
        )
    });

    // 3. Explosion resolution / depile: the ascending fatal-wave liability sort
    //    plus the whole-pot depile, on an already-exploded round (built once).
    let boomed = exploded_round();
    group.bench_function("explosion_depile", |b| {
        b.iter(|| {
            black_box(boomed.detonators());
            black_box(boomed.depile())
        })
    });

    // 4. Modifier stacking: push the full six-modifier stack and read every
    //    aggregate (the cumulative compose round scoring leans on each round).
    group.bench_function("modifier_stacking", |b| {
        b.iter(|| {
            let mut mods = ActiveModifiers::new();
            for kind in ALL_MODIFIERS {
                mods.push(kind);
            }
            black_box(mods.boiling_point_delta(&reg));
            black_box(mods.start_volatility(&reg));
            black_box(mods.pot_bonus_per_card(&reg));
            black_box(mods.pot_multiplier(&reg));
            black_box(mods.reversed(&reg));
            black_box(mods.effective_boiling_point(11, &reg))
        })
    });

    group.finish();
}

criterion_group! {
    name = engine;
    // Noise discipline (D2/R5): a 0.10 noise threshold over the observed 6–12%
    // rerun variance, extended measurement/warm-up, and a fixed sample size so the
    // workload, not the schedule, is what every record measures.
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3))
        .sample_size(100)
        .noise_threshold(0.10);
    targets = bench_engine
}
criterion_main!(engine);
