//! The free-for-all player rating (`boom2-identity`, capability `player-rating`).
//!
//! A 4-player result must update four ratings in one consistent computation, so
//! 2-player Elo cannot serve here ([design D2], [roadmap "Identity"]). This is a
//! **Weng-Lin** Bayesian online rating (the Bradley-Terry *full-pair* model from
//! Weng & Lin 2011) — the same family TrueSkill belongs to, but with a
//! closed-form pairwise update and no factor graph. Each player carries a skill
//! estimate `Skill { mu, sigma }`; a finished game's full finishing order folds
//! every pair's win/loss/draw into one update.
//!
//! Server-authoritative (Principle I): the server owns `mu`/`sigma` and exposes
//! only the rounded conservative [`RatingView`] on the wire. The model's
//! parameters are hypotheses marked `[needs playtesting]` (Principle IV) — the
//! [`crate`]'s rating simulation (`clients/ai` `rating_sim`) validates
//! convergence and the chosen values are recorded in
//! `docs/03_architecture/05_identity-and-rating.md`.

use std::collections::HashMap;

use boiling_point_protocol::{AccountId, RatingView};
use dashmap::DashMap;

/// One player's skill estimate: a Gaussian belief with mean `mu` and standard
/// deviation `sigma` (uncertainty). A fresh player starts wide and firms up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Skill {
    /// The estimated skill (mean of the belief).
    pub mu: f64,
    /// The uncertainty (standard deviation); shrinks as games accrue.
    pub sigma: f64,
}

/// The model's tunable parameters. The defaults are the standard
/// TrueSkill/openskill starting points — **hypotheses** (`[needs playtesting]`,
/// Principle IV) validated by the rating simulation and recorded in
/// `docs/03_architecture/05_identity-and-rating.md`.
#[derive(Debug, Clone, Copy)]
pub struct RatingParams {
    /// Initial skill mean for a fresh player.
    pub mu0: f64,
    /// Initial skill uncertainty for a fresh player.
    pub sigma0: f64,
    /// Performance noise per game (the `beta` scale — how much a single result
    /// can move skill). Larger ⇒ more responsive, noisier.
    pub beta: f64,
    /// Dynamics: a small uncertainty added back each game so a settled rating
    /// stays responsive to genuine skill change.
    pub tau: f64,
    /// Floor on the variance-shrink multiplier, so a single lopsided game can
    /// never collapse `sigma` to zero.
    pub kappa: f64,
}

impl Default for RatingParams {
    fn default() -> Self {
        // mu0=25, sigma0=mu0/3, beta=sigma0/2, tau=sigma0/100 — the canonical
        // TrueSkill defaults; validated for this game by `rating_sim` (5.2).
        let mu0 = 25.0;
        let sigma0 = mu0 / 3.0;
        RatingParams {
            mu0,
            sigma0,
            beta: sigma0 / 2.0,
            tau: sigma0 / 100.0,
            kappa: 0.0001,
        }
    }
}

impl RatingParams {
    /// A fresh skill at this parameterisation.
    pub fn fresh(&self) -> Skill {
        Skill {
            mu: self.mu0,
            sigma: self.sigma0,
        }
    }

    /// The conservative ordinal: skill discounted by 3 standard deviations, so a
    /// rating only counts once it is *both* high and certain. Fresh players sit
    /// at `mu0 - 3·sigma0` (≈ 0 at the defaults).
    pub fn ordinal(&self, s: Skill) -> f64 {
        s.mu - 3.0 * s.sigma
    }
}

/// Display scale applied to the conservative ordinal for the wire readout.
const DISPLAY_SCALE: f64 = 40.0;
/// Display offset, so a fresh player reads a friendly round number rather than 0.
const DISPLAY_BASE: i32 = 1000;
/// Rated games below which a rating is still flagged provisional.
const PROVISIONAL_GAMES: u32 = 5;

/// The public, rounded rating view for an account at this skill + game count.
pub fn rating_view(params: &RatingParams, skill: Skill, games_played: u32) -> RatingView {
    RatingView {
        display: (params.ordinal(skill) * DISPLAY_SCALE).round() as i32 + DISPLAY_BASE,
        games_played,
        provisional: games_played < PROVISIONAL_GAMES,
    }
}

/// Update every player's skill from one finished game's finishing order.
///
/// `entrants[i]` is `(skill, rank)` where `rank` is the 1-based finishing
/// position (1 = winner; **ties share a rank**). Returns the post-game skills in
/// the same order. Pure: no I/O, no store — the [`RatingStore`] and the
/// simulation both call this.
///
/// This is the Bradley-Terry full-pair update: for each ordered pair `(i, q)`
/// the result contributes a logistic win-probability surprise to `i`'s mean and
/// shrinks its uncertainty by how informative the pairing was.
pub fn update_finishing_order(params: &RatingParams, entrants: &[(Skill, u32)]) -> Vec<Skill> {
    let n = entrants.len();
    if n < 2 {
        // Nothing to compare against: skill is unchanged (the incomplete /
        // single-account case is handled by the caller, but stay total here).
        return entrants.iter().map(|(s, _)| *s).collect();
    }
    let two_beta_sq = 2.0 * params.beta * params.beta;
    // Inflate variance by the dynamics term once, before pairing.
    let inflated: Vec<Skill> = entrants
        .iter()
        .map(|(s, _)| Skill {
            mu: s.mu,
            sigma: (s.sigma * s.sigma + params.tau * params.tau).sqrt(),
        })
        .collect();

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let si = inflated[i];
        let rank_i = entrants[i].1;
        let sigma_i_sq = si.sigma * si.sigma;
        let mut omega = 0.0; // mean adjustment
        let mut delta = 0.0; // variance shrink (a fraction in [0, 1))
        for q in 0..n {
            if q == i {
                continue;
            }
            let sq = inflated[q];
            let rank_q = entrants[q].1;
            let c = (sigma_i_sq + sq.sigma * sq.sigma + two_beta_sq).sqrt();
            // P(i beats q), via the logistic to stay overflow-safe.
            let p_iq = 1.0 / (1.0 + ((sq.mu - si.mu) / c).exp());
            // Comparison outcome from i's perspective: a lower rank number is a
            // better finish, so i ahead ⇒ win, equal ⇒ draw, behind ⇒ loss.
            let score = match rank_i.cmp(&rank_q) {
                std::cmp::Ordering::Less => 1.0,
                std::cmp::Ordering::Equal => 0.5,
                std::cmp::Ordering::Greater => 0.0,
            };
            let gamma = si.sigma / c; // Weng-Lin variance weight
            omega += (sigma_i_sq / c) * (score - p_iq);
            delta += gamma * (sigma_i_sq / (c * c)) * p_iq * (1.0 - p_iq);
        }
        let new_mu = si.mu + omega;
        let shrink = (1.0 - delta).max(params.kappa);
        let new_sigma = (sigma_i_sq * shrink).sqrt();
        out.push(Skill {
            mu: new_mu,
            sigma: new_sigma,
        });
    }
    out
}

/// One account's stored rating: its skill estimate and the rated-game count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StoredRating {
    /// The current skill estimate.
    pub skill: Skill,
    /// Rated games folded into this estimate.
    pub games: u32,
}

/// Per-account ratings (capability `player-rating`): in-memory and authoritative
/// at runtime, mirroring how [`crate::lobby::SessionStore`] keeps sessions and
/// [`crate::lobby::accounts::AccountStore`] keeps accounts. Durable persistence
/// (write-through + hydrate) is layered on by the caller when a database is
/// configured; with no database the store still works fully in memory (so the
/// e2e suite and the rating simulation need no DB — Principle II).
#[derive(Default)]
pub struct RatingStore {
    params: RatingParams,
    ratings: DashMap<AccountId, StoredRating>,
}

impl RatingStore {
    /// A store with the given parameters.
    pub fn with_params(params: RatingParams) -> Self {
        RatingStore {
            params,
            ratings: DashMap::new(),
        }
    }

    /// The model parameters in force.
    pub fn params(&self) -> &RatingParams {
        &self.params
    }

    /// This account's stored rating, defaulting a fresh skill if unseen.
    pub fn get(&self, account: AccountId) -> StoredRating {
        self.ratings
            .get(&account)
            .map(|r| *r)
            .unwrap_or(StoredRating {
                skill: self.params.fresh(),
                games: 0,
            })
    }

    /// This account's public rating readout.
    pub fn view(&self, account: AccountId) -> RatingView {
        let r = self.get(account);
        rating_view(&self.params, r.skill, r.games)
    }

    /// Seed (hydrate) an account's stored rating from durable storage. Used on
    /// boot when a database is configured.
    pub fn seed(&self, account: AccountId, skill: Skill, games: u32) {
        self.ratings.insert(account, StoredRating { skill, games });
    }

    /// Apply one finished game's full finishing order to the **accounts** at the
    /// table, in one consistent computation, and return the post-game stored
    /// ratings for each. `ordered` is `(account, finish_rank)` for the rated
    /// participants only (anonymous players are excluded by the caller, task
    /// 2.3); ranks share for ties. A game with fewer than two distinct rated
    /// accounts is a no-op (nothing to compare), returning an empty vector.
    ///
    /// Pure in-memory mutation; the caller persists the returned ratings when a
    /// database is configured.
    pub fn apply_finished_game(
        &self,
        ordered: &[(AccountId, u32)],
    ) -> Vec<(AccountId, StoredRating)> {
        // Deduplicate defensively: one account should appear once per game.
        let mut seen: HashMap<AccountId, u32> = HashMap::new();
        for (acc, rank) in ordered {
            seen.entry(*acc).or_insert(*rank);
        }
        if seen.len() < 2 {
            return Vec::new();
        }
        let accounts: Vec<AccountId> = ordered
            .iter()
            .map(|(a, _)| *a)
            .filter({
                let mut once = std::collections::HashSet::new();
                move |a| once.insert(*a)
            })
            .collect();
        let entrants: Vec<(Skill, u32)> = accounts
            .iter()
            .map(|a| (self.get(*a).skill, seen[a]))
            .collect();
        let updated = update_finishing_order(&self.params, &entrants);
        let mut out = Vec::with_capacity(accounts.len());
        for (acc, skill) in accounts.into_iter().zip(updated) {
            let games = self.get(acc).games + 1;
            let stored = StoredRating { skill, games };
            self.ratings.insert(acc, stored);
            out.push((acc, stored));
        }
        out
    }

    /// Number of accounts with a stored (non-default) rating.
    pub fn len(&self) -> usize {
        self.ratings.len()
    }

    /// Whether no account has a stored rating yet.
    pub fn is_empty(&self) -> bool {
        self.ratings.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::AccountId;
    use uuid::Uuid;

    fn acc(n: u128) -> AccountId {
        AccountId(Uuid::from_u128(n))
    }

    /// Winning lifts the mean and shrinks the uncertainty; losing lowers it.
    #[test]
    fn winner_gains_loser_loses() {
        let p = RatingParams::default();
        let s = p.fresh();
        // A two-player game, player 0 wins (rank 1), player 1 loses (rank 2).
        let out = update_finishing_order(&p, &[(s, 1), (s, 2)]);
        assert!(out[0].mu > s.mu, "winner's mean rises");
        assert!(out[1].mu < s.mu, "loser's mean falls");
        assert!(out[0].sigma < s.sigma, "a played game reduces uncertainty");
        assert!(out[1].sigma < s.sigma);
        // Symmetric start ⇒ symmetric move.
        assert!((out[0].mu - s.mu + (out[1].mu - s.mu)).abs() < 1e-9);
    }

    /// A four-way result moves all four consistently: monotone by finish.
    #[test]
    fn four_way_result_is_monotone_by_finish() {
        let p = RatingParams::default();
        let s = p.fresh();
        let out = update_finishing_order(&p, &[(s, 1), (s, 2), (s, 3), (s, 4)]);
        let mu: Vec<f64> = out.iter().map(|s| s.mu).collect();
        assert!(
            mu[0] > mu[1] && mu[1] > mu[2] && mu[2] > mu[3],
            "means strictly decrease with finishing position: {mu:?}"
        );
        // Conservation: a symmetric field's mean is preserved on aggregate.
        let total: f64 = mu.iter().sum();
        assert!(
            (total - 4.0 * s.mu).abs() < 1e-6,
            "mean is conserved: {total}"
        );
    }

    /// A draw between equally rated players moves neither mean.
    #[test]
    fn equal_ranks_are_a_draw() {
        let p = RatingParams::default();
        let s = p.fresh();
        let out = update_finishing_order(&p, &[(s, 1), (s, 1)]);
        assert!(
            (out[0].mu - s.mu).abs() < 1e-9,
            "a draw leaves the mean put"
        );
        assert!(
            out[0].sigma < s.sigma,
            "but the game still informs certainty"
        );
    }

    /// Repeatedly beating the field converges the mean upward and the sigma down.
    #[test]
    fn repeated_wins_converge() {
        let p = RatingParams::default();
        let mut winner = p.fresh();
        for _ in 0..30 {
            let field = p.fresh();
            let out =
                update_finishing_order(&p, &[(winner, 1), (field, 2), (field, 3), (field, 4)]);
            winner = out[0];
        }
        assert!(
            winner.mu > p.mu0 + 5.0,
            "a dominant player climbs: {winner:?}"
        );
        // Uncertainty settles well below the fresh width (the `tau` dynamics term
        // floors how far it can fall, so it plateaus rather than reaching zero).
        assert!(winner.sigma < p.sigma0 * 0.75, "and settles: {winner:?}");
    }

    /// The store applies a game, bumps game counts, and the conservative display
    /// orders winner above loser; a single-account game is a no-op.
    #[test]
    fn store_applies_and_orders() {
        let store = RatingStore::default();
        let (a, b, c, d) = (acc(1), acc(2), acc(3), acc(4));
        let updated = store.apply_finished_game(&[(a, 1), (b, 2), (c, 3), (d, 4)]);
        assert_eq!(updated.len(), 4);
        for (_, r) in &updated {
            assert_eq!(r.games, 1);
        }
        assert!(
            store.view(a).display > store.view(d).display,
            "the winner outranks the loser on the conservative display"
        );
        assert!(store.view(a).provisional, "one game is still provisional");

        // A single rated account has nobody to compare against: no update.
        let solo = store.apply_finished_game(&[(a, 1)]);
        assert!(solo.is_empty());
    }

    /// A fresh account reads the friendly base display and zero games.
    #[test]
    fn fresh_account_reads_base() {
        let store = RatingStore::default();
        let v = store.view(acc(9));
        assert_eq!(v.games_played, 0);
        assert!(v.provisional);
        // Fresh ordinal ≈ 0 ⇒ display ≈ the base.
        assert!((v.display - DISPLAY_BASE).abs() <= 1);
    }
}
