# Design — tui-readability-pass

## Approach

The client architecture stays exactly as the review praised it: `ui.rs` is a pure
function of `App`, and `on_server`/`on_key`/`on_tick` are side-effect-free reducers.
This change adds **rendering** and a little **timed state**, nothing else.

- **State added to `App`:** an `anim_ms: u32` accumulator (advanced by `on_tick`) and a
  `codex_open: bool`. The inspector needs **no** new state — it derives entirely from
  the existing `cursor` + `vm.hand`.
- **Help text source.** A single in-client table maps each `EffectKind` →
  `(name, vol, pts, blurb, visibility)`. These are **public game facts** (game-design
  §9), not secrets. Reuse the existing `effect_name()` and `modifier_desc()` in `ui.rs`
  — `modifier_desc` already returns *direction only* ("boiling point lower"), which is
  exactly what the Codex and modifier chips must show.

### Card face
Widen the mini-card and lay it out by readability priority: a **loud volatility numeral**
(bold, ember), a **player-color frame + shape sigil** (triangle/heart/circle/square/lion,
single-width so it survives low-color terminals — extends today's `palette::glyph`), a row
of **points pips**, and the **effect name** in place of the bare `◆`. Keep the existing
soft `░` drop-shadow and the selected/committed border treatment.

### Live inspector
A bordered panel rendered on the playing and round-start screens, below the hand. It reads
`vm.hand[cursor]` (or the pass slot) and prints color · volatility · points and, for an
effect card, the effect name + blurb + visibility line. **Responsive:** the full panel when
there is room; it collapses to a single summary line at the 80×24 minimum.

### Effect & Modifier Codex
A `?`-toggled overlay, painted like the existing `emote_palette`/`peek_modal` overlays in
`draw()`. Lists all eight effects (name, vol/pts, blurb) and all six modifiers (name +
qualitative direction). Dismissed with `?`/`Esc`. Holds no hidden state.

### Ambient, blind cauldron animation
Bubble/steam frames are chosen from `anim_ms` **only** — never from `vm.cauldron_count`,
contributions, or anything correlated with volatility. The motion's distribution is
identical whether the pot is at 2 or 13. The depile flip-frame and the boom shake/flash ride
the timers that already exist (`depile_accum_ms`, `boom_ms`).

### Deterministic animation for snapshots
`on_tick(dt)` advances `anim_ms`; `render_to_buffer` (the test helper) renders at the current
phase. Snapshot tests pin `anim_ms` to a fixed value (e.g. 0) so buffers are stable, and add
coverage for: inspector on a plain card, on an effect card, on pass, and the codex overlay.

## Decisions

- **Live inspector is primary; the Codex complements it.** Confirmed in exploration: the
  inspector teaches the card in your hand with zero keystrokes; the Codex is the full
  reference for `?`. We are **not** gating effect help behind a press-only overlay.
- **Animation must be blind.** Atmosphere is allowed *only* if it is statistically
  independent of the hidden state — this is encoded as a spec scenario, not left to taste.
- **Depile bar stays descending.** Faithful to the existing `tui-reveal-and-score` spec and
  to `ui.rs`; an ascending "climb" was considered and rejected to avoid diverging the spec.
- **Single-width sigils for color shape.** The shipping client keeps the emoji player glyphs;
  the new effect/codex sigils are single-width to keep cell alignment exact (validated in the
  terminal-faithful sketch: every row is exactly the grid width).

## Constitution Check

- **§I Server-Authoritative** — PASS. The client gains no game logic and no new secret.
  The inspector and Codex show only public effect semantics and the player's own,
  already-known hand; the cauldron stays blind, and the *Opaque Cauldron* delta **tightens**
  that guarantee (ambient motion must be independent of hidden state).
- **§II Agent-Driven** — PASS. Pure reducers preserved; the new animation is made
  deterministic precisely so the `TestBackend` snapshot layer keeps working, and new
  snapshots cover the new surfaces. The look was pre-validated with Playwright over the
  HTML sketches.
- **§III Start Simple** — PASS. Reuses `draw_card`, `on_tick`, the overlay pattern, and the
  existing cursor; adds two small fields and one help table; no new dependency.
  *Rejected simpler alternative:* "just enlarge the volatility glyph" — fails the core ask
  (it still can't explain effects).
- **§IV Playtest-Driven** — N/A to balance. This is UX surfaced directly by a playtest; no
  scoring value, threshold, or card effect changes.
