//! A single round as a sequence of simultaneous waves.
//!
//! The `Round` owns the pot and the active/locked-out bookkeeping and decides
//! when the round ends — explosion, everyone-remaining-passed, or the
//! one-player-final-wave guard (no wave cap). It does NOT own hands: the caller
//! (the game runner) validates choices against hands, removes committed cards,
//! and tells the round who played / passed / emptied their hand. Recalled cards
//! come back via the wave outcome for the caller to return to hands.

use std::collections::{HashMap, HashSet};

use boiling_point_protocol::{CardId, PlayerId};

use crate::content::ContentRegistry;

use super::card::Card;
use super::pot::Pot;
use super::resolve::{WaveOutcome, resolve_wave};

/// How a round ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundEnd {
    /// Volatility crossed the boiling point.
    Exploded,
    /// The pot settled safely (everyone locked out, or the final wave passed).
    Settled,
}

/// One card's contribution to the depile, in play order.
#[derive(Debug, Clone, Copy)]
pub struct DepileItem {
    /// Who played the card.
    pub player: PlayerId,
    /// The card (revealed at depile).
    pub card: Card,
    /// Cumulative base volatility after this card landed (for marking the crossing).
    pub running_volatility: i32,
}

/// The data needed to render the end-of-round depile.
#[derive(Debug, Clone)]
pub struct DepileData {
    /// Cards last-added-first (reverse play order).
    pub reveals: Vec<DepileItem>,
    /// Index into `reveals` of the card that tipped past the boiling point (if exploded).
    pub crossing_index: Option<usize>,
    /// The boiling point — disclosed by the caller only on an explosion.
    pub boiling_point: u8,
}

/// One player's choice in a wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveChoice {
    /// Commit a specific card.
    Play(CardId),
    /// Pass (permanent lockout for the round).
    Pass,
}

/// The caller's resolved input for a wave: the cards actually committed (already
/// removed from hands), who passed, who emptied their hand by playing, and any
/// recall targets.
pub struct WaveInput {
    /// Players who played, with the card they committed.
    pub committed: Vec<(PlayerId, Card)>,
    /// Active players who passed or timed out this wave.
    pub passers: Vec<PlayerId>,
    /// Players who, after this wave (and any recall), now hold no cards.
    pub emptied: Vec<PlayerId>,
    /// Recall targets (player → the own card id they pull back).
    pub recalls: HashMap<PlayerId, CardId>,
}

/// What happened in a wave.
pub struct WaveReport {
    /// The resolver's outcome (explosion flag, peeks, exposes, recalled cards).
    pub outcome: WaveOutcome,
    /// `Some` if this wave ended the round.
    pub ended: Option<RoundEnd>,
}

/// A round in progress.
pub struct Round {
    /// Effective (post-modifier) boiling point — hidden from players.
    boiling_point: i32,
    /// The accumulating pot.
    pot: Pot,
    /// Players who played Shield this round.
    shielded: HashSet<PlayerId>,
    /// Players still in the round (will act next wave), in seating order.
    active: Vec<PlayerId>,
    /// 1-based wave counter.
    wave_number: u8,
    /// Whether the one-player final wave has been granted.
    final_wave_used: bool,
    /// Set once the round ends.
    ended: Option<RoundEnd>,
}

impl Round {
    /// Begin a round. `active` are the players who start with cards, in order;
    /// `start_volatility` seeds the pot (e.g. from Residue).
    pub fn start(active: Vec<PlayerId>, boiling_point: i32, start_volatility: i32) -> Self {
        Round {
            boiling_point,
            pot: Pot::new(start_volatility),
            shielded: HashSet::new(),
            active,
            wave_number: 1,
            final_wave_used: false,
            ended: None,
        }
    }

    /// Players still active (will act next wave).
    pub fn active(&self) -> &[PlayerId] {
        &self.active
    }

    /// The current 1-based wave number.
    pub fn wave_number(&self) -> u8 {
        self.wave_number
    }

    /// Whether the round is still open for more waves.
    pub fn is_open(&self) -> bool {
        self.ended.is_none()
    }

    /// How the round ended, if it has.
    pub fn ended(&self) -> Option<RoundEnd> {
        self.ended
    }

    /// The shielded players (for scoring).
    pub fn shielded(&self) -> &HashSet<PlayerId> {
        &self.shielded
    }

    /// The final pot (for scoring once the round ends).
    pub fn pot(&self) -> &Pot {
        &self.pot
    }

    /// Apply one wave: resolve the committed cards/effects, then update the
    /// active set and decide whether the round ends.
    pub fn apply_wave(&mut self, registry: &ContentRegistry, input: WaveInput) -> WaveReport {
        debug_assert!(self.is_open(), "wave applied to a finished round");

        let played: Vec<PlayerId> = input.committed.iter().map(|(p, _)| *p).collect();
        let outcome = resolve_wave(
            registry,
            &mut self.pot,
            self.boiling_point,
            &mut self.shielded,
            input.committed,
            &input.recalls,
        );

        // Players still active after this wave: those who played and still hold cards.
        let played_set: HashSet<PlayerId> = played.iter().copied().collect();
        let emptied_set: HashSet<PlayerId> = input.emptied.iter().copied().collect();
        let next: Vec<PlayerId> = self
            .active
            .iter()
            .copied()
            .filter(|p| played_set.contains(p) && !emptied_set.contains(p))
            .collect();

        let ended = if outcome.exploded {
            Some(RoundEnd::Exploded)
        } else if played.is_empty() {
            // All remaining active players passed in the same wave.
            Some(RoundEnd::Settled)
        } else if self.final_wave_used {
            // The granted one-player final wave just resolved.
            Some(RoundEnd::Settled)
        } else if next.len() <= 1 {
            if next.is_empty() {
                Some(RoundEnd::Settled)
            } else {
                // Only one active player remains — grant exactly one more wave.
                self.final_wave_used = true;
                self.active = next;
                None
            }
        } else {
            self.active = next;
            None
        };

        self.ended = ended;
        self.wave_number += 1;
        WaveReport { outcome, ended }
    }

    /// Build the depile (reverse play order). `running_volatility` and the
    /// crossing index use cumulative base card volatility, which is what the
    /// "crack" visual marks.
    pub fn depile(&self) -> DepileData {
        let mut running = 0i32;
        let mut play_order: Vec<DepileItem> = Vec::with_capacity(self.pot.cards.len());
        let mut crossing_play_order: Option<usize> = None;
        for (i, pc) in self.pot.cards.iter().enumerate() {
            running += pc.card.volatility as i32;
            if crossing_play_order.is_none() && running > self.boiling_point {
                crossing_play_order = Some(i);
            }
            play_order.push(DepileItem {
                player: pc.player,
                card: pc.card,
                running_volatility: running,
            });
        }
        let len = play_order.len();
        play_order.reverse();
        // Translate the crossing index into the reversed list.
        let crossing_index = crossing_play_order.map(|i| len - 1 - i);
        DepileData {
            reveals: play_order,
            crossing_index,
            boiling_point: self.boiling_point.max(0) as u8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::card::Card;
    use boiling_point_protocol::vocab::Color;
    use uuid::Uuid;

    fn pid(n: u128) -> PlayerId {
        PlayerId(Uuid::from_u128(n))
    }

    fn registry() -> ContentRegistry {
        crate::config::ContentConfig::from_toml(include_str!("../../content.toml"))
            .unwrap()
            .build_registry()
            .unwrap()
    }

    fn card(id: u32, vol: u8) -> Card {
        Card {
            id: CardId(id),
            color: Color::Ruby,
            volatility: vol,
            points: 1,
            effect: None,
        }
    }

    fn play(player: PlayerId, c: Card) -> WaveInput {
        WaveInput {
            committed: vec![(player, c)],
            passers: vec![],
            emptied: vec![],
            recalls: HashMap::new(),
        }
    }

    #[test]
    fn explosion_ends_the_round() {
        let reg = registry();
        let ps: Vec<PlayerId> = (1..=4).map(pid).collect();
        let mut round = Round::start(ps.clone(), 3, 0);
        // One big card past the boiling point of 3.
        let report = round.apply_wave(
            &reg,
            WaveInput {
                committed: vec![(ps[0], card(1, 5))],
                passers: vec![ps[1], ps[2], ps[3]],
                emptied: vec![],
                recalls: HashMap::new(),
            },
        );
        assert_eq!(report.ended, Some(RoundEnd::Exploded));
        assert!(!round.is_open());
    }

    #[test]
    fn everyone_passing_settles() {
        let reg = registry();
        let ps: Vec<PlayerId> = (1..=4).map(pid).collect();
        let mut round = Round::start(ps.clone(), 20, 0);
        let report = round.apply_wave(
            &reg,
            WaveInput {
                committed: vec![],
                passers: ps.clone(),
                emptied: vec![],
                recalls: HashMap::new(),
            },
        );
        assert_eq!(report.ended, Some(RoundEnd::Settled));
    }

    #[test]
    fn shrinking_field_triggers_one_player_final_wave() {
        let reg = registry();
        let ps: Vec<PlayerId> = (1..=4).map(pid).collect();
        let mut round = Round::start(ps.clone(), 50, 0);

        // Wave 1: players 1 & 2 play, 3 & 4 pass → 2 active remain.
        let r1 = round.apply_wave(
            &reg,
            WaveInput {
                committed: vec![(ps[0], card(1, 1)), (ps[1], card(2, 1))],
                passers: vec![ps[2], ps[3]],
                emptied: vec![],
                recalls: HashMap::new(),
            },
        );
        assert_eq!(r1.ended, None);
        assert_eq!(round.active(), &[ps[0], ps[1]]);

        // Wave 2: player 1 plays, player 2 passes → only 1 active → final wave granted.
        let r2 = round.apply_wave(
            &reg,
            WaveInput {
                committed: vec![(ps[0], card(3, 1))],
                passers: vec![ps[1]],
                emptied: vec![],
                recalls: HashMap::new(),
            },
        );
        assert_eq!(r2.ended, None, "the lone survivor gets one final wave");
        assert_eq!(round.active(), &[ps[0]]);

        // Wave 3 (the final wave): player 1 plays once more → round settles.
        let r3 = round.apply_wave(&reg, play(ps[0], card(4, 1)));
        assert_eq!(r3.ended, Some(RoundEnd::Settled));
    }

    #[test]
    fn depile_marks_the_crossing_card() {
        let reg = registry();
        let ps: Vec<PlayerId> = (1..=4).map(pid).collect();
        let mut round = Round::start(ps.clone(), 4, 0);
        // Cumulative volatility: 2, then 5 (>4) → crossing at the 2nd card (play index 1).
        round.apply_wave(
            &reg,
            WaveInput {
                committed: vec![(ps[0], card(1, 2)), (ps[1], card(2, 3))],
                passers: vec![ps[2], ps[3]],
                emptied: vec![],
                recalls: HashMap::new(),
            },
        );
        let d = round.depile();
        assert_eq!(d.reveals.len(), 2);
        // Reversed: index 0 is the last-added (crossing) card.
        assert_eq!(d.crossing_index, Some(0));
        assert_eq!(d.boiling_point, 4);
    }
}
