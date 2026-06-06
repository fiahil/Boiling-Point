# Boiling Point — V2 / Post-Launch Roadmap

Features intentionally **out of v1 scope**, parked for a post-launch (v2) pass.
This is a product/platform roadmap — distinct from the *design* deferrals in
[02_game-design.md §18](02_game-design.md) (round objectives, spectator/replays,
modifier expansions), and distinct from the architecture notes in
[`03_architecture/`](03_architecture/).

Per the [constitution](../CLAUDE.md) Principle III (*Start Simple, Scale Later*),
v1 ships the simplest viable thing; these are the "scale later" items with the
seams designed in now.

---

## Identity, Rating & Skill-Based Matchmaking

The v1 stance (see [02_game-design.md §14](02_game-design.md)): **matchmaking yes**
(simple table-filling queue, FIFO/next-open-table, **not** skill-based), running
on **anonymous session tokens** with **no persistent accounts and no rating.**

Moved to v2:

| Feature | Why it's v2 | Dependency / note |
|---|---|---|
| **Persistent player accounts** | v1 uses anonymous per-session tokens; cross-game identity isn't needed to fill tables. | Either lightweight device-bound anonymous accounts or OAuth (Google/Discord). The server doc's "OAuth later" lives here. **This is the unlock** — rating and skill-based matchmaking both depend on it. |
| **Player rating** | No persistent identity in v1 → nothing to attach a rating to. | Free-for-all results need a **multiplayer rating model — TrueSkill / Weng-Lin**, *not* 2-player Elo — which is why the v1 schema carries no rating column. |
| **Skill-based matchmaking** | Requires ratings to match by skill. | v1's table-filling queue is the seam; v2 swaps the matching policy without changing the queue's shape. |

**Ordering:** persistent accounts first (the unlock), then rating, then
skill-based matchmaking on top of rating.

---

## Deployment, Delivery & CI/CD

Per Principle III (*Start Simple, Scale Later*), v1 is locally runnable behind a
CI **gate** — `.github/workflows/ci.yml` runs `fmt`, `clippy` (warnings-as-errors)
and the **unit** tests on every push to `main` and every PR — but nothing is
hosted, there's no deploy step, and there's no public web presence. v2 turns the
project into a continuously deployed, publicly reachable service.

| Feature | Why it's v2 | Dependency / note |
|---|---|---|
| **Landing page** | v1 distributes via invite links to people who already know the game; no acquisition surface is needed to validate the core loop. | Static marketing page (what the game is, screenshots/trailer, a "play now" → create/join room CTA). Sits alongside or in front of the PixiJS `web-client/`, which is the "play" target. |
| **Fuller tests in CI** | v1 CI gates `fmt` + `clippy` + **unit** tests only; `make test-unit` deliberately skips the transport tests that boot an in-process server. | Extend CI to the three Principle II testing layers: transport/integration tests, the **bot-harness** balance runs (seeded, deterministic), an **agent-harness** Claude-as-player smoke, and **web-client** build + Playwright visual tests once the Pixi client lands. |
| **Continuous deployment pipeline** | v1 has a CI gate but no deploy step — releases are manual/local. | CD layered on the existing CI: build + publish the server container and web-client bundle, run DB migrations, and promote on green `main`. Gated behind the fuller test suite above. |
| **Deployment architecture & target** | v1 runs as a single local binary + local Postgres; nothing is hosted. | Pick a target — a managed container host + managed Postgres is the simplest viable, and the Principle III single-binary monolith maps cleanly to one container. Decide TLS/WebSocket ingress, config/secrets, DB backups, and the staging→prod path. The single-server stance is the seam; horizontal scaling stays out of v2. |

**Benchmarks** fold into this work: the [Server benchmarks](#other-post-v1-candidates)
regression harness (below) is meant to be *tracked over time*, so its seeded runs
land in this CI/CD pipeline once it exists.

**Ordering:** fuller tests in CI first (they gate everything), then pick the
deployment target/architecture, then the CD pipeline on top, with the landing
page in parallel.

---

## Other Post-V1 Candidates

These also sit beyond v1. (Some overlap with the design-side deferrals in
02_game-design.md §18 — cross-referenced, not duplicated.)

- **Round objectives** — per-round scoring tweaks to nudge specific game-theory
  dilemmas. The modifier stack covers round-to-round variety in v1; revisit if
  rounds feel samey. *(Design deferral — see 02_game-design.md §18.)*
- **Spectator mode & replays** — needs an append-only event log; v1 persists
  post-game only. *(Design deferral — see 02_game-design.md §18.)*
- **Cauldron-modifier expansions** — more modifiers beyond the launch 6, once the
  stacking system is validated. *(Design deferral — see 02_game-design.md §18.)*
- **OAuth / cross-device identity** — folds into the persistent-accounts work
  above.
- **Player profiles** — per-player career stats, history, and identity surfaced
  on top of persisted match results. Depends on **persistent accounts** (above):
  the v1 *persistence-and-replays* work stores match results and replays but
  attaches **no** profile or cross-game identity. Moved here out of the v1
  persistence scope.
- **Server benchmarks** — a performance-regression harness, deliberately out of
  v1 (correctness and balance come first); the regression runs land in the v2
  CI/CD pipeline (see *Deployment, Delivery & CI/CD* above). Two layers:
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
- **Delivery:** CI gate now (`fmt`/`clippy`/unit) → fuller test layers + CD later
  (additive; the Makefile targets and bot/agent harnesses are the seams).
- **Hosting:** single local binary + local Postgres now → managed container +
  managed Postgres later (same monolith, one container).
