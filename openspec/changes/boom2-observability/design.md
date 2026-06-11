# Design: boom2-observability

## Context

The v1 observability stack is a designed pipeline: the server emits OTEL spans following a versioned schema contract (`SPAN_SCHEMA_VERSION = 1`, human companion `docs/03_architecture/04_span-schema-contract.md`, source of truth `server/src/observability/span_schema.rs`); an in-process span-lifecycle hook feeds the admin projection (open-span registry, unsampled rolling aggregates, bounded replay buffer); the admin UI reads only the projection and renders the balance dashboard via embedded Grafana over Prometheus; control is a separate audited command API. All of it is contracted on the v1 game model.

`boom2-combat-core` replaces that model wholesale (waves with ingredient-or-pass + optional spell, detonator-only boom, every-round depile, two decks) with no v1/v2 coexistence. Without this change, the v2 core lands with a span tree nobody emits, aggregates over events that no longer happen, and a dashboard charting retired metrics — during exactly the live playtests Principle IV needs instrumented. Separately, the user direction (2026-06-11) is that observability is part of the **admin command center**: one operator surface, no standalone tools.

Constitution constraints: v2.1.1 §II (docs updated in the same change; e2e server-side and headless), §IV (balance data-informed via telemetry and the dashboard), §I (spans stay operator-only; nothing new on the player wire).

## Goals / Non-Goals

**Goals:**
- Span schema v2 covering the combat core, shipped so there is no unobservable window after the engine swap.
- The v2 balance metric set defined once (`boom-balance-metrics`) and consumed by the live pipeline and the balance studies alike.
- The dashboard, inspector, and replays re-keyed on v2, hosted inside the admin command center.
- The v2 contract doc (`04_span-schema-contract.md`) rewritten in the same change.

**Non-Goals:**
- No new infrastructure: Prometheus + embedded Grafana stay; the OTLP backend decision still rides `boom2-delivery`.
- No projection architecture change: registry/aggregates/replay-buffer mechanics, the lifecycle hook, and the read-only guarantees are untouched — only what they are keyed on changes.
- No player-facing change of any kind (no wire-protocol delta).
- No new balance numbers: targets are seeded from the decision log and re-derived by studies (R7).

## Decisions

### D1. Metric definitions as a public server module (R1)

`server/src/observability/balance_metrics.rs` holds named definitions — id, formula as a pure function over v2 span/engine events, unit, optional `[needs playtesting]` target. Prometheus emitters and the projection's aggregates evaluate them in-process; the balance-study runner (`boom2-benchmarking`), which already links the server engine, imports the same module. Rejected: a separate definitions crate (structure before need — the module path is the seam) and per-consumer formulas with a conformance test (institutionalizes drift).

### D2. One schema bump; additive growth documented as planned (R2)

`SPAN_SCHEMA_VERSION` 1 → 2 once, with the combat-core rebase. The v2 contract documents the whole intended tree — including `brewer.pick` and `draft` marked *planned* — so content changes extend additively under the projection's ignore-unknown tolerance, with no further bumps. Rejected: a bump per content change (version churn signalling breakage where there is none).

### D3. The command center is a surface capability, not a refactor (R3)

`admin-command-center` specs the surface rule (everything operator-facing lives in the admin UI, reads via projection, control via command API); `admin-auth` / `admin-control` / `admin-span-projection` stay untouched as mechanism specs. The admin app gains navigation/hosting for the v2 panels; embedded Grafana remains the renderer; the offline bench dashboard is linked, never hosted. Rejected: renaming the admin specs around the term (churn without behavior change).

### D4. Atomic cutover with combat-core (R4)

This change lands with or immediately behind `boom2-combat-core` — same deploy window. v1 balance series simply stop being written and queried (historical Prometheus data remains); fleet/ops metrics keep their identity across the cutover, giving operators one continuous set of charts through the swap. Metric ids are versioned by the v2 definitions, so series never collide.

### D5. Per-feature panels phase in here (R5)

Task groups 5–7 are gated on `boom2-brewers` / `boom2-apothecary` / `boom2-compounding` landing; each adds definitions (D1) + spans (D2, additive) + a panel (D3). If this change archives before a content change lands, its remaining phase moves to a small follow-up change rather than holding the archive open.

## Risks / Trade-offs

- **[Combat-core engine churn invalidates instrumentation]** → instrument via the engine's event seams, not its internals; the span emitters live beside the engine events combat-core defines, and the harness-driven balance iteration (§IV) changes numbers, not event shapes.
- **[Shared definitions create a build coupling: balance studies ↔ server crate]** → already true (studies link the engine to play games in-process); the module adds no new edge. If the coupling ever hurts, the module is the seam to split into a crate.
- **[This change stays open a long time waiting on phases 5–7]** → D5's escape hatch: archive after the core phases; spin remaining panels into a follow-up change.
- **[Dashboard reads garbage if it ships before combat-core]** → hard sequencing dependency stated in the proposal; tasks 2+ block on combat-core's engine events existing.
- **[Grafana embed tempts standalone use]** → the command-center capability makes "embedded only, behind admin auth" a standing requirement, testable in review.

## Migration Plan

1. Land span schema v2 + metric definitions behind the combat-core engine swap (same deploy window; no coexistence).
2. Projection re-keys on v2 names; unknown-span tolerance covers any emission/consumption skew during the window.
3. Dashboard panels swap to the v2 metric ids; v1 Grafana panels are deleted, historical series left in storage.
4. Rollback = rolling back the engine swap itself (observability follows the core; it has no independent rollback).

## Open Questions

- None blocking. Target bands populate as balance studies run (R7); the OTLP backend choice intentionally waits for `boom2-delivery`.
