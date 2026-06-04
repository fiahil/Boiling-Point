<!-- New fill/standings messages and a guest flag on public player info. BREAKING:
     bumps PROTOCOL_VERSION; all clients update in lockstep. -->

## ADDED Requirements

### Requirement: Group Fill Messages

The protocol SHALL provide bound-group client messages to request matchmaking fill
(`FillGroup`) and to cancel it (`CancelFill`), and a server broadcast announcing the
searching state and how many more players the group needs.

#### Scenario: Fill request is accepted from a partial group

- **WHEN** a member of a group with fewer than 4 present members sends `FillGroup`
- **THEN** the server begins searching and broadcasts a searching message stating how many more players are needed

#### Scenario: Fill is rejected for a full group

- **WHEN** a `FillGroup` is sent by a group that already has 4 in its roster
- **THEN** the server replies with an `Error` and starts no search

#### Scenario: Cancel stops the search

- **WHEN** a member sends `CancelFill` while the group is searching
- **THEN** the server stops searching and the group returns to its idle lobby

### Requirement: Guest Flag In Public Player Info

Public per-player table information SHALL indicate whether a seated player is a **guest**
(placed by fill) rather than a member, so clients can render the distinction. Member
seats are not guests.

#### Scenario: A guest is marked on the table

- **WHEN** a guest fills a group's seat and the table is conveyed (on join, game start, or snapshot)
- **THEN** that player's public info marks them as a guest and the members are not marked as guests

### Requirement: Standings Message

The server SHALL convey a group's standings to its members as a dedicated message: the
per-member games/wins (win-rate derivable) and the aggregate guest line. It is scoped to
the group's members and carries no per-game secrets.

#### Scenario: Members receive standings at game end

- **WHEN** a group's game ends
- **THEN** the group's members receive a standings message with each member's games and wins and the guest aggregate
