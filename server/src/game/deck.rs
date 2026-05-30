//! The shared deck: builds concrete cards from the content registry, deals a
//! refill-to-5 floor with carryover, and reshuffles the discard back in when the
//! draw pile is exhausted (D-R5). Seeded for reproducible games.

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

use boiling_point_protocol::CardId;

use crate::config::HAND_SIZE;
use crate::content::ContentRegistry;

use super::card::Card;

/// The draw/discard deck. Cards leave the draw pile into hands/pot, are revealed
/// at the depile, then return to the discard; when the draw pile empties, the
/// discard is reshuffled into it (a visible table event — card counting resets).
pub struct Deck {
    draw: Vec<Card>,
    discard: Vec<Card>,
    rng: StdRng,
}

impl Deck {
    /// Build a shuffled deck from the registry's enabled card archetypes, seeded
    /// for reproducibility. Card ids are assigned sequentially.
    pub fn build(registry: &ContentRegistry, seed: u64) -> Self {
        let mut cards = Vec::new();
        let mut next_id = 0u32;
        for def in registry.cards() {
            for _ in 0..def.copies {
                cards.push(Card {
                    id: CardId(next_id),
                    color: def.color,
                    volatility: def.volatility,
                    points: def.points,
                    effect: def.effect,
                });
                next_id += 1;
            }
        }
        let mut rng = StdRng::seed_from_u64(seed);
        cards.shuffle(&mut rng);
        Deck {
            draw: cards,
            discard: Vec::new(),
            rng,
        }
    }

    /// Number of cards remaining in the draw pile.
    pub fn draw_remaining(&self) -> usize {
        self.draw.len()
    }

    /// Draw a single card, reshuffling the discard in if the draw pile is empty.
    /// Returns the card and whether a reshuffle happened. `None` only if both
    /// piles are empty (which a validated deck size avoids in practice).
    pub fn draw_one(&mut self) -> Option<(Card, bool)> {
        if self.draw.is_empty() {
            if self.discard.is_empty() {
                return None;
            }
            std::mem::swap(&mut self.draw, &mut self.discard);
            self.draw.shuffle(&mut self.rng);
            let card = self.draw.pop().expect("non-empty after reshuffle");
            return Some((card, true));
        }
        Some((self.draw.pop().expect("non-empty"), false))
    }

    /// Return cards to the discard pile (e.g. after a depile).
    pub fn discard_cards(&mut self, cards: impl IntoIterator<Item = Card>) {
        self.discard.extend(cards);
    }

    /// Refill a hand of `current_len` cards up to [`HAND_SIZE`] — a floor that
    /// only ever adds. Returns the drawn cards and whether a reshuffle occurred.
    pub fn refill(&mut self, current_len: usize) -> (Vec<Card>, bool) {
        let mut drawn = Vec::new();
        let mut reshuffled = false;
        while current_len + drawn.len() < HAND_SIZE as usize {
            match self.draw_one() {
                Some((card, r)) => {
                    reshuffled |= r;
                    drawn.push(card);
                }
                None => break,
            }
        }
        (drawn, reshuffled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> ContentRegistry {
        let cfg = crate::config::ContentConfig::from_toml(include_str!("../../content.toml"))
            .expect("parse");
        cfg.build_registry().expect("build")
    }

    #[test]
    fn build_yields_full_deck() {
        let deck = Deck::build(&registry(), 42);
        assert_eq!(deck.draw_remaining(), 90);
    }

    #[test]
    fn refill_tops_up_to_five() {
        let mut deck = Deck::build(&registry(), 1);
        let (drawn, reshuffled) = deck.refill(2);
        assert_eq!(drawn.len(), 3); // 2 carried over + 3 drawn = 5
        assert!(!reshuffled);
        let (none, _) = deck.refill(5);
        assert!(none.is_empty()); // already at the floor, draw nothing
    }

    #[test]
    fn reshuffles_discard_when_draw_empties() {
        let mut deck = Deck::build(&registry(), 7);
        // Drain the draw pile into the discard.
        let mut pulled = Vec::new();
        while let Some((c, r)) = deck.draw_one() {
            assert!(!r, "no reshuffle while cards remain");
            pulled.push(c);
            if deck.draw_remaining() == 0 {
                break;
            }
        }
        deck.discard_cards(pulled);
        // Next draw must reshuffle the discard back in.
        let (_, reshuffled) = deck.draw_one().expect("card after reshuffle");
        assert!(reshuffled);
    }
}
