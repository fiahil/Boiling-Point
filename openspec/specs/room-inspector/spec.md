# room-inspector Specification

## Purpose
TBD - created by archiving change admin-ui. Update Purpose after archive.
## Requirements
### Requirement: Live Room, Session, And Queue Listing

The room inspector SHALL present, from the projection's open-span registry, a live
view of active rooms (id, mode, current phase, round X / total, age, player count,
last-wave age), connected players/sessions, and the auto-match queue depth. The
listing SHALL update as rooms and waves open and close.

#### Scenario: Active rooms are listed live

- **WHEN** an operator opens the room inspector
- **THEN** every room with an open `room.lifetime` span is listed with its current
  phase and round/wave derived from its open child spans

#### Scenario: Listing reflects lifecycle changes

- **WHEN** a room advances a phase, a new wave opens, or the room ends
- **THEN** the listing reflects the change without a manual refresh

#### Scenario: Queue depth is shown

- **WHEN** players are waiting in the auto-match queue
- **THEN** the inspector shows the current queue depth from the projection

### Requirement: Privileged Hidden-State Reveal

For a selected live room, any authenticated operator SHALL be able to reveal hidden
game state — the round's boiling point, each player's committed card and hand, the
current total volatility, and active modifiers — read from the attributes of that
room's **open** spans. The reveal is a read (no elevated role required), but SHALL
be served only over the authenticated admin channel and SHALL never be reachable
from a player connection.

#### Scenario: Authorized reveal returns hidden state

- **WHEN** an authenticated operator requests the reveal for a live room
- **THEN** the admin channel returns that room's boiling point, committed cards,
  hands, current volatility total, and active modifiers from its open spans

#### Scenario: Reveal is never served to a player

- **WHEN** any request for hidden state arrives on a player connection
- **THEN** no boiling point, hand, card, or volatility data is returned

#### Scenario: Reveal of a room with no open round

- **WHEN** an operator requests the reveal for a room that is between rounds (no
  open `round` span)
- **THEN** the inspector reports no round in progress rather than stale secret data

### Requirement: Stuck And Anomalous Room Detection

The inspector SHALL flag rooms whose open `wave` or `round` span has exceeded its
expected duration, and rooms associated with error-flagged spans, so operators can
identify stuck or misbehaving rooms.

#### Scenario: A stalled wave is flagged

- **WHEN** a room's open `wave` span age exceeds the expected wave-timer budget by
  a configured margin
- **THEN** the inspector flags that room as stuck

#### Scenario: An errored room is flagged

- **WHEN** a span associated with a room is marked as an error
- **THEN** the inspector surfaces that room as anomalous with the error context

### Requirement: Per-Game Replay

The inspector SHALL let an operator replay a recent completed game from the
projection's replay buffer, stepping through its spans wave by wave (commits,
resolve, score) including the now-completed round's revealed state.

#### Scenario: Replay a completed game wave by wave

- **WHEN** an operator selects a completed game retained in the replay buffer
- **THEN** the inspector replays its rounds and waves in order, showing what was
  committed and when the round exploded or settled

#### Scenario: Evicted game is not replayable

- **WHEN** an operator selects a game that has been evicted from the bounded buffer
- **THEN** the inspector reports it is no longer retained rather than showing
  partial data

