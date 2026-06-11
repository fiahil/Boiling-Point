# boom-balance-harness

Harness mode: the Principle IV reinstatement — seeded batch runs over an in-process server, the persona × Brewer × deck-archetype matrix, and diffable balance reports.

## ADDED Requirements

### Requirement: Seeded Reproducible Batch Runs

The harness SHALL run batches of complete v2 games from a root seed through a deterministic RNG tree (root → per-game → per-seat). With bot brains only, the same seed SHALL produce identical outcomes; different seeds SHALL diverge. Runs including an agent-brain seat SHALL be marked non-reproducible in the report.

#### Scenario: Reproducibility

- **WHEN** the same bot-brain-only batch is run twice with the same root seed
- **THEN** every game's outcome is identical between the runs

#### Scenario: Agent seats void the guarantee visibly

- **WHEN** a batch includes an agent-brain seat
- **THEN** the report marks the run non-reproducible

### Requirement: Matrix Sample Specification

The harness SHALL accept a matrix sample spec — which persona × Brewer × deck-archetype cells to run and how many games per cell — rather than only full factorial runs. Brewer assignment and deck-archetype scripting SHALL be controllable per seat.

#### Scenario: Targeted cell run

- **WHEN** a sample spec requests 500 games of (aggressive persona, Cinderwright, Nightshade-heavy archetype) against three fixed opponents
- **THEN** the harness runs exactly those games with the specified seat configurations

### Requirement: Batch Runs Use The In-Process Transport By Default

Batch runs SHALL default to the in-process frame-channel transport for throughput and reproducibility, with a WebSocket option retained for transport-parity validation. A thousands-of-games bot-brain batch SHALL complete unattended in a single invocation.

#### Scenario: Unattended at scale

- **WHEN** a 1000-game bot-brain batch is invoked
- **THEN** it runs to completion without manual intervention and emits its report

### Requirement: Balance Reports

The harness SHALL emit a human-readable report and a machine-readable JSON report, diffable across config versions. Reports SHALL include at minimum: explosion rate, detonator distribution, per-Brewer and per-deck-archetype win rates, spell fire rates (including the Peek economy), fold-to-safety rates, freeze (all-pass) frequency, game length, and per-seat fallback rates.

#### Scenario: Diffable across tunings

- **WHEN** the same seeded sample spec is run against two content configs
- **THEN** the two JSON reports are directly comparable metric-for-metric

### Requirement: Degenerate Strategy Detection

The harness SHALL support including the uniform-random baseline archetype in any matrix sample so that archetypes statistically indistinguishable from random — or single cells that dominate all others — are detectable from the report.

#### Scenario: Dominant cell surfaces

- **WHEN** one persona × Brewer × archetype cell wins disproportionately across a sample
- **THEN** the report exposes the per-cell win rates that make the dominance visible

### Requirement: Agent Brains In Batches Are Opt-In

Batch runs SHALL default to bot brains; including an agent-brain seat in a batch SHALL require an explicit flag and respects the agent brain's spend caps.

#### Scenario: No accidental spend

- **WHEN** a batch is invoked without the explicit agent flag
- **THEN** every seat runs a bot brain and no Claude calls are made
