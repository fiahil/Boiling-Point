## MODIFIED Requirements

### Requirement: Documented, Versioned Span Tree

**BREAKING** (the game subtree re-bases on the v2 core; `SPAN_SCHEMA_VERSION` bumps 1 → 2): the server SHALL emit a documented span tree whose long-lived spans nest `room.lifetime` → `game` → `round` → `wave`, with leaf spans for the v2 core under the appropriate parent — `commit` (the wave's ingredient-or-pass with its Vote color, colored or colorless), `spell.cast` (the wave's optional spell), `resolve` (pot value P, the fatal-wave volatility sort, the detonator split), `depile` (the every-round volatility-sorted reveal including the boiling point), and `score` — plus the unchanged `ws.message`, `reconnect`, and `db.write` spans, and the documented-as-planned pre-game spans `brewer.pick` and `draft` that land additively (no version bump) with `boom2-brewers` / `boom2-apothecary`. The set of span names, their hierarchy, and their attribute keys SHALL be captured in the single versioned span-schema contract exposed by the server, carrying schema version **2**. Some attributes carry sensitive game state (the boiling point, pantry and spell hands, uncommitted wave plays, mid-round pot volatility, deck seeds); these ride in spans for the admin reveal but are never carried on the player wire.

#### Scenario: The v2 span tree nests as documented

- **WHEN** a game runs a round and a wave within a live room
- **THEN** the `wave` span is a child of the `round` span, with its `commit`, `spell.cast`, and `resolve` leaves under it, and the `round`'s `depile` span records the boiling-point reveal

#### Scenario: The schema contract is the single source of names

- **WHEN** the projection needs a span name or an attribute key
- **THEN** it reads them from the versioned span-schema contract rather than hard-coding strings, and the contract reports schema version 2

#### Scenario: Content changes extend the tree without a bump

- **WHEN** `boom2-brewers` adds the `brewer.pick` span documented as planned in the v2 contract
- **THEN** the schema version stays 2 and existing consumers are unaffected
