## ADDED Requirements

### Requirement: Admin Read Model Derived Solely From The Span Stream

The admin read surface SHALL be served from an in-process **projection** built
**solely** by consuming the server's OTEL span lifecycle. The projection SHALL
contain no independent game logic, SHALL derive every value it exposes from span
data, and SHALL be the single read source for the room inspector and the live
balance figures.

#### Scenario: A read value traces back to a span

- **WHEN** the admin surface displays any live value (room state, reveal field, or
  balance figure)
- **THEN** that value was derived from one or more spans the server emitted, not
  from a separate query into game internals

#### Scenario: Untraced state is invisible to the admin surface

- **WHEN** a piece of game state is not represented in any span
- **THEN** the admin surface does not display it, surfacing the instrumentation
  gap rather than reaching around the projection

### Requirement: Read-Only By Construction

The projection SHALL only observe spans; it SHALL hold no handle that can mutate
game state, and consuming the span stream SHALL never alter, delay, or block the
game loop.

#### Scenario: Projection cannot change game state

- **WHEN** the projection processes any span (start or end)
- **THEN** no game state is created, modified, or deleted as a result

#### Scenario: A slow projection does not stall the game

- **WHEN** the projection consumer is slow or its buffers are full
- **THEN** span emission and the game loop are not blocked; the projection drops
  or coalesces rather than backpressuring the game

### Requirement: Live Open-Span Registry

The projection SHALL track currently **open** spans (at least `room.lifetime`,
`game`, `round`, and `wave`) via span start and end, keyed by their room / game /
round / wave identifiers. An open span SHALL appear in the registry while it is
in flight and SHALL be removed when it ends. This registry is the source for the
live room/session/queue view and for the hidden-state reveal.

#### Scenario: A new room appears live

- **WHEN** a `room.lifetime` span starts
- **THEN** the room appears in the live registry with its current phase derived
  from its open child spans

#### Scenario: A finished room leaves the live view

- **WHEN** a room's `room.lifetime` span ends
- **THEN** the room is removed from the live registry

#### Scenario: Current phase reflects the deepest open span

- **WHEN** a room has an open `round` with an open `wave` child
- **THEN** the live view reports the room as in that round and wave

### Requirement: Unsampled Rolling Aggregates

The projection SHALL fold **completed** spans into rolling balance aggregates
(at least explosion rate, round/game duration, cards per round, dominant-color
rate, timeout rate, reconnection rate). These aggregates SHALL be computed from
the **unsampled** in-process stream — observed before any export sampling — and
SHALL NOT be sourced from a sampled exported trace.

#### Scenario: Explosion rate counts every completed round

- **WHEN** rounds complete while export sampling is enabled
- **THEN** the explosion-rate aggregate reflects 100% of completed rounds, not the
  sampled subset

#### Scenario: Aggregates update as spans close

- **WHEN** a `round`/`resolve` span ends
- **THEN** the relevant aggregates incorporate it without querying any external
  store

### Requirement: Bounded Replay Buffer

The projection SHALL retain recent **completed** games in a bounded in-memory
buffer that preserves their span tree (rounds → waves → commits → resolve →
score) for wave-by-wave replay. The buffer SHALL evict oldest entries when full,
bounding memory.

#### Scenario: A completed game is replayable

- **WHEN** a `game` span ends and is retained in the buffer
- **THEN** the admin surface can replay that game's spans in order, wave by wave

#### Scenario: Buffer stays bounded

- **WHEN** completed games exceed the buffer's capacity
- **THEN** the oldest retained game is evicted and memory stays bounded

### Requirement: Versioned Span-Schema Contract

The projection SHALL depend on a documented, **versioned** span-schema contract
enumerating the span names, their hierarchy, and their attribute keys (including the
sensitive attributes the reveal reads — boiling point, committed cards, hands,
mid-round volatility, deck seed). The schema version SHALL be recorded, and the
projection SHALL ignore unrecognized spans/attributes gracefully rather than
failing.

#### Scenario: Unknown span is ignored

- **WHEN** the projection receives a span name not in its schema version
- **THEN** it ignores the span without error and continues processing others

#### Scenario: Sensitive attributes are documented for the reveal

- **WHEN** the schema contract is consulted
- **THEN** it identifies which attributes carry sensitive game state, so the reveal
  has a single authoritative source for what it surfaces (these are admin-only and
  never carried on the player wire)
