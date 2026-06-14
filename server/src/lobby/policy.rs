//! The auto-match queue's pluggable **matching policy** (`boom2-identity`,
//! capability `lobby-and-matchmaking`).
//!
//! Skill-based matchmaking is a *policy*, not a new queue ([design D3]): the v1
//! anchor-and-fill queue keeps its shape — waiting solos backfill a searching
//! group as guests, solos with no group to fill assemble into a fresh four, and
//! a game starts only at exactly four — and only the *ordering* of who fills
//! which seat / who assembles together swaps. [`FirstCome`] reproduces v1
//! first-come exactly (the default and the unrated fallback); [`SkillBased`]
//! prefers similar ratings when everyone in the decision is rated, and **falls
//! back to first-come whenever any participant is unrated** (task 3.3), so
//! anonymous play is unaffected.

/// One queued solo as the policy sees it: just its conservative rating, or
/// `None` if the player is unrated (anonymous, or an account with no games yet
/// surfaced as unrated to the policy). The queue resolves these from the account
/// + rating stores before consulting the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    /// The candidate's conservative display rating, or `None` if unrated.
    pub rating: Option<i32>,
}

/// A matching policy decides ordering within the fixed anchor-and-fill queue.
pub trait MatchPolicy: Send + Sync {
    /// A stable label (for metrics / logging).
    fn name(&self) -> &'static str;

    /// Choose which waiting solo (index into `candidates`, non-empty) backfills
    /// an anchor seat. `anchor_rating` is the searching group's members' mean
    /// rating, or `None` if that group is unrated.
    fn pick_for_anchor(&self, candidates: &[Candidate], anchor_rating: Option<i32>) -> usize;

    /// Choose the four waiting solos (indices into `candidates`, which has at
    /// least four entries) to assemble into a fresh table.
    fn pick_table(&self, candidates: &[Candidate]) -> [usize; 4];
}

/// The v1 policy: strict first-come. The oldest waiter fills the next seat, and
/// the four oldest waiters form a fresh table — independent of rating. The
/// default, and what the queue uses for unrated play.
#[derive(Debug, Default, Clone, Copy)]
pub struct FirstCome;

impl MatchPolicy for FirstCome {
    fn name(&self) -> &'static str {
        "first-come"
    }

    fn pick_for_anchor(&self, _candidates: &[Candidate], _anchor_rating: Option<i32>) -> usize {
        0
    }

    fn pick_table(&self, _candidates: &[Candidate]) -> [usize; 4] {
        [0, 1, 2, 3]
    }
}

/// The skill-based policy: when every participant in a decision is rated, prefer
/// to group players of similar rating; otherwise defer to first-come. This keeps
/// rated tables tight without ever mixing a rated and unrated decision (the
/// unrated fallback, task 3.3) and never changes the queue's exactly-four shape.
#[derive(Debug, Default, Clone, Copy)]
pub struct SkillBased;

impl MatchPolicy for SkillBased {
    fn name(&self) -> &'static str {
        "skill-based"
    }

    fn pick_for_anchor(&self, candidates: &[Candidate], anchor_rating: Option<i32>) -> usize {
        // First-come unless the anchor is rated and *every* waiter is rated.
        let (Some(target), true) = (anchor_rating, candidates.iter().all(|c| c.rating.is_some()))
        else {
            return 0;
        };
        // The closest-rated waiter (ties → the oldest, i.e. lowest index).
        candidates
            .iter()
            .enumerate()
            .min_by_key(|(_, c)| (c.rating.unwrap() - target).abs())
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    fn pick_table(&self, candidates: &[Candidate]) -> [usize; 4] {
        // First-come unless every waiter is rated (mixed populations stay v1).
        if !candidates.iter().all(|c| c.rating.is_some()) {
            return [0, 1, 2, 3];
        }
        // Sort indices by rating, then take the tightest window of four
        // consecutive (minimum spread) — the four most evenly matched players.
        let mut order: Vec<usize> = (0..candidates.len()).collect();
        order.sort_by_key(|&i| candidates[i].rating.unwrap());
        let mut best_start = 0;
        let mut best_spread = i32::MAX;
        for start in 0..=order.len() - 4 {
            let lo = candidates[order[start]].rating.unwrap();
            let hi = candidates[order[start + 3]].rating.unwrap();
            let spread = hi - lo;
            if spread < best_spread {
                best_spread = spread;
                best_start = start;
            }
        }
        [
            order[best_start],
            order[best_start + 1],
            order[best_start + 2],
            order[best_start + 3],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rated(r: i32) -> Candidate {
        Candidate { rating: Some(r) }
    }
    fn unrated() -> Candidate {
        Candidate { rating: None }
    }

    #[test]
    fn first_come_is_positional() {
        let p = FirstCome;
        let c = vec![
            rated(2000),
            rated(1000),
            rated(1500),
            rated(1200),
            rated(900),
        ];
        assert_eq!(
            p.pick_for_anchor(&c, Some(1000)),
            0,
            "oldest fills, ignoring skill"
        );
        assert_eq!(
            p.pick_table(&c),
            [0, 1, 2, 3],
            "the four oldest form a table"
        );
    }

    #[test]
    fn skill_based_picks_the_closest_rated_filler() {
        let p = SkillBased;
        let c = vec![rated(2000), rated(1050), rated(1500)];
        // An anchor near 1000 pulls the 1050 waiter (index 1), not the oldest.
        assert_eq!(p.pick_for_anchor(&c, Some(1000)), 1);
    }

    #[test]
    fn skill_based_groups_the_tightest_four() {
        let p = SkillBased;
        // Six rated waiters; the tightest four are {1000,1050,1100,1150}.
        let c = vec![
            rated(1000),
            rated(2000),
            rated(1050),
            rated(2500),
            rated(1100),
            rated(1150),
        ];
        let mut picked: Vec<i32> = p
            .pick_table(&c)
            .iter()
            .map(|&i| c[i].rating.unwrap())
            .collect();
        picked.sort();
        assert_eq!(picked, vec![1000, 1050, 1100, 1150]);
    }

    #[test]
    fn skill_based_falls_back_to_first_come_when_any_unrated() {
        let p = SkillBased;
        // A mixed pool (some unrated) is matched first-come, exactly as v1.
        let c = vec![rated(1000), unrated(), rated(1500), rated(1200), rated(900)];
        assert_eq!(p.pick_table(&c), [0, 1, 2, 3]);
        // An anchor with an unrated waiter present also defers to first-come.
        assert_eq!(p.pick_for_anchor(&c, Some(1000)), 0);
        // And an unrated anchor (no group rating) defers regardless.
        let all_rated = vec![rated(2000), rated(1000)];
        assert_eq!(p.pick_for_anchor(&all_rated, None), 0);
    }
}
