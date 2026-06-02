# agent-personas Specification

## Purpose
TBD - created by archiving change agent-player-harness. Update Purpose after archive.
## Requirements
### Requirement: Persona Shapes Playstyle Bias

An agent MAY be assigned an optional persona (archetype) that biases its playstyle tendencies without granting any capability. Persona and difficulty SHALL be independent axes, so the same archetype can run at any difficulty. The v0 archetypes are Gambler (high-volatility commits), Turtle (cautious, pass-leaning), Bandwagoner (favors the leading color), and Trickster (decoy color early, own color late). With no persona assigned, the agent plays straight.

#### Scenario: Different archetypes diverge at equal difficulty

- **WHEN** a Gambler and a Turtle play at the same difficulty preset
- **THEN** the Gambler tends toward higher-volatility commits and the Turtle toward caution or passing, reflecting their archetypes

#### Scenario: No persona plays straight

- **WHEN** an agent is launched without a persona
- **THEN** it plays to its difficulty preset with no archetype bias applied

### Requirement: Persona-Driven Emote Selection

A persona SHALL express itself only through the server's preset-emote palette — the single `table-talk` communication channel — selecting emotes that fit its archetype. The agent MUST NOT attempt free-text communication (the server rejects it), and emotes carry no mechanical weight.

#### Scenario: Persona emotes in character

- **WHEN** a persona agent reacts to a large pot or a rival's play
- **THEN** it sends an emote drawn from the configured palette that fits its archetype, and never free text

#### Scenario: Emotes never alter game state

- **WHEN** a persona agent sends a valid palette emote during play
- **THEN** the emote is broadcast attributed to the agent and the cauldron, hands, scores, and wave timer are unchanged

### Requirement: Optional Blunder Injection

The harness SHALL support a configurable epsilon probability that, when greater than zero, overrides the agent's chosen action with a uniformly random legal action — a reliable difficulty lever independent of model competence. Epsilon SHALL default to zero (off) in v0.

#### Scenario: Epsilon introduces measurable random play

- **WHEN** an agent runs with epsilon greater than zero over many decisions
- **THEN** a proportion of its actions approximating epsilon are random-but-legal substitutions of its chosen action

#### Scenario: Epsilon zero leaves choices untouched

- **WHEN** an agent runs with epsilon at zero
- **THEN** no chosen action is ever overridden by random play
