## ADDED Requirements

### Requirement: Free-For-All Multiplayer Rating Model

The server SHALL maintain a per-account rating using a **multiplayer free-for-all** model (Weng-Lin / TrueSkill-style), **not** 2-player Elo, so that a single 4-player result updates all four ratings consistently.

#### Scenario: A 4-player result updates all four ratings

- **WHEN** a rated game finishes with a full finishing order of four accounts
- **THEN** the server updates each account's rating from the multiplayer result in one consistent computation

#### Scenario: Mixed rated/unrated tables

- **WHEN** a game includes anonymous-only participants alongside accounts
- **THEN** rating updates apply only to the accounts; anonymous participants neither gain nor affect durable rating (the result still completes normally)

### Requirement: Ratings Update From Finished Games Only

Rating updates SHALL be computed server-side from **finished** game results (the authoritative finishing order/scores), consistent with the post-game persistence model. Mid-game state SHALL NOT alter ratings.

#### Scenario: Abandoned games do not corrupt ratings

- **WHEN** a game ends without a complete result (e.g. mass disconnect before finish)
- **THEN** the server applies the defined incomplete-game rule rather than a partial rating update
