## Why

The server has **two implementations of the round/wave/scoring/deathmatch flow**:
the synchronous `Game::play_out` (`server/src/game/runner.rs`) — the one with deep
determinism + 300-game stress + tie-break tests — and the async `session.rs::run_game`
that actually drives live games. This is finding **F2** in the
[server review](../../../docs/reviews/server-review.md).

The `review-remediation` change shipped F2's sanctioned fallback (async-path
determinism + stress tests) and **deferred full convergence to this change**. The two
loops can still drift — they already differ (the sync runner tracks `cards_played`
and per-round analytics; the async path does not) — and the path that ships is the
less-tested one. This change closes the gap by giving both paths **one orchestration
core**.

## What Changes

- **Single orchestration core (F2).** `run_game` SHALL delegate the
  round/wave/scoring/deathmatch orchestration to the tested engine core rather than
  re-deriving it — e.g. drive `Game` through a **network-backed `Decider` /
  `DeathmatchDecider`** that awaits real commits within the wave timer and surfaces the
  broadcasts the wire needs (`WaveOpened`, `WaveResolved`, peek/expose tells, depile,
  scoring, snapshots). No external (wire) behavior change.
- **Reconcile the two documented divergences** that block exact parity today:
  - **RNG seed derivation.** Align the async path's seeds with the sync path's
    (`run_game` uses `seed ^ 0xBEEF_F00D` for its `rng` and `seed ^ 0xD3A7` for the
    deathmatch; the sync runner uses `seed` and `seed ^ 0xD3A7_4A7C`). Pick one
    derivation and use it in both. Invisible in production (per-game seeds are random),
    but required for a parity test.
  - **Recall visibility.** The async path adds a Recall'd card back to the server-side
    hand but never tells the owning client, so a wire client can't track its true hand.
    Decide how the converged core surfaces a recalled card to its owner (e.g. a private
    `YourHand`/hand-delta) so client and server agree.
- **Prove parity.** Add an engine-level test asserting the converged async path
  produces the **same final scores as `Game::play_out` for a fixed seed**, replacing
  `review-remediation`'s self-determinism check with true sync==async equality.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
<!-- This is an internal refactor toward the EXISTING `wire-protocol` behavior; the
     observable wire contract is unchanged, so there is no spec delta. The Recall
     visibility decision MAY introduce a new private hand-update message — if so, it
     extends (does not change) the wire-protocol spec; resolve during design. -->

## Impact

- **Code:** `server/src/session.rs` (delegate orchestration to the core), reuse or
  extend `server/src/game/runner.rs` (`Game`, `Decider`, `DeathmatchDecider`,
  `to_game_result`), possibly `protocol/src/server.rs` (only if Recall surfacing needs
  a new private message).
- **Tests:** a sync==async parity test for a fixed seed; the async path keeps the
  determinism/stress coverage `review-remediation` added.
- **Risk:** this is the riskiest edit in the F-series — it touches the live game loop.
  Keep it behind the green `make check` suite; ship incrementally.
- **No** schema/migration impact. No dependency changes.
