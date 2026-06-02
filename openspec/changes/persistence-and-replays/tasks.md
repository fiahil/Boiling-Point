## 1. Schema

- [ ] 1.1 Add migration `migrations/0002_replays.sql`: a 1:1 `game_replays(game_id PK/FK → games, payload BYTEA/TEXT, format_version, engine_version, config_fingerprint, integrity_hash, created_at)`
- [ ] 1.2 Keep migrations idempotent (`CREATE TABLE IF NOT EXISTS`), runnable via the existing `run_migrations`

## 2. Engine action recorder

- [ ] 2.1 Add a per-game action recorder to the round/wave path capturing the ordered per-wave actions (commit/pass/lock-in, effect targets where carried)
- [ ] 2.2 Record the root seed and content-config fingerprint alongside the action log
- [ ] 2.3 Expose the recorded log from `GameOutcome`/the runner so the session can persist it
- [ ] 2.4 Note the Recall-target limitation until the wire carries it (cross-ref tui review T4)

## 3. Replay payload (encode/decode)

- [ ] 3.1 Define the payload struct `{ format_version, engine_version, config_fingerprint, seed, action_log }` + `integrity_hash`
- [ ] 3.2 Encode to a single compact column value (MessagePack → base64) and decode back
- [ ] 3.3 Reconstruct a game by re-running the pinned engine from the payload; expose a public-event stream
- [ ] 3.4 Replay round-trip test: record a played game → re-run from payload → identical waves/reveals/scores (guards determinism)
- [ ] 3.5 Integrity test: a tampered payload fails hash verification

## 4. Runtime wiring (supersedes F4)

- [ ] 4.1 Add `--database-url` (and `DATABASE_URL`) to the server CLI
- [ ] 4.2 At boot, when configured: `connect` a `PgPool`, `run_migrations`, store `Option<PgPool>` in `AppState`
- [ ] 4.3 Thread the pool to `run_game`; at `GameOver` build the `GameResult` (reuse `to_game_result`; populate `cards_played`/per-round analytics on the async path) and persist results + replay in one completion write
- [ ] 4.4 No DB configured ⇒ skip the write, log once, never fail startup or play
- [ ] 4.5 Trace the write as the `db.write` span (existing observability requirement)

## 5. Validation

- [ ] 5.1 Integration test (with a test Postgres): a completed game persists one game row, per-player rows, per-round rows, and one replay payload
- [ ] 5.2 The replay loads by game id, verifies, and reconstructs the public event stream
- [ ] 5.3 `make check` green; update server-review **F4** status to resolved
