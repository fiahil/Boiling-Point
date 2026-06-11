# Proposal: boom2-observability

## Why

`boom2-combat-core` replaces the game model that the entire observability read path is contracted on. The span-schema contract (v1 tree: room → game → round → wave → commit/resolve → score, `SPAN_SCHEMA_VERSION = 1`), the admin projection's unsampled aggregates, and the balance dashboard's metric set (explosion rate vs the ~30–40% v1 target, cards per round, dominant-color rate, reshuffle frequency) are all v1-shaped — and no other `boom2-` change carries observability deltas. The day the v2 core lands, v2 games go dark for operators, exactly when Principle IV needs the admin balance dashboard watching the rebuilt balance economy in live play (the harness covers the offline population; this is its live counterpart).

## What Changes

- **BREAKING (span schema): the v2 span tree.** `SPAN_SCHEMA_VERSION` bumps 1 → 2 — the versioning seam the contract was designed around. The game subtree is re-based on the v2 core: wave spans carry the ingredient-or-pass action and the optional spell cast; resolution spans carry pot value P, the fatal-wave sort, the detonator split, and the every-round depile/boiling-point reveal; pre-game spans cover the brewer pick and the Apothecary draft (staged as those changes land). The projection's forward/backward tolerance is unchanged.
- **BREAKING (metrics): the v2 balance metric set.** The v1 figures retire with the model that defined them. The v2 set (all targets `[needs playtesting]`): boom rate, detonator distribution and fold timing, wave depth and duration, per-spell cast rate, per-Brewer pick and win rate, bucket pick rates, and compounding trigger rates — the last three staged behind `boom2-brewers` / `boom2-apothecary` / `boom2-compounding`. Fleet/ops figures (live games, groups, connected players, timeouts, reconnections) carry over unchanged.
- **One metric definition, two populations.** Every v2 balance metric is defined **once**, in server code, and consumed by both the live pipeline (Prometheus + projection + dashboard, human games) and the benchmarking suite's balance studies (`boom2-benchmarking`, bot games) — so "does live play match the harness?" is a direct comparison, never a reconciliation.
- **Observability is part of the admin command center.** The admin UI is the single operator surface: every operator-facing observability surface (balance dashboard, room inspector, replays) lives inside it, next to the control actions, behind admin auth. New observability lands there — embedded panels, not standalone tools; Grafana remains an embedded renderer, never a separate destination.

## Capabilities

### New Capabilities

- `boom-balance-metrics` — the v2 balance metric definitions: named metrics, formulas over v2 span attributes, and `[needs playtesting]` targets, defined once and consumed by both live telemetry and balance studies.
- `admin-command-center` — the admin UI as the single operator surface: hosts all operator-facing observability alongside control, behind admin auth; new observability surfaces MUST land inside it.

### Modified Capabilities

- `otel-span-pipeline` — the Documented, Versioned Span Tree requirement re-bases on the v2 game subtree (schema v2).
- `persistence-and-observability` — Structured Tracing names the v2 tree; Game-Balance Metrics emits the v2 set (via `boom-balance-metrics`).
- `admin-span-projection` — the Versioned Span-Schema Contract requirement moves to v2; Unsampled Rolling Aggregates compute the v2 metrics.
- `balance-dashboard` — the metrics surface swaps to the v2 set; the dashboard is anchored inside the admin command center (embedded-Grafana-behind-auth rule unchanged).
- `room-inspector` — live listing and the privileged reveal re-key on v2 spans (waves, pot, spell hands, the hidden boiling point).
- `match-replays` — the per-wave action log vocabulary gains the v2 actions (spell casts, colored/colorless Votes, pass/fold).

## Impact

- **Server:** `server/src/observability/` (span schema v2, metric emitters, the shared metric-definition module), the admin projection, and the admin routes. No player-wire change (Principle I untouched — spans stay operator-only).
- **Sequencing:** hard dependency on `boom2-combat-core` (it owns the engine events being instrumented); lands with or immediately behind it so there is no unobservable window. Brewer/Apothecary/compounding panels are phased tasks gated on those changes.
- **`boom2-benchmarking`:** its `balance-study` instrument consumes `boom-balance-metrics` definitions instead of redefining its own — one small coordination note there, no structural change.
- **Docs (constitution v2.1.1 docs-currency):** `docs/03_architecture/04_span-schema-contract.md` is rewritten as the v2 contract in the same change.
- **Infrastructure:** none added — Prometheus + embedded Grafana stay as-is; the OTLP backend decision still rides `boom2-delivery`.
- **Testing:** unit tests on the metric definitions and projection re-keying; the server-side headless e2e suite's "triggering a boom" scenario is the natural place to assert the boom span/metrics appear (no new e2e infrastructure).
