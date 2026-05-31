## ADDED Requirements

### Requirement: Dominance By Total Color Points

When a round resolves safely, the engine SHALL decide the winning color by the highest sum of point values among that color's cards in the pot — not by card count. Wild points count toward the pot's total value but belong to no color and so never decide dominance.

#### Scenario: Fewer high-point cards beat many cheap ones

- **WHEN** Blue holds two 3-point cards (6) and Red holds three 1-point cards (3)
- **THEN** Blue is the dominant color

#### Scenario: Wild points do not win for a color

- **WHEN** the pot contains wild cards with points
- **THEN** those points are added to the pot total but are not attributed to any color when determining dominance

### Requirement: Winner Takes All

The dominant color's player SHALL receive all point value in the pot — every card of every color, including wilds. Zero-point cards add color presence and volatility but nothing to the *base* pot value (though Bountiful Brew's +1-per-card bonus applies to them like any card — see cauldron-modifiers).

#### Scenario: Sole dominant color scoops the pot

- **WHEN** one color has the strictly highest point total
- **THEN** that player gains points equal to the entire pot value and all other players gain nothing from the brew

### Requirement: Alliance and Commune Splits

When two or more colors tie for the highest point total, those players SHALL split the entire pot value equally, rounded down, with the scoreboard kept integer-only; any leftover point evaporates.

#### Scenario: Two-way tie splits and rounds down

- **WHEN** two colors tie for the highest point total in a pot worth 7
- **THEN** each of the two players gains 3 and the leftover 1 point evaporates

#### Scenario: Three-way tie splits equally

- **WHEN** three colors tie for the highest point total
- **THEN** those three players split the pot value equally, rounded down

### Requirement: Absent Player Scores Zero

A player who contributed zero cards to a safely resolved pot SHALL gain zero from it (no risk, no reward).

#### Scenario: Spectator gains nothing on a safe brew

- **WHEN** a round resolves safely and a player contributed no cards
- **THEN** that player's score is unchanged for the round

### Requirement: Shared-Loss Explosion

On an explosion, every player — including spectators who contributed zero cards — SHALL lose points equal to the total pot value. The pot value is computed identically to a safe brew — sum of all card points, plus Bountiful Brew (+1/card), times Double Stakes (×2) — omitting only the dominance step, so "you lose exactly what the pot was worth" holds even under modifiers. There is no blame role, no floor, and no ceiling.

#### Scenario: Everyone loses the pot

- **WHEN** a round explodes with a pot worth P
- **THEN** every non-shielded player's score decreases by P, regardless of how many cards they contributed

#### Scenario: No cap on a large explosion

- **WHEN** modifiers have inflated the pot value to a large number and it explodes
- **THEN** the full inflated value is deducted from each non-shielded player with no ceiling applied

### Requirement: Scoring Sequence — Winner First, Then Value

The engine SHALL decide the winner from per-color point totals *first* (with Copycat/Double Down already baked in by their wave), applying Reversal to pick the lowest present color and **excluding Bountiful Brew's colorless bonus from this step**. It SHALL then compute the pot value as the sum of all card points, plus additive bonuses (Bountiful Brew, +1/card), and finally any multiplier (Double Stakes, ×2) — **additive bonuses before multipliers**. The resulting pot value is awarded to the winner(s) or deducted on explosion.

#### Scenario: Bountiful does not shift dominance

- **WHEN** Bountiful Brew is active
- **THEN** the dominant color is decided on per-color point totals only, and the per-card colorless bonus changes only the pot's value, not which color wins

#### Scenario: Bountiful then Double Stakes compose

- **WHEN** both Bountiful Brew and Double Stakes are active
- **THEN** the per-card bonus is added to the pot total first and the sum is then doubled before being awarded or deducted

### Requirement: Shield Excludes a Player From Round Scoring

A player who played Shield in the round SHALL take no explosion loss; if the round instead resolves safely, that player forfeits all scoring for the round.

#### Scenario: Shielded player is skipped on explosion loss

- **WHEN** the round explodes and a player played Shield
- **THEN** that player loses nothing while all other non-shielded players lose the full pot value

#### Scenario: Shielded player forfeits a safe payout

- **WHEN** the round resolves safely and a player played Shield
- **THEN** that player neither gains nor loses points for the round even if their color won or tied
