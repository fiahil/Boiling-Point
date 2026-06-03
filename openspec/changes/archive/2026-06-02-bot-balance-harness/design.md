## Context

`server-release-1` delivers the authoritative engine and the `protocol/` wire crate, validated by smoke and in-process tests. What it deliberately defers is the Layer-1 bot harness — the constitution's primary instrument for data-informed balance (Principle IV) and a continuous test of the secret boundary (Principle I/II). This change builds that harness on top of R1, reusing the same protocol a real client will use.

## Goals / Non-Goals

**Goals:**
- Play thousands of complete games headlessly, reproducibly (seeded), and aggregate the statistics that drive every `[needs playtesting]` value.
- Reuse the `protocol/` crate verbatim and consume only player-permitted information, so the harness doubles as a living test of the no-leak contract.
- Make strategies pluggable so play patterns compete and degenerate ones surface.

**Non-Goals:**
- No Claude-as-player agent harness (Layer 2) and no visual tests (Layer 3) — separate future changes.
- No changes to the server's rules or protocol requirements — this is a pure consumer.
- No automatic config tuning — the harness *reports*; a human (or agent) edits the content config and re-runs.

## Decisions

### D1. Transport — in-process server handle by default, WebSocket optional

The batch runner defaults to spawning the server **in-process** (a room/game handle in the same binary) for speed and determinism, avoiding socket and scheduling noise across thousands of games. A WebSocket mode (via `tokio-tungstenite`) is also supported to exercise the *real* wire path in a smaller batch. *Why:* in-process makes thousands of games fast and seed-reproducible; the WS mode keeps the harness honest about the actual protocol. Both go through the same `protocol/` message types, so a bot's code is transport-agnostic.

### D2. Bot domain model is the player's view, by construction

The bot builds its world solely from received `ServerMessage`s. There is intentionally **no type** in the bot for the boiling point, opponents' hands, or the draw deck — the only way such a value enters the model is a message that legitimately carries it (the bot's own `PeekResult`, or an explosion depile). This makes leakage a compile-time impossibility rather than a discipline, and turns every batch into a secret-boundary test.

### D3. Strategy as a trait (Strategy pattern)

Each decision point (commit/pass, which card, which effect target, emotes) is a method on a `Strategy` trait. Baseline implementations — cautious, aggressor, diplomat, random — ship for comparison. Strategies are pure functions of the bot's player-visible model plus its RNG, which keeps them deterministic under a seed.

### D4. Determinism via a single seeded RNG tree

One root seed derives per-game and per-bot RNG streams, so a `(seed, strategy assignment, content config)` triple fully reproduces a run — essential for debugging a flagged game and for regression comparisons between config versions. *Risk:* any non-seeded source (wall-clock, hashmap iteration order) breaks reproducibility → the harness forbids them on the decision path and tests the same-seed-same-result property.

### D5. Reporting feeds the config knobs

Aggregated statistics and balance-smell flags are emitted as a structured report (human-readable summary + machine-readable file) keyed to the content-config version under test, so a tuning loop is: run → read flags → edit `game-content-config` → re-run → diff.

## Risks / Trade-offs

- **In-process coupling to server internals** → keep the harness on the public `protocol/` boundary even in in-process mode; the server exposes a thin "spawn a game, feed it protocol messages" handle, not its guts.
- **Strategies too naive to surface real degeneracies** → baselines are a starting point; the pluggable trait lets sharper strategies be added as balance understanding grows.
- **Determinism drift** → a same-seed-same-result test guards it; any failure means a non-seeded source crept onto the decision path.
- **Statistics mislead if the bot pool is unrepresentative** → report always states the strategy mix, so results are read in context.

## Migration Plan

Additive — a new crate consuming R1. No schema or protocol changes. Ships after `server-release-1` is implemented enough to host a game (engine + protocol; persistence optional for in-process runs).

## Open Questions

- Whether the server should expose a dedicated in-process "game driver" API or the harness should always go over WebSocket — leaning in-process-by-default with a WS mode (D1), revisit if the in-process handle proves leaky.
- The exact balance-smell thresholds — themselves `[needs playtesting]`; ship sensible defaults and refine from real batch output.
