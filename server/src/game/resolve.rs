//! The within-wave effect resolver: applies a wave's cards and effects against a
//! frozen pre-wave snapshot, in the fixed 7-step order, then performs the single
//! explosion check.
//!
//! Snapshot semantics: Copycat/Double Down read the pot as it stood *before* this
//! wave, so same-wave cards don't see each other and duplicate effects sum
//! (two same-colour Double Downs → ×3, order-independent — D-R3). The acting
//! card is tracked by stable [`CardId`], so removals (Recall) never disturb other
//! effects' lookups.

use std::collections::{HashMap, HashSet};

use boiling_point_protocol::vocab::{CardView, Color};
use boiling_point_protocol::{CardId, PlayerId};

use crate::content::effect::EffectCtx;
use crate::content::ContentRegistry;

use super::card::Card;
use super::pot::{Pot, PotCard};

/// Side-effects produced by resolving a wave, beyond the pot mutation itself.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct WaveOutcome {
    /// Whether the post-effect volatility exceeded the boiling point.
    pub exploded: bool,
    /// Players who should privately receive the boiling point (played Peek).
    pub peeked: Vec<PlayerId>,
    /// Cards revealed to the whole table (by Expose).
    pub exposed: Vec<CardView>,
    /// Cards pulled back out of the pot (by Recall) — returned to their owners' hands.
    pub recalled: Vec<(PlayerId, Card)>,
}

/// Mutable resolution state for one wave. Does NOT hold the registry, so a
/// borrowed effect strategy can call back into it freely.
struct WaveResolution<'a> {
    pot: &'a mut Pot,
    snapshot_points: HashMap<Color, u32>,
    snapshot_dominant: Option<Color>,
    shielded: &'a mut HashSet<PlayerId>,
    recalls: &'a HashMap<PlayerId, CardId>,
    peeked: Vec<PlayerId>,
    exposed: Vec<CardView>,
    recalled: Vec<(PlayerId, Card)>,
    cur_card: CardId,
    cur_player: PlayerId,
}

impl WaveResolution<'_> {
    /// Index of the currently-resolving effect's card in the pot.
    fn cur_index(&self) -> Option<usize> {
        self.pot
            .cards
            .iter()
            .position(|p| p.card.id == self.cur_card)
    }
}

impl EffectCtx for WaveResolution<'_> {
    fn card_color(&self) -> Color {
        self.cur_index()
            .map(|i| self.pot.cards[i].color)
            .unwrap_or(Color::Wild)
    }

    fn adjust_volatility(&mut self, delta: i16) {
        self.pot.volatility = (self.pot.volatility + delta as i32).max(0);
    }

    fn reveal_boiling_point_to_actor(&mut self) {
        self.peeked.push(self.cur_player);
    }

    fn shield_actor(&mut self) {
        self.shielded.insert(self.cur_player);
    }

    fn expose_random_card(&mut self) {
        // Reveal the earliest-played card other than the Expose card itself
        // (deterministic; true randomness is not load-bearing for correctness).
        if let Some(pc) = self.pot.cards.iter().find(|p| p.card.id != self.cur_card) {
            self.exposed.push(pc.card.view());
        }
    }

    fn adopt_pre_wave_dominant_color(&mut self) {
        let color = self.snapshot_dominant.unwrap_or(Color::Wild);
        if let Some(i) = self.cur_index() {
            self.pot.cards[i].color = color;
        }
    }

    fn recall_own_card(&mut self) {
        let target = self.recalls.get(&self.cur_player).copied();
        // Prefer the explicitly targeted own card; else the player's earliest own
        // card — never the Recall card itself.
        let pos = self
            .pot
            .cards
            .iter()
            .position(|p| {
                p.player == self.cur_player
                    && p.card.id != self.cur_card
                    && Some(p.card.id) == target
            })
            .or_else(|| {
                self.pot
                    .cards
                    .iter()
                    .position(|p| p.player == self.cur_player && p.card.id != self.cur_card)
            });
        if let Some(pos) = pos {
            let removed = self.pot.cards.remove(pos);
            self.pot.volatility = (self.pot.volatility - removed.card.volatility as i32).max(0);
            self.recalled.push((removed.player, removed.card));
        }
    }

    fn double_pre_wave_color_points(&mut self, color: Color) {
        let snap = self.snapshot_points.get(&color).copied().unwrap_or(0);
        *self.pot.color_bonus.entry(color).or_insert(0) += snap;
    }
}

/// Resolve one wave: add the committed cards, run their effects in the fixed
/// category order against the pre-wave snapshot, then check for explosion.
pub fn resolve_wave(
    registry: &ContentRegistry,
    pot: &mut Pot,
    effective_boiling_point: i32,
    shielded: &mut HashSet<PlayerId>,
    committed: Vec<(PlayerId, Card)>,
    recalls: &HashMap<PlayerId, CardId>,
) -> WaveOutcome {
    // 1. Freeze the pre-wave snapshot (before this wave's cards land).
    let snapshot_dominant = pot.dominant_player_color();
    let snapshot_points: HashMap<Color, u32> = Color::PLAYER_COLORS
        .into_iter()
        .map(|c| (c, pot.color_points(c)))
        .collect();

    // 2. Add committed cards; collect their effects.
    let mut effects: Vec<(CardId, PlayerId, boiling_point_protocol::vocab::EffectKind)> =
        Vec::new();
    for (player, card) in committed {
        pot.cards.push(PotCard {
            player,
            card,
            color: card.color,
            points: card.points,
        });
        pot.volatility += card.volatility as i32;
        if let Some(effect) = card.effect {
            effects.push((card.id, player, effect));
        }
    }

    // 3. Order effects by category (the fixed 7-step order).
    effects.sort_by_key(|(_, _, kind)| {
        registry
            .effect(*kind)
            .map(|b| b.category())
            .unwrap_or(crate::content::EffectCategory::Information)
    });

    // 4. Resolve each effect against the wave state.
    let mut wave = WaveResolution {
        pot,
        snapshot_points,
        snapshot_dominant,
        shielded,
        recalls,
        peeked: Vec::new(),
        exposed: Vec::new(),
        recalled: Vec::new(),
        cur_card: CardId(0),
        cur_player: PlayerId(uuid::Uuid::nil()),
    };
    for (card_id, player, kind) in effects {
        if let Some(behavior) = registry.effect(kind) {
            wave.cur_card = card_id;
            wave.cur_player = player;
            behavior.resolve(&mut wave);
        }
    }

    // 5. Single explosion check on the true post-effect total.
    let exploded = wave.pot.volatility > effective_boiling_point;
    WaveOutcome {
        exploded,
        peeked: wave.peeked,
        exposed: wave.exposed,
        recalled: wave.recalled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::vocab::EffectKind;
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

    fn card(id: u32, color: Color, vol: u8, pts: u8, effect: Option<EffectKind>) -> Card {
        Card {
            id: CardId(id),
            color,
            volatility: vol,
            points: pts,
            effect,
        }
    }

    #[test]
    fn dampen_and_surge_adjust_volatility_and_explosion_check() {
        let reg = registry();
        let mut pot = Pot::new(0);
        let mut shielded = HashSet::new();
        let recalls = HashMap::new();
        // A plain vol-3 card + a Volatile Surge (base vol 3, +2) → 8 volatility.
        let committed = vec![
            (pid(1), card(1, Color::Ruby, 3, 1, None)),
            (
                pid(2),
                card(2, Color::Emerald, 3, 0, Some(EffectKind::VolatileSurge)),
            ),
        ];
        let out = resolve_wave(&reg, &mut pot, 7, &mut shielded, committed, &recalls);
        assert_eq!(pot.volatility, 8); // 3 + 3 + 2(surge)
        assert!(out.exploded); // 8 > 7
    }

    #[test]
    fn two_same_colour_double_downs_sum_to_triple() {
        let reg = registry();
        let mut pot = Pot::new(0);
        let mut shielded = HashSet::new();
        let recalls = HashMap::new();
        // Pre-wave: a Ruby card worth 3.
        resolve_wave(
            &reg,
            &mut pot,
            99,
            &mut shielded,
            vec![(pid(1), card(1, Color::Ruby, 1, 3, None))],
            &recalls,
        );
        assert_eq!(pot.color_points(Color::Ruby), 3);
        // This wave: two Ruby Double Downs (0 points each) → +3 +3 = +6 bonus.
        let out = resolve_wave(
            &reg,
            &mut pot,
            99,
            &mut shielded,
            vec![
                (
                    pid(1),
                    card(2, Color::Ruby, 1, 0, Some(EffectKind::DoubleDown)),
                ),
                (
                    pid(2),
                    card(3, Color::Ruby, 1, 0, Some(EffectKind::DoubleDown)),
                ),
            ],
            &recalls,
        );
        assert!(!out.exploded);
        assert_eq!(pot.color_points(Color::Ruby), 9); // 3 base + 6 = 3×
    }

    #[test]
    fn copycat_adopts_pre_wave_dominant_colour() {
        let reg = registry();
        let mut pot = Pot::new(0);
        let mut shielded = HashSet::new();
        let recalls = HashMap::new();
        // Pre-wave: Ruby dominant.
        resolve_wave(
            &reg,
            &mut pot,
            99,
            &mut shielded,
            vec![(pid(1), card(1, Color::Ruby, 1, 3, None))],
            &recalls,
        );
        // Copycat card printed Amethyst → should become Ruby.
        resolve_wave(
            &reg,
            &mut pot,
            99,
            &mut shielded,
            vec![(
                pid(2),
                card(2, Color::Amethyst, 1, 1, Some(EffectKind::Copycat)),
            )],
            &recalls,
        );
        let copycat = pot.cards.iter().find(|p| p.card.id == CardId(2)).unwrap();
        assert_eq!(copycat.color, Color::Ruby);
    }

    #[test]
    fn shield_marks_actor() {
        let reg = registry();
        let mut pot = Pot::new(0);
        let mut shielded = HashSet::new();
        let recalls = HashMap::new();
        resolve_wave(
            &reg,
            &mut pot,
            99,
            &mut shielded,
            vec![(
                pid(1),
                card(1, Color::Amethyst, 2, 0, Some(EffectKind::Shield)),
            )],
            &recalls,
        );
        assert!(shielded.contains(&pid(1)));
    }

    #[test]
    fn recall_removes_own_previous_card_and_its_volatility() {
        let reg = registry();
        let mut pot = Pot::new(0);
        let mut shielded = HashSet::new();
        let recalls = HashMap::new();
        // Previous wave: player 1 plays a vol-3 card.
        resolve_wave(
            &reg,
            &mut pot,
            99,
            &mut shielded,
            vec![(pid(1), card(1, Color::Ruby, 3, 2, None))],
            &recalls,
        );
        assert_eq!(pot.volatility, 3);
        // This wave: player 1 plays Recall (vol 1) → pulls back card 1.
        resolve_wave(
            &reg,
            &mut pot,
            99,
            &mut shielded,
            vec![(pid(1), card(2, Color::Ruby, 1, 0, Some(EffectKind::Recall)))],
            &recalls,
        );
        // 3 (card1) + 1 (recall) - 3 (recalled card1) = 1.
        assert_eq!(pot.volatility, 1);
        assert!(pot.cards.iter().all(|p| p.card.id != CardId(1)));
    }
}
