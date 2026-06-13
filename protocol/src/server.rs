//! Messages sent from the server to clients, plus the audience model that keeps
//! secrets from leaking onto broadcasts.
//!
//! Secret discipline (v4): in-round, the boiling point appears in exactly one
//! message — [`ServerMessage::PeekResult`] (private, to the peeker). Post-round
//! it is no longer a secret: every [`ServerMessage::Depile`] (boom *and* safe)
//! reveals it, so a safe brew gets its near-miss payoff. No other message type
//! carries it; this is asserted by tests.

use serde::{Deserialize, Serialize};

use crate::frame::PendingDecision;
use crate::ids::{EmoteId, GroupCode, PlayerId};
use crate::vocab::{
    Brewer, Color, HandIngredient, HandSpell, IngredientView, ModifierKind, Recipe, SpellKind,
};

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
    /// Whether this player is a **guest** — placed by matchmaking fill for one
    /// game, not a member of the group (members are `false`).
    pub guest: bool,
}

/// One player's public Brewer identity — a single (player, brewer) pair, used
/// instead of a map for stable wire order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerBrewer {
    /// The player.
    pub player: PlayerId,
    /// Their chosen Brewer (public from before the first wave).
    pub brewer: Brewer,
}

/// One player's public recipe — a single (player, recipe) pair, used instead
/// of a map for stable wire order. The recipe (buckets + reserves) is public;
/// the realized decks never cross the wire (`boom2-apothecary`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerRecipe {
    /// The player.
    pub player: PlayerId,
    /// Their submitted (or defaulted) recipe.
    pub recipe: Recipe,
}

/// One member's line in a group's live standings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberStanding {
    /// The member.
    pub player: PlayerId,
    /// Games this member has played in the group.
    pub games_played: u32,
    /// Games this member has won (co-champions each count).
    pub wins: u32,
}

/// A single (player, score) pair — used instead of a map for stable wire order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerScore {
    /// The player.
    pub player: PlayerId,
    /// Their current cumulative score (may be negative).
    pub score: i32,
}

/// How many ingredients a player has contributed to the current pot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contribution {
    /// The player.
    pub player: PlayerId,
    /// Their contributed-card count (public — the key political signal).
    pub count: u8,
}

/// Compounding that fired on a depile entry (change `boom2-compounding`): what
/// in-pot interaction paid off on this card and by how much, so the depile can
/// explain a combo/threshold contribution — and any effective-volatility shift
/// that moved the detonator. Only the **completing** card of a combo carries
/// the fire (a lone half narrates nothing — it is a plain card).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum CompoundingFire {
    /// A named combo paid off — all its members were in the pot (a Herbalist's
    /// fires twice). Bigger combos pay massively.
    Combo {
        /// The combo's size (2–5 members) — the bigger, the rarer and richer.
        size: u8,
        /// Bonus points the combo added to the owner's colour.
        bonus_points: u8,
        /// Bonus volatility the combo added (non-zero only for an Alchemist).
        bonus_volatility: u8,
    },
    /// A count-threshold paid off in a large pot.
    Threshold {
        /// Bonus points the threshold added to this card's colour.
        bonus_points: u8,
    },
}

/// One revealed ingredient in a depile, in **ascending effective-volatility**
/// order (the "fuse climb" — every round, boom or safe).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepileEntry {
    /// Who played the ingredient.
    pub player: PlayerId,
    /// The ingredient's now-revealed printed attributes.
    pub ingredient: IngredientView,
    /// Whether it was played colorless (volatility only, zero points).
    pub colorless: bool,
    /// The 1-based wave it was played in (fatal-wave liability is wave-scoped).
    pub wave_number: u8,
    /// Cumulative volatility after this entry in the sorted climb (rises toward —
    /// or past — the revealed boiling point). Includes any combo-added volatility.
    pub running_volatility: u8,
    /// Whether this entry is liable for the explosion (a detonator card). Always
    /// `false` on a safe brew.
    pub liable: bool,
    /// The compounding that fired on this card, if any (`boom2-compounding`):
    /// the combo/threshold contribution narrated to the table.
    #[serde(default)]
    pub compounding: Option<CompoundingFire>,
}

/// A fired Active spell, narrated at resolution (wards and Hex on an explosion,
/// Harvest on a safe brew). This is the visible-when-activated moment for
/// Actives — a primed-but-unfired spell is never disclosed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpellFire {
    /// Who had primed the spell.
    pub player: PlayerId,
    /// The spell that fired.
    pub spell: SpellKind,
    /// The player it was aimed at, when the spell targets a player.
    pub target: Option<PlayerId>,
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
    /// The committed ingredient is not in the player's hand.
    NotYourCard,
    /// The cast spell is not in the player's grimoire hand.
    NotYourSpell,
    /// The spell's target is missing, of the wrong kind, or illegal.
    InvalidTarget,
    /// A second spell was cast in the same wave (the limit is one).
    SpellLimit,
    /// The action is illegal in the current phase.
    WrongPhase,
    /// The player is locked out of the round (already passed / timed out).
    LockedOut,
    /// The action answered a decision frame that has already been resolved
    /// (its deadline passed / the phase advanced); the auto-resolved outcome
    /// stands and no state changed.
    StaleFrame,
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
    /// The recipient's private hand: ingredients (topped up to the hand floor
    /// each wave) and the hoarded grimoire spells. (private)
    YourHand {
        /// The ingredients now in hand.
        ingredients: Vec<HandIngredient>,
        /// The spells now in hand (drawn at round start, carried over; replenished
        /// in-round only by Forage).
        spells: Vec<HandSpell>,
    },
    /// The pre-game brewer phase closed: every player's chosen Brewer, published
    /// to the whole table before deck construction and the first wave. (broadcast)
    BrewersRevealed {
        /// Each seated player's chosen Brewer, in seating order.
        brewers: Vec<PlayerBrewer>,
    },
    /// The pre-game Apothecary draft closed: every player's public recipe,
    /// published to the whole table before deck realization and the first
    /// wave (`boom2-apothecary`). Only the recipes are public — the realized
    /// cards and draw order stay hidden, owner included. (broadcast)
    RecipesRevealed {
        /// Each seated player's recipe, in seating order.
        recipes: Vec<PlayerRecipe>,
    },
    /// The recipient owes a decision: the pending decision kind and its complete
    /// legal action set (see [`crate::frame`]). Sent whenever a decision opens,
    /// and re-sent (refreshed) when the legal set shrinks mid-decision (e.g. the
    /// one allowed spell was cast). Carries only recipient-permitted
    /// information. (private)
    DecisionFrame {
        /// 1-based round number the decision belongs to.
        round_number: u8,
        /// 1-based wave number within the round (0 for future non-wave kinds).
        wave_number: u8,
        /// Remaining decision budget in milliseconds, when a timer applies
        /// (informational; the server alone closes the decision).
        timer_ms: Option<u32>,
        /// The pending decision and its enumerated legal actions.
        decision: PendingDecision,
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
    /// An Instant spell activated at the wave reveal — visible to the whole table
    /// (caster + which spell; a volatility spell thereby reveals its *delta*,
    /// never the cauldron's absolute total). Primed Actives emit nothing. (broadcast)
    SpellCast {
        /// Who cast it.
        player: PlayerId,
        /// The spell that activated.
        spell: SpellKind,
        /// The colour it was aimed at, for colour-targeted spells (Double Down,
        /// Sour). Player-targeted Actives disclose their target only on fire.
        color_target: Option<Color>,
    },
    /// A wave resolved: who acted and the new count, never card identities. (broadcast)
    WaveResolved {
        /// Players who played an ingredient this wave.
        played: Vec<PlayerId>,
        /// Players who passed (now locked out).
        passed: Vec<PlayerId>,
        /// Total ingredients now in the cauldron.
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
    /// An Expose spell revealed a pot ingredient to the whole table. (broadcast)
    Exposed {
        /// Who had played the revealed ingredient.
        player: PlayerId,
        /// The revealed ingredient.
        ingredient: IngredientView,
        /// Whether it had been played colorless.
        colorless: bool,
    },
    /// A preset emote from a player. (broadcast)
    EmoteBroadcast {
        /// The sender.
        from: PlayerId,
        /// The emote sent.
        emote: EmoteId,
    },
    /// Private Peek result: the exact boiling point, only to the caster. (private, secret)
    PeekResult {
        /// The exact boiling point.
        boiling_point: u8,
    },
    /// Private Assay result: the dominant colour and its point lead, only to the
    /// caster. (private, secret)
    AssayResult {
        /// The currently dominant colour (None if no colored Votes yet).
        dominant: Option<Color>,
        /// Its lead in points over the runner-up colour.
        lead: u32,
    },
    /// End-of-round reveal of the whole pot, sorted ascending by effective
    /// volatility, **revealing the boiling point every round** (boom and safe).
    /// On a boom the running climb crosses the line and the liable entries are
    /// marked; on a safe brew it stops short. (broadcast)
    Depile {
        /// Revealed ingredients in ascending effective-volatility order.
        reveals: Vec<DepileEntry>,
        /// Whether the round exploded.
        exploded: bool,
        /// The boiling point — revealed every round once the pot settles.
        boiling_point: u8,
        /// Index into `reveals` where the sorted climb first crossed the boiling
        /// point, if exploded.
        crossing_index: Option<usize>,
    },
    /// A safe-brew scoring result: the dominant colour wins +P. (broadcast)
    RoundScored {
        /// Per-colour point totals used to decide dominance.
        color_points: Vec<(Color, u32)>,
        /// The dominance outcome.
        outcome: ScoringOutcome,
        /// Points awarded to each player this round.
        awards: Vec<PlayerScore>,
        /// Harvests that fired on this win, narrated here.
        fired: Vec<SpellFire>,
    },
    /// An explosion: the detonator(s) split −P; everyone else loses nothing. (broadcast)
    Explosion {
        /// The pot's value P that the detonators split.
        pot_value: u32,
        /// The liable players (fatal-wave trigger + heavier cards).
        detonators: Vec<PlayerId>,
        /// Per-player score delta applied (after wards/Redirect/Hex; zero for
        /// the unaffected).
        deltas: Vec<PlayerScore>,
        /// Wards and Hexes that fired at this resolution, narrated in fire order.
        fired: Vec<SpellFire>,
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
    /// face-down cauldron identities, no primed Actives.
    StateSnapshot {
        /// The group's invite code.
        group_code: GroupCode,
        /// The recipient's id.
        your_player_id: PlayerId,
        /// Current 1-based round number.
        round_number: u8,
        /// The table.
        players: Vec<PlayerPublic>,
        /// Every player's public Brewer (empty before the brewer phase closes).
        brewers: Vec<PlayerBrewer>,
        /// Every player's public recipe (empty before the draft closes).
        recipes: Vec<PlayerRecipe>,
        /// Current cumulative scores.
        scores: Vec<PlayerScore>,
        /// Active cauldron modifiers.
        active_modifiers: Vec<ModifierKind>,
        /// Per-player contributed-card counts in the current pot.
        contributions: Vec<Contribution>,
        /// The recipient's own ingredients.
        your_ingredients: Vec<HandIngredient>,
        /// The recipient's own spells.
        your_spells: Vec<HandSpell>,
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
    /// The group is searching matchmaking for more players to fill the table
    /// ("looking for a 4th…"). (broadcast)
    GroupSearching {
        /// How many more players the group needs to reach a full table.
        needed: u8,
    },
    /// The group's live standings: per-member games/wins, plus the aggregate guest
    /// line so guest results don't vanish. Conveyed to members. (broadcast)
    StandingsUpdate {
        /// One line per current member.
        members: Vec<MemberStanding>,
        /// Games this group has played that included a guest.
        guest_games: u32,
        /// Games won by the group's guest (across all guests).
        guest_wins: u32,
    },
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
                | ServerMessage::DecisionFrame { .. }
                | ServerMessage::PeekResult { .. }
                | ServerMessage::AssayResult { .. }
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
