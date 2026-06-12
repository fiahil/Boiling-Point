//! The deterministic RNG tree (design D7): root seed → per-game → per-seat.
//!
//! A single root seed derives every stream in a batch: the server-side game
//! seed and each seat's decision stream. A `(root seed, seat configuration,
//! content config)` triple therefore fully reproduces a run. All derivation
//! goes through [`derive`], a pure `splitmix64` mix of the parent seed and a
//! salt; nothing reads the wall clock or the OS entropy pool. Bot brains draw
//! exclusively from a [`rand::rngs::StdRng`] seeded by this tree — the one
//! sanctioned randomness source on the decision path.

use rand::SeedableRng;
use rand::rngs::StdRng;

/// Salt mixed in to derive a single game's seed from the root.
const GAME_SALT: u64 = 0x6761_6D65_5345_4544; // "gameSEED"
/// Salt mixed in to derive the server-side game seed from a game's seed.
const SERVER_SALT: u64 = 0x7372_7672_5345_4544; // "srvrSEED"
/// Salt mixed in to derive a per-seat bot stream from a game's seed.
const SEAT_SALT: u64 = 0x7365_6174_5345_4544; // "seatSEED"

/// Mix a parent seed with a salt into a child seed (the `splitmix64`
/// finaliser): deterministic and well-distributed, so adjacent indices yield
/// uncorrelated child streams.
pub fn derive(parent: u64, salt: u64) -> u64 {
    let mut z = parent
        .wrapping_add(salt)
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The full seed set for a single game, derived from the root and game index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameSeeds {
    /// Seed for the server's engine (deck shuffles, boiling points, modifiers).
    pub server: u64,
    /// The per-game root the per-seat bot streams branch from.
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

    /// A fresh, independent decision RNG for the bot in seat `seat` (0-based).
    pub fn bot_rng(&self, seat: usize) -> StdRng {
        StdRng::seed_from_u64(derive(self.game, derive(SEAT_SALT, seat as u64)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    /// Derivation is a pure function and salt/seed sensitive.
    #[test]
    fn derive_is_deterministic_and_sensitive() {
        assert_eq!(derive(42, GAME_SALT), derive(42, GAME_SALT));
        assert_ne!(derive(42, GAME_SALT), derive(43, GAME_SALT));
        assert_ne!(derive(42, GAME_SALT), derive(42, SEAT_SALT));
    }

    /// Every game and every seat gets a distinct stream under one root, and
    /// the whole tree reproduces from the root.
    #[test]
    fn tree_is_distinct_and_reproducible() {
        let g0 = GameSeeds::for_game(7, 0);
        let g1 = GameSeeds::for_game(7, 1);
        assert_ne!(g0.server, g1.server);
        assert_ne!(g0.bot_rng(0).next_u64(), g0.bot_rng(1).next_u64());

        let again = GameSeeds::for_game(7, 0);
        assert_eq!(g0, again);
        let (mut a, mut b) = (g0.bot_rng(2), again.bot_rng(2));
        for _ in 0..8 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }
}
