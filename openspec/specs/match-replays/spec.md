# match-replays Specification

## Purpose
TBD - created by archiving change persistence-and-replays. Update Purpose after archive.
## Requirements
### Requirement: Per-Wave Action Log

The engine SHALL record, for each game, the root seed, the content-config identity (a
fingerprint/version), and the ordered sequence of per-wave player actions (commit, pass,
lock-in, and effect targets where the wire carries them) — sufficient to deterministically
reconstruct the game by re-running the pinned engine version.

#### Scenario: A completed game yields a reconstructible action log

- **WHEN** a game completes
- **THEN** its recorded seed, content-config identity, and ordered action log, re-run through the same engine version, reproduce the identical sequence of waves, reveals, and scores

#### Scenario: Recording is bounded and discarded after the write

- **WHEN** a game is in progress
- **THEN** the action log accumulates only the per-wave actions and is retained no longer than the post-game completion write

### Requirement: Timeless Replay Payload

A game's replay SHALL be encoded as a single, compact, self-describing payload suitable
for one database column, carrying a `format_version`, an `engine_version`, the
content-config identity, the action log, and an integrity hash. A replay SHALL remain
renderable for the lifetime of the product: it is reconstructed by re-running the pinned
engine version, and when the engine changes incompatibly the stored payload is
migrated/re-rendered rather than discarded.

#### Scenario: Replay fits one column and round-trips

- **WHEN** a replay payload is encoded and stored, then loaded and decoded
- **THEN** it fits a single column, its integrity hash verifies, and it reconstructs the same game

#### Scenario: Replay survives an engine change

- **WHEN** the engine version advances incompatibly after a replay was stored
- **THEN** the payload's `format_version`/`engine_version` select a compatible reconstruction path (or a migration re-renders it), and the replay still plays back

#### Scenario: Tampered payload is rejected

- **WHEN** a stored payload's bytes do not match its integrity hash
- **THEN** reconstruction fails with an integrity error rather than producing a wrong replay

### Requirement: Replay Retrieval

A stored replay SHALL be retrievable by game id and reconstruct the full public event
stream of the game (deals, wave reveals, depile, final scores). Because a replay is
post-game, it MAY reveal everything the end-of-round depile already revealed.

#### Scenario: Fetch and reconstruct a replay

- **WHEN** a replay is requested by game id
- **THEN** the server loads the payload, verifies its integrity hash, and reconstructs the game's public event stream for playback

