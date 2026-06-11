# boom2-ai-client

## Why

Constitution v2.0.0 retired the v1 harnesses to `archive/` and left a standing mandate
(Principle IV): **before boom2 ships, at-scale automated playtesting MUST be reinstated** —
thousands of games to surface degenerate strategies before humans play. Meanwhile the v2
game (deeper, longer, political — [docs/06_boom2/02_toward-a-v2-core.md](../../../docs/06_boom2/02_toward-a-v2-core.md))
also wants **AI seat-fillers as a product feature**: a 4-player political game needs full
tables, and the v2 pacing (~25s/15s waves, ~90s draft) finally fits an LLM player's
latency. Rather than reviving two divergent stacks (Rust bot-harness + TypeScript
agent-harness, each with its own hand-built client layer), this change builds **one Rust
AI client** that serves both purposes with **two pluggable brains**.

## What Changes

- **One new Rust AI client** at `clients/ai/` (cargo workspace member) — a pure consumer
  of the public wire protocol, replacing both archived v1 harnesses for the v2 game. The
  archive stays archived (reference material, not a dependency).
- **Strict client/server firewall.** Only the `protocol/` crate is shared with the
  server. The client keeps its own player-visible data structures at all times — no
  server domain types, and no view-model field that *could* hold a secret (no boiling
  point, no opponents' hands, no deck realization). Even the in-process transport
  exchanges wire-protocol frames only, never domain objects.
- **Two brains, two implementations, one `Brain` trait**, each with its own settings:
  - **Bot brain** — deterministic Rust heuristics. Settings: archetype/persona, scripted
    draft policy, blunder epsilon, seed. Zero-cost, instant, reproducible.
  - **Agent brain** — Claude-driven. Settings: model, persona, difficulty, latency
    budget, fallback-to-bot-brain policy, auth mode and spend caps. (Direct API vs
    sidecar is a design decision.)
- **Decision frames in the protocol.** The server enumerates, per player, the **pending
  decision and its legal action set** (additive protocol capability, coordinated with
  `boom2-combat-core`). Every brain becomes `f(view, decision_frame) → action`; bots
  can't desync from v2 rules, agent tools are schema-derived, and the TUI/web clients
  get UI affordances for free.
- **Harness mode** (playtest): seeded batch runner over an in-process server, thousands
  of games, the **persona × Brewer × deck-archetype** balance matrix, aggregated stats
  and reports. In this mode the Apothecary draft is a **scripted experimental variable**
  (deck-archetype axis), not a brain decision. This mode IS the Principle IV
  reinstatement.
- **Seat-filler mode** (product): a long-running process that joins real rooms over
  WebSocket (invite code, enqueue, or server-requested fill), plays with either brain,
  and **never stalls a wave** — the agent brain degrades to its bot fallback on the
  latency budget. In this mode the agent brain genuinely drafts (the synergy hunt).
- **Full v2 decision surface:** Brewer pick (1 of 2), Apothecary draft (pantry +
  grimoire buckets + reserve), ingredient-or-pass per wave, optional spell + target
  (15 spells), and Active spell priming.

## Capabilities

### New Capabilities

- `boom-decision-frame`: the server-enumerated pending decision + legal action set per
  player — the protocol/server contract every brain (and rendering client) consumes.
- `boom-ai-client-core`: the shared Rust client — firewall rules, secret-free view
  model, WebSocket and in-process-frame transports, the `Brain` trait and decision loop.
- `boom-bot-brain`: the deterministic heuristic brain, its archetypes, and its settings.
- `boom-agent-brain`: the Claude-driven brain, its settings, and its timeliness/fallback
  contract.
- `boom-balance-harness`: harness mode — seeded batch runs, the persona × Brewer ×
  deck-archetype matrix, balance statistics and reports (Principle IV reinstatement).
- `boom-seat-filler`: product mode — joining real rooms, timeliness guarantees, persona
  presentation, and operational/cost controls.

### Modified Capabilities

<!-- The v1 harnesses were retired without standing capability specs; no existing
     spec's requirements change. Decision frames are additive protocol surface and are
     specced as a new capability rather than a wire-protocol delta, following the
     boom2-combat-core precedent for new message shapes. -->

None.

## Impact

- **Protocol crate:** new decision-frame message shapes (pending decision + legal
  actions). Coordinate with `boom2-combat-core`, which owns the v2 protocol surface and
  ships first.
- **Server:** legal-action enumeration (the validation logic inverted — Principle I
  already requires the server to know what's legal); a seam for requesting/admitting AI
  seat-fillers into rooms.
- **Workspace:** new `clients/ai/` cargo member; `clients/` now holds `web/` (TS) and
  `ai/` (Rust).
- **CI (`boom2-delivery`):** tasks 1.2/1.3 reference the archived bot/agent harnesses —
  they should target this client's harness mode and an agent-brain smoke instead.
- **Other boom2 changes:** `boom2-combat-core` 7.2/7.3, `boom2-brewers` 4.2,
  `boom2-apothecary` 5.2, and `boom2-identity` 5.1 each carry per-change harness tasks;
  those land against this client once it exists.
- **Dependencies:** `boom2-combat-core` (v2 protocol shapes — hard dependency);
  `boom2-brewers` / `boom2-apothecary` (matrix axes — the harness scales with what has
  landed); agent brain requires Claude access (auth/billing per design).
- **Cost/ops:** the agent brain spends real money; spend caps and per-decision budgets
  are first-class settings, and harness-mode batch runs default to the bot brain.
