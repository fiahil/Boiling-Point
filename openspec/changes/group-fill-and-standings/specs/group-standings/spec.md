<!-- NEW capability: a live, in-memory per-group win tally. Not persisted (dies with
     the group); distinct from persistence-and-replays' durable match history. -->

## ADDED Requirements

### Requirement: Live Per-Member Standings

A group SHALL keep a live, in-memory tally for each of its **members**: games played and
games won, with a win-rate derivable from the two. The tally exists only while the group
does and is never persisted.

#### Scenario: A member's win is tallied

- **WHEN** a game run by the group reaches `GameOver` and a member is among the winners
- **THEN** that member's games-played and wins both increase by one, and their win-rate reflects the new totals

#### Scenario: A member who played but did not win

- **WHEN** a game ends and a member played but is not a winner
- **THEN** that member's games-played increases by one and their wins is unchanged

#### Scenario: Co-champions each count

- **WHEN** a game ends in a Deathmatch with two or more members as co-champions
- **THEN** each co-champion member is credited with a win

### Requirement: Guest Wins Aggregate

A group SHALL keep an aggregate tally for **guests** (games that included a guest, and
guest wins) so a guest's result is recorded against the group's "guests" line rather than
vanishing. Guests do not get an individual per-member entry.

#### Scenario: A guest win rolls into the guests aggregate

- **WHEN** a game ends and the winner was the group's guest for that game
- **THEN** the group's guest-wins increases by one (and its guest-games count reflects that the game had a guest), with no per-guest member entry created

#### Scenario: Member standings exclude guests

- **WHEN** standings are computed
- **THEN** only members have individual entries; a guest's outcome appears solely in the guests aggregate

### Requirement: Standings Conveyed To Members

The server SHALL convey the current standings to a group's members — at least at
`GameOver` and when the membership changes — over the player wire, scoped to the group.

#### Scenario: Standings update after a game

- **WHEN** a game ends
- **THEN** the group's members receive an updated standings message reflecting the result

#### Scenario: Standings are dropped with the group

- **WHEN** a group is destroyed (empties, idles out, or is killed)
- **THEN** its standings are discarded and are not persisted or restored
