## ADDED Requirements

### Requirement: CI Runs The Full Test Gate

CI SHALL extend beyond `fmt` + `clippy` + unit tests to run the Principle-II testing layers (constitution v2.0.0): **transport/integration** tests (booting an in-process server), and — once the Pixi client lands — the **web client** (`clients/web/`) build and Playwright visual suite. When the archived bot harness (`archive/bot-harness/`) is revived for boom2 balance work (required before boom2 balance ships, §IV), its seeded deterministic balance runs SHALL join the gate.

#### Scenario: A protocol/transport regression fails CI

- **WHEN** a change breaks the transport/integration layer
- **THEN** the CI run fails before any deploy step

#### Scenario: Revived balance runs are seeded and deterministic

- **WHEN** the revived bot-harness balance layer runs in CI
- **THEN** it uses fixed seeds so results are reproducible and a regression is attributable

### Requirement: The Test Gate Precedes Deployment

A deployment SHALL proceed only after the full CI test gate passes on `main`. A red gate SHALL block promotion.

#### Scenario: Red gate blocks deploy

- **WHEN** any CI test layer fails on a `main` commit
- **THEN** the continuous-deployment pipeline does not build, publish, or promote that commit
