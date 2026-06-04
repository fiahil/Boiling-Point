<!-- Add an active-games gauge and reword the lingering room→group in the metrics
     requirement (a group-model straggler outside that change's delta). -->

## MODIFIED Requirements

### Requirement: Game-Balance Metrics

The server SHALL emit Prometheus-scrapable metrics covering at least: active groups,
groups created, **active games (games in progress)**, connected players, game and round
durations, explosion rate, turn/wave timeout rate, cards per round, and reconnection
rate.

#### Scenario: Explosion rate is observable

- **WHEN** games complete over time
- **THEN** the explosion-rate metric reflects the share of rounds that exploded, supporting the ~30–40% balance target

#### Scenario: Live games and groups are observable

- **WHEN** games are in progress and groups exist
- **THEN** the active-games gauge reflects the number of in-progress games and the active-groups gauge reflects the number of live groups
