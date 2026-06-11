## ADDED Requirements

### Requirement: Hosted Target Is A Bare-Metal Single Server

The service SHALL run on a single bare-metal dedicated server (Scaleway Dedibox) with **no containers**: the single-binary monolith runs as a systemd service and PostgreSQL runs on the same host.

#### Scenario: The monolith runs as a systemd service

- **WHEN** the server is deployed
- **THEN** the single-binary monolith runs as a systemd service on the box, connected to the same-host PostgreSQL

#### Scenario: Secrets are injected at runtime

- **WHEN** the service starts
- **THEN** configuration and secrets come from a root-only environment file at runtime, never committed to the repository or baked into build artifacts

### Requirement: Caddy Is The Sole Public Ingress

**Caddy** SHALL be the only publicly exposed process, terminating TLS with automatic certificate management and reverse-proxying `/ws` (WebSocket upgrade) to the game server, which SHALL bind to localhost only.

#### Scenario: Players connect over wss through Caddy

- **WHEN** a client opens a WebSocket connection to the public origin
- **THEN** Caddy terminates TLS and proxies the upgraded connection to the localhost-bound game server

#### Scenario: The game server is not directly reachable

- **WHEN** a connection is attempted to the game server's port from outside the box
- **THEN** it is refused — only Caddy's ports (80/443) are publicly open

### Requirement: Caddy Serves All Static Content

Caddy SHALL serve the landing page and the web-client (`clients/web/`) bundle from disk via `file_server`; the game server SHALL serve no static assets and remain a pure `/ws` origin.

#### Scenario: Statics serve without the game server

- **WHEN** the game-server process is down
- **THEN** the landing page and client bundle still serve from Caddy

### Requirement: Database Backups Ship Off-Site

PostgreSQL SHALL be backed up nightly with the dumps shipped **off the box** to object storage, and a restore from an off-site dump SHALL be exercised rather than assumed.

#### Scenario: A backup survives the box

- **WHEN** the nightly backup completes
- **THEN** the dump exists in off-site object storage, independent of the server's disks

#### Scenario: Restores are verified

- **WHEN** the backup procedure is established, and periodically thereafter
- **THEN** a restore from an off-site dump is performed successfully against a scratch database

### Requirement: Admin Surface Is Not Publicly Exposed

The admin API (`:8081`) and operator dashboards (Grafana) SHALL remain bound to localhost, reached over an SSH tunnel or VPN, and SHALL NOT be routed through the public Caddy ingress.

#### Scenario: Operators reach admin via tunnel

- **WHEN** an operator needs the admin API or dashboard
- **THEN** they connect through an SSH tunnel/VPN to the localhost-bound ports; no public route exists

### Requirement: Staging Is The Developer's Localhost

There SHALL be no hosted staging environment: pre-deploy verification is the CI gate plus the full stack (server, PostgreSQL, client bundle) running on the developer's machine, and green `main` deploys directly to production.

#### Scenario: Local stack mirrors production software

- **WHEN** the stack is run locally for verification
- **THEN** it runs the same monolith, migrations, and client bundle that production receives

### Requirement: Single-Server Only

The deployment SHALL remain **single-server**; horizontal scaling is explicitly out of v2 scope. The single-server stance is the documented seam for later scaling.

#### Scenario: No multi-node coordination is introduced

- **WHEN** the deployment is provisioned
- **THEN** it runs a single server instance with no horizontal-scaling/coordination layer
