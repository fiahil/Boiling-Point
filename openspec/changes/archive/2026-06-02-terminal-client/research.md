## Open Questions

- R1: Which TUI framework?
- R2: Where does the client view model live — in `tui-client`, or a new shared
  `client-core` crate?
- R3: How is the wave timer rendered, and who has authority over the deadline?
- R4: How are the four player colors made legible across terminals?
- R5: How do we develop and test the client without a live server?
- R6: Which behaviours need new server/protocol support the client cannot
  provide alone?
- R7: Is the commit final, or changeable during a wave?
- R8: What is the depile's volatility bar, given the boiling point is hidden on a
  safe brew?

## R1: Which TUI framework?

**Decision:** `ratatui` + `crossterm`.

**Rationale:** `ratatui` is the de-facto Rust TUI library, immediate-mode
(redraw the whole screen each frame — a natural fit for a state-driven game),
actively maintained, and ships a `TestBackend` that renders to an in-memory
text buffer (see R5). `crossterm` is its cross-platform backend (macOS / Linux /
Windows terminals) with truecolor support for the player palette. Living in the
same cargo workspace as `protocol/` means zero codegen and compile-time wire
safety (Constitution II).

**Alternatives Considered:**
- `cursive` — higher-level, callback/retained-widget model; awkward for
  frame-driven game rendering (depile, boom, ticking timer).
- raw `crossterm` only — maximum control, but we would reimplement layout and
  diffing that `ratatui` already provides.

**Key Details:** immediate-mode redraw at ~20–30 fps; layout via `ratatui`
constraints for responsiveness (R4); `TestBackend` for snapshot tests (R5).

## R2: Where does the client view model live?

**Decision:** `tui-client` **owns its own** player-visible view model now,
mirroring how `bot-harness` owns its own narrow domain (`server-release-1`
D-A1). Do **not** introduce a shared `client-core` crate yet. Design the seam so
the view model + net layer can be extracted into `client-core` when the agent
harness (Layer 2) lands.

**Rationale:** Constitution III (start simple; design the seam). The only future
consumer of a shared `client-core` — the agent harness — does not exist yet;
building it now is speculative. The server team already accepted *deliberate
duplication* of domain models for an airtight secret boundary; the client side
has no secret-boundary concern (it only ever receives player-visible data), so a
later extraction is a clean refactor, not a rewrite.

**Alternatives Considered:**
- Build `client-core` now and have `tui-client` depend on it — premature; no
  second consumer exists.
- Share the server's domain types — rejected by `server-release-1` (per-side
  domain models); the client renders wire DTOs instead.

**Key Details:** Resolves the `server-release-1` open question about a shared
content-types crate: **not needed.** The client renders the public card DTOs
that already cross the wire (`YourHand`, depile reveal payloads); it consults no
content registry. The `protocol/` waist stays minimal and secret-free.

## R3: Wave-timer rendering and deadline authority

**Decision:** The **server is authoritative** over the wave deadline; the client
only renders a countdown and never enforces it. The client renders remaining
time from a server-provided wave-open timestamp/duration. Locking a commit in is
always safe; failing to commit before the server closes the wave is an auto-pass
(lockout), identical to a timeout or disconnect (`round-engine`).

**Rationale:** Constitution I — clients are untrusted; only the server may
decide whether a commit landed in time. A laggy or slow terminal must never let
client-side clock drift change the outcome.

**Alternatives Considered:** client-side countdown that locally rejects late
commits — rejected; it duplicates authority and can disagree with the server.

**Key Details (cross-change ask):** the wave-open broadcast must carry the
timer's duration/deadline. `server-release-1`'s `round-engine` defines the
countdown but the `wire-protocol` spec does not yet enumerate this field; it
should be added there (the bot harness needs it to time commits too). Effect
target-picks (Recall) must be completed *before* close — an unfinished pick is
never sent, so the player auto-passes. This is harsh-but-simple and consistent
with the hidden-commit model; flag for playtest.

## R4: Player-color legibility across terminals

**Decision:** Every player is identified by **color *and* a letter** (Ruby=`R`,
Sapphire=`B`, Emerald=`G`, Amethyst=`A`; Wild=`W`). Color is never the sole
signal.

**Rationale:** survives 16-color terminals and color-blindness; keeps the table
readable when piped to a snapshot test (text only). Aligns with the art-direction
priority *Volatility > Color > Points > Effect* — legibility first.

**Key Details:** detect terminal color capability via `crossterm`; truecolor
palette (Ruby/Sapphire/Emerald/Amethyst) when available, graceful fallback to
the nearest ANSI-16 otherwise. Prefer box-drawing + ASCII over emoji for core
state (emoji width is inconsistent across terminals); reserve emoji for the
optional emote palette.

## R5: Developing and testing without a live server

**Decision:** Two server-free paths. (1) **`TestBackend` snapshot tests** assert
the rendered text buffer for each screen/phase against fixtures (Layer-3 visual
regression, as plain text). (2) **Deterministic replay** records the received
message stream to a file and plays it back into the view model + renderer with
no socket. A thin **mock/in-process server** drives interactive manual testing.

**Rationale:** Constitution II makes agent testability a first-class criterion. A
text-buffer snapshot is something Claude reads directly — strictly better for the
agent loop than Playwright-on-WASM screenshots, which the tech-stack doc assumed
for graphical clients. Replay turns any reported bug into a reproducible fixture.

**Key Details:** fixtures are recorded protocol message sequences (one per
scenario: a clean round, an explosion, a Deathmatch, a reconnection). Snapshots
live alongside the `ui/` module.

## R6: What needs new server/protocol support?

**Decision:** Two items the client cannot provide alone are both server-owned,
and **both are now specced in `server-release-1`** (resolved during this
proposal):
1. **Preset-emote relay** — the canonical design (§10) makes emotes the only
   comms channel. `server-release-1` now carries a dedicated `table-talk`
   capability (palette-id in, `EmoteBroadcast` out, non-binding, rate-limited).
   The client renders the palette and sends ids against it.
2. **Wave timer field** — `server-release-1`'s `wire-protocol` now requires the
   wave-open broadcast to carry the wave's timer budget (see R3). The client
   renders the countdown from it.

**Rationale:** the client must not invent server behaviour (Constitution I).
Naming the dependencies kept the two changes honest; rather than leaving them as
flagged gaps, they were folded into `server-release-1` so nothing is gated.

**Key Details:** the emote relay lives in the `table-talk` capability (not
`wire-protocol`) since it is a room-relay behaviour with no game-state effect;
the timer budget lives in `wire-protocol` as a message-payload requirement.

## R7: Is a commit final or changeable?

**Decision:** **Changeable until the wave closes.** The client lets the player
re-pick a different card or switch to/from pass any time before close; only the
latest selection is sent and applied.

**Rationale:** directly matches `server-release-1` `round-engine` ("A player may
change their commit before close … only their latest selection is applied at
reveal") and D-R1. Corrects this exploration's earlier assumption that a commit
was final.

**Key Details:** "lock in" is an explicit *I'm-done-don't-wait-for-me* signal;
when all active players have locked in, the server closes the wave early. The
client shows tentative-selection vs locked-in distinctly.

## R8: The depile volatility bar with a hidden boiling point

**Decision:** A **descending** volatility bar. It starts at the pot's total
volatility and *decreases* as cards are peeled last-added-first. On an
**explosion** the boiling point is revealed as a marked line; the first peeled
card that drops the running total back below it is the marked **crossing card**
("the crack"). On a **safe brew** the bar still descends (each card's volatility
is shown as it flips, per the depile spec) but **no boiling-point line is drawn**
— faithful to "you only learn it stayed under it."

**Rationale:** one animation path serves both outcomes; reverse order naturally
delivers the "blame escalation, last-to-first" the design wants while showing
exactly who pushed it over. Settles this exploration's earlier
reverse-vs-forward tension in favor of always-reverse (matches `round-engine`).

**Key Details:** the depile reveal payload already carries each card's volatility
and owner; the bar is a pure client render. Recalled cards never appear in the
depile (they left the pot), so the bar math stays consistent.

## Summary

- **R1** `ratatui` + `crossterm`.
- **R2** view model owned by `tui-client` now; extract `client-core` only when
  the agent harness lands. No shared content crate needed.
- **R3** server-authoritative deadline; client renders the countdown only;
  needs a timer field on the wave-open broadcast.
- **R4** color **and** letter for every player; capability-detected palette;
  ASCII-first.
- **R5** `TestBackend` snapshots + deterministic replay + a mock server; no live
  server required to develop or test.
- **R6** emotes (`table-talk`) and the wave-timer field (`wire-protocol`) are
  server-owned; both are now specced in `server-release-1`, so nothing is gated.
- **R7** commits are changeable until wave close; "lock in" closes early when
  unanimous.
- **R8** one descending volatility bar for both outcomes; boiling-point line and
  crossing card on explosion only.
