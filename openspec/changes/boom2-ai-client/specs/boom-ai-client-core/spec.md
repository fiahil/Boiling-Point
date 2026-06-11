# boom-ai-client-core

The shared Rust AI-client core at `clients/ai/`: the firewalled protocol consumer, the secret-free view model, the two transports, and the `Brain` trait every brain plugs into.

## ADDED Requirements

### Requirement: Client Shares Only The Protocol Crate

The AI client SHALL depend on the `protocol/` crate and MUST NOT depend on the `server/` crate or any server domain types. Its data structures SHALL remain distinct from the server's at all times; the wire protocol is the only shared vocabulary.

#### Scenario: No server dependency

- **WHEN** the `clients/ai` crate's dependency graph is inspected
- **THEN** it contains `protocol` and no `server` crate (the batch-runner binary that boots in-process games is a separate target and communicates with seats only via frame channels)

### Requirement: View Model Is Structurally Secret-Free

The client's player-visible view model SHALL be rebuilt solely from messages the seat received and SHALL have no field capable of holding a secret — no boiling point, no opponents' hands or hoards, no unrealized own-deck contents.

#### Scenario: A secret has nowhere to land

- **WHEN** the view-model types are reviewed
- **THEN** no field exists that could store the boiling point (outside post-round depile reveals), another player's hidden cards, or the player's own unrealized deck

### Requirement: Two Transports, One Codec

The client SHALL support a WebSocket transport (real wire) and an in-process transport whose channels carry **encoded wire frames** — the same bytes, through the same codec, as the WebSocket path. Domain objects MUST NOT cross the in-process boundary.

#### Scenario: In-process games exercise the codec

- **WHEN** a game runs over the in-process transport
- **THEN** every message a seat sends or receives passes through the wire encode/decode path

#### Scenario: Same scenario, both transports

- **WHEN** the same seeded scenario is driven over the in-process and WebSocket transports
- **THEN** the game outcomes are identical

### Requirement: Entry Handshake Compliance

A connection's first frame SHALL be an entry message (join by invite code, create, or enqueue); the client MUST NOT send heartbeats or any other message before entering.

#### Scenario: First frame is entry

- **WHEN** the client opens a WebSocket connection
- **THEN** the first frame it sends is an entry message, and heartbeats begin only after entry

### Requirement: The Brain Trait

The core SHALL define a `Brain` interface of the form `decide(view, decision_frame) → action`, and the decision loop SHALL submit only actions drawn from the frame's legal action set. Brains are interchangeable without changes to the client core.

#### Scenario: Brains choose among enumerated actions

- **WHEN** any brain returns an action for a decision frame
- **THEN** the submitted action is one of the frame's enumerated legal actions

### Requirement: Host Decision Policy

The host SHALL be able to configure, per decision kind, whether the decision is **Scripted** (the host supplies the answer, e.g. a fixed deck-archetype draft) or **Delegated** to the brain. The policy is a host setting, not a brain property.

#### Scenario: Harness scripts the draft

- **WHEN** a harness seat is configured with a scripted deck-archetype
- **THEN** Apothecary draft frames are answered by the script and the brain never sees them, while wave-commit frames still go to the brain

### Requirement: Decisions Race A Latency Budget

The decision loop SHALL enforce a per-decision latency budget (derived from the frame deadline minus a safety margin). If the configured brain has not answered within the budget, the loop SHALL commit the fallback answer (bot-brain policy) so the seat never misses a deadline; a late brain answer is discarded.

#### Scenario: Slow brain, on-time seat

- **WHEN** a brain exceeds the decision budget on a wave commit
- **THEN** the fallback answer is submitted before the deadline and the brain's late answer is discarded
