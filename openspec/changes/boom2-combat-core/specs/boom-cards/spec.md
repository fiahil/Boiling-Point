## ADDED Requirements

### Requirement: Two Card Types — Ingredients And Spells

The game SHALL use two distinct card types drawn from two separate decks. **Ingredients** are played into the cauldron and carry a color, a volatility, and a point value. **Spells** are active effects that are **never** placed in the cauldron, carry **no** point value, and add **no** volatility of their own.

#### Scenario: An ingredient enters the cauldron

- **WHEN** a player plays an ingredient
- **THEN** its color, volatility, and points are added to the cauldron and it is tracked as a face-down pot card

#### Scenario: A spell never enters the cauldron

- **WHEN** a player casts a spell
- **THEN** the spell resolves as an active effect and is never added to the pot, contributing no volatility or points of its own

### Requirement: Ingredient Attributes

Every ingredient SHALL carry a **color** (one of the four player colors, or wild), a **volatility** in the range **0–7**, and a **point value** in the range **0–3**. Volatility and points are independent attributes.

#### Scenario: Volatility spans the full range

- **WHEN** ingredients are generated
- **THEN** their volatility values fall within 0–7 inclusive, with high values (5–7) rare

### Requirement: Points Score Only On Colored Votes

Points SHALL count toward the pot only when an ingredient is played as its **color** (a Vote). An ingredient played colorless (a wild / go-neutral push), and any wild ingredient, SHALL contribute its volatility but **zero** points.

#### Scenario: A colored vote scores

- **WHEN** a player plays a red ingredient as red
- **THEN** its points are added to red's color total and to the pot value P

#### Scenario: A colorless play scores nothing

- **WHEN** a player plays an ingredient colorless, or plays a wild
- **THEN** its volatility is added to the cauldron but its points contribute 0 to P

### Requirement: Fixed Color-Anchored Decks

Until deck-drafting ships, each player SHALL play from a **fixed, color-anchored pantry** (~75% their own color, the remainder a toolkit of off-color and wild ingredients) and a **fixed grimoire**, identical in composition across players up to color choice.

#### Scenario: A player's pantry is anchored to their color

- **WHEN** a player's pantry is dealt
- **THEN** about three-quarters of its ingredients are the player's own color and the remainder are toolkit cards

### Requirement: Dealing — Ingredients Top Up Each Wave, Spells Are Hoarded

At the start of each wave the engine SHALL top up each active player's ingredient hand to **3**, drawing only enough to refill. Spells SHALL be drawn only at **round start** (a fixed count), SHALL NOT be replenished within a round except by an effect that grants draws, and unused spells SHALL **carry over** between rounds.

#### Scenario: Ingredient hand refills to three

- **WHEN** a player ends a wave holding 2 ingredients and a new wave begins
- **THEN** the engine deals 1 ingredient so the player starts the wave with 3

#### Scenario: Spells are not refilled mid-round

- **WHEN** a player casts a spell during a round
- **THEN** no replacement spell is drawn until the next round start, unless an effect explicitly grants a draw
