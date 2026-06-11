## ADDED Requirements

### Requirement: Engine Hot Paths Are Micro-Benchmarked Per Main Merge

The project SHALL maintain `criterion` micro-benchmarks in `server/benches/` covering the v2 engine hot paths — deck realization, wave resolution, explosion resolution/depile, and modifier stacking — and CI SHALL run them on every merge to `main`, appending the results (per-bench estimates with confidence bounds, commit, timestamp) to the bench history.

#### Scenario: A merge to main produces a bench record

- **WHEN** a commit merges to `main`
- **THEN** the bench job runs the criterion suite and appends one history record for that commit to the `bench-data` branch

#### Scenario: Hot-path coverage tracks the engine

- **WHEN** a new engine hot path lands (e.g. a new resolution phase)
- **THEN** a corresponding micro-benchmark is added in `server/benches/`

### Requirement: Benchmark Workloads Are Seeded And Deterministic

Every benchmark workload SHALL be constructed from fixed seeds (decks, waves, modifier stacks), so that run-to-run variance reflects only the environment, never the input.

#### Scenario: Identical input across reruns

- **WHEN** the same bench suite runs twice on the same commit
- **THEN** both runs measure byte-identical workloads (same realized decks, same waves)

### Requirement: Regressions Are Read As Trends, Not Single-Run Deltas

Because rerun variance on this project is observed at 6–12% wall-clock, criterion SHALL be configured with a noise threshold at or above 0.10 (plus extended measurement/warm-up windows and fixed sample sizes), and a performance regression SHALL be identified only as a sustained level shift across consecutive `main`-merge records in the dashboard trend — never from one run's delta.

#### Scenario: A single-run delta inside the noise floor

- **WHEN** one bench run differs from the previous record by less than the noise floor
- **THEN** no regression is reported

#### Scenario: A sustained level shift

- **WHEN** a bench's trend shows a level shift persisting across consecutive `main` merges, beyond the confidence bands
- **THEN** the dashboard surfaces it as a probable regression for human investigation

### Requirement: Server Benchmarks Never Gate

Benchmark results SHALL NOT block CI, merges, or deployments. (Benchmark *compilation* remains covered by the ordinary build gate.)

#### Scenario: A slow result does not block

- **WHEN** a `main` merge's bench results regress
- **THEN** the CI gate and any deploy promotion proceed unaffected, and the regression is visible only in the dashboard
