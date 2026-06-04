<!-- BREAKING: "room" → "group" throughout — both the requirement titles below and
     their bodies. The RENAMED block records the title changes; the MODIFIED block
     restates the bodies in group terms (and adds the persistent-group behavior); the
     ADDED block introduces the cross-game lifecycle. -->

## RENAMED Requirements

- FROM: `### Requirement: Invite-Code Rooms`
- TO: `### Requirement: Invite-Code Groups`

- FROM: `### Requirement: Idle Room Cleanup`
- TO: `### Requirement: Idle Group Cleanup`

## MODIFIED Requirements

### Requirement: Invite-Code Groups

The server SHALL let a player create a **group** and receive a short, human-readable
invite code (e.g. `BREW-7K3F`); other players join by submitting that code. Internally
each group is also keyed by a UUID for storage and logging. A group is a persistent
table that may run multiple games over its lifetime (see *Group Persists Across Games*).

#### Scenario: Create and join by code

- **WHEN** a player creates a group and shares the returned invite code, and another player submits that code
- **THEN** the second player joins the same group

#### Scenario: Unknown code is rejected

- **WHEN** a player submits an invite code that maps to no active group
- **THEN** the server replies with an `Error` (`UnknownGroup`) and does not join them to any group

### Requirement: Auto-Match Queue

The server SHALL provide an auto-match queue: players enqueue and the server assembles
them into groups of exactly 4, creating a group per set of four.

#### Scenario: Queue assembles a full table

- **WHEN** a fourth player enters the auto-match queue
- **THEN** the server removes those four from the queue, creates a group containing exactly them, and begins the game

### Requirement: Hostless Auto-Start at Four

Groups SHALL be hostless with no configurable settings; a game starts automatically and
only when the group holds exactly 4 ready players. There is no host role and no manual
start action.

#### Scenario: Game starts on the fourth ready player

- **WHEN** a group holds 4 ready players
- **THEN** the server transitions the group into a game and begins dealing without any player issuing a start command

#### Scenario: Partial group does not start

- **WHEN** a group holds fewer than 4 ready players
- **THEN** the server keeps it in the group lobby and does not start a game

### Requirement: Idle Group Cleanup

A group that sits in its lobby without an active game SHALL be destroyed after a
5-minute idle timeout, releasing its invite code.

#### Scenario: Idle group is reclaimed

- **WHEN** a group has stayed in its lobby for 5 minutes without reaching 4 ready players (including after a finished game)
- **THEN** the server destroys the group and releases its invite code

## ADDED Requirements

### Requirement: Group Persists Across Games

A group SHALL outlive any single game it runs. When a game ends (`GameOver`), the server
SHALL return its players to the group lobby — preserving the group's code and roster —
rather than destroying the group, so the same table can play again without re-queuing.

#### Scenario: Group returns to its lobby after a game

- **WHEN** a game run by a group reaches `GameOver`
- **THEN** the players are returned to that group's lobby with the group's code and roster intact, and the group is not destroyed

#### Scenario: Play again with the same table

- **WHEN** the players in a post-game group lobby choose to play again and 4 are ready
- **THEN** the server starts a fresh game for the same group, reusing the existing group and code

#### Scenario: Leaving frees a seat

- **WHEN** a player leaves the post-game group lobby
- **THEN** their seat is freed; the group persists for the remaining players (subject to idle cleanup)

### Requirement: Connection Persists Across Groups

A player's connection SHALL be a session that outlives any single group: it holds zero or
one group at a time and is never torn down by a game or group ending. Leaving a group
returns the connection to an unbound **menu** state on the same socket, from which the
player MAY create, join, or enqueue for another group without reconnecting.

#### Scenario: Connection survives a finished game

- **WHEN** a game reaches `GameOver`
- **THEN** each player's connection stays open and returns to the group lobby, rather than being closed

#### Scenario: Leave a group, then join another on one socket

- **WHEN** a player leaves their group and then sends a `JoinGroup` for a different code on the same connection
- **THEN** they join the second group without opening a new connection or re-authenticating

#### Scenario: Menu connection is kept alive while unbound

- **WHEN** a connected player sits in the unbound menu state without joining a group
- **THEN** the server keeps the connection alive as long as it heartbeats, independent of any group's lifecycle
