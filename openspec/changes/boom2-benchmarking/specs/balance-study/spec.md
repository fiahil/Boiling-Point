## ADDED Requirements

### Requirement: Balance Studies Run On Demand At Scale

The AI client's harness mode (`clients/ai` `balance_tester`, wrapped as `bench/balance-study/`) SHALL run balance studies **on demand** — thousands of seeded games per study — emitting the §IV metrics for the v2 core: explosion rate (vs the ~45% working target), detonator distribution, freeze (all-pass) rate, Peek-fire rate, and per-Brewer / per-persona / per-deck-archetype outcomes.

#### Scenario: A knob change triggers a study

- **WHEN** a balance knob changes (e.g. the boiling-point range) and a study is requested
- **THEN** a single command runs the configured at-scale study and produces a report, with no CI involvement required

### Requirement: Balance Studies Are Purely Observational

Balance-study metrics SHALL NOT gate CI, merges, or deployments — no metric-band assertions anywhere in automation. The only CI-adjacent use of the revived harness is `boom2-delivery`'s gate **smoke**, which asserts completion and determinism only. Off-target metrics are highlighted in the report for human decision (Principle IV).

#### Scenario: An off-target explosion rate stays green

- **WHEN** a study measures an explosion rate far from the ~45% target
- **THEN** the report flags it prominently, and CI/CD remain green

### Requirement: Study Reports Are Versioned And Reproducible

Every study SHALL emit a versioned report recording its full provenance — seed set, game count, balance-config hash, engine commit — alongside its metrics, such that rerunning with the same provenance reproduces the same metrics.

#### Scenario: A rerun reproduces a report

- **WHEN** a study is rerun with the same seeds, config hash, and engine commit
- **THEN** the emitted metrics are identical to the original report's

### Requirement: Studies Cover The Strategy Matrix

A full study SHALL sweep the persona × Brewer × deck-archetype matrix (× explosion-model while that question is open), reporting per-cell outcomes so degenerate strategies surface before human playtesting (Principle IV).

#### Scenario: A degenerate cell surfaces

- **WHEN** one matrix cell (e.g. one Brewer under one persona) wins far outside the expected band across the study
- **THEN** the report identifies the cell and its win-rate so designers can investigate
