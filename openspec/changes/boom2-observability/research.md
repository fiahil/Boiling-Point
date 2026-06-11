# Research: boom2-observability

## Open Questions

- R1: Where do the shared metric definitions live, and in what form?
- R2: One span-schema bump or staged bumps as the content changes land?
- R3: What is the "admin command center" in spec terms?
- R4: Do v1 metrics survive a transition window?
- R5: Who owns the Brewer/Apothecary/compounding panels — this change or those changes?
- R6: Is `match-replays` in scope?
- R7: Where do the v2 metric targets come from?

## R1: Where do the shared metric definitions live?

**Decision:** A public module in the server crate — `server/src/observability/balance_metrics.rs` — exposing named metric definitions (id, formula over v2 span/engine events, unit, `[needs playtesting]` target). The live pipeline (Prometheus emitters + admin projection) calls it in-process; `boom2-benchmarking`'s balance-study runner, which already links the server engine to play games in-process, imports the same module.

**Rationale:** One definition, two populations was the point (proposal). The balance study already depends on the server crate, so a module is the zero-new-surface answer (Principle III).

**Alternatives Considered:**
- A separate `metrics-defs` workspace crate — clean, but a crate split with exactly two consumers in the same workspace is structure before need; the module's path is the seam if a third consumer appears.
- Independent definitions + a conformance test — guarantees drift gets caught, but accepts drift as a normal state; rejected.

**Key Details:** Definitions are data + pure functions (no I/O); both consumers feed them events, neither re-derives a formula. The bench-dashboard and the balance dashboard render the same metric ids.

## R2: One span-schema bump or staged bumps?

**Decision:** One bump, `SPAN_SCHEMA_VERSION` 1 → 2, shipping with the combat-core rebase. The v2 tree documents the pre-game subtree names up front (`brewer.pick`, `draft.*`); Brewers/Apothecary/compounding then add their spans and attributes **additively, without a bump**.

**Rationale:** The contract already says bump on *breaking* change only, and the projection ignores unknown spans/attributes — additive growth is the designed tolerant path. Staged bumps would force projection/dashboard releases in lockstep with every content change for no consumer benefit.

**Alternatives Considered:** A bump per content change — rejected as version churn that signals breakage where there is none.

**Key Details:** v2 is breaking because the v1 game subtree (`round`/`commit`/`score` shapes, `round.exploded`, `dominant_color`) disappears with the model. Reserved-but-unimplemented span names are listed in the v2 contract doc as *planned*, so the projection team (and the docs) see the whole tree once.

## R3: What is the "admin command center" in spec terms?

**Decision:** A small new capability, `admin-command-center`, holding the **surface** requirements: the admin UI is the single operator surface; every operator-facing observability surface (balance dashboard, room inspector, replays) is hosted inside it next to the control actions, behind admin auth; new observability surfaces MUST land inside it rather than as standalone tools.

**Rationale:** Direct user directive (2026-06-11): observability is part of the admin command center. No existing capability owns the surface — `admin-auth`/`admin-control`/`admin-span-projection` are mechanism specs, `balance-dashboard`/`room-inspector` are individual surfaces. A surface-level capability gives later changes (`boom2-delivery` ops panels, `boom2-identity` rating panels) a standing requirement to land in the same place.

**Alternatives Considered:**
- Fold the rule into `balance-dashboard` — too narrow; the inspector and replays are equally covered.
- Rename/restructure the `admin-*` specs around the command-center term — churn with no behavior change; the mechanisms are fine as named.

**Key Details:** Embedded Grafana stays the chart renderer *inside* the command center (existing `balance-dashboard` rule); the offline bench-dashboard (static HTML, `boom2-benchmarking`) MAY be linked from the command center but is not hosted by it — it has no server and no auth boundary of its own.

## R4: Do v1 metrics survive a transition window?

**Decision:** No. The v1 balance metrics (explosion rate vs ~30–40%, cards per round, dominant-color rate, reshuffle frequency) retire atomically with the v1 core. Fleet/ops metrics (active games/groups, connected players, timeout/reconnection rates, durations) carry over unchanged.

**Rationale:** `boom2-combat-core` is BREAKING with no v1/v2 coexistence ("v1/v2 cannot coexist as the core"), so there is no population for v1 metrics to describe after cutover. A deprecation window would chart zeros.

**Alternatives Considered:** Dual-emitting during a transition — rejected; there is no transition.

**Key Details:** Historical v1 Prometheus series remain in storage untouched; the dashboard simply stops querying them. Metric *names* are versioned by the v2 set's ids (R1), so v1/v2 series never collide.

## R5: Who owns the per-feature panels?

**Decision:** This change owns them, as phased task groups gated on `boom2-brewers`, `boom2-apothecary`, and `boom2-compounding` landing — mirroring `boom2-ai-client`'s "the matrix scales with what has landed" pattern.

**Rationale:** One home for the observability capability. The alternative spreads small deltas to `balance-dashboard`/`boom-balance-metrics` across three already-written changes, creating overlapping spec ownership for no benefit.

**Alternatives Considered:** Per-content-change deltas — rejected as above; revisit only if this change archives before the content changes land (then those panels become a small follow-up change).

**Key Details:** Phase gating is additive-only (per R2, no schema bumps): Brewer pick/win-rate panel after `boom2-brewers`; bucket pick-rate/archetype panel after `boom2-apothecary`; compounding trigger-rate panel after `boom2-compounding`.

## R6: Is `match-replays` in scope?

**Decision:** Yes, vocabulary only: the per-wave action log's enumerated inputs grow to the v2 set (ingredient commit with colored/colorless Vote choice, pass/fold, spell cast, emotes). Payload mechanics — root seed + ordered action log, engine/format version pinning, integrity hash — are untouched.

**Rationale:** The replay is part of the operator read surface this change owns (room-inspector's Per-Game Replay reads it), and `boom2-combat-core`'s impact list doesn't carry it.

**Alternatives Considered:** Leave it to combat-core — rejected; it would be the only observability-surface delta living outside this change.

**Key Details:** Replays recorded under engine v1 stay reconstructable under their pinned engine version (existing requirement); no migration.

## R7: Where do the v2 metric targets come from?

**Decision:** Every v2 balance target ships as `[needs playtesting]`, seeded from the starting numbers in the decision log (`docs/06_boom2/02_toward-a-v2-core.md`) where one exists, otherwise unset (the panel renders the observed value with no target band). Targets are updated from balance-study results, not hand-tuned in the dashboard.

**Rationale:** Constitution IV — numbers are hypotheses until playtested; the harness re-derives the balance economy from scratch.

**Alternatives Considered:** Carrying the v1 explosion-rate target over to boom rate — rejected; detonator-only boom is a different event with different economics.

**Key Details:** Targets live in the `boom-balance-metrics` definitions (R1), so the dashboard and the balance studies always compare against the same band.

## Summary

- **R1** — metric definitions are a public server module (`observability/balance_metrics.rs`); projection, Prometheus, and balance studies all import it.
- **R2** — single schema bump to v2 with the combat-core rebase; content changes extend additively, no further bumps.
- **R3** — new `admin-command-center` capability owns the single-operator-surface rule; mechanism specs untouched.
- **R4** — v1 balance metrics retire atomically with the v1 core; fleet/ops metrics carry over.
- **R5** — this change owns the per-feature panels as phases gated on the content changes.
- **R6** — `match-replays` gets a vocabulary-only delta (v2 action set).
- **R7** — all targets `[needs playtesting]`, seeded from the decision log, updated by balance studies.
