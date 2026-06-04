<!-- The auto-match queue gains partial-group fill; the group lifecycle gains a
     member/guest distinction. Builds on group-model's persistent groups. -->

## MODIFIED Requirements

### Requirement: Auto-Match Queue

The server SHALL provide an auto-match queue with two intents: a **solo** player enqueues
to find any table, and a **partial group** (fewer than 4 present members) enqueues to
**fill** its empty seats. The queue is anchor-and-fill: waiting solos backfill a
searching group's empty seats as **guests** (first-come); solos with no group to fill
still assemble into a fresh group of exactly 4 (all members). A game starts only when a
roster reaches exactly 4.

#### Scenario: Queue fills a partial group with a guest

- **WHEN** a group with 3 present members is searching for fill and a solo player is in the queue
- **THEN** the server places that solo into the group as a guest, bringing the roster to 4, and the group's game begins

#### Scenario: Solos with no group to fill form a fresh table

- **WHEN** four solo players are queued and no group is searching for fill
- **THEN** the server assembles them into a fresh group of exactly 4, all of them members, and begins the game

### Requirement: Hostless Auto-Start at Four

Groups SHALL be hostless with no configurable settings; a game starts automatically and
only when the roster (present members plus any guests) is exactly 4 ready players. A
group SHALL hold at most 4 members (the table size), so it never needs more than the
fill required to reach 4. There is no host role and no manual start action.

#### Scenario: Game starts on the fourth ready player

- **WHEN** a group's roster reaches 4 ready players (members, optionally topped up by guests)
- **THEN** the server transitions the group into a game and begins dealing without any player issuing a start command

#### Scenario: Member cap equals the table

- **WHEN** a group already holds 4 members
- **THEN** it does not request fill and admits no guest

## ADDED Requirements

### Requirement: Group Fill Matchmaking

A group with fewer than 4 present members SHALL be able to request matchmaking **fill**
for its empty seats, entering a visible "searching" state until the roster reaches 4 or a
member cancels. Players placed by fill join as **guests**, not members.

#### Scenario: Requesting fill enters a searching state

- **WHEN** a member of a partial group requests fill
- **THEN** the group enters a searching state, announces how many more players it needs, and registers with the queue for that many seats

#### Scenario: Cancel returns to the idle lobby

- **WHEN** a member cancels the fill search
- **THEN** the group leaves the queue and returns to its idle lobby with its current members, admitting no guest

#### Scenario: A searching group that loses its last member is reclaimed

- **WHEN** a searching group drops below one present member
- **THEN** the server removes it from the fill queue and reclaims it (idle/empty-group cleanup)

### Requirement: Members Persist, Guests Are One-Game

A group SHALL distinguish **members** (joined by invite code, or a fresh quick-match
table's founding players) from **guests** (placed by fill). When a game ends, the group
returns to its lobby keeping only its members (plus reconnected members); guest seats are
dropped and the guest's connection returns to the unbound menu.

#### Scenario: The guest is dropped after the game

- **WHEN** a game that included a guest reaches `GameOver`
- **THEN** the group returns to its lobby with only its members, the guest seat is freed, and the guest's connection is in the menu state

#### Scenario: Members return to the same group

- **WHEN** a game ends
- **THEN** the group's members are returned to the group lobby with the group's code and roster of members intact, ready to play again or fill again
