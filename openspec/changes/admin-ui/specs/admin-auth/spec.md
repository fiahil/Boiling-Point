> **STUB.** Placeholder so the change is structurally valid. Only `admin-auth` is
> sketched (one requirement); `room-inspector`, `balance-dashboard`, and
> `content-config-admin` are named in the proposal and to be specced when this
> change is promoted from stub.

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
