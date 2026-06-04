## 1. Readable card face

- [x] 1.1 Add single-width shape sigils per color in `palette.rs` (triangle/heart/circle/square/lion) alongside the existing emoji `glyph()`, for low-color/cell-exact rendering
- [x] 1.2 Rework `draw_card` in `ui.rs`: loud volatility numeral, color frame + sigil, points pips (`●○○`), and the effect **name** (via `effect_short`/`effect_name`) instead of the bare `◆`
- [x] 1.3 Keep the existing drop-shadow (now shared `card_shadow`), and the selected (cursor) and committed (in-pot) border styling
- [x] 1.4 Snapshot: a plain color card and an effect card render with the new face (`card_face_shows_effect_name_and_pips`)

## 2. Live inspector

- [x] 2.1 Add an `effect_help(EffectKind) -> (vol, pts, blurb, visibility)` table in `ui.rs` (public §9 facts; reuse `effect_name`)
- [x] 2.2 Render an inspector panel on the playing and round-start screens describing `vm.hand[cursor]` — color · volatility (incl. effective volatility note for Surge) · points, plus the effect blurb + visibility line
- [x] 2.3 When the cursor is on the pass slot, the inspector explains pass = locked out for the round, and the explosion loss still applies
- [x] 2.4 Make it responsive: full panel when space allows (`insp_h == 5`), collapse to a single summary line at the 80×24 minimum (`insp_h == 1`)
- [x] 2.5 Assert the inspector never prints a boiling-point value or hidden cauldron state (`inspector_explains_selected_effect` asserts the playing screen still lacks "boiling")
- [x] 2.6 Snapshots: inspector on a plain card, on an effect card, and on pass (`inspector_follows_cursor`, `inspector_explains_selected_effect`, `inspector_explains_pass`)

## 3. Effect & Modifier Codex

- [x] 3.1 Add `codex_open: bool` to `App` and a `?` key toggle (and `Esc`/`?` to dismiss) in `on_key`
- [x] 3.2 Render a Codex overlay (like `emote_palette`/`peek_modal`) listing all eight effects with name + vol/pts + blurb
- [x] 3.3 List all six modifiers with **qualitative direction only** (reuse `modifier_desc`) — never a numeric magnitude
- [x] 3.4 Assert the Codex shows direction not magnitude (`codex_lists_effects_and_modifiers` asserts "boiling point lower" present, `-4`/`+4`/`+3` absent)
- [x] 3.5 Snapshot: the Codex overlay

## 4. Ambient (blind) cauldron animation

- [x] 4.1 Add `anim_ms: u32` to `App`; advance it in `on_tick(dt_ms)` (wrapping)
- [x] 4.2 Render ambient cauldron motion (bubbling) driven by `anim_ms` **only** — never by `cauldron_count`, contributions, or any volatility-correlated value
- [x] 4.3 Polish the boom with a shake + ember/red flash on the existing `boom_ms` timer; the depile already animates per the existing `tui-reveal-and-score` spec (stepped reverse reveal + descending bar), left intact
- [x] 4.4 Confirmed the cauldron path reads only `anim_ms` for motion and `cauldron_count` only for the (always-`??`) count text — no animation parameter reads hidden/state-correlated data

## 5. Deterministic animation for snapshots

- [x] 5.1 `render_to_buffer` renders at the current `anim_ms`; snapshot fixtures leave it at 0 (the default) so buffers are stable
- [x] 5.2 Confirmed the existing core-phase snapshots still pass with animation present at the pinned phase (all 18 prior + 6 new)
- [x] 5.3 Added a test that advancing `on_tick` leaves the public counts/`?? / ??` intact and that phase-0 renders are identical (`ambient_animation_is_deterministic_and_blind`)

## 6. Validation

- [x] 6.1 `make check` green (fmt + clippy -D warnings + tests)
- [x] 6.2 Re-confirmed the secret-boundary snapshot tests (`playing_cauldron_is_opaque`, `depile_safe_hides_boiling_point`) still pass
- [x] 6.3 Updated [docs/reviews/tui-client-review.md](../../../docs/reviews/tui-client-review.md) with a T7 note on the readability pass
