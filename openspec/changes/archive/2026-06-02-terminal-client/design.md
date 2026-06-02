## Context

`server-release-1` stands up the authoritative server, the `protocol/` wire
crate, and a headless bot harness — but no human can play, and the graphical
client (Macroquad / Godot / Flutter) is an open decision. This change adds the
**first human-playable client**, a terminal UI, which is also the team's primary
dev/debug tool. Boiling Point is an information game (blind volatility, hidden
commits, public counts, a dramatic reveal), so a terminal — where information
*is* the interface — is a faithful first home, and a deliberately simple one
(Constitution III): it proves the protocol and the full game loop end-to-end with
a real player before the project pays for a heavyweight GUI stack.

The client is constrained by Constitution I: it owns **no game logic**, is an
untrusted renderer of server state and a sender of player intents, and must be
incapable of holding secrets. It is constrained by Constitution II: agent
testability is first-class, which a TUI serves unusually well because its
"screenshot" is a plain-text buffer.

## Goals / Non-Goals

**Goals:**
- A human plays a complete 4-player game in a terminal: lobby/queue → 5 rounds →
  Deathmatch → game over.
- Faithful rendering of the design's information architecture — no volatility
  cue, contributor-tagged chips, hidden commits, reverse depile, boiling point on
  explosion only.
- The client cannot represent or render a secret it was never sent (secret
  boundary by construction, mirroring `bot-harness`).
- Trivial dev/debug: a debug overlay, deterministic replay, and `TestBackend`
  text-snapshot tests that Claude reads natively.
- Plain-text, fully agent-writable Rust; one cargo workspace, one language.

**Non-Goals:**
- No game logic, validation, or scoring in the client (server-authoritative).
- No extraction of a shared `client-core` crate yet — the agent harness (L2)
  that would consume it does not exist; design the seam only.
- No graphical client; this does not pick or preclude the eventual GUI stack.
- No spectator mode, no replays *of other players*, no rating UI.
- Emote **sending** and the wave **countdown** rely on server-side support that
  is now specced in `server-release-1` (`table-talk` and the `wire-protocol`
  timer budget — see Decisions D6); the client is a renderer over both.

## Decisions

### D1. `ratatui` + `crossterm`, immediate-mode loop

`ratatui` (immediate-mode, `TestBackend`) on `crossterm` (cross-platform,
truecolor). A `tokio` task reads the WebSocket, decodes via `protocol/`, and
forwards `ServerMessage`s on a channel. The main loop `select!`s over terminal
input events, incoming messages, and a ~20–30 fps render tick; each tick advances
animations and redraws. Rationale: immediate-mode redraw matches a state-driven
game; one workspace, compile-time wire safety. (Research R1.)

### D2. Client owns its player-visible view model; renders wire DTOs

`tui-client` builds its own narrow view model from received messages, exactly as
`bot-harness` does (`server-release-1` D-A1) — it has no field for the boiling
point, no opponents' hands, no draw deck. It depends only on `protocol/` and
renders the public card DTOs that already cross the wire (`YourHand`, depile
reveals), so it needs **no content registry and no shared content crate**. This
resolves the `server-release-1` open question about a shared content-types crate:
not needed. The net + view-model layer is structured for a clean later extraction
into `client-core` when the agent harness lands. (Research R2.)

### D3. Animations are time-driven, skippable; timing is server-authoritative

Depile, boom, and the wave countdown are state machines advanced by elapsed
time, and every animation is skippable. The wave deadline is the **server's** to
enforce; the client renders a countdown from a server-provided wave-open
duration and never rejects a commit locally. An unfinished Recall pick at close
is simply never sent (auto-pass). (Research R3, R7, R8.)

### D4. Color **and** letter, capability-detected palette, ASCII-first

Every player is color + letter (`R`/`B`/`G`/`A`, Wild `W`); color is never the
sole signal. Truecolor when the terminal supports it, ANSI-16 fallback
otherwise. Core state uses box-drawing + ASCII (stable widths); emoji is reserved
for the optional emote palette. (Research R4.)

### D5. Testability: snapshots + replay + a mock server

Layer-3 visual tests are `TestBackend` text-buffer snapshots per screen/phase —
agent-readable, no browser. Deterministic replay records the message stream and
plays it back with no socket, turning any bug into a fixture. A thin
mock/in-process server drives interactive manual testing. The client therefore
does not block on a live deployment. (Research R5.)

### D6. Emote and timer dependencies are server-owned — and now specced

The canonical design (§10) makes preset emotes the only comms channel, and the
wave countdown needs a timer/deadline on the wave-open broadcast. Both are
server-owned, and both were folded into `server-release-1` during this proposal:
the **`table-talk`** capability provides the preset-emote relay (palette id in,
`EmoteBroadcast` out, non-binding, rate-limited), and **`wire-protocol`** now
requires the wave-open message to carry the timer budget. The client is a pure
renderer/sender over both — nothing is gated. (Research R6.)

## Risks / Trade-offs

- **Animation jank over SSH / slow terminals** → keep animations short and always
  skippable; never block input on an animation.
- **Terminal color/emoji variance** → capability-detect; ANSI-16 fallback;
  ASCII-first for core state; emoji confined to emotes.
- **Client clock drift vs. the server deadline** → server is authoritative; the
  client only renders the countdown and never enforces it.
- **Emote/timer depend on the server** → both are specced in `server-release-1`
  (`table-talk`, `wire-protocol`); build the client against those contracts and
  cover them in replay fixtures.
- **"Feature-complete" is broad** → tasks are phased so a playable core (shell →
  lobby → one round → scoring) lands before depile polish, Deathmatch, emotes,
  and debug tooling.
- **Hidden-info leak through a careless render** → the view model has no secret
  fields to leak; `TestBackend` snapshots assert no boiling point / opponent hand
  appears.

## Migration Plan

Additive — a new `tui-client/` crate in the existing workspace; no migration, no
persisted state to reconcile. Depends on the `protocol/` crate from
`server-release-1`; can be developed and tested against recorded replays and a
mock server ahead of a live server. Rollback is removal of the crate.

## Open Questions

- Exact field shapes for the wave-open timer budget and the `EmoteBroadcast` —
  pinned to whatever `server-release-1` finalizes in `wire-protocol` /
  `table-talk` (D6 / R3 / R6).
- Whether to extract `client-core` proactively or wait for the agent harness —
  current call is to wait (R2); revisit when L2 is scheduled.

## Constitution Check

- **I. Server-Authoritative** — Met. The client holds no game logic, validates
  nothing, and computes no scores; it cannot represent the boiling point or
  opponents' hidden state; timing authority stays on the server.
- **II. Agent-Driven Development** — Advanced. Adds a human-playable client and a
  Layer-3 visual test that is a plain-text snapshot (agent-readable, closing the
  render loop better than browser screenshots), plus deterministic replay. All
  source is plain-text Rust.
- **III. Start Simple, Scale Later** — Met. A terminal client is the simplest
  viable feature-complete client; the heavyweight GUI decision is deferred behind
  a proven protocol; `client-core` extraction is a designed seam, not built now.
- **IV. Playtest-Driven Balance** — Supported, not owned. The client surfaces
  balance signals (debug overlay, the depile's revealed numbers) but owns no
  balance values; those live in the server's validated config.
