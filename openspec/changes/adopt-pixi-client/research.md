## Open Questions

- R1: Which rendering technology for the graphical client?
- R2: How do we reach mobile without a second codebase?
- R3: How do we keep text selectable/accessible when the renderer is a canvas?
- R4: How do we stop the web client's wire types from drifting from the Rust server?
- R5: Where does the client live, and what becomes of the "compiles to WASM" assumption?

## R1: Which rendering technology for the graphical client?

**Decision:** **PixiJS v8 (TypeScript), rendering to a WebGL/WebGPU canvas.**

**Rationale:**
- **Web-first reach** — the client is a static web bundle that runs everywhere a browser
  does, with zero install. The TUI covers terminals; Pixi covers the open web in one step.
- **Agent-driven dev (§II)** — pure TypeScript/HTML source, no GUI-only editor state; the
  code→render→screenshot→adjust loop works headlessly (validated: the
  `boiling-point-pixi.html` sketch was built and screenshotted across all four scenes via
  Playwright with zero errors).
- **Animation ceiling** — of the three sketches, Pixi reached the richest result (GPU
  particles, additive bloom, a `ShockwaveFilter` boom, a `DisplacementFilter` heat-haze)
  with the **fastest iteration loop** (instant reload vs Flutter's ~25s compile).
- **Mobile via hybrid** (see R2) — the same source ships to iOS/Android through Capacitor.

**Alternatives Considered:**
- **DOM/CSS (`boiling-point-ui.html`)** — kept selectable/accessible text for free and
  was surprisingly capable, but it is the only sketch whose animation ceiling we *hit*:
  every particle costs a DOM node (jank at scale on phones), the animated SVG
  `feTurbulence` heat-haze is a mobile battery/FPS killer, and 3-D `rotateY` flips
  misrender under headless GPU. Rejected as the *primary* renderer; **retained as the
  text/overlay layer** (R3).
- **Flutter + Flame (`flame-prototype/`)** — best *native* feel, 120 Hz, real semantics
  tree, and the only true-native target. Rejected for now because it is **Dart, not Rust**
  (forfeits shared types, same as Pixi but with a heavier toolchain), its **web target is
  heavy** (CanvasKit download, slow first paint, the worst mobile-web story), and it
  demands a **second presentation codebase**. **Deferred, not killed** — revisit if a
  premium native app becomes a goal.
- **Macroquad** (constitution candidate) — the only option preserving shared Rust types,
  but it is a bare GPU canvas with immature text, accessibility, and mobile packaging —
  strictly weaker than Pixi on every client concern except type sharing, which R4
  neutralizes via codegen.
- **Godot** (constitution candidate) — fastest to "game feel," but the editor-driven,
  scene-file workflow fits the agent-writable / screenshot-loop mandate (§II) far worse
  than plain code, and web/mobile export is heavier. Rejected.

**Key Details:** PixiJS v8; renderer auto-selects WebGPU with WebGL2 fallback (safe in
modern mobile WebViews, iOS 15+). The client never holds game logic (§I).

## R2: How do we reach mobile without a second codebase?

**Decision:** **Wrap the same web bundle with Capacitor** for iOS/Android store builds;
offer an installable **PWA** for web/desktop. No native rewrite.

**Rationale:** Capacitor is a thin native shell around the system WebView with a mature
plugin bridge (push, haptics, IAP, safe-area). Pixi's heavy effects survive a mobile
WebView because they run on the **GPU** (one canvas), unlike the DOM sketch's per-node
particles. One TypeScript source → web + iOS + Android. This is the simplest viable path
that covers web **and** mobile **and** the spectacle (§III).

**Alternatives Considered:**
- **PWA only** — simplest, but iOS treats PWAs as second-class (WebKit-only, limited
  push, storage eviction). Kept as an *additional* delivery, not the only one.
- **Two native clients (Pixi web + Flutter native)** — best per-platform feel but doubles
  the presentation codebase and adds protocol-drift surface; rejected under §III. (The
  cross-client maintenance analysis is recorded in design.md.)

**Key Details:** Performance for a turn-based card game is forgiving — most screens are
static, so the render loop can idle (R-perf in design). Per-device animation intensity is
a **needs-playtesting** item (§IV) — tune on real phones, not in the simulator.

## R3: How do we keep text selectable/accessible when the renderer is a canvas?

**Decision:** **A thin DOM overlay** carries all text a player may need to copy, find,
translate, or have read aloud (room code, chat, player names, scores); the Pixi canvas
carries spectacle and at-a-glance card faces.

**Rationale:** Canvas text is invisible to selection, screen readers, and OS font
scaling. The DOM is excellent at exactly that. Compositing real HTML over the canvas
recovers the web's accessibility for free and is the standard hybrid pattern. The DOM
sketch we already built *is* this overlay layer.

**Alternatives Considered:** Pixi's `accessibility` plugin (injects ARIA divs for
*interactive* objects, not free text) — used as a complement for canvas buttons, not a
substitute for the text overlay.

**Key Details:** Rule of thumb — if a string must be copyable, findable, translatable, or
announced, it is a DOM element; otherwise it may be drawn on the canvas.

## R4: How do we stop the web client's wire types from drifting from the Rust server?

**Decision:** **Generate the TypeScript wire types from the canonical Rust `protocol`
crate**; hand-written duplicates are prohibited and CI fails on stale output.

**Rationale:** The only thing the web client *shares* with the server is the protocol
contract (§I makes the client thin). Generating types converts "silent drift" into "the
TS build won't compile until both sides agree." This is the single lever that makes a
non-Rust client safe.

**Alternatives Considered:**
- Hand-maintained TS types — works for a few dozen messages but rots; rejected.
- Switch the wire format to Protobuf/FlatBuffers (schema → Rust + TS) — would also solve
  it but displaces the constitution's MessagePack/serde decision; deferred unless drift
  pain demands it.

**Key Details:** Candidate tools — `typeshare` or `ts-rs` over the `protocol` crate.
MessagePack transport is unchanged (`rmp-serde` server-side; `@msgpack/msgpack`
client-side). Recorded protocol message-sequence fixtures double as parity tests across
the TUI, the web client, and the harnesses.

## R5: Where does the client live, and what about "compiles to WASM"?

**Decision:** A new **`web-client/`** TypeScript workspace (sibling to `tui-client/`),
outside the cargo workspace. The constitution's project-structure note that `client/`
"compiles to WASM for web" is **retired** — the graphical client is TypeScript/Pixi, not
a Rust→WASM build.

**Rationale:** Pixi is a JS/TS library; the natural layout is a Node/TS package with its
own toolchain. Naming it `web-client/` keeps it parallel to `tui-client/` and leaves room
for a future `native-client/` if Flutter is ever un-deferred.

**Alternatives Considered:** Reusing the documented `client/` slot — avoided to prevent
implying the retired WASM build; the rename is noted as a governance impact.

## Summary

- **R1 → PixiJS v8 / TypeScript** on a WebGL/WebGPU canvas (over DOM, Flutter, Macroquad,
  Godot).
- **R2 → Capacitor hybrid** for iOS/Android + PWA, one codebase.
- **R3 → DOM text overlay** for selectable/accessible text; canvas for spectacle.
- **R4 → generate TS types from the Rust `protocol` crate**, enforced in CI.
- **R5 → new `web-client/` TS workspace**; the "client compiles to WASM" assumption is
  retired.
- The **TUI stays** as the reference client; **Flutter native is deferred**.
