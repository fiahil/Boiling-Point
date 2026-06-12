//! The versioned span-schema contract: the single source of truth for the span
//! names the server emits, their hierarchy, and their attribute keys.
//!
//! The admin span projection reads these names and attribute keys to drive its
//! open-span registry and the privileged reveal. Some attributes carry **sensitive
//! game state** ([`SENSITIVE_ATTRS`]: boiling point, pantry/spell hands, committed
//! wave plays, mid-round pot volatility, active spell effects, deck seed); these
//! ride in spans and may reach the trusted, operator-only trace backend, but the
//! trust boundary that matters is the **player wire**, which never carries them
//! (the admin channel is a separate transport). There is therefore no export-time
//! redaction.
//!
//! The instrumentation call sites (`info_span!("round", round.number = …)`) must
//! use field names that match these constants; the lifecycle-consumer tests assert
//! that the emitted keys line up with the schema, catching drift.
//!
//! The human companion to this module is
//! `docs/03_architecture/04_span-schema-contract.md`.

/// Schema version. Bump on any breaking change to names/attributes so consumers
/// can detect a mismatch; additive growth (new spans/attributes, including the
/// documented-as-[`planned`] pre-game spans) does **not** bump it — the projection
/// ignores what it does not recognize.
///
/// v2: the boom2 combat-core rebase — `room`→`group` rename (`group.lifetime`
/// span, `group.code` attribute) and the v2 game subtree (waves carry `commit` /
/// `spell.cast` leaves; rounds end with the `depile` boiling-point reveal; the
/// v1-only `round.exploded` / `dominant_color` attributes are retired).
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
    /// The pre-game Brewer pick phase (`boom2-brewers`): the dealt disjoint
    /// pairs and the table's picks, both public. Landed additively, no schema
    /// bump (it was documented as planned). Child of [`GAME`].
    pub const BREWER_PICK: &str = "brewer.pick";
    /// One round within a game. Child of [`GAME`].
    pub const ROUND: &str = "round";
    /// One wave (commit window) within a round. Child of [`ROUND`].
    pub const WAVE: &str = "wave";
    /// A seated player's pantry + spell hands for a round. Held open for the round
    /// so the reveal can read both hands from a live span. Child of [`ROUND`].
    pub const HAND: &str = "hand";
    /// A single player's ingredient-or-pass commit within a wave, with its Vote
    /// colour. Open from the moment the (hidden) commit is accepted until the wave
    /// resolves, so the reveal can show committed-but-unrevealed plays. Child of
    /// [`WAVE`].
    pub const COMMIT: &str = "commit";
    /// A wave's optional spell cast (visible Instant activations; primed Actives
    /// stay silent until they fire). Child of [`WAVE`].
    pub const SPELL_CAST: &str = "spell.cast";
    /// Resolution of a wave through the engine. The round-ending (fatal) wave's
    /// resolve span additionally carries the pot value P and the detonator split.
    /// Child of [`WAVE`].
    pub const RESOLVE: &str = "resolve";
    /// The end-of-round volatility-sorted reveal (the fuse climb) — the boiling
    /// point is revealed **every** round, boom and safe. Child of [`ROUND`].
    pub const DEPILE: &str = "depile";
    /// Scoring of a round (detonator split or safe-brew payout). Child of [`ROUND`].
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

/// Pre-game spans documented as **planned**: their content changes have not landed,
/// so the server does not emit them yet. They will land **additively, without a
/// schema bump** (the projection ignores unknown spans until then) — exactly as
/// `brewer.pick` did when `boom2-brewers` landed it (now in [`span`]).
pub mod planned {
    /// The pre-game Apothecary draft (buckets taken are public; the realized decks
    /// stay sensitive). Lands with `boom2-apothecary`. Child of `game`.
    pub const DRAFT: &str = "draft";
}

/// Stable attribute keys. Public keys may be exported; the keys in
/// [`SENSITIVE_ATTRS`] carry hidden game state and are only ever read in-process
/// (the privileged reveal) or by the trusted, operator-only trace backend.
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
    /// Ingredients committed in a wave (public — clients see who played).
    pub const WAVE_COMMITS: &str = "wave.commits";
    /// Players who passed/folded in a wave (public — clients see who passed).
    pub const WAVE_PASSES: &str = "wave.passes";
    /// Number of seated players.
    pub const PLAYERS_COUNT: &str = "players.count";
    /// The pre-game brewer deal: the four dealt pairs, in seat order (public —
    /// each player sees their own pair, and disjointness is the design).
    pub const BREWER_OFFERS: &str = "brewer.offers";
    /// The table's chosen Brewers, in seat order (public from the reveal).
    pub const BREWER_PICKS: &str = "brewer.picks";
    /// Whether a round boomed — the v2 detonator-only explosion (public after the
    /// depile).
    pub const ROUND_BOOMED: &str = "round.boomed";
    /// Whether a round froze (settled with an empty pot — everyone passed).
    pub const ROUND_FROZEN: &str = "round.frozen";
    /// Active cauldron modifiers for a round, comma-joined (public — clients see
    /// each `ModifierRevealed`).
    pub const MODIFIERS: &str = "modifiers";
    /// Cards in the cauldron (the public signal clients already see).
    pub const POT_CARD_COUNT: &str = "pot.card_count";
    /// The pot's scored value P (public at settlement — the payout or the split).
    pub const POT_VALUE: &str = "pot.value";
    /// The detonators who split −P, comma-joined in fatal-wave sort order (public
    /// in the Explosion message).
    pub const DETONATORS: &str = "detonators";
    /// The depile's reveal sequence in ascending effective-volatility order (public
    /// at the depile — every entry is broadcast).
    pub const REVEALS: &str = "reveals";
    /// Index where the depile's sorted climb crossed the boiling point (public,
    /// boom rounds only).
    pub const CROSSING_INDEX: &str = "crossing_index";
    /// A cast spell's kind (public — Instant activations are broadcast).
    pub const SPELL_KIND: &str = "spell.kind";
    /// A cast spell's target, where the wire carries one (public).
    pub const SPELL_TARGET: &str = "spell.target";
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
    /// The round's (post-modifier) boiling point. Revealed to players only at the
    /// depile; hidden from players in-flight (this key also rides publicly on the
    /// `depile` span, where the reveal has already happened).
    pub const BOILING_POINT: &str = "boiling_point";
    /// A committed card's identity before the depile reveals it.
    pub const COMMITTED_CARD: &str = "committed_card";
    /// A commit's Vote colour — the ingredient's colour, or `colorless` — hidden
    /// until the depile.
    pub const VOTE_COLOR: &str = "vote.color";
    /// A player's pantry (ingredient) hand contents.
    pub const HAND_PANTRY: &str = "hand.pantry";
    /// A player's spell (grimoire) hand contents.
    pub const HAND_SPELLS: &str = "hand.spells";
    /// Mid-round running cauldron volatility (hidden until the depile).
    pub const VOLATILITY_TOTAL: &str = "volatility_total";
    /// Active spell effects: primed Actives (hidden until they fire) and a pending
    /// Quench shield.
    pub const EFFECTS_ACTIVE: &str = "effects.active";
    /// The deck/game seed — derives the boiling points and the entire deck order.
    pub const DECK_SEED: &str = "deck_seed";
}

/// The attribute keys that carry sensitive game state — the single authoritative
/// list the privileged reveal reads from. Everything else is public.
pub const SENSITIVE_ATTRS: &[&str] = &[
    attr::BOILING_POINT,
    attr::COMMITTED_CARD,
    attr::VOTE_COLOR,
    attr::HAND_PANTRY,
    attr::HAND_SPELLS,
    attr::VOLATILITY_TOTAL,
    attr::EFFECTS_ACTIVE,
    attr::DECK_SEED,
];

/// Whether `key` is a sensitive attribute (admin reveal only).
pub fn is_sensitive(key: &str) -> bool {
    SENSITIVE_ATTRS.contains(&key)
}

/// The span tree as `(span, parent)` pairs. A `None` parent marks a span that may
/// be a root (connection- or registry-scoped rather than nested under a group).
pub const SPAN_TREE: &[(&str, Option<&str>)] = &[
    (span::GROUP_LIFETIME, None),
    (span::LOBBY_WAIT, None),
    (span::GAME, Some(span::GROUP_LIFETIME)),
    (span::BREWER_PICK, Some(span::GAME)),
    (span::ROUND, Some(span::GAME)),
    (span::HAND, Some(span::ROUND)),
    (span::WAVE, Some(span::ROUND)),
    (span::COMMIT, Some(span::WAVE)),
    (span::SPELL_CAST, Some(span::WAVE)),
    (span::RESOLVE, Some(span::WAVE)),
    (span::DEPILE, Some(span::ROUND)),
    (span::SCORE, Some(span::ROUND)),
    (span::RECONNECT, Some(span::GAME)),
    (span::DB_WRITE, Some(span::GAME)),
    (span::WS_MESSAGE, None),
    (span::ADMIN_COMMAND, None),
];

/// The planned (not yet emitted) spans and their documented parents. Kept out of
/// [`SPAN_TREE`] so the projection treats them as unknown until they land.
pub const PLANNED_SPANS: &[(&str, Option<&str>)] = &[(planned::DRAFT, Some(span::GAME))];

/// Whether `name` is a span the schema knows about. The projection uses this to
/// ignore unrecognized spans gracefully (forward/backward tolerance) — including
/// the [`planned`] spans until their content changes land.
pub fn is_known_span(name: &str) -> bool {
    SPAN_TREE.iter().any(|(n, _)| *n == name)
}

/// Whether `name` is a documented-as-planned span (not yet emitted).
pub fn is_planned_span(name: &str) -> bool {
    PLANNED_SPANS.iter().any(|(n, _)| *n == name)
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
        for (name, parent) in SPAN_TREE.iter().chain(PLANNED_SPANS) {
            if let Some(p) = parent {
                assert!(
                    SPAN_TREE.iter().any(|(n, _)| n == p),
                    "span {name} names parent {p} which is not in the tree"
                );
            }
        }
    }

    /// The documented v2 core nesting holds: group → game → round → wave, with the
    /// wave's commit/spell-cast/resolve leaves and the round's depile/score.
    #[test]
    fn core_nesting_is_documented() {
        assert_eq!(parent_of(span::COMMIT), Some(span::WAVE));
        assert_eq!(parent_of(span::SPELL_CAST), Some(span::WAVE));
        assert_eq!(parent_of(span::RESOLVE), Some(span::WAVE));
        assert_eq!(parent_of(span::DEPILE), Some(span::ROUND));
        assert_eq!(parent_of(span::SCORE), Some(span::ROUND));
        assert_eq!(parent_of(span::WAVE), Some(span::ROUND));
        assert_eq!(parent_of(span::ROUND), Some(span::GAME));
        assert_eq!(parent_of(span::GAME), Some(span::GROUP_LIFETIME));
        assert_eq!(parent_of(span::GROUP_LIFETIME), None);
    }

    /// The version is exposed and is the v2 contract.
    #[test]
    fn schema_version_is_v2() {
        const { assert!(SPAN_SCHEMA_VERSION == 2) };
    }

    /// Planned pre-game spans are documented but not "known" — the projection's
    /// ignore-unknown tolerance covers them until their content changes land.
    #[test]
    fn planned_spans_are_documented_but_not_known() {
        for (name, _) in PLANNED_SPANS {
            assert!(is_planned_span(name), "{name} should be planned");
            assert!(
                !is_known_span(name),
                "{name} must stay unknown (ignored) until its change lands"
            );
        }
    }

    /// The sensitive-attribute markers cover the v2 hidden state the reveal reads:
    /// boiling point, both hands, committed plays with their Vote colour, pot
    /// volatility, active effects, and the deck seed.
    #[test]
    fn sensitive_markers_cover_the_v2_reveal() {
        for key in [
            attr::BOILING_POINT,
            attr::HAND_PANTRY,
            attr::HAND_SPELLS,
            attr::COMMITTED_CARD,
            attr::VOTE_COLOR,
            attr::VOLATILITY_TOTAL,
            attr::EFFECTS_ACTIVE,
            attr::DECK_SEED,
        ] {
            assert!(is_sensitive(key), "{key} must be marked sensitive");
        }
        // Public outcome attributes are not sensitive.
        for key in [
            attr::ROUND_BOOMED,
            attr::POT_VALUE,
            attr::DETONATORS,
            attr::REVEALS,
        ] {
            assert!(!is_sensitive(key), "{key} must be public");
        }
    }
}
