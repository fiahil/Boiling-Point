# bot-harness

The **Layer-1 balance harness**: headless Rust bots that play complete games and a
seeded batch runner that aggregates balance statistics. It's a pure consumer of the
public wire protocol — a bot receives only what a real player would, which doubles as
a check on the server's secret-management contract (constitution Principle IV).

## Layout

| Module | Role |
|---|---|
| `bot.rs`, `model.rs` | A bot and its **player-visible** domain model (no field can hold a secret — no boiling point, no opponents' hands, no deck). |
| `strategy.rs` | Pluggable per-seat strategies. Baselines: `cautious`, `aggressor`, `diplomat`, `random`. |
| `runner.rs`, `transport.rs` | The batch runner and the two transports: `in-process` (reproducible) and `websocket` (real wire). |
| `rng.rs` | The deterministic seeded RNG tree (reproducible runs). |
| `stats.rs`, `report.rs` | Balance aggregation and the human/JSON reports. |

## Run

```sh
cargo run -p boiling-point-bot-harness -- --help

# 1000 games, default 4 baseline strategies, print a markdown report:
cargo run -p boiling-point-bot-harness -- --games 1000 --seed 1

# Custom seats + machine-readable report for diffing across config versions:
cargo run -p boiling-point-bot-harness -- \
  --strategies cautious,aggressor,diplomat,random --report report.json
```

| Flag | Default | Purpose |
|---|---|---|
| `--games` | `1000` | number of complete games |
| `--seed` | `0` | root seed for the deterministic RNG tree |
| `--transport` | `in-process` | `in-process` (reproducible) or `websocket` (real wire) |
| `--strategies` | the four baselines | exactly 4 comma-separated seat strategies |
| `--config <PATH>` | embedded server config | content/balance config to test |
| `--report <PATH>` | — | write the JSON report |

## Test

```sh
cargo test -p boiling-point-bot-harness
```

`tests/validation.rs` checks reproducibility (same seed → identical outcomes), seed
divergence, and degenerate-strategy detection, across both transports.

The harness is the primary tool for tuning the open balance knobs in
[`docs/game-design.md §16`](../docs/game-design.md). The Layer-2 Claude-as-player
harness lives in [`agent-harness/`](../agent-harness/README.md).
