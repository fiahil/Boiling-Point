## ADDED Requirements

### Requirement: Deploy On Green Main

On a `main` commit that passes the full CI test gate, the pipeline SHALL **build and publish** the server container and the web-client (`clients/web/`) bundle, **run database migrations**, and **promote** the release (through staging to production).

#### Scenario: Green main triggers a deploy

- **WHEN** a commit lands on `main` and the full CI gate is green
- **THEN** the pipeline builds and publishes the server container and web bundle, runs migrations, and promotes the release

#### Scenario: Migrations run as part of the release

- **WHEN** a release includes schema changes
- **THEN** the pipeline runs the database migrations as part of promotion, before traffic is served by the new version

### Requirement: Benchmark Regression Runs Land In The Pipeline

The seeded server-benchmark regression runs SHALL execute within this pipeline so performance is **tracked over time**.

#### Scenario: Benchmarks are tracked across releases

- **WHEN** a release is built
- **THEN** the seeded benchmark runs execute and their results are recorded for regression comparison across releases
