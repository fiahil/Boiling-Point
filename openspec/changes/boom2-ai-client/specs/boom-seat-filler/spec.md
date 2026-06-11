# boom-seat-filler

Product mode: AI seats in real games — joining rooms over the real wire, playing with either brain, never holding up the table.

## ADDED Requirements

### Requirement: Joins Real Rooms Over WebSocket

The seat-filler SHALL join games over the WebSocket transport by invite code or by matchmaking enqueue, honoring the entry handshake, and SHALL play complete games to GameOver with either brain.

#### Scenario: Fill by invite code

- **WHEN** a filler is started with a room's invite code and a brain selection
- **THEN** it enters that room, plays every phase (Brewer pick, draft, rounds), and finishes the game

### Requirement: Never Stalls The Table

A filler seat SHALL commit every decision before its deadline, using the core latency budget and bot-brain fallback. From the table's perspective an AI seat is never the reason a timer runs out.

#### Scenario: Liveness under slow decisions

- **WHEN** an agent-brain filler experiences slow or failed Claude calls for an entire game
- **THEN** every wave still commits on time via fallback and the game completes

### Requirement: Delegated Pre-Game Decisions

In seat-filler mode the Brewer pick and the Apothecary draft SHALL default to **Delegated** — the brain genuinely picks and drafts (the synergy hunt), within the frame's legal options.

#### Scenario: The agent drafts for itself

- **WHEN** an agent-brain filler reaches the Apothecary phase
- **THEN** the brain selects buckets and the reserve itself, with no scripted archetype

### Requirement: Persona Presentation

A filler seat SHALL present a configurable display name and persona-appropriate table presence (e.g. emotes/table-talk where the protocol supports them), configured per seat.

#### Scenario: Two fillers, two personalities

- **WHEN** two filler seats join the same room with different persona settings
- **THEN** they present distinct names and persona-consistent behavior

### Requirement: Multiple Seats Per Process

One filler process SHALL be able to operate several seats concurrently (across one or more rooms), each with its own brain, settings, and connection.

#### Scenario: Three seats, one process

- **WHEN** a process is configured with three seats (two bot, one agent)
- **THEN** all three play concurrently with independent settings and connections

### Requirement: Survives Transient Disconnects

A filler seat SHALL attempt reconnection per the protocol's reconnection contract after a transient disconnect and resume play; if reconnection ultimately fails, it exits that seat cleanly without disturbing its other seats.

#### Scenario: Reconnect and resume

- **WHEN** a filler's connection drops mid-round and the server still holds the seat
- **THEN** the filler reconnects, rebuilds its view from the resync, and continues playing
