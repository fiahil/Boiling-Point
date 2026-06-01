//! The deterministic RNG tree (D4).
//!
//! A single root seed derives every other stream in a run: a per-game seed, the
//! server-side game seed, and a per-bot decision stream. A
//! `(root seed, strategy assignment, content config)` triple therefore fully
//! reproduces a batch — essential for debugging a flagged game and for diffing
//! two config versions.
//!
//! All derivation goes through [`derive`], a pure `splitmix64` mix of the parent
//! seed and a salt. Nothing here reads the wall clock, the OS entropy pool, or
//! hashmap iteration order, so the same inputs always yield the same streams.
//! Bots draw exclusively from a [`rand::rngs::StdRng`] seeded by this tree — the
//! one sanctioned source on the decision path (see [`crate::bot`]).

use rand::SeedableRng;
use rand::rngs::StdRng;

/// Salt mixed in to derive a single game's seed from the root.
const GAME_SALT: u64 = 0x6761_6D65_5345_4544; // "gameSEED"
/// Salt mixed in to derive the server-side game seed from a game's seed.
const SERVER_SALT: u64 = 0x7372_7672_5345_4544; // "srvrSEED"
/// Salt mixed in to derive a per-seat bot stream from a game's seed.
const SEAT_SALT: u64 = 0x7365_6174_5345_4544; // "seatSEED"

/// Mix a parent seed with a salt into a child seed (the `splitmix64` finaliser).
///
/// Deterministic and well-distributed: tiny input changes (e.g. an adjacent seat
/// index) produce uncorrelated child seeds, so two bots never share a stream.
pub fn derive(parent: u64, salt: u64) -> u64 {
    let mut z = parent
        .wrapping_add(salt)
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The full set of seeds for a single game, derived from the root and game index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameSeeds {
    /// Seed handed to the server's `run_game` (deck shuffle, boiling point, modifiers).
    pub server: u64,
    /// The per-game root from which per-seat bot streams are derived.
    game: u64,
}

impl GameSeeds {
    /// Derive all seeds for game `index` (0-based) under `root`.
    pub fn for_game(root: u64, index: u64) -> Self {
        let game = derive(root, derive(GAME_SALT, index));
        GameSeeds {
            server: derive(game, SERVER_SALT),
            game,
        }
    }

    /// A fresh, independent RNG for the bot in seat `seat` (0-based).
    pub fn bot_rng(&self, seat: usize) -> StdRng {
        StdRng::seed_from_u64(derive(self.game, derive(SEAT_SALT, seat as u64)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    /// Derivation is a pure function: same inputs, same output.
    #[test]
    fn derive_is_deterministic() {
        assert_eq!(derive(42, GAME_SALT), derive(42, GAME_SALT));
        assert_ne!(derive(42, GAME_SALT), derive(43, GAME_SALT));
        assert_ne!(derive(42, GAME_SALT), derive(42, SEAT_SALT));
    }

    /// Each game and each seat gets a distinct stream under one root.
    #[test]
    fn seeds_are_distinct_per_game_and_seat() {
        let g0 = GameSeeds::for_game(7, 0);
        let g1 = GameSeeds::for_game(7, 1);
        assert_ne!(g0.server, g1.server);
        // Per-seat streams within a game differ.
        let mut a = g0.bot_rng(0);
        let mut b = g0.bot_rng(1);
        let xa: u64 = a.next_u64();
        let xb: u64 = b.next_u64();
        assert_ne!(xa, xb);
    }

    /// The whole tree reproduces from the root (the reproducibility guarantee).
    #[test]
    fn whole_tree_reproduces_from_root() {
        let first = GameSeeds::for_game(123, 5);
        let again = GameSeeds::for_game(123, 5);
        assert_eq!(first, again);
        let mut r1 = first.bot_rng(2);
        let mut r2 = again.bot_rng(2);
        let s1: [u64; 4] = [r1.next_u64(), r1.next_u64(), r1.next_u64(), r1.next_u64()];
        let s2: [u64; 4] = [r2.next_u64(), r2.next_u64(), r2.next_u64(), r2.next_u64()];
        assert_eq!(s1, s2);
    }
}
