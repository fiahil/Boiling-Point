## 1. Suite scaffolding & governance

- [ ] 1.1 Create the `bench/` umbrella (`bench/balance-study/`, `bench/dashboard/`) as workspace members; add `server/benches/` with a placeholder seeded bench.
- [ ] 1.2 Amend the constitution (MINOR): add `bench/` + `server/benches/` to the project tree, note the suite's observational stance under §IV, log the amendment.
- [ ] 1.3 Update doc pointers: roadmap server-benchmarks entry and `docs/06_boom2/index.md` reference this change.

## 2. Server benchmarks (criterion)

- [ ] 2.1 Add `criterion` as a `server` dev-dependency with `[[bench]]` targets; configure noise discipline (noise_threshold ≥ 0.10, measurement_time 10s, warm_up 3s, fixed sample_size, `--noplot` in CI).
- [ ] 2.2 Write seeded hot-path benches as the v2 engine stabilizes (combat-core): deck realization, wave resolution, explosion resolution/depile, modifier stacking.
- [ ] 2.3 Add the per-`main`-merge CI job: run benches alone on a pinned runner type, extract estimates + confidence bounds, append one JSON record to the orphan `bench-data` branch.

## 3. Balance study (revived harness)

- [ ] 3.1 Revive `archive/bot-harness/` into `bench/balance-study/` — transport, seeded batch runner, stats — compiling against the v2 protocol (bot play on the new loop is combat-core 7.2; coordinate, don't duplicate).
- [ ] 3.2 Build the study runner: one command taking a study config (seed set, game count, knob values, matrix axes) and producing a run.
- [ ] 3.3 Define the versioned report schema: provenance (seeds, config hash, engine commit, game count) + metrics (explosion rate, detonator distribution, freeze rate, Peek-fire rate, persona × Brewer × deck-archetype cells).
- [ ] 3.4 Verify reproducibility: same provenance → identical metrics (make it a transport/integration test).
- [ ] 3.5 Document the on-demand workflow (Make target + how to read a report); confirm no CI step asserts on any metric.

## 4. Dashboard

- [ ] 4.1 Build the `bench/dashboard/` generator: read full `bench-data` history + study reports, emit a single self-contained `benches.html` (inline data/JS/CSS, zero external requests, renders from disk).
- [ ] 4.2 Render criterion trends with confidence/noise bands and sustained-level-shift highlighting; render study reports with provenance and matrix-cell outliers flagged.
- [ ] 4.3 Wire regeneration into the per-merge CI job (publish as artifact; switch to the delivery pipeline's static hosting when `boom2-delivery` lands) and into the local on-demand flow.
