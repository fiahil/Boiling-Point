## ADDED Requirements

### Requirement: Deploy On Green Main

On a `main` commit that passes the full CI test gate, the pipeline SHALL **build** the release server binary and the web-client (`clients/web/`) bundle, **sync** them to the production box, **run database migrations**, and **restart** the service.

#### Scenario: Green main triggers a deploy

- **WHEN** a commit lands on `main` and the full CI gate is green
- **THEN** the pipeline builds the server binary and web bundle, syncs them to the box, runs migrations, and restarts the service

#### Scenario: Migrations run as part of the release

- **WHEN** a release includes schema changes
- **THEN** the pipeline runs the database migrations before the new version serves traffic

### Requirement: The Benchmarking Suite Rides The Pipeline

The pipeline SHALL run the `boom2-benchmarking` per-merge jobs — the seeded criterion benches, the bench-history append, and the dashboard republish — so performance is **tracked over time**. These jobs are observational: their results SHALL NOT gate the deploy.

#### Scenario: Benchmarks are tracked across releases

- **WHEN** a release is built
- **THEN** the seeded criterion benches execute, their results are appended to the bench history, and the dashboard is republished

#### Scenario: Bench results do not block promotion

- **WHEN** a release's bench results regress
- **THEN** promotion proceeds, and the regression is visible only on the bench dashboard
