# persistence-and-observability Specification

## Purpose
TBD - created by archiving change server-release-1. Update Purpose after archive.
## Requirements
### Requirement: Post-Game Persistence Only

The server SHALL persist results only after a game completes (`GameOver`), writing the
anonymous player records and **one consolidated game record** — queryable metadata,
denormalized summary stats, and the game's **replay payload** — in a single completion
write. No game state is persisted mid-game; a crash before completion loses only the
in-progress game. When no database is configured, the server SHALL play games to
completion normally and skip the completion write — persistence is optional
infrastructure, not a precondition for play.

#### Scenario: Completed game is written once

- **WHEN** a game reaches `GameOver` and a database is configured
- **THEN** the server writes the participants' player records and exactly one consolidated game row (metadata, summary stats, and replay payload), in a single completion write

#### Scenario: In-progress game is not persisted

- **WHEN** a game is still in progress
- **THEN** no game row has been written for it

#### Scenario: No database configured

- **WHEN** the server runs without a configured database URL
- **THEN** games play to completion normally and the completion write is skipped (logged once, never fatal)

### Requirement: Consolidated Schema

Persistence SHALL use PostgreSQL with two tables: `players` (UUID identity, display name,
created timestamp) and a single consolidated `game_replays` table holding one row per
completed game. The game row SHALL carry queryable metadata (start/end timestamps, the
player id array, and the winner id array — null when no winner was declared, multiple
entries for a tie), a per-player breakdown (final score, finish position, colour, cards
played) stored as JSON, denormalized `stats_*` summary columns (round count, player
count, explosions, total cards played, high/low score, and whether a deathmatch tiebreak
ran), and the **replay payload stored in a single column** carrying its format/engine
versions and an integrity hash. Per-round detail is recoverable by reconstructing the
replay rather than stored as separate rows.

#### Scenario: Results are queryable per player

- **WHEN** a completed game has been persisted
- **THEN** each participant's final score and finish position can be read from the game row's per-player JSON breakdown without decoding the replay payload

#### Scenario: Winners and summary stats are queryable

- **WHEN** a completed game has been persisted
- **THEN** its winner id array and `stats_*` summary columns (including whether a deathmatch ran) are queryable directly on the game row

#### Scenario: A game's replay is stored in one column

- **WHEN** a completed game has been persisted
- **THEN** its replay payload is retrievable from a single column keyed by game id, without loading it to query the metadata or summary stats

### Requirement: Replay Payload

The game's replay payload SHALL be a MessagePack body that re-runs the pinned engine to
reconstruct the full public event stream — the root seed plus the ordered per-wave action
log — and SHALL additionally carry an observational, timestamped log of every raw in-game
input the players sent (card commits, passes, lock-ins, and emotes), each stamped with
milliseconds since game start. The payload SHALL be wrapped by format/engine version tags
and an integrity hash over its bytes; a payload whose bytes do not match its hash SHALL be
rejected rather than reconstructed.

#### Scenario: Replay reconstructs the public event stream

- **WHEN** a stored replay is reconstructed against the engine version and content config it was recorded under
- **THEN** the re-run reproduces the game's public event stream, final scores, and winners identically

#### Scenario: Replay captures emotes and action timing

- **WHEN** a completed game's replay payload is decoded
- **THEN** it contains every in-game input each player sent — including emotes — with a millisecond timestamp relative to game start

#### Scenario: Tampered payload is rejected

- **WHEN** a stored replay payload's bytes do not match its integrity hash
- **THEN** reconstruction is refused rather than producing an incorrect replay

### Requirement: Anonymous Player Records

Player records SHALL be created from anonymous session authentication — keyed by the issued UUID with a display name — without any email or password.

#### Scenario: Player row exists for a participant

- **WHEN** a game completes
- **THEN** every participant has a player row identified by their session-issued UUID

### Requirement: Game-Balance Metrics

The server SHALL emit Prometheus-scrapable metrics covering at least: active groups,
groups created, **active games (games in progress)**, connected players, game and round
durations, explosion rate, turn/wave timeout rate, cards per round, and reconnection
rate.

#### Scenario: Explosion rate is observable

- **WHEN** games complete over time
- **THEN** the explosion-rate metric reflects the share of rounds that exploded, supporting the ~30–40% balance target

#### Scenario: Live games and groups are observable

- **WHEN** games are in progress and groups exist
- **THEN** the active-games gauge reflects the number of in-progress games and the active-groups gauge reflects the number of live groups

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

### Requirement: Runtime Persistence Wiring

The server SHALL, when a database URL is configured (via `--database-url` /
`DATABASE_URL`), connect a connection pool at startup, run idempotent migrations
before accepting connections, hold the pool in shared application state, and invoke
the post-game write at every `GameOver` on the live path. Absence of a URL SHALL
disable persistence cleanly rather than fail startup.

#### Scenario: Migrations run before serving

- **WHEN** the server starts with a configured database URL
- **THEN** it connects a pool and applies pending migrations before accepting player connections

#### Scenario: Completion write is invoked on the live path

- **WHEN** a live game reaches `GameOver` with a configured database
- **THEN** the running server (not only an ignored test) writes the game result and replay payload

