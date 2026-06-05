<!-- Surface live games and live groups on the dashboard. -->

## MODIFIED Requirements

### Requirement: Game-Balance Metrics Surface

The balance dashboard SHALL surface the Principle IV metrics over a selectable
time window: at least explosion rate, round and game durations, cards per round,
dominant-color (dominant-strategy) rate, turn/wave timeout rate, reconnection
rate, and reshuffle frequency. It SHALL also surface the live fleet: the number of
**games in progress** and the number of **groups that exist**.

#### Scenario: Explosion rate is observable against its target

- **WHEN** an operator views the balance dashboard over a chosen window
- **THEN** the explosion-rate figure is shown so it can be compared against the
  ~30–40% design target

#### Scenario: Core balance metrics are present

- **WHEN** the dashboard renders
- **THEN** it includes explosion rate, round/game duration, cards per round,
  dominant-color rate, timeout rate, reconnection rate, and reshuffle frequency

#### Scenario: Live games and groups are shown

- **WHEN** an operator views the dashboard while games are being played
- **THEN** it shows the current number of live games in progress and the current number of live groups
