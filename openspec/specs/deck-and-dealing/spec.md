# deck-and-dealing Specification

## Purpose
TBD - created by archiving change server-release-1. Update Purpose after archive.
## Requirements
### Requirement: Shared Deck of Mixed Colors

The deck SHALL be a single shared pile assembled from the validated content config, mixing all four player colors plus wilds and the enabled effect cards. A player's hand is a random draw from this shared deck, and a player MAY play any color, not only their own.

#### Scenario: Hands contain a random color mix

- **WHEN** a hand is dealt
- **THEN** its cards are drawn from the shared deck and may include any colors, wilds, and effects regardless of the player's assigned color

### Requirement: Deal-To-Five With Carryover

At the start of each round the engine SHALL top up each player's hand to 5 cards, drawing only enough to refill it; unplayed cards carry over between rounds and count toward the 5. The refill is a *floor* — it only ever adds cards and never forces a discard; there is no hand cap, so if an effect ever leaves a hand above 5 those cards are kept.

#### Scenario: Refill replaces only what was used

- **WHEN** a player ends a round holding 2 unplayed cards and a new round begins
- **THEN** the engine deals 3 fresh cards so the player starts the new round with 5

#### Scenario: A full hand draws nothing

- **WHEN** a player carries over 5 unplayed cards into a new round
- **THEN** the engine deals them no new cards that round

### Requirement: Reshuffle From Discard On Exhaustion

When a refill would empty the draw deck, the engine SHALL reshuffle the discard pile (all previously revealed/used cards) into a fresh draw deck and continue dealing. A reshuffle is a visible table event; card counting (fed by the depile) operates per shuffle, resetting transparently and equally for all players — like a real card shoe.

#### Scenario: Empty draw deck reshuffles the discard

- **WHEN** a round-start refill needs more cards than the draw deck currently holds
- **THEN** the engine reshuffles the discard pile into the draw deck and completes the refill

#### Scenario: Reshuffle is announced to everyone

- **WHEN** the draw deck is reshuffled from the discard
- **THEN** all players are notified that a reshuffle occurred, so any card counting resets for everyone at the same moment

