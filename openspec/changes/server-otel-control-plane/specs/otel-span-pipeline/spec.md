> **STUB.** Placeholder so the change is structurally valid. Only
> `otel-span-pipeline` is sketched (one requirement); `telemetry-redaction`,
> `span-lifecycle-hook`, and `admin-command-primitives` are named in the proposal
> and to be specced when this change is promoted from stub.

## ADDED Requirements

### Requirement: Server Emits OTEL Spans As The Telemetry Source

The server SHALL emit its tracing as OpenTelemetry spans following a documented,
versioned span tree (room → game → round → wave → commit/resolve → score, plus
message handling, reconnection, and DB writes), so that downstream consumers —
Prometheus, the OTLP export, and the `admin-ui` projection — read from one
instrumentation surface. Secret game state SHALL be carried only in-process and
redacted before export (detailed when promoted).

#### Scenario: A phase transition is emitted as an OTEL span

- **WHEN** a room transitions between phases
- **THEN** an OTEL span records the transition with room and phase context,
  available to the in-process span-lifecycle consumer
