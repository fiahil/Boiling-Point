## Why

A playtest surfaced that the TUI is hard to read: a hand card is a cramped token
(`[💙 v2 p1 ◆]`) that shows *that* a card has an effect but never *what it does*,
and the table has no atmosphere. New players can't learn the eight effects or six
modifiers from the screen. This change makes the client **readable and alive**
without leaking any hidden state — the cauldron stays blind by design (game-design
§4/§15). Direction and look were explored and validated as HTML sketches in
[docs/ui-explorations/](../../../docs/ui-explorations/) (a terminal-faithful cell-grid
preview, Playwright-checked for grid alignment, the live inspector, and no
boiling-point leak).

## What Changes

- **Readable card face.** Render each hand card at the design's readability priority
  — **volatility loudest**, then color, then points, then effect — with color carried
  by a **shape/sigil** as well as hue (color-blind / low-color safe), and any effect
  shown by **name**, not just a generic `◆` marker.
- **Live inspector (the terminal-native "tooltip").** A panel that explains the
  **cursor-selected** hand item live: a card's color/volatility/points and, for an
  effect, a plain-language description of what it does and its visibility; for pass,
  that it locks you out for the round while the blast still hits you. No mouse hover —
  it follows the existing `←/→` cursor and updates instantly.
- **Effect & Modifier Codex.** A toggleable `?` reference overlay listing all eight
  effects (with volatility/points + description) and all six modifiers (**qualitative
  direction only** — never a server-side magnitude, matching the client today).
- **Ambient, blind cauldron animation.** Permit information-free motion (bubbling,
  steam) that is **statistically independent** of the hidden volatility/boiling point —
  atmosphere that cannot become a cue. Polish the existing depile reveal and boom in
  the same spirit (no new disclosure).
- **Deterministic animation for tests.** Any time-based animation SHALL render under a
  fixed/seeded animation clock so the Layer-3 `TestBackend` snapshots stay stable
  (Constitution §II).

Out of scope: the Recall wire-target gap (tracked separately), any server/protocol
change, and the single-game lifecycle (owned by `group-model`).

## Capabilities

### New Capabilities

- **tui-codex** — a toggleable in-client reference for every effect and modifier.

### Modified Capabilities

- **tui-round-play** — adds a Readable Card Face requirement and a Card Inspector
  requirement, and modifies *Opaque Cauldron And Public Contribution* to explicitly
  permit ambient animation that is independent of the hidden state (so the new motion
  is spec-legal and the blind-volatility guarantee is sharpened, not weakened).
- **tui-debug-and-test** — modifies *TestBackend Snapshot Tests* to require
  deterministic rendering of time-based animation at a pinned phase.

> `tui-reveal-and-score` is **not** modified: its *Reverse-Order Depile* and *Boom
> Sequence* requirements already mandate the animated, skippable, descending-bar depile
> and the boom; the polish here is implementation detail under those requirements.

## Impact

- **Code (client only):** `tui-client/src/ui.rs` (card face, inspector panel, codex
  overlay, ambient cauldron frames, depile/boom polish), `tui-client/src/app.rs`
  (animation clock advanced in `on_tick`, `codex_open` state + `?` key; the cursor and
  hand state already exist), `tui-client/src/palette.rs` (single-width shape sigils
  alongside the emoji glyphs), `tui-client/tests/snapshots.rs` (pin the animation phase;
  add inspector-state and codex snapshots).
- **No** server, protocol, schema, dependency, or balance-number changes. The client
  remains a pure renderer of server state (Constitution §I).
