//! The cauldron-modifier escalation engine: a cumulative stack of active
//! modifiers whose pure offsets/multipliers compose by plain arithmetic, so
//! contradictions cancel with no special cases.

use boiling_point_protocol::vocab::ModifierKind;

use crate::content::ContentRegistry;

/// The modifiers active in a round (one drawn per round from round 2). Round N
/// (N ≥ 2) holds N−1 of these.
#[derive(Debug, Default, Clone)]
pub struct ActiveModifiers {
    kinds: Vec<ModifierKind>,
}

impl ActiveModifiers {
    /// An empty stack (round 1).
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a newly drawn modifier to the cumulative stack.
    pub fn push(&mut self, kind: ModifierKind) {
        self.kinds.push(kind);
    }

    /// The active modifier kinds, in draw order.
    pub fn kinds(&self) -> &[ModifierKind] {
        &self.kinds
    }

    /// Net additive shift to the boiling point (summed; opposites cancel).
    pub fn boiling_point_delta(&self, reg: &ContentRegistry) -> i32 {
        self.each(reg).map(|m| m.boiling_point_delta() as i32).sum()
    }

    /// Total starting volatility seeded into the cauldron (summed).
    pub fn start_volatility(&self, reg: &ContentRegistry) -> i32 {
        self.each(reg).map(|m| m.start_volatility() as i32).sum()
    }

    /// Colourless pot bonus per card played (summed).
    pub fn pot_bonus_per_card(&self, reg: &ContentRegistry) -> u32 {
        self.each(reg).map(|m| m.pot_point_bonus_per_card()).sum()
    }

    /// Multiplier applied to the final pot value (product of each modifier's).
    pub fn pot_multiplier(&self, reg: &ContentRegistry) -> u32 {
        self.each(reg).map(|m| m.pot_multiplier()).product()
    }

    /// Whether dominance is reversed — parity of the reversing modifiers, so two
    /// Reversals cancel back to "highest wins".
    pub fn reversed(&self, reg: &ContentRegistry) -> bool {
        self.each(reg).filter(|m| m.reverses_dominance()).count() % 2 == 1
    }

    /// The effective boiling point for a round given a base value, never below 0.
    pub fn effective_boiling_point(&self, base: u8, reg: &ContentRegistry) -> i32 {
        (base as i32 + self.boiling_point_delta(reg)).max(0)
    }

    fn each<'a>(
        &'a self,
        reg: &'a ContentRegistry,
    ) -> impl Iterator<Item = &'a dyn crate::content::Modifier> {
        self.kinds.iter().filter_map(move |k| reg.modifier(*k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> ContentRegistry {
        crate::config::ContentConfig::from_toml(include_str!("../../content.toml"))
            .unwrap()
            .build_registry()
            .unwrap()
    }

    #[test]
    fn opposite_physics_modifiers_cancel() {
        let reg = registry();
        let mut m = ActiveModifiers::new();
        m.push(ModifierKind::ThinIce);
        m.push(ModifierKind::DeepCauldron);
        assert_eq!(m.boiling_point_delta(&reg), 0);
        assert_eq!(m.effective_boiling_point(11, &reg), 11);
    }

    #[test]
    fn double_reversal_reverts() {
        let reg = registry();
        let mut m = ActiveModifiers::new();
        assert!(!m.reversed(&reg));
        m.push(ModifierKind::Reversal);
        assert!(m.reversed(&reg));
        m.push(ModifierKind::Reversal);
        assert!(!m.reversed(&reg));
    }

    #[test]
    fn double_stakes_multiplies_and_bountiful_adds() {
        let reg = registry();
        let mut m = ActiveModifiers::new();
        m.push(ModifierKind::DoubleStakes);
        m.push(ModifierKind::BountifulBrew);
        assert_eq!(m.pot_multiplier(&reg), 2);
        assert_eq!(m.pot_bonus_per_card(&reg), 1);
    }

    #[test]
    fn residue_seeds_starting_volatility() {
        let reg = registry();
        let mut m = ActiveModifiers::new();
        m.push(ModifierKind::Residue);
        assert_eq!(m.start_volatility(&reg), 3);
    }
}
