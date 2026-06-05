## MODIFIED Requirements

### Requirement: TestBackend Snapshot Tests

The render layer SHALL be exercisable through `ratatui`'s `TestBackend`, producing
deterministic text-buffer snapshots for each screen and phase that serve as the Layer-3
visual-regression tests. Any time-based animation (ambient cauldron motion, depile reveal,
boom) SHALL render deterministically under a fixed/seeded animation clock, and snapshot tests
SHALL render at a pinned animation phase so the buffers are stable.

#### Scenario: A screen renders to a stable snapshot

- **WHEN** a screen is rendered against `TestBackend` for a fixed view-model fixture
- **THEN** the produced text buffer matches the committed snapshot, and a render change that
  alters the buffer fails the test

#### Scenario: Animation renders at a pinned phase

- **WHEN** a screen containing time-based animation is rendered against `TestBackend` with the
  animation clock fixed
- **THEN** the produced buffer is deterministic and matches the committed snapshot regardless
  of wall-clock time

#### Scenario: Snapshots cover the core phases

- **WHEN** the snapshot suite runs
- **THEN** it includes at least the lobby, round-start, playing, depile (safe and explosion),
  scoring, deathmatch, and game-over screens
