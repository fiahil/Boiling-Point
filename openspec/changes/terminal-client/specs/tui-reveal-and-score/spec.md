## ADDED Requirements

### Requirement: Reverse-Order Depile

After every round the client SHALL animate the depile, revealing cauldron cards
one by one in reverse order (last-added first), each flip disclosing the card's
color, points, volatility, any effect, and the contributing player, alongside a
**descending** volatility bar that starts at the pot total and decreases as each
card is peeled. The animation SHALL be skippable.

#### Scenario: Cards reveal last-added first

- **WHEN** a round ends and the server emits the depile
- **THEN** the client reveals each card from last-added to first-added with its
  full attributes and contributing player

#### Scenario: Descending bar tracks the peel

- **WHEN** each depile card is revealed
- **THEN** the volatility bar decreases by that card's volatility

#### Scenario: Depile can be skipped

- **WHEN** the player presses the skip/advance key during the depile
- **THEN** the client jumps to the fully revealed end state

### Requirement: Boiling Point Shown On Explosion Only

The client SHALL draw the boiling-point line and mark the crossing card **only**
when the round ended in an explosion; on a safe brew it SHALL NOT display any
boiling-point value or line.

#### Scenario: Explosion marks the crossing card

- **WHEN** the depile is for an exploded round
- **THEN** the client draws the boiling-point line and highlights the card at
  which the running volatility crossed it

#### Scenario: Safe brew hides the boiling point

- **WHEN** the depile is for a safely resolved round
- **THEN** the client draws no boiling-point line and shows no boiling-point
  value

### Requirement: Boom Sequence

On an explosion the client SHALL play a brief full-screen boom effect and state
the shared loss — every player loses the full pot value.

#### Scenario: Boom communicates the shared loss

- **WHEN** a round explodes
- **THEN** the client shows the boom effect and the per-player loss equal to the
  pot value

### Requirement: Round Scoring Screen

After the depile the client SHALL show the round outcome (Domination, Alliance,
Commune, or Absent), the pot value with any modifier effects applied, and each
player's point delta and new total, then offer to continue.

#### Scenario: Outcome and deltas are shown

- **WHEN** a round is scored
- **THEN** the client shows the outcome, the pot value, and every player's delta
  and new running total

#### Scenario: Continue to next round

- **WHEN** the player advances from the scoring screen and another round remains
- **THEN** the client proceeds to the next round-start reveal

### Requirement: Deathmatch Screens

When the game enters Deathmatch the client SHALL render its distinct rules:
participants only, no modifiers, a forced single commit each wave with **no pass
option**, and a volatility-only framing. It SHALL announce the Detonator
(most-volatility) elimination, the Shield-redirect cascade when it occurs, and
the co-champions outcome.

#### Scenario: Forced commit, no pass

- **WHEN** a Deathmatch wave is open
- **THEN** the client requires the player to commit one card and offers no pass
  action

#### Scenario: Detonator elimination is announced

- **WHEN** a Deathmatch wave explodes
- **THEN** the client announces the most-volatility contributor as the eliminated
  Detonator and the resulting survivor(s)

#### Scenario: Shield redirect is shown

- **WHEN** the would-be Detonator is Shielded and the blast is redirected
- **THEN** the client shows the redirect to the next-highest contributor (and any
  further cascade)

#### Scenario: Co-champions on no boom

- **WHEN** a Deathmatch ends with hands exhausted and no explosion
- **THEN** the client declares all surviving participants co-champions

### Requirement: Game Over

At game end the client SHALL show the final standings (ranked scores with the
winner highlighted), a short brew summary, and options to return to the lobby or
re-enter the queue.

#### Scenario: Final standings are shown

- **WHEN** the game ends
- **THEN** the client shows every player's final score ranked, the winner
  highlighted, and a return-to-lobby option
