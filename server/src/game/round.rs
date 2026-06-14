//! A single round as a sequence of simultaneous waves.
//!
//! The `Round` owns the pot, the primed Actives, the Quench window, and the
//! active/locked-out bookkeeping, and decides when the round ends — explosion,
//! everyone-remaining-passed, or the one-player-final-wave guard. It does NOT
//! own hands or decks: the caller (the game runner) validates choices against
//! hands, removes committed cards, draws Forage spells, and tells the round who
//! played / passed / can no longer act.

use std::collections::{HashMap, HashSet};

use boiling_point_protocol::server::CompoundingFire;
use boiling_point_protocol::vocab::{Color, SpellTarget};
use boiling_point_protocol::{CardId, PlayerId};

use crate::content::spell::SpellValues;

use super::card::Ingredient;
use super::compounding::{self, CompoundingBrewers};
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
/// up to one spell — two for the Channeler (`boom2-brewers`), whose second
/// cast rides `second_spell`. Serializable so the per-game action log can ride
/// in a timeless replay payload (the deterministic input the engine re-runs;
/// `second_spell` defaults to `None` so pre-brewer payloads still decode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WaveChoice {
    /// Play an ingredient or pass.
    pub action: WaveAction,
    /// The optional spell (a spell never substitutes for the action and never
    /// keeps a passed player active).
    pub spell: Option<SpellChoice>,
    /// The Channeler's optional second cast (dropped for everyone else).
    #[serde(default)]
    pub second_spell: Option<SpellChoice>,
}

impl WaveChoice {
    /// A bare pass with no spell (the auto-pass / fallback choice).
    pub fn pass() -> Self {
        WaveChoice {
            action: WaveAction::Pass,
            spell: None,
            second_spell: None,
        }
    }
}

/// The caller's resolved input for a wave: cards already removed from hands,
/// spell commits already validated, who passed, and who can no longer act
/// (hand and pantry both empty).
pub struct WaveInput {
    /// Players who played, with the ingredient and how it was played.
    pub committed: Vec<(PlayerId, Ingredient, bool)>,
    /// Validated spell commits (at most one per player; two for a Channeler).
    pub spells: Vec<CastCommit>,
    /// Active players who passed or timed out this wave.
    pub passers: Vec<PlayerId>,
    /// Players who played but cannot act next wave (no ingredients anywhere).
    pub exhausted: Vec<PlayerId>,
}

impl WaveInput {
    /// An input that lands nothing — the no-late-commit case of a staged wave.
    pub fn empty() -> Self {
        WaveInput {
            committed: Vec::new(),
            spells: Vec::new(),
            passers: Vec::new(),
            exhausted: Vec::new(),
        }
    }
}

/// The half-applied state of a staged (Lurker-deferred) wave between
/// [`Round::apply_wave_partial`] and [`Round::finalize_wave`]: who has acted
/// so far and whether a Quench shield was consumed at the wave's start.
struct StagedWave {
    /// Players whose ingredients landed in the partial step.
    played: Vec<PlayerId>,
    /// Players who passed in the partial step.
    passers: Vec<PlayerId>,
    /// Players exhausted by the partial step.
    exhausted: Vec<PlayerId>,
    /// Whether a Quench shields this wave's (deferred) explosion check.
    quench_shield: bool,
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
    /// Cumulative volatility after this entry in the sorted climb (includes any
    /// combo-added volatility).
    pub running_volatility: i32,
    /// Whether this entry is liable for the explosion.
    pub liable: bool,
    /// The compounding that fired on this card, if any (`boom2-compounding`).
    pub compounding: Option<CompoundingFire>,
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
    /// Players whose cards count as the lowest at their value in the
    /// fatal-wave sort (the Featherhand bend, `boom2-brewers`).
    featherhands: HashSet<PlayerId>,
    /// The compounding Brewer seats (`boom2-compounding`): the Herbalist /
    /// Distiller / Alchemist bends consulted when the pot's compounding is
    /// recomputed. Empty when no brewer phase ran (combos/thresholds still fire,
    /// just without the bends).
    compounding: CompoundingBrewers,
    /// Each seat's anchor colour, so a combo credits its **owner's** colour
    /// (cross-colour safe, `boom2-compounding`). Empty in brewerless sync tests
    /// — combos then credit the completing card's own colour.
    player_color: HashMap<PlayerId, Color>,
    /// The half-applied state of a staged (Lurker-deferred) wave, between the
    /// partial step and its finalize.
    staged: Option<StagedWave>,
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
            featherhands: HashSet::new(),
            compounding: CompoundingBrewers::default(),
            player_color: HashMap::new(),
            staged: None,
        }
    }

    /// Mark which players hold the Featherhand bend (their cards sort lowest
    /// at their value in the fatal-wave liability sort).
    pub fn with_featherhands(mut self, featherhands: HashSet<PlayerId>) -> Self {
        self.featherhands = featherhands;
        self
    }

    /// Supply the compounding Brewer seats (`boom2-compounding`): the Herbalist /
    /// Distiller / Alchemist bends consulted each time the pot's compounding is
    /// recomputed.
    pub fn with_compounding(mut self, compounding: CompoundingBrewers) -> Self {
        self.compounding = compounding;
        self
    }

    /// Supply each seat's anchor colour, so a combo credits its owner's colour
    /// (cross-colour safe, `boom2-compounding`).
    pub fn with_player_color(mut self, player_color: HashMap<PlayerId, Color>) -> Self {
        self.player_color = player_color;
        self
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

    /// The primed Actives, read-only (the operator reveal's active-effects view).
    pub fn primed(&self) -> &[PrimedSpell] {
        &self.primed
    }

    /// Whether a Quench shields the next wave's explosion check.
    pub fn quench_pending(&self) -> bool {
        self.quench_next_wave
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
        let outcome = self.apply_wave_partial(values, input);
        let mut report = self.finalize_wave(values, WaveInput::empty());
        // The finalize's own outcome is empty (no late input); the wave's real
        // spell outcome is the partial one.
        report.outcome = outcome;
        report
    }

    /// Land one wave's `input` (cards + spells) **without** running the
    /// explosion check or closing the wave — the first step of a staged wave
    /// (the Lurker's deferred commit, `boom2-brewers`). The wave stays open at
    /// the same number until [`Round::finalize_wave`] lands the late commit
    /// and runs the deferred check, so the late card joins this same wave's
    /// pot and liability sort. A staged wave finalized with an empty late
    /// input is byte-identical to the one-shot [`Round::apply_wave`] — replays
    /// re-run staged waves as simultaneous ones.
    pub fn apply_wave_partial(
        &mut self,
        values: &SpellValues,
        input: WaveInput,
    ) -> WaveSpellOutcome {
        debug_assert!(self.is_open(), "wave applied to a finished round");
        debug_assert!(self.staged.is_none(), "a staged wave is already open");

        // A Quench cast last wave shields this wave's (possibly deferred) check.
        let quench_shield = self.quench_next_wave;
        self.quench_next_wave = false;

        let WaveInput {
            committed,
            spells,
            passers,
            exhausted,
        } = input;
        let (played, outcome) = self.land(values, committed, spells);
        self.staged = Some(StagedWave {
            played,
            passers,
            exhausted,
            quench_shield,
        });
        outcome
    }

    /// Land cards into the open wave and resolve spells against the pre-input
    /// snapshot. Shared by the partial and finalize steps.
    fn land(
        &mut self,
        values: &SpellValues,
        committed: Vec<(PlayerId, Ingredient, bool)>,
        spells: Vec<CastCommit>,
    ) -> (Vec<PlayerId>, WaveSpellOutcome) {
        // Freeze the pre-input per-colour snapshot (what score spells read).
        let snapshot: HashMap<Color, u32> = Color::PLAYER_COLORS
            .into_iter()
            .map(|c| (c, self.pot.color_points(c)))
            .collect();

        let played: Vec<PlayerId> = committed.iter().map(|(p, _, _)| *p).collect();
        for (player, ingredient, colorless) in committed {
            self.pot.cards.push(PotIngredient {
                player,
                ingredient,
                colorless,
                wave_number: self.wave_number,
                exposed: false,
                compounding: compounding::CardCompounding::default(),
            });
        }

        let outcome =
            resolve_wave_spells(&mut self.pot, &snapshot, spells, &mut self.primed, values);
        if outcome.quenched {
            self.quench_next_wave = true;
        }
        self.removed.extend(outcome.skimmed.iter().copied());

        // Recompute compounding now the pot has changed (cards landed, a Skim
        // may have left): combo-added volatility must feed this same wave's
        // explosion check, and the bonuses drive scoring and the depile.
        compounding::recompute(&mut self.pot.cards, &self.compounding, &self.player_color);

        (played, outcome)
    }

    /// Finalize the open (staged) wave: land the `late` input (the Lurker's
    /// post-reveal commit — or nothing), run the single explosion check on the
    /// full wave, update the active set, and decide whether the round ends.
    pub fn finalize_wave(&mut self, values: &SpellValues, late: WaveInput) -> WaveReport {
        let WaveInput {
            committed,
            spells,
            passers: late_passers,
            exhausted: late_exhausted,
        } = late;
        let (late_played, outcome) = self.land(values, committed, spells);
        let mut staged = self.staged.take().expect("a staged wave is open");
        staged.played.extend(late_played);
        staged.passers.extend(late_passers);
        staged.exhausted.extend(late_exhausted);
        let StagedWave {
            played,
            passers: _,
            exhausted,
            quench_shield,
        } = staged;

        // The single explosion check, on the running total (cards + spell deltas).
        let exploded = !quench_shield && self.pot.total_volatility() > self.boiling_point;
        if exploded {
            self.fatal_wave = Some(self.wave_number);
        }

        // Players still active after this wave: those who played and can act again.
        let played_set: HashSet<PlayerId> = played.iter().copied().collect();
        let exhausted: HashSet<PlayerId> = exhausted.into_iter().collect();
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

    /// The fatal-wave sort key: ascending effective volatility, with a
    /// Featherhand's cards counting as the **lowest at their value**
    /// (`boom2-brewers`) — they sort before equal-volatility cards and slip
    /// out of ties. With no Featherhand seated this is exactly the plain
    /// volatility sort.
    fn sort_key(&self, card: &PotIngredient) -> (u8, u8) {
        (
            card.effective_volatility(),
            u8::from(!self.featherhands.contains(&card.player)),
        )
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
        fatal_cards.sort_by_key(|p| self.sort_key(p));

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

        // Walk the ascending sort to find the trigger card's key.
        let mut cumulative = base;
        let mut trigger_key: Option<(u8, u8)> = None;
        for card in &fatal_cards {
            cumulative += card.effective_volatility() as i32;
            if cumulative > self.boiling_point {
                trigger_key = Some(self.sort_key(card));
                break;
            }
        }
        let Some(trigger_key) = trigger_key else {
            // The check fired but the climb never crosses card-by-card — only
            // possible when a spell delta alone tipped the total. Nobody pays.
            return (Vec::new(), HashSet::new());
        };

        // Trigger + everything at-or-after it in the sort is liable (equal
        // volatilities are simultaneous — except a Featherhand's, which sort
        // strictly below equal-volatility cards and so slip out of the tie
        // unless their own card is the trigger).
        let mut players: Vec<PlayerId> = Vec::new();
        let mut cards: HashSet<CardId> = HashSet::new();
        for card in &fatal_cards {
            if self.sort_key(card) >= trigger_key {
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
        sorted.sort_by_key(|p| self.sort_key(p));

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
                compounding: pc.compounding.fire,
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
    use boiling_point_protocol::vocab::{ComboId, Compounding, SpellKind};
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
            compounding: None,
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

    /// A member of the 2-ingredient SageMint combo (members 0 and 1).
    fn combo_ing(id: u32, vol: u8, member: u8) -> Ingredient {
        Ingredient {
            id: CardId(id),
            color: Color::Ruby,
            volatility: vol,
            points: 1,
            compounding: Some(Compounding::Combo {
                combo: ComboId::SageMint,
                member,
            }),
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

    /// The Featherhand bend (`boom2-brewers`): in the fatal-wave sort their
    /// cards count as the lowest at their value, so they slip out of the
    /// equal-volatility tie that would otherwise make them liable.
    #[test]
    fn featherhand_slips_out_of_ties() {
        let ps: Vec<PlayerId> = (1..=3).map(pid).collect();
        let mut round = Round::start(ps.clone(), 5, 0).with_featherhands(HashSet::from([ps[0]]));
        // Same wave as `equal_volatility_cards_are_simultaneous`: two 3s and a
        // 1 against boiling point 5 — but p1's 3 sorts below p2's, so the
        // climb (1 → 4 → 7) crosses on p2's card alone.
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
        assert_eq!(
            round.detonators(),
            vec![ps[1]],
            "the Featherhand slips the tie"
        );
    }

    /// A Featherhand whose own card triggers the crossing is still liable —
    /// the bend moves them out of ties, never out of causality.
    #[test]
    fn featherhand_still_pays_when_their_card_triggers() {
        let ps: Vec<PlayerId> = (1..=2).map(pid).collect();
        let mut round = Round::start(ps.clone(), 3, 0).with_featherhands(HashSet::from([ps[0]]));
        let r = round.apply_wave(
            &values(),
            wave(
                vec![(ps[0], ing(1, 3), false), (ps[1], ing(2, 1), false)],
                vec![],
            ),
        );
        assert_eq!(r.ended, Some(RoundEnd::Exploded));
        // Climb: p2's 1 (key 1,1) then p1's 3 (key 3,0) crosses 3 → 4 > 3.
        // The equal-or-heavier rule then catches nothing above (3,0) but p1.
        assert_eq!(round.detonators(), vec![ps[0]]);
    }

    /// A staged wave (the Lurker's deferred commit) finalized with the same
    /// cards is state-identical to the one-shot wave: same end, same
    /// detonators, same pot — replays may re-run it as simultaneous.
    #[test]
    fn staged_wave_matches_the_one_shot_wave() {
        let ps: Vec<PlayerId> = (1..=3).map(pid).collect();
        let run_one_shot = || {
            let mut round = Round::start(ps.clone(), 6, 0);
            let r = round.apply_wave(
                &values(),
                wave(
                    vec![
                        (ps[0], ing(1, 2), false),
                        (ps[1], ing(2, 3), false),
                        (ps[2], ing(3, 4), false),
                    ],
                    vec![],
                ),
            );
            (r.ended, round.detonators(), round.pot().total_volatility())
        };
        let run_staged = || {
            let mut round = Round::start(ps.clone(), 6, 0);
            // Stage A: everyone but the lurker (p3).
            let _ = round.apply_wave_partial(
                &values(),
                wave(
                    vec![(ps[0], ing(1, 2), false), (ps[1], ing(2, 3), false)],
                    vec![],
                ),
            );
            // The wave is still open at the same number; no check has run.
            assert!(round.is_open());
            assert_eq!(round.wave_number(), 1);
            // The late commit lands into the same wave; the check runs once.
            let r = round.finalize_wave(&values(), wave(vec![(ps[2], ing(3, 4), false)], vec![]));
            (r.ended, round.detonators(), round.pot().total_volatility())
        };
        assert_eq!(run_one_shot(), run_staged());
        // The late card is part of the fatal wave's liability sort: vol 4
        // triggers (2 → 5 → 9 crosses at the 4 past 6... climb 2, 5, 9: the 4
        // crosses), so the deferring player pays.
        let (_, detonators, _) = run_staged();
        assert!(
            detonators.contains(&ps[2]),
            "the late card carries liability"
        );
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

    /// An Alchemist's combo adds volatility that feeds the very wave's explosion
    /// check: the same cards explode with the Alchemist bend where they settle
    /// safely without it (`boom2-compounding` — chemistry as a weapon).
    #[test]
    fn alchemist_combo_volatility_tips_the_explosion_check() {
        let ps: Vec<PlayerId> = (1..=2).map(pid).collect();
        // Two combo halves (vol 3 each = 6) against boiling point 7: safe alone,
        // but an Alchemist's +2 combo volatility (→ 8) crosses the line.
        let cards = || {
            vec![
                (ps[0], combo_ing(1, 3, 0), false),
                (ps[0], combo_ing(2, 3, 1), false),
            ]
        };

        let mut plain = Round::start(ps.clone(), 7, 0);
        let r = plain.apply_wave(&values(), wave(cards(), vec![ps[1]]));
        assert_ne!(
            r.ended,
            Some(RoundEnd::Exploded),
            "6 ≤ 7: no Alchemist, safe"
        );
        assert_eq!(plain.pot().total_volatility(), 6);

        let alchemist = CompoundingBrewers {
            alchemists: vec![ps[0]],
            ..Default::default()
        };
        let mut round = Round::start(ps.clone(), 7, 0).with_compounding(alchemist);
        let r = round.apply_wave(&values(), wave(cards(), vec![ps[1]]));
        assert_eq!(
            r.ended,
            Some(RoundEnd::Exploded),
            "8 > 7: the combo tips it"
        );
        assert_eq!(round.detonators(), vec![ps[0]]);
        assert_eq!(round.pot().total_volatility(), 8);
    }

    /// The depile narrates a fired combo on its completing entry, and the
    /// running climb reflects the combo-added volatility.
    #[test]
    fn depile_narrates_a_fired_combo() {
        let ps: Vec<PlayerId> = (1..=2).map(pid).collect();
        let alchemist = CompoundingBrewers {
            alchemists: vec![ps[0]],
            ..Default::default()
        };
        let mut round = Round::start(ps.clone(), 50, 0).with_compounding(alchemist);
        round.apply_wave(
            &values(),
            wave(
                vec![
                    (ps[0], combo_ing(1, 2, 0), false),
                    (ps[0], combo_ing(2, 2, 1), false),
                ],
                vec![ps[1]],
            ),
        );
        let d = round.depile();
        let fired: Vec<CompoundingFire> = d.reveals.iter().filter_map(|r| r.compounding).collect();
        assert_eq!(
            fired,
            vec![CompoundingFire::Combo {
                size: 2,
                bonus_points: crate::game::compounding::combo_bonus(2),
                bonus_volatility: crate::game::brewers::ALCHEMIST_COMBO_VOLATILITY,
            }]
        );
        // Climb reflects the +2 combo volatility: 2 + (2 + 2) = 6.
        assert_eq!(d.reveals.last().unwrap().running_volatility, 6);
    }
}
