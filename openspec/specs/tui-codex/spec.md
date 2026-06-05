# tui-codex Specification

## Purpose
TBD - created by archiving change tui-readability-pass. Update Purpose after archive.
## Requirements
### Requirement: Effect & Modifier Codex

The client SHALL provide a toggleable reference overlay — the Codex — listing every special
effect and every cauldron modifier, openable and dismissable by a single key from in-game
screens. For each effect the Codex SHALL show the effect's name, its volatility and points,
and a plain-language description of its mechanic and its visibility. For each modifier the
Codex SHALL show its name and the **qualitative direction** of its effect (for example
"boiling point lower") only — never a server-side numeric magnitude. The Codex SHALL contain
no hidden game state.

#### Scenario: Codex toggles open and closed

- **WHEN** the player presses the codex key
- **THEN** the overlay appears listing all effects and modifiers, and pressing the codex key
  (or the dismiss key) again hides it

#### Scenario: Effects are fully explained

- **WHEN** the Codex lists a special effect
- **THEN** it shows the effect's name, its volatility and points, and a description of its
  mechanic and visibility

#### Scenario: Modifiers show direction only

- **WHEN** the Codex lists a cauldron modifier
- **THEN** it shows the qualitative direction of the modifier's effect and never a numeric
  magnitude

#### Scenario: Codex reveals no secret

- **WHEN** the Codex renders
- **THEN** it contains no boiling-point value, no cauldron volatility, and no opponent hand
  contents

