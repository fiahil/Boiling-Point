## Why

Two measurement workloads currently have no home: the roadmap's **server benchmarks** candidate ([05_roadmap.md — Other Post-V1 Candidates](../../../docs/05_roadmap.md)) and the at-scale **balance runs** that constitution §IV (v2.0.0) requires before boom2 balance ships. Both are the same *kind* of thing — **measurements tracked against targets over time**, not pass/fail tests — and they share machinery (seeded runs, history, reporting). This change gives them one umbrella: a **benchmarking suite** with two instruments and a shared dashboard, the instrument `boom2-combat-core` §8 needs to re-derive the balance economy.

## What Changes

- **Benchmarking suite (umbrella)** — a `bench/` workspace area plus `server/benches/`, with shared conventions: seeded deterministic workloads, per-run history, one report surface. Benchmarks **measure**; tests **gate** — nothing in this suite ever blocks CI or deploys.
- **Server benchmarks** — `criterion` micro-benchmarks over the v2 engine hot paths (deck realization, wave resolution, explosion resolution/depile, modifier stacking), run **on every merge to `main`**, results appended to a history and read as **trends** (this project observes 6–12% wall-clock variance between criterion reruns, so single-run deltas are noise — see research R5).
- **Balance study** — the revived bot harness (`archive/bot-harness/` → `bench/balance-study/`, revival shared with `boom2-combat-core` task 7.2) run **on demand** at scale (thousands of seeded games), emitting versioned reports: explosion rate vs the ~45% target, detonator distribution, freeze rate, and the persona × Brewer × deck-archetype matrix. **Purely observational** — reports inform humans (Principle IV); they never gate.
- **Bench dashboard** — a **single self-contained HTML page** aggregating both instruments: criterion trends with noise bands plus balance-study reports. Renders offline from one file; regenerated per `main` merge and on demand.
- **Deferred: WebSocket load harness** — the roadmap's second server-benchmark layer (concurrent rooms over the real wire) stays out of scope; the report/dashboard conventions are its seam.
- **Constitution amendment (MINOR)** — add `bench/` to the project-structure tree and note the suite under §IV.
- **Relationship to `boom2-delivery`** — its pipeline *runs* the per-merge criterion job and republishes the dashboard; its CI **gate** keeps only a thin deterministic bot-harness **smoke** (completes, reproducible — no balance-metric assertions).

## Capabilities

### New Capabilities

- `server-benchmarks` — criterion engine micro-benchmarks, per-`main`-merge, seeded, trend-tracked, never gating.
- `balance-study` — on-demand at-scale bot-harness studies with versioned reproducible reports; purely observational.
- `bench-dashboard` — the single self-contained HTML page aggregating all bench history and study reports.

### Modified Capabilities

<!-- None in openspec/specs/. Coherence edits to the in-flight boom2-delivery
     draft (gate smoke vs. observational study) are made directly in that change. -->

## Impact

- **Workspace:** new `server/benches/` (criterion) and `bench/` (`balance-study/`, `dashboard/`); `archive/bot-harness/` is revived, not rewritten.
- **CI:** one new per-merge job on `main` (bench + history append + dashboard regen) on top of the existing gate; history lives on a dedicated `bench-data` branch. The job rides `boom2-delivery`'s pipeline once that lands; until then it's a plain workflow.
- **Dependencies:** `criterion` as a dev-dependency of `server`. Balance study depends on `boom2-combat-core`'s engine (benching the v1 loop is throwaway work — see design Risks).
- **Docs/governance:** CLAUDE.md project tree (MINOR amendment), roadmap pointers, `docs/06_boom2/index.md` cross-reference.
- **No** game-logic, protocol, or balance-number change — this change builds the measuring instruments, not the values.
