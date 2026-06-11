## Why

The constitution leaves the **client undecided** (candidates: Macroquad, Godot,
Flutter/Flame) while the only shipping client is the Rust TUI. We need a graphical
client that (a) reaches the **web** instantly, (b) reaches **mobile** cheaply, (c)
sustains the game's signature spectacle — the depile reveal and the boom — and (d)
stays inside the agent-driven dev loop (Constitution §II: pure source, screenshot loop,
test layers). Three throwaway sketches were built and screenshot-verified in
[docs/ui-explorations/](../../../docs/ui-explorations/): a **DOM/CSS** sketch
(`boiling-point-ui.html`), a **PixiJS/WebGL** sketch (`boiling-point-pixi.html`), and a
**Flutter+Flame** native prototype (`flame-prototype/`). PixiJS hit the highest
animation ceiling with the fastest iteration loop while remaining web-first and
agent-writable. This change **materializes PixiJS as the graphical client for web and
mobile**, packaged as a hybrid app, and records the decision and its seams.

## What Changes

- **Adopt PixiJS (v8, TypeScript) as the graphical client** for the web, rendering on a
  WebGL/WebGPU canvas. It stays a **pure renderer** of server state over the existing
  MessagePack/WebSocket protocol (§I) — no game logic moves client-side.
- **Ship mobile as a hybrid** via a Capacitor wrapper: the *same* TS/Pixi source
  produces an installable PWA and iOS/Android store builds — not a second native
  codebase. (The ask was explicit: "native or not — hybrid is fine.")
- **Hybrid render split.** The Pixi canvas owns the board, cards, cauldron, particles,
  and boom; a thin **DOM overlay** owns long-form, selectable, accessible text (room
  code, chat, player names, scores), closing canvas text's selection / screen-reader /
  font-scaling gap.
- **Generate the wire types for TypeScript from the canonical Rust `protocol` crate**, so
  the web client cannot drift from the server contract (no hand-maintained duplicates).
- **Add the visual test harness** for this client (Playwright: Pixi canvas
  screenshots + DOM-overlay assertions) behind a **deterministic animation clock** —
  the §II visual layer (constitution v2.0.0).
- **Flutter+Flame native is deferred, not rejected** — revisit only if a "premium native
  feel" later justifies a second presentation codebase. *(Superseded note: this change
  originally kept the Rust TUI as the agent-test reference client; the TUI has since
  been retired to `archive/tui-client/` — `retire-v1-harnesses`.)*
- **BREAKING (governance):** resolves the constitution's "Client (undecided)" row →
  PixiJS (web + mobile hybrid). Requires a constitution amendment recorded per its
  procedure, and supersedes the project-structure note that `client/` "compiles to WASM"
  (the client is now TypeScript/Pixi, not a Rust→WASM build).

## Capabilities

### New Capabilities

- `web-client-shell` — the contract for the Pixi/TS hybrid client: protocol handshake,
  phase-driven rendering as a pure renderer, the blind cauldron, readability priority,
  the DOM text-overlay seam, deterministic animation, idle rendering, and web + mobile
  packaging.
- `protocol-typegen` — a build step that generates TypeScript wire types from the Rust
  `protocol` crate as the single source of truth, enforced in CI.

### Modified Capabilities

<!-- none — this is additive. (The TUI client specs it originally sat alongside were
     removed by retire-v1-harnesses; the new client renders the same protocol.) -->

## Impact

- **New code:** a `clients/web/` TypeScript workspace (Pixi app, protocol client over
  WebSocket + MessagePack, scene renderers, DOM overlay), a Capacitor project for mobile,
  a typegen step, and a Playwright suite. The animation direction already exists as the
  verified sketch in `docs/ui-explorations/boiling-point-pixi.html`.
- **Build/CI:** adds a Node/TS toolchain + Playwright; wires typegen to the `protocol`
  crate; mobile (Capacitor) build lanes likely follow in a later change.
- **Governance:** `CLAUDE.md` constitution updated — Client decided = Pixi hybrid;
  WASM-client assumption retired.
- **No** server, game-logic, balance, or wire-format change. MessagePack/WebSocket and
  the `protocol` crate stay canonical. *(The client-agnostic bot and Claude-as-player
  harnesses this originally left untouched are now archived — `retire-v1-harnesses`.)*
