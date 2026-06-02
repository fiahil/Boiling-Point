# Boiling Point — Documentation

Boiling Point is a 4-player free-for-all card game with a **server-authoritative**
Rust backend: the server owns all game state and secrets, and clients are untrusted
renderers (see the [constitution](../CLAUDE.md)).

## Start here

| If you want to… | Read |
|---|---|
| Get the project running and play a game | [getting-started.md](getting-started.md) |
| Understand the system shape (crates, data flow, lifecycle) | [architecture/overview.md](architecture/overview.md) |
| Know the rules, cards, scoring, and balance knobs | [game-design.md](game-design.md) |
| See what's parked for after v1 | [roadmap.md](roadmap.md) |
| Review code health and release-readiness | [reviews/](reviews/) |

## Map of the docs

```
docs/
├── README.md                  ← you are here (index)
├── getting-started.md         prerequisites, how to build / run / test / playtest
├── game-design.md             canonical game design (rules, cards, scoring, modifiers)
├── roadmap.md                 v2 / post-launch features and the seams left for them
├── architecture/
│   ├── overview.md            component map + lifecycle + state machine (ASCII diagrams)
│   ├── server-architecture.md historical infra brainstorm (topology, rooms, persistence…)
│   ├── tech-stack-exploration.md  why Rust/Axum/Tokio/Postgres/MessagePack
│   └── span-schema-contract.md    observability span tree + attribute schema
└── reviews/
    ├── server-review.md           server code review (findings F1–F5)
    ├── agent-harness-review.md     Claude-as-player harness review
    ├── tui-client-review.md        terminal client review
    └── release-readiness-review.md cross-cutting v1-launch gate
```

## Related, outside `docs/`

- **[`CLAUDE.md`](../CLAUDE.md)** — the project constitution (server-authoritative,
  agent-driven, start-simple, playtest-driven).
- **[`openspec/`](../openspec/)** — the change-management workflow. Active proposals
  live in `openspec/changes/`; the current resolved capability specs live in
  `openspec/specs/`; shipped work is in `openspec/changes/archive/`.
- Per-crate READMEs in `protocol/`, `server/`, `tui-client/`, `bot-harness/`, and
  `agent-harness/`.
