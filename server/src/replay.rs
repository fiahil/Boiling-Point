//! Replays: a compact, self-describing payload that re-runs the pinned engine to
//! reconstruct a completed game's public event stream, plus a timestamped log of
//! everything the players sent.
//!
//! The *reconstruction* part of a replay is the **deterministic input** to the
//! engine, not a recording of its output: the root seed plus the ordered per-wave
//! action log. Because the engine is fully deterministic from `seed` + content
//! config, re-running it reproduces the exact game — every deal, reveal, depile,
//! and score. Alongside it the body carries an **observational** `input_log`: the
//! raw in-game messages each player actually sent (commit/pass/lock-in/emote),
//! each stamped with ms-since-game-start, so playback can show emotes and pacing.
//! It is not fed back into the engine, so determinism is unaffected.
//!
//! The body is MessagePack-encoded into a single `BYTEA` database column, wrapped
//! by a `format_version`/`engine_version` tag (so a future engine change selects a
//! compatible reconstruction path or migrates the payload) and an integrity hash
//! over the encoded bytes (so tampering is rejected rather than mis-replayed).
//!
//! The canonical replay engine is the synchronous [`Game`] (the "synchronous
//! heart" the async room loop drives). The async `session::run_game` records the
//! same seed + action log on the live path; exact reconstruction parity of those
//! live logs converges with the two game loops (review-remediation **F2**) and
//! the Recall-target wire gap (tui review **T4**, now in
//! `docs/99_archive/tui-client-review.md`) — until then a live log
//! records what the wire carries.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use boiling_point_protocol::vocab::{Brewer, Color, Recipe, SpellTarget};
use boiling_point_protocol::{CardId, EmoteId, PlayerId};

use crate::config::ContentConfig;
use crate::content::ContentRegistry;
use crate::game::round::WaveChoice;
use crate::game::runner::Decider;
use crate::game::runner::{Game, GameOutcome, ReplayEvent};
use crate::game::state::{Hand, Player};
use crate::persistence::StoredReplay;

/// Replay payload format version. Bump on an incompatible payload *shape* change
/// (the wrapper around the action log); a decode of a newer version is rejected.
///
/// v2: the body gained the observational `input_log` (timestamped raw inputs) and
/// the payload column moved from base64 `TEXT` to raw `BYTEA`.
/// v3: the boom2 combat core — `WaveChoice` became ingredient-or-pass + optional
/// spell, and the recorded inputs gained `CommitIngredient`/`CastSpell`.
/// v4: the Brewers (`boom2-brewers`) — the body gained the table's brewer
/// assignments (a reconstruction input: decks and bends key off them),
/// `WaveChoice` gained the Channeler's `second_spell`, and the recorded inputs
/// gained `PickBrewer`/`CommitDefer`.
/// v5: the Apothecary (`boom2-apothecary`) — the body gained the table's
/// recipes (a reconstruction input: deck realization keys off them) and the
/// recorded inputs gained `SubmitRecipe`.
pub const REPLAY_FORMAT_VERSION: u16 = 5;

/// Engine version the payload was recorded under. Bump when an engine change
/// alters deterministic reconstruction; stored payloads then select a migration
/// / re-render path keyed off this tag rather than being lost.
///
/// v2: the boom2 combat-core engine (pantries/grimoire, detonator-only explosion).
/// v3: the Brewer bends (`boom2-brewers`) — per-Brewer rule hooks alter dealing,
/// liability, and scoring.
/// v4: the Apothecary realizer (`boom2-apothecary`) — decks are realized from
/// recipes (recipeless seats keep the fixed deal, so pre-draft action logs
/// still reconstruct under this engine given an empty recipe set).
pub const ENGINE_VERSION: u16 = 4;

/// A seated player as the replay must remember them to rebuild the roster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ReplayPlayer {
    /// Stable player id.
    id: PlayerId,
    /// Assigned colour (seat).
    color: Color,
    /// Display name.
    display_name: String,
}

/// A raw in-game input exactly as a player sent it on the wire, independent of
/// whether the engine ultimately accepted it. This is the observational record —
/// what was *sent* — not the effective `WaveChoice` the engine resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordedInput {
    /// The player committed a specific ingredient.
    CommitIngredient {
        /// The hand ingredient committed.
        card: CardId,
        /// Whether it was played colorless.
        colorless: bool,
    },
    /// The player cast a spell.
    CastSpell {
        /// The grimoire spell cast.
        spell: CardId,
        /// The chosen target, if any.
        target: Option<SpellTarget>,
    },
    /// The player committed to passing the wave.
    CommitPass,
    /// The player (a Lurker) deferred their commit past the wave reveal.
    CommitDefer,
    /// The player picked a Brewer from their dealt pre-game pair.
    PickBrewer {
        /// The Brewer picked.
        brewer: Brewer,
    },
    /// The player submitted their Apothecary recipe.
    SubmitRecipe {
        /// The recipe submitted.
        recipe: Recipe,
    },
    /// The player locked in their current selection.
    LockIn,
    /// The player sent a table-talk emote.
    Emote {
        /// The palette emote sent.
        emote: EmoteId,
    },
}

/// One recorded input with its author and a millisecond offset from game start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedInput {
    /// Who sent it.
    pub player: PlayerId,
    /// Milliseconds since the game started (monotonic).
    pub at_ms: u32,
    /// What they sent.
    pub input: RecordedInput,
}

/// The decoded replay body: everything needed to re-run the pinned engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ReplayBody {
    /// Payload format version (the wrapper shape).
    format_version: u16,
    /// Engine version the game was played under.
    engine_version: u16,
    /// Content-config identity the game ran against.
    config_fingerprint: String,
    /// The root seed driving all deterministic RNG.
    seed: u64,
    /// The seated roster, in seating order.
    players: Vec<ReplayPlayer>,
    /// Each player's picked Brewer — a reconstruction input (deck building and
    /// the in-round bends key off it). Empty for brewerless games (and absent,
    /// hence empty, in pre-v4 payloads).
    #[serde(default)]
    brewers: Vec<(PlayerId, Brewer)>,
    /// Each player's drafted recipe — a reconstruction input (deck realization
    /// keys off it; `boom2-apothecary`). Empty for fixed-deck games (and
    /// absent, hence empty, in pre-v5 payloads).
    #[serde(default)]
    recipes: Vec<(PlayerId, Recipe)>,
    /// Ordered per-wave player actions in engine decision order — the
    /// deterministic input the reconstruction re-runs.
    action_log: Vec<WaveChoice>,
    /// Every raw in-game input the players sent, in arrival order, each stamped
    /// with ms-since-game-start. Observational only (not fed to the engine).
    input_log: Vec<TimedInput>,
}

/// A reconstructed game: its public event stream plus the recomputed outcome.
#[derive(Debug, Clone)]
pub struct Reconstruction {
    /// The public event stream (deals, wave reveals, depile, scores, game over).
    pub events: Vec<ReplayEvent>,
    /// The recomputed final outcome (scores, winners).
    pub outcome: GameOutcome,
}

/// Why a stored replay could not be decoded or reconstructed.
#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    /// The payload bytes do not match the stored integrity hash.
    #[error("replay integrity check failed: payload does not match its hash")]
    Integrity,
    /// The payload's format version is not understood by this build.
    #[error(
        "unsupported replay format version {found} (this build understands {REPLAY_FORMAT_VERSION})"
    )]
    UnsupportedFormat {
        /// The version found in the payload.
        found: u16,
    },
    /// The MessagePack body failed to decode.
    #[error("replay messagepack decode failed: {0}")]
    Decode(#[from] rmp_serde::decode::Error),
    /// The MessagePack body failed to encode.
    #[error("replay messagepack encode failed: {0}")]
    Encode(#[from] rmp_serde::encode::Error),
}

/// A hex SHA-256 over `bytes` — the replay integrity hash.
fn hash_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// A stable fingerprint of the content config a game ran against. Two configs
/// that would deal/score differently produce different fingerprints, so a replay
/// can detect a content mismatch. (Derived from the serialized config; the first
/// 16 hex chars of its SHA-256 are ample to distinguish content revisions.)
pub fn config_fingerprint(config: &ContentConfig) -> String {
    let json = serde_json::to_vec(config).unwrap_or_default();
    hash_hex(&json)[..16].to_string()
}

/// Assemble and encode a completed game's replay into its storable row. The
/// `seed` and `action_log` come from the played game (e.g. [`GameOutcome`]);
/// `players` is the seated roster in seating order; `brewers` is the table's
/// picked identities (empty for a brewerless game); `recipes` is the table's
/// drafted recipes (empty for a fixed-deck game); `input_log` is the
/// session-recorded raw inputs (commit/pass/lock-in/emote) with timestamps.
#[allow(clippy::too_many_arguments)]
pub fn encode_replay(
    game_id: Uuid,
    seed: u64,
    config: &ContentConfig,
    players: impl IntoIterator<Item = (PlayerId, Color, String)>,
    brewers: impl IntoIterator<Item = (PlayerId, Brewer)>,
    recipes: impl IntoIterator<Item = (PlayerId, Recipe)>,
    action_log: &[WaveChoice],
    input_log: &[TimedInput],
) -> Result<StoredReplay, ReplayError> {
    let body = ReplayBody {
        format_version: REPLAY_FORMAT_VERSION,
        engine_version: ENGINE_VERSION,
        config_fingerprint: config_fingerprint(config),
        seed,
        players: players
            .into_iter()
            .map(|(id, color, display_name)| ReplayPlayer {
                id,
                color,
                display_name,
            })
            .collect(),
        brewers: brewers.into_iter().collect(),
        recipes: recipes.into_iter().collect(),
        action_log: action_log.to_vec(),
        input_log: input_log.to_vec(),
    };
    let payload = rmp_serde::to_vec_named(&body)?;
    let integrity_hash = hash_hex(&payload);
    Ok(StoredReplay {
        game_id,
        payload,
        format_version: body.format_version as i16,
        engine_version: body.engine_version as i16,
        config_fingerprint: body.config_fingerprint,
        integrity_hash,
    })
}

/// Decode and verify a stored replay: check the payload's integrity hash *before*
/// trusting the bytes, then decode and gate on the format version.
fn decode_and_verify(stored: &StoredReplay) -> Result<ReplayBody, ReplayError> {
    if hash_hex(&stored.payload) != stored.integrity_hash {
        return Err(ReplayError::Integrity);
    }
    let body: ReplayBody = rmp_serde::from_slice(&stored.payload)?;
    if body.format_version != REPLAY_FORMAT_VERSION {
        return Err(ReplayError::UnsupportedFormat {
            found: body.format_version,
        });
    }
    Ok(body)
}

/// Reconstruct a stored replay by re-running the pinned engine from its payload,
/// producing the game's public event stream (and recomputed outcome). Verifies
/// the integrity hash first.
pub fn reconstruct(
    stored: &StoredReplay,
    registry: &ContentRegistry,
    config: &ContentConfig,
) -> Result<Reconstruction, ReplayError> {
    let body = decode_and_verify(stored)?;
    let players: Vec<Player> = body
        .players
        .iter()
        .map(|p| Player {
            id: p.id,
            color: p.color,
            display_name: p.display_name.clone(),
        })
        .collect();
    let mut game = Game::with_recipes(
        registry,
        config,
        players,
        body.brewers.iter().copied().collect(),
        body.recipes.iter().cloned().collect(),
        body.seed,
    );
    let mut decider = ReplayDecider::new(body.action_log);
    let (outcome, events) = game.play_out_with_events(&mut decider);
    Ok(Reconstruction { events, outcome })
}

/// Replays a recorded action log into the engine: returns each recorded choice
/// in decision order. A short log (e.g. a truncated recording) falls back to
/// `Pass`, which the engine treats as a safe lockout.
struct ReplayDecider {
    actions: std::vec::IntoIter<WaveChoice>,
}

impl ReplayDecider {
    fn new(log: Vec<WaveChoice>) -> Self {
        Self {
            actions: log.into_iter(),
        }
    }
}

impl Decider for ReplayDecider {
    fn decide(&mut self, _player: PlayerId, _hand: &Hand) -> WaveChoice {
        self.actions.next().unwrap_or_else(WaveChoice::pass)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ContentConfig;
    use crate::content::ContentRegistry;
    use crate::game::round::WaveChoice;
    use crate::game::state::Hand;
    use boiling_point_protocol::vocab::Color;
    use uuid::Uuid;

    fn registry_and_config() -> (ContentRegistry, ContentConfig) {
        let cfg = ContentConfig::from_toml(include_str!("../content.toml")).unwrap();
        let reg = cfg.build_registry().unwrap();
        (reg, cfg)
    }

    fn four_players() -> Vec<Player> {
        Color::PLAYER_COLORS
            .into_iter()
            .enumerate()
            .map(|(i, color)| Player {
                id: PlayerId(Uuid::from_u128(i as u128 + 1)),
                color,
                display_name: format!("p{i}"),
            })
            .collect()
    }

    /// An eager decider: play the first ingredient in hand as a Vote, else pass.
    fn eager() -> impl FnMut(PlayerId, &Hand) -> WaveChoice {
        |_player, hand| match hand.ingredients().first() {
            Some(first) => WaveChoice {
                action: crate::game::round::WaveAction::Play {
                    card: first.id,
                    colorless: false,
                },
                spell: None,
                second_spell: None,
            },
            None => WaveChoice::pass(),
        }
    }

    /// Record a played game, then re-run it from the encoded payload and assert
    /// an identical public event stream, scores, and winners (guards engine
    /// determinism end to end: record → encode → decode → re-run).
    #[test]
    fn replay_round_trips_to_an_identical_event_stream() {
        let (reg, cfg) = registry_and_config();
        let roster = four_players();

        // Play the original game, capturing its public event stream.
        let mut original = Game::new(&reg, &cfg, roster.clone(), 0xC0FFEE);
        let mut decider = eager();
        let (outcome, events) = original.play_out_with_events(&mut decider);

        // Encode the replay from the recorded seed + action log.
        let stored = encode_replay(
            Uuid::new_v4(),
            outcome.seed,
            &cfg,
            roster
                .iter()
                .map(|p| (p.id, p.color, p.display_name.clone())),
            std::iter::empty(),
            std::iter::empty(),
            &outcome.action_log,
            &[],
        )
        .expect("encode");

        // Reconstruct and compare.
        let recon = reconstruct(&stored, &reg, &cfg).expect("reconstruct");
        assert_eq!(recon.events, events, "reconstructed event stream differs");
        assert_eq!(recon.outcome.scores, outcome.scores, "scores differ");
        assert_eq!(recon.outcome.winners, outcome.winners, "winners differ");
        // The payload fits a single column (one MessagePack blob).
        assert!(!stored.payload.is_empty());
    }

    /// The observational `input_log` round-trips inside the payload and does not
    /// disturb deterministic reconstruction.
    #[test]
    fn input_log_round_trips_in_the_payload() {
        let (reg, cfg) = registry_and_config();
        let roster = four_players();
        let mut game = Game::new(&reg, &cfg, roster.clone(), 0xBEEF);
        let mut decider = eager();
        let (outcome, events) = game.play_out_with_events(&mut decider);

        let inputs = vec![
            TimedInput {
                player: roster[0].id,
                at_ms: 12,
                input: RecordedInput::CommitIngredient {
                    card: CardId(1),
                    colorless: true,
                },
            },
            TimedInput {
                player: roster[1].id,
                at_ms: 340,
                input: RecordedInput::CastSpell {
                    spell: CardId(9),
                    target: Some(SpellTarget::Player {
                        player: roster[0].id,
                    }),
                },
            },
            TimedInput {
                player: roster[2].id,
                at_ms: 980,
                input: RecordedInput::CommitPass,
            },
        ];
        let stored = encode_replay(
            Uuid::new_v4(),
            outcome.seed,
            &cfg,
            roster
                .iter()
                .map(|p| (p.id, p.color, p.display_name.clone())),
            std::iter::empty(),
            std::iter::empty(),
            &outcome.action_log,
            &inputs,
        )
        .expect("encode");

        // The recorded inputs survive a decode...
        let body = decode_and_verify(&stored).expect("decode");
        assert_eq!(body.input_log, inputs, "input log differs after round-trip");
        // ...and the event stream is still reconstructed identically.
        let recon = reconstruct(&stored, &reg, &cfg).expect("reconstruct");
        assert_eq!(recon.events, events, "reconstructed event stream differs");
    }

    /// Brewer assignments are a reconstruction input: a game whose brewers
    /// bend dealing (Forager) and the grimoire (Cinderwright) re-runs to the
    /// identical stream only because the payload carries them.
    #[test]
    fn replay_with_brewers_round_trips() {
        let (reg, cfg) = registry_and_config();
        let roster = four_players();
        let brewers: std::collections::HashMap<PlayerId, Brewer> = [
            (roster[0].id, Brewer::Forager),
            (roster[1].id, Brewer::Cinderwright),
            (roster[2].id, Brewer::Featherhand),
            (roster[3].id, Brewer::Broker),
        ]
        .into();

        let mut original = Game::with_brewers(&reg, &cfg, roster.clone(), brewers.clone(), 0xB4E3);
        let mut decider = eager();
        let (outcome, events) = original.play_out_with_events(&mut decider);

        let stored = encode_replay(
            Uuid::new_v4(),
            outcome.seed,
            &cfg,
            roster
                .iter()
                .map(|p| (p.id, p.color, p.display_name.clone())),
            brewers.iter().map(|(p, b)| (*p, *b)),
            std::iter::empty(),
            &outcome.action_log,
            &[],
        )
        .expect("encode");
        let recon = reconstruct(&stored, &reg, &cfg).expect("reconstruct");
        assert_eq!(recon.events, events, "reconstructed event stream differs");
        assert_eq!(recon.outcome.scores, outcome.scores);

        // Dropping the brewers from the payload yields a DIFFERENT game (the
        // bends are load-bearing) — guarding against a silent omission.
        let brewerless = encode_replay(
            Uuid::new_v4(),
            outcome.seed,
            &cfg,
            roster
                .iter()
                .map(|p| (p.id, p.color, p.display_name.clone())),
            std::iter::empty(),
            std::iter::empty(),
            &outcome.action_log,
            &[],
        )
        .expect("encode");
        let recon = reconstruct(&brewerless, &reg, &cfg).expect("reconstruct");
        assert_ne!(
            recon.events, events,
            "brewer bends must be part of deterministic reconstruction"
        );
    }

    /// Recipes are a reconstruction input (`boom2-apothecary`): a game whose
    /// decks were realized from recipes re-runs to the identical stream only
    /// because the payload carries them — dropping them yields a different
    /// (fixed-deck) game.
    #[test]
    fn replay_with_recipes_round_trips() {
        use boiling_point_protocol::vocab::{GrimoireBucket, PantryBucket, SpellKind};
        let (reg, cfg) = registry_and_config();
        let roster = four_players();
        let recipe = |pantry: &[PantryBucket], grimoire: &[GrimoireBucket]| Recipe {
            pantry: pantry.to_vec(),
            grimoire: grimoire.to_vec(),
            reserves: vec![],
        };
        let recipes: std::collections::HashMap<PlayerId, Recipe> = [
            (
                roster[0].id,
                recipe(
                    &[PantryBucket::Nightshade, PantryBucket::Saffron],
                    &[GrimoireBucket::Ironbark, GrimoireBucket::Brimstone],
                ),
            ),
            (
                roster[1].id,
                Recipe {
                    pantry: vec![PantryBucket::Sage, PantryBucket::Hellebore],
                    grimoire: vec![GrimoireBucket::Eyebright, GrimoireBucket::Hoarfrost],
                    reserves: vec![SpellKind::Peek],
                },
            ),
            (
                roster[2].id,
                recipe(
                    &[PantryBucket::Ochre, PantryBucket::Wisp, PantryBucket::Mint],
                    &[GrimoireBucket::Farsight, GrimoireBucket::Wormwood],
                ),
            ),
            (
                roster[3].id,
                recipe(
                    &[PantryBucket::Honey, PantryBucket::Bramble],
                    &[GrimoireBucket::Goldenseal, GrimoireBucket::Mandrake],
                ),
            ),
        ]
        .into();

        let mut original = Game::with_recipes(
            &reg,
            &cfg,
            roster.clone(),
            std::collections::HashMap::new(),
            recipes.clone(),
            0xA90C,
        );
        let mut decider = eager();
        let (outcome, events) = original.play_out_with_events(&mut decider);

        let stored = encode_replay(
            Uuid::new_v4(),
            outcome.seed,
            &cfg,
            roster
                .iter()
                .map(|p| (p.id, p.color, p.display_name.clone())),
            std::iter::empty(),
            recipes.iter().map(|(p, r)| (*p, r.clone())),
            &outcome.action_log,
            &[],
        )
        .expect("encode");
        let recon = reconstruct(&stored, &reg, &cfg).expect("reconstruct");
        assert_eq!(recon.events, events, "reconstructed event stream differs");
        assert_eq!(recon.outcome.scores, outcome.scores);

        // Dropping the recipes yields a DIFFERENT (fixed-deck) game — guarding
        // against a silent omission of the realization input.
        let recipeless = encode_replay(
            Uuid::new_v4(),
            outcome.seed,
            &cfg,
            roster
                .iter()
                .map(|p| (p.id, p.color, p.display_name.clone())),
            std::iter::empty(),
            std::iter::empty(),
            &outcome.action_log,
            &[],
        )
        .expect("encode");
        let recon = reconstruct(&recipeless, &reg, &cfg).expect("reconstruct");
        assert_ne!(
            recon.events, events,
            "deck realization must be part of deterministic reconstruction"
        );
    }

    /// A payload whose bytes have been tampered with fails the integrity check
    /// before any reconstruction, rather than producing a wrong replay.
    #[test]
    fn tampered_payload_is_rejected() {
        let (reg, cfg) = registry_and_config();
        let roster = four_players();
        let mut game = Game::new(&reg, &cfg, roster.clone(), 42);
        let mut decider = eager();
        let outcome = game.play_out(&mut decider);

        let mut stored = encode_replay(
            Uuid::new_v4(),
            outcome.seed,
            &cfg,
            roster
                .iter()
                .map(|p| (p.id, p.color, p.display_name.clone())),
            std::iter::empty(),
            std::iter::empty(),
            &outcome.action_log,
            &[],
        )
        .expect("encode");

        // Tamper with the encoded bytes (flip one byte) while keeping the original
        // hash: reconstruction must refuse rather than mis-replay.
        stored.payload[0] ^= 0xFF;

        match reconstruct(&stored, &reg, &cfg) {
            Err(ReplayError::Integrity) => {}
            other => panic!("expected an integrity error, got {other:?}"),
        }
    }

    /// The fingerprint changes when the content config changes (so a replay can
    /// tell it was recorded against different content).
    #[test]
    fn config_fingerprint_tracks_content() {
        let (_, cfg) = registry_and_config();
        let base = config_fingerprint(&cfg);
        let mut altered = cfg.clone();
        altered.boiling_point.max += 1;
        assert_ne!(base, config_fingerprint(&altered));
        // Stable for the same config.
        assert_eq!(base, config_fingerprint(&cfg));
    }
}
