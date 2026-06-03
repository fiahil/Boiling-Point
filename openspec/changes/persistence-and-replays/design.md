## Context

`persistence.rs` (sqlx/Postgres) and the `to_game_result` bridge (`game/runner.rs`) are
complete and tested but never called at runtime: `AppState` has no `PgPool`, `main.rs`
never connects/migrates, and `run_game` ends at the `GameOver` broadcast (server-review
**F4**). Separately, replays need data the engine doesn't keep yet — per-wave actions
are consumed and discarded. This change wires persistence and adds the replay path,
reusing the deterministic seed the engine already threads through every game.

## Goals / Non-Goals

**Goals:**
- Completed games persist match results **and** a replay payload, on the live path.
- The replay payload is timeless (survives engine changes) and fits one DB column.
- Persistence is optional infra: no DB configured ⇒ play still works.

**Non-Goals:**
- Player profiles / accounts / rating (→ roadmap).
- Replay *playback* UI or an HTTP/admin replay endpoint (separate, later change).
- Event-sourcing / spectating / partial-replay infrastructure (v2 roadmap).
- Any player-wire protocol change.

## Decisions

- **D1 — Hybrid replay payload (see [research.md](research.md) R1).** Encode
  `{ format_version, engine_version, config_fingerprint, seed, action_log }` as
  MessagePack, base64 into one column, with an `integrity_hash` over the bytes.
  Reconstruct by re-running the pinned engine; migrate/re-render on incompatible engine
  change. The input log is small because the engine is deterministic from `seed` +
  config.
- **D2 — Engine action recorder.** Add a recorder to the round/wave path that appends
  each wave's ordered actions (commit/pass/lock-in and any effect targets) to a
  per-game log. It must capture enough to replay deterministically — including the
  Recall target once that gap (tui review T4) is closed; until then, record what the
  wire carries and note the limitation.
- **D3 — Wire through `AppState`.** `--database-url`/`DATABASE_URL` → `connect` +
  `run_migrations` at boot → `Option<PgPool>` in `AppState` → passed to `run_game` →
  `persist_game(result, replay)` at `GameOver`. `None` pool ⇒ skip writes (log once).
- **D4 — Schema.** New `0002_*` migration adds replay storage (prefer a 1:1
  `game_replays` table so the `games` row stays lean). Reuse the existing
  `to_game_result` bridge for the results half; the async path must also populate
  `cards_played`/per-round analytics it currently omits (server-review F2/F4 note).

## Risks / Trade-offs

- **[Risk] Re-run-to-render couples replay validity to engine version** → mitigated by
  the `engine_version` tag + a migration/re-render path; for extra safety we may cache a
  resolved public-event snapshot alongside the input log (storage cost vs. robustness).
- **[Risk] Determinism must hold exactly** (any nondeterminism breaks replay) → the
  engine already seeds all RNG from one `u64` and makes "random" choices deterministic
  (e.g. Expose); add a replay round-trip test (record → re-run → identical event stream)
  to guard it, mirroring the existing determinism test.
- **[Risk] Action recorder on the hot path** → it's an append of small values per wave;
  negligible, and only retained until the completion write.
- **[Trade-off] Reusing the sync `to_game_result`** nudges toward converging the two
  loops (review-remediation **F2**); coordinate so the async path produces the same
  result struct.

## Migration Plan

Additive and optional. Ship the migration (`0002`), then the recorder, then the wiring.
No DB configured ⇒ no behavior change. Backfill is not required (no prior replays). The
player wire is untouched. Order: schema → action recorder + replay encode/decode (+
round-trip test) → runtime wiring (`AppState`/`main.rs`/`run_game`).

## Open Questions

- Cache a resolved event-snapshot beside the input log for cheaper playback, or
  re-run-only until playback exists? (Lean re-run-only for v1; revisit when playback is
  built.)
- Exact `engine_version` source — a manual constant bumped on engine changes, or derived
  from a hash of the engine crate? (Start with a manual constant.)
