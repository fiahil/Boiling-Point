//! The per-player decks: the colour-anchored **pantry** (30 ingredients, ~75%
//! own colour) and the fixed **grimoire** (20 spells). Both are instantiated
//! per seat from the content registry's archetypes and seeded for reproducible
//! games.
//!
//! Dealing rules (boom2 combat core): ingredients top up to the
//! [`crate::config::INGREDIENT_HAND`] floor at the start of every wave
//! (refill-only); spells are drawn only at round start (a fixed count), are not
//! replenished in-round except by Forage, and unused ones carry over. The pantry
//! reshuffles its discard (depiled cards) back in when the draw pile empties;
//! the grimoire never reshuffles — cast spells are spent for the game, and a
//! short draw simply yields fewer spells.

use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

use boiling_point_protocol::CardId;
use boiling_point_protocol::vocab::Color;

use crate::config::INGREDIENT_HAND;
use crate::content::card::PantrySlot;
use crate::content::registry::ContentRegistry;

use super::card::{Ingredient, Spell};

/// A player's personal ingredient deck (draw + discard), colour-anchored to its
/// owner.
pub struct Pantry {
    draw: Vec<Ingredient>,
    discard: Vec<Ingredient>,
    rng: StdRng,
}

impl Pantry {
    /// Build one seat's shuffled pantry from the registry's archetypes. `Own`
    /// slots take the owner's colour; `OffColor` slots cycle deterministically
    /// through the other player colours; `Wild` slots are colourless. Instance
    /// ids are pulled from the shared `next_id` counter so ids stay unique
    /// across all decks in a game.
    pub fn build(
        registry: &ContentRegistry,
        own_color: Color,
        next_id: &mut u32,
        seed: u64,
    ) -> Self {
        let off_colors: Vec<Color> = Color::PLAYER_COLORS
            .into_iter()
            .filter(|c| *c != own_color)
            .collect();
        let mut off_cursor = 0usize;
        let mut cards = Vec::new();
        for def in registry.ingredients() {
            for _ in 0..def.copies {
                let color = match def.slot {
                    PantrySlot::Own => own_color,
                    PantrySlot::OffColor => {
                        let c = off_colors[off_cursor % off_colors.len()];
                        off_cursor += 1;
                        c
                    }
                    PantrySlot::Wild => Color::Wild,
                };
                cards.push(Ingredient {
                    id: CardId(*next_id),
                    color,
                    volatility: def.volatility,
                    points: def.points,
                });
                *next_id += 1;
            }
        }
        let mut rng = StdRng::seed_from_u64(seed);
        cards.shuffle(&mut rng);
        Pantry {
            draw: cards,
            discard: Vec::new(),
            rng,
        }
    }

    /// Wrap an already-composed (and already-shuffled) deck — the Apothecary
    /// realizer's output (`boom2-apothecary`). The seed drives only future
    /// discard reshuffles; the realized draw order is kept as given.
    pub fn from_cards(cards: Vec<Ingredient>, seed: u64) -> Self {
        Pantry {
            draw: cards,
            discard: Vec::new(),
            rng: StdRng::seed_from_u64(seed),
        }
    }

    /// Number of ingredients remaining in the draw pile.
    pub fn draw_remaining(&self) -> usize {
        self.draw.len()
    }

    /// Whether both piles are empty (the owner can never act again this game).
    pub fn is_exhausted(&self) -> bool {
        self.draw.is_empty() && self.discard.is_empty()
    }

    /// Draw a single ingredient, reshuffling the discard in if the draw pile is
    /// empty. `None` only if both piles are empty.
    fn draw_one(&mut self) -> Option<Ingredient> {
        if self.draw.is_empty() {
            if self.discard.is_empty() {
                return None;
            }
            std::mem::swap(&mut self.draw, &mut self.discard);
            self.draw.shuffle(&mut self.rng);
        }
        self.draw.pop()
    }

    /// Return spent ingredients (depiled or skimmed) to the discard pile.
    pub fn discard_cards(&mut self, cards: impl IntoIterator<Item = Ingredient>) {
        self.discard.extend(cards);
    }

    /// Top up a hand of `current_len` ingredients to the [`INGREDIENT_HAND`]
    /// floor — refill-only, never trimming. Returns the drawn cards (possibly
    /// short if the pantry exhausted).
    pub fn top_up(&mut self, current_len: usize) -> Vec<Ingredient> {
        self.top_up_to(current_len, INGREDIENT_HAND as usize)
    }

    /// Top up to an explicit `floor` — the Forager's deeper hand
    /// (`boom2-brewers`); refill-only, never trimming.
    pub fn top_up_to(&mut self, current_len: usize, floor: usize) -> Vec<Ingredient> {
        let mut drawn = Vec::new();
        while current_len + drawn.len() < floor {
            match self.draw_one() {
                Some(card) => drawn.push(card),
                None => break,
            }
        }
        drawn
    }
}

/// A player's personal spell deck. No reshuffle: cast spells are spent.
pub struct Grimoire {
    draw: Vec<Spell>,
}

impl Grimoire {
    /// Build one seat's shuffled grimoire from the registry's spell archetypes,
    /// pulling instance ids from the shared `next_id` counter.
    pub fn build(registry: &ContentRegistry, next_id: &mut u32, seed: u64) -> Self {
        Grimoire::build_excluding(registry, next_id, seed, &[])
    }

    /// Build a grimoire **without** the `excluded` spell kinds — the
    /// Cinderwright's ward-free deck (`boom2-brewers`): "can never play a
    /// Ward" is enforced at construction, so no dead card ever reaches the
    /// hand. The deck is simply smaller; a short round-start draw degrades
    /// gracefully like any exhausted grimoire.
    pub fn build_excluding(
        registry: &ContentRegistry,
        next_id: &mut u32,
        seed: u64,
        excluded: &[boiling_point_protocol::vocab::SpellKind],
    ) -> Self {
        let mut spells = Vec::new();
        for def in registry.spells() {
            if excluded.contains(&def.kind) {
                continue;
            }
            for _ in 0..def.copies {
                spells.push(Spell {
                    id: CardId(*next_id),
                    kind: def.kind,
                });
                *next_id += 1;
            }
        }
        let mut rng = StdRng::seed_from_u64(seed);
        spells.shuffle(&mut rng);
        Grimoire { draw: spells }
    }

    /// Wrap an already-composed (and already-shuffled) spell deck — the
    /// Apothecary realizer's output (`boom2-apothecary`). No RNG: the grimoire
    /// never reshuffles.
    pub fn from_spells(spells: Vec<Spell>) -> Self {
        Grimoire { draw: spells }
    }

    /// Number of spells remaining to draw.
    pub fn draw_remaining(&self) -> usize {
        self.draw.len()
    }

    /// Draw up to `n` spells (short if the grimoire is exhausted).
    pub fn draw(&mut self, n: usize) -> Vec<Spell> {
        let take = n.min(self.draw.len());
        self.draw.split_off(self.draw.len() - take)
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
    fn pantry_is_color_anchored() {
        let reg = registry();
        let mut next_id = 0;
        let pantry = Pantry::build(&reg, Color::Ruby, &mut next_id, 42);
        assert_eq!(pantry.draw_remaining(), 30);
        let own = pantry
            .draw
            .iter()
            .filter(|c| c.color == Color::Ruby)
            .count();
        let wild = pantry
            .draw
            .iter()
            .filter(|c| c.color == Color::Wild)
            .count();
        // ~75% own colour; wilds are colourless; the rest are other players' colours.
        assert_eq!(own, 22);
        assert_eq!(wild, 3);
        assert!(
            pantry
                .draw
                .iter()
                .all(|c| c.volatility <= 7 && c.points <= 3)
        );
    }

    #[test]
    fn top_up_refills_to_three() {
        let reg = registry();
        let mut next_id = 0;
        let mut pantry = Pantry::build(&reg, Color::Ruby, &mut next_id, 1);
        let drawn = pantry.top_up(1);
        assert_eq!(drawn.len(), 2); // 1 carried over + 2 drawn = 3
        let none = pantry.top_up(3);
        assert!(none.is_empty()); // already at the floor, draw nothing
    }

    #[test]
    fn pantry_reshuffles_discard_when_exhausted() {
        let reg = registry();
        let mut next_id = 0;
        let mut pantry = Pantry::build(&reg, Color::Emerald, &mut next_id, 7);
        // Drain the whole draw pile.
        let mut pulled = Vec::new();
        while let Some(c) = pantry.draw_one() {
            pulled.push(c);
        }
        assert_eq!(pulled.len(), 30);
        assert!(pantry.is_exhausted());
        pantry.discard_cards(pulled);
        assert!(!pantry.is_exhausted());
        // The next top-up reshuffles the discard back in.
        let drawn = pantry.top_up(0);
        assert_eq!(drawn.len(), 3);
    }

    #[test]
    fn grimoire_draws_short_when_exhausted_and_never_reshuffles() {
        let reg = registry();
        let mut next_id = 100;
        let mut grimoire = Grimoire::build(&reg, &mut next_id, 9);
        assert_eq!(grimoire.draw_remaining(), 20);
        let first = grimoire.draw(18);
        assert_eq!(first.len(), 18);
        // Only 2 left: the draw comes up short, with no reshuffle.
        let last = grimoire.draw(3);
        assert_eq!(last.len(), 2);
        assert_eq!(grimoire.draw_remaining(), 0);
        assert!(grimoire.draw(1).is_empty());
    }

    /// The Cinderwright's grimoire is built without the three wards — "can
    /// never play a Ward" enforced at construction (`boom2-brewers`).
    #[test]
    fn grimoire_excluding_wards_holds_none() {
        use crate::game::brewers::WARDS;
        let reg = registry();
        let mut next_id = 0;
        let grimoire = Grimoire::build_excluding(&reg, &mut next_id, 5, &WARDS);
        // 20 spells minus the 4 ward copies (Cap 2, Halve 1, Redirect 1).
        assert_eq!(grimoire.draw_remaining(), 16);
        assert!(grimoire.draw.iter().all(|s| !WARDS.contains(&s.kind)));
        // The empty exclusion is exactly the plain build.
        let mut next_id = 100;
        let plain = Grimoire::build_excluding(&reg, &mut next_id, 5, &[]);
        assert_eq!(plain.draw_remaining(), 20);
    }

    /// The Forager's deeper floor: top_up_to refills to 4, never trimming.
    #[test]
    fn top_up_to_respects_a_deeper_floor() {
        let reg = registry();
        let mut next_id = 0;
        let mut pantry = Pantry::build(&reg, Color::Sapphire, &mut next_id, 11);
        assert_eq!(pantry.top_up_to(1, 4).len(), 3);
        assert!(pantry.top_up_to(5, 4).is_empty(), "never trims");
    }

    #[test]
    fn card_ids_are_unique_across_decks() {
        let reg = registry();
        let mut next_id = 0;
        let p1 = Pantry::build(&reg, Color::Ruby, &mut next_id, 1);
        let g1 = Grimoire::build(&reg, &mut next_id, 2);
        let p2 = Pantry::build(&reg, Color::Sapphire, &mut next_id, 3);
        let mut ids: Vec<u32> = p1
            .draw
            .iter()
            .map(|c| c.id.0)
            .chain(g1.draw.iter().map(|s| s.id.0))
            .chain(p2.draw.iter().map(|c| c.id.0))
            .collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), total, "duplicate card ids across decks");
    }
}
