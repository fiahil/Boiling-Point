# Boiling Point — Agent Guidelines

## Constitution (v2.2.0)

### I. Server-Authoritative

The Rust server is the single source of truth for all game state.
Clients are untrusted renderers.

- The server MUST validate every player action before applying it
- The server MUST never send information a player should not have
- The server MUST compute all scores, thresholds, and outcomes
- No game logic in the client — the client renders state and sends player intents
- Invalid actions receive an error response with no state change

### II. Agent-Driven Development

The codebase MUST support Claude as an autonomous co-developer
operating in a closed code-render-screenshot-adjust loop.

- All source files MUST be fully agent-writable — no binary editor state or GUI-only configuration
- Three testing layers MUST be maintained alongside game code:
  1. Unit tests — public API surfaces and component-level behaviour, plus
     transport/integration tests that boot the in-process server and drive it over
     the real wire protocol
  2. End-to-end tests — few but important, each covering one specific aspect of a
     game (matchmaking, launching a game, playing a round, triggering a boom, …).
     The e2e suite lives server-side and runs fully headless: it drives test-bot
     clients (`boom2-ai-client`, bot brain) against a real server process —
     reproducible seeded scenarios, **no mocking**, and no infrastructure beyond
     the server and the test bots (e2e tests MUST NOT require a database). The AI
     client itself carries only minimal unit testing; it is the instrument, not
     the home of the suite. The test harness MUST manage the server/bot processes
     and capture their outputs so deterministic games yield deterministic
     assertions
  3. Visual client tests — Playwright screenshots + DOM assertions, landing with the
     web client (`adopt-pixi-client`)
- Documentation MUST be kept current — a change that alters behaviour, specs, or
  design updates the corresponding docs (`openspec/` contracts, `docs/` rationale,
  this constitution for governance) in the same change
- Agent testability is a first-class selection criterion for client technology decisions
- The **bot harness is revived** as a live workspace member (`bot-harness/`,
  change `boom2-combat-core`) — the at-scale balance instrument Principle IV
  mandates. The Claude-as-player harness and the TUI reference client remain
  retired to `archive/` — revivable, not deleted

### III. Start Simple, Scale Later

Every architectural decision MUST start with the simplest viable option.
Complexity MUST be justified against a simpler rejected alternative with documented reasoning.

- Single binary monolith before service split
- Post-game persistence before event sourcing
- Invite links before matchmaking
- Exact threshold clues before fuzzy clues
- Anonymous session auth before OAuth
- Single server before horizontal scaling
- When in doubt, build the thing that works today and design the seam for the thing you might need tomorrow

### IV. Playtest-Driven Balance

Game mechanics, scoring values, thresholds, and card effects are hypotheses until validated by playtesting.

- Design documents MUST mark unvalidated numbers with "needs playtesting"
- Balance changes MUST be data-informed — server telemetry, the admin balance
  dashboard, structured player feedback, or revived-harness statistics
- Before large balance reworks (e.g. boom2) ship, at-scale automated playtesting MUST
  run — the revived bot harness (`bot-harness/`, reinstated with `boom2-combat-core`)
  is the standing instrument for running thousands of seeded games to surface
  degenerate strategies and derive the balance numbers
- No balance number is sacred — if data says change it, change it

## Technology Stack

**Server (decided):**
- Rust with Axum + Tokio async runtime
- PostgreSQL for player accounts, match history, leaderboards
- MessagePack over WebSocket for real-time game communication
- serde for serialization with JSON fallback for debugging

**Client (decided — v1.1.0): PixiJS (web + mobile hybrid).**

PixiJS v8 + TypeScript on a WebGL/WebGPU canvas, web-first, packaged for iOS/Android as a
hybrid (Capacitor) app from one codebase. The client is a pure renderer of server state
(Principle I); a thin DOM overlay carries selectable/accessible text (room code, chat,
names, scores); the TypeScript wire types are generated from the Rust `protocol` crate so
the client cannot drift. Selected in change `adopt-pixi-client`
(`openspec/changes/adopt-pixi-client/`; full rationale and the rejected/deferred
alternatives in `docs/03_architecture/03_tech-stack-exploration.md`).

| Candidate | Core bet | Outcome |
|---|---|---|
| **PixiJS + Capacitor** | Web-first reach, agent-writable, GPU spectacle, one codebase → web + mobile | **Chosen** |
| Flutter / Flame | Polished native feel, mature mobile exports | Deferred — revisit for a premium native app |
| Macroquad | Full Rust stack, shared types | Rejected — immature text/a11y/mobile; type-sharing solved via codegen |
| Godot | Fastest to polished game feel, full 2D editor | Rejected — editor-driven workflow conflicts with the agent-writable closed loop (§II) |

The Rust TUI was the v1 agent-test reference client; it is retired to
`archive/tui-client/` (change `retire-v1-harnesses`). Agent testability remains a
first-class selection criterion for any client technology decision.

**Project structure:**

```
├── server/        # authoritative game logic (Axum + Tokio) — cargo workspace member
├── protocol/      # wire protocol types, game enums, serde derives (canonical source)
├── bot-harness/   # headless balance harness (revived with boom2-combat-core) — workspace member
├── clients/
│   └── web/       # graphical client — TypeScript + PixiJS (lands with adopt-pixi-client)
└── archive/       # retired v1 components — tui-client, agent-harness, playtest.sh —
                   # revivable, not deleted
```

## Governance

This constitution supersedes all other development practices for the Boiling Point project.
When a practice conflicts with a principle above, the principle wins.

**Amendment procedure:**
1. Propose the change with rationale
2. Document the version bump type (MAJOR/MINOR/PATCH)
3. Update this file
4. Record the change in a commit message

**Versioning (semantic):**
- MAJOR: Principle removal, redefinition, or backward-incompatible governance change
- MINOR: New principle or section added, material expansion of existing guidance
- PATCH: Clarifications, wording fixes, non-semantic refinements

**Compliance:** All implementation plans MUST include a Constitution Check section.
Violations MUST be documented with justification and rejected simpler alternative.

**Amendment log:**
- **v2.2.0 (2026-06-11)** — MINOR. Revived the bot harness from `archive/` to a
  live workspace member (`bot-harness/`), satisfying Principle IV's pre-ship
  mandate for the boom2 rework: Principle II's archive bullet and Principle IV's
  reinstatement bullet now reference the live harness, and the project structure
  gains `bot-harness/`. Change `boom2-combat-core` (which also re-derived the
  blind-volatility economy — boiling point 31–43 at a ~45% explosion rate).
- **v2.1.1 (2026-06-11)** — PATCH. Clarified the e2e layer's placement: the suite
  lives server-side and runs fully headless; `boom2-ai-client` is the instrument it
  drives, not the home of the suite, and the AI client itself carries only minimal
  unit testing.
- **v2.1.0 (2026-06-11)** — MINOR. Expanded Principle II's testing layers from two to
  three: unit tests scoped to public API surfaces and component-level behaviour; a
  new end-to-end layer (few but important, one scenario per game aspect —
  matchmaking, launching a game, playing a round, triggering a boom) driven by
  test-bot clients (`boom2-ai-client`) with no mocking and no infrastructure beyond
  the server and bots (no database), harness-managed processes with captured outputs
  for deterministic assertions; and a standing docs-currency duty (docs updated in
  the same change that alters behaviour).
- **v2.0.0 (2026-06-11)** — MAJOR. Retired the v1 test/reference components to
  `archive/` (`tui-client`, `bot-harness`, `agent-harness`, `playtest.sh`) and
  redefined Principle II's mandatory testing layers (server tests now; Playwright
  visual tests when `clients/web` lands; archived harnesses revivable) and Principle
  IV's standing bot-harness mandate (at-scale runs reinstated on revival, required
  before large balance reworks ship). Restructured the client tree under `clients/`.
  Change `retire-v1-harnesses`.
- **v1.1.0 (2026-06-04)** — MINOR. Resolved the client technology decision: adopted
  **PixiJS (web + mobile hybrid via Capacitor)**, recorded Flutter/Flame as deferred and
  Macroquad/Godot as rejected, and retired the "`client/` compiles to WASM" project-
  structure note (the graphical client is TypeScript/Pixi, not Rust→WASM). Rationale,
  alternatives, and specs in change `adopt-pixi-client`.
- **v1.0.0** — Initial constitution.
