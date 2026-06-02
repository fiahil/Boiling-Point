# Getting Started

## Prerequisites

- **Rust** (stable, edition 2024) — `rustup` recommended. Builds the server,
  terminal client, and bot harness.
- **Node.js ≥ 22** — only for the `agent-harness/` (Claude-as-player). Not needed
  to build or run the Rust workspace.
- **PostgreSQL** — *not required for v1 play.* Persistence is being reworked (see
  the `persistence-and-replays` change); the server runs fully in memory today.
- A terminal that renders Unicode + 256 colors for the TUI client.

## Build, lint, test

The [`Makefile`](../Makefile) wraps the common commands:

```sh
make check      # fmt + clippy (-D warnings) + the full test suite — the CI gate
make fmt        # cargo fmt --all -- --check
make lint       # cargo clippy --workspace --all-targets -- -D warnings
make test       # cargo test --workspace
make test-unit  # tests minus the in-process server boot (transport::tests)
```

## Run the pieces individually

Each binary has `--help` (powered by `clap`):

```sh
# Server — player WebSocket (8080), admin API (8081), Prometheus metrics (9090).
cargo run -p boiling-point-server --             # defaults
cargo run -p boiling-point-server -- --ws-addr 0.0.0.0:9000 --log-level debug

# Terminal client — connect to a live server, replay a recording, or run the mock.
cargo run -p boiling-point-tui -- --connect ws://127.0.0.1:8080/ws --name You
cargo run -p boiling-point-tui -- --mock         # offline scripted demo, no server

# Bot balance harness — seeded batch of headless games + balance report.
cargo run -p boiling-point-bot-harness -- --games 1000 --seed 1 --report out.json
```

## One-command playtest

[`scripts/playtest.sh`](../scripts/playtest.sh) (also `make playtest`) brings up the
server, fills a table with agent opponents, and drops you into the client — everyone
enters the matchmaking queue, so the table assembles itself (no invite codes to share):

```sh
# Free, instant heuristic opponents (no Claude auth, good for UI/flow testing):
make playtest ARGS="--brain fallback --agents 3"

# Real Claude opponents (uses your Claude Code login; costs tokens):
make playtest ARGS="--brain claude --difficulty hard"
```

Useful flags: `--agents N`, `--difficulty easy|hard`, `--brain claude|fallback`,
`--persona gambler|turtle|bandwagoner|trickster`, `--name`, `--no-build`. Logs land
in `.playtest/`.

> `--brain claude` authenticates via the Claude Code CLI login (or
> `CLAUDE_CODE_OAUTH_TOKEN`); any `ANTHROPIC_API_KEY` is neutralized so play doesn't
> silently bill API credits. See [`agent-harness/README.md`](../agent-harness/README.md).

## Content & balance

Game balance (deck, thresholds, modifiers, effects) lives in
[`server/content.toml`](../server/content.toml) and is validated at startup — an
inconsistent config fails the boot, not a game. Load a custom config with
`--config <PATH>`. The numbers are hypotheses until playtested; the bot harness is
the tool for tuning them (see [game-design.md §16](game-design.md)).

## Where to go next

- [architecture/overview.md](architecture/overview.md) — how the crates fit together.
- [game-design.md](game-design.md) — the rules and the design rationale.
- [`openspec/`](../openspec/) — how changes are proposed and tracked.
