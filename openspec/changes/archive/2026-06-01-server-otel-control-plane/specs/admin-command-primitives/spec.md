## ADDED Requirements

### Requirement: Command Primitives Are Server-Owned And Off The Player Wire

The server SHALL expose control operations — config reload, per-item toggle, and
room lifecycle (seed / force-start / kill) — as an authoritative server API for the
admin command plane to call. These operations SHALL NOT be reachable from the
player `wire-protocol`: no `ClientMessage` SHALL trigger them, and they SHALL act
only through the authoritative game loop, never by mutating telemetry or projection
state.

#### Scenario: A player message cannot trigger a command primitive

- **WHEN** a connected player sends any `ClientMessage`
- **THEN** no config reload, toggle, or room lifecycle operation is invoked

#### Scenario: Primitives act through the game loop

- **WHEN** a room lifecycle primitive runs
- **THEN** it issues an authoritative command to the room's task rather than
  mutating shared state behind the loop's back

### Requirement: Validated Config Reload

The reload primitive SHALL parse and validate a new content/balance config reusing
the server's existing fail-fast validation, and apply it atomically. An invalid
config SHALL be rejected with its validation error and SHALL NOT be partially
applied; the running config SHALL be unchanged on rejection. A successful reload
SHALL affect rooms created after it.

#### Scenario: Invalid config is rejected wholesale

- **WHEN** the reload primitive is given a config that fails validation
- **THEN** it returns the validation error and the running config is unchanged

#### Scenario: Valid config applies atomically

- **WHEN** the reload primitive is given a config that passes validation
- **THEN** the new config and its derived registry are swapped in atomically and
  rooms created afterward use the new content

### Requirement: Per-Item Enable/Disable Toggle

The toggle primitive SHALL enable or disable a single content item (a card, effect,
or modifier) and re-validate the resulting config the same way as a reload, without
requiring a full config-file replacement. A toggle that would produce an invalid
config SHALL be rejected and leave the running config unchanged.

#### Scenario: Disabling a card takes effect for new rooms

- **WHEN** the toggle primitive disables a specific card and the result validates
- **THEN** the running config is updated and rooms created afterward deal from a
  deck excluding that card

#### Scenario: An invalid toggle is rejected

- **WHEN** a toggle would fail validation (e.g. emptying a required pool or breaking
  the deck-size invariant)
- **THEN** the toggle is rejected and the running config is unchanged

### Requirement: Room Lifecycle Primitives

The server SHALL provide operator room lifecycle primitives: **seed** (create a
fresh room), **force-start** (start a room that has not yet auto-started), and
**kill** (tear down an idle or stuck room). Kill and force-start SHALL be delivered
to the target room's task as authoritative commands; killing a room SHALL end its
`room.lifetime` span.

#### Scenario: Killing a room ends it authoritatively

- **WHEN** the kill primitive targets a live room
- **THEN** the room's task tears the room down, it is removed from the registry, and
  its `room.lifetime` span ends

#### Scenario: Force-start begins the game

- **WHEN** the force-start primitive targets a room waiting in the lobby
- **THEN** the room's task starts the game without waiting for the table to fill

#### Scenario: Seed creates a room

- **WHEN** the seed primitive runs
- **THEN** a fresh room is created in the registry with an invite code, exactly as a
  player-created room

### Requirement: Command Primitives Are Audited As Spans

Every command primitive invocation SHALL emit an audit span capturing the operator
identity, the action, its target, and its outcome (success or rejection). The audit
SHALL ride the same span stream that feeds the read surface, so the effect of a
command re-appears through telemetry rather than a side channel.

#### Scenario: A command is audited

- **WHEN** the admin command plane invokes any primitive
- **THEN** an audit span records the operator, action, target, and outcome

#### Scenario: A kill is confirmed through telemetry

- **WHEN** a kill primitive succeeds
- **THEN** the audit span records success and the room's `room.lifetime` span ends,
  which is what the live registry shows as confirmation
