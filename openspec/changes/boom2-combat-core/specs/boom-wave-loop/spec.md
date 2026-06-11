## ADDED Requirements

### Requirement: Wave Commit — Ingredient Or Pass, Plus Optional Spell

In each wave every active player SHALL secretly choose to **play one ingredient** into the cauldron **or pass**, and MAY additionally cast **up to one spell**. Committing no ingredient (passing) locks the player out of the round for its remainder. A spell never substitutes for the ingredient-or-pass choice and never keeps a passed player active. Commits are hidden until the wave reveals simultaneously.

#### Scenario: Playing an ingredient keeps you active

- **WHEN** a player commits an ingredient in a wave
- **THEN** the player remains active and the ingredient enters the pot at reveal

#### Scenario: Passing locks you out even with a spell

- **WHEN** a player commits no ingredient in a wave
- **THEN** the player is locked out of the round for its remainder, regardless of whether they cast a spell

#### Scenario: At most one spell per wave

- **WHEN** a player attempts to cast a second spell in the same wave
- **THEN** the second cast is rejected; at most one spell resolves per player per wave

### Requirement: Timer Expiry Is Auto-Pass

If the wave timer ends before a player commits an ingredient, the engine SHALL treat them as having passed (locked out), with no grace period.

#### Scenario: Running out the clock locks you out

- **WHEN** the wave timer expires and a player has committed no ingredient
- **THEN** that player is locked out of the round, exactly as if they had passed

### Requirement: Round Termination

A round SHALL end the instant any of the following occurs: an **explosion**; **all** remaining active players pass in the same wave; or **one player remains** (who gets exactly one final wave to play or pass, after which the pot settles regardless).

#### Scenario: Everyone passing ends the round safely

- **WHEN** all remaining active players pass in the same wave
- **THEN** the round ends and the pot is scored as a safe brew

#### Scenario: The last player gets one final wave

- **WHEN** all but one player has locked out
- **THEN** the remaining player gets exactly one more wave to play or pass, after which the pot settles
