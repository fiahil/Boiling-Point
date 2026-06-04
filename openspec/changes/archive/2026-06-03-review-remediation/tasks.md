## 1. F5 — Polish (mechanical, do first)

- [x] 1.1 Replace the invariant `unwrap()`s on the async path (`session.rs` hand/score/deathmatch-shed) with `expect("invariant: …")` or `entry().or_insert(0)`
- [x] 1.2 Fix the "fixed 7-step order" doc nit in `resolve.rs` to match the 6 `EffectCategory` variants
- [x] 1.3 Confirm the logging-level item is closed (the `--log-level` flag + `EnvFilter` already shipped)

## 2. F1 — Error responses for invalid in-wave actions

- [x] 2.1 In `collect_wave`, replace the `_ => {}` silent drop so a bad `CommitCard` (not in hand) replies `Error{NotYourCard}` with no state change
- [x] 2.2 A commit/pass/lock-in from a passed or locked-out player replies `Error{LockedOut}` (or `WrongPhase`) with no state change
- [x] 2.3 An off-palette emote mid-wave replies `Error{InvalidEmote}`, matching the lobby behavior (resolve the lobby-vs-wave inconsistency)
- [x] 2.4 Verify error payloads carry no hidden state (reason only — no pot/volatility/boiling-point)
- [x] 2.5 Tests: each invalid action yields the right error code and leaves game state unchanged

## 3. F3 — Enforce the secret-routing boundary

- [x] 3.1 Route server→client sends through `Outbound`/`Audience` so `is_private_only()` guards every send (replace the hand-written `broadcast`/`send_to` helpers)
- [x] 3.2 Add an end-to-end test that plays a full game and scans every broadcast frame for leaked secrets (boiling point, opponents' hands, deck) outside legitimate private/explosion disclosures
- [x] 3.3 Confirm the `is_private_only()` debug-assert fires in tests if a private message is broadcast

## 4. F2 — Converge the two game loops

- [ ] 4.1 Introduce (or reuse) a network-backed `Decider`/`DeathmatchDecider` that awaits real commits and surfaces the broadcasts the wire needs — **deferred** to a follow-up (chose D2's sanctioned fallback; see note below)
- [ ] 4.2 Have `run_game` drive `Game` through that core instead of re-deriving the round/wave/scoring/deathmatch flow — **deferred** with 4.1
- [x] 4.3 If full convergence is out of scope, instead add engine-level tests asserting the async path matches the sync path for a fixed seed — added in-process `run_game` tests (`session::tests`): a fixed seed is reproducible, mirroring `runner.rs`'s determinism guarantee
- [x] 4.4 Ensure the shipping path inherits determinism/stress coverage (parity with `runner.rs` tests) — `async_path_is_deterministic_for_a_fixed_seed` + `async_path_completes_across_many_seeds_without_panicking`

> **F2 scope note.** Full convergence (4.1/4.2 — a single orchestration core behind both
> loops via a network-backed `Decider`) is the riskiest edit and touches the live loop;
> the design's Open Question + Risk plan explicitly sanction shipping D2's fallback
> (async-path engine-level tests) and splitting convergence into a follow-up. Exact
> async==sync score equality is additionally blocked today by two real divergences this
> change documents rather than papering over: the two paths derive their RNG seeds
> differently (`seed` vs `seed ^ 0xBEEF_F00D`; deathmatch `^0xD3A7_4A7C` vs `^0xD3A7`),
> and the async path doesn't surface Recall'd cards to the client. The async path now
> carries its own determinism + no-panic stress coverage; **unifying the core is tracked
> in the follow-up `converge-game-loops` change.**

## 5. Validation

- [x] 5.1 `make check` green (fmt + clippy -D warnings + tests)
- [x] 5.2 Update [docs/reviews/server-review.md](../../../docs/reviews/server-review.md) finding statuses to resolved as each lands
