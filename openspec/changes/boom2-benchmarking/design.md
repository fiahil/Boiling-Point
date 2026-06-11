## Context

Constitution v2.0.0 retired the v1 harnesses to `archive/` and made bot-harness revival the precondition for shipping boom2 balance (§IV). Meanwhile the roadmap's server-benchmark candidate has criterion micro-benches "tracked over time" with nowhere to land. Both workloads are measurements against targets — performance budgets, balance targets — which makes them one suite with shared conventions, distinct from the pass/fail CI gate that `boom2-delivery` owns. Decisions below were resolved in [research.md](research.md) (R1–R5).

## Goals / Non-Goals

**Goals**
- One benchmarking suite: criterion server benches (per `main` merge) + on-demand balance studies, sharing seeding, history, and a single self-contained HTML dashboard.
- A noise discipline that survives the observed 6–12% criterion rerun variance.
- The instrument `boom2-combat-core` §8 needs for re-deriving the balance economy.

**Non-Goals**
- Gating anything on benchmark output (performance or balance) — ever, in this change.
- The WebSocket load harness (roadmap layer 2) — deferred; the report/dashboard conventions are its seam.
- The harness's v2 gameplay adaptation itself (bots playing the new loop) — that is `boom2-combat-core` 7.2; this change owns where it lives and how its output is run, recorded, and read.
- Live-service observability — `ops/grafana` covers runtime telemetry; this suite measures *code at commits*.

## Decisions

### D1: Benchmarks measure, tests gate

The suite produces numbers read by humans; the CI gate stays pass/fail and owned by `boom2-delivery`. The single overlap is the revived harness's **smoke** in the gate (completes + deterministic — a correctness property), with no metric bands. *Alternative rejected:* band-gating balance metrics (e.g. explosion rate 40–50%) — during the boom2 re-derivation the numbers swing by design, and even post-validation Principle IV makes data an input to human balance decisions, not a build verdict.

### D2: Criterion per `main` merge, trends as the regression signal (R2, R5)

Workloads fully seeded; criterion configured with `noise_threshold ≥ 0.10`, extended `measurement_time`/`warm_up_time`, fixed `sample_size`, `--noplot`; the bench job runs alone on a pinned runner type. A regression is a **sustained level shift across consecutive merges** in the dashboard, beyond the confidence bands — never a single-run delta (observed rerun noise: 6–12%). *Alternative rejected:* alerting on criterion's single-run change detection — guaranteed false positives at this noise floor. *Deferred:* `iai-callgrind` instruction counts — near-zero noise but Valgrind/Linux-only; revisit if wall-clock trends stay unreadable.

### D3: Balance studies on demand, observational, reproducible (R2, R4)

Studies are question-driven (a knob changed; a degenerate strategy is suspected), run by one command, at thousands of seeded games, emitting versioned reports with full provenance (seeds, config hash, engine commit). *Alternative rejected:* scheduled nightly studies — burns compute with no question attached; on-demand keeps every run attributable to a hypothesis (Principle IV).

Dependency (`boom2-observability`): the balance-study runner imports the shared metric definitions from `server/src/observability/balance_metrics.rs` and evaluates them over its bot games — one definition, two populations; study reports never re-derive a formula.

### D4: One self-contained HTML page, generator-owned (R4)

`bench/dashboard/` reads the bench history + study reports and emits a single `benches.html` with inline JSON data and inline rendering (hand-rolled SVG or an inlined micro-lib — no CDN, no external requests; must open from disk). Agent-writable, diffable, hosting-free (Principles II/III). *Alternative rejected:* grafana — that's live-service telemetry in `ops/`; bench history is per-commit and belongs with the repo, not a TSDB.

### D5: History on a dedicated `bench-data` branch

The per-merge job appends one JSON record (commit, timestamp, per-bench estimates with confidence bounds; study reports when present) to an orphan `bench-data` branch; the dashboard regenerates from the full history. *Alternatives rejected:* CI artifacts only — no cross-run trends, and trends are the noise-killer (D2); committing history to `main` — a noise commit per merge.

### D6: Layout — `server/benches/` + `bench/` umbrella (R3)

Criterion benches stay cargo-idiomatic in `server/benches/`. The suite's code lives under `bench/`: `balance-study/` (the revived `archive/bot-harness/`) and `dashboard/` (the generator). Constitution project tree amended (MINOR). *Alternatives rejected:* reviving the harness to its old top-level spot — it's now one instrument of a suite; `ops/` — config/observability space, not workspace code.

## Constitution Check

| Principle | Compliance |
|---|---|
| **I — Server-authoritative** | The suite measures the server engine and drives it over the real protocol; no game logic moves anywhere. |
| **II — Agent-driven** | Benches, study configs, reports, history, and the dashboard generator are all plain source/JSON/HTML — fully agent-writable; the dashboard closes a measure-read-adjust loop. |
| **III — Start simple** | Two instruments + one static page; load harness deferred; history is a JSON-per-merge branch, not a metrics service. **Rejected simpler alternative:** no suite — but §IV (v2.0.0) requires at-scale runs before boom2 balance ships, and untracked benches can't beat a 6–12% noise floor. |
| **IV — Playtest-driven** | This change *is* the §IV instrument: seeded at-scale studies, matrix sweeps to surface degenerate strategies, data informing — never auto-deciding — balance. |

## Risks / Trade-offs

- **[Sequencing]** Benching the v1 engine is throwaway — `boom2-combat-core` replaces it. → CI plumbing, history branch, and dashboard land first with a placeholder bench; the hot-path suite fills in as the v2 engine stabilizes; the balance study activates with combat-core 7.2.
- **[Runner variance]** Shared CI runners may exceed the local 6–12% noise floor. → Bands are computed from observed dispersion in the history itself; if trends stay unreadable, escalate to the deferred iai-callgrind path (D2).
- **[History growth]** One record per merge grows unboundedly. → Records are small JSON; if it ever matters, the generator can window/aggregate old history — a seam, not a now-problem.
- **[Two-change coupling]** Harness revival is shared with `boom2-combat-core` 7.2. → This change owns location + runner + report format; combat-core owns bot behavior on the new loop; the boundary is written into both task lists.

## Open Questions

- Exact criterion sample/measurement settings per bench (set empirically once the first trends exist; start `sample_size=100`, `measurement_time=10s`, `warm_up=3s`).
- Whether the dashboard publishes via the delivery pipeline's static hosting or stays artifact-only until traffic warrants it (decide in `boom2-delivery` 3.x).
