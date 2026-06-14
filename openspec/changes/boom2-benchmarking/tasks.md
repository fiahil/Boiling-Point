## 1. Suite scaffolding & governance

- [x] 1.1 Create the `bench/` umbrella (`bench/balance-study/`, `bench/dashboard/`) as workspace members; add `server/benches/` with a placeholder seeded bench.
- [x] 1.2 Amend the constitution (MINOR): add `bench/` + `server/benches/` to the project tree, note the suite's observational stance under §IV, log the amendment.
- [x] 1.3 Update doc pointers: roadmap server-benchmarks entry and `docs/06_boom2/index.md` reference this change.

## 2. Server benchmarks (criterion)

- [x] 2.1 Add `criterion` as a `server` dev-dependency with `[[bench]]` targets; configure noise discipline (noise_threshold ≥ 0.10, measurement_time 10s, warm_up 3s, fixed sample_size, `--noplot` in CI).
- [x] 2.2 Write seeded hot-path benches as the v2 engine stabilizes (combat-core): deck realization, wave resolution, explosion resolution/depile, modifier stacking.
- [x] 2.3 Add the per-`main`-merge CI job: run benches alone on a pinned runner type, extract estimates + confidence bounds, append one JSON record to the orphan `bench-data` branch.

## 3. Balance study (revived harness)

- [x] 3.1 Build `bench/balance-study/` as a thin study wrapper over the AI client's harness mode (`clients/ai` `balance_tester`, change `boom2-ai-client` — transports, seeded runner, stats, and reports already live there): study configs in, versioned reports out; don't duplicate the runner.
- [x] 3.2 Build the study runner: one command taking a study config (seed set, game count, knob values, matrix axes) and producing a run.
- [x] 3.3 Define the versioned report schema: provenance (seeds, config hash, engine commit, game count) + metrics (explosion rate, detonator distribution, freeze rate, Peek-fire rate, persona × Brewer × deck-archetype cells).
- [x] 3.4 Verify reproducibility: same provenance → identical metrics (make it a transport/integration test).
- [x] 3.5 Document the on-demand workflow (Make target + how to read a report); confirm no CI step asserts on any metric.

## 4. Dashboard

- [x] 4.1 Build the `bench/dashboard/` generator: read full `bench-data` history + study reports, emit a single self-contained `benches.html` (inline data/JS/CSS, zero external requests, renders from disk).
- [x] 4.2 Render criterion trends with confidence/noise bands and sustained-level-shift highlighting; render study reports with provenance and matrix-cell outliers flagged.
- [x] 4.3 Wire regeneration into the per-merge CI job (publish as artifact; switch to the delivery pipeline's static hosting when `boom2-delivery` lands) and into the local on-demand flow.
