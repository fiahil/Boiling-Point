# wire-protocol Specification

## Purpose
TBD - created by archiving change server-release-1. Update Purpose after archive.
## Requirements
### Requirement: Binary Message Encoding

The server SHALL encode all real-time game messages as MessagePack over WebSocket, using enum-tagged variants (e.g. `#[serde(tag = "type")]`) so that a JSON fallback mode is available for debugging without changing message shape.

#### Scenario: Client and server exchange MessagePack frames

- **WHEN** a connected client sends a well-formed MessagePack-encoded client message
- **THEN** the server decodes it, processes it, and replies with MessagePack-encoded frames

#### Scenario: Malformed frame is rejected without state change

- **WHEN** the server receives a frame it cannot decode into a known client message
- **THEN** it discards the frame, leaves all game state unchanged, and emits an `Error` to that connection only

### Requirement: Audience-Scoped Messages

Every server message SHALL declare an audience of either Private (delivered only to one player) or Broadcast (delivered to all players in the group). The server MUST NOT place hidden information — hands, exact volatility totals, the boiling point, or other players' wave commits — in a Broadcast message.

#### Scenario: Private message reaches only its recipient

- **WHEN** the server sends a `YourHand`, `PeekResult`, `Error`, or `StateSnapshot` message
- **THEN** only the targeted player's connection receives it

#### Scenario: Broadcast carries no secrets

- **WHEN** the server broadcasts a wave or round event
- **THEN** the payload contains only public information (action counts, identities of who played or passed, depile reveal data) and never face-down card identities nor the boiling point

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

### Requirement: Per-Connection Rate Limiting

The server SHALL cap inbound actions at one per 100 ms per connection and silently drop excess messages.

#### Scenario: Burst is throttled

- **WHEN** a connection sends more than one action within a 100 ms window
- **THEN** the server processes at most one action and silently drops the remainder, with no `Error` and no state change

### Requirement: Connection Liveness

The protocol SHALL provide a heartbeat: clients send periodic heartbeats and the server acknowledges them, so a dead connection can be detected and routed into the reconnection flow rather than silently lingering.

#### Scenario: Heartbeat keeps a connection alive

- **WHEN** a client sends periodic heartbeats
- **THEN** the server treats the connection as live and acknowledges them

#### Scenario: Missing heartbeat triggers disconnect handling

- **WHEN** a connection stops sending heartbeats beyond the allowed interval
- **THEN** the server treats it as disconnected and applies the reconnection grace handling

### Requirement: Fire-and-Forget Invalid Actions

Client messages SHALL be fire-and-forget. An action the server cannot apply produces an `Error` to the sender and no state change; the server MUST never partially apply an action.

#### Scenario: Invalid action produces an error only

- **WHEN** a client sends an action that fails validation (wrong phase, a card not in their hand, acting while locked out)
- **THEN** the server replies with an `Error` to that sender and leaves game state unchanged

### Requirement: Wave-Open Message Carries the Timer Budget

The broadcast that opens a wave SHALL carry that wave's timer budget (its duration or absolute deadline) so every client and bot can render or time a countdown. The server remains the sole authority over when the wave closes; the budget is informational and clients MUST NOT use it to locally accept or reject a commit.

#### Scenario: Wave-open broadcast includes the timer

- **WHEN** the server opens a wave
- **THEN** the broadcast announcing the open wave includes the wave's timer budget (30 seconds for wave 1 of a round, 10 seconds for subsequent waves **[needs playtesting]**)

#### Scenario: Server alone decides close

- **WHEN** a client's local countdown reaches zero but the server has not yet closed the wave
- **THEN** the client may still submit or change its selection, and the server applies it if the wave is still open on the server's authoritative clock

### Requirement: Wave-Open Final-Wave Flag

The wave-open broadcast SHALL indicate whether the wave is the one-player final wave — the single wave granted to the last remaining active player before the pot settles — so clients can render that it is the final wave.

#### Scenario: Final wave is flagged

- **WHEN** only one active player remains and the server opens their single final wave
- **THEN** the wave-open broadcast marks the wave as the final wave

#### Scenario: Ordinary waves are not flagged

- **WHEN** the server opens a wave with two or more active players
- **THEN** the wave-open broadcast does not mark the wave as final

### Requirement: Deathmatch Start Broadcast

When the game reaches the Deathmatch tiebreaker (two or more players tied for the lead after the final round), the server SHALL broadcast that the Deathmatch has begun, naming its participants. The tiebreaker outcome (champion or co-champions) is conveyed by the subsequent `GameOver` message.

#### Scenario: Deathmatch start is announced with participants

- **WHEN** the final round ends with two or more players tied for the lead
- **THEN** the server broadcasts a Deathmatch-start message listing the tied participants, before the `GameOver` that reports the winner(s)

#### Scenario: No Deathmatch on a clear winner

- **WHEN** the final round ends with a single player leading
- **THEN** no Deathmatch-start message is sent and the game ends directly with `GameOver`

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

