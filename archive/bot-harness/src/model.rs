//! The bot's player-visible domain model (D2).
//!
//! This is the world as a *player* may know it, assembled solely from received
//! [`ServerMessage`]s. There is deliberately **no field** for an opponent's hand,
//! the decks, primed Actives, or the cauldron's absolute volatility — the only
//! way such data could enter is a message that carries it, and none does. The
//! single exception is the boiling point, which a player legitimately learns in
//! exactly two ways: a [`ServerMessage::PeekResult`] from a Peek they cast, or
//! the post-round depile (which reveals it every round). It enters only through
//! [`PlayerView::disclose_boiling_point`], so leakage is a structural
//! impossibility rather than a matter of discipline, and every game doubles as a
//! secret-boundary test (see [`crate::bot`]).

use boiling_point_protocol::ServerMessage;
use boiling_point_protocol::server::{Contribution, PlayerPublic, PlayerScore};
use boiling_point_protocol::vocab::{
    Color, HandIngredient, HandSpell, IngredientView, ModifierKind, SpellKind,
};
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
    /// This bot's own private ingredient hand (topped up each wave).
    pub ingredients: Vec<HandIngredient>,
    /// This bot's own private spell hand (the hoard).
    pub spells: Vec<HandSpell>,
    /// Current 1-based round number.
    pub round_number: u8,
    /// Current 1-based wave number within the round.
    pub wave_number: u8,
    /// Total ingredients now in the cauldron (public count, never identities).
    pub cauldron_card_count: u8,
    /// Per-player contributed-card counts in the current pot (the key political signal).
    pub contributions: Vec<Contribution>,
    /// Current cumulative scores.
    pub scores: Vec<PlayerScore>,
    /// Modifiers active this round (cumulative from round 2).
    pub active_modifiers: Vec<ModifierKind>,
    /// Ingredients Exposed to the whole table this round: (owner, card, colorless).
    pub exposed_this_round: Vec<(PlayerId, IngredientView, bool)>,
    /// Net visible volatility-spell pressure this round: +1 per observed Surge,
    /// −1 per observed Dampen (the public *delta* signal — magnitudes are
    /// content numbers a bot estimates; the absolute total is never on the wire).
    pub volatility_spell_pressure: i32,
    /// The 1-based wave number a observed Quench shields (that wave cannot
    /// explode), if one is pending.
    pub quench_shielded_wave: Option<u8>,
    /// Whether this bot has passed (is locked out) this round.
    pub passed: bool,
    /// The boiling point — `Some` ONLY after a sanctioned disclosure (a Peek
    /// this bot cast, or the post-round depile). Never otherwise.
    boiling_point: Option<u8>,
}

impl PlayerView {
    /// A fresh view for a bot that knows its own identity and seat colour.
    ///
    /// In-process the harness assigns these (a player always knows their own
    /// seat); over WebSocket they are confirmed by [`ServerMessage::GroupJoined`].
    pub fn new(me: PlayerId, my_color: Color) -> Self {
        PlayerView {
            me,
            my_color,
            players: Vec::new(),
            ingredients: Vec::new(),
            spells: Vec::new(),
            round_number: 0,
            wave_number: 0,
            cauldron_card_count: 0,
            contributions: Vec::new(),
            scores: Vec::new(),
            active_modifiers: Vec::new(),
            exposed_this_round: Vec::new(),
            volatility_spell_pressure: 0,
            quench_shielded_wave: None,
            passed: false,
            boiling_point: None,
        }
    }

    /// Start-of-round reset: the round's transient knowledge (pass lockout,
    /// exposed cards, quench window, spell pressure, any peeked boiling point)
    /// clears. Cumulative state (scores, modifiers, hands) is untouched.
    pub fn begin_round(&mut self) {
        self.passed = false;
        self.exposed_this_round.clear();
        self.volatility_spell_pressure = 0;
        self.quench_shielded_wave = None;
        self.cauldron_card_count = 0;
        self.boiling_point = None;
    }

    /// Record the sanctioned disclosure of the boiling point. The ONLY route by
    /// which this value enters the model; called from exactly the Peek-result
    /// and depile message arms.
    pub fn disclose_boiling_point(&mut self, value: u8) {
        self.boiling_point = Some(value);
    }

    /// The boiling point if (and only if) it has been disclosed to this bot.
    pub fn known_boiling_point(&self) -> Option<u8> {
        self.boiling_point
    }

    /// Remove an ingredient this bot committed from its own hand (its own
    /// action, not a secret). Called once the server confirms the play in
    /// `WaveResolved`; the next wave's `YourHand` refresh re-syncs regardless.
    pub fn remove_ingredient(&mut self, card: CardId) {
        if let Some(pos) = self.ingredients.iter().position(|c| c.id == card) {
            self.ingredients.remove(pos);
        }
    }

    /// Remove a spell this bot cast from its own hand (optimistic — a primed
    /// Active is never echoed back; the next `YourHand` refresh re-syncs).
    pub fn remove_spell(&mut self, spell: CardId) {
        if let Some(pos) = self.spells.iter().position(|s| s.id == spell) {
            self.spells.remove(pos);
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

    /// A coarse public estimate of the cauldron's volatility: card count times
    /// an assumed mean per-card volatility, plus the observed spell pressure at
    /// an assumed magnitude. Pure heuristics over public signals — the absolute
    /// total is never on the wire.
    pub fn estimated_volatility(&self) -> f64 {
        const ASSUMED_MEAN_VOLATILITY: f64 = 2.4;
        const ASSUMED_SPELL_DELTA: f64 = 3.0;
        self.cauldron_card_count as f64 * ASSUMED_MEAN_VOLATILITY
            + self.volatility_spell_pressure as f64 * ASSUMED_SPELL_DELTA
    }

    /// Fold one inbound message into the view (the pure state part; the bot's
    /// act-on-`WaveOpened` logic and observation bookkeeping live in
    /// [`crate::bot`]).
    pub fn observe(&mut self, msg: &ServerMessage) {
        match msg {
            ServerMessage::GroupJoined {
                your_player_id,
                your_color,
                players,
                ..
            } => {
                self.me = *your_player_id;
                self.my_color = *your_color;
                self.players = players.clone();
            }
            ServerMessage::GameStarting { players, .. } => {
                self.players = players.clone();
            }
            ServerMessage::YourHand {
                ingredients,
                spells,
            } => {
                self.ingredients = ingredients.clone();
                self.spells = spells.clone();
            }
            ServerMessage::WaveOpened {
                round_number,
                wave_number,
                ..
            } => {
                if *wave_number == 1 {
                    self.begin_round();
                }
                self.round_number = *round_number;
                self.wave_number = *wave_number;
                // A spent quench window (an earlier wave) is forgotten.
                if self.quench_shielded_wave.is_some_and(|w| w < *wave_number) {
                    self.quench_shielded_wave = None;
                }
            }
            ServerMessage::SpellCast { spell, .. } => match spell {
                SpellKind::Surge => self.volatility_spell_pressure += 1,
                SpellKind::Dampen => self.volatility_spell_pressure -= 1,
                // A Quench revealed during wave N's resolution shields wave N+1.
                SpellKind::Quench => self.quench_shielded_wave = Some(self.wave_number + 1),
                _ => {}
            },
            ServerMessage::WaveResolved {
                passed,
                cauldron_card_count,
                contributions,
                ..
            } => {
                self.cauldron_card_count = *cauldron_card_count;
                self.contributions = contributions.clone();
                if passed.contains(&self.me) {
                    self.passed = true;
                }
            }
            ServerMessage::ModifierRevealed { modifier, .. } => {
                self.active_modifiers.push(*modifier);
            }
            ServerMessage::Exposed {
                player,
                ingredient,
                colorless,
            } => {
                self.exposed_this_round
                    .push((*player, *ingredient, *colorless));
            }
            ServerMessage::PeekResult { boiling_point } => {
                // Sanctioned disclosure: a Peek this bot cast.
                self.disclose_boiling_point(*boiling_point);
            }
            ServerMessage::Depile { boiling_point, .. } => {
                // Sanctioned disclosure: the post-round reveal (every round).
                self.disclose_boiling_point(*boiling_point);
            }
            ServerMessage::ScoreUpdate { scores } => {
                self.scores = scores.clone();
            }
            ServerMessage::PlayerConnectionChanged { player, connected } => {
                if let Some(p) = self.players.iter_mut().find(|p| p.id == *player) {
                    p.connected = *connected;
                }
            }
            ServerMessage::StateSnapshot {
                scores,
                active_modifiers,
                contributions,
                your_ingredients,
                your_spells,
                ..
            } => {
                // Reconnection rehydrate (unused in batch play, handled for safety):
                // refresh visible state without touching the pass lockout.
                self.scores = scores.clone();
                self.active_modifiers = active_modifiers.clone();
                self.contributions = contributions.clone();
                self.ingredients = your_ingredients.clone();
                self.spells = your_spells.clone();
            }
            _ => {}
        }
    }
}
