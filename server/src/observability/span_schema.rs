//! The versioned span-schema contract: the single source of truth for the span
//! names the server emits, their hierarchy, the **public** attribute keys that may
//! leave the process, and the **secret** attribute keys that must never be
//! exported.
//!
//! Two consumers read from here so neither hard-codes strings:
//! - [`crate::observability::redact`] uses [`is_public`] as the export allow-list.
//! - the `admin-ui` span projection (a separate change) reads the names and the
//!   secret set to drive its open-span registry and the privileged reveal.
//!
//! The instrumentation call sites (`info_span!("round", round.number = …)`) must
//! use field names that match these constants; the lifecycle-consumer tests assert
//! that the emitted keys line up with the schema, catching drift.

/// Schema version. Bump on any breaking change to names/attributes so consumers
/// can detect a mismatch.
pub const SPAN_SCHEMA_VERSION: u32 = 1;

/// Span names emitted by the server, as stable string constants.
pub mod span {
    /// A room's whole lifetime (lobby → game → teardown). Long-lived; the live
    /// open-span registry is keyed off this one.
    pub const ROOM_LIFETIME: &str = "room.lifetime";
    /// One full game within a room. Child of [`ROOM_LIFETIME`].
    pub const GAME: &str = "game";
    /// One round within a game. Child of [`GAME`].
    pub const ROUND: &str = "round";
    /// One wave (commit window) within a round. Child of [`ROUND`].
    pub const WAVE: &str = "wave";
    /// Resolution of a wave through the engine. Child of [`WAVE`].
    pub const RESOLVE: &str = "resolve";
    /// A single player's commit within a wave. Child of [`WAVE`].
    pub const COMMIT: &str = "commit";
    /// Scoring (or explosion) of a round. Child of [`ROUND`].
    pub const SCORE: &str = "score";
    /// Handling of one inbound WebSocket message. Connection-scoped.
    pub const WS_MESSAGE: &str = "ws.message";
    /// A player reconnecting mid-game. Child of [`GAME`].
    pub const RECONNECT: &str = "reconnect";
    /// The single post-game persistence write. Child of [`GAME`].
    pub const DB_WRITE: &str = "db.write";
    /// An admin command-plane action (reload/toggle/seed/force-start/kill).
    pub const ADMIN_COMMAND: &str = "admin.command";
}

/// Stable attribute keys. Public keys are exportable; secret keys are stripped at
/// the export boundary and only ever read in-process.
pub mod attr {
    // --- public (exportable) ---
    /// Room invite code.
    pub const ROOM_CODE: &str = "room.code";
    /// Game UUID.
    pub const GAME_ID: &str = "game.id";
    /// 1-based round number.
    pub const ROUND_NUMBER: &str = "round.number";
    /// 1-based wave number within a round.
    pub const WAVE_NUMBER: &str = "wave.number";
    /// Wave commit-window budget in milliseconds.
    pub const WAVE_TIMER_MS: &str = "wave.timer_ms";
    /// Number of seated players.
    pub const PLAYERS_COUNT: &str = "players.count";
    /// Whether a round exploded (public after depile).
    pub const ROUND_EXPLODED: &str = "round.exploded";
    /// Cards in the cauldron (the public signal clients already see).
    pub const POT_CARD_COUNT: &str = "pot.card_count";
    /// Pot value at explosion (public in the Explosion message).
    pub const POT_VALUE: &str = "pot.value";
    /// A player's own id (already public on the wire).
    pub const PLAYER_ID: &str = "player.id";
    /// Inbound message kind (the variant name, never its secret payload).
    pub const WS_MESSAGE_KIND: &str = "ws.message_kind";
    /// Rows written by the persistence write.
    pub const DB_ROWS: &str = "db.rows";
    /// Schema version stamped on root spans.
    pub const SCHEMA_VERSION: &str = "schema.version";
    // admin command-plane (public audit fields)
    /// Operator identity issuing a command.
    pub const OPERATOR: &str = "operator";
    /// The command action (e.g. `reload`, `kill`).
    pub const ACTION: &str = "action";
    /// The command target (e.g. a room code or item id).
    pub const TARGET: &str = "target";
    /// The command outcome (`ok` or a rejection reason).
    pub const OUTCOME: &str = "outcome";

    // --- secret (never exported) ---
    /// The round's (post-modifier) boiling point. Revealed to players only on
    /// explosion; secret in-flight.
    pub const BOILING_POINT: &str = "boiling_point";
    /// A committed card's identity before public resolution.
    pub const COMMITTED_CARD: &str = "committed_card";
    /// A player's hand contents.
    pub const HAND: &str = "hand";
    /// Mid-round running cauldron volatility (hidden until depile).
    pub const VOLATILITY_TOTAL: &str = "volatility_total";
    /// The deck/game seed — derives the boiling point and the entire deck order.
    pub const DECK_SEED: &str = "deck_seed";
}

/// The export allow-list: attribute keys that may leave the process. Anything not
/// in this set is stripped at the export boundary (fail-closed).
pub const PUBLIC_ATTRS: &[&str] = &[
    attr::ROOM_CODE,
    attr::GAME_ID,
    attr::ROUND_NUMBER,
    attr::WAVE_NUMBER,
    attr::WAVE_TIMER_MS,
    attr::PLAYERS_COUNT,
    attr::ROUND_EXPLODED,
    attr::POT_CARD_COUNT,
    attr::POT_VALUE,
    attr::PLAYER_ID,
    attr::WS_MESSAGE_KIND,
    attr::DB_ROWS,
    attr::SCHEMA_VERSION,
    attr::OPERATOR,
    attr::ACTION,
    attr::TARGET,
    attr::OUTCOME,
];

/// The authoritative secret-attribute set: keys carried in spans in-process only
/// and guaranteed never to be exported. This is the one place that enumerates
/// what redaction (and the privileged reveal) treat as secret.
pub const SECRET_ATTRS: &[&str] = &[
    attr::BOILING_POINT,
    attr::COMMITTED_CARD,
    attr::HAND,
    attr::VOLATILITY_TOTAL,
    attr::DECK_SEED,
];

/// The span tree as `(span, parent)` pairs. A `None` parent marks a span that may
/// be a root (connection- or registry-scoped rather than nested under a room).
pub const SPAN_TREE: &[(&str, Option<&str>)] = &[
    (span::ROOM_LIFETIME, None),
    (span::GAME, Some(span::ROOM_LIFETIME)),
    (span::ROUND, Some(span::GAME)),
    (span::WAVE, Some(span::ROUND)),
    (span::RESOLVE, Some(span::WAVE)),
    (span::COMMIT, Some(span::WAVE)),
    (span::SCORE, Some(span::ROUND)),
    (span::RECONNECT, Some(span::GAME)),
    (span::DB_WRITE, Some(span::GAME)),
    (span::WS_MESSAGE, None),
    (span::ADMIN_COMMAND, None),
];

/// Whether `key` is on the public export allow-list.
pub fn is_public(key: &str) -> bool {
    PUBLIC_ATTRS.contains(&key)
}

/// Whether `key` is an enumerated secret attribute.
pub fn is_secret(key: &str) -> bool {
    SECRET_ATTRS.contains(&key)
}

/// The documented parent of `span`, if it nests under another span.
pub fn parent_of(span: &str) -> Option<&'static str> {
    SPAN_TREE
        .iter()
        .find(|(name, _)| *name == span)
        .and_then(|(_, parent)| *parent)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Public and secret attribute sets must be disjoint — no key can be both
    /// exportable and secret, or redaction's meaning is ambiguous.
    #[test]
    fn public_and_secret_sets_are_disjoint() {
        for s in SECRET_ATTRS {
            assert!(
                !is_public(s),
                "secret attr {s} must not be on the public allow-list"
            );
        }
        for p in PUBLIC_ATTRS {
            assert!(
                !is_secret(p),
                "public attr {p} must not be enumerated secret"
            );
        }
    }

    /// Every secret key reports secret and not public, and vice versa.
    #[test]
    fn membership_helpers_agree_with_the_sets() {
        assert!(is_secret(attr::BOILING_POINT));
        assert!(!is_public(attr::BOILING_POINT));
        assert!(is_public(attr::ROOM_CODE));
        assert!(!is_secret(attr::ROOM_CODE));
        assert!(!is_public("code.filepath")); // infra attr is not allow-listed
    }

    /// The hierarchy is internally consistent: every named parent is itself a span
    /// in the tree.
    #[test]
    fn every_parent_is_a_known_span() {
        for (name, parent) in SPAN_TREE {
            if let Some(p) = parent {
                assert!(
                    SPAN_TREE.iter().any(|(n, _)| n == p),
                    "span {name} names parent {p} which is not in the tree"
                );
            }
        }
    }

    /// The documented core nesting room → game → round → wave holds.
    #[test]
    fn core_nesting_is_documented() {
        assert_eq!(parent_of(span::WAVE), Some(span::ROUND));
        assert_eq!(parent_of(span::ROUND), Some(span::GAME));
        assert_eq!(parent_of(span::GAME), Some(span::ROOM_LIFETIME));
        assert_eq!(parent_of(span::ROOM_LIFETIME), None);
    }

    /// The version is exposed and non-zero (checked at compile time).
    #[test]
    fn schema_version_is_exposed() {
        const { assert!(SPAN_SCHEMA_VERSION >= 1) };
    }
}
