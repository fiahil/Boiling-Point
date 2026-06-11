## 1. Governance & decision record

- [ ] 1.1 Update `CLAUDE.md` constitution: resolve **Client (undecided) → PixiJS (web +
  mobile hybrid)**, record the rationale, and retire the "`client/` compiles to WASM"
  project-structure note (amendment procedure, MINOR bump).
- [ ] 1.2 Note in the constitution that Flutter+Flame native is **deferred, not rejected**,
  with the trigger to revisit (premium native feel).

## 2. Protocol typegen (single source of truth)

- [ ] 2.1 Choose the typegen tool (`typeshare` / `ts-rs` / small custom emitter) by
  verifying full coverage of the `protocol` crate's messages, enums, and IDs.
- [ ] 2.2 Annotate/configure the Rust `protocol` crate so client-facing types emit cleanly.
- [ ] 2.3 Add a `generate-types` step emitting checked-in TypeScript into `clients/web/`.
- [ ] 2.4 Add a CI check that fails when committed generated types are stale vs the crate.

## 3. Web-client workspace skeleton

- [ ] 3.1 Create the `clients/web/` TypeScript workspace (build + dev server, Pixi v8 dep,
  `@msgpack/msgpack`), all source agent-writable (§II).
- [ ] 3.2 Add the index HTML shell with the DOM-overlay container layered over the Pixi
  canvas mount.
- [ ] 3.3 Port the shared visual primitives from `docs/ui-explorations/boiling-point-pixi.html`
  into structured modules (palette, card factory, particle system, glow/filters).

## 4. Protocol client (pure renderer)

- [ ] 4.1 Implement the WebSocket + MessagePack transport using the generated types.
- [ ] 4.2 Implement the `JoinRoom` handshake (send `protocol_version`, proceed on
  `RoomJoined`, surface version-mismatch `Error`).
- [ ] 4.3 Decode server state into a view-model; send only player intents; never compute
  outcomes or self-advance phase (§I).

## 5. First vertical slice (the table) + scene routing

- [ ] 5.1 Phase-driven scene router: render exactly one scene for the server phase, switch
  on server advance.
- [ ] 5.2 Implement the **table** scene from real server state (seats, blind cauldron, hand,
  intents) — the cauldron animation statistically independent of hidden state.
- [ ] 5.3 Implement readable card faces (volatility › color-by-sigil › points › effect-name).
- [ ] 5.4 Wire the **depile** and **boom** scenes to server reveal/scoring messages.

## 6. DOM text-overlay seam

- [ ] 6.1 Render room/invite code, player names, scores, and chat as DOM elements over the
  canvas (selectable, screen-reader exposed).
- [ ] 6.2 Establish the convention/lint: copyable/findable/translatable/announced text ⇒ DOM,
  never canvas-only.

## 7. Animation clock & idle rendering

- [ ] 7.1 Route all time-based animation through an injectable clock (pinnable for tests).
- [ ] 7.2 Idle the render loop when nothing is animating; resume on state change/animation.

## 8. Packaging

- [ ] 8.1 Produce a static web build + installable PWA manifest/service worker.
- [ ] 8.2 Add the Capacitor project wrapping the same bundle for iOS/Android (store CI &
  signing may be a follow-up change).

## 9. Layer-3 visual tests (Playwright)

- [ ] 9.1 Add Playwright; capture Pixi canvas screenshots under the pinned animation clock.
- [ ] 9.2 Assert DOM-overlay text (e.g., the room code is selectable/announced).
- [ ] 9.3 Add recorded protocol message-sequence fixtures; replay them to drive deterministic
  scene snapshots. (TUI-parity cross-checks dropped — the TUI retired to `archive/`
  in `retire-v1-harnesses`.)

## 10. Validate

- [ ] 10.1 `openspec validate adopt-pixi-client --strict` passes; specs match implementation.
