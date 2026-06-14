## ADDED Requirements

### Requirement: Count-Threshold Compounding

Some ingredients SHALL carry a **count threshold** that scores more when the pot is large, keyed off the **public** pot card count (so it is plannable). The bonus SHALL apply at scoring based on the card count at resolution.

#### Scenario: A threshold card scores more in a big pot

- **WHEN** a Honey ingredient with "+1 point per card past the 5th" resolves in an 8-card pot
- **THEN** it adds 3 bonus points (cards 6, 7, 8) to its color total

#### Scenario: The trigger is public information

- **WHEN** a player considers a count-threshold card
- **THEN** they can reason about it from the public contribution counts, without seeing the hidden pot contents

### Requirement: Named-Combo Bonuses Are Bonuses, Never Requirements

Some ingredients SHALL belong to **named combos** of 2–5 distinct members that grant a **size-scaling bonus** when every member is in the pot (larger combos pay more and are rarer to assemble). A combo card SHALL be fully playable and useful **alone**; the combo only adds an upside. No card SHALL be dead or penalized for lacking its combo-mates.

#### Scenario: A combo pays off when completed

- **WHEN** all members of a named combo are in the pot
- **THEN** the size-scaling combo bonus applies (more points for larger combos; a Herbalist's fires twice; an Alchemist's also adds volatility)

#### Scenario: A lone combo member is still a normal card

- **WHEN** only some of a combo's members are in the pot
- **THEN** each present member plays as a normal ingredient with no penalty for the missing members

### Requirement: Effective Volatility Feeds Resolution

When compounding changes an ingredient's volatility (e.g. a combo that adds volatility), the **effective** value SHALL be used by both the explosion check and the fatal-wave detonator sort, consistent with `boom-resolution`.

#### Scenario: A combo's added volatility can change the detonator

- **WHEN** a combo raises a card's effective volatility above another fatal-wave card
- **THEN** the detonator sort uses the effective values, and liability follows the post-compounding order

### Requirement: Color-Synergy Compounding Is Capped Or Peek-Gated

If color-synergy compounding (scoring that scales with same-color cards in the hidden pot) is used, it SHALL be **capped** or gated behind information (e.g. Peek) so it cannot produce an unbounded, hidden snowball.

#### Scenario: Color synergy cannot run away

- **WHEN** a color-synergy card resolves in a heavily mono-color pot
- **THEN** its bonus is bounded by the cap (it does not scale without limit)

### Requirement: Reach-In Manipulation Stays In The Grimoire

Compounding SHALL NOT bake table-reaching manipulation (doubling or reducing another color's points) onto ingredients; those effects remain **grimoire spells** (Double Down, Sour).

#### Scenario: Ingredients compound, spells manipulate

- **WHEN** a designer adds a new in-pot interaction
- **THEN** passive/self or count/combo effects may live on ingredients, but any effect that reaches into another color's totals is a spell, not an ingredient tag
