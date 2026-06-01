//! The bot's player-visible domain model (D2).
//!
//! This is the world as a *player* may know it, assembled solely from received
//! [`ServerMessage`]s. There is deliberately **no field** for an opponent's hand
//! or the draw deck — the only way such data could enter is a message that
//! carries it, and none does. The single exception is the boiling point, which a
//! player legitimately learns in exactly two ways: a [`ServerMessage::PeekResult`]
//! from a Peek they played, or the reveal on an exploded
//! [`ServerMessage::Depile`]. That value enters only through
//! [`PlayerView::disclose_boiling_point`], so leakage is a structural
//! impossibility rather than a matter of discipline, and every game doubles as a
//! secret-boundary test (see [`crate::bot`]).

use boiling_point_protocol::server::{Contribution, PlayerPublic, PlayerScore};
use boiling_point_protocol::vocab::{CardView, Color, HandCard, ModifierKind};
use boiling_point_protocol::{CardId, PlayerId};

/// The player-visible game state a bot reasons over.
#[derive(Debug, Clone)]
pub struct PlayerView {
    /// This bot's own stable id.
    pub me: PlayerId,
    /// This bot's assigned colour.
    pub my_color: Color,
    /// Everyone at the table (public info only — never hand contents).
    pub players: Vec<PlayerPublic>,
    /// This bot's own private hand (the only cards it may see).
    pub hand: Vec<HandCard>,
    /// Current 1-based round number.
    pub round_number: u8,
    /// Current 1-based wave number within the round.
    pub wave_number: u8,
    /// Total cards now in the cauldron (public count, never identities).
    pub cauldron_card_count: u8,
    /// Per-player contributed-card counts in the current pot (the key political signal).
    pub contributions: Vec<Contribution>,
    /// Current cumulative scores.
    pub scores: Vec<PlayerScore>,
    /// Modifiers active this round (cumulative from round 2).
    pub active_modifiers: Vec<ModifierKind>,
    /// Cards Exposed to the whole table this round (public reveals).
    pub exposed_this_round: Vec<CardView>,
    /// Whether this bot has passed (is locked out) this round.
    pub passed: bool,
    /// The boiling point — `Some` ONLY after a sanctioned disclosure this round
    /// (a Peek this bot played, or an explosion reveal). Never otherwise.
    boiling_point: Option<u8>,
}

impl PlayerView {
    /// A fresh view for a bot that knows its own identity and seat colour.
    ///
    /// In-process the harness assigns these (a player always knows their own
    /// seat); over WebSocket they are confirmed by [`ServerMessage::RoomJoined`].
    pub fn new(me: PlayerId, my_color: Color) -> Self {
        PlayerView {
            me,
            my_color,
            players: Vec::new(),
            hand: Vec::new(),
            round_number: 0,
            wave_number: 0,
            cauldron_card_count: 0,
            contributions: Vec::new(),
            scores: Vec::new(),
            active_modifiers: Vec::new(),
            exposed_this_round: Vec::new(),
            passed: false,
            boiling_point: None,
        }
    }

    /// Start-of-round reset: a fresh hand is dealt and the round's transient,
    /// per-round knowledge (pass lockout, exposed cards, any peeked boiling point)
    /// clears. Cumulative state (scores, modifiers) is untouched.
    pub fn begin_round(&mut self, hand: Vec<HandCard>) {
        self.hand = hand;
        self.passed = false;
        self.exposed_this_round.clear();
        self.boiling_point = None;
    }

    /// Record the sanctioned disclosure of the boiling point. The ONLY route by
    /// which this value enters the model; called from exactly the Peek-result and
    /// exploded-depile message arms.
    pub fn disclose_boiling_point(&mut self, value: u8) {
        self.boiling_point = Some(value);
    }

    /// The boiling point if (and only if) it has been disclosed to this bot.
    pub fn known_boiling_point(&self) -> Option<u8> {
        self.boiling_point
    }

    /// Remove a card this bot committed from its own hand (its own action, not a
    /// secret). Called once the server confirms the play in `WaveResolved`.
    pub fn remove_from_hand(&mut self, card: CardId) {
        if let Some(pos) = self.hand.iter().position(|c| c.id == card) {
            self.hand.remove(pos);
        }
    }

    /// This bot's current cumulative score, if known.
    pub fn my_score(&self) -> Option<i32> {
        self.scores
            .iter()
            .find(|s| s.player == self.me)
            .map(|s| s.score)
    }

    /// The highest score currently held by any *opponent*, if known.
    pub fn best_opponent_score(&self) -> Option<i32> {
        self.scores
            .iter()
            .filter(|s| s.player != self.me)
            .map(|s| s.score)
            .max()
    }
}
