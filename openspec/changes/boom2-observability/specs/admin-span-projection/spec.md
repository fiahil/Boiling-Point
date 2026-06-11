## MODIFIED Requirements

### Requirement: Unsampled Rolling Aggregates

The projection SHALL fold **completed** spans into rolling balance aggregates evaluated from the `boom-balance-metrics` definitions (at least the core v2 set: explosion/boom rate, detonator distribution, fold/freeze rate, wave depth and duration, round/game duration, timeout rate, reconnection rate). These aggregates SHALL be computed from the **unsampled** in-process stream — observed before any export sampling — and SHALL NOT be sourced from a sampled exported trace.

#### Scenario: Boom rate counts every completed round

- **WHEN** rounds complete while export sampling is enabled
- **THEN** the explosion-rate aggregate reflects 100% of completed rounds, not the sampled subset

#### Scenario: Aggregates update as spans close

- **WHEN** a `round`/`resolve` span ends
- **THEN** the relevant aggregates incorporate it without querying any external store

#### Scenario: Aggregates use the shared definitions

- **WHEN** the projection folds a completed span into a balance aggregate
- **THEN** the computation comes from the `boom-balance-metrics` definition for that metric id, identical to what balance studies evaluate

### Requirement: Bounded Replay Buffer

The projection SHALL retain recent **completed** games in a bounded in-memory buffer that preserves their span tree (rounds → waves → commits/spell-casts → resolve → depile/score) for wave-by-wave replay. The buffer SHALL evict oldest entries when full, bounding memory.

#### Scenario: A completed game is replayable

- **WHEN** a `game` span ends and is retained in the buffer
- **THEN** the admin surface can replay that game's spans in order, wave by wave, including spell casts and the depile

#### Scenario: Buffer stays bounded

- **WHEN** completed games exceed the buffer's capacity
- **THEN** the oldest retained game is evicted and memory stays bounded

### Requirement: Versioned Span-Schema Contract

The projection SHALL depend on the documented, **versioned** span-schema contract at schema version **2**, enumerating the v2 span names, their hierarchy, and their attribute keys (including the sensitive attributes the reveal reads — the boiling point, committed wave plays, pantry and spell hands, mid-round pot volatility, deck seeds). The schema version SHALL be recorded, and the projection SHALL ignore unrecognized spans/attributes gracefully rather than failing — including the planned pre-game spans (`brewer.pick`, `draft`) until their content changes land.

#### Scenario: Unknown span is ignored

- **WHEN** the projection receives a span name not in its schema version
- **THEN** it ignores the span without error and continues processing others

#### Scenario: Sensitive attributes are documented for the reveal

- **WHEN** the schema contract is consulted
- **THEN** it identifies which attributes carry sensitive game state, so the reveal has a single authoritative source for what it surfaces (these are admin-only and never carried on the player wire)
