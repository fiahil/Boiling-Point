//! The rated-population simulation runner (`boom2-identity` task 5.1/5.2): the
//! Principle IV instrument that validates the FFA rating model and the
//! skill-based matching policy against synthetic finishing orders.
//!
//! Defaults to a 200-player, 20k-game seeded run, reporting convergence
//! (Spearman of rating vs hidden true skill), match quality (within-table skill
//! spread, skill policy vs the first-come control), and cold-start (games for a
//! strong newcomer to climb above the median). All observational — the numbers
//! feed `docs/03_architecture/05_identity-and-rating.md`, they never gate.

use clap::Parser;

use boiling_point_ai_client::harness::{Matching, SimConfig, SimReport, run_simulation};

/// Seeded rated-population simulation (Principle IV).
#[derive(Parser)]
#[command(name = "rating_sim")]
struct Cli {
    /// Population size.
    #[arg(long, default_value_t = 200)]
    players: usize,
    /// Games to simulate.
    #[arg(long, default_value_t = 20_000)]
    games: u64,
    /// Root seed.
    #[arg(long, default_value_t = 0xB011_1465)]
    seed: u64,
    /// Hidden true-skill spread (std-dev, performance units).
    #[arg(long, default_value_t = 1.0)]
    skill_spread: f64,
    /// Queued-pool size the matching policy chooses four from (quality↑/wait↑).
    #[arg(long, default_value_t = 8)]
    pool: usize,
}

fn main() {
    let cli = Cli::parse();
    let config = SimConfig {
        players: cli.players,
        games: cli.games,
        seed: cli.seed,
        skill_spread: cli.skill_spread,
        pool: cli.pool,
    };
    let skill = run_simulation(config, Matching::Skill);
    let random = run_simulation(config, Matching::Random);
    print!("{}", markdown(&skill, &random));
}

/// A diffable markdown summary of a paired skill/first-come run.
fn markdown(skill: &SimReport, random: &SimReport) -> String {
    let c = skill.config;
    let mut s = String::new();
    s.push_str("# Rating simulation\n\n");
    s.push_str(&format!(
        "Population {}, games {}, seed {}, skill spread {}, pool {}.\n\n",
        c.players, c.games, c.seed, c.skill_spread, c.pool
    ));
    s.push_str("| metric | skill matching | first-come (control) |\n");
    s.push_str("|---|---|---|\n");
    s.push_str(&format!(
        "| Spearman (rating vs true skill) | {:.3} | {:.3} |\n",
        skill.spearman, random.spearman
    ));
    s.push_str(&format!(
        "| mean within-table skill spread | {:.3} | {:.3} |\n",
        skill.mean_table_spread, random.mean_table_spread
    ));
    s.push_str(&format!(
        "| cold-start games (strong newcomer → above median) | {} | {} |\n",
        skill
            .coldstart_games
            .map(|g| g.to_string())
            .unwrap_or_else(|| "never".into()),
        random
            .coldstart_games
            .map(|g| g.to_string())
            .unwrap_or_else(|| "never".into()),
    ));
    s.push_str(&format!(
        "| mean games / player | {:.1} | {:.1} |\n",
        skill.mean_games_per_player, random.mean_games_per_player
    ));
    s
}
