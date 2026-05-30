# round-engine Specification

## Purpose
TBD - created by archiving change server-release-1. Update Purpose after archive.
## Requirements
### Requirement: Simultaneous Hidden Single-Card Waves

A round SHALL proceed as a sequence of waves. In each wave every still-active player secretly commits either 0 or 1 card from their hand. Commits MUST remain hidden from all other players until the wave reveals; the server MUST NOT disclose who committed, or what, before reveal.

#### Scenario: Commit stays hidden until reveal

- **WHEN** a player commits a card during an open wave
- **THEN** no other player receives any message indicating that player committed, nor the card's identity, until the wave closes

#### Scenario: A player may change their commit before close

- **WHEN** a player commits a card and then, before the wave closes, commits a different card or switches to pass
- **THEN** only their latest selection is applied at reveal

### Requirement: Wave Timer and Close

Each wave SHALL run a shared countdown — 30 seconds for wave 1 of a round and 10 seconds for subsequent waves **[needs playtesting]**. The wave closes when the timer expires, or earlier if every active player has locked in their selection.

#### Scenario: Wave closes on timer expiry

- **WHEN** the wave timer reaches zero
- **THEN** the wave closes and each active player's current selection (card or pass) is applied; players who selected nothing are treated as passing

#### Scenario: Unanimous lock-in closes the wave early

- **WHEN** every active player has locked in a selection before the timer expires
- **THEN** the wave closes immediately rather than waiting for the timer

### Requirement: Synchronized Reveal

At wave close all committed cards SHALL enter the cauldron at once, face-down to the table. The server then broadcasts which players played, which passed, and the new total card count — but never the identities of the committed cards.

#### Scenario: Table learns who acted, not what

- **WHEN** a wave reveals
- **THEN** every player receives a broadcast listing who played and who passed and the cauldron's new card count, and no player (other than via their own knowledge) learns any committed card's color, points, volatility, or effect

### Requirement: Pass Is Permanent Lockout

Committing 0 cards in a wave SHALL permanently lock a player out of the remainder of the round. Timer expiry without a selection and holding an empty hand MUST be treated identically to passing (locked out).

#### Scenario: Passing removes the player for the round

- **WHEN** a player passes in a wave
- **THEN** they are excluded from all further waves in that round and cannot commit again until the next round

#### Scenario: Empty hand is treated as locked out

- **WHEN** a player has no cards in hand at the start of a wave
- **THEN** they are treated as locked out for that round

### Requirement: Round Termination Without a Wave Cap

A round SHALL end the instant any of the following occurs, with no artificial cap on the number of waves: an explosion; all remaining active players pass in the same wave; or, the moment only one active player remains, that player takes exactly one more wave (play or pass) and then the pot settles. If the last remaining active player has an empty hand, the pot settles immediately.

#### Scenario: Explosion ends the round

- **WHEN** total cauldron volatility exceeds the boiling point at an explosion check
- **THEN** the round ends immediately on the explosion path

#### Scenario: Everyone passing ends the round

- **WHEN** all still-active players pass in the same wave
- **THEN** the round ends and the pot settles (safe brew)

#### Scenario: One survivor gets a single final wave

- **WHEN** all but one player have locked out
- **THEN** the remaining player is given exactly one more wave to play or pass, after which the pot settles regardless

### Requirement: Blind Volatility

Players SHALL receive zero cues about the cauldron's volatility state during a round — no rumble, glow, or gauge. A player knows only the boiling-point *range* (and how active modifiers shift it), the volatility of the cards they personally played, and how many cards each other player contributed. The exact boiling point is revealed only by the Peek effect.

#### Scenario: No volatility cue is ever broadcast

- **WHEN** cards accumulate in the cauldron during a round
- **THEN** the server emits no message conveying the running volatility total or proximity to the boiling point to any player

### Requirement: Per-Phase Information Visibility

The server SHALL enforce, on every message, the information each player is permitted at the current phase: own hand is private; other players' hand sizes and per-player contributed-card counts are public; cauldron card identities and exact volatility are hidden until the depile; the boiling point is hidden (revealed only via Peek to the peeker, and to everyone on an explosion depile — see Depile); scores and active modifiers are public; wave commits are hidden until reveal; and that a special effect was played is hidden by default (revealed at the depile), except for Peek, Expose, and Recall (see card-effects).

#### Scenario: Opponent hand contents are never sent

- **WHEN** any phase is active
- **THEN** a player receives the contents of only their own hand, while learning only the sizes of other players' hands

#### Scenario: Boiling point stays hidden on a safe brew

- **WHEN** a round resolves safely (no explosion)
- **THEN** no player learns the exact boiling-point value; they learn only that the volatility total stayed under it

### Requirement: Depile After Every Round

After every round — explosion or safe brew — the server SHALL reveal all cauldron cards one by one in reverse order (last-added first), each flip disclosing color, points, volatility, any effect, and which player played it. On an explosion the depile MUST mark the card at which the running volatility crossed the boiling point.

#### Scenario: Full reverse-order reveal

- **WHEN** a round ends
- **THEN** the server emits a depile revealing every cauldron card from last-added to first-added, with full attributes and the contributing player for each

#### Scenario: Explosion crossing point is marked

- **WHEN** the round ended in an explosion
- **THEN** the depile identifies the card at which cumulative volatility first exceeded the boiling point

#### Scenario: Boiling point is revealed only on explosion

- **WHEN** the round ended in an explosion
- **THEN** the depile discloses the exact boiling-point value to all players
- **AND** when the round instead resolved safely, the boiling-point value is not disclosed

