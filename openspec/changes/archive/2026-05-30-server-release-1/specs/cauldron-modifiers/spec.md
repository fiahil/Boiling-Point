## ADDED Requirements

### Requirement: Round-One Clean, One Draw Per Subsequent Round

Round 1 SHALL have no modifier. At the start of each subsequent round (2–5) the engine SHALL draw exactly one modifier from the pool and reveal it at round start.

#### Scenario: First round is unmodified

- **WHEN** round 1 begins
- **THEN** no modifier is active

#### Scenario: A modifier is revealed at the start of later rounds

- **WHEN** round 2, 3, 4, or 5 begins
- **THEN** the engine draws one modifier from the pool and broadcasts it as newly active before the first wave

### Requirement: Cumulative Stacking

Active modifiers SHALL persist for the rest of the game and accumulate: round N (for N ≥ 2) has N−1 active modifiers, all publicly visible.

#### Scenario: Modifiers accumulate across rounds

- **WHEN** round 4 begins
- **THEN** three modifiers are active — the ones drawn at the starts of rounds 2, 3, and 4

### Requirement: Clean Composition of Offsets and Multipliers

Each modifier SHALL be a single offset or multiplier so that stacking composes arithmetically with no special-case rules; contradictory modifiers MUST simply cancel.

#### Scenario: Opposite physics modifiers cancel

- **WHEN** both Thin Ice (boiling point −4) and Deep Cauldron (boiling point +4) are active
- **THEN** their net effect on the boiling point is zero

#### Scenario: Reversal pairs cancel

- **WHEN** two Reversal modifiers are active
- **THEN** dominance reverts to the highest-point color winning (double negation)

### Requirement: The Six Modifier Effects

The engine SHALL implement six modifier types, each doing exactly one thing: **Residue** (cauldron starts at +3 volatility), **Thin Ice** (boiling point −4), **Deep Cauldron** (boiling point +4), **Bountiful Brew** (+1 point to the pot total per card played — colorless, applied to *every* card including 0-point cards, and never attributed to a color), **Double Stakes** (all pot points ×2, affecting both the win payout and the explosion loss), and **Reversal** (the lowest-point color *present in the pot* wins instead of the highest). All magnitudes are content/config **[needs playtesting]**.

#### Scenario: Residue seeds starting volatility

- **WHEN** a round begins with Residue active
- **THEN** the cauldron starts at +3 volatility instead of 0

#### Scenario: Bountiful Brew inflates the pot total only

- **WHEN** Bountiful Brew is active and N cards are played in the round
- **THEN** the pot's total point value increases by N (one per card) without changing any color's point total used to decide dominance

#### Scenario: Double Stakes scales both directions

- **WHEN** Double Stakes is active
- **THEN** both the winner's payout and every player's explosion loss are computed from the doubled pot value

#### Scenario: Reversal flips dominance to the lowest color present

- **WHEN** Reversal is active and the round resolves safely with two or more colors present
- **THEN** the color with the lowest point total *among colors actually in the pot* wins the pot, never an absent color, evaluated on final per-color totals (after Double Down) and unaffected by Bountiful Brew's colorless bonus

#### Scenario: Reversal is a no-op with a single color present

- **WHEN** Reversal is active and only one color is present in the pot
- **THEN** that color is both highest and lowest and wins as normal

#### Scenario: Reversal tie for lowest splits normally

- **WHEN** Reversal is active and two colors tie for the lowest point total present
- **THEN** those players split the pot via the usual Alliance/Commune rules

### Requirement: Weighted Pool Draw

Modifiers SHALL be drawn from a weighted pool whose size, per-type copies, and weights come from validated content config, so dramatic modifiers can be made rarer.

#### Scenario: Draw respects configured weights

- **WHEN** the engine draws a modifier
- **THEN** the selection is made from the configured pool according to its per-type copies/weights, and never returns a disabled modifier type
