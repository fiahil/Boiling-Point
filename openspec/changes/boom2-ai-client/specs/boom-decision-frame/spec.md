# boom-decision-frame

The server-enumerated decision contract: for every decision a player owes, the server sends the pending decision kind and its complete legal action set. Brains (and rendering clients) choose among enumerated options instead of re-deriving rules.

## ADDED Requirements

### Requirement: Server Emits A Decision Frame For Every Pending Decision

Whenever a player owes a decision, the server SHALL send that player a **decision frame** carrying the decision kind, the deadline (when a timer applies), and the complete set of legal actions. Decision kinds SHALL cover the full v2 surface: Brewer pick, Apothecary draft (pantry buckets, grimoire buckets, reserve), and wave commit (ingredient-or-pass plus optional spell).

#### Scenario: Wave commit frame enumerates the full choice

- **WHEN** a wave opens and a player is active with 3 ingredients in hand and 2 spells in their hoard
- **THEN** the player receives a decision frame listing each playable ingredient, the pass option, and each castable spell with its legal targets

#### Scenario: Brewer pick frame offers exactly the dealt pair

- **WHEN** the pre-game Brewer phase starts and a player is dealt the pair (Featherhand, Broker)
- **THEN** the player's decision frame lists exactly those two Brewers as the legal picks

### Requirement: Legal Action Sets Are Exact

The legal action set SHALL be exact in both directions: every enumerated action, if submitted, passes server validation; and every action that would pass validation is enumerated. Targeted actions (e.g. Redirect, Hex, Sour, Double Down) SHALL enumerate their legal targets.

#### Scenario: Enumerated actions always validate

- **WHEN** a client submits an action copied verbatim from its current decision frame before the deadline
- **THEN** the server accepts it without a validation error

#### Scenario: Illegal options are absent

- **WHEN** a player has already cast a spell this wave
- **THEN** the wave-commit frame they hold (or its refresh) contains no further spell options

### Requirement: Decision Frames Carry No Secrets

A decision frame SHALL contain only information the receiving player is permitted to have. Enumerating an action MUST NOT leak hidden state (the boiling point, opponents' hands or commits, unrealized deck contents).

#### Scenario: Frames reveal no hidden pot state

- **WHEN** a player receives any decision frame
- **THEN** no field of the frame encodes the boiling point, another player's hand or hoard, or the player's own unrealized deck

### Requirement: Frames Invalidate On Phase Advance

When the game state advances past a pending decision (timer expiry, round end), the server SHALL resolve the decision per the game rules (e.g. timer expiry is auto-pass) and reject late submissions against the stale frame with an error and no state change.

#### Scenario: Late answer to an expired frame is rejected

- **WHEN** a player submits an action from a frame whose deadline has passed and been auto-resolved
- **THEN** the server rejects it with an error and the auto-resolved outcome stands
