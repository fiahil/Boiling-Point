## ADDED Requirements

### Requirement: Hosted Target Is A Managed Container Plus Managed Postgres

The service SHALL run on a **managed container host** with **managed Postgres**, with the single-binary monolith deployed as **one container**. The architecture SHALL define TLS/WebSocket ingress, configuration/secrets handling, and database backups.

#### Scenario: The monolith deploys as one container

- **WHEN** the server is deployed
- **THEN** the single-binary monolith runs as one managed container connected to managed Postgres, reachable over TLS/WebSocket

#### Scenario: Secrets are not baked into the image

- **WHEN** the container is built and run
- **THEN** configuration and secrets are injected at runtime, not embedded in the image

### Requirement: Staging Precedes Production

There SHALL be a **staging** environment that mirrors production, and changes SHALL pass through staging before promotion to production.

#### Scenario: Promotion goes through staging

- **WHEN** a green `main` build is deployed
- **THEN** it is released to staging first and promoted to production only after staging verification

### Requirement: Single-Server Only

The deployment SHALL remain **single-server**; horizontal scaling is explicitly out of v2 scope. The single-server stance is the documented seam for later scaling.

#### Scenario: No multi-node coordination is introduced

- **WHEN** the deployment is provisioned
- **THEN** it runs a single server instance with no horizontal-scaling/coordination layer
