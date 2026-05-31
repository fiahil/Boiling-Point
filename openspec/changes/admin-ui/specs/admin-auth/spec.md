## ADDED Requirements

### Requirement: Admin Authentication Separate From Players

The admin interface SHALL authenticate operators through a mechanism entirely
separate from anonymous player session tokens, and every admin capability —
especially any reveal of hidden game state — SHALL be reachable only over the
authenticated admin channel and never from a player connection.

#### Scenario: Player credentials cannot reach admin functions

- **WHEN** a request presents only an anonymous player session token
- **THEN** the admin interface denies access to all admin capabilities

#### Scenario: Authenticated operator reaches the admin surface

- **WHEN** an operator authenticates through the admin mechanism
- **THEN** they are granted access to the admin capabilities their role permits,
  served over the admin channel only

### Requirement: Role-Based Capability Gating

Admin capabilities SHALL be gated by operator role. The privileged hidden-state
reveal and all control actions SHALL require an elevated role; read-only
observation (fleet overview, room list, balance dashboard) MAY be granted to a
lower observer role.

#### Scenario: Observer role is denied the reveal

- **WHEN** an operator with only an observer role requests the hidden-state reveal
  for a room
- **THEN** the request is denied and no boiling point, hand, or volatility data is
  returned

#### Scenario: Observer role is denied control actions

- **WHEN** an operator with only an observer role issues a control action (reload,
  toggle, or room lifecycle)
- **THEN** the action is rejected with an authorization error and no state changes

#### Scenario: Elevated role reaches reveal and control

- **WHEN** an operator with an elevated role requests the reveal or issues a
  control action
- **THEN** the request is authorized and served over the admin channel

### Requirement: Admin Channel Isolation From The Player Protocol

The admin API SHALL be served on a transport surface distinct from the player
WebSocket (a separate route namespace and/or port), and SHALL never accept the
player `protocol/` wire or widen it. A connection established as a player
connection SHALL never be upgraded to admin privileges.

#### Scenario: Player WebSocket cannot invoke admin endpoints

- **WHEN** a client connected on the player WebSocket attempts to call an admin
  endpoint or request privileged data
- **THEN** the server does not serve any admin capability over that connection

#### Scenario: Admin endpoints reject the player wire format

- **WHEN** a request to the admin API carries a player-protocol message
- **THEN** it is rejected; the admin API exposes only its own command/read schema
