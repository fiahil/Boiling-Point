# tui-client

The Boiling Point **terminal client** — an untrusted [ratatui](https://ratatui.rs)
renderer over the wire protocol. It renders server state and turns key presses into
client intents; it owns **no game logic and holds no secrets** (constitution
Principle I).

## How it's built

`App` and its pure reducers (`on_server`, `on_key`, `on_tick`) are the testable core:
they return intents rather than sending, so the whole client runs with **no terminal
and no server**.

| Module | Role |
|---|---|
| `app.rs` | `App` state machine + phase reducer (the heart of the client). |
| `view.rs` | `ViewModel` — player-visible state folded from `ServerMessage`s. |
| `ui.rs` | All rendering. Every screen is a pure function of `App` (snapshot-testable). |
| `net.rs` | Live WebSocket transport (read/write tasks bridged by channels). |
| `fixtures.rs`, `replay.rs` | Scripted demo game and JSON-lines record/replay. |
| `palette.rs`, `term.rs` | Colors/glyphs and terminal setup/teardown. |

## Run

```sh
cargo run -p boiling-point-tui -- --help

cargo run -p boiling-point-tui -- --connect ws://127.0.0.1:8080/ws --name You
cargo run -p boiling-point-tui -- --connect ws://127.0.0.1:8080/ws --enqueue   # auto-join the queue
cargo run -p boiling-point-tui -- --mock                                       # offline scripted demo
cargo run -p boiling-point-tui -- --replay session.jsonl                        # replay a recording
```

`--enqueue` (requires `--connect`) skips the entry menu and drops straight into the
matchmaking queue — used by [`scripts/playtest.sh`](../scripts/playtest.sh).

## Test

```sh
cargo test -p boiling-point-tui
```

- `tests/snapshots.rs` — Layer-3 visual tests: render each screen through
  `TestBackend` and assert on the text buffer. These are the agent-readable
  "screenshots" (plain text, no terminal, no server) and the canonical place to cover
  a screen — there are deliberately **no `examples/`**.
- `tests/live_server.rs` — end-to-end against a real in-process server.

See [`docs/04_reviews/03_tui-client-review.md`](../docs/04_reviews/03_tui-client-review.md).
