<!-- BREAKING: the entry/handshake messages and the invite-code type are renamed
     room → group. This bumps PROTOCOL_VERSION; all clients update in lockstep. -->

## MODIFIED Requirements

### Requirement: Protocol Version Handshake

The first client message on a connection SHALL be a **group entry message**
(`CreateGroup`, `JoinGroup`, or `EnqueueMatch`) carrying a `protocol_version`. The
server MUST accept a compatible version and reject an incompatible one before any game
state is shared. The invite code carried by `JoinGroup` and returned in `GroupJoined`
is a `GroupCode` (the `BREW-XXXX` format is unchanged).

#### Scenario: Compatible version is accepted

- **WHEN** a client sends a group entry message with a `protocol_version` the server supports
- **THEN** the server responds with `GroupJoined` and proceeds

#### Scenario: Incompatible version is rejected

- **WHEN** a client sends a group entry message with an unsupported `protocol_version`
- **THEN** the server responds with an `Error` describing the version mismatch and does not join the player to a group

#### Scenario: Unknown group code is rejected

- **WHEN** a client sends `JoinGroup` with a `GroupCode` that maps to no active group
- **THEN** the server responds with `Error { code: UnknownGroup }` and does not join the player to a group
