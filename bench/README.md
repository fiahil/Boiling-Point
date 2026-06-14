# Benchmarking suite

Change [`boom2-benchmarking`](../openspec/changes/boom2-benchmarking/). Two
measurement instruments and one reading surface, all **observational**:

> **Benchmarks measure; tests gate.** Nothing in this suite ever blocks CI, a
> merge, or a deploy (decision D1). Off-target numbers are *flagged for a human*
> (Principle IV) — never turned into a build verdict. The only CI-adjacent use of
> the harness is `boom2-delivery`'s gate **smoke** (completes + deterministic; no
> metric-band assertions).

| Instrument | Where | When | Reads as |
|---|---|---|---|
| **Server benchmarks** (criterion) | [`server/benches/engine.rs`](../server/benches/engine.rs) | per merge to `main` (+ on demand) | **trends** over the bench history |
| **Balance study** (AI-client harness) | [`balance-study/`](balance-study/) | **on demand** (question-driven) | a versioned, reproducible report |
| **Dashboard** | [`dashboard/`](dashboard/) | regenerated per merge + on demand | one self-contained `benches.html` |

## Why trends, not single runs

This project observes **6–12% wall-clock variance between criterion reruns**
(research R5). So a single run's delta is *noise*. The criterion suite is
configured with a `noise_threshold` of 0.10, extended measurement/warm-up
windows, and a fixed sample size; the bench job runs **alone on a pinned runner
type**. A regression is a **sustained level shift across consecutive `main`
merges** in the dashboard, beyond the confidence bands — the dashboard draws
those bands so the wobble stays visually distinct from a real shift, and flags a
probable sustained shift for you to investigate.

## On-demand local flow

```sh
make bench            # 1. run the criterion engine suite
make bench-study STUDY=bench/balance-study/studies/explosion-band.toml   # 2. (optional) a study
make bench-dashboard  # 3. collect the run + render the page
open target/bench/benches.html   # opens fully offline, zero network
```

- `make bench-study` with no `STUDY=` runs a quick all-bot baseline (override
  `GAMES=` / `SEED=`). Reports land in `target/bench/studies/` as `*.json` (for the
  dashboard) and `*.md` (to read directly).
- `make bench-dashboard` collects the latest `target/criterion` run into
  `target/bench/history/` and renders `target/bench/benches.html` from that history
  plus every study report — the same page CI publishes.

## Running a balance study

A study is **question-driven**: a knob changed, or a degenerate strategy is
suspected. Write a TOML config (see
[`balance-study/studies/explosion-band.toml`](balance-study/studies/explosion-band.toml))
naming the seed set, the game count, the content config under test (the *knob
values*), and the persona × Brewer × deck-archetype matrix cells, then:

```sh
cargo run -p boiling-point-balance-study --bin balance_study -- \
  --study bench/balance-study/studies/explosion-band.toml \
  --out target/bench/studies/explosion-band
```

All-bot studies make **zero Claude calls** and are reproducible by construction;
agent seats need `--allow-agents` (real spend, voids reproducibility).

### How to read a report

A report (`<name>.md`, or a card on the dashboard) has three parts:

1. **Provenance** — root seed, game count, content-config fingerprint, and engine
   commit. *Same provenance ⇒ identical metrics*, so any report re-runs.
2. **§IV metrics** — the **shared** [`balance_metrics`](../server/src/observability/balance_metrics.rs)
   definitions (boom rate vs the ~45% target, freeze rate, detonators per boom,
   fold rate, spell casts, …) folded over the bot games. One definition, two
   populations: the live dashboard and the study evaluate the *same* formulas, so
   "does live play match the harness?" is a direct comparison. Wall-clock metrics
   (round/wave/game seconds) show **no data** here — the harness resolves games
   instantly; durations belong to the live pipeline.
3. **Flags & matrix** — off-target rates and degenerate persona × Brewer ×
   deck-archetype cells (a per-seat win rate far from the 25% baseline), surfaced
   for a human to investigate. A flag never fails anything.

## CI

The [`bench` workflow](../.github/workflows/bench.yml) runs **after** a merge to
`main` (not on PRs — it is not a required check). It runs the suite, appends one
JSON record to the orphan **`bench-data`** branch, re-renders the dashboard, and
uploads it as an artifact. **No CI step asserts on any metric** — the ordinary
`ci.yml` gate (`fmt` / `clippy` / unit tests) is untouched, and this workflow only
records and publishes numbers.
