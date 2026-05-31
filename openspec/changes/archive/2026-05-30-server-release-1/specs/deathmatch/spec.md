## ADDED Requirements

### Requirement: Deathmatch Trigger and Setup

When two or more players tie for the highest score after the final round, the engine SHALL run a Deathmatch among only the tied players to order them. Main-game scores freeze and are unaffected by the Deathmatch. Each Deathmatch round starts with a fresh hidden boiling point (8–14) and NO modifiers. A tied player who starts a Deathmatch with an empty hand is eliminated immediately and placed last among the tied.

#### Scenario: Only tied players participate

- **WHEN** three players tie for first and a fourth is lower
- **THEN** the Deathmatch includes only the three tied players and the fourth's placement is already settled

#### Scenario: Main-game scores are frozen

- **WHEN** a Deathmatch resolves
- **THEN** it only orders the tied players and does not change any player's recorded score

### Requirement: Forced Volatility-Only Commits

In a Deathmatch every participant MUST commit exactly 1 card per wave — passing is not allowed — under the same simultaneous-wave timing as normal play. Color and points are irrelevant; only volatility matters.

#### Scenario: No passing in Deathmatch

- **WHEN** a Deathmatch wave is open and a participant has cards
- **THEN** they must commit one card and cannot pass

### Requirement: Detonator Elimination

On a Deathmatch explosion, the participant who contributed the most total volatility to the pot SHALL be eliminated as the Detonator. If one survivor remains they are champion; if two or more remain, a fresh Deathmatch runs among them. If multiple participants tie for the most volatility, all tied-for-most are eliminated together; if that eliminates everyone, the remaining set are co-champions.

#### Scenario: Highest-volatility contributor is eliminated

- **WHEN** a Deathmatch pot explodes
- **THEN** the participant with the single highest total volatility contribution is eliminated and the rest continue

#### Scenario: Tie for most volatility eliminates all of them

- **WHEN** two participants tie for the highest volatility contribution at explosion
- **THEN** both are eliminated together, and if that leaves no one, the just-eliminated set are co-champions

### Requirement: Shield Redirect Cascade

If the highest-volatility contributor played Shield, the Detonator blast SHALL be redirected to the next-highest volatility contributor, cascading through any further Shields. If every remaining participant is Shielded, there is no casualty and a fresh Deathmatch runs.

#### Scenario: Shield shoves elimination onto a rival

- **WHEN** the most-volatility participant is Shielded and the next-highest is not
- **THEN** the next-highest is eliminated instead

#### Scenario: All shielded yields no casualty

- **WHEN** every remaining participant is Shielded at a Deathmatch explosion
- **THEN** no one is eliminated and a fresh Deathmatch runs among them

### Requirement: No-Explosion Co-Champions

If a Deathmatch ends without an explosion because participants exhaust their hands, all surviving participants SHALL be co-champions.

#### Scenario: Hands run out safely

- **WHEN** all Deathmatch participants exhaust their hands with no explosion
- **THEN** all surviving participants are declared co-champions
