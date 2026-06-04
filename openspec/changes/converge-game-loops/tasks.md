## 1. Reconcile the divergences (do first — small, enables the parity test)

- [ ] 1.1 Align the async path's RNG seed derivation with the sync runner's (`rng = seed`, deathmatch `seed ^ 0xD3A7_4A7C`) in `session.rs::run_game`
- [ ] 1.2 Surface a Recall'd card to its owner (D3: re-send a private `YourHand` after a wave that recalled, or a dedicated hand-delta) so client and server agree on the hand
- [ ] 1.3 Add a sync==async parity scaffold: a test that can drive `run_game` with the same decisions as `Game::play_out` for a fixed seed

## 2. Single orchestration core (F2)

- [ ] 2.1 Introduce (or reuse) a network-backed `Decider`/`DeathmatchDecider` that awaits real commits within the wave timer and surfaces the broadcasts the wire needs
- [ ] 2.2 Have `run_game` drive `Game` through that core instead of re-deriving the round/wave/scoring/deathmatch flow (or extract shared per-round/per-wave functions both paths call)
- [ ] 2.3 Carry over the data the async path currently drops (`cards_played`, per-round analytics) so the converged path can feed `to_game_result`

## 3. Prove parity & coverage

- [ ] 3.1 Assert the converged async path produces the **same final scores as `Game::play_out`** for a fixed seed (replacing `review-remediation`'s self-determinism check)
- [ ] 3.2 Keep the async path's determinism + no-panic stress coverage green

## 4. Validation

- [ ] 4.1 `make check` green (fmt + clippy -D warnings + tests)
- [ ] 4.2 Confirm no observable wire-behavior change (existing transport integration tests stay green); update [docs/reviews/server-review.md](../../../docs/reviews/server-review.md) F2 status to fully resolved
