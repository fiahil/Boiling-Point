## ADDED Requirements

### Requirement: Protocol Handshake On Connect

On connecting, the client SHALL send `JoinRoom` carrying its supported
`protocol_version` and the player's identity (display name or session token) as
the first message, and SHALL proceed only after `RoomJoined`. On a version
mismatch the client MUST surface the error and MUST NOT enter a game.

#### Scenario: Compatible version proceeds

- **WHEN** the client connects and the server replies `RoomJoined`
- **THEN** the client transitions out of the connecting state into the lobby (or
  the snapshot phase the server reports)

#### Scenario: Incompatible version is surfaced

- **WHEN** the server replies with an `Error` describing a `protocol_version`
  mismatch
- **THEN** the client shows a clear, human-readable message and does not enter
  any game screen

### Requirement: Phase-Driven Screen Routing

The client SHALL render exactly one screen corresponding to the server-reported
phase — lobby, round-start, playing, depile, scoring, deathmatch, or game-over —
and SHALL switch screens when the server advances the phase. The client MUST NOT
advance phases on its own.

#### Scenario: Server advance changes the screen

- **WHEN** the server signals the round has entered the depile
- **THEN** the client switches from the playing screen to the depile screen

#### Scenario: Client never self-advances

- **WHEN** no phase-changing message has been received
- **THEN** the client keeps rendering the current phase regardless of local input
  or timers

### Requirement: Player-Visible View Model Only

The client SHALL construct its entire state from received messages and SHALL
never store, display, or infer-and-reveal information it was not sent — including
cauldron card identities, the exact volatility total, the boiling point (except
when the server reveals it), other players' hands, or other players' commits
before reveal.

#### Scenario: No hidden state is renderable

- **WHEN** any in-round screen is shown
- **THEN** no widget displays a running volatility total, a boiling-point value,
  another player's hand contents, or another player's pending commit

#### Scenario: State derives only from messages

- **WHEN** the client needs to render the cauldron, scores, or contribution
  counts
- **THEN** every value shown traces to a field in a message the server sent to
  this player

### Requirement: Responsive Layout And Minimum Size

The client SHALL render legibly at a terminal size of at least 80×24, SHALL
adapt to terminal resize events, and SHALL show a resize prompt when the
terminal is smaller than the supported minimum.

#### Scenario: Below minimum shows a prompt

- **WHEN** the terminal is smaller than 80×24
- **THEN** the client replaces the game view with a message asking the user to
  enlarge the terminal, and restores the game view when it is enlarged

#### Scenario: Resize re-lays-out

- **WHEN** the terminal is resized at or above the minimum
- **THEN** the client re-lays-out the current screen to the new dimensions
  without losing game state

### Requirement: Reconnection Overlay

When the connection drops mid-game the client SHALL show a reconnection overlay
with the grace-period countdown and a note that the player auto-passes each wave
while absent, and SHALL resume cleanly when the server delivers a
`StateSnapshot`.

#### Scenario: Drop shows the overlay

- **WHEN** the connection is lost during a game
- **THEN** the client shows the reconnecting overlay with a countdown and the
  auto-pass warning

#### Scenario: Snapshot resumes play

- **WHEN** the client reconnects and receives a `StateSnapshot`
- **THEN** it rebuilds the view model from the snapshot and resumes at the
  reported phase, showing any waves it missed as locked-out where applicable

### Requirement: Clean Terminal Teardown

The client SHALL restore the terminal to its prior state — leave raw mode, leave
the alternate screen, show the cursor — on normal quit and on panic.

#### Scenario: Quit restores the terminal

- **WHEN** the user quits the client
- **THEN** the terminal returns to a normal shell prompt with no residual raw
  mode or hidden cursor

#### Scenario: Panic restores the terminal

- **WHEN** the client panics
- **THEN** a panic hook restores the terminal before the process exits so the
  error is readable
