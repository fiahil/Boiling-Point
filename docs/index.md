# Boiling Point — Documentation

Boiling Point is a 4-player free-for-all card game with a **server-authoritative**
Rust backend: the server owns all game state and secrets, and clients are untrusted
renderers (see the [constitution](../CLAUDE.md)).

Pages and chapters are numbered (`01_`, `02_`, …) to give a reading order; each chapter
folder has its own `index.md`.

## Start here

| If you want to… | Read |
|---|---|
| Get the project running and play a game | [01_getting-started.md](01_getting-started.md) |
| Know the rules, cards, scoring, and balance knobs | [02_game-design.md](02_game-design.md) |
| Understand the system shape (crates, data flow, lifecycle) | [03_architecture/](03_architecture/index.md) |
| Review code health and release-readiness | [04_reviews/](04_reviews/index.md) |
| See what's parked for after v1 | [05_roadmap.md](05_roadmap.md) |
| Explore proposed core-depth adjustments (Vote/Spell, Brewers, Ingredients) | [06_depth-and-complexity.md](06_depth-and-complexity.md) |
| Track the committed v2 core rework (decision log) | [07_toward-a-v2-core.md](07_toward-a-v2-core.md) |

## Map of the docs

```
docs/
├── index.md                       ← you are here (hub)
├── 01_getting-started.md          prerequisites, how to build / run / test / playtest
├── 02_game-design.md              canonical game design (rules, cards, scoring, modifiers)
├── 03_architecture/               how the system is built
│   ├── index.md
│   ├── 01_overview.md             component map + lifecycle + state machine (ASCII diagrams)
│   ├── 02_server-infrastructure.md  topology, rooms, concurrency, persistence, scaling
│   ├── 03_tech-stack-exploration.md why Rust/Axum/Tokio/Postgres/MessagePack + the client choice
│   └── 04_span-schema-contract.md   observability span tree + attribute schema
├── 04_reviews/                    code health & launch readiness
│   ├── index.md
│   ├── 01_release-readiness-review.md cross-cutting v1-launch gate (start here)
│   ├── 02_server-review.md            server code review
│   ├── 03_tui-client-review.md        terminal client review
│   └── 04_agent-harness-review.md     Claude-as-player harness review
├── 05_roadmap.md                  v2 / post-launch features and the seams left for them
├── 06_depth-and-complexity.md     PROPOSAL — core-depth adjustments (Vote/Spell, Brewers, Ingredients)
├── 07_toward-a-v2-core.md         DECISION LOG — the committed v2 core rework (deeper/strategic/political)
└── 99_archive/                    resolved / superseded notes, kept for history
    ├── index.md
    └── naming-ideas.md
```

## Related, outside `docs/`

- **[`CLAUDE.md`](../CLAUDE.md)** — the project constitution (server-authoritative,
  agent-driven, start-simple, playtest-driven).
- **[`openspec/`](../openspec/)** — the change-management workflow. Active proposals
  live in `openspec/changes/`; the current resolved capability specs live in
  `openspec/specs/`; shipped work is in `openspec/changes/archive/` (ordered by
  ISO-date prefix). These are the authoritative requirements; `docs/` is the
  human-facing rationale and overview.
- Per-crate READMEs in `protocol/`, `server/`, `tui-client/`, `bot-harness/`, and
  `agent-harness/`, plus `ops/` for the admin surface and balance dashboard.
