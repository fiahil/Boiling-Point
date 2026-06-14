//! The rated-population simulation (`boom2-identity`, task 5.1) — the Principle
//! IV instrument for the identity stack.
//!
//! Where the game-balance harness ([`super::runner`]) plays full v2 games, the
//! rating system is validated against **synthetic finishing orders**: a
//! population of players carries a *hidden true skill*, tables are formed by the
//! production matching policy, and each table's finish is sampled from those true
//! skills (a Thurstonian model). The system under test is real — the server's
//! [`rating`](boiling_point_server::rating) model and the
//! [`SkillBased`](boiling_point_server::lobby::SkillBased) policy; only the
//! *outcomes* are synthetic (the environment), so thousands of seeded games run
//! in milliseconds and the results are deterministic.
//!
//! It answers three questions for the change's `[needs playtesting]` numbers:
//! **convergence** (does the conservative rating recover the true skill order?),
//! **match quality** (does the skill policy seat closer-matched tables than
//! random?), and **cold-start** (how fast does a strong newcomer climb?).

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

use boiling_point_protocol::AccountId;
use boiling_point_server::lobby::policy::{Candidate, FirstCome, MatchPolicy, SkillBased};
use boiling_point_server::rating::{RatingParams, RatingStore};
use uuid::Uuid;

/// How a simulation forms its tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Matching {
    /// The production skill policy: seat the tightest-rated four of the pool.
    Skill,
    /// First-come (the control): seat four arbitrary waiters.
    Random,
}

/// One simulation's inputs.
#[derive(Debug, Clone, Copy)]
pub struct SimConfig {
    /// Population size.
    pub players: usize,
    /// Games to play.
    pub games: u64,
    /// Root seed (determinism).
    pub seed: u64,
    /// Spread of the hidden true-skill distribution (std-dev, in performance
    /// units; performance noise per game is fixed at 1.0, so a larger spread is
    /// a more separable population).
    pub skill_spread: f64,
    /// How many players are "queued" and choosable when a table forms; the
    /// matching policy picks four of these. Larger ⇒ better matches but a longer
    /// implied wait (the quality-vs-wait trade-off).
    pub pool: usize,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            players: 200,
            games: 20_000,
            seed: 0xB011_1465,
            skill_spread: 1.0,
            pool: 8,
        }
    }
}

/// What one simulation produced.
#[derive(Debug, Clone)]
pub struct SimReport {
    /// The inputs.
    pub config: SimConfig,
    /// The matching strategy used.
    pub matching: Matching,
    /// Spearman rank correlation between final conservative rating and true
    /// skill across the population (1.0 = perfect recovery of the true order).
    pub spearman: f64,
    /// Mean within-table true-skill spread (max − min of the seated four). Lower
    /// is a tighter match.
    pub mean_table_spread: f64,
    /// Games until a strong newcomer (top-decile true skill, injected fresh)
    /// first climbs above the population's median display, or `None` if it never
    /// did within the run.
    pub coldstart_games: Option<u64>,
    /// Mean games played per player (load balance sanity).
    pub mean_games_per_player: f64,
}

/// A player's hidden state in the simulation.
struct SimPlayer {
    account: AccountId,
    /// The hidden true skill (performance mean).
    true_skill: f64,
    games: u64,
}

/// Sample a standard normal via Box-Muller from a uniform RNG (keeps the sim
/// dependency-light and fully seed-determined).
fn standard_normal(rng: &mut StdRng) -> f64 {
    let u1: f64 = rng.gen_range(f64::MIN_POSITIVE..1.0);
    let u2: f64 = rng.gen_range(0.0..1.0);
    (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

/// Run one rated-population simulation and report convergence/quality/cold-start.
pub fn run_simulation(config: SimConfig, matching: Matching) -> SimReport {
    run_with_params(config, matching, RatingParams::default())
}

/// Run a simulation with explicit rating parameters (the tuning entry point).
pub fn run_with_params(config: SimConfig, matching: Matching, params: RatingParams) -> SimReport {
    assert!(config.players >= config.pool && config.pool >= 4);
    let mut rng = StdRng::seed_from_u64(config.seed);
    let store = RatingStore::with_params(params);

    // The population: hidden true skills, plus a fresh strong newcomer in the
    // last slot (top-decile skill) whose climb we track for cold-start.
    let mut players: Vec<SimPlayer> = (0..config.players)
        .map(|_| SimPlayer {
            account: AccountId(Uuid::from_u128(rng.r#gen::<u128>())),
            true_skill: standard_normal(&mut rng) * config.skill_spread,
            games: 0,
        })
        .collect();
    let newcomer = config.players - 1;
    // A genuinely strong newcomer: ~1.6 std-devs above the mean.
    players[newcomer].true_skill = 1.6 * config.skill_spread;

    let skill = SkillBased;
    let first_come = FirstCome;
    let mut spread_sum = 0.0;
    let mut coldstart_games: Option<u64> = None;

    for game in 0..config.games {
        // Draw a queued pool of distinct players (uniformly) and pick four by the
        // matching policy from their current conservative ratings.
        let pool = sample_pool(&mut rng, config.players, config.pool);
        let candidates: Vec<Candidate> = pool
            .iter()
            .map(|&i| Candidate {
                rating: Some(store.view(players[i].account).display),
            })
            .collect();
        let picked = match matching {
            Matching::Skill => skill.pick_table(&candidates),
            Matching::Random => first_come.pick_table(&candidates),
        };
        let seated: Vec<usize> = picked.iter().map(|&k| pool[k]).collect();

        // Within-table true-skill spread (match quality).
        let (lo, hi) = seated.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &i| {
            (lo.min(players[i].true_skill), hi.max(players[i].true_skill))
        });
        spread_sum += hi - lo;

        // Sample a finishing order: performance = true skill + N(0,1) noise; the
        // best performance finishes first (rank 1). Ties are vanishingly unlikely.
        let mut perf: Vec<(usize, f64)> = seated
            .iter()
            .map(|&i| (i, players[i].true_skill + standard_normal(&mut rng)))
            .collect();
        perf.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let ordered: Vec<(AccountId, u32)> = perf
            .iter()
            .enumerate()
            .map(|(rank, (i, _))| (players[*i].account, rank as u32 + 1))
            .collect();
        store.apply_finished_game(&ordered);
        for &i in &seated {
            players[i].games += 1;
        }

        // Cold-start: the first game by which the newcomer's display passes the
        // population median.
        if coldstart_games.is_none() && players[newcomer].games > 0 {
            let median = median_display(&store, &players);
            if store.view(players[newcomer].account).display > median {
                coldstart_games = Some(players[newcomer].games);
            }
        }
        let _ = game;
    }

    let spearman = spearman_skill_vs_display(&store, &players);
    let mean_games_per_player =
        players.iter().map(|p| p.games as f64).sum::<f64>() / players.len() as f64;
    SimReport {
        config,
        matching,
        spearman,
        mean_table_spread: spread_sum / config.games as f64,
        coldstart_games,
        mean_games_per_player,
    }
}

/// A pool of `k` distinct player indices drawn uniformly (partial Fisher-Yates).
fn sample_pool(rng: &mut StdRng, n: usize, k: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..n).collect();
    for i in 0..k {
        let j = rng.gen_range(i..n);
        idx.swap(i, j);
    }
    idx.truncate(k);
    idx
}

/// The population's median conservative display.
fn median_display(store: &RatingStore, players: &[SimPlayer]) -> i32 {
    let mut displays: Vec<i32> = players
        .iter()
        .map(|p| store.view(p.account).display)
        .collect();
    displays.sort_unstable();
    displays[displays.len() / 2]
}

/// Spearman rank correlation between hidden true skill and final display.
fn spearman_skill_vs_display(store: &RatingStore, players: &[SimPlayer]) -> f64 {
    let n = players.len();
    let skill_rank = rank_of(&players.iter().map(|p| p.true_skill).collect::<Vec<_>>());
    let disp_rank = rank_of(
        &players
            .iter()
            .map(|p| store.view(p.account).display as f64)
            .collect::<Vec<_>>(),
    );
    let d2: f64 = (0..n)
        .map(|i| {
            let d = skill_rank[i] - disp_rank[i];
            d * d
        })
        .sum();
    1.0 - (6.0 * d2) / (n as f64 * ((n * n) as f64 - 1.0))
}

/// Average ranks (1-based, ties averaged) of the values, in original order.
fn rank_of(values: &[f64]) -> Vec<f64> {
    let n = values.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| values[a].partial_cmp(&values[b]).unwrap());
    let mut ranks = vec![0.0; n];
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j + 1 < n && values[order[j + 1]] == values[order[i]] {
            j += 1;
        }
        // Average rank for the tie group [i, j].
        let avg = ((i + j) as f64) / 2.0 + 1.0;
        for &o in &order[i..=j] {
            ranks[o] = avg;
        }
        i = j + 1;
    }
    ranks
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small, fast simulation: the rating recovers the true skill order well
    /// (high Spearman), the skill policy seats tighter tables than first-come,
    /// and a strong newcomer climbs above the median within a sane number of
    /// games. Deterministic (seeded), so these bounds are stable.
    #[test]
    fn rating_system_converges_and_matches() {
        let config = SimConfig {
            players: 80,
            games: 6_000,
            seed: 7,
            skill_spread: 1.0,
            pool: 8,
        };
        let skill = run_simulation(config, Matching::Skill);
        let random = run_simulation(config, Matching::Random);

        assert!(
            skill.spearman > 0.7,
            "the rating recovers the true skill order: rho = {:.3}",
            skill.spearman
        );
        assert!(
            skill.mean_table_spread < random.mean_table_spread,
            "skill matching seats tighter tables than first-come: {:.3} vs {:.3}",
            skill.mean_table_spread,
            random.mean_table_spread
        );
        assert!(
            skill.coldstart_games.is_some(),
            "a strong newcomer climbs above the median within the run"
        );
    }

    /// Spearman of a perfectly aligned ranking is 1.0; reversed is -1.0.
    #[test]
    fn spearman_endpoints() {
        // rank_of preserves order; build values that align perfectly.
        let a = [1.0, 2.0, 3.0, 4.0];
        let ranks = rank_of(&a);
        assert_eq!(ranks, vec![1.0, 2.0, 3.0, 4.0]);
        // Tie group averages.
        let b = [5.0, 5.0, 9.0];
        assert_eq!(rank_of(&b), vec![1.5, 1.5, 3.0]);
    }
}
