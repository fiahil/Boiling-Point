# persistence-and-observability Specification

## Purpose
TBD - created by archiving change server-release-1. Update Purpose after archive.
## Requirements
### Requirement: Post-Game Persistence Only

The server SHALL persist results only after a game completes (`GameOver`), writing the game, its participants and their final results, and optional per-round detail. No game state is persisted mid-game; a crash before completion loses only the in-progress game.

#### Scenario: Completed game is written once

- **WHEN** a game reaches `GameOver`
- **THEN** the server writes one game row, one result row per participant, and (if enabled) per-round rows, in a single completion write

#### Scenario: In-progress game is not persisted

- **WHEN** a game is still in progress
- **THEN** no game, participant, or round rows have been written for it

### Requirement: Relational Schema

Persistence SHALL use PostgreSQL with tables for players (UUID identity, display name, created timestamp), games (timestamps, round count), game_players (final score, finish position, per-player stats), and game_rounds (threshold, exploded flag, volatility total, cards played) for analytics.

#### Scenario: Results are queryable per player

- **WHEN** a completed game has been persisted
- **THEN** each participant's final score and finish position can be retrieved by joining game_players to players

### Requirement: Anonymous Player Records

Player records SHALL be created from anonymous session authentication — keyed by the issued UUID with a display name — without any email or password.

#### Scenario: Player row exists for a participant

- **WHEN** a game completes
- **THEN** every participant has a player row identified by their session-issued UUID

### Requirement: Game-Balance Metrics

The server SHALL emit Prometheus-scrapable metrics covering at least: active rooms, rooms created, connected players, game and round durations, explosion rate, turn/wave timeout rate, cards per round, and reconnection rate.

#### Scenario: Explosion rate is observable

- **WHEN** games complete over time
- **THEN** the explosion-rate metric reflects the share of rounds that exploded, supporting the ~30–40% balance target

### Requirement: Structured Tracing

The server SHALL emit its tracing as OpenTelemetry spans following a documented,
versioned span tree (room → game → round → wave → commit/resolve → score, plus
inbound-message handling, reconnection, and database writes), bridged from the
server's existing `tracing` instrumentation so there is a single instrumentation
surface. Spans carry stable, versioned attribute names; sensitive game state rides
in span attributes and may reach the trusted, operator-only trace backend, but is
never carried on the player wire. Existing structured (JSON) logging remains
available alongside the OTEL bridge.

#### Scenario: A phase transition is emitted as an OTEL span

- **WHEN** a room transitions between phases
- **THEN** an OTEL span records the transition with room and phase context, nested
  under that room's `room.lifetime` span and available to the in-process
  span-lifecycle consumer

#### Scenario: Database writes are traced

- **WHEN** the server performs its post-game persistence write
- **THEN** a `db.write` span records the write with room/game context

