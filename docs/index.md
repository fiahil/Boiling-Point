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
| Dive into the **boom2** (v2 core) rework — rationale + decision log behind the OpenSpec saga | [06_boom2/](06_boom2/index.md) |
| Build UI to the locked visual direction (Apothecary Ink) | [07_design-system.md](07_design-system.md) |

## Map of the docs

```
docs/
├── index.md                       ← you are here (hub)
├── 01_getting-started.md          prerequisites, how to build / run / test
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
│   └── 02_server-review.md            server code review
├── 05_roadmap.md                  v2 / post-launch features and the seams left for them
├── 06_boom2/                      the boom2 (v2 core) rework — design corpus behind the boom2 OpenSpec saga
│   ├── index.md                   chapter hub: design docs ↔ the 7 boom2 OpenSpec changes
│   ├── 01_depth-and-complexity.md  PROPOSAL — core-depth adjustments (Vote/Spell, Brewers, Ingredients)
│   └── 02_toward-a-v2-core.md      DECISION LOG — the committed v2 core rework (deeper/strategic/political)
├── 07_design-system.md            Apothecary Ink — the locked visual direction (tokens, type, card, motion)
└── 99_archive/                    resolved / superseded notes, kept for history
    ├── index.md
    ├── naming-ideas.md
    ├── tui-client-review.md       review of the retired TUI (component in archive/)
    └── agent-harness-review.md    review of the retired agent harness (in archive/)
```

## Related, outside `docs/`

- **[`CLAUDE.md`](../CLAUDE.md)** — the project constitution (server-authoritative,
  agent-driven, start-simple, playtest-driven).
- **[`openspec/`](../openspec/)** — the change-management workflow. Active proposals
  live in `openspec/changes/`; the current resolved capability specs live in
  `openspec/specs/`; shipped work is in `openspec/changes/archive/` (ordered by
  ISO-date prefix). These are the authoritative requirements; `docs/` is the
  human-facing rationale and overview.
- Per-crate READMEs in `protocol/` and `server/`, plus `ops/` for the admin surface
  and balance dashboard. Retired v1 components keep their READMEs under `archive/`
  (inventory + revival recipe in [`archive/README.md`](../archive/README.md)).
