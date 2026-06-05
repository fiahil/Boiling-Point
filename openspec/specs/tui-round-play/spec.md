# tui-round-play Specification

## Purpose
TBD - created by archiving change terminal-client. Update Purpose after archive.
## Requirements
### Requirement: Round-Start Reveal

At the start of each round the client SHALL show: the newly drawn cauldron
modifier (rounds 2–5; round 1 is clean), the full cumulative stack of active
modifiers, the hand refilled **to** five with newly drawn cards distinctly
marked, and the boiling-point range shifted by the active modifiers.

#### Scenario: Modifier and refill are shown

- **WHEN** a round begins and the server reveals the round's modifier and the
  refilled hand
- **THEN** the client shows the new modifier, the cumulative active-modifier
  stack, and the hand with newly drawn cards marked

#### Scenario: Round one is clean

- **WHEN** round one begins
- **THEN** the client shows no newly drawn modifier and an empty active-modifier
  stack

#### Scenario: Shifted range is displayed

- **WHEN** active modifiers shift the boiling-point range
- **THEN** the client displays the shifted range computed from the public
  modifier effects, never an exact boiling-point value

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

### Requirement: Opponent Status

For each opponent the client SHALL show their player color and name, current
score, hand size, and number of cards contributed this round, and SHALL NOT show
any current-wave commit status while a wave is open.

#### Scenario: Opponent panel content

- **WHEN** any in-round screen is shown
- **THEN** each opponent panel shows color, name, score, hand size, and
  contributed-card count

#### Scenario: No live commit signal

- **WHEN** a wave is open and an opponent has committed
- **THEN** the client shows no indication that the opponent has or has not
  committed until the wave reveals

### Requirement: Hidden Changeable Commit

During an open wave the client SHALL let the active player select 0 or 1 card or
pass, SHALL allow the selection to be changed any number of times until the wave
closes, SHALL send only the latest selection, and SHALL render the wave timer.
Selecting pass SHALL be presented as locking the player out of the round.

#### Scenario: Selection is changeable until close

- **WHEN** the player selects a card and then, before the wave closes, selects a
  different card or switches to pass
- **THEN** the client sends the updated selection and reflects it locally, and
  only the latest selection stands at reveal

#### Scenario: Pass is presented as lockout

- **WHEN** the player chooses pass
- **THEN** the client communicates that passing locks them out of the remainder
  of the round before it is sent

#### Scenario: Input stops at close

- **WHEN** the wave closes
- **THEN** the client stops accepting commit changes and shows the wave as
  resolving

### Requirement: Wave Resolution Reveal

At wave close the client SHALL show which players played and which passed (now
locked out) and the cauldron's new card count, and SHALL surface a Recall only
as a drop in a player's public contribution count — never as a named effect.

#### Scenario: Who acted is shown, not what

- **WHEN** a wave reveals
- **THEN** the client shows the set of players who played, who passed, and the
  new total card count, and reveals no committed card's identity

#### Scenario: Recall shows as a contribution drop

- **WHEN** a Recall reduces a player's cards in the pot
- **THEN** the client reflects the lowered contribution count without naming the
  Recall effect or the recalled card

### Requirement: One-Player Final-Wave Indicator

When only one active player remains the client SHALL indicate that this is the
single final wave before the pot settles.

#### Scenario: Final wave is flagged

- **WHEN** all but one player have locked out
- **THEN** the client shows the remaining player that they have exactly one final
  wave to play or pass

### Requirement: Effect Interactions

The client SHALL handle the player-facing effect surfaces: a private Recall
target prompt at commit time, a private Peek result, a public Expose reveal, and
the anonymous "someone peeked" notice. All other effects SHALL be silent until
the depile.

#### Scenario: Recall prompts for a target at commit time

- **WHEN** the player commits a Recall card
- **THEN** the client privately prompts them to choose one of their own cards
  currently in the pot (shown face-up to them only), and the choice is part of
  the commit, completed before the wave closes

#### Scenario: Peek delivers a private result

- **WHEN** the server returns a `PeekResult` to this player
- **THEN** the client shows the exact boiling point privately and shows other
  players only an anonymous "someone peeked" notice

#### Scenario: Expose reveals publicly

- **WHEN** the server reveals an Exposed card to the table
- **THEN** the client shows that card's identity to all players

#### Scenario: Other effects stay silent

- **WHEN** Dampen, Volatile Surge, Copycat, Double Down, or Shield is played
- **THEN** the client gives no indication it occurred until that card is revealed
  in the depile

### Requirement: Preset Emotes

The client SHALL provide a fixed palette of preset emotes the player may send
during play, SHALL display received emotes transiently beside the sending
player, and SHALL offer no free-text channel.

#### Scenario: Sending an emote

- **WHEN** the player selects a preset emote and the emote relay is available
- **THEN** the client sends it and shows it briefly beside the player's own seat

#### Scenario: Receiving an emote

- **WHEN** the server relays another player's emote
- **THEN** the client shows that emote transiently beside the sender's panel

#### Scenario: No free text

- **WHEN** the player looks for a way to communicate
- **THEN** only the preset emote palette is offered, with no free-text input

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

