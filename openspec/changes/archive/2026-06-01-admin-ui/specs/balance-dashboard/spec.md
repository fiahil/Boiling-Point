## ADDED Requirements

### Requirement: Game-Balance Metrics Surface

The balance dashboard SHALL surface the Principle IV metrics over a selectable
time window: at least explosion rate, round and game durations, cards per round,
dominant-color (dominant-strategy) rate, turn/wave timeout rate, reconnection
rate, and reshuffle frequency.

#### Scenario: Explosion rate is observable against its target

- **WHEN** an operator views the balance dashboard over a chosen window
- **THEN** the explosion-rate figure is shown so it can be compared against the
  ~30–40% design target

#### Scenario: Core balance metrics are present

- **WHEN** the dashboard renders
- **THEN** it includes explosion rate, round/game duration, cards per round,
  dominant-color rate, timeout rate, reconnection rate, and reshuffle frequency

### Requirement: Metrics Derived From An Unsampled Source

Every balance figure on the dashboard SHALL be derived from an **unsampled**
source — the projection's in-process aggregates and/or Prometheus — and SHALL NOT
be computed from a sampled exported trace. Figures SHALL reflect the full
population of completed games over the selected window.

#### Scenario: Sampling does not bias the numbers

- **WHEN** trace export sampling is enabled and an operator reads the dashboard
- **THEN** the balance figures still reflect 100% of completed rounds/games, not
  the sampled subset

#### Scenario: Source is the projection or Prometheus, not the trace store

- **WHEN** a balance figure is rendered
- **THEN** it was sourced from the unsampled in-process aggregates or Prometheus,
  never queried from a sampled trace backend

### Requirement: Embedded Metrics Visualization

The balance dashboard SHALL be rendered via **embedded Grafana** panels backed by
Prometheus, rather than re-implementing time-series charting in the custom admin
app. The embed SHALL be reachable only behind admin authentication.

#### Scenario: Balance charts render via embedded Grafana

- **WHEN** an authenticated operator opens the balance dashboard
- **THEN** the time-series charts are served by embedded Grafana panels over
  Prometheus

#### Scenario: Embed is gated by admin auth

- **WHEN** an unauthenticated request targets the embedded dashboard
- **THEN** it is denied; the embed is not publicly reachable
