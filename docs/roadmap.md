# Boiling Point — V2 / Post-Launch Roadmap

Features intentionally **out of v1 scope**, parked for a post-launch (v2) pass.
This is a product/platform roadmap — distinct from the *design* deferrals in
[game-design.md §18](game-design.md) (round objectives, spectator/replays,
modifier expansions), and distinct from the historical design brainstorms in
[`architecture/`](architecture/).

Per the [constitution](../CLAUDE.md) Principle III (*Start Simple, Scale Later*),
v1 ships the simplest viable thing; these are the "scale later" items with the
seams designed in now.

---

## Identity, Rating & Skill-Based Matchmaking

The v1 stance (see [game-design.md §14](game-design.md)): **matchmaking yes**
(simple table-filling queue, FIFO/next-open-table, **not** skill-based), running
on **anonymous session tokens** with **no persistent accounts and no rating.**

Moved to v2:

| Feature | Why it's v2 | Dependency / note |
|---|---|---|
| **Persistent player accounts** | v1 uses anonymous per-session tokens; cross-game identity isn't needed to fill tables. | Either lightweight device-bound anonymous accounts or OAuth (Google/Discord). The server doc's "OAuth later" lives here. **This is the unlock** — rating and skill-based matchmaking both depend on it. |
| **Player rating** | No persistent identity in v1 → nothing to attach a rating to. | Free-for-all results need a **multiplayer rating model — TrueSkill / Weng-Lin**, *not* 2-player Elo. The server doc's `players.elo_rating` column is a placeholder and should be revisited. |
| **Skill-based matchmaking** | Requires ratings to match by skill. | v1's table-filling queue is the seam; v2 swaps the matching policy without changing the queue's shape. |

**Ordering:** persistent accounts first (the unlock), then rating, then
skill-based matchmaking on top of rating.

---

## Other Post-V1 Candidates

These also sit beyond v1. (Some overlap with the design-side deferrals in
game-design.md §18 — cross-referenced, not duplicated.)

- **Round objectives** — per-round scoring tweaks to nudge specific game-theory
  dilemmas. The modifier stack covers round-to-round variety in v1; revisit if
  rounds feel samey. *(Design deferral — see game-design.md §18.)*
- **Spectator mode & replays** — needs an append-only event log; v1 persists
  post-game only. *(Design deferral — see game-design.md §18.)*
- **Cauldron-modifier expansions** — more modifiers beyond the launch 6, once the
  stacking system is validated. *(Design deferral — see game-design.md §18.)*
- **OAuth / cross-device identity** — folds into the persistent-accounts work
  above.
- **Player profiles** — per-player career stats, history, and identity surfaced
  on top of persisted match results. Depends on **persistent accounts** (above):
  the v1 *persistence-and-replays* work stores match results and replays but
  attaches **no** profile or cross-game identity. Moved here out of the v1
  persistence scope.
- **Server benchmarks** — a performance-regression harness, deliberately out of
  v1 (correctness and balance come first). Two layers:
  - **Engine micro-benchmarks** (`criterion`): hot paths in the round engine —
    wave resolution, depile/scoring, deck deal/reshuffle, modifier stacking —
    tracked over time to catch regressions.
  - **WebSocket load harness**: many concurrent rooms driven over the real wire
    (reuse the `bot-harness` WebSocket transport), measuring tick latency,
    broadcast fan-out cost, and memory per room, against target throughput/latency
    budgets. The bot harness's existing seeded batch runner is the seam.

---

## What v1 Deliberately Keeps Simple (the seams)

So the v2 work is a swap, not a rewrite:

- **Auth:** anonymous session token now → persistent account later (additive).
- **Matchmaking:** table-filling queue now → skill-based policy later (same queue).
- **Persistence:** post-game results now → append-only event log later (enables
  replays/spectating).
