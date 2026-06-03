## MODIFIED Requirements

### Requirement: Post-Game Persistence Only

The server SHALL persist results only after a game completes (`GameOver`), writing the
game, its participants and their final results, optional per-round detail, and the
game's **replay payload**, in a single completion write. No game state is persisted
mid-game; a crash before completion loses only the in-progress game. When no database is
configured, the server SHALL play games to completion normally and skip the completion
write — persistence is optional infrastructure, not a precondition for play.

#### Scenario: Completed game is written once

- **WHEN** a game reaches `GameOver` and a database is configured
- **THEN** the server writes one game row, one result row per participant, the per-round rows, and one replay payload, in a single completion write

#### Scenario: In-progress game is not persisted

- **WHEN** a game is still in progress
- **THEN** no game, participant, round, or replay rows have been written for it

#### Scenario: No database configured

- **WHEN** the server runs without a configured database URL
- **THEN** games play to completion normally and the completion write is skipped (logged once, never fatal)

### Requirement: Relational Schema

Persistence SHALL use PostgreSQL with tables for players (UUID identity, display name, created timestamp), games (timestamps, round count), game_players (final score, finish position, per-player stats), game_rounds (threshold, exploded flag, volatility total, cards played) for analytics, and a **replay payload stored in a single column** (on the game row or a 1:1 `game_replays` table) carrying its format/engine versions and an integrity hash.

#### Scenario: Results are queryable per player

- **WHEN** a completed game has been persisted
- **THEN** each participant's final score and finish position can be retrieved by joining game_players to players

#### Scenario: A game's replay is stored in one column

- **WHEN** a completed game has been persisted
- **THEN** its replay payload is retrievable from a single column keyed by game id, without loading it to query the result analytics

## ADDED Requirements

### Requirement: Runtime Persistence Wiring

When a database URL is configured (via `--database-url` / `DATABASE_URL`), the server
SHALL connect a connection pool at startup, run idempotent migrations before accepting
connections, hold the pool in shared application state, and invoke the post-game write
at every `GameOver` on the live path. Absence of a URL SHALL disable persistence
cleanly rather than fail startup.

#### Scenario: Migrations run before serving

- **WHEN** the server starts with a configured database URL
- **THEN** it connects a pool and applies pending migrations before accepting player connections

#### Scenario: Completion write is invoked on the live path

- **WHEN** a live game reaches `GameOver` with a configured database
- **THEN** the running server (not only an ignored test) writes the game result and replay payload
