# boom-agent-brain

The Claude-driven brain: persona-driven decisions over the same `Brain` interface, with its own settings and a hard timeliness/cost contract.

## ADDED Requirements

### Requirement: Decisions Via Tool-Forced Claude Calls

The agent brain SHALL obtain each decision from a Claude API call whose tool schema is derived from the decision frame's legal action set, so the model can only express legal actions. The response SHALL be mapped back to an enumerated action; an unparseable or illegal response triggers the fallback policy.

#### Scenario: Schema follows the frame

- **WHEN** a wave-commit frame offers 3 ingredients, pass, and 2 spells with targets
- **THEN** the tool schema presented to Claude permits exactly those choices

#### Scenario: Malformed response falls back

- **WHEN** the model response cannot be mapped to a legal action
- **THEN** the fallback (bot-brain) answer is committed and the incident is logged

### Requirement: Prompt Context Is Player-Permitted Only

The agent brain's prompt — persona, difficulty framing, and the running game transcript — SHALL be built exclusively from the seat's secret-free view model and the events the seat legitimately observed. No server-side or hidden information reaches the prompt.

#### Scenario: No secrets in the prompt

- **WHEN** any prompt sent by the agent brain is inspected
- **THEN** it contains no boiling point (outside revealed depiles), no opponents' hidden cards, and no unrealized own-deck contents

### Requirement: Agent Brain Settings

The agent brain SHALL expose its own settings, distinct from the bot brain's: model selection, persona, difficulty, per-decision latency budget, fallback policy, and authentication/spend configuration.

#### Scenario: Settings are independent

- **WHEN** an agent seat and a bot seat run in the same process
- **THEN** each brain is configured by its own settings block with no shared knobs

### Requirement: Never Stalls A Seat

The agent brain SHALL operate under the core latency budget: a missed budget commits the fallback answer, and the seat never misses a deadline. The per-game fallback rate SHALL be recorded and reported.

#### Scenario: Budget miss is invisible to the table

- **WHEN** a Claude call exceeds the decision budget
- **THEN** the seat still commits in time via fallback, and the game proceeds without delay

### Requirement: Hard Spend Caps

The agent brain SHALL enforce a configurable spend cap (per process and per game). When a cap is reached, the seat SHALL degrade to the bot brain for the remainder rather than exceed the cap or abandon the seat.

#### Scenario: Cap reached mid-game

- **WHEN** the per-game spend cap is exhausted in round 3
- **THEN** rounds 4–5 are played by the fallback bot brain and the game completes normally

### Requirement: Persona Shapes Play, Not Legality

Personas and difficulty SHALL influence prompts, decision style, and table presentation only. They MUST NOT alter the legal action set, the view model, or the timeliness contract.

#### Scenario: Persona cannot widen the action space

- **WHEN** any persona or difficulty is configured
- **THEN** submitted actions are still drawn only from the decision frame's legal set
