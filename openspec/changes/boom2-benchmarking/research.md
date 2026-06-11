## Open Questions

Resolved in the 2026-06-11 explore session that produced this change.

- **R1** — Where does the benchmarking suite live in OpenSpec: inside `boom2-delivery` or as its own change?
- **R2** — What cadence does each instrument run at?
- **R3** — Where does the suite live in the (reshaped, v2.0.0) workspace tree?
- **R4** — Does the balance study ever gate CI, and what is the reporting surface?
- **R5** — How do we get readable signal out of criterion given observed rerun noise?

## R1 — A dedicated change, not a delivery sub-scope

**Decision:** New change `boom2-benchmarking`, separate from `boom2-delivery`.

**Rationale:** Delivery owns *plumbing* (gate, hosting, CD); benchmarking owns *instruments* (what is measured, how often, how it's read). Mixing them would bloat delivery's scope and bury the tests-gate/benchmarks-measure distinction. Delivery's pipeline remains the *runner* seam for the per-merge job.

**Alternatives considered:** (a) reshape `boom2-delivery` with a `benchmarking-suite` capability — rejected, conflates gate and measurement concerns; (b) leave the balance-study methodology inside `boom2-combat-core` — rejected, the suite outlives the rework and serves later balance passes too.

## R2 — Criterion per `main` merge; balance studies on demand

**Decision:** Server micro-benchmarks run on every merge to `main` (cheap, trend-feed). Balance studies run **on demand** — when a knob changes or a degenerate strategy is suspected — because thousands-of-games sweeps are too heavy for per-merge.

**Alternatives considered:** per-PR benches (CI cost + noisy runners for no gating benefit — rejected); nightly balance studies (scheduled burn with nothing to ask of the data most nights — rejected; on-demand keeps runs question-driven, Principle IV).

## R3 — `server/benches/` + a `bench/` umbrella

**Decision:** Criterion benches live in `server/benches/` (cargo-idiomatic). The suite's own code lives under a new top-level `bench/`: `bench/balance-study/` (the revived `archive/bot-harness/`, adapted to the v2 core) and `bench/dashboard/` (the report generator). Constitution tree gets a MINOR amendment.

**Rationale:** The pre-reshape layout (top-level `bot-harness/` crate) is gone; `archive/` is explicitly revivable and §IV names revival as the expected path. `ops/` is runtime/observability config (grafana), not workspace code — wrong home.

**Alternatives considered:** revive the harness to its old top-level spot — rejected, it's now one instrument of a suite, the tree should say so; put tooling in `ops/` — rejected, scatters workspace code into config space.

## R4 — Purely observational, surfaced on one self-contained HTML page

**Decision:** No benchmark — performance or balance — ever gates CI/CD. Balance-study metrics (explosion rate vs ~45%, detonator distribution, freeze rate, matrix cells) are highlighted in reports for humans to act on. The single CI-gate touchpoint is `boom2-delivery`'s **smoke**: the revived harness must *complete deterministically* (a correctness property), with no metric-band assertions. All bench output lands on **one self-contained HTML dashboard** (inline data + inline rendering, opens from disk, zero external requests).

**Rationale:** During the boom2 re-derivation the numbers swing *by design* — banding them would make CI flap on intent. Even post-validation, Principle IV says data **informs** balance changes; the gate's job is correctness only. Self-contained HTML keeps the surface agent-writable, diffable, and hosting-free (Principles II/III).

**Alternatives considered:** band-gating explosion rate post-validation (rejected — turns a design dial into a build failure); grafana for bench display (rejected — `ops/grafana` is *live-service* telemetry; bench history is per-commit and belongs with the repo, not a TSDB).

## R5 — Noise discipline: trends over single-run deltas

**Decision:** Observed rerun variance on this project is **6–12%** wall-clock for unchanged code. Therefore: (a) bench workloads are fully seeded/deterministic so variance is environmental only; (b) criterion is configured with `noise_threshold` ≥ 0.10, longer `measurement_time`/`warm_up_time`, fixed `sample_size`, `--noplot` in CI; (c) the **regression signal is a sustained level shift across consecutive `main` merges in the dashboard trend (with confidence bands)** — never a single-run delta; (d) the bench job runs alone (no parallel jobs on the runner) on a pinned runner type.

**Alternatives considered:** gate or alert on criterion's own single-run change detection — rejected, the 6–12% floor guarantees false positives; instruction-count benchmarking (iai-callgrind) — **deferred**, near-zero noise but Valgrind/Linux-only and measures instructions not wall time; revisit if wall-clock trends stay unreadable.

## Summary

- **R1:** dedicated change `boom2-benchmarking`; delivery's pipeline is just the runner.
- **R2:** criterion per `main` merge; balance studies on demand.
- **R3:** `server/benches/` + `bench/{balance-study,dashboard}/`; constitution MINOR amendment.
- **R4:** everything observational; one self-contained HTML dashboard; the only gate item is delivery's determinism smoke.
- **R5:** seeded workloads, raised noise threshold, regression = sustained trend shift; iai-callgrind deferred.
