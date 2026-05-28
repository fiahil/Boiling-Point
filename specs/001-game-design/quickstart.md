# Quickstart: Boiling Point — Development Setup

**Feature**: `001-game-design` | **Date**: 2026-05-28

---

## Prerequisites

| Tool | Version | Purpose |
|---|---|---|
| Rust | stable (1.75+) | Server, shared types, bot harness |
| PostgreSQL | 15+ | Player accounts, match history |
| Node.js | 20+ | Playwright for visual client tests (when client is built) |
| cargo-watch | latest | Auto-rebuild on file changes |

---

## Project Structure

```
cargo workspace
├── server/        # Axum + Tokio — authoritative game logic
├── client/        # Game client (TBD — Macroquad/Godot/Flutter)
├── shared/        # Protocol types, enums, serde derives
├── bot-harness/   # Headless bots for balance testing
└── agent-harness/ # Claude-as-player wrapper
```

---

## First-Time Setup

```bash
# Clone and enter
git clone <repo-url> && cd boiling-point

# Install Rust toolchain
rustup default stable
rustup target add wasm32-unknown-unknown  # for future WASM client

# Database setup
createdb boiling_point
# Run migrations (when they exist)
# cargo run --bin migrate

# Build everything
cargo build --workspace
```

---

## Development Workflow

### Run the server

```bash
cargo watch -x 'run --bin server'
# Listens on ws://localhost:3000
```

### Run bot harness (balance testing)

```bash
# Run N simulated games with heuristic bots
cargo run --bin bot-harness -- --games 1000 --report

# Output: explosion rate, strategy win rates, round durations
```

### Run agent harness (Claude-as-player)

```bash
# Start a game with 3 bots + 1 Claude player
cargo run --bin agent-harness -- --claude-players 1 --bot-players 3
```

### Run tests

```bash
# All workspace tests
cargo test --workspace

# Server tests only
cargo test -p server

# Shared types tests
cargo test -p shared
```

---

## Key Development Milestones

This feature (001-game-design) produces design artifacts only. The implementation sequence for subsequent features:

1. **`shared/` crate** — Protocol types, enums, CardDefinition, serialization
2. **`server/` game logic** — Round state machine, scoring, effect resolution
3. **`server/` networking** — WebSocket server, room management, message routing
4. **`bot-harness/`** — Headless bots, balance statistics, strategy heuristics
5. **`client/`** — Game UI (pending client technology decision)
6. **`agent-harness/`** — Claude-as-player testing wrapper

---

## Configuration

### Environment Variables

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | `postgres://localhost/boiling_point` | PostgreSQL connection string |
| `BIND_ADDR` | `0.0.0.0:3000` | Server listen address |
| `LOG_LEVEL` | `info` | tracing log level |
| `WAVE_TIMER_FIRST_MS` | `30000` | First wave timer (ms) |
| `WAVE_TIMER_SUBSEQUENT_MS` | `10000` | Subsequent wave timers (ms) |
| `RECONNECT_GRACE_MS` | `60000` | Reconnection grace period (ms) |

### Deck Configuration

The 88-card deck is defined in `shared/src/deck.rs` (when implemented). All card definitions, distributions, and effect parameters are data-driven and configurable for playtesting.

---

## Observability

| Metric | Description | Target |
|---|---|---|
| `explosion_rate` | % of rounds ending in explosion | 30–40% |
| `round_duration_ms` | Time from drafting to scoring | 60,000–90,000 |
| `dominant_strategy_rate` | % of games won by any single strategy type | <35% |
| `turn_timeout_rate` | % of wave actions that were auto-pass | <15% |
| `wave_count_per_round` | Average waves before round ends | 3–6 |
