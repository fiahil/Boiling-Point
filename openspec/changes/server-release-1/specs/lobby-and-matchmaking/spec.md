## ADDED Requirements

### Requirement: Anonymous Session Authentication

A player SHALL authenticate anonymously: on first connection they supply a display name and the server issues a stable player UUID plus a session token. No email or password is required.

#### Scenario: New player receives identity

- **WHEN** a client connects and supplies a display name
- **THEN** the server issues a player UUID and a session token and associates the connection with that identity

#### Scenario: Session token re-authenticates the same identity

- **WHEN** a client reconnects presenting a previously issued session token
- **THEN** the server resolves it to the same player UUID rather than minting a new one

### Requirement: Invite-Code Rooms

The server SHALL let a player create a room and receive a short, human-readable invite code (e.g. `BREW-7K3F`); other players join by submitting that code. Internally each room is also keyed by a UUID for storage and logging.

#### Scenario: Create and join by code

- **WHEN** a player creates a room and shares the returned invite code, and another player submits that code
- **THEN** the second player joins the same room

#### Scenario: Unknown code is rejected

- **WHEN** a player submits an invite code that maps to no active room
- **THEN** the server replies with an `Error` and does not join them to any room

### Requirement: Auto-Match Queue

The server SHALL provide an auto-match queue: players enqueue and the server assembles them into groups of exactly 4, creating a room per group.

#### Scenario: Queue assembles a full table

- **WHEN** a fourth player enters the auto-match queue
- **THEN** the server removes those four from the queue, creates a room containing exactly them, and begins the game

### Requirement: Hostless Auto-Start at Four

Rooms SHALL be hostless with no configurable settings; the game starts automatically and only when the room holds exactly 4 players. There is no host role and no manual start action.

#### Scenario: Game starts on the fourth join

- **WHEN** a room reaches 4 players
- **THEN** the server transitions the room out of the lobby and begins dealing without any player issuing a start command

#### Scenario: Partial room does not start

- **WHEN** a room holds fewer than 4 players
- **THEN** the server keeps it in the lobby and does not start the game

### Requirement: Idle Room Cleanup

A room that sits in the lobby without starting a game SHALL be destroyed after a 5-minute idle timeout.

#### Scenario: Idle invite room is reclaimed

- **WHEN** a room has stayed in the lobby for 5 minutes without reaching 4 players
- **THEN** the server destroys the room and releases its invite code
