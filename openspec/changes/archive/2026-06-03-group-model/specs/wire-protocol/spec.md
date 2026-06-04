<!-- BREAKING: the entry/handshake messages and the invite-code type are renamed
     room → group. This bumps PROTOCOL_VERSION; all clients update in lockstep. The
     connection becomes a durable session that can leave one group and bind to another
     on the same socket (LeaveGroup / LeftGroup). -->

## MODIFIED Requirements

### Requirement: Protocol Version Handshake

The first client message on a connection SHALL be a **group entry message**
(`CreateGroup`, `JoinGroup`, or `EnqueueMatch`) carrying a `protocol_version`. The
server MUST accept a compatible version and reject an incompatible one before any game
state is shared. The invite code carried by `JoinGroup` and returned in `GroupJoined`
is a `GroupCode` (the `BREW-XXXX` format is unchanged).

The connection is a **durable session**: the `protocol_version` is negotiated and the
player's identity (via `session_token`) is established **once**, on this first message.
After a player leaves a group, the same connection MAY carry another group entry message
to bind to a different group, and the protocol version SHALL NOT be renegotiated.

#### Scenario: Compatible version is accepted

- **WHEN** a client sends a group entry message with a `protocol_version` the server supports
- **THEN** the server responds with `GroupJoined` and proceeds

#### Scenario: Incompatible version is rejected

- **WHEN** a client sends a group entry message with an unsupported `protocol_version`
- **THEN** the server responds with an `Error` describing the version mismatch and does not join the player to a group

#### Scenario: Unknown group code is rejected

- **WHEN** a client sends `JoinGroup` with a `GroupCode` that maps to no active group
- **THEN** the server responds with `Error { code: UnknownGroup }` and does not join the player to a group

#### Scenario: Re-entry on an established session

- **WHEN** a player who has left their group sends another `CreateGroup`/`JoinGroup`/`EnqueueMatch` on the same connection
- **THEN** the server binds the connection to the new group using the connection's established identity, without renegotiating the protocol version

## ADDED Requirements

### Requirement: Leave Group Without Disconnecting

The protocol SHALL provide a `LeaveGroup` client message that detaches the connection
from its current group without closing the socket. The server SHALL free the player's
seat in that group and acknowledge with a `LeftGroup` message, after which the connection
is in an unbound (menu) state and ready to accept another group entry message.

#### Scenario: Leaving returns to the menu state

- **WHEN** a player sends `LeaveGroup` while bound to a group
- **THEN** the server removes them from that group, replies with `LeftGroup`, keeps the socket open, and accepts a subsequent group entry message on the same connection

#### Scenario: Leave with no group bound is rejected

- **WHEN** a player sends `LeaveGroup` while in the unbound menu state
- **THEN** the server replies with an `Error`, changes no state, and the connection stays in the menu state

### Requirement: Table Actions Require a Bound Group

The server SHALL accept table/game messages (`CommitCard`, `CommitPass`, `LockIn`,
`Emote`) only from a connection currently bound to a group; received in the unbound menu
state they SHALL be rejected with an `Error` and change no state. `Heartbeat` SHALL be
serviced in any state, including the unbound menu state.

#### Scenario: Game action in the menu state is rejected

- **WHEN** a connection in the unbound menu state sends a `CommitCard`
- **THEN** the server replies with an `Error` and applies no state change

#### Scenario: Heartbeat keeps an unbound connection alive

- **WHEN** a connection in the unbound menu state sends `Heartbeat`
- **THEN** the server services it and the connection is not reaped by the idle timeout
