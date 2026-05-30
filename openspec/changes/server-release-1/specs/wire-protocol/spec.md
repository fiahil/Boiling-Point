## ADDED Requirements

### Requirement: Binary Message Encoding

The server SHALL encode all real-time game messages as MessagePack over WebSocket, using enum-tagged variants (e.g. `#[serde(tag = "type")]`) so that a JSON fallback mode is available for debugging without changing message shape.

#### Scenario: Client and server exchange MessagePack frames

- **WHEN** a connected client sends a well-formed MessagePack-encoded client message
- **THEN** the server decodes it, processes it, and replies with MessagePack-encoded frames

#### Scenario: Malformed frame is rejected without state change

- **WHEN** the server receives a frame it cannot decode into a known client message
- **THEN** it discards the frame, leaves all game state unchanged, and emits an `Error` to that connection only

### Requirement: Audience-Scoped Messages

Every server message SHALL declare an audience of either Private (delivered only to one player) or Broadcast (delivered to all players in the room). The server MUST NOT place hidden information — hands, exact volatility totals, the boiling point, or other players' wave commits — in a Broadcast message.

#### Scenario: Private message reaches only its recipient

- **WHEN** the server sends a `YourHand`, `PeekResult`, `Error`, or `StateSnapshot` message
- **THEN** only the targeted player's connection receives it

#### Scenario: Broadcast carries no secrets

- **WHEN** the server broadcasts a wave or round event
- **THEN** the payload contains only public information (action counts, identities of who played or passed, depile reveal data) and never face-down card identities nor the boiling point

### Requirement: Protocol Version Handshake

The first client message on a connection SHALL be `JoinRoom` carrying a `protocol_version`. The server MUST accept a compatible version and reject an incompatible one before any game state is shared.

#### Scenario: Compatible version is accepted

- **WHEN** a client sends `JoinRoom` with a `protocol_version` the server supports
- **THEN** the server responds with `RoomJoined` and proceeds

#### Scenario: Incompatible version is rejected

- **WHEN** a client sends `JoinRoom` with an unsupported `protocol_version`
- **THEN** the server responds with an `Error` describing the version mismatch and does not join the player to a room

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
