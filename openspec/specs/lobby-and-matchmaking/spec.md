# lobby-and-matchmaking Specification

## Purpose
TBD - created by archiving change server-release-1. Update Purpose after archive.
## Requirements
### Requirement: Anonymous Session Authentication

A player SHALL authenticate anonymously: on first connection they supply a display name and the server issues a stable player UUID plus a session token. No email or password is required.

#### Scenario: New player receives identity

- **WHEN** a client connects and supplies a display name
- **THEN** the server issues a player UUID and a session token and associates the connection with that identity

#### Scenario: Session token re-authenticates the same identity

- **WHEN** a client reconnects presenting a previously issued session token
- **THEN** the server resolves it to the same player UUID rather than minting a new one

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

The server SHALL provide an auto-match queue with two intents: a **solo** player enqueues
to find any table, and a **partial group** (fewer than 4 present members) enqueues to
**fill** its empty seats. The queue is anchor-and-fill: waiting solos backfill a
searching group's empty seats as **guests** (first-come); solos with no group to fill
still assemble into a fresh group of exactly 4 (all members). A game starts only when a
roster reaches exactly 4.

#### Scenario: Queue fills a partial group with a guest

- **WHEN** a group with 3 present members is searching for fill and a solo player is in the queue
- **THEN** the server places that solo into the group as a guest, bringing the roster to 4, and the group's game begins

#### Scenario: Solos with no group to fill form a fresh table

- **WHEN** four solo players are queued and no group is searching for fill
- **THEN** the server assembles them into a fresh group of exactly 4, all of them members, and begins the game

### Requirement: Hostless Auto-Start at Four

Groups SHALL be hostless with no configurable settings; a game starts automatically and
only when the roster (present members plus any guests) is exactly 4 ready players. A
group SHALL hold at most 4 members (the table size), so it never needs more than the
fill required to reach 4. There is no host role and no manual start action.

#### Scenario: Game starts on the fourth ready player

- **WHEN** a group's roster reaches 4 ready players (members, optionally topped up by guests)
- **THEN** the server transitions the group into a game and begins dealing without any player issuing a start command

#### Scenario: Member cap equals the table

- **WHEN** a group already holds 4 members
- **THEN** it does not request fill and admits no guest

### Requirement: Idle Group Cleanup

A group that sits in its lobby without an active game SHALL be destroyed after a
5-minute idle timeout, releasing its invite code.

#### Scenario: Idle group is reclaimed

- **WHEN** a group has stayed in its lobby for 5 minutes without reaching 4 ready players (including after a finished game)
- **THEN** the server destroys the group and releases its invite code

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

### Requirement: Group Fill Matchmaking

A group with fewer than 4 present members SHALL be able to request matchmaking **fill**
for its empty seats, entering a visible "searching" state until the roster reaches 4 or a
member cancels. Players placed by fill join as **guests**, not members.

#### Scenario: Requesting fill enters a searching state

- **WHEN** a member of a partial group requests fill
- **THEN** the group enters a searching state, announces how many more players it needs, and registers with the queue for that many seats

#### Scenario: Cancel returns to the idle lobby

- **WHEN** a member cancels the fill search
- **THEN** the group leaves the queue and returns to its idle lobby with its current members, admitting no guest

#### Scenario: A searching group that loses its last member is reclaimed

- **WHEN** a searching group drops below one present member
- **THEN** the server removes it from the fill queue and reclaims it (idle/empty-group cleanup)

### Requirement: Members Persist, Guests Are One-Game

A group SHALL distinguish **members** (joined by invite code, or a fresh quick-match
table's founding players) from **guests** (placed by fill). When a game ends, the group
returns to its lobby keeping only its members (plus reconnected members); guest seats are
dropped and the guest's connection returns to the unbound menu.

#### Scenario: The guest is dropped after the game

- **WHEN** a game that included a guest reaches `GameOver`
- **THEN** the group returns to its lobby with only its members, the guest seat is freed, and the guest's connection is in the menu state

#### Scenario: Members return to the same group

- **WHEN** a game ends
- **THEN** the group's members are returned to the group lobby with the group's code and roster of members intact, ready to play again or fill again

