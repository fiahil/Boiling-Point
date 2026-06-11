## MODIFIED Requirements

### Requirement: Privileged Hidden-State Reveal

For a selected live room, any authenticated operator SHALL be able to reveal hidden game state — the round's boiling point, each player's pantry hand and spell hand, committed-but-unrevealed wave plays, the current pot volatility, and active spell effects — read from the attributes of that room's **open** spans. The reveal is a read (no elevated role required), but SHALL be served only over the authenticated admin channel and SHALL never be reachable from a player connection.

#### Scenario: Authorized reveal returns hidden state

- **WHEN** an authenticated operator requests the reveal for a live room
- **THEN** the admin channel returns that room's boiling point, committed wave plays, pantry and spell hands, current pot volatility, and active spell effects from its open spans

#### Scenario: Reveal is never served to a player

- **WHEN** any request for hidden state arrives on a player connection
- **THEN** no boiling point, hand, card, spell, or volatility data is returned

#### Scenario: Reveal of a room with no open round

- **WHEN** an operator requests the reveal for a room that is between rounds (no open `round` span)
- **THEN** the inspector reports no round in progress rather than stale secret data
