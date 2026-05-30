## ADDED Requirements

### Requirement: Entry Menu With Three Join Paths

The lobby SHALL offer three ways to enter a game — **quick match** (auto-match
queue), **create room** (receive an invite code), and **join by code** — and
SHALL collect the player's display name before joining.

#### Scenario: Player picks a join path

- **WHEN** the client reaches the lobby and the player selects one of quick
  match, create room, or join by code
- **THEN** the client sends the corresponding request carrying the entered
  display name

#### Scenario: Name is required

- **WHEN** the player attempts to join without a display name
- **THEN** the client prompts for a name and does not send a join request until
  one is provided

### Requirement: Invite-Code Display And Copy

On creating a room the client SHALL display the server-issued invite code (e.g.
`BREW-7K3F`) prominently and SHALL provide a one-key copy of a shareable
join string.

#### Scenario: Created room shows its code

- **WHEN** the server returns an invite code for a newly created room
- **THEN** the client displays the code and a copy affordance

#### Scenario: Copy places a shareable string on the clipboard

- **WHEN** the player presses the copy key
- **THEN** the client copies a shareable join string containing the code to the
  system clipboard

### Requirement: Join By Code With Error Feedback

When joining by code the client SHALL submit the entered code and, if the server
rejects it as unknown, SHALL show an error and keep the player on the entry
screen to retry.

#### Scenario: Unknown code is reported

- **WHEN** the player submits a code the server reports as unknown
- **THEN** the client shows an error and remains on the join-by-code screen
  without entering a room

### Requirement: Seat Roster And Hostless Auto-Start

While in a room the client SHALL show the four seats with their player colors,
names, and occupancy, the current player count out of four, and SHALL communicate
that the game starts automatically at four players with no host and no settings.

#### Scenario: Roster reflects occupancy

- **WHEN** players join or leave the room
- **THEN** the seat roster updates to show each seat's color, name, and
  occupied/empty state and the `N/4` count

#### Scenario: Auto-start is communicated, not triggered

- **WHEN** the room holds fewer than four players
- **THEN** the client shows that the game will start automatically at four and
  offers no manual start action

### Requirement: Queue Waiting State

In quick match the client SHALL show an "assembling table" waiting state until
the server places the player into a room, then transition to the seat roster.

#### Scenario: Waiting then matched

- **WHEN** the player is in the auto-match queue and the server assembles a table
- **THEN** the client transitions from the waiting state to the seat roster for
  the assigned room

### Requirement: Idle Timeout Visibility

The client SHALL display the room's idle-timeout countdown so the player knows a
non-filling room will be reclaimed.

#### Scenario: Idle countdown is shown

- **WHEN** a room sits in the lobby without filling
- **THEN** the client shows the remaining idle time before the room is reclaimed
