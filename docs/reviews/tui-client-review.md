# Boiling Point — Terminal Client Review

A review of the terminal client crate (`tui-client/`) — a [ratatui](https://ratatui.rs)
renderer over the wire protocol — against the [constitution](../../CLAUDE.md)
(especially §I server-authoritative and §II agent-driven) and the
[game design](../game-design.md).

Reviewed 2026-06-02 against `main`; **refreshed 2026-06-05** after `group-model`,
`tui-readability-pass`, and `converge-game-loops` landed. Tests green: 24 snapshot
tests + the live-server integration test pass. Several items this review would have
raised were fixed in the same pass that produced it — they're recorded below as
**resolved** for traceability.

**Overall:** an exemplary untrusted renderer. The client holds **no game logic and no
secrets by construction**, its core is a set of pure reducers that snapshot-test with
neither a terminal nor a server, and it respects the secret boundary cleanly. The one
remaining gap is a genuine protocol gap — Recall has no wire *target* (T4); the
single-game lifecycle that this review flagged is now resolved by `group-model`'s
play-again.

---

## 1. Architecture

`App` (`src/app.rs`) is a pure state machine: `on_server`, `on_key`, and `on_tick`
are deterministic and side-effect-free — they fold a message/keypress/tick into state
and **return** `ClientMessage` intents rather than sending them. The transport
(`src/net.rs`) and terminal (`src/term.rs`) are the only impure edges, and the client
is fully exercisable without either (research R5).

| Module | Role |
|---|---|
| `app.rs` | `App` + the three reducers; phase transitions; input → intents. |
| `view.rs` | `ViewModel` — the player-visible state, folded from `ServerMessage`s. |
| `ui.rs` | All rendering. Every screen is a pure function of `App`. |
| `net.rs` | Live WebSocket: a read task decodes inbound, a write task encodes outbound, bridged by channels. |
| `fixtures.rs`, `replay.rs` | Scripted demo game + JSON-lines record/replay. |
| `palette.rs`, `term.rs` | Colors/glyphs; terminal setup/teardown. |

Three transport modes (`--connect`, `--replay`, `--mock`) plus the scripted
`--enqueue` entry, all resolved through `clap` (`src/lib.rs`).

## 2. Server-authoritative & the secret boundary — strong

The `ViewModel` (`view.rs:109-157`) is **secret-free by construction**: there is no
field for the boiling point (except `my_peek`, which exists only because the server
privately told *this* player), no opponents' hands, and no draw deck. The snapshot
test `playing_cauldron_is_opaque` asserts the playing screen never contains
`"boiling"`, and `depile_safe_hides_boiling_point` asserts a safe brew shows no bp
value — leakage is prevented by omission and guarded by test. The client validates
nothing and computes no outcome; it renders server state and emits intents
(`app.rs` `on_key` → `Vec<ClientMessage>`). This is the constitution's §I in textbook
form.

## 3. Agent-driven testability — strong

`tests/snapshots.rs` renders each screen through ratatui's `TestBackend` and asserts
on the flattened text buffer — the agent-readable "screenshots," with no terminal and
no server. Coverage spans lobby, round-start, playing, both depile outcomes, scoring,
explosion, deathmatch, game-over, reconnect overlay, and the replay round-trip.
`tests/live_server.rs` adds an end-to-end pass against a real in-process server. This
is the canonical place to cover a screen — there are deliberately **no `examples/`**
(see T3).

## 4. Findings

### T1 — Group/invite code vanished the instant play began *(usability — RESOLVED 2026-06-02)*

The code rendered **only** on the `lobby()` screen, shown for the
`Connecting|Queue|Lobby` phases. The moment the table filled, the phase advanced to
`RoundStart`→`Playing` and the lobby (and the code) disappeared — with no persistent
chrome carrying it. **Fixed:** the code now renders in the shared `header()`
(`ui.rs`), visible across every in-game phase, with the regression test
`group_code_visible_during_play`.

### T2 — "Copy invite code" was unreliable and is removed *(usability — RESOLVED 2026-06-02)*

Copy used OSC 52 terminal escapes, which silently no-op on terminals that don't
support them (a common case) — the playtester reported it "doesn't work." Rather than
add a native-clipboard dependency, the feature was **removed** (the `clipboard`
module, the `[c]` keybinding, and the hint). The code is plainly visible to read/share.

### T3 — `examples/gallery.rs` duplicated the snapshot tests *(hygiene — RESOLVED 2026-06-02)*

The example rendered three screens to stdout — all already asserted in
`tests/snapshots.rs`. Per the constitution's "examples should be tests" spirit it was
**removed**; the snapshot suite is the single source of visual coverage.

### T4 — Recall has no target on the wire; the effect is not fully playable *(protocol gap, medium — OPEN)*

`commit_cursor`/`key_recall` open a target picker for Recall, but `CommitCard` carries
no target field, so the chosen card can't be transmitted — the client sends the Recall
card and toasts *"recall target not yet carried by the wire"* (`app.rs`). This is a
**protocol-level gap**, not a client bug: `ClientMessage::CommitCard { card }`
(`protocol/src/client.rs`) still has no Recall-target field, and the server must honor
it. `converge-game-loops` added D3 — the owner now learns their hand grew when a recall
fires (a private `YourHand`) — but that is the *visibility* of an auto-chosen recall,
not player-driven target *selection*; until `CommitCard` carries a target, Recall (a
designed §9 effect) stays only partially playable end-to-end. Track as a dedicated
wire-protocol change.

### T5 — Single-game lifecycle *(RESOLVED by `group-model`)*

The original review flagged that at `GameOver` the client could only re-enter the queue
or reset to the entry menu — no "play again with the same table." **Resolved:**
`group-model` made groups persist across games, and the client now opts in with
`ClientMessage::PlayAgain` at `GameOver`, keeping the roster, identity, and group code
(`app.rs` `reset_for_new_game` / the play-again path). A fresh game starts once the
table re-readies.

### T6 — Minor notes *(low)*

- The `--mock`/`--replay` fixtures hard-code `BREW-7K3F`; harmless, and useful for
  deterministic snapshots.
- `set_deathmatch`/`set_reconnecting` are test/mock-only helpers; real play drives
  both from `ServerMessage`s (`DeathmatchStarted` sets `vm.deathmatch` in
  `view.rs`), so the live paths are covered.
- Display wording and the protocol field names are now uniformly **group** (`GroupCode`,
  `group_code`) — `group-model` completed the room→group rename end to end.

### T7 — Readability pass: card faces, a live inspector, and a Codex *(addressed by `tui-readability-pass`)*

A playtest found the UI hard to read — a hand card showed only *that* it had an effect
(`◆`), never what. The **`tui-readability-pass`** change reworks the card face (volatility
loudest, colour as a shape sigil, points as pips, the effect **by name**), adds a
cursor-driven **live inspector** that explains the selected card or the Pass slot, a `?`
**Codex** of all effects and modifiers (effects with vol/points; modifiers direction-only),
and ambient — information-free — cauldron motion. The cauldron stays blind
(`playing_cauldron_is_opaque` still passes) and the animation is rendered at a pinned phase
so the `TestBackend` snapshots stay deterministic.

## 5. Recommendations

1. **Close T4 at the protocol layer** — add the Recall target to `CommitCard` and
   honor it server-side, then wire the picker's choice through. The one remaining
   open item: it makes a designed effect fully playable. (D3 already gives the owner
   visibility of an auto-chosen recall, so this is purely the target-selection wire.)
2. ~~Leave T5 to `group-model`~~ — **done**: `group-model` landed and the client's
   play-again flow keeps the table together across games.
3. Keep new screens covered by `tests/snapshots.rs` (no `examples/`).

## 6. Constitution compliance

| Principle | Verdict | Notes |
|---|---|---|
| **I. Server-Authoritative** | Strong | No game logic, no secrets; `ViewModel` is secret-free by construction; client only renders + emits intents. |
| **II. Agent-Driven** | Strong | Pure reducers; `TestBackend` snapshot screenshots; deterministic; no terminal/server needed to test. |
| **III. Start Simple** | Strong | Single-game flow, OSC-free (now no clipboard dep), three simple transport modes. |
| **IV. Playtest-Driven** | n/a (client) | The client surfaces the playtest UX; this batch came directly from a playtest. |
