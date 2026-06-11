## MODIFIED Requirements

### Requirement: Per-Wave Action Log

The engine SHALL record, for each game, the root seed, the content-config identity (a fingerprint/version), and the ordered sequence of per-wave player actions under the v2 vocabulary (ingredient commit with its Vote color — colored or colorless — pass/fold, spell cast with its targets where the wire carries them) — sufficient to deterministically reconstruct the game by re-running the pinned engine version.

#### Scenario: A completed game yields a reconstructible action log

- **WHEN** a game completes
- **THEN** its recorded seed, content-config identity, and ordered action log, re-run through the same engine version, reproduce the identical sequence of waves, reveals (including the depile), and scores

#### Scenario: Recording is bounded and discarded after the write

- **WHEN** a game is in progress
- **THEN** the action log accumulates only the per-wave actions and is retained no longer than the post-game completion write

#### Scenario: Engine-v1 replays stay reconstructable

- **WHEN** a replay recorded under engine v1 is reconstructed
- **THEN** it re-runs against its pinned engine version per the existing payload versioning, with no migration of stored payloads
