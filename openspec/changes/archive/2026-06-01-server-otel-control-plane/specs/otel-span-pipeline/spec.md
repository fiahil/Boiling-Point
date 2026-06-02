## ADDED Requirements

### Requirement: Server Emits OTEL Spans As The Telemetry Source

The server SHALL emit its tracing as OpenTelemetry spans bridged from the existing
`tracing` instrumentation, so that downstream consumers — Prometheus, the OTLP
export, and the `admin-ui` projection — read from one instrumentation surface. The
bridge SHALL be additive: existing structured logging and Prometheus metrics
continue to work alongside it.

#### Scenario: A phase transition is emitted as an OTEL span

- **WHEN** a room transitions between phases
- **THEN** an OTEL span records the transition with room and phase context,
  available to the in-process span-lifecycle consumer

#### Scenario: Existing metrics and logs are unaffected

- **WHEN** the OTEL pipeline is initialised
- **THEN** Prometheus metrics are still exported and structured logs are still
  emitted, with no loss of the existing observability surface

### Requirement: Documented, Versioned Span Tree

The server SHALL emit a documented span tree whose long-lived spans nest
`room.lifetime` → `game` → `round` → `wave`, with leaf spans for `commit`,
`resolve`, and `score` under the appropriate parent, plus `ws.message`,
`reconnect`, and `db.write` spans. The set of span names, their hierarchy, and
their attribute keys SHALL be captured in a single **versioned span-schema
contract** exposed by the server, carrying an explicit schema version. Some
attributes carry sensitive game state (boiling point, hands, mid-round volatility,
deck seed); these ride in spans for the admin reveal but are never carried on the
player wire.

#### Scenario: The span tree nests as documented

- **WHEN** a game runs a round and a wave within a live room
- **THEN** the `wave` span is a child of the `round` span, which is a child of the
  `game` span, which is a child of that room's `room.lifetime` span

#### Scenario: The schema contract is the single source of names

- **WHEN** the projection needs a span name or an attribute key
- **THEN** it reads them from the versioned span-schema contract rather than
  hard-coding strings, and the contract reports its schema version

### Requirement: Stable, Versioned Attribute Names

Span attribute keys SHALL be stable identifiers defined in the span-schema
contract. Long-lived spans SHALL carry the identifiers needed to key the live
registry: `room.lifetime` carries the room code; `game`, `round`, and `wave` carry
their respective room / game / round / wave identifiers and numbers.

#### Scenario: Live-registry keys are present on open spans

- **WHEN** a `round` span with a child `wave` span is open
- **THEN** both carry the room, game, round, and wave identifiers needed to place
  them in the live open-span registry

### Requirement: OTLP Export Wired, Backend Deferred

The server SHALL wire an OTLP span exporter behind a configurable endpoint, but
SHALL NOT require a running trace backend (Tempo/Jaeger) to operate. Startup and
the game loop SHALL proceed normally when no OTLP endpoint is reachable; export
failures SHALL be tolerated (logged/dropped), never fatal.

#### Scenario: Server runs with no trace backend present

- **WHEN** the server starts and no OTLP endpoint is reachable
- **THEN** startup completes, games run, and the in-process span-lifecycle consumer
  still sees spans; only the export to the (absent) backend fails, without crashing
  or stalling the game loop
