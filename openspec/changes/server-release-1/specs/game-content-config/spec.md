## ADDED Requirements

### Requirement: Content Is Separated From Engine

All game *content* — card attributes, special effects, and cauldron modifiers — SHALL be defined in a content module that is independent of the wire protocol and the game loop. The loop MUST operate only on stable abstractions (traits/enums for behavior) and never reference a specific named card, effect, or modifier. Adding, removing, or retuning content MUST NOT require changes to the protocol or the loop.

#### Scenario: Retuning a card touches only content

- **WHEN** a card's volatility, points, or count is changed
- **THEN** the change is made only in the content module/config and neither the protocol types nor the game-loop code are modified

#### Scenario: Loop dispatches by abstraction

- **WHEN** the loop resolves a played card's effect
- **THEN** it dispatches through a behavioral abstraction (effect trait/enum) rather than matching on a specific card identity

### Requirement: Distinct Content Kinds

Regular (point/color) cards, special-effect cards, and cauldron modifiers SHALL be modeled as distinct types. The system MUST NOT collapse different content kinds into a single shared union or table.

#### Scenario: A modifier cannot be dealt as a hand card

- **WHEN** a hand is dealt from the deck
- **THEN** only card-kind content can be drawn and cauldron modifiers are drawn exclusively from the separate modifier pool

### Requirement: Per-Item Enable/Disable Toggle

Each content item (card type, effect, modifier) SHALL carry an `enabled` flag in configuration. Disabled items MUST be excluded from the deck and modifier pool without any code change.

#### Scenario: Disabled effect never appears in play

- **WHEN** an effect is marked `enabled = false` in the content config
- **THEN** the assembled deck contains no copies of that effect and it is never dealt or resolved

### Requirement: Config-Driven Counts and Ratios

Deck composition, per-effect copy counts, and modifier-pool weights SHALL be defined in a file-based config (e.g. TOML or RON), not hardcoded in the loop.

#### Scenario: Counts come from config

- **WHEN** the server assembles the deck and modifier pool at startup
- **THEN** the quantities and weights are read from the content config file

### Requirement: Fail-Fast Startup Validation

The server SHALL validate the entire content config at startup and refuse to start on an invalid config, reporting a clear, actionable error. Validation MUST at least check: declared counts sum to the stated deck size; color/effect/wild ratios fall within configured bounds; every effect referenced by the rules is present and enabled (or explicitly disabled); the modifier pool holds enough cards to draw one per applicable round; and the deck is large enough to deal the initial round-1 hands (4 × 5) with margin (the discard reshuffle, not deck size, covers later exhaustion).

#### Scenario: Invalid config aborts startup

- **WHEN** the content config fails any validation rule (e.g. counts do not sum, or the deck is too small to cover the initial deal)
- **THEN** the server logs a specific error identifying the failed rule and exits without serving traffic

#### Scenario: Valid config boots cleanly

- **WHEN** the content config passes all validation rules
- **THEN** the server completes startup and begins accepting connections
