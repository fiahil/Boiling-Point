## ADDED Requirements

### Requirement: CI Runs The Three Testing Layers

CI SHALL extend beyond `fmt` + `clippy` + unit tests to run the project's three Principle-II testing layers: **transport/integration** tests (booting an in-process server), seeded deterministic **bot-harness** balance runs, and an **agent-harness** Claude-as-player smoke. Once the Pixi client lands, CI SHALL also run the **web-client** build and Playwright visual suite.

#### Scenario: A protocol/transport regression fails CI

- **WHEN** a change breaks the transport/integration layer
- **THEN** the CI run fails before any deploy step

#### Scenario: Balance runs are seeded and deterministic

- **WHEN** the bot-harness balance layer runs in CI
- **THEN** it uses fixed seeds so results are reproducible and a regression is attributable

### Requirement: The Test Gate Precedes Deployment

A deployment SHALL proceed only after the full CI test gate passes on `main`. A red gate SHALL block promotion.

#### Scenario: Red gate blocks deploy

- **WHEN** any CI test layer fails on a `main` commit
- **THEN** the continuous-deployment pipeline does not build, publish, or promote that commit
