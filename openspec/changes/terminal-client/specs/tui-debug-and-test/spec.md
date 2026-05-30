## ADDED Requirements

### Requirement: Debug Overlay

The client SHALL provide a toggleable debug overlay showing connection round-trip
time, inbound/outbound message counts, the current phase and wave, a scrolling
log of raw messages in and out, and a live JSON dump of the client view model.
The overlay SHALL display only data the client legitimately holds and SHALL never
fabricate hidden state.

#### Scenario: Overlay toggles on and off

- **WHEN** the player presses the debug-overlay key
- **THEN** the overlay appears over the current screen, and pressing the key
  again hides it

#### Scenario: Overlay shows only client-known data

- **WHEN** the overlay renders the view model
- **THEN** it shows only fields the client received from the server, with no
  boiling point or opponent hand contents

### Requirement: Deterministic Event Replay

The client SHALL be able to record the received message stream to a file and to
replay a recorded stream into the view model and renderer with no live server,
reproducing the session deterministically.

#### Scenario: Record a session

- **WHEN** recording is enabled during a session
- **THEN** the client writes every received message, in order, to a replay file

#### Scenario: Replay reproduces the session

- **WHEN** the client is launched against a recorded replay file
- **THEN** it reconstructs the same sequence of screens and view-model states
  without connecting to a server

### Requirement: TestBackend Snapshot Tests

The render layer SHALL be exercisable through `ratatui`'s `TestBackend`,
producing deterministic text-buffer snapshots for each screen and phase that
serve as the Layer-3 visual-regression tests.

#### Scenario: A screen renders to a stable snapshot

- **WHEN** a screen is rendered against `TestBackend` for a fixed view-model
  fixture
- **THEN** the produced text buffer matches the committed snapshot, and a render
  change that alters the buffer fails the test

#### Scenario: Snapshots cover the core phases

- **WHEN** the snapshot suite runs
- **THEN** it includes at least the lobby, round-start, playing, depile (safe and
  explosion), scoring, deathmatch, and game-over screens
