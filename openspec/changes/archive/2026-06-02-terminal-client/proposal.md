## Why

`server-release-1` delivers an authoritative game server plus a headless bot
harness, but **nothing a human can play** — and the graphical client
(Macroquad / Godot / Flutter) is still an open decision (`CLAUDE.md` tech
stack). A **terminal client** is the simplest viable *feature-complete* client:
a human can play a whole game in a terminal, it shares the `protocol/` crate
with the server (zero codegen, compile-time wire safety), and it is trivial to
run and debug. It validates the wire protocol and the full game loop
**end-to-end with a real player** before the project commits to a heavy GUI
stack — and it doubles as the team's primary dev/debug tool and the rendering
seam the future agent harness (Layer 2) will reuse.

Boiling Point is fundamentally an **information game** (blind volatility, hidden
commits, public contribution counts, a dramatic reveal). A terminal — where the
information *is* the interface — is a faithful first home for it, not a
compromise.

## What Changes

- Add a **`tui-client/` crate** to the cargo workspace: a `ratatui` + `crossterm`
  terminal client that connects to the server over WebSocket/MessagePack.
- The client **depends only on `protocol/`** and, exactly like `bot-harness`,
  **owns its own narrow player-visible view model** built solely from received
  messages — it has no field for the boiling point, no other players' hands, no
  cauldron card identities. The secret boundary (Constitution I) holds *by
  construction*: the client cannot represent or render a secret it is never sent.
- Render **every phase** of a complete game: lobby (quick-match queue **and**
  invite codes) → round-start (cumulative modifier reveal + refill-to-5) →
  simultaneous **hidden** single-card waves (selection changeable until close) →
  reverse-order **depile** (boiling point shown on explosion only, crossing card
  marked) → **boom** → scoring → next round → **Deathmatch** → game over.
- Faithfully render the design's **information architecture**: opaque cauldron
  with **no volatility cue**; pot shown as face-down chips tagged by the
  *contributing player's* color (never the card's color); per-player
  contribution counts and hand sizes public; commits hidden until reveal; a
  Recall surfaced only as a contribution-count drop; Peek as a private result +
  anonymous "someone peeked" toast; Expose as a public reveal.
- **One interactive effect prompt** (Recall — pick your own pot card, at commit
  time, privately); all other effects are passive (silent until depile, or a
  result display).
- **Preset emotes** as the only comms channel (no free text), rendered against
  `server-release-1`'s `table-talk` relay (palette id in, `EmoteBroadcast` out).
- **Reconnection UX**: a grace-countdown overlay, auto-pass-while-away messaging,
  and seamless resume from the server's `StateSnapshot`.
- **Dev/debug affordances**: a toggleable debug overlay (RTT, message log, live
  view-model JSON), **deterministic event replay** (record the message stream,
  replay it with no live server), and **`TestBackend` snapshot tests** — making
  the Layer-3 "visual" test a plain-text buffer Claude reads natively (no
  browser, no screenshots, no OCR).

This change adds **no game logic** and modifies **no server behavior**. The
client is an untrusted renderer of server state and a sender of player intents.

## Capabilities

### New Capabilities
- `tui-client-shell`: connection + `JoinRoom`/`RoomJoined` version handshake,
  the render/input/event loop, phase-driven screen routing, the player-visible
  view model (no secrets), responsive layout / minimum-size handling,
  reconnection overlay, and clean terminal teardown on quit or panic.
- `tui-lobby`: the entry menu (quick-match / create-room / join-by-code) with a
  display name, invite-code display + copy, join-by-code error feedback, the
  4-seat roster with hostless auto-start feedback, and the queue waiting state.
- `tui-round-play`: the in-round table — round-start modifier reveal +
  refill-to-5, the opaque cauldron and contribution chips, opponent status, the
  hidden changeable commit against the wave timer, pass-as-lockout, the
  one-player final-wave indicator, wave-resolution reveal, the effect
  interactions (Recall pick, Peek result, Expose reveal, "someone peeked"), and
  preset emotes.
- `tui-reveal-and-score`: the reverse-order depile with a descending volatility
  bar, boiling-point-on-explosion-only with the crossing card marked, the boom
  sequence, the round scoring screen, the Deathmatch screens (forced 1/wave, no
  pass, volatility-only, Detonator elimination + Shield-redirect cascade +
  co-champions), and the game-over standings.
- `tui-debug-and-test`: the debug overlay, deterministic record/replay of the
  message stream, and `TestBackend` snapshot tests as the Layer-3 visual-test
  mechanism.

### Modified Capabilities
- _None._ This change does not alter any `server-release-1` requirement. The two
  server-owned pieces the client depends on are now specced there: the
  **wave-open timer budget** (added to the `wire-protocol` capability) and the
  **preset-emote relay** (the `table-talk` capability). The client renders
  against both — nothing is left gated.

## Impact

- **New code:** `tui-client/` crate — `net/` (ws connect, MessagePack decode →
  events, encode intents), `view/` (player-visible view model + `apply(msg)`),
  `ui/` (ratatui screens per phase), `input/` (keys → intents), `anim/`
  (time-driven depile/boom/timer animations), `replay/` (record + playback),
  `debug/` (overlay). Builds on the existing `protocol/` crate.
- **New dependencies:** `ratatui`, `crossterm`, `tokio` (client-side), `serde`
  / `rmp-serde` (via `protocol/`), and `arboard` (or equivalent) for clipboard
  copy of invite codes.
- **Depends on `server-release-1`:** the `protocol/` crate and a running server.
  For development and tests the client runs against **recorded replays and a
  mock/in-process server**, so it does not block on a live deployment.
- **Server-owned dependencies (now specced in `server-release-1`):** the
  wave-open broadcast carries the wave's timer budget (`wire-protocol`) so the
  client can render the countdown (the bot harness uses it too), and the
  preset-emote relay (`table-talk`) provides palette-id-in / `EmoteBroadcast`-out.
  The client renders against both; nothing remains gated.
- **Resolves a `server-release-1` open question:** "should `shared/` carry
  content *types* for a client to render cards?" — **No.** The client renders the
  public card DTOs that already cross the wire (`YourHand`, depile reveals); it
  needs no content registry and no shared content crate. The `protocol/` waist
  stays minimal.
- **Constitution:** advances Principle II (a human-playable client and a
  text-snapshot Layer-3 test that closes the agent loop) and Principle III (the
  simplest viable client first; the heavyweight GUI decision is deferred behind a
  proven protocol). Principle I is preserved — the client owns no game logic and
  cannot hold secrets.
