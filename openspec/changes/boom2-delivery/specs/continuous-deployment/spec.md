## ADDED Requirements

### Requirement: Deploy On Green Main

On a `main` commit that passes the full CI test gate, the pipeline SHALL **build and publish** the server container and the web-client (`clients/web/`) bundle, **run database migrations**, and **promote** the release (through staging to production).

#### Scenario: Green main triggers a deploy

- **WHEN** a commit lands on `main` and the full CI gate is green
- **THEN** the pipeline builds and publishes the server container and web bundle, runs migrations, and promotes the release

#### Scenario: Migrations run as part of the release

- **WHEN** a release includes schema changes
- **THEN** the pipeline runs the database migrations as part of promotion, before traffic is served by the new version

### Requirement: The Benchmarking Suite Rides The Pipeline

The pipeline SHALL run the `boom2-benchmarking` per-merge jobs — the seeded criterion benches, the bench-history append, and the dashboard republish — so performance is **tracked over time**. These jobs are observational: their results SHALL NOT gate the deploy.

#### Scenario: Benchmarks are tracked across releases

- **WHEN** a release is built
- **THEN** the seeded criterion benches execute, their results are appended to the bench history, and the dashboard is republished

#### Scenario: Bench results do not block promotion

- **WHEN** a release's bench results regress
- **THEN** promotion proceeds, and the regression is visible only on the bench dashboard
