## Context

`review-remediation` resolved F1/F3/F5 and shipped F2's fallback: the async
`run_game` now has its own determinism + no-panic stress tests (`session::tests`). It
explicitly left **full convergence** to this change, and documented the two concrete
divergences that block exact sync==async parity (RNG seed derivation; un-surfaced
Recall'd cards). This change unifies the orchestration so the shipping path *is* the
tested path.

## Goals / Non-Goals

**Goals:**
- One orchestration core behind both the sync (`Game::play_out`) and async (`run_game`)
  paths — `run_game` drives the engine, it does not re-derive the flow.
- Exact parity: the async path matches `Game::play_out`'s final scores for a fixed seed.
- No observable wire-behavior change for clients.

**Non-Goals:**
- New game mechanics, cards, or balance changes.
- Persistence/replays (`persistence-and-replays`) or the room→group rename
  (`group-model`).

## Decisions

- **D1 — Network-backed `Decider` seam.** Prefer driving `Game` from `run_game` via a
  `Decider`/`DeathmatchDecider` whose `decide` awaits real commits over the room
  channel within the wave timer, and a small "presenter" hook that emits the wire
  broadcasts (`WaveOpened`/`WaveResolved`/peek-expose/depile/scoring/snapshots) at the
  right points. `Game` already exposes the `Decider` trait and the low-level pieces
  (`Round`, `resolve_wave`, `score_safe`, `explosion`, `run_deathmatch`); the work is
  threading async + presentation through them without leaking transport into the
  engine. If `Game`'s current shape makes this too invasive, the fallback is to extract
  `run_game`'s per-round/per-wave body into shared functions both paths call.
- **D2 — One seed derivation.** Use the **sync runner's** derivation in both paths
  (`rng = seed`, deathmatch `seed ^ 0xD3A7_4A7C`), changing `run_game`'s
  `seed ^ 0xBEEF_F00D` / `seed ^ 0xD3A7`. Production seeds are random per game, so this
  is invisible to players but makes the paths comparable.
- **D3 — Surface Recall to its owner.** When the converged core returns a Recall'd card
  to a hand, the owner must learn its hand grew. Cheapest option: re-send that player a
  private `YourHand` after the wave resolves (reuses an existing message, no spec
  delta). Evaluate vs a dedicated hand-delta message during implementation.

## Risks / Trade-offs

- **[Risk] Touches the live loop.** Keep every step behind `make check`; land
  incrementally (seed alignment + parity scaffold first, then the orchestration
  delegation). The parity test is the safety net — it fails loudly if the converged
  path diverges from the tested engine.
- **[Trade-off] A new private hand-update (D3) adds a message** vs leaving Recall
  invisible. Re-using `YourHand` avoids a spec delta; accept a little redundant traffic
  over a new message type unless profiling says otherwise.

## Open Questions

- Does the `Decider`-seam approach (D1) fit, or is the shared-function extraction the
  pragmatic landing? Resolve against the size of the async/presentation threading once
  the seam is prototyped.
- Is re-sending `YourHand` (D3) sufficient, or do clients need an explicit "card
  recalled" signal for UX? Defer to client needs.
