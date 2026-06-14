## ADDED Requirements

### Requirement: Persistent Accounts Are An Additive Upgrade

The server SHALL support persistent player accounts as an optional upgrade from an anonymous session; anonymous play SHALL remain fully supported and require no account, and upgrading to a device or passkey account SHALL preserve the player's existing identity.

#### Scenario: Anonymous play still works with no account

- **WHEN** a player connects without an account
- **THEN** they receive an anonymous session and can join groups and play games exactly as in v1

#### Scenario: Upgrading an anonymous session to a device or passkey account

- **WHEN** an anonymous player creates a device account or registers a passkey
- **THEN** their current player identity is bound to the new account and persists across future sessions (and, for a passkey, across devices)

### Requirement: Three Account Types — Device-Bound, Passkey, And OAuth

The system SHALL offer a device-bound anonymous account (a durable token, no credentials), a passkey account (a pseudonym plus a WebAuthn credential, with no password and no password backup), and an OAuth account (Google, Apple, Microsoft, or Discord); all resolve to the same durable player-identity model.

#### Scenario: Device-bound account survives a session

- **WHEN** a player with a device-bound account reconnects on the same device with its token
- **THEN** the server resolves them to their existing durable identity without new credentials

#### Scenario: Passkey account signs in with pseudonym and passkey

- **WHEN** a player signs in with their pseudonym and a passkey assertion the server verifies against the stored credential
- **THEN** the server resolves them to their durable identity, with no password involved at any point

#### Scenario: OAuth account is portable across devices

- **WHEN** a player signs in with the same OAuth provider identity on a new device
- **THEN** the server resolves them to the same durable identity established on first sign-in

### Requirement: One Identity Per Account With No Conflicts

An OAuth account SHALL be keyed by its (provider, subject) identity so the same provider identity always resolves to the same account; a brand-new provider identity SHALL create a fresh account on first sign-in; the server SHALL NOT link a second provider (or a different identity) to an existing account, and sign-in SHALL never fail with a conflict.

#### Scenario: Same provider identity resolves to the same account

- **WHEN** a player signs in twice with the same provider and subject (e.g. from two devices)
- **THEN** both resolve to the same account and durable player identity

#### Scenario: A different provider is a different account

- **WHEN** a player signs in with a different provider (even with an equal-looking subject)
- **THEN** the server resolves a separate account; the two are never merged

### Requirement: Accounts Carry No Email And No Real Name

An account SHALL store no email address and no real name; OAuth/passkey sign-in SHALL request no profile scopes and read only the provider's stable opaque subject (or the passkey credential). Every account SHALL be auto-assigned a unique, themed pseudonym as its display name.

#### Scenario: A new account is auto-named, never from the provider

- **WHEN** any account is created (device, passkey, or first OAuth sign-in)
- **THEN** the server assigns a unique, themed pseudonym and stores no email or provider-supplied real name

### Requirement: Display Name May Be Changed Once

A player SHALL be able to change their account's display name exactly once; the new name MUST be unique and well-formed, and once the single change is spent the name is locked.

#### Scenario: The one rename succeeds then locks

- **WHEN** a player changes their display name to an available, well-formed name
- **THEN** the name is applied and further rename attempts are rejected as locked

#### Scenario: A taken or malformed name is rejected

- **WHEN** a player requests a name that is already taken by another account or is malformed
- **THEN** the change is rejected and the current name is unchanged (the one rename is not consumed)

### Requirement: Players May Delete Their Account

A player SHALL be able to delete their account; deletion SHALL erase the account, its rating, and its player record (no durable history is preserved for it), after which the connection continues as an anonymous player. Shared anonymous game replays are immutable records and are left intact.

#### Scenario: Deletion erases the durable identity

- **WHEN** a player deletes their account
- **THEN** the account, its rating, and its player record are removed, and a previously issued credential (e.g. a device token) no longer resolves to an identity

### Requirement: Accounts Record Last Login

The server SHALL record each account's most recent successful sign-in timestamp in durable storage, updated on every resume / OAuth / passkey login.

#### Scenario: Last login is updated on sign-in

- **WHEN** a player signs in to an existing account
- **THEN** the account's stored last-login timestamp is updated to the time of that sign-in

### Requirement: Accounts Are The Attachment Point For Durable State

Durable cross-game state (rating; later, profiles/career stats) SHALL attach to an account, not to an anonymous session; anonymous-only players SHALL NOT accrue durable rating or profile state.

#### Scenario: Durable state requires an account

- **WHEN** a finished game would update durable state for a participant
- **THEN** that update applies only if the participant has an account; anonymous-only participants accrue no durable rating
