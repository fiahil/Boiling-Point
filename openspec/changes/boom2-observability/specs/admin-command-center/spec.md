## ADDED Requirements

### Requirement: Single Operator Surface

The admin UI SHALL be the single operator surface — the **admin command center** — hosting every operator-facing observability surface (the balance dashboard, the room inspector, per-game replays) alongside the control actions, all behind admin authentication.

#### Scenario: Observability and control share one surface

- **WHEN** an authenticated operator opens the command center
- **THEN** the balance dashboard, room inspector, replays, and the control actions are all reachable from that one surface without visiting a separate tool

#### Scenario: The surface is gated by admin auth

- **WHEN** an unauthenticated request targets any command-center page
- **THEN** it is denied; no observability or control surface is publicly reachable

### Requirement: New Observability Surfaces Land In The Command Center

Any new operator-facing observability surface SHALL be hosted inside the command center as an embedded view; external visualization tools appear only as embedded renderers behind the command center's auth, never as separate operator destinations, and serverless artifacts (e.g. the offline bench dashboard) are linked from the command center rather than hosted by it.

#### Scenario: A v2 panel lands inside the command center

- **WHEN** a new balance panel (e.g. per-Brewer win rates) ships
- **THEN** it is reachable from the command center's navigation as an embedded view, not as a standalone dashboard URL operators must bookmark separately

#### Scenario: The bench dashboard is linked, not hosted

- **WHEN** the benchmarking suite's static HTML dashboard is surfaced to operators
- **THEN** the command center links out to the static artifact rather than embedding or serving it

### Requirement: Reads And Control Stay Separate Channels Within One Surface

Hosting observability and control together SHALL NOT merge their channels: every read is served from the span projection and every control action goes through the separate audited command API, whose effects re-appear in the span stream the command center reads.

#### Scenario: A control action from the command center round-trips through telemetry

- **WHEN** an operator triggers a control action from the command center
- **THEN** it executes via the audited command API and its effect re-appears as spans, so the command center confirms it through the same read path as everything else
