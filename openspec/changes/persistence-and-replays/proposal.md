## Why

Persistence is fully built but never called at runtime (server-review **F4**) — so
completed games are not saved. Rather than just wire the v1 module as-is, we are
**reworking** persistence around its two real consumers: **saving match results** and
**saving timeless replays**. Replays are a hard requirement ("game results MUST be
serialized into a neat format that allows timeless replays, fitting nicely into a
database column"). Player *profiles* are explicitly **out of scope** here and move to
the [roadmap](../../../docs/roadmap.md) (they depend on persistent accounts).

## What Changes

- **Wire persistence on the live path (F4).** Add a `--database-url`/`DATABASE_URL`
  option, connect a `PgPool` into shared state, run idempotent migrations at boot, and
  call the post-game write at every `GameOver`. With no URL configured, the server
  runs normally and skips the write (persistence is optional infra, not a precondition
  for play).
- **Record a per-wave action log.** The engine gains an action/event recorder (today's
  `WaveChoice` is transient) capturing the root seed, the content-config identity, and
  the ordered per-wave actions — enough to deterministically reconstruct a game.
- **Add timeless replays (`match-replays`).** Encode each game's replay as a single,
  compact, self-describing payload (a **hybrid** of the deterministic input log plus a
  `format_version`/`engine_version` tag and an integrity hash) that fits one DB column.
  A replay reconstructs by re-running the pinned engine version; on an incompatible
  engine change the payload is migrated/re-rendered rather than lost.
- **Extend the schema** with a replay payload column (on the game row or a 1:1
  `game_replays` table) via a new migration.
- **Out of scope:** player profiles / career stats / cross-game identity → roadmap. The
  existing anonymous per-game player record stays (it's the FK for match results).

## Capabilities

### New Capabilities
- `match-replays`: the per-wave action log, the timeless replay payload (encoding,
  versioning, integrity), and replay retrieval/reconstruction.

### Modified Capabilities
- `persistence-and-observability`: the post-game completion write now also stores the
  replay payload; the relational schema gains replay storage; persistence is explicitly
  runtime-wired and DB-URL-configured, degrading cleanly when unconfigured.

## Impact

- **Code:** `server/src/main.rs` (DB URL, pool, migrations at boot — extends the new
  clap CLI), `server/src/transport.rs` (`PgPool` in `AppState`), `server/src/session.rs`
  + `server/src/game/{runner,round}.rs` (action recorder; build & persist the result +
  replay at `GameOver`; reuse the existing `to_game_result` bridge),
  `server/src/persistence.rs` (write/read replay payload), `server/migrations/` (new
  `0002_*` migration).
- **Supersedes** server-review **F4** (do not wire the v1 module unchanged).
- **No player wire change.** No new client messages. (Replay *playback* surfaces — an
  endpoint or admin view — are a later, separate change.)
