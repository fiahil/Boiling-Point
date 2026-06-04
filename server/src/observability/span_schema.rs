//! The versioned span-schema contract: the single source of truth for the span
//! names the server emits, their hierarchy, and their attribute keys.
//!
//! The `admin-ui` span projection reads these names and attribute keys to drive its
//! open-span registry and the privileged reveal. Some attributes carry **sensitive
//! game state** (boiling point, hands, mid-round volatility, deck seed); these ride
//! in spans and may reach the trusted, operator-only trace backend, but the trust
//! boundary that matters is the **player wire**, which never carries them (the admin
//! channel is a separate transport). There is therefore no export-time redaction.
//!
//! The instrumentation call sites (`info_span!("round", round.number = …)`) must
//! use field names that match these constants; the lifecycle-consumer tests assert
//! that the emitted keys line up with the schema, catching drift.

/// Schema version. Bump on any breaking change to names/attributes so consumers
/// can detect a mismatch.
///
/// v2: room→group rename (`group.lifetime` span, `group.code` attribute).
pub const SPAN_SCHEMA_VERSION: u32 = 2;

/// Span names emitted by the server, as stable string constants.
pub mod span {
    /// A group's whole lifetime (lobby → game → teardown). Long-lived; the live
    /// open-span registry is keyed off this one.
    pub const GROUP_LIFETIME: &str = "group.lifetime";
    /// A player waiting in the auto-match queue. Open while they wait; the count of
    /// open `lobby.wait` spans is the live queue depth. Connection-scoped root.
    pub const LOBBY_WAIT: &str = "lobby.wait";
    /// One full game within a group. Child of [`GROUP_LIFETIME`].
    pub const GAME: &str = "game";
    /// One round within a game. Child of [`GAME`].
    pub const ROUND: &str = "round";
    /// One wave (commit window) within a round. Child of [`ROUND`].
    pub const WAVE: &str = "wave";
    /// A seated player's hand for a round. Held open for the round so the reveal
    /// can read it from a live span. Child of [`ROUND`].
    pub const HAND: &str = "hand";
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
    /// Group invite code.
    pub const GROUP_CODE: &str = "group.code";
    /// Game UUID.
    pub const GAME_ID: &str = "game.id";
    /// 1-based round number.
    pub const ROUND_NUMBER: &str = "round.number";
    /// 1-based wave number within a round.
    pub const WAVE_NUMBER: &str = "wave.number";
    /// Wave commit-window budget in milliseconds.
    pub const WAVE_TIMER_MS: &str = "wave.timer_ms";
    /// Whether a wave closed on its timer rather than everyone locking in.
    pub const WAVE_TIMED_OUT: &str = "wave.timed_out";
    /// Number of seated players.
    pub const PLAYERS_COUNT: &str = "players.count";
    /// Whether a round exploded (public after depile).
    pub const ROUND_EXPLODED: &str = "round.exploded";
    /// The colour that dominated a round's scoring, or `split`/`none` (public).
    pub const DOMINANT_COLOR: &str = "dominant_color";
    /// Active cauldron modifiers for a round, comma-joined (public — clients see
    /// each `ModifierRevealed`).
    pub const MODIFIERS: &str = "modifiers";
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
    /// The command target (e.g. a group code or item id).
    pub const TARGET: &str = "target";
    /// The command outcome (`ok` or a rejection reason).
    pub const OUTCOME: &str = "outcome";

    // --- sensitive game state (admin reveal only; never on the player wire) ---
    /// The round's (post-modifier) boiling point. Revealed to players only on
    /// explosion; hidden from players in-flight.
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

/// The span tree as `(span, parent)` pairs. A `None` parent marks a span that may
/// be a root (connection- or registry-scoped rather than nested under a group).
pub const SPAN_TREE: &[(&str, Option<&str>)] = &[
    (span::GROUP_LIFETIME, None),
    (span::LOBBY_WAIT, None),
    (span::GAME, Some(span::GROUP_LIFETIME)),
    (span::ROUND, Some(span::GAME)),
    (span::HAND, Some(span::ROUND)),
    (span::WAVE, Some(span::ROUND)),
    (span::RESOLVE, Some(span::WAVE)),
    (span::COMMIT, Some(span::WAVE)),
    (span::SCORE, Some(span::ROUND)),
    (span::RECONNECT, Some(span::GAME)),
    (span::DB_WRITE, Some(span::GAME)),
    (span::WS_MESSAGE, None),
    (span::ADMIN_COMMAND, None),
];

/// Whether `name` is a span the schema knows about. The projection uses this to
/// ignore unrecognized spans gracefully (forward/backward tolerance).
pub fn is_known_span(name: &str) -> bool {
    SPAN_TREE.iter().any(|(n, _)| *n == name)
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

    /// The documented core nesting group → game → round → wave holds.
    #[test]
    fn core_nesting_is_documented() {
        assert_eq!(parent_of(span::WAVE), Some(span::ROUND));
        assert_eq!(parent_of(span::ROUND), Some(span::GAME));
        assert_eq!(parent_of(span::GAME), Some(span::GROUP_LIFETIME));
        assert_eq!(parent_of(span::GROUP_LIFETIME), None);
    }

    /// The version is exposed and non-zero (checked at compile time).
    #[test]
    fn schema_version_is_exposed() {
        const { assert!(SPAN_SCHEMA_VERSION >= 1) };
    }
}
