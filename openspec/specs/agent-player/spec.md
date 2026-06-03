# agent-player Specification

## Purpose
TBD - created by archiving change agent-player-harness. Update Purpose after archive.
## Requirements
### Requirement: Claude-Driven Protocol Client

The harness SHALL connect over the same public WebSocket protocol a real client uses — the `protocol/` message catalog and the `JoinRoom`/`protocol_version` handshake — with no rendering, and play a complete game end to end (join, all rounds of waves, any Deathmatch, through `GameOver`). It MUST receive only the information a real player would and issue only valid commits, passes, effect target-picks, and emotes.

#### Scenario: Agent finishes a game over the real protocol

- **WHEN** an agent joins a room and the game starts
- **THEN** it communicates entirely through the public wire protocol, receives only player-permitted messages, and plays every phase to completion until the server reaches `GameOver`

#### Scenario: Incompatible protocol version is surfaced, not crashed through

- **WHEN** the agent sends `JoinRoom` with a `protocol_version` the server rejects
- **THEN** the harness reports the version mismatch from the server's `Error` and does not attempt to play

### Requirement: Player-Visible View Model

The agent SHALL maintain a narrow view model built solely from received `ServerMessage`s. It MUST NOT hold the boiling point, other players' hand contents, or the draw deck except where the server legitimately discloses them (the agent's own `PeekResult`, or an explosion depile). A runtime assertion SHALL fail if any secret field is ever populated outside such a disclosure.

#### Scenario: Agent has no boiling-point value unless disclosed

- **WHEN** the agent tracks state across a round that ends in a safe brew
- **THEN** its view model never contains the boiling-point value, and the secret-boundary assertion holds

#### Scenario: Disclosed secrets enter the model only via their message

- **WHEN** the agent plays a Peek or a round ends in an explosion depile
- **THEN** the boiling point appears in the view model only as carried by that `PeekResult` or depile message, and never otherwise

### Requirement: Actions and Capabilities as In-Process MCP Tools

Every move the agent can make SHALL be exposed to Claude as an in-process MCP tool — at minimum `commit_card`, `pass`, `lock_in`, `pick_target` (for targeted effects such as Recall), and `send_emote` — and analytical capabilities SHALL be exposed as separate capability tools. Claude acts only by calling tools; the harness validates each tool call against the view model and forwards a corresponding `ClientMessage`.

#### Scenario: A tool call becomes a validated client message

- **WHEN** Claude calls `commit_card` with a card the view model shows in hand during an open wave
- **THEN** the harness sends the corresponding commit `ClientMessage` and reflects the server's response back to the session

#### Scenario: An impossible tool call is refused locally

- **WHEN** Claude calls `commit_card` for a card not in the agent's hand, or acts while locked out
- **THEN** the harness rejects the tool call with an error result and sends nothing to the server

### Requirement: Difficulty Is the Granted Tool Set

A difficulty preset SHALL be defined exactly as the set of capability tools made callable (the Agent SDK `allowedTools`). Withholding a capability tool MUST remove that capability entirely. The v0 presets are **Easy** (action tools only) and **Hard** (action tools plus the reveal-history capability tool).

#### Scenario: Easy lacks the card-history capability

- **WHEN** an agent runs at the Easy preset
- **THEN** no card-history tool is callable, and the agent cannot retrieve any past depile reveal

#### Scenario: Hard can call the card-history capability

- **WHEN** an agent runs at the Hard preset
- **THEN** the reveal-history tool is callable and returns past reveals scoped to the current shuffle epoch

### Requirement: Revocable Information Lives Behind Tools

The per-turn context delivered to Claude SHALL carry only thin public state — the agent's own hand, public wave state (who committed or passed and the pot count), scores, and the threshold range. Card identities from the depile SHALL be obtainable only through a capability tool, so that an agent whose preset omits that tool never receives them, even across a long-lived session.

#### Scenario: A tool-starved agent never accumulates hidden history

- **WHEN** an Easy agent plays several rounds in one session
- **THEN** its context never contains past card identities, because the only source of them is a tool it cannot call

#### Scenario: A capable agent obtains history only on demand

- **WHEN** a Hard agent wants to count cards
- **THEN** the past identities enter its context only as the result of an explicit reveal-history tool call

### Requirement: Timely Commitment Within the Wave

The agent SHALL begin deliberating for the next wave when the previous wave resolves, and SHALL commit or lock in within the server's wave window. If no decision is ready near the deadline, the harness SHALL commit a fast local fallback action (a cheap heuristic or a pass) computed without an LLM call. The server alone decides when a wave closes; the harness MUST NOT treat its local clock as authoritative.

#### Scenario: Deliberation that overruns the timer falls back

- **WHEN** Claude has not produced an action as the wave deadline approaches
- **THEN** the harness submits its local fallback action so the wave is not stalled waiting on the agent

#### Scenario: Early lock-in keeps the table flowing

- **WHEN** the agent has decided well before the deadline
- **THEN** it locks in its selection, allowing the server to close the wave early once all active players have locked in

### Requirement: Per-Seat Process

Each agent SHALL run as its own process with its own connection, Agent SDK session, difficulty, and persona; agents MUST NOT share state with one another.

#### Scenario: Multiple independent agents fill a room

- **WHEN** three agents are launched against a room code and the developer joins as the fourth seat
- **THEN** each agent is a separate process making its own decisions, and no agent can read another's hand or reasoning
