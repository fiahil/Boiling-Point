## MODIFIED Requirements

### Requirement: Structured Tracing

The server SHALL emit its tracing as OpenTelemetry spans following a documented,
versioned span tree (room → game → round → wave → commit/resolve → score, plus
inbound-message handling, reconnection, and database writes), bridged from the
server's existing `tracing` instrumentation so there is a single instrumentation
surface. Spans carry stable, versioned attribute names; secret game state rides in
span attributes in-process only and is redacted at the export boundary. Existing
structured (JSON) logging remains available alongside the OTEL bridge.

#### Scenario: A phase transition is emitted as an OTEL span

- **WHEN** a room transitions between phases
- **THEN** an OTEL span records the transition with room and phase context, nested
  under that room's `room.lifetime` span and available to the in-process
  span-lifecycle consumer

#### Scenario: Database writes are traced

- **WHEN** the server performs its post-game persistence write
- **THEN** a `db.write` span records the write with room/game context
