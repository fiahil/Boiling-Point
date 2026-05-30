## Why

The `server-release-1` change ships the authoritative engine and validates it with connection smoke tests and in-process integration tests — but the constitution makes the **headless bot harness a first-class testing layer** (Principle II) and the **primary tool for data-informed balance** (Principle IV): the deck distributions, thresholds, modifier weights, and effect counts are all hypotheses until thousands of bot games confirm a healthy explosion rate (~30–40%) and surface degenerate strategies. This change adds that Layer-1 harness as a separate, focused effort so it can be built and detailed properly without bloating R1.

## What Changes

- Add a `bot-harness/` crate that drives the server over the **same public WebSocket protocol** a real client uses (reusing the `protocol/` crate), receiving only player-permitted information.
- Give the bot its **own narrow, player-visible domain model** built solely from received messages — structurally incapable of holding server secrets (no boiling point, no opponents' hands, no draw deck), continuously validating the secret boundary.
- Provide a **pluggable strategy interface** plus several baseline heuristic strategies (cautious, aggressor, diplomat, random) so different play patterns can compete and be compared.
- Provide a **seeded, deterministic batch runner** that plays thousands of complete games (4 bots each, including any Deathmatch) and **aggregates balance statistics**, with reproducible runs from a seed.
- Detect and report **degenerate strategies and balance smells** (a strategy or color winning disproportionately, explosion rate far off target, runaway pots) to feed the content config knobs.

## Capabilities

### New Capabilities
- `bot-harness`: a headless protocol client with its own player-visible domain model, a pluggable strategy interface, and baseline strategies that play complete games end to end.
- `balance-analysis`: the seeded batch runner, statistics aggregation, and degenerate-strategy/balance-smell detection and reporting.

### Modified Capabilities
- _None_ — this change consumes the `server-release-1` protocol and engine without changing their requirements.

## Impact

- **Depends on `server-release-1`**: requires the published `protocol/` crate and a runnable server (or an in-process server handle the harness can spawn).
- **New code:** a `bot-harness/` workspace crate (strategy trait, baseline strategies, batch runner, statistics, reporting) and a CLI entry point for batch runs.
- **New dependencies:** `tokio-tungstenite` (or reuse of an in-process server handle), a seeded RNG (`rand` with a fixed seed), and a stats/serialization helper for reports.
- **Feeds:** the `game-content-config` balance knobs in `server-release-1` — harness output is the data behind every `[needs playtesting]` value.
