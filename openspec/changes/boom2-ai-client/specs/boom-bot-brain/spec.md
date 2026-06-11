# boom-bot-brain

The deterministic heuristic brain: instant, zero-cost, reproducible — the balance harness's workhorse and every seat's timeliness floor.

## ADDED Requirements

### Requirement: Deterministic Given The Seat Seed

Given the same view, decision frame, settings, and seat RNG state, the bot brain SHALL return the same action. All randomness (tie-breaks, blunders) SHALL draw exclusively from the seat's seeded RNG.

#### Scenario: Same seed, same decisions

- **WHEN** two runs present identical views, frames, settings, and seat seeds
- **THEN** the bot brain produces identical action sequences

### Requirement: Covers The Full v2 Decision Surface

The bot brain SHALL answer every decision kind it is delegated: Brewer pick, Apothecary draft, ingredient-or-pass, optional spell casting including target selection, and Active spell priming. It MUST NOT require scripting to function in any mode.

#### Scenario: Delegated draft still works

- **WHEN** a seat-filler bot is delegated the Apothecary draft
- **THEN** the bot brain picks legal buckets and a reserve without host scripting

### Requirement: Archetype Settings

The bot brain SHALL expose an archetype/persona setting selecting among distinct heuristic postures (at minimum: cautious, aggressive, political/diplomatic, and a uniform-random baseline), and the archetypes SHALL make meaningful use of the v2 systems they are named for — spell usage, fold timing, and draft posture differ between archetypes.

#### Scenario: Archetypes behave differently

- **WHEN** a seeded batch runs the same seats with two different archetypes
- **THEN** the aggregate statistics (fold rate, spell fire rate, explosion involvement) differ measurably between the archetypes

#### Scenario: Random baseline exists

- **WHEN** the `random` archetype is selected
- **THEN** the brain chooses uniformly among the frame's legal actions using the seat RNG

### Requirement: Blunder Injection

The bot brain SHALL support an epsilon setting (0..1): with probability epsilon per decision, drawn from the seat RNG, it substitutes a uniformly random legal action for its heuristic choice. Epsilon 0 disables blunders.

#### Scenario: Epsilon zero is pure heuristic

- **WHEN** epsilon is 0
- **THEN** every decision is the heuristic choice with no random substitution

### Requirement: Effectively Instant

The bot brain SHALL answer well inside any wave timer without external calls — it makes no network requests and performs no unbounded computation.

#### Scenario: No external dependencies at decision time

- **WHEN** the bot brain decides
- **THEN** it completes locally without network I/O
