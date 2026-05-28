<!-- Sync Impact Report
Version change: N/A → 1.0.0 (initial ratification)
Modified principles: N/A (all new)
Added sections:
  - Core Principles (4 principles)
  - Technology Stack
  - Development Workflow
  - Governance
Removed sections: N/A
Templates requiring updates:
  - .specify/templates/plan-template.md — ✅ compatible (Constitution Check section is generic)
  - .specify/templates/spec-template.md — ✅ compatible (no constitution-specific references)
  - .specify/templates/tasks-template.md — ✅ compatible (phase structure aligns with principles)
Follow-up TODOs:
  - Client tech stack decision pending (Macroquad vs Godot vs Flutter/Flame)
-->

# Boiling Point Constitution

## Core Principles

### I. Server-Authoritative

The Rust server is the single source of truth for all game state.
Clients are untrusted renderers.

- The server MUST validate every player action before applying it
- The server MUST never send information a player should not have
- The server MUST compute all scores, thresholds, and outcomes
- No game logic in the client — the client renders state and
  sends player intents
- Invalid actions receive an error response with no state change

### II. Agent-Driven Development

The codebase MUST support Claude as an autonomous co-developer
operating in a closed code-render-screenshot-adjust loop.

- All source files MUST be fully agent-writable — no binary
  editor state or GUI-only configuration
- Three testing layers MUST be maintained alongside game code:
  1. Protocol bot harness (headless Rust bots for balance testing)
  2. Claude-as-player harness (structured JSON over WebSocket)
  3. Visual client tests (Playwright or equivalent)
- Agent testability is a first-class selection criterion for
  client technology decisions

### III. Start Simple, Scale Later

Every architectural decision MUST start with the simplest viable
option. Complexity MUST be justified against a simpler rejected
alternative with documented reasoning.

- Single binary monolith before service split
- Post-game persistence before event sourcing
- Invite links before matchmaking
- Exact threshold clues before fuzzy clues
- Anonymous session auth before OAuth
- Single server before horizontal scaling
- When in doubt, build the thing that works today and design
  the seam for the thing you might need tomorrow

### IV. Playtest-Driven Balance

Game mechanics, scoring values, thresholds, and card effects are
hypotheses until validated by playtesting.

- Design documents MUST mark unvalidated numbers with
  "needs playtesting"
- Balance changes MUST be data-informed — bot harness statistics
  (explosion rate, dominant strategy rate, round duration) or
  structured player feedback
- The bot harness MUST be able to run thousands of games to
  surface degenerate strategies before human playtesting begins
- No balance number is sacred — if data says change it, change it

## Technology Stack

**Server (decided):**

- Rust with Axum + Tokio async runtime
- PostgreSQL for player accounts, match history, leaderboards
- MessagePack over WebSocket for real-time game communication
- serde for serialization with JSON fallback for debugging

**Client (undecided — shortlist):**

| Candidate | Core Bet |
|---|---|
| Macroquad | Full Rust stack, shared types, agent-driven dev |
| Godot | Fastest to polished game feel, full 2D editor |
| Flutter/Flame | Proven cross-platform, mature 2D engine |

**Observability:**

- `tracing` crate + `tracing-subscriber` for structured logging
- `metrics` crate + Prometheus exporter for game-specific metrics
- Key metrics: explosion_rate, round_duration, turn_timeout_rate,
  dominant_strategy_rate

**Project structure:**

```
cargo workspace
├── server/        # authoritative game logic (Axum + Tokio)
├── client/        # game client (compiles to WASM for web)
├── shared/        # protocol types, game enums, serde derives
├── bot-harness/   # headless bot players for balance testing
└── agent-harness/ # Claude-as-player wrapper
```

## Development Workflow

- All changes go through feature branches
- Spec → Plan → Tasks → Implement pipeline via the Specify toolkit
- Every implementation plan MUST pass a Constitution Check gate
  before Phase 0 research begins, and re-check after Phase 1 design
- Commit after each task or logical group of changes
- Game design decisions deferred to playtesting MUST be tracked
  as open questions in the relevant design document

## Governance

This constitution supersedes all other development practices
for the Boiling Point project. When a practice conflicts with
a principle above, the principle wins.

**Amendment procedure:**

1. Propose the change with rationale
2. Document the version bump type (MAJOR/MINOR/PATCH)
3. Update this file and propagate changes to dependent templates
4. Record the change in the Sync Impact Report comment

**Versioning policy (semantic):**

- MAJOR: Principle removal, redefinition, or backward-incompatible
  governance change
- MINOR: New principle or section added, material expansion of
  existing guidance
- PATCH: Clarifications, wording fixes, non-semantic refinements

**Compliance review:**

- All implementation plans MUST include a Constitution Check section
- Violations MUST be documented in the Complexity Tracking table
  with justification and rejected simpler alternative

**Version**: 1.0.0 | **Ratified**: 2026-05-27 | **Last Amended**: 2026-05-27
