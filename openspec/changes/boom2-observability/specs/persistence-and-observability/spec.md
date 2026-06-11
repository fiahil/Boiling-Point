## MODIFIED Requirements

### Requirement: Game-Balance Metrics

**BREAKING** (the v1 balance figures — explosion rate vs ~30–40%, cards per round, dominant-color rate, reshuffle frequency — retire with the v1 core; no coexistence window): the server SHALL emit Prometheus-scrapable metrics evaluated from the `boom-balance-metrics` definitions, covering at least the core v2 set — explosion (boom) rate, detonator distribution, fold/freeze rate, wave depth and duration, round and game durations — plus the carried-over fleet figures: active groups, groups created, active games (games in progress), connected players, turn/wave timeout rate, and reconnection rate.

#### Scenario: Boom rate is observable

- **WHEN** games complete over time
- **THEN** the explosion-rate metric reflects the share of rounds that boomed under the v2 detonator-only model, supporting its `[needs playtesting]` working target

#### Scenario: Live games and groups are observable

- **WHEN** games are in progress and groups exist
- **THEN** the active-games gauge reflects the number of in-progress games and the active-groups gauge reflects the number of live groups

#### Scenario: Metrics come from the shared definitions

- **WHEN** a balance metric is emitted to Prometheus
- **THEN** its value was evaluated from the `boom-balance-metrics` definition for that id, not from a formula local to the emitter

### Requirement: Structured Tracing

The server SHALL emit its tracing as OpenTelemetry spans following the documented, versioned **v2** span tree (room → game → round → wave → commit/spell-cast/resolve → depile/score, plus inbound-message handling, reconnection, and database writes), bridged from the server's existing `tracing` instrumentation so there is a single instrumentation surface. Spans carry stable, versioned attribute names; sensitive game state rides in span attributes and may reach the trusted, operator-only trace backend, but is never carried on the player wire. Existing structured (JSON) logging remains available alongside the OTEL bridge.

#### Scenario: A phase transition is emitted as an OTEL span

- **WHEN** a room transitions between phases
- **THEN** an OTEL span records the transition with room and phase context, nested under that room's `room.lifetime` span and available to the in-process span-lifecycle consumer

#### Scenario: A spell cast is traced

- **WHEN** a player casts a spell during a wave
- **THEN** a `spell.cast` span records it under that wave with the spell's identity, available to the projection

#### Scenario: Database writes are traced

- **WHEN** the server performs its post-game persistence write
- **THEN** a `db.write` span records the write with room/game context

### Requirement: Replay Payload

The game's replay payload SHALL be a MessagePack body that re-runs the pinned engine to reconstruct the full public event stream — the root seed plus the ordered per-wave action log — and SHALL additionally carry an observational, timestamped log of every raw in-game input the players sent (ingredient commits with their Vote color, passes/folds, spell casts, and emotes), each stamped with milliseconds since game start. The payload SHALL be wrapped by format/engine version tags and an integrity hash over its bytes; a payload whose bytes do not match its hash SHALL be rejected rather than reconstructed.

#### Scenario: Replay reconstructs the public event stream

- **WHEN** a stored replay is reconstructed against the engine version and content config it was recorded under
- **THEN** the re-run reproduces the game's public event stream, final scores, and winners identically

#### Scenario: Replay captures emotes and action timing

- **WHEN** a completed game's replay payload is decoded
- **THEN** it contains every in-game input each player sent — including spell casts and emotes — with a millisecond timestamp relative to game start

#### Scenario: Tampered payload is rejected

- **WHEN** a stored replay payload's bytes do not match its integrity hash
- **THEN** reconstruction is refused rather than producing an incorrect replay
