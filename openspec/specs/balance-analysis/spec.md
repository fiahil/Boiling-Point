# balance-analysis Specification

## Purpose
TBD - created by archiving change bot-balance-harness. Update Purpose after archive.
## Requirements
### Requirement: Seeded Deterministic Batch Runner

The harness SHALL run many complete games in batch (targeting thousands) headlessly, driven by a configurable seed so that a run is fully reproducible from that seed.

#### Scenario: Same seed reproduces a run

- **WHEN** the batch runner is executed twice with the same seed, strategy assignment, and content config
- **THEN** both runs produce identical game outcomes and identical aggregated statistics

#### Scenario: A batch completes at scale

- **WHEN** a batch of thousands of games is requested
- **THEN** the runner plays them to completion without manual intervention and emits an aggregated report

### Requirement: Balance Statistics Aggregation

The harness SHALL aggregate balance statistics across a batch, including at least: explosion rate, win distribution by color and by strategy, average pot value, average cards per round, average waves per round, and modifier-draw frequency.

#### Scenario: Report includes the explosion rate

- **WHEN** a batch finishes
- **THEN** the report states the observed explosion rate so it can be compared against the ~30–40% target

#### Scenario: Win distribution is attributed

- **WHEN** a batch finishes with distinct strategies assigned
- **THEN** the report attributes wins per strategy and per color so imbalance is visible

### Requirement: Degenerate-Strategy and Balance-Smell Detection

The harness SHALL flag balance smells against configurable thresholds — a strategy or color winning disproportionately, an explosion rate far outside the target band, or runaway pot values — so the content-config knobs can be tuned.

#### Scenario: Dominant strategy is flagged

- **WHEN** one strategy wins far more than an even share across a large batch
- **THEN** the report flags it as a degenerate-strategy candidate with the supporting numbers

#### Scenario: Off-target explosion rate is flagged

- **WHEN** the observed explosion rate falls outside the configured target band
- **THEN** the report flags it and identifies the band it missed
