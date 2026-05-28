# Implementation Plan: Boiling Point — Complete Game Design

**Branch**: `001-game-design` | **Date**: 2026-05-28 | **Spec**: `specs/001-game-design/spec.md`

**Input**: Feature specification from `specs/001-game-design/spec.md`

## Summary

Produce the final, complete game design for Boiling Point — a 4-player FFA card game with blind volatility, simultaneous waves, and color-based political scoring. This feature resolves all open design questions (multiplier system, shield balance, deck composition, scoring edge cases, deathmatch rules) and produces a full data model and WebSocket protocol contract ready for implementation.

## Technical Context

**Language/Version**: Rust stable (1.75+) for server, shared types, and bot harness. Client TBD (Macroquad/Godot/Flutter shortlist).

**Primary Dependencies**: Axum (HTTP/WebSocket), Tokio (async runtime), serde + rmp-serde (MessagePack serialization), PostgreSQL (persistence).

**Storage**: PostgreSQL for player accounts, match history, leaderboards. In-memory game state per room (tokio::spawn task per room, mpsc channels).

**Testing**: cargo test (unit/integration), bot harness (headless balance testing), agent harness (Claude-as-player), Playwright (visual client tests when client exists).

**Target Platform**: Web first (WASM client), server on Linux. Mobile/desktop later via client technology choice.

**Project Type**: Real-time multiplayer game — authoritative server + thin game client.

**Performance Goals**: 60fps client rendering, sub-100ms server action processing, 4 concurrent players per room. Bot harness: 1000+ games/minute for balance testing.

**Constraints**: Complete 5-round game in <15 minutes (SC-001). Individual rounds in 60–90 seconds (SC-002). Explosion rate 30–40% (SC-003).

**Scale/Scope**: 4-player games, single server, invite-link rooms. No matchmaking, no spectators, no horizontal scaling in v1.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Pre-Phase 0 Check

| Principle | Status | Evidence |
|---|---|---|
| **I. Server-Authoritative** | ✅ PASS | All game mechanics (scoring, thresholds, explosions, effects) are server-computed. Protocol contract enforces client-as-renderer: clients send intents, server validates and broadcasts state. No game logic in client. |
| **II. Agent-Driven Development** | ✅ PASS | All source files are agent-writable (Rust + text configs). Three testing layers designed: bot harness, agent harness, visual tests. Data-driven deck configuration enables agent tuning. |
| **III. Start Simple, Scale Later** | ✅ PASS | 4-player only, no modifiers, no matchmaking, anonymous auth, invite links, single server. Threshold tightening chosen over hybrid escalation. Floor division for scoring splits. No explicit wave limits. |
| **IV. Playtest-Driven Balance** | ✅ PASS | All numerical values (thresholds, deck distribution, timer durations) marked with playtesting validation triggers. Bot harness metrics defined (explosion rate, strategy diversity, wave count). |

No violations. No entries needed in Complexity Tracking.

### Post-Phase 1 Re-Check

| Principle | Status | Delta from Pre-Check |
|---|---|---|
| **I. Server-Authoritative** | ✅ PASS | Protocol contract explicitly enforces: server validates all actions, error response on invalid input, information visibility matrix prevents information leakage. |
| **II. Agent-Driven Development** | ✅ PASS | Data model uses Rust enums for state machines (compiler-enforced transitions). WebSocket protocol is structured MessagePack — directly consumable by agent harness. |
| **III. Start Simple, Scale Later** | ✅ PASS | 88-card deck is the minimum viable composition. Effect resolution order is deterministic (no ambiguity). Deathmatch explosion = shared victory (simplest resolution). |
| **IV. Playtest-Driven Balance** | ✅ PASS | Research doc specifies concrete playtesting triggers: >10% rounds exceeding 7 waves → add wave cap; explosion rate outside 30-40% → adjust threshold ranges. Bot harness metrics mapped to success criteria. |

No new violations.

## Project Structure

### Documentation (this feature)

```text
specs/001-game-design/
├── plan.md              # This file
├── research.md          # Phase 0: resolved all open design questions
├── data-model.md        # Phase 1: entities, enums, state machines, validation rules
├── quickstart.md        # Phase 1: development setup and workflow
├── contracts/
│   └── protocol.md      # Phase 1: WebSocket protocol (client↔server messages)
└── tasks.md             # Phase 2 output (NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
cargo workspace
├── server/
│   └── src/
│       ├── main.rs          # Axum server bootstrap
│       ├── room.rs          # Game room task (tokio::spawn per room)
│       ├── game.rs          # Round state machine, scoring, effects
│       ├── deck.rs          # Deck construction, shuffle, draw
│       ├── ws.rs            # WebSocket handler, message routing
│       └── db.rs            # PostgreSQL persistence (post-game)
├── shared/
│   └── src/
│       ├── lib.rs
│       ├── types.rs         # PlayerColor, CardColor, SpecialEffect enums
│       ├── protocol.rs      # Client↔Server message types (serde)
│       ├── card.rs          # CardDefinition, deck composition data
│       └── scoring.rs       # Scoring logic (shared for bot harness reuse)
├── bot-harness/
│   └── src/
│       ├── main.rs          # CLI: --games N --report
│       ├── bot.rs           # Heuristic bot strategies
│       └── stats.rs         # Explosion rate, strategy analysis
├── agent-harness/
│   └── src/
│       ├── main.rs          # Claude-as-player wrapper
│       └── adapter.rs       # WebSocket↔JSON structured adapter
└── client/                  # TBD pending technology decision
```

**Structure Decision**: Cargo workspace with 4 crates (server, shared, bot-harness, agent-harness) plus a future client crate. Shared crate enables compile-time type safety across server and bot harness. Client crate is deferred until technology decision is made.

## Complexity Tracking

No violations to justify. All design decisions follow Constitution Principle III (Start Simple).

## Phase 0 Output

**Artifact**: `specs/001-game-design/research.md`

9 research items resolved:
1. **R1** — Escalation: threshold tightening replaces multipliers
2. **R2** — Shield: 2 copies, wild-only, no scoring on success
3. **R3** — Deck: 88 cards (52 colored, 12 wild, 24 effect)
4. **R4** — Effect points: Peek 1, Dampen 1, Surge 1, Shield 0, Expose 1
5. **R5** — Scoring edges: floor division, 0-point pots resolve naturally
6. **R6** — Wave limits: none (playtesting trigger at >7 waves)
7. **R7** — Deathmatch: brew = standard scoring, explosion = shared victory
8. **R8** — Timer expiry: auto-pass (lock out)

## Phase 1 Output

**Artifacts**:
- `specs/001-game-design/data-model.md` — 9 entities, 7 enums, validation rules, full round lifecycle state diagram
- `specs/001-game-design/contracts/protocol.md` — WebSocket protocol with 30+ message types, visibility matrix, error codes
- `specs/001-game-design/quickstart.md` — Dev setup, build/run commands, environment config, observability metrics
