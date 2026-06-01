## ADDED Requirements

### Requirement: In-Process Span Start/End Hook

The server SHALL expose an in-process seam that observes span **start** and **end**
events for the documented span tree (a `tracing` `Layer`, equivalently an OTEL
`SpanProcessor`'s `on_start`/`on_end`). A consumer registered against the seam SHALL
receive a start event carrying the span's name, identity, parent, and attributes
when a span opens, and an end event when it closes.

#### Scenario: A consumer sees a span open and close

- **WHEN** a `room.lifetime` span starts and later ends
- **THEN** the registered consumer receives a start event with the room code on
  open and an end event for the same span on close

#### Scenario: Attributes are available to the consumer

- **WHEN** a span carrying attributes (public and secret) opens
- **THEN** the consumer receives those attributes in-process, before any export
  redaction

### Requirement: Hook Is Upstream Of Export Sampling

The lifecycle seam SHALL observe **every** span — 100% of the in-process stream —
independent of any sampling applied to the OTLP export path, so that consumers can
compute unsampled aggregates and a complete live registry.

#### Scenario: Sampled export does not thin the in-process stream

- **WHEN** export sampling is enabled and drops spans from the OTLP path
- **THEN** the lifecycle consumer still observes 100% of spans

### Requirement: Hook Never Backpressures The Game Loop

Consuming the lifecycle seam SHALL NOT block, delay, or stall span emission or the
game loop. When a consumer is slow or its buffer is full, the seam SHALL drop or
coalesce events rather than apply backpressure to the emitting code.

#### Scenario: A slow consumer does not stall the game

- **WHEN** the registered consumer is slow or its buffer is full
- **THEN** span emission and the game loop proceed without blocking; events are
  dropped or coalesced rather than backpressuring the emitter

### Requirement: Hook Is Read-Only By Construction

The seam SHALL hand consumers only observed span data; it SHALL NOT expose any
handle through which a consumer can mutate game state. Observing the stream SHALL
never alter game or config state.

#### Scenario: Observation cannot mutate state

- **WHEN** a consumer processes any start or end event
- **THEN** no game or config state is created, modified, or deleted as a result
