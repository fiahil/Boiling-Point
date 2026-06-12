//! Per-game balance observations, derived purely from the messages a seat
//! received — the unit the balance harness aggregates (Principle IV) and the
//! seat-filler logs. Broadcast-visible facts only; identical across seats.

use boiling_point_protocol::PlayerId;
use boiling_point_protocol::vocab::{Color, ModifierKind};

/// What a seat observed in one round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundObservation {
    /// 1-based round number.
    pub round_number: u8,
    /// Whether the round exploded.
    pub exploded: bool,
    /// The pot's scored value — split by detonators on a boom, paid to the
    /// dominant colour on a safe brew.
    pub pot_value: u32,
    /// The detonators, if the round exploded.
    pub detonators: Vec<PlayerId>,
    /// Ingredients in the cauldron at settle (the depile length).
    pub cards_in_pot: u32,
    /// Waves the round ran.
    pub waves: u32,
    /// Visible Peek casts (the Peek-economy signal).
    pub peek_casts: u32,
    /// All visible spell activations (Instants; unfired primes stay invisible).
    pub spell_casts: u32,
    /// Whether the round ended on an all-pass wave (the freeze signal).
    pub ended_all_pass: bool,
    /// Players who folded (passed) before the round settled, then watched it
    /// resolve safely from the sidelines — the fold-to-safety signal.
    pub folded_safe: Vec<PlayerId>,
    /// The modifier revealed at the start of this round, if any.
    pub modifier: Option<ModifierKind>,
}

impl RoundObservation {
    /// A fresh observation for round `round_number`.
    pub fn new(round_number: u8) -> Self {
        RoundObservation {
            round_number,
            exploded: false,
            pot_value: 0,
            detonators: Vec::new(),
            cards_in_pot: 0,
            waves: 0,
            peek_casts: 0,
            spell_casts: 0,
            ended_all_pass: false,
            folded_safe: Vec::new(),
            modifier: None,
        }
    }
}

/// Everything one seat saw across a complete game.
#[derive(Debug, Clone, Default)]
pub struct GameObservation {
    /// The observing seat's player id.
    pub me: Option<PlayerId>,
    /// The observing seat's colour.
    pub my_color: Option<Color>,
    /// Per-round observations, in order.
    pub rounds: Vec<RoundObservation>,
    /// The game's winner(s).
    pub winners: Vec<PlayerId>,
    /// Final cumulative scores.
    pub final_scores: Vec<(PlayerId, i32)>,
    /// Whether the game reached `GameOver`.
    pub completed: bool,
}
