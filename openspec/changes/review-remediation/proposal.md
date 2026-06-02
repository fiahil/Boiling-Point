## Why

The server code review ([docs/reviews/server-review.md](../../../docs/reviews/server-review.md))
recorded five findings. F4 (persistence) is handled by the separate
`persistence-and-replays` change. This change closes the other four — one is a direct
constitution **§I** compliance gap (invalid actions get no error response), and the
rest are robustness/coverage gaps that make the *shipping* path less safe than the
tested engine core.

## What Changes

- **F1 — Invalid in-wave actions receive an error response (§I).** Today
  `collect_wave` silently drops a bad commit/pass/lock-in/emote (`session.rs`). The
  server SHALL reply with the existing error codes (`NotYourCard`, `LockedOut`,
  `WrongPhase`, `InvalidEmote`) and apply no state change, making the in-wave path
  consistent with the lobby/handshake paths. The error response carries **no hidden
  state**, so it does not weaken blind volatility.
- **F3 — The secret boundary enforces itself.** Route all server→client sends through
  the existing `Outbound`/`Audience` type so the `is_private_only()` debug-assert
  actually guards against broadcasting a private message, and add an end-to-end test
  that scans a whole game's broadcast stream for leaked secrets.
- **F2 — Converge the two game loops** (internal). The async `run_game` SHALL delegate
  the round/wave/scoring/deathmatch orchestration to the same tested core the sync
  `Game::play_out` uses (e.g. drive `Game` via a network-backed `Decider`), or — at
  minimum — gain engine-level tests over the async path. No external behavior change.
- **F5 — Polish** (internal): replace the invariant `unwrap()`s on the async path with
  `expect`/`entry`, and fix the "7-step"→6-category doc nit in `resolve.rs`. *(The
  logging-level sub-item is already resolved by the `--log-level` flag.)*

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
<!-- none — these are implementation/test fixes that bring the server into compliance
     with EXISTING `wire-protocol` requirements; no spec-level behavior changes. -->

> The behavior F1 and F3 require is **already specified**: the `wire-protocol` spec's
> *"Fire-and-Forget Invalid Actions"* requirement mandates an `Error` (not a silent
> drop) for an action the server can't apply, and *"Audience-Scoped Messages"* mandates
> private routing / no secrets in broadcasts. This change makes the implementation meet
> those requirements (plus a whole-game no-leak test), so it carries **no spec delta**.

## Impact

- **Code:** `server/src/session.rs` (collect_wave error replies; delegate orchestration;
  remove `unwrap`s), `server/src/game/runner.rs` (shared core / `Decider` seam),
  `protocol/src/server.rs` (`Outbound`/`Audience` usage), `server/src/game/resolve.rs`
  (doc nit). New end-to-end no-leak test under `server/tests/` or `transport.rs`.
- **Contract:** clients may now receive `Error` messages during a wave; clients already
  handle `Error` (the TUI toasts it), so this is additive.
- **No** schema/migration impact. No dependency changes.
