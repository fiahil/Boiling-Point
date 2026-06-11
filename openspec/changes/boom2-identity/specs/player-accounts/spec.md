## ADDED Requirements

### Requirement: Persistent Accounts Are An Additive Upgrade

The server SHALL support **persistent player accounts** as an **optional upgrade** from an anonymous session. Playing anonymously SHALL remain fully supported and require no account. Creating or linking an account SHALL preserve the player's existing identity.

#### Scenario: Anonymous play still works with no account

- **WHEN** a player connects without an account
- **THEN** they receive an anonymous session and can join groups and play games exactly as in v1

#### Scenario: Upgrading an anonymous session to an account

- **WHEN** an anonymous player creates or links an account
- **THEN** their current player identity is bound to the account and persists across future sessions and devices (per the account type)

### Requirement: Two Account Types — Device-Bound Anonymous And OAuth

The system SHALL offer a **device-bound anonymous** account (a durable token tied to the device, no credentials) and an **OAuth** account (e.g. Google/Discord). Both resolve to the same durable player identity model.

#### Scenario: Device-bound account survives a session

- **WHEN** a player with a device-bound anonymous account reconnects on the same device
- **THEN** the server resolves them to their existing durable identity without new credentials

#### Scenario: OAuth account is portable across devices

- **WHEN** a player signs in with OAuth on a new device
- **THEN** the server resolves them to the same durable identity established on first sign-in

### Requirement: Accounts Are The Attachment Point For Durable State

Durable cross-game state (rating; later, profiles/career stats) SHALL attach to an **account**, not to an anonymous session. Anonymous-only players SHALL NOT accrue durable rating or profile state.

#### Scenario: Durable state requires an account

- **WHEN** a finished game would update durable state for a participant
- **THEN** that update applies only if the participant has an account; anonymous-only participants accrue no durable rating
