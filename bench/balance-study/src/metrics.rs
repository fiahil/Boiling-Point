//! The shared §IV fold (design D3): map the harness's completed bot games onto
//! [`BalanceEvent`]s and evaluate the **same**
//! [`boiling_point_server::observability::balance_metrics`] definitions the live
//! pipeline evaluates — one definition, two populations. The study never
//! re-derives a formula; it feeds events and reads values.
//!
//! The harness's per-game [`RoundObservation`](boiling_point_ai_client::observe::RoundObservation)
//! is broadcast-visible facts only, derived from the wire — so the fold carries
//! the *rate* metrics (boom, freeze, detonators-per-boom, fold, wave depth,
//! spell casts) faithfully, but it has **no wall clock**: a batch resolves games
//! instantly. The duration-denominated definitions (round/wave/game seconds) are
//! therefore live-telemetry only and are reported as "no data" rather than a
//! misleading `0s` — see [`study_metrics`].

use boiling_point_ai_client::harness::SampleRun;
use boiling_point_server::observability::balance_metrics::{
    Accumulator, BalanceEvent, MetricValue, Unit, evaluate_all,
};

/// Fold every round of every game of a completed sample into the shared
/// accumulator. Durations are left at zero (the harness has no wall clock); the
/// rate populations (rounds, booms, detonators, waves, commits/folds, casts) are
/// exact.
pub fn accumulate(run: &SampleRun) -> Accumulator {
    let mut acc = Accumulator::default();
    for cell in &run.cells {
        for game in &cell.games {
            acc.record(&BalanceEvent::GameCompleted { duration_ms: 0 });
            for round in &game.rounds {
                acc.record(&BalanceEvent::RoundSettled {
                    boomed: round.exploded,
                    // The shared "freeze" is an all-pass settle — the harness's
                    // own all-pass signal, identical population.
                    frozen: round.ended_all_pass,
                    duration_ms: 0,
                });
                if round.exploded {
                    acc.record(&BalanceEvent::Boom {
                        detonators: round.detonators.len() as u64,
                    });
                }
                // Spell casts split into Peek vs the rest, so the per-spell view
                // keeps the Peek-economy signal the §IV set leans on.
                let peeks = u64::from(round.peek_casts);
                let others = u64::from(round.spell_casts.saturating_sub(round.peek_casts));
                for _ in 0..peeks {
                    acc.record(&BalanceEvent::SpellCast {
                        kind: "Peek".into(),
                    });
                }
                for _ in 0..others {
                    acc.record(&BalanceEvent::SpellCast {
                        kind: "other".into(),
                    });
                }
                // Wave-level commits/folds aren't broken out per wave on the wire;
                // attribute the round's totals to its first wave so the per-round
                // wave *count* and the fold-rate population are both exact.
                let commits = u64::from(round.cards_in_pot);
                let folds = round.folded_safe.len() as u64;
                for w in 0..round.waves {
                    acc.record(&BalanceEvent::WaveResolved {
                        timed_out: false,
                        commits: if w == 0 { commits } else { 0 },
                        folds: if w == 0 { folds } else { 0 },
                        duration_ms: 0,
                    });
                }
            }
        }
    }
    acc
}

/// The §IV metric values for a study: every shared definition evaluated over the
/// fold, with the duration-denominated metrics nulled to "no data" (the harness
/// measures rates, not wall-clock — durations belong to the live pipeline).
pub fn study_metrics(acc: &Accumulator) -> Vec<MetricValue> {
    evaluate_all(acc)
        .into_iter()
        .map(|mut m| {
            if m.unit == Unit::Seconds {
                m.value = None;
            }
            m
        })
        .collect()
}
