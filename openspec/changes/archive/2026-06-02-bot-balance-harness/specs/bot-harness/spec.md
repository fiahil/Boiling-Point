## ADDED Requirements

### Requirement: Headless Protocol Client

The harness SHALL drive the server over the same public WebSocket protocol a real client uses (the `protocol/` crate), with no rendering. A bot MUST receive only the information a real player would, validating the server's secret-management contract.

#### Scenario: Bot connects like a real client

- **WHEN** a bot joins a game
- **THEN** it communicates entirely through the public wire protocol and receives only player-permitted messages (its own hand, public counts/scores/modifiers), never engine internals

### Requirement: Player-Visible Domain Model Only

A bot SHALL maintain its own narrow domain model built solely from received messages, with no field capable of holding a server secret — no boiling point, no other players' hand contents, no draw deck. Leakage is prevented by construction.

#### Scenario: Bot cannot represent the boiling point

- **WHEN** a bot tracks game state across a round
- **THEN** its model contains the boiling-point value only if and when the server discloses it (a Peek the bot itself played, or an explosion depile), and never otherwise

### Requirement: Pluggable Strategy Interface

Bot decision-making SHALL be expressed through a strategy interface (trait) so strategies can be swapped and assigned per seat. The harness MUST ship at least the baseline strategies: cautious, aggressor, diplomat, and random.

#### Scenario: Strategies are assignable per seat

- **WHEN** a game is configured with a strategy per seat
- **THEN** each bot makes its commit/pass/effect decisions through its assigned strategy

### Requirement: Complete-Game Play

A bot SHALL play a complete game end to end — join, deal, all five rounds of waves, any Deathmatch, through `GameOver` — issuing only valid commits, passes, and effects.

#### Scenario: Four bots finish a game

- **WHEN** four bots are placed in a room and the game starts
- **THEN** they play every phase to completion and the server reaches `GameOver` with recorded results
