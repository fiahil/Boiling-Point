## ADDED Requirements

### Requirement: Disconnect Grace With Auto-Pass

When a player disconnects mid-game the server SHALL hold their seat for 60 seconds. While they are absent, the player auto-passes each wave — identical to a timer-expiry lockout — so the game never stalls.

#### Scenario: Absent player auto-passes

- **WHEN** a player is disconnected while a wave is open
- **THEN** they are treated as passing that wave (locked out for the round) and the wave proceeds for everyone else

#### Scenario: Seat is held during grace

- **WHEN** a player disconnects and reconnects within 60 seconds
- **THEN** their seat and identity are still present and they resume as themselves

### Requirement: State Snapshot On Rejoin

On reconnection the server SHALL send the rejoining player a full state snapshot scoped strictly to what that player is permitted to know — their own hand, public scores, round number, active modifiers, per-player contributed-card counts, hand sizes, and current phase — and never hidden data such as cauldron card identities or the boiling point.

#### Scenario: Snapshot restores allowed state only

- **WHEN** a player reconnects within the grace period
- **THEN** they receive a snapshot containing their own hand and all public state, and the snapshot omits other players' hands, cauldron card identities, and the boiling point

### Requirement: Abandonment After Grace

If a player remains disconnected for more than 60 seconds the server SHALL mark them abandoned, auto-pass all their future waves, and continue tracking their score for the rest of the game.

#### Scenario: Game continues after abandonment

- **WHEN** a player stays disconnected beyond 60 seconds
- **THEN** the server marks them abandoned, auto-passes their remaining waves, and the game continues with the other players while still recording the abandoned player's score

### Requirement: All-Disconnected Room Cleanup

If all players are disconnected mid-game the server SHALL hold the room for 60 seconds and then destroy it.

#### Scenario: Empty room is reclaimed

- **WHEN** every player has been disconnected from a room for 60 seconds
- **THEN** the server destroys the room and releases its resources
