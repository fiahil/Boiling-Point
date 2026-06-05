## ADDED Requirements

### Requirement: Readable Card Face

The client SHALL render each hand card so its attributes are legible at the design's
readability priority — **volatility most prominent**, then color, then points, then
effect. Color SHALL be conveyed by a shape/sigil in addition to hue, so a card stays
distinguishable on low-color terminals and for color-blind players. A card carrying an
effect SHALL display that effect by **name**, not only a generic "has-effect" marker.

#### Scenario: Volatility is the loudest mark

- **WHEN** a hand card is rendered
- **THEN** its volatility is the most visually prominent attribute on the card face

#### Scenario: Color is not signaled by hue alone

- **WHEN** a hand card is rendered
- **THEN** its color is conveyed by a shape/sigil as well as hue, remaining
  distinguishable without color

#### Scenario: Effects are named on the face

- **WHEN** a hand card carries a special effect
- **THEN** the card face shows the effect's name, not only a generic effect marker

### Requirement: Card Inspector

During round-start and any open wave the client SHALL render a **live inspector** that
describes the currently cursor-selected hand item and updates immediately as the cursor
moves. For a card it SHALL show the card's color, volatility (including the effective
volatility where an effect changes it), and points, and — if the card carries an effect —
the effect's name with a plain-language description of what it does and its visibility. For
the pass option it SHALL state that committing nothing locks the player out for the rest of
the round while the explosion loss still applies. The inspector SHALL describe only public,
player-known information and SHALL NOT reveal any boiling-point value or hidden cauldron
state.

#### Scenario: Inspector follows the cursor

- **WHEN** the player moves the selection cursor to a different hand item
- **THEN** the inspector immediately updates to describe the newly selected item

#### Scenario: Effect cards are explained

- **WHEN** the cursor is on a card that carries an effect
- **THEN** the inspector shows the effect's name and a description of its mechanic and its
  visibility

#### Scenario: Pass is explained as lockout

- **WHEN** the cursor is on the pass option
- **THEN** the inspector states that passing locks the player out for the remainder of the
  round and that the explosion loss still applies

#### Scenario: Inspector reveals no secret

- **WHEN** the inspector renders for any selected item
- **THEN** it shows no boiling-point value and no hidden cauldron state

## MODIFIED Requirements

### Requirement: Opaque Cauldron And Public Contribution

During a round the client SHALL render the cauldron with **no volatility cue** — nothing
whose appearance is correlated with the cauldron's volatility, the boiling point, or its
proximity (no gauge, no proximity rumble or glow, no value). The client MAY animate the
cauldron with **ambient** motion (for example bubbling or steam) provided that motion is
**statistically independent** of the hidden state — identical in distribution whether the
pot is near-empty or one card from the edge. The client SHALL show only the public total
card count and a face-down chip per card tagged with the **contributing player's** color —
never the card's own color, points, volatility, or effect.

#### Scenario: No state-correlated cue is shown

- **WHEN** cards accumulate in the cauldron
- **THEN** the cauldron shows no running volatility, no proximity indicator, and no
  boiling-point value, and any ambient animation is unchanged in character by the
  accumulation

#### Scenario: Ambient motion leaks nothing

- **WHEN** the cauldron animates across a wave in which the hidden volatility changed
- **THEN** the animation's parameters are drawn independently of that change, carrying no
  information about the cauldron's state

#### Scenario: Chips reflect contributor, not card

- **WHEN** the cauldron holds cards from multiple players
- **THEN** each face-down chip is tagged by the color of the player who contributed it, and
  no chip reveals the card's actual color or attributes
