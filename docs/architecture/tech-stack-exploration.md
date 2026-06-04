# Boiling Point — Tech Stack Exploration

> **Decision (2026-06-04, constitution v1.1.0):** the client is **PixiJS (web + mobile
> hybrid via Capacitor)** — a pure renderer of server state with a DOM overlay for
> selectable/accessible text and TypeScript wire types generated from the Rust `protocol`
> crate. **Flutter/Flame is deferred** (revisit for a premium native app); **Macroquad and
> Godot are rejected.** See change [`adopt-pixi-client`](../../openspec/changes/adopt-pixi-client/)
> and `CLAUDE.md`. The exploration below is retained as the historical record that led here
> (direction was prototyped and screenshot-verified in [`docs/ui-explorations/`](../ui-explorations/)).

## Design Constraints

The tech stack must support:

1. **Authoritative Rust server** — non-negotiable, already decided
2. **Real-time multiplayer** via WebSockets (3-4 players per room, fast rounds)
3. **Agent-friendly development** — Claude as co-developer with a closed code → render → screenshot → adjust loop
4. **Agent-as-player testing harness** — Claude connects as a player via structured protocol, no vision needed
5. **Cross-platform client** — web first, mobile/desktop later
6. **"Fun to build" factor** — this is an exploration project

## Server: Rust (Decided)

| Component | Choice | Rationale |
|---|---|---|
| Async runtime | **Tokio** | Industry standard, entire ecosystem builds on it |
| HTTP + WebSocket | **Axum** | Built on Tokio + Hyper, lightweight, native WebSocket upgrade support |
| Game room model | One `tokio::spawn` task per room | Each room holds its own state struct, players communicate via `mpsc` channels. Clean ownership, no locks |
| Serialization | **serde + MessagePack** (`rmp-serde`) | Fast binary messages on the wire, JSON fallback for debugging |
| State machine | Rust enums for round phases | `Idle → Drafting → Playing → Revealing → Resolving → Scoring` — compiler enforces valid transitions |
| Database | **PostgreSQL** | Player accounts, match history, leaderboards, card collection |
| Observability | TBD stack | Metrics, tracing, logging |

### Game Room Architecture

```
Room Task (tokio::spawn)
├── State: CauldronState, PlayerHands, Threshold, Scores
├── Inbound: mpsc::Receiver<PlayerAction>
├── Outbound: broadcast::Sender<GameEvent>
└── Lifecycle: lives for N rounds, then dies
```

Players send `PlayerAction` messages, room validates and broadcasts `GameEvent` updates. No shared mutable state.

## Client: Options Explored

### Option 1 — "The Proven Path" (Classical Web)

**React + TypeScript + Socket.IO + Redis + PostgreSQL**

- Battle-tested, tons of libraries, easy to hire for
- Socket.IO handles reconnection gracefully
- Weekend-hackathon prototype speed
- **Rejected:** React is a UI framework, not a game renderer. Fighting the DOM for animations, particles, card physics

### Option 2 — "The Weird One" (Edge-First, No Server)

**SolidJS + PartyKit (Cloudflare Durable Objects) + Yjs (CRDT) + Theatre.js**

- No traditional backend — game state is a shared CRDT document
- Commit/reveal pattern with encryption for face-down cards
- Sub-50ms latency on edge, scales for pennies
- **Rejected:** Interesting architecture but SolidJS/React aren't game renderers. Also conflicts with the Rust server decision

### Option 3 — PixiJS / Vanilla TypeScript

**PixiJS + Vite + Vanilla TS + WebSocket to Rust server**

- Proper 2D game renderer (WebGL), 60fps, particles, sprites
- No framework overhead — thin render loop + input handler
- Testable with Playwright (screenshots, DOM inspection, network mocking)
- PWA installable for app-like feel
- **Viable.** Lightest web option, excellent agent testability

### Option 4 — Flutter + Flame

**Dart + Flutter + Flame 2D engine**

- Production-tested cross-platform (iOS, Android, web, desktop)
- Flame has finished game loop, sprite batching, particles, collision
- GC can cause micro-stutters (2-4ms pauses), invisible for a card game
- Web export ships Skia/CanvasKit WASM (~2-3MB minimum payload)
- **Tradeoff:** Dart is a second language — protocol types live in two places, need codegen to stay in sync

### Option 5 — Dioxus (Full Rust)

**Rust + Dioxus + wgpu**

- Same language as server → shared protocol crate, zero codegen, zero drift
- Compiles to WASM (web), desktop, mobile
- Zero GC, predictable frame times
- **Tradeoff:** Young ecosystem, thin community, mobile support still rough. You're solving problems nobody has Stack Overflow answers for

### Option 6 — Macroquad (Full Rust, Game Engine)

**Rust + Macroquad**

- Dead simple 2D game engine inspired by Raylib
- WASM output ~200KB, starts instantly
- Compiles to web, Windows, Mac, Linux, iOS, Android
- Same language as server → shared crate in one cargo workspace
- API learnable in an afternoon, sufficient for a polished card game
- **Strong contender.** Best of the Rust-native options for this game's scope

### Option 7 — Godot + GDExtension

**Godot 4 + GDScript + Rust via gdext**

- Full 2D editor: scene tree, animation player, tween engine, particles, UI, audio
- Polished prototype in a single weekend
- Text-based files (.tscn, .gd) — agent-writable
- Built-in multiplayer networking (WebSocketPeer connects to Rust server natively)
- **Tradeoff:** 80% code / 20% editor GUI. Agent can build architecture and logic, but visual tweaking (particle rates, easing curves) needs the editor. Custom screenshot pipeline needed for agent testing (vs Playwright for web)

## Comparison Matrix

| Criteria | PixiJS/Web | Flutter/Flame | Macroquad | Godot |
|---|---|---|---|---|
| Shared types with Rust server | No (codegen) | No (codegen) | **Yes (same crate)** | Partial (via gdext) |
| Agent can write everything | **Yes** | **Yes** | **Yes** | 80% |
| Agent can see results | **Playwright** | Custom | **Playwright on WASM** | Custom pipeline |
| Agent closed loop | **Excellent** | Medium | **Excellent** | Medium |
| Time to polished prototype | Medium | Medium | Medium | **Fast** |
| 2D game engine built-in | PixiJS (good) | Flame (good) | **Built-in** | **Full editor** |
| Cross-platform | Web + PWA | **All native** | Web + native | **All native** |
| Runtime size | Tiny | 2-3MB WASM | **~200KB WASM** | ~20-30MB |
| Ecosystem maturity | Massive | Large | Small | Large |
| GC pauses | None (JS is GC'd but not Rust) | Yes (Dart GC) | **None** | None (GDScript is ref-counted) |

## Agent Testing Architecture (Stack-Agnostic)

The Rust server enables a three-layer testing harness regardless of client choice:

### Layer 1 — Protocol Bot Harness (Rust crate)

Headless bots connecting via WebSocket. Balance testing, strategy discovery, regression testing. Run 10,000 games with heuristic bots to validate explosion thresholds, dominant strategies, etc. No rendering, no Claude needed.

### Layer 2 — Claude-as-Player (Agent harness)

Thin wrapper piping WebSocket state to Claude API as structured JSON:

```json
{
  "phase": "playing",
  "hand": [
    {"id": 7, "color": "red", "volatility": 2, "effect": null},
    {"id": 12, "color": "wild", "volatility": 1, "effect": "peek"}
  ],
  "cauldron": {"cards_played": 3, "rumble_level": "calm"},
  "scores": {"player_1": 14, "player_2": 9, "agent": 11},
  "round": 4
}
```

Claude reasons about game theory, bluffing, push-your-luck math from structured data. Thousands of games per minute. Gets playtesting AND design feedback simultaneously.

### Layer 3 — Visual Client Testing

- **Web targets (PixiJS/Macroquad WASM):** Playwright — programmatic control, screenshots, DOM inspection, visual regression diffing
- **Godot:** Custom screenshot pipeline from editor test runner
- **Flutter:** Flutter integration tests + screenshot comparison

## Recommended Project Structure

```
cargo workspace
├── server/        # authoritative game logic (axum + tokio)
├── client/        # game client (compiles to WASM for web)
├── shared/        # protocol types, game enums, serde derives
├── bot-harness/   # headless bot players for balance testing
└── agent-harness/ # Claude-as-player wrapper
```

## Decision Framework

The real fork:

| Priority | Best Choice |
|---|---|
| Claude as autonomous co-developer (closed loop) | **Macroquad** or **PixiJS** — code → render → Playwright screenshot → adjust |
| Ship a polished game fast, human does visual polish | **Godot** — editor is unbeatable for "game feel" iteration |
| Full Rust stack, shared types, one language | **Macroquad** — one cargo workspace, compile-time protocol guarantees |
| Cross-platform native distribution | **Flutter/Flame** or **Godot** — proven mobile exports |
| Lowest friction for players to join | **Web (PixiJS or Macroquad WASM)** — tap a link, you're in |

## Current Shortlist

**Server:** settled — **Rust (Axum + Tokio) + PostgreSQL + observability stack**.

**Client:** decided — **PixiJS (web + mobile hybrid via Capacitor)** (change
`adopt-pixi-client`, constitution v1.1.0). After prototyping all three short-listed
options plus a DOM/CSS sketch in [`docs/ui-explorations/`](../ui-explorations/), PixiJS won
on web-first reach + animation ceiling + agent-writability + a one-codebase hybrid mobile
path, with its two weaknesses mitigated: shared-type drift via codegen from the Rust
`protocol` crate, and canvas text/a11y via a DOM overlay.

| | **PixiJS + Capacitor** | Flutter/Flame | Macroquad | Godot |
|---|---|---|---|---|
| **Core bet** | Web-first, agent-writable, GPU spectacle, one codebase → web + mobile | Polished native feel, mature exports | Full Rust stack, shared types | Fastest to polished game feel |
| **Agent testability** | Excellent (Playwright: canvas screenshots + DOM assertions) | Medium (Flutter integration tests) | Excellent (Playwright on WASM) | Medium (custom pipeline) |
| **Outcome** | **Chosen** | Deferred — revisit for a premium native app | Rejected — immature text/a11y/mobile | Rejected — editor-driven vs agent closed loop |
