## ADDED Requirements

### Requirement: Single Source Of Metric Definitions

Every v2 balance metric SHALL be defined exactly once, in a public server-crate module (`server/src/observability/balance_metrics.rs`), as a named definition carrying its id, its formula over v2 span/engine events, its unit, and its target band; the live pipeline (Prometheus emitters and the admin projection) and the benchmarking suite's balance studies SHALL consume these definitions and SHALL NOT re-derive a formula independently.

#### Scenario: Live dashboard and balance study agree

- **WHEN** the same population of completed games is measured by the live pipeline and by a balance-study run
- **THEN** each shared metric id reports the same value from both consumers, because both evaluated the same definition

#### Scenario: A new metric is defined once

- **WHEN** a new balance metric is added to the definitions module
- **THEN** the dashboard and the balance-study reports can both surface it without either consumer implementing its formula

### Requirement: Core V2 Metric Set

The definitions SHALL cover at least the combat-core-derivable set: explosion (boom) rate, detonator distribution (who split −P, fatal-wave position), fold/pass timing and freeze (all-pass) rate, wave depth and duration, round and game duration, per-spell cast rate, and the carried-over fleet figures (active games, active groups, connected players, timeout rate, reconnection rate).

#### Scenario: Boom rate is defined over the v2 model

- **WHEN** the explosion-rate definition is evaluated over completed rounds
- **THEN** it counts detonator-only booms per the v2 resolution model, not the retired v1 everyone-loses explosion

#### Scenario: Fleet figures carry over

- **WHEN** the v2 definitions ship
- **THEN** active games, active groups, connected players, timeout rate, and reconnection rate remain available with unchanged meaning

### Requirement: Staged Per-Feature Metrics

Per-feature metrics SHALL extend the definitions additively as their content changes land — per-Brewer pick and win rates after `boom2-brewers`, bucket pick rates and deck-archetype outcomes after `boom2-apothecary`, compounding trigger rates after `boom2-compounding` — without renaming or redefining existing metric ids.

#### Scenario: Brewer metrics appear when Brewers land

- **WHEN** `boom2-brewers` is implemented and games with Brewer picks complete
- **THEN** per-Brewer pick-rate and win-rate definitions are available to both consumers, and all pre-existing metric ids are unchanged

### Requirement: Targets Are Playtest Hypotheses

Every balance target in the definitions SHALL be tagged `[needs playtesting]`, seeded from the decision log's starting numbers where one exists and absent otherwise; targets SHALL be updated from balance-study results, and a metric with no validated target SHALL render its observed value without a target band.

#### Scenario: An unvalidated target is visibly a hypothesis

- **WHEN** an operator views a metric whose target came from the decision log without study validation
- **THEN** the target is presented as a `[needs playtesting]` working value, not a validated band

#### Scenario: A study updates a target

- **WHEN** a balance study re-derives a metric's healthy band
- **THEN** the target changes in the definitions module, and both the dashboard and future study reports compare against the new band
