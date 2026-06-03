//! Messages sent from the server to clients, plus the audience model that keeps
//! secrets from leaking onto broadcasts.
//!
//! Secret discipline: the boiling point appears in exactly two messages —
//! [`ServerMessage::PeekResult`] (private, to the peeker) and
//! [`ServerMessage::Depile`] (only when the round exploded). No other message
//! type carries it; this is asserted by tests.

use serde::{Deserialize, Serialize};

use crate::ids::{EmoteId, GroupCode, PlayerId};
use crate::vocab::{CardView, Color, HandCard, ModifierKind};

/// Public, per-player lobby/table information (never includes hand contents).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerPublic {
    /// The player's stable id.
    pub id: PlayerId,
    /// The player's chosen display name.
    pub display_name: String,
    /// The player's assigned colour.
    pub color: Color,
    /// Whether the player is currently connected.
    pub connected: bool,
}

/// A single (player, score) pair — used instead of a map for stable wire order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerScore {
    /// The player.
    pub player: PlayerId,
    /// Their current cumulative score (may be negative).
    pub score: i32,
}

/// How many cards a player has contributed to the current pot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contribution {
    /// The player.
    pub player: PlayerId,
    /// Their contributed-card count (public — the key political signal).
    pub count: u8,
}

/// One revealed card in a depile, in reverse play order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepileEntry {
    /// Who played the card.
    pub player: PlayerId,
    /// The card's now-revealed attributes.
    pub card: CardView,
    /// Cumulative volatility after this card landed (for marking the crossing).
    pub running_volatility: u8,
}

/// The dominance outcome of a safely-resolved round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ScoringOutcome {
    /// One colour had the strictly highest total — its player takes all.
    Domination {
        /// The winning colour.
        winner: Color,
    },
    /// Two or more colours tied for the lead — those players split the pot.
    Split {
        /// The tied winning colours.
        colors: Vec<Color>,
    },
}

/// Machine-readable error codes for rejected actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    /// The client's protocol version is unsupported.
    VersionMismatch,
    /// The invite code maps to no active group.
    UnknownGroup,
    /// The committed card is not in the player's hand.
    NotYourCard,
    /// The action is illegal in the current phase.
    WrongPhase,
    /// The player is locked out of the round (already passed / timed out).
    LockedOut,
    /// The emote id is not in the configured palette.
    InvalidEmote,
    /// An unexpected server-side error.
    Internal,
}

/// A message from server to client. Enum-tagged for JSON-fallback readability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// Confirms a join and conveys the joining player's identity and the table. (private)
    GroupJoined {
        /// The group's invite code.
        group_code: GroupCode,
        /// The joining player's id.
        your_player_id: PlayerId,
        /// The joining player's assigned colour.
        your_color: Color,
        /// The connection's session token. The client persists this and replays it
        /// (as `session_token`) on future entry messages so its identity — and a
        /// held seat — survives a socket drop.
        session_token: String,
        /// Everyone currently in the group.
        players: Vec<PlayerPublic>,
    },
    /// The game is starting. (broadcast)
    GameStarting {
        /// Final table.
        players: Vec<PlayerPublic>,
        /// Number of rounds to be played.
        round_count: u8,
    },
    /// The recipient's private hand. (private)
    YourHand {
        /// The cards now in hand.
        cards: Vec<HandCard>,
    },
    /// A new wave has opened; carries the timer budget for a client countdown. (broadcast)
    WaveOpened {
        /// 1-based round number.
        round_number: u8,
        /// 1-based wave number within the round.
        wave_number: u8,
        /// Wave duration in milliseconds (informational; the server alone closes the wave).
        timer_ms: u32,
        /// Whether this is the one-player final wave: only one active player
        /// remains, who gets exactly this wave before the pot settles.
        final_wave: bool,
    },
    /// A wave resolved: who acted and the new count, never card identities. (broadcast)
    WaveResolved {
        /// Players who played a card this wave.
        played: Vec<PlayerId>,
        /// Players who passed (now locked out).
        passed: Vec<PlayerId>,
        /// Total cards now in the cauldron.
        cauldron_card_count: u8,
        /// Per-player contributed-card counts after this wave.
        contributions: Vec<Contribution>,
    },
    /// A new cauldron modifier was drawn and is now active. (broadcast)
    ModifierRevealed {
        /// The newly active modifier.
        modifier: ModifierKind,
        /// The round it was drawn for.
        round_number: u8,
    },
    /// Someone played Peek — anonymous; the value is not disclosed. (broadcast)
    SomeonePeeked,
    /// An Expose effect revealed a pot card to the whole table. (broadcast)
    Exposed {
        /// The revealed card.
        card: CardView,
    },
    /// The draw deck was reshuffled from the discard; any card counting resets. (broadcast)
    DeckReshuffled,
    /// A preset emote from a player. (broadcast)
    EmoteBroadcast {
        /// The sender.
        from: PlayerId,
        /// The emote sent.
        emote: EmoteId,
    },
    /// Private Peek result: the exact boiling point, only to the peeker. (private, secret)
    PeekResult {
        /// The exact boiling point.
        boiling_point: u8,
    },
    /// End-of-round reverse-order reveal of the whole pot. (broadcast)
    ///
    /// `boiling_point` is `Some` only when the round exploded; on a safe brew it
    /// stays hidden.
    Depile {
        /// Revealed cards, last-added first.
        reveals: Vec<DepileEntry>,
        /// Whether the round exploded.
        exploded: bool,
        /// The boiling point — revealed only on explosion.
        boiling_point: Option<u8>,
        /// Index into `reveals` of the card that tipped past the boiling point, if exploded.
        crossing_index: Option<usize>,
    },
    /// A safe-brew scoring result. (broadcast)
    RoundScored {
        /// Per-colour point totals used to decide dominance.
        color_points: Vec<(Color, u32)>,
        /// The dominance outcome.
        outcome: ScoringOutcome,
        /// Points awarded to each player this round.
        awards: Vec<PlayerScore>,
    },
    /// An explosion: everyone loses the pot value (Shielded players exempt). (broadcast)
    Explosion {
        /// The pot's total value that was lost.
        pot_value: u32,
        /// Per-player score delta applied (negative for the loss; zero for shielded).
        deltas: Vec<PlayerScore>,
        /// Players who were shielded and took no loss.
        shielded: Vec<PlayerId>,
    },
    /// Updated cumulative scores. (broadcast)
    ScoreUpdate {
        /// Every player's current score.
        scores: Vec<PlayerScore>,
    },
    /// The game (and any Deathmatch) is over. (broadcast)
    GameOver {
        /// Final cumulative scores.
        final_scores: Vec<PlayerScore>,
        /// The winner(s) — multiple only for co-champions.
        winners: Vec<PlayerId>,
    },
    /// An action was rejected. (private)
    Error {
        /// Machine-readable code.
        code: ErrorCode,
        /// Human-readable detail.
        message: String,
    },
    /// A player's connection state changed. (broadcast)
    PlayerConnectionChanged {
        /// The affected player.
        player: PlayerId,
        /// Whether they are now connected.
        connected: bool,
    },
    /// Full state for a reconnecting player, scoped to what they may know. (private)
    ///
    /// Deliberately omits secrets: no boiling point, no other players' hands, no
    /// face-down cauldron card identities.
    StateSnapshot {
        /// The group's invite code.
        group_code: GroupCode,
        /// The recipient's id.
        your_player_id: PlayerId,
        /// Current 1-based round number.
        round_number: u8,
        /// The table.
        players: Vec<PlayerPublic>,
        /// Current cumulative scores.
        scores: Vec<PlayerScore>,
        /// Active cauldron modifiers.
        active_modifiers: Vec<ModifierKind>,
        /// Per-player contributed-card counts in the current pot.
        contributions: Vec<Contribution>,
        /// The recipient's own hand.
        your_hand: Vec<HandCard>,
    },
    /// The post-game Deathmatch tiebreaker has begun among the tied leaders.
    /// The outcome (champion or co-champions) follows in [`ServerMessage::GameOver`]. (broadcast)
    DeathmatchStarted {
        /// The players tied for the lead who are contesting the tiebreaker.
        participants: Vec<PlayerId>,
    },
    /// Acknowledges a `LeaveGroup`: the seat is freed and the connection is now in
    /// the unbound menu state, ready for another entry message. (private)
    LeftGroup,
    /// Liveness acknowledgement. (private)
    Heartbeat,
}

/// Who a given [`ServerMessage`] is addressed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    /// Delivered only to one player's connection.
    Private(PlayerId),
    /// Delivered to every connection in the group.
    Broadcast,
}

/// A [`ServerMessage`] paired with its delivery [`Audience`]. The server builds
/// these deliberately so routing carries the secret/public decision explicitly.
#[derive(Debug, Clone, PartialEq)]
pub struct Outbound {
    /// Who receives the message.
    pub audience: Audience,
    /// The message itself.
    pub message: ServerMessage,
}

impl ServerMessage {
    /// Whether this message type must NEVER be broadcast (carries private or
    /// secret data). Used by the transport layer to assert correct routing.
    pub fn is_private_only(&self) -> bool {
        matches!(
            self,
            ServerMessage::YourHand { .. }
                | ServerMessage::PeekResult { .. }
                | ServerMessage::Error { .. }
                | ServerMessage::Heartbeat
                | ServerMessage::GroupJoined { .. }
                | ServerMessage::StateSnapshot { .. }
                | ServerMessage::LeftGroup
        )
    }

    /// Address this message privately to one player.
    pub fn to(self, player: PlayerId) -> Outbound {
        Outbound {
            audience: Audience::Private(player),
            message: self,
        }
    }

    /// Address this message to the whole group.
    pub fn broadcast(self) -> Outbound {
        debug_assert!(
            !self.is_private_only(),
            "attempted to broadcast a private-only message: {self:?}"
        );
        Outbound {
            audience: Audience::Broadcast,
            message: self,
        }
    }
}
