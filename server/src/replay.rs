//! Timeless replays: a compact, self-describing payload that re-runs the pinned
//! engine to reconstruct a completed game's public event stream.
//!
//! A replay is the *deterministic input* to the engine, not a recording of its
//! output: the root seed plus the ordered per-wave action log. Because the
//! engine is fully deterministic from `seed` + content config, re-running it
//! reproduces the exact game — every deal, reveal, depile, and score. The body
//! is MessagePack-encoded and base64'd into a single database column, wrapped by
//! a `format_version`/`engine_version` tag (so a future engine change selects a
//! compatible reconstruction path or migrates the payload) and an integrity hash
//! over the encoded bytes (so tampering is rejected rather than mis-replayed).
//!
//! The canonical replay engine is the synchronous [`Game`] (the "synchronous
//! heart" the async room loop drives). The async `session::run_game` records the
//! same seed + action log on the live path; exact reconstruction parity of those
//! live logs converges with the two game loops (review-remediation **F2**) and
//! the Recall-target wire gap (tui review **T4**) — until then a live log
//! records what the wire carries.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use boiling_point_protocol::PlayerId;
use boiling_point_protocol::vocab::Color;

use crate::config::ContentConfig;
use crate::content::ContentRegistry;
use crate::game::round::WaveChoice;
use crate::game::runner::Decider;
use crate::game::runner::{Game, GameOutcome, ReplayEvent};
use crate::game::state::{Hand, Player};
use crate::persistence::StoredReplay;

/// Replay payload format version. Bump on an incompatible payload *shape* change
/// (the wrapper around the action log); a decode of a newer version is rejected.
pub const REPLAY_FORMAT_VERSION: u16 = 1;

/// Engine version the payload was recorded under. Bump when an engine change
/// alters deterministic reconstruction; stored payloads then select a migration
/// / re-render path keyed off this tag rather than being lost.
pub const ENGINE_VERSION: u16 = 1;

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
    /// Ordered per-wave player actions in engine decision order.
    action_log: Vec<WaveChoice>,
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
    /// The base64 column value failed to decode.
    #[error("replay base64 decode failed: {0}")]
    Base64(#[from] base64::DecodeError),
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
/// `players` is the seated roster in seating order.
pub fn encode_replay(
    game_id: Uuid,
    seed: u64,
    config: &ContentConfig,
    players: impl IntoIterator<Item = (PlayerId, Color, String)>,
    action_log: &[WaveChoice],
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
        action_log: action_log.to_vec(),
    };
    let bytes = rmp_serde::to_vec_named(&body)?;
    let integrity_hash = hash_hex(&bytes);
    Ok(StoredReplay {
        game_id,
        payload: BASE64.encode(&bytes),
        format_version: body.format_version as i16,
        engine_version: body.engine_version as i16,
        config_fingerprint: body.config_fingerprint,
        integrity_hash,
    })
}

/// Decode and verify a stored replay: base64-decode the payload, check its
/// integrity hash *before* trusting the bytes, then decode and gate on the
/// format version.
fn decode_and_verify(stored: &StoredReplay) -> Result<ReplayBody, ReplayError> {
    let bytes = BASE64.decode(&stored.payload)?;
    if hash_hex(&bytes) != stored.integrity_hash {
        return Err(ReplayError::Integrity);
    }
    let body: ReplayBody = rmp_serde::from_slice(&bytes)?;
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
    let mut game = Game::new(registry, config, players, body.seed);
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
        self.actions.next().unwrap_or(WaveChoice::Pass)
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

    /// An eager decider: play the first card in hand, else pass.
    fn eager() -> impl FnMut(PlayerId, &Hand) -> WaveChoice {
        |_player, hand| {
            hand.views()
                .first()
                .map(|c| WaveChoice::Play(c.id))
                .unwrap_or(WaveChoice::Pass)
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
            &outcome.action_log,
        )
        .expect("encode");

        // Reconstruct and compare.
        let recon = reconstruct(&stored, &reg, &cfg).expect("reconstruct");
        assert_eq!(recon.events, events, "reconstructed event stream differs");
        assert_eq!(recon.outcome.scores, outcome.scores, "scores differ");
        assert_eq!(recon.outcome.winners, outcome.winners, "winners differ");
        // The payload fits a single column (it is just one base64 string).
        assert!(!stored.payload.is_empty());
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
            &outcome.action_log,
        )
        .expect("encode");

        // Tamper with the encoded bytes (flip one byte, re-base64), keeping the
        // original hash: reconstruction must refuse rather than mis-replay.
        let mut bytes = BASE64.decode(&stored.payload).unwrap();
        bytes[0] ^= 0xFF;
        stored.payload = BASE64.encode(&bytes);

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
