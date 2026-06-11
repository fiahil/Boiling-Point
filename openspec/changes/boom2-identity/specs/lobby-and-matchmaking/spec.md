## MODIFIED Requirements

### Requirement: Anonymous Session Authentication

A player SHALL authenticate anonymously: on first connection they supply a display name and the server issues a stable player UUID plus a session token. No email or password is required. A player MAY **optionally upgrade** that anonymous session to a persistent account (device-bound anonymous or OAuth); the upgrade preserves the existing player UUID, and anonymous play remains the default with no account required.

#### Scenario: New player receives identity

- **WHEN** a client connects and supplies a display name
- **THEN** the server issues a player UUID and a session token and associates the connection with that identity

#### Scenario: Session token re-authenticates the same identity

- **WHEN** a client reconnects presenting a previously issued session token
- **THEN** the server resolves it to the same player UUID rather than minting a new one

#### Scenario: Anonymous session upgrades to an account

- **WHEN** an anonymous player creates or links an account
- **THEN** the server binds the existing player UUID to the account so the identity persists across future sessions, while still permitting other players to continue anonymously

### Requirement: Auto-Match Queue

The server SHALL provide an auto-match queue with two intents: a **solo** player enqueues to find any table, and a **partial group** (fewer than 4 present members) enqueues to **fill** its empty seats. The queue is anchor-and-fill: waiting solos backfill a searching group's empty seats as **guests** (first-come); solos with no group to fill still assemble into a fresh group of exactly 4 (all members). A game starts only when a roster reaches exactly 4. When participants are **rated** (have accounts with ratings), the queue MAY apply a **skill-based ordering** to which solos fill which seats and which solos assemble together; this changes only the matching *policy*, never the queue's anchor-and-fill shape or the exactly-4 rule. Unrated (anonymous) play falls back to the first-come policy.

#### Scenario: Queue fills a partial group with a guest

- **WHEN** a group with 3 present members is searching for fill and a solo player is in the queue
- **THEN** the server places that solo into the group as a guest, bringing the roster to 4, and the group's game begins

#### Scenario: Solos with no group to fill form a fresh table

- **WHEN** four solo players are queued and no group is searching for fill
- **THEN** the server assembles them into a fresh group of exactly 4, all of them members, and begins the game

#### Scenario: Rated solos are grouped by skill

- **WHEN** several rated solo players are queued and the skill-based policy is active
- **THEN** the server prefers to assemble/fill tables among players of similar rating, still forming exactly-4 rosters via the same anchor-and-fill queue

#### Scenario: Unrated play is unaffected

- **WHEN** queued players have no ratings
- **THEN** the server uses the first-come anchor-and-fill policy exactly as in v1
