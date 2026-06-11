# bot-harness

The **Layer-1 balance harness**: headless Rust bots that play complete games and a
seeded batch runner that aggregates balance statistics. It's a pure consumer of the
public wire protocol — a bot receives only what a real player would, which doubles as
a check on the server's secret-management contract (constitution Principles I/IV).

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

Revived from `archive/` with change `boom2-combat-core` and rewritten against the
v4 (ingredient/spell) protocol. Beyond the v1 metrics, the report now carries the
boom2 mandate's statistics: explosion rate vs the ~45% target, detonator
distribution by strategy, Peek-fire rate, and freeze (all-pass) rates — the
first derivation is recorded in
[`docs/06_boom2/02_toward-a-v2-core.md`](../docs/06_boom2/02_toward-a-v2-core.md).
The retired Layer-2 Claude-as-player harness lives in
[`archive/agent-harness/`](../archive/agent-harness/README.md).
