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

Hosting observability and control together SHALL NOT merge their channels: every live read is served from the span projection, every historical read (popularity stats) is a read-only query of post-game persistence, and every control action goes through the separate audited command API, whose effects re-appear in the span stream the command center reads.

#### Scenario: A control action from the command center round-trips through telemetry

- **WHEN** an operator triggers a control action from the command center
- **THEN** it executes via the audited command API and its effect re-appears as spans, so the command center confirms it through the same read path as everything else

### Requirement: Popularity Stats From Post-Game Persistence

The command center SHALL surface popularity figures over a selectable window of UTC days — at least games per day as a bar chart, unique players per day with first-ever players distinguished, a games-by-hour-of-day (UTC) histogram, the returning-player share (window players who played on more than one distinct day), and window/lifetime totals (games, players, new players) — served read-only from post-game persistence (the consolidated game records), since the live projection only knows the current process; with no database configured the panel SHALL report the stats as unavailable rather than failing (persistence is optional infrastructure, never a precondition).

#### Scenario: Games per day render as a bar chart

- **WHEN** an operator opens the popularity panel with persistence configured
- **THEN** a bar chart shows games completed for every day of the selected window (zero-height bars for days without games), alongside unique and new players per day and the window/lifetime totals

#### Scenario: Play time-of-day and stickiness are visible

- **WHEN** an operator views the popularity panel over a chosen window
- **THEN** a 24-bucket histogram shows when in the (UTC) day the window's games were played, and the returning-player share reports what fraction of the window's players came back on a second day

#### Scenario: No database degrades gracefully

- **WHEN** the server runs without a configured database
- **THEN** the popularity panel reports the stats as unavailable (and why) while every projection-backed surface keeps working

#### Scenario: The popularity read is operator-gated and read-only

- **WHEN** an unauthenticated request targets the popularity stats, or an operator reads them
- **THEN** the unauthenticated request is denied, and the operator read mutates no game, config, or persistence state
