//! A single round as a sequence of simultaneous waves.
//!
//! The `Round` owns the pot, the primed Actives, the Quench window, and the
//! active/locked-out bookkeeping, and decides when the round ends — explosion,
//! everyone-remaining-passed, or the one-player-final-wave guard. It does NOT
//! own hands or decks: the caller (the game runner) validates choices against
//! hands, removes committed cards, draws Forage spells, and tells the round who
//! played / passed / can no longer act.

use std::collections::{HashMap, HashSet};

use boiling_point_protocol::vocab::{Color, SpellTarget};
use boiling_point_protocol::{CardId, PlayerId};

use crate::content::spell::SpellValues;

use super::card::Ingredient;
use super::pot::{Pot, PotIngredient};
use super::spells::{CastCommit, PrimedSpell, WaveSpellOutcome, resolve_wave_spells};

/// How a round ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundEnd {
    /// Volatility crossed the boiling point.
    Exploded,
    /// The pot settled safely (everyone locked out, or the final wave passed).
    Settled,
}

/// One player's ingredient-or-pass choice in a wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WaveAction {
    /// Commit a specific ingredient.
    Play {
        /// The hand ingredient committed.
        card: CardId,
        /// Whether it is played colorless (volatility only, zero points).
        colorless: bool,
    },
    /// Pass (permanent lockout for the round).
    Pass,
}

/// One player's optional spell cast in a wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpellChoice {
    /// The grimoire spell cast.
    pub spell: CardId,
    /// The chosen target, when the spell requires one.
    pub target: Option<SpellTarget>,
}

/// One player's full choice for a wave: the mandatory ingredient-or-pass plus
/// up to one spell. Serializable so the per-game action log can ride in a
/// timeless replay payload (the deterministic input the engine re-runs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WaveChoice {
    /// Play an ingredient or pass.
    pub action: WaveAction,
    /// The optional spell (a spell never substitutes for the action and never
    /// keeps a passed player active).
    pub spell: Option<SpellChoice>,
}

impl WaveChoice {
    /// A bare pass with no spell (the auto-pass / fallback choice).
    pub fn pass() -> Self {
        WaveChoice {
            action: WaveAction::Pass,
            spell: None,
        }
    }
}

/// The caller's resolved input for a wave: cards already removed from hands,
/// spell commits already validated, who passed, and who can no longer act
/// (hand and pantry both empty).
pub struct WaveInput {
    /// Players who played, with the ingredient and how it was played.
    pub committed: Vec<(PlayerId, Ingredient, bool)>,
    /// Validated spell commits (at most one per player).
    pub spells: Vec<CastCommit>,
    /// Active players who passed or timed out this wave.
    pub passers: Vec<PlayerId>,
    /// Players who played but cannot act next wave (no ingredients anywhere).
    pub exhausted: Vec<PlayerId>,
}

/// What happened in a wave.
pub struct WaveReport {
    /// The spell resolver's outcome (visible casts, private reads, forage owes).
    pub outcome: WaveSpellOutcome,
    /// Whether this wave's explosion check fired.
    pub exploded: bool,
    /// `Some` if this wave ended the round.
    pub ended: Option<RoundEnd>,
}

/// One entry of the end-of-round depile, in ascending effective-volatility order.
#[derive(Debug, Clone, Copy)]
pub struct DepileItem {
    /// Who played the ingredient.
    pub player: PlayerId,
    /// The revealed ingredient.
    pub ingredient: Ingredient,
    /// Whether it was played colorless.
    pub colorless: bool,
    /// The wave it landed in.
    pub wave_number: u8,
    /// Cumulative volatility after this entry in the sorted climb.
    pub running_volatility: i32,
    /// Whether this entry is liable for the explosion.
    pub liable: bool,
}

/// The data needed to render the end-of-round depile. The boiling point is
/// revealed **every** round (boom and safe) — the near-miss payoff.
#[derive(Debug, Clone)]
pub struct DepileData {
    /// Entries in ascending effective-volatility order (the fuse climb).
    pub reveals: Vec<DepileItem>,
    /// Index of the entry where the sorted climb first crossed the boiling
    /// point (`None` on a safe brew).
    pub crossing_index: Option<usize>,
    /// The revealed boiling point.
    pub boiling_point: u8,
}

/// A round in progress.
pub struct Round {
    /// Effective (post-modifier) boiling point — hidden from players in-round.
    boiling_point: i32,
    /// The accumulating pot.
    pot: Pot,
    /// Primed Active spells (hidden until they fire).
    primed: Vec<PrimedSpell>,
    /// Ingredients Skim pulled out of the pot (returned to pantries at settle).
    removed: Vec<(PlayerId, Ingredient)>,
    /// Whether a Quench shields the **next** wave's explosion check.
    quench_next_wave: bool,
    /// The wave whose check exploded, if any (detonator liability is scoped to it).
    fatal_wave: Option<u8>,
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
    /// Begin a round. `active` are the players who start with ingredients, in
    /// order; `start_volatility` seeds the pot (e.g. from Residue).
    pub fn start(active: Vec<PlayerId>, boiling_point: i32, start_volatility: i32) -> Self {
        Round {
            boiling_point,
            pot: Pot::new(start_volatility),
            primed: Vec::new(),
            removed: Vec::new(),
            quench_next_wave: false,
            fatal_wave: None,
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

    /// The final pot (for scoring once the round ends).
    pub fn pot(&self) -> &Pot {
        &self.pot
    }

    /// The primed Actives, mutably (resolution fires them at settle).
    pub fn primed_mut(&mut self) -> &mut Vec<PrimedSpell> {
        &mut self.primed
    }

    /// The pot and the primed Actives together — a split borrow so resolution
    /// can read the pot while firing wards.
    pub fn pot_and_primed_mut(&mut self) -> (&Pot, &mut Vec<PrimedSpell>) {
        (&self.pot, &mut self.primed)
    }

    /// Ingredients Skim removed during the round (returned to pantries at settle).
    pub fn removed(&self) -> &[(PlayerId, Ingredient)] {
        &self.removed
    }

    /// Apply one wave: land the committed ingredients, resolve the spell
    /// commits, run the explosion check (honouring a Quench window), then
    /// update the active set and decide whether the round ends.
    pub fn apply_wave(&mut self, values: &SpellValues, input: WaveInput) -> WaveReport {
        debug_assert!(self.is_open(), "wave applied to a finished round");

        // A Quench cast last wave shields this wave's check.
        let quench_shield = self.quench_next_wave;
        self.quench_next_wave = false;

        // Freeze the pre-wave per-colour snapshot (what score spells read).
        let snapshot: HashMap<Color, u32> = Color::PLAYER_COLORS
            .into_iter()
            .map(|c| (c, self.pot.color_points(c)))
            .collect();

        // Land this wave's ingredients.
        let played: Vec<PlayerId> = input.committed.iter().map(|(p, _, _)| *p).collect();
        for (player, ingredient, colorless) in input.committed {
            self.pot.cards.push(PotIngredient {
                player,
                ingredient,
                colorless,
                wave_number: self.wave_number,
                exposed: false,
            });
        }

        // Resolve the wave's spells (Instants fire, Actives prime).
        let outcome = resolve_wave_spells(
            &mut self.pot,
            &snapshot,
            input.spells,
            &mut self.primed,
            values,
        );
        if outcome.quenched {
            self.quench_next_wave = true;
        }
        self.removed.extend(outcome.skimmed.iter().copied());

        // The single explosion check, on the running total (cards + spell deltas).
        let exploded = !quench_shield && self.pot.total_volatility() > self.boiling_point;
        if exploded {
            self.fatal_wave = Some(self.wave_number);
        }

        // Players still active after this wave: those who played and can act again.
        let played_set: HashSet<PlayerId> = played.iter().copied().collect();
        let exhausted: HashSet<PlayerId> = input.exhausted.iter().copied().collect();
        let next: Vec<PlayerId> = self
            .active
            .iter()
            .copied()
            .filter(|p| played_set.contains(p) && !exhausted.contains(p))
            .collect();

        let ended = if exploded {
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
        WaveReport {
            outcome,
            exploded,
            ended,
        }
    }

    /// The detonators: only the **fatal wave's** ingredients, applied on top of
    /// the hidden pre-wave base (everything else in the running total — earlier
    /// waves' cards, the start volatility, and all spell deltas), sorted
    /// ascending by effective volatility. The trigger is the first card pushing
    /// the cumulative past the boiling point; its player and every player in
    /// that wave holding an equal-or-heavier card are liable. Empty on a safe
    /// brew, and empty if the fatal wave landed no ingredients (a parting
    /// Surge) — then nobody pays.
    pub fn detonators(&self) -> Vec<PlayerId> {
        let (players, _) = self.liability();
        players
    }

    /// Liability computation shared by [`Round::detonators`] and the depile's
    /// per-entry flags: the liable players and the liable card ids.
    fn liability(&self) -> (Vec<PlayerId>, HashSet<CardId>) {
        let Some(fatal) = self.fatal_wave else {
            return (Vec::new(), HashSet::new());
        };
        let mut fatal_cards: Vec<&PotIngredient> = self
            .pot
            .cards
            .iter()
            .filter(|p| p.wave_number == fatal)
            .collect();
        if fatal_cards.is_empty() {
            return (Vec::new(), HashSet::new());
        }
        fatal_cards.sort_by_key(|p| p.effective_volatility());

        // The hidden pre-wave base: everything in the running total except the
        // fatal wave's cards (cauldron-level spell deltas sit in the base, not
        // the per-card sort).
        let fatal_sum: i32 = fatal_cards
            .iter()
            .map(|p| p.effective_volatility() as i32)
            .sum();
        let base = self.pot.start_volatility
            + self.pot.spell_volatility_delta
            + (self
                .pot
                .cards
                .iter()
                .map(|p| p.effective_volatility() as i32)
                .sum::<i32>()
                - fatal_sum);

        // Walk the ascending sort to find the trigger volatility.
        let mut cumulative = base;
        let mut trigger_vol: Option<u8> = None;
        for card in &fatal_cards {
            cumulative += card.effective_volatility() as i32;
            if cumulative > self.boiling_point {
                trigger_vol = Some(card.effective_volatility());
                break;
            }
        }
        let Some(trigger_vol) = trigger_vol else {
            // The check fired but the climb never crosses card-by-card — only
            // possible when a spell delta alone tipped the total. Nobody pays.
            return (Vec::new(), HashSet::new());
        };

        // Trigger + every equal-or-heavier card in the fatal wave is liable
        // (equal volatilities are simultaneous).
        let mut players: Vec<PlayerId> = Vec::new();
        let mut cards: HashSet<CardId> = HashSet::new();
        for card in &fatal_cards {
            if card.effective_volatility() >= trigger_vol {
                cards.insert(card.ingredient.id);
                if !players.contains(&card.player) {
                    players.push(card.player);
                }
            }
        }
        (players, cards)
    }

    /// Build the depile: the whole pot sorted **ascending by effective
    /// volatility** (stable by play order), the running climb starting from the
    /// hidden base (start volatility + spell deltas), the boiling point
    /// revealed, and — on a boom — the crossing point and the liable entries
    /// marked.
    pub fn depile(&self) -> DepileData {
        let exploded = self.ended == Some(RoundEnd::Exploded);
        let (_, liable_cards) = self.liability();

        let mut sorted: Vec<&PotIngredient> = self.pot.cards.iter().collect();
        sorted.sort_by_key(|p| p.effective_volatility());

        let base = (self.pot.start_volatility + self.pot.spell_volatility_delta).max(0);
        let mut running = base;
        let mut reveals: Vec<DepileItem> = Vec::with_capacity(sorted.len());
        let mut crossing_index: Option<usize> = None;
        for (i, pc) in sorted.iter().enumerate() {
            running += pc.effective_volatility() as i32;
            if exploded && crossing_index.is_none() && running > self.boiling_point {
                crossing_index = Some(i);
            }
            reveals.push(DepileItem {
                player: pc.player,
                ingredient: pc.ingredient,
                colorless: pc.colorless,
                wave_number: pc.wave_number,
                running_volatility: running,
                liable: liable_cards.contains(&pc.ingredient.id),
            });
        }
        DepileData {
            reveals,
            crossing_index,
            boiling_point: self.boiling_point.max(0) as u8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::card::Spell;
    use boiling_point_protocol::vocab::SpellKind;
    use uuid::Uuid;

    fn pid(n: u128) -> PlayerId {
        PlayerId(Uuid::from_u128(n))
    }

    fn values() -> SpellValues {
        SpellValues {
            dampen: 3,
            surge: 3,
            cap_max: 3,
            hex_bonus: 5,
            harvest_bonus: 3,
            forage_draws: 2,
        }
    }

    fn ing(id: u32, vol: u8) -> Ingredient {
        Ingredient {
            id: CardId(id),
            color: Color::Ruby,
            volatility: vol,
            points: 1,
        }
    }

    fn wave(committed: Vec<(PlayerId, Ingredient, bool)>, passers: Vec<PlayerId>) -> WaveInput {
        WaveInput {
            committed,
            spells: vec![],
            passers,
            exhausted: vec![],
        }
    }

    #[test]
    fn explosion_ends_the_round() {
        let ps: Vec<PlayerId> = (1..=4).map(pid).collect();
        let mut round = Round::start(ps.clone(), 3, 0);
        let report = round.apply_wave(
            &values(),
            wave(vec![(ps[0], ing(1, 5), false)], vec![ps[1], ps[2], ps[3]]),
        );
        assert_eq!(report.ended, Some(RoundEnd::Exploded));
        assert!(!round.is_open());
        assert_eq!(round.detonators(), vec![ps[0]]);
    }

    #[test]
    fn everyone_passing_settles() {
        let ps: Vec<PlayerId> = (1..=4).map(pid).collect();
        let mut round = Round::start(ps.clone(), 20, 0);
        let report = round.apply_wave(&values(), wave(vec![], ps.clone()));
        assert_eq!(report.ended, Some(RoundEnd::Settled));
    }

    #[test]
    fn shrinking_field_triggers_one_player_final_wave() {
        let ps: Vec<PlayerId> = (1..=4).map(pid).collect();
        let mut round = Round::start(ps.clone(), 50, 0);

        // Wave 1: players 1 & 2 play, 3 & 4 pass → 2 active remain.
        let r1 = round.apply_wave(
            &values(),
            wave(
                vec![(ps[0], ing(1, 1), false), (ps[1], ing(2, 1), false)],
                vec![ps[2], ps[3]],
            ),
        );
        assert_eq!(r1.ended, None);
        assert_eq!(round.active(), &[ps[0], ps[1]]);

        // Wave 2: player 1 plays, player 2 passes → only 1 active → final wave granted.
        let r2 = round.apply_wave(
            &values(),
            wave(vec![(ps[0], ing(3, 1), false)], vec![ps[1]]),
        );
        assert_eq!(r2.ended, None, "the lone survivor gets one final wave");
        assert_eq!(round.active(), &[ps[0]]);

        // Wave 3 (the final wave): player 1 plays once more → round settles.
        let r3 = round.apply_wave(&values(), wave(vec![(ps[0], ing(4, 1), false)], vec![]));
        assert_eq!(r3.ended, Some(RoundEnd::Settled));
    }

    /// The keystone (design O1): the fatal wave sorts ascending; the trigger and
    /// every heavier card split the loss; lighter fatal-wave cards — and every
    /// earlier wave — are exempt.
    #[test]
    fn detonators_are_the_heavy_fatal_wave_cards() {
        let ps: Vec<PlayerId> = (1..=4).map(pid).collect();
        // Boiling point 10. Wave 1: total 6 (safe). Wave 2: cards 1, 4, 6 on
        // base 6 → sorted climb 7, 11 (crosses at the 4) → 4 and 6 are liable.
        let mut round = Round::start(ps.clone(), 10, 0);
        let r1 = round.apply_wave(
            &values(),
            wave(
                vec![(ps[0], ing(1, 3), false), (ps[1], ing(2, 3), false)],
                vec![],
            ),
        );
        assert_eq!(r1.ended, None);
        let r2 = round.apply_wave(
            &values(),
            wave(
                vec![
                    (ps[0], ing(3, 1), false),
                    (ps[1], ing(4, 4), false),
                    (ps[2], ing(5, 6), false),
                ],
                vec![],
            ),
        );
        assert_eq!(r2.ended, Some(RoundEnd::Exploded));
        assert_eq!(round.detonators(), vec![ps[1], ps[2]]);
        // The lighter fatal-wave card (vol 1) and the wave-1 players are exempt.
        assert!(!round.detonators().contains(&ps[0]));
    }

    /// Folding before the fatal wave is safe — earlier heavy cards carry no
    /// liability.
    #[test]
    fn folding_before_the_fatal_wave_is_safe() {
        let ps: Vec<PlayerId> = (1..=2).map(pid).collect();
        let mut round = Round::start(ps.clone(), 10, 0);
        // Wave 1: p1 plays a heavy 7 then will pass; p2 plays 1.
        round.apply_wave(
            &values(),
            wave(
                vec![(ps[0], ing(1, 7), false), (ps[1], ing(2, 1), false)],
                vec![],
            ),
        );
        // Wave 2: p1 passes; p2's 4 tips 8 → 12 past 10. Only p2 is liable.
        let r2 = round.apply_wave(
            &values(),
            wave(vec![(ps[1], ing(3, 4), false)], vec![ps[0]]),
        );
        assert_eq!(r2.ended, Some(RoundEnd::Exploded));
        assert_eq!(round.detonators(), vec![ps[1]]);
    }

    /// Equal volatilities are simultaneous: all are liable if one triggers.
    #[test]
    fn equal_volatility_cards_are_simultaneous() {
        let ps: Vec<PlayerId> = (1..=3).map(pid).collect();
        let mut round = Round::start(ps.clone(), 5, 0);
        let r = round.apply_wave(
            &values(),
            wave(
                vec![
                    (ps[0], ing(1, 3), false),
                    (ps[1], ing(2, 3), false),
                    (ps[2], ing(3, 1), false),
                ],
                vec![],
            ),
        );
        assert_eq!(r.ended, Some(RoundEnd::Exploded));
        // Climb: 1 → 4 (3 triggers past 5? 1+3=4, +3=7 > 5: the second 3 triggers;
        // both 3s are equal → simultaneous; the 1 is exempt.
        let dets = round.detonators();
        assert!(dets.contains(&ps[0]) && dets.contains(&ps[1]));
        assert!(!dets.contains(&ps[2]));
    }

    /// Quench shields exactly the next wave's check, then terror resumes.
    #[test]
    fn quench_shields_the_next_wave_only() {
        let ps: Vec<PlayerId> = (1..=2).map(pid).collect();
        let mut round = Round::start(ps.clone(), 5, 0);
        // Wave 1: p1 plays small + casts Quench.
        let r1 = round.apply_wave(
            &values(),
            WaveInput {
                committed: vec![(ps[0], ing(1, 1), false), (ps[1], ing(2, 1), false)],
                spells: vec![CastCommit {
                    player: ps[0],
                    spell: Spell {
                        id: CardId(100),
                        kind: SpellKind::Quench,
                    },
                    target: None,
                }],
                passers: vec![],
                exhausted: vec![],
            },
        );
        assert_eq!(r1.ended, None);
        // Wave 2 (quenched): a huge overload does NOT explode.
        let r2 = round.apply_wave(
            &values(),
            wave(
                vec![(ps[0], ing(3, 7), false), (ps[1], ing(4, 7), false)],
                vec![],
            ),
        );
        assert_eq!(r2.ended, None, "the quenched wave cannot explode");
        // Wave 3: the shield is gone; the pre-wave base already exceeds the
        // boiling point, so any card triggers — the whole fatal wave is liable.
        let r3 = round.apply_wave(
            &values(),
            wave(vec![(ps[0], ing(5, 0), false)], vec![ps[1]]),
        );
        assert_eq!(r3.ended, Some(RoundEnd::Exploded));
        assert_eq!(round.detonators(), vec![ps[0]]);
    }

    /// The depile sorts ascending by volatility, reveals the boiling point on a
    /// SAFE brew too, and marks no liability or crossing.
    #[test]
    fn safe_depile_reveals_the_near_miss() {
        let ps: Vec<PlayerId> = (1..=2).map(pid).collect();
        let mut round = Round::start(ps.clone(), 10, 0);
        round.apply_wave(
            &values(),
            wave(
                vec![(ps[0], ing(1, 4), false), (ps[1], ing(2, 2), false)],
                vec![],
            ),
        );
        // Both pass → settles.
        round.apply_wave(&values(), wave(vec![], ps.clone()));
        let d = round.depile();
        assert_eq!(d.boiling_point, 10, "the line is revealed on a safe brew");
        assert_eq!(d.crossing_index, None);
        // Ascending: vol 2 then vol 4; the climb stops short of the line.
        assert_eq!(d.reveals[0].ingredient.volatility, 2);
        assert_eq!(d.reveals[1].ingredient.volatility, 4);
        assert_eq!(d.reveals[1].running_volatility, 6);
        assert!(d.reveals.iter().all(|r| !r.liable));
    }

    /// On a boom the depile marks the crossing and the liable entries.
    #[test]
    fn boom_depile_marks_crossing_and_culprits() {
        let ps: Vec<PlayerId> = (1..=3).map(pid).collect();
        let mut round = Round::start(ps.clone(), 4, 0);
        round.apply_wave(
            &values(),
            wave(
                vec![
                    (ps[0], ing(1, 2), false),
                    (ps[1], ing(2, 3), false),
                    (ps[2], ing(3, 1), false),
                ],
                vec![],
            ),
        );
        let d = round.depile();
        // Sorted: 1, 2, 3 → running 1, 3, 6; crossing at index 2 (6 > 4).
        assert_eq!(d.crossing_index, Some(2));
        assert_eq!(d.boiling_point, 4);
        // Trigger climb in the fatal wave: 1 → 3 → 6 crosses at the 3 (vol 3);
        // liable = vol ≥ 3 → only the vol-3 card.
        let liable: Vec<u8> = d
            .reveals
            .iter()
            .filter(|r| r.liable)
            .map(|r| r.ingredient.volatility)
            .collect();
        assert_eq!(liable, vec![3]);
    }
}
