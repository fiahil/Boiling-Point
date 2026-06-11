## Context

At decision time the Rust TUI was the only shipping client and the agent-test reference
(since retired to `archive/tui-client/` — `retire-v1-harnesses`). The constitution
lists the *graphical* client as undecided (Macroquad / Godot / Flutter). Direction and
feasibility were explored as three throwaway sketches in `docs/ui-explorations/`; PixiJS
won on web reach + animation ceiling + agent-writability + a cheap hybrid mobile path
(see `research.md`). This design fixes **how** the Pixi client is structured so it stays a
thin, server-authoritative renderer that also reaches mobile and stays testable.

## Goals / Non-Goals

**Goals:**
- Establish PixiJS v8 / TypeScript as the graphical client, web-first, with a hybrid
  (Capacitor) path to iOS/Android from one source.
- Keep the client a pure renderer of the existing MessagePack/WebSocket protocol (§I).
- Define the **DOM-overlay seam** for selectable/accessible text over the canvas.
- Define **protocol typegen** so the TS client cannot drift from the Rust server.
- Complete the Layer-3 visual test story (Playwright + deterministic clock) for §II.

**Non-Goals:**
- Implementing every screen/animation now (the verified sketch is the direction, not the
  product). Scope here is the **skeleton + seams + first vertical slice**, per §III.
- A Flutter/native client (deferred).
- Any server, game-logic, balance, or wire-format change.
- Standing up mobile store CI/signing (a likely follow-up change once the web slice lands).

## Decisions

### D1: Layered hybrid topology

```
┌───────────────────────────────────────────────┐
│  DOM overlay  (HTML/CSS)                        │  selectable/a11y text:
│  room code · chat · names · scores · inputs     │  copy / find / screen-reader / scaling
├───────────────────────────────────────────────┤
│  PixiJS canvas  (WebGL/WebGPU)                  │  board · cards · cauldron ·
│  scenes, particles, bloom, depile, boom         │  particles · spectacle
├───────────────────────────────────────────────┤
│  Protocol client  (TS)                          │  WebSocket + @msgpack/msgpack,
│  generated wire types  ◄── Rust `protocol`      │  handshake, intent send, state in
└───────────────────────────────────────────────┘
```

The protocol client decodes server state and feeds a view-model; Pixi draws spectacle and
card faces; the DOM overlay (positioned over the canvas) owns text. The same bundle is
served on the web and wrapped by Capacitor on mobile. *Alternative rejected:* all-canvas
(no DOM) — loses text selection/a11y/scaling (R3).

### D2: Protocol types are generated, not written

The Rust `protocol` crate stays canonical; a typegen step (e.g. `typeshare`/`ts-rs`) emits
TypeScript consumed by the protocol client, checked in and CI-verified for staleness.
Transport stays MessagePack over WebSocket — typegen describes the *same* messages, it
does not introduce a new format. *Alternative rejected:* hand-maintained TS types (drift);
*deferred:* Protobuf/FlatBuffers schema-first (displaces the MessagePack decision).

### D3: Mobile = the same web bundle in a Capacitor shell

PWA for the open web; Capacitor (system WebView + plugin bridge) for iOS/Android store
builds. Pixi's GPU canvas keeps the spectacle cheap inside a WebView; the DOM sketch's
per-node particle approach (jank on phones) is explicitly not used for spectacle.
*Alternative rejected:* a second native (Flutter) client now — doubles the presentation
codebase and the protocol-drift surface (§III).

### D4: Idle rendering for battery/perf

Because the game is turn-based and mostly static, the render loop idles (stops requesting
frames) when nothing is animating and resumes on state change/animation. Full-screen
filter passes (bloom, shockwave, heat-haze) are used only during the moments that earn
them (depile/boom), and their intensity on real devices is a **needs-playtesting** dial
(§IV).

### D5: Testing — the visual layer for this client

*(Re-scoped by `retire-v1-harnesses` / constitution v2.0.0: the v1 harnesses that
validated game correctness client-agnostically are archived; server tests carry that
load now.)*

- The visual client test layer (§II) is added here with **Playwright**: Pixi canvas
  screenshots under a pinned animation clock + DOM-overlay text assertions (selectable
  code, etc.).
- Recorded protocol message-sequence **fixtures** are replayed into the client to drive
  deterministic scene snapshots.

## Constitution Check

- **§I Server-Authoritative** — ✅ the client validates nothing and computes no outcomes;
  it renders server state and emits intents only (spec'd in `web-client-shell`: *Pure
  Renderer Over The Protocol*, *Client never self-advances*).
- **§II Agent-Driven Development** — ✅ pure TypeScript/HTML source, no GUI-only state;
  headless screenshot loop proven on the sketch; this change supplies §II's visual test
  layer (Playwright with a deterministic clock) alongside the server-test layer
  (constitution v2.0.0; the v1 harness layers are archived per `retire-v1-harnesses`).
- **§III Start Simple, Scale Later** — ✅ chooses the **simplest path that reaches web +
  mobile + spectacle**: one TS codebase + a hybrid wrapper. *Rejected simpler alternative:*
  a **pure-DOM client** (no canvas) — simpler, but its animation ceiling is too low for the
  depile/boom and it janks on mobile, so it is demoted to the text overlay rather than the
  whole client. *Also deferred (not built now):* a second native client. Scope is limited
  to skeleton + seams + one vertical slice, not the whole UI.
- **§IV Playtest-Driven Balance** — ✅ no balance numbers touched; animation intensity and
  per-device perf are marked **needs playtesting** and tuned from device data, not guessed.

*Governance note:* this resolves the constitution's "Client (undecided)" row → Pixi hybrid
and retires the "`client/` compiles to WASM" structure note. Recording that in `CLAUDE.md`
is a constitution amendment (MINOR — a decision made / material expansion) and is tracked
as a task.

## Risks / Trade-offs

- **Not shared Rust types (Dart/TS gap)** → Mitigated by D2 (generate TS from the Rust
  `protocol` crate; CI fails on drift).
- **Canvas text loses selection/a11y/scaling** → Mitigated by D1/R3 (DOM overlay owns all
  such text; rule: copyable/findable/translatable/announced ⇒ DOM).
- **WebView perf/feel on low-end phones** (input latency, filter cost) → Mitigated by D4
  (idle loop, filters only during spectacle) + a real-device playtest dial (§IV). Residual:
  a hybrid will never feel as native as Flutter — accepted; native is deferred, not denied.
- **iOS WebGPU still maturing** → Pixi v8 auto-falls back to WebGL2 (iOS 15+); no action.
- ~~**A second client increases drift/maintenance** (TUI + web)~~ — moot since
  `retire-v1-harnesses`: the TUI is archived and `clients/web/` is the only live
  client; type parity with the server is enforced (D2).
- **iOS PWA limits** (push/storage/eviction) → Capacitor build is the first-class iOS path;
  PWA is an extra, not the only delivery.

## Migration Plan

Additive — no rollback of existing behavior. Sequence:
stand up `clients/web/` (typegen → protocol client → one vertical slice: connect + table
scene) behind nothing (it ships when ready); add Playwright; then broaden scenes; the
Capacitor mobile lane and store CI are a follow-up change. The constitution edit lands with
this change.

## Open Questions

- Exact typegen tool (`typeshare` vs `ts-rs` vs a small custom emitter) — pick during 2.x;
  decision criterion is full coverage of the protocol's enums/IDs/generics.
- ~~Repo placement of the TS workspace~~ — resolved by `retire-v1-harnesses`: it lives
  at `clients/web/`.
- How much of the verified sketch (`boiling-point-pixi.html`) is lifted directly vs
  rebuilt as structured modules.
