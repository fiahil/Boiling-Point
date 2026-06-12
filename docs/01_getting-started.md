# Getting Started

## Prerequisites

- **Rust** (stable, edition 2024) — `rustup` recommended. Builds the workspace
  (`protocol/` + `server/`).
- **PostgreSQL** — *optional.* With `DATABASE_URL` set, the server persists match
  results and replays post-game; without it, it runs fully in memory (persistence is a
  clean no-op). Not needed to build or play.

## Build, lint, test

The [`Makefile`](../Makefile) wraps the common commands:

```sh
make check      # fmt + clippy (-D warnings) + the full test suite — the CI gate
make fmt        # cargo fmt --all -- --check
make lint       # cargo clippy --workspace --all-targets -- -D warnings
make test       # cargo test --workspace
make test-unit  # tests minus the in-process server boot (transport::tests)
```

## Run the server

The server binary has `--help` (powered by `clap`):

```sh
# Server — player WebSocket (8080), admin API (8081), Prometheus metrics (9090).
cargo run -p boiling-point-server --             # defaults
cargo run -p boiling-point-server -- --ws-addr 0.0.0.0:9000 --log-level debug
```

The graphical client is the PixiJS web client at `clients/web/` (landing with the
`adopt-pixi-client` change). The v1 terminal client, bot harness, agent harness, and
the one-command `playtest.sh` are retired to [`archive/`](../archive/README.md) —
revivable, not deleted.

## Content & balance

Game balance (deck, thresholds, modifiers, effects) lives in
[`server/content.toml`](../server/content.toml) and is validated at startup — an
inconsistent config fails the boot, not a game. Load a custom config with
`--config <PATH>`. The numbers are hypotheses until playtested; balance is tuned from
the admin balance dashboard and structured playtests, and at-scale automated runs
("night brews") come from the AI client's harness mode —
[`clients/ai/`](../clients/ai/README.md) `balance_tester` (constitution §IV; see
[02_game-design.md §16](02_game-design.md)).

## Where to go next

- [03_architecture/01_overview.md](03_architecture/01_overview.md) — how the crates fit together.
- [02_game-design.md](02_game-design.md) — the rules and the design rationale.
- [`openspec/`](../openspec/) — how changes are proposed and tracked.
