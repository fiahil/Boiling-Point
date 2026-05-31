## ADDED Requirements

### Requirement: Control Is A Separate Command Channel, Never Telemetry

Control actions SHALL be issued over an explicit admin **command API** distinct
from the read projection. Telemetry (spans, the projection) SHALL remain
read-only and SHALL NOT be a path for performing writes. Every control action
SHALL require an elevated operator role (per `admin-auth`).

#### Scenario: A control action goes through the command API

- **WHEN** an operator triggers reload, a toggle, or a room lifecycle action
- **THEN** the action is carried by the command API, not by the span/projection
  read path

#### Scenario: The read projection cannot mutate state

- **WHEN** any read of the projection or span stream occurs
- **THEN** no game or config state changes as a result

### Requirement: Validated Content And Config Reload

The command API SHALL reload content/balance config reusing the server's existing
fail-fast validation. Invalid config SHALL be rejected with validation errors and
SHALL NOT be partially applied; a successful reload SHALL apply atomically.

#### Scenario: Invalid config is rejected wholesale

- **WHEN** an operator reloads a config that fails validation
- **THEN** the command returns the validation errors and the running config is
  unchanged

#### Scenario: Valid config applies atomically

- **WHEN** an operator reloads a config that passes validation
- **THEN** the new config is applied atomically and the reload is reported as
  successful

### Requirement: Per-Item Enable/Disable Toggles

The command API SHALL allow per-item enable/disable of content (cards, effects,
modifiers), validated the same way as a reload, without requiring a full config
file replacement.

#### Scenario: Disabling a card takes effect

- **WHEN** an operator disables a specific card
- **THEN** subsequent deals exclude that card and the change is reflected in the
  running config

#### Scenario: An invalid toggle is rejected

- **WHEN** an operator toggles an item in a way that fails validation (e.g.
  emptying a required pool)
- **THEN** the toggle is rejected and the running config is unchanged

### Requirement: Room Lifecycle Actions

The command API SHALL support operator room lifecycle actions: seed/create a room,
force-start a room, and kill an idle or stuck room. These actions SHALL go through
the server's authoritative game loop, never by mutating projection state.

#### Scenario: Killing a stuck room ends it authoritatively

- **WHEN** an operator kills a room flagged as stuck
- **THEN** the game loop tears the room down and its `room.lifetime` span ends

#### Scenario: Force-start begins the game

- **WHEN** an operator force-starts a seeded room
- **THEN** the game loop starts the game for that room

### Requirement: Control Actions Are Audited And Observable

Every control action SHALL emit an audit record — as a span — capturing the
operator identity, the action, its target, and its outcome. The action's effect
SHALL re-appear in the span stream so the admin UI confirms it through the same
telemetry that drives the read surface (the loop closes).

#### Scenario: An action is audited

- **WHEN** an operator issues any control action
- **THEN** an audit span records the operator, action, target, and outcome

#### Scenario: The UI confirms via telemetry

- **WHEN** a kill-room action succeeds
- **THEN** the room's disappearance from the live registry (its `room.lifetime`
  span ending) is what the UI shows as confirmation, not a side-channel ack
