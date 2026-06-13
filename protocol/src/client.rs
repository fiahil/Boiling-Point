//! Messages sent from a client (or bot) to the server.
//!
//! All client messages are fire-and-forget: the server validates each one and
//! either applies it or replies with an [`crate::server::ServerMessage::Error`],
//! never partially.

use serde::{Deserialize, Serialize};

use crate::ids::{CardId, EmoteId, GroupCode};
use crate::vocab::{Brewer, Recipe, SpellTarget};

/// The protocol version a client speaks, sent on the first (entry) message so
/// the server can reject incompatible clients before sharing any state.
pub type ProtocolVersion = u16;

/// The current wire protocol version.
///
/// v2: room→group rename, plus the persistent session connection (`LeaveGroup` /
/// [`crate::server::ServerMessage::LeftGroup`], re-entry on one socket) and the
/// `PlayAgain` post-game opt-in.
/// v3: group matchmaking fill (`FillGroup`/`CancelFill`, `GroupSearching`), the
/// member/guest distinction (`PlayerPublic.guest`), and group standings
/// (`StandingsUpdate`).
/// v4: the boom2 combat core — the ingredient/spell card split
/// (`CommitIngredient` with the colorless Vote choice, `CastSpell`), the
/// detonator-only explosion, and the volatility-sorted depile that reveals the
/// boiling point every round.
/// v5: decision frames — the server enumerates each pending decision's complete
/// legal action set ([`crate::server::ServerMessage::DecisionFrame`]) and
/// rejects submissions against an already-resolved frame with
/// [`crate::server::ErrorCode::StaleFrame`].
/// v6: the Brewers (`boom2-brewers`) — the pre-game pick-1-of-2 phase (the
/// dealt pair rides a `BrewerPick` decision frame; the pick intent is
/// [`ClientMessage::PickBrewer`]; the table's public identities arrive in
/// [`crate::server::ServerMessage::BrewersRevealed`]), plus the Lurker's
/// once-per-round deferred commit ([`ClientMessage::CommitDefer`] against a
/// frame whose `can_defer` is set).
/// v7: the Apothecary (`boom2-apothecary`) — the pre-game two-ledger draft
/// after the Brewer pick (the bucket rosters and allowances ride an
/// `ApothecaryDraft` decision frame; the intent is
/// [`ClientMessage::SubmitRecipe`]; the table's public recipes arrive in
/// [`crate::server::ServerMessage::RecipesRevealed`] and on the snapshot).
/// Decks are realized server-side from the recipes and stay hidden, owner
/// included.
/// v8: compounding (`boom2-compounding`) — ingredients gain an optional
/// [`crate::vocab::Compounding`] tag (count-threshold / named-combo half) on
/// [`crate::vocab::IngredientView`], and the depile narrates which combos and
/// thresholds fired via [`crate::server::DepileEntry::compounding`].
pub const PROTOCOL_VERSION: ProtocolVersion = 8;

/// A message from client to server. Enum-tagged so a JSON fallback stays
/// human-readable for debugging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Join an existing group by its invite code (entry message; carries the version).
    JoinGroup {
        /// Protocol version the client speaks.
        protocol_version: ProtocolVersion,
        /// Display name to use at the table.
        display_name: String,
        /// Prior session token, to resume an existing identity if presented.
        session_token: Option<String>,
        /// The invite code of the group to join.
        group_code: GroupCode,
    },
    /// Create a fresh group and receive its invite code (entry message).
    CreateGroup {
        /// Protocol version the client speaks.
        protocol_version: ProtocolVersion,
        /// Display name to use at the table.
        display_name: String,
        /// Prior session token, to resume an existing identity if presented.
        session_token: Option<String>,
    },
    /// Enter the auto-match queue to be assembled into a table of four (entry message).
    EnqueueMatch {
        /// Protocol version the client speaks.
        protocol_version: ProtocolVersion,
        /// Display name to use at the table.
        display_name: String,
        /// Prior session token, to resume an existing identity if presented.
        session_token: Option<String>,
    },
    /// Commit an ingredient into the current wave (hidden until the wave reveals).
    /// Playing keeps the player active; the ingredient-or-pass choice is mandatory
    /// each wave.
    CommitIngredient {
        /// The hand ingredient to commit.
        card: CardId,
        /// Play it colorless (a wild / go-neutral push): its volatility still
        /// enters the cauldron but it scores **zero** points and serves no colour.
        colorless: bool,
    },
    /// Commit to passing this wave (permanent lockout for the round).
    CommitPass,
    /// Defer this wave's commit until after the wave reveals (the Lurker's
    /// once-per-round bend; legal only against a frame offering `can_defer`).
    /// The late commit that follows the reveal is an ingredient-or-pass only —
    /// no spell rides it.
    CommitDefer,
    /// Pick one Brewer from the dealt pre-game pair (answers the
    /// [`crate::frame::PendingDecision::BrewerPick`] frame; final on receipt).
    PickBrewer {
        /// The chosen Brewer (must be one of the frame's two options).
        brewer: Brewer,
    },
    /// Submit the whole Apothecary recipe — both ledgers' bucket sets plus any
    /// grimoire reserve(s) — in one message (answers the
    /// [`crate::frame::PendingDecision::ApothecaryDraft`] frame; final on
    /// receipt, like the Brewer pick).
    SubmitRecipe {
        /// The submitted recipe (must satisfy the frame's
        /// [`crate::frame::PendingDecision::permits_recipe`]).
        recipe: Recipe,
    },
    /// Cast a spell this wave (at most one per player per wave; optional, layered
    /// on the ingredient-or-pass choice — a spell never substitutes for it and
    /// never keeps a passed player active). Hidden until the wave reveals; an
    /// Instant fires at reveal, an Active primes face-down.
    CastSpell {
        /// The grimoire spell to cast.
        spell: CardId,
        /// The target, when the spell's [`crate::vocab::TargetKind`] requires one.
        target: Option<SpellTarget>,
    },
    /// Lock in the current selection; if all active players lock in, the wave closes early.
    LockIn,
    /// Send a preset emote to the table (the only communication channel).
    Emote {
        /// The palette emote to send.
        emote: EmoteId,
    },
    /// Opt in to play another game with the same group after `GameOver` (the
    /// post-game "ready" signal; a fresh game starts once 4 seats opt in).
    PlayAgain,
    /// Request matchmaking **fill**: top the group up to a full table with guests
    /// from the queue. Only meaningful from a partial group (fewer than 4 present);
    /// the server announces the search via [`crate::server::ServerMessage::GroupSearching`].
    FillGroup,
    /// Cancel an in-progress fill search and return to the idle group lobby.
    CancelFill,
    /// Leave the current group and return the connection to the unbound menu
    /// state, without closing the socket. The server frees the seat and replies
    /// with [`crate::server::ServerMessage::LeftGroup`].
    LeaveGroup,
    /// Liveness keepalive.
    Heartbeat,
}
