## MODIFIED Requirements

### Requirement: Game-Balance Metrics Surface

**BREAKING** (the v1 figures — explosion rate vs ~30–40%, cards per round, dominant-color rate, reshuffle frequency — leave the dashboard with the v1 core): the balance dashboard SHALL surface the Principle IV metrics from the `boom-balance-metrics` definitions over a selectable time window: at least explosion (boom) rate against its `[needs playtesting]` working target, detonator distribution, fold/freeze rate, wave depth and duration, round and game durations, turn/wave timeout rate, and reconnection rate — extended additively with the per-feature panels as their changes land (per-Brewer pick/win rates after `boom2-brewers`, bucket pick rates and deck-archetype outcomes after `boom2-apothecary`, compounding trigger rates after `boom2-compounding`). It SHALL also surface the live fleet: the number of **games in progress** and the number of **groups that exist**.

#### Scenario: Boom rate is observable against its target

- **WHEN** an operator views the balance dashboard over a chosen window
- **THEN** the explosion-rate figure is shown against its current `[needs playtesting]` working target so live play can be compared with balance-study findings

#### Scenario: Core v2 balance metrics are present

- **WHEN** the dashboard renders
- **THEN** it includes boom rate, detonator distribution, fold/freeze rate, wave depth/duration, round/game duration, timeout rate, and reconnection rate

#### Scenario: Live games and groups are shown

- **WHEN** an operator views the dashboard while games are being played
- **THEN** it shows the current number of live games in progress and the current number of live groups

#### Scenario: A per-feature panel appears when its change lands

- **WHEN** `boom2-brewers` is implemented and the selected window includes Brewer games
- **THEN** the dashboard surfaces per-Brewer pick and win rates without any change to the pre-existing panels
