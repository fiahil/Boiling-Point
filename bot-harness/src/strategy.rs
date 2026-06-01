//! Pluggable bot decision-making (D3).
//!
//! Every decision a bot makes in a wave — play which card, or pass, plus the
//! optional table-talk emote — is a method on the [`Strategy`] trait, a pure
//! function of the bot's [`PlayerView`] and its seeded RNG. Purity under a seed is
//! what keeps a whole batch reproducible (D4): a strategy may consult only the
//! player-visible view and the RNG handed to it, never the wall clock or any
//! ambient state.
//!
//! Four baselines ship for comparison — [`Cautious`], [`Aggressor`],
//! [`Diplomat`], and [`Random`]. They are deliberately simple starting
//! hypotheses; the trait exists so sharper strategies can be slotted in as
//! balance understanding grows. Strategies are assigned per seat via
//! [`assignment_from_names`].

use rand::rngs::StdRng;
use rand::Rng;

use boiling_point_protocol::vocab::{EffectKind, HandCard};
use boiling_point_protocol::{CardId, EmoteId};

use crate::model::PlayerView;

/// A bot's chosen action for a wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveAction {
    /// Commit a specific card from hand into the cauldron.
    Play(CardId),
    /// Pass — a permanent lockout for the rest of the round.
    Pass,
}

/// A play pattern: how a bot decides each wave. Pure in `(view, rng)`.
pub trait Strategy: Send + Sync {
    /// A stable, human-readable name (used to attribute wins in the report).
    fn name(&self) -> &'static str;

    /// Choose this wave's action from the player-visible view and the seeded RNG.
    ///
    /// Called only when the bot is still active (has cards and has not passed), so
    /// implementations always have at least one card to consider.
    fn decide(&self, view: &PlayerView, rng: &mut StdRng) -> WaveAction;

    /// Optionally send a preset emote (the only comms channel). Cosmetic — it
    /// never affects game state — so the default is silence.
    fn emote(
        &self,
        _view: &PlayerView,
        _palette: &[EmoteId],
        _rng: &mut StdRng,
    ) -> Option<EmoteId> {
        None
    }
}

/// Cauldron card count at or above which the cautious/diplomat bots treat the pot
/// as explosion-risky. A proxy: a bot cannot see the hidden volatility total, only
/// how many cards have accumulated.
const RISKY_CARD_COUNT: u8 = 7;

/// Whether the bot owns (scores for) a card's colour.
fn is_mine(view: &PlayerView, card: &HandCard) -> bool {
    card.view.color == view.my_color
}

/// The bot's own-colour card that scores best for the least risk: highest points,
/// then lowest volatility, then lowest id (a fully deterministic tiebreak).
fn best_scoring_card(view: &PlayerView) -> Option<CardId> {
    view.hand
        .iter()
        .filter(|c| is_mine(view, c))
        .max_by(|a, b| {
            a.view
                .points
                .cmp(&b.view.points)
                .then(b.view.volatility.cmp(&a.view.volatility)) // prefer lower volatility
                .then(b.id.0.cmp(&a.id.0)) // prefer lower id
        })
        .map(|c| c.id)
}

/// The lowest-volatility card in hand, preferring own colour then higher points,
/// then lower id. The cautious "tread carefully" pick.
fn safest_card(view: &PlayerView) -> Option<CardId> {
    view.hand
        .iter()
        .min_by(|a, b| {
            a.view
                .volatility
                .cmp(&b.view.volatility)
                .then(is_mine(view, b).cmp(&is_mine(view, a))) // prefer own colour
                .then(b.view.points.cmp(&a.view.points)) // prefer more points
                .then(a.id.0.cmp(&b.id.0))
        })
        .map(|c| c.id)
}

/// The first card in hand carrying a given effect, if any.
fn card_with_effect(view: &PlayerView, effect: EffectKind) -> Option<CardId> {
    view.hand
        .iter()
        .filter(|c| c.view.effect == Some(effect))
        .min_by_key(|c| c.id.0)
        .map(|c| c.id)
}

/// The highest-points card in hand regardless of colour (the aggressor fallback).
fn highest_points_card(view: &PlayerView) -> Option<CardId> {
    view.hand
        .iter()
        .max_by(|a, b| {
            a.view
                .points
                .cmp(&b.view.points)
                .then(a.view.volatility.cmp(&b.view.volatility)) // aggressor likes volatility
                .then(b.id.0.cmp(&a.id.0))
        })
        .map(|c| c.id)
}

/// Plays low and bails early: passes once the pot looks risky or it is comfortably
/// ahead, otherwise sheds its safest card.
pub struct Cautious;

impl Strategy for Cautious {
    fn name(&self) -> &'static str {
        "cautious"
    }

    fn decide(&self, view: &PlayerView, _rng: &mut StdRng) -> WaveAction {
        let ahead = matches!(
            (view.my_score(), view.best_opponent_score()),
            (Some(me), Some(best)) if me > best
        );
        if view.cauldron_card_count >= RISKY_CARD_COUNT || (ahead && view.cauldron_card_count >= 4)
        {
            return WaveAction::Pass;
        }
        match safest_card(view) {
            Some(card) => WaveAction::Play(card),
            None => WaveAction::Pass,
        }
    }
}

/// Pushes the pot: plays its biggest-scoring card every wave and effectively never
/// passes, stress-testing whether reckless play is over-rewarded.
pub struct Aggressor;

impl Strategy for Aggressor {
    fn name(&self) -> &'static str {
        "aggressor"
    }

    fn decide(&self, view: &PlayerView, _rng: &mut StdRng) -> WaveAction {
        best_scoring_card(view)
            .or_else(|| highest_points_card(view))
            .map(WaveAction::Play)
            .unwrap_or(WaveAction::Pass)
    }
}

/// Adapts to the table: shields or peeks when the pot turns dangerous, presses for
/// points when behind, and plays safe or passes when ahead.
pub struct Diplomat;

impl Strategy for Diplomat {
    fn name(&self) -> &'static str {
        "diplomat"
    }

    fn decide(&self, view: &PlayerView, _rng: &mut StdRng) -> WaveAction {
        let risky = view.cauldron_card_count >= RISKY_CARD_COUNT;

        // When the pot is dangerous, reach for protection or information.
        if risky {
            if let Some(shield) = card_with_effect(view, EffectKind::Shield) {
                return WaveAction::Play(shield);
            }
            return WaveAction::Pass;
        }
        // Early in a round, a free Peek buys (private) certainty.
        if view.wave_number <= 1
            && view.known_boiling_point().is_none()
            && let Some(peek) = card_with_effect(view, EffectKind::Peek)
        {
            return WaveAction::Play(peek);
        }

        let behind = matches!(
            (view.my_score(), view.best_opponent_score()),
            (Some(me), Some(best)) if me < best
        );
        let pick = if behind {
            best_scoring_card(view).or_else(|| safest_card(view))
        } else {
            safest_card(view)
        };
        pick.map(WaveAction::Play).unwrap_or(WaveAction::Pass)
    }

    fn emote(&self, _view: &PlayerView, palette: &[EmoteId], rng: &mut StdRng) -> Option<EmoteId> {
        // An occasional, harmless table-talk emote — purely to exercise the channel.
        if palette.is_empty() || !rng.gen_bool(0.1) {
            return None;
        }
        Some(palette[rng.gen_range(0..palette.len())])
    }
}

/// Uniformly random over {pass} ∪ {play each held card} — the noise floor every
/// other strategy must beat to justify its heuristics.
pub struct Random;

impl Strategy for Random {
    fn name(&self) -> &'static str {
        "random"
    }

    fn decide(&self, view: &PlayerView, rng: &mut StdRng) -> WaveAction {
        // Options: pass (index 0) or play hand[i] (index i+1).
        let choice = rng.gen_range(0..=view.hand.len());
        if choice == 0 {
            WaveAction::Pass
        } else {
            WaveAction::Play(view.hand[choice - 1].id)
        }
    }
}

/// Construct a baseline strategy by name, or `None` if the name is unknown.
pub fn by_name(name: &str) -> Option<Box<dyn Strategy>> {
    match name {
        "cautious" => Some(Box::new(Cautious)),
        "aggressor" => Some(Box::new(Aggressor)),
        "diplomat" => Some(Box::new(Diplomat)),
        "random" => Some(Box::new(Random)),
        _ => None,
    }
}

/// The names of every baseline strategy, in a stable order.
pub const BASELINE_NAMES: [&str; 4] = ["cautious", "aggressor", "diplomat", "random"];

/// Build a per-seat strategy assignment from four names (one per seat). Returns
/// the first unknown name as an error.
pub fn assignment_from_names(names: &[String]) -> Result<Vec<Box<dyn Strategy>>, String> {
    names
        .iter()
        .map(|n| by_name(n).ok_or_else(|| format!("unknown strategy: {n}")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::vocab::{CardView, Color};
    use uuid::Uuid;

    fn view_with(hand: Vec<HandCard>, cauldron: u8) -> PlayerView {
        let mut v = PlayerView::new(PlayerId(Uuid::from_u128(1)), Color::Ruby);
        v.hand = hand;
        v.cauldron_card_count = cauldron;
        v.wave_number = 2;
        v
    }

    use boiling_point_protocol::PlayerId;

    fn card(id: u32, color: Color, vol: u8, pts: u8, effect: Option<EffectKind>) -> HandCard {
        HandCard {
            id: CardId(id),
            view: CardView {
                color,
                volatility: vol,
                points: pts,
                effect,
            },
        }
    }

    fn rng() -> StdRng {
        use rand::SeedableRng;
        StdRng::seed_from_u64(1)
    }

    /// The cautious bot bails out of a crowded (risky) pot.
    #[test]
    fn cautious_passes_a_risky_pot() {
        let v = view_with(vec![card(1, Color::Ruby, 1, 2, None)], RISKY_CARD_COUNT);
        assert_eq!(Cautious.decide(&v, &mut rng()), WaveAction::Pass);
    }

    /// The cautious bot sheds its lowest-volatility card in a calm pot.
    #[test]
    fn cautious_plays_safest_card() {
        let v = view_with(
            vec![
                card(1, Color::Ruby, 3, 3, None),
                card(2, Color::Ruby, 1, 1, None),
            ],
            1,
        );
        assert_eq!(Cautious.decide(&v, &mut rng()), WaveAction::Play(CardId(2)));
    }

    /// The aggressor maximises its own-colour points.
    #[test]
    fn aggressor_plays_best_scoring_card() {
        let v = view_with(
            vec![
                card(1, Color::Ruby, 1, 1, None),
                card(2, Color::Ruby, 3, 3, None),
            ],
            1,
        );
        assert_eq!(
            Aggressor.decide(&v, &mut rng()),
            WaveAction::Play(CardId(2))
        );
    }

    /// The diplomat reaches for a Shield when the pot is dangerous.
    #[test]
    fn diplomat_shields_when_risky() {
        let v = view_with(
            vec![
                card(1, Color::Ruby, 1, 2, None),
                card(2, Color::Amethyst, 2, 0, Some(EffectKind::Shield)),
            ],
            RISKY_CARD_COUNT,
        );
        assert_eq!(Diplomat.decide(&v, &mut rng()), WaveAction::Play(CardId(2)));
    }

    /// Random only ever returns options that exist (a held card or a pass).
    #[test]
    fn random_stays_in_bounds() {
        let v = view_with(vec![card(5, Color::Ruby, 1, 1, None)], 0);
        for seed in 0..50u64 {
            use rand::SeedableRng;
            let mut r = StdRng::seed_from_u64(seed);
            match Random.decide(&v, &mut r) {
                WaveAction::Pass => {}
                WaveAction::Play(id) => assert_eq!(id, CardId(5)),
            }
        }
    }

    /// Names resolve to strategies; unknown names are rejected.
    #[test]
    fn assignment_resolves_known_names() {
        let names: Vec<String> = BASELINE_NAMES.iter().map(|s| s.to_string()).collect();
        let assigned = assignment_from_names(&names).expect("all known");
        assert_eq!(assigned.len(), 4);
        assert!(assignment_from_names(&["nope".to_string()]).is_err());
    }
}
