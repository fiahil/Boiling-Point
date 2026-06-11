## ADDED Requirements

### Requirement: The Grimoire Is Fifteen Spells In Two Timing Modes

The grimoire SHALL consist of **15 distinct spells** across five roles — info, volatility control, wards, score, and economy/offense. Each spell is either **Instant** (fires on cast, then spent) or **Active** (primed face-down, fires on its trigger, then spent — an unfired Active is a wasted bet). No single spell SHALL be able to win the game alone.

#### Scenario: An instant spell fires on cast

- **WHEN** a player casts Peek (Instant)
- **THEN** it resolves immediately — the caster learns the boiling point — and is spent

#### Scenario: An active spell waits for its trigger

- **WHEN** a player primes a Ward (Active)
- **THEN** it stays primed until the player would take detonation damage, then fires and is spent

### Requirement: Spell Visibility — Visible When Activated, Else Hidden

A spell SHALL be **hidden** while held in hand and while **primed-but-unfired**, and SHALL become **visible to the whole table when it activates** — an Instant on cast, an Active when it fires. A visible volatility spell reveals its **delta** (e.g. +3), never the cauldron's absolute total, which only Peek reveals — preserving blind volatility.

#### Scenario: A primed ward stays secret until it fires

- **WHEN** a player primes a ward and the round continues without their detonation
- **THEN** the table never learns the ward existed unless and until it fires

#### Scenario: An instant cast is public

- **WHEN** a player casts Surge
- **THEN** the table sees the caster and that Surge fired (its +volatility delta), but not the cauldron's absolute total

### Requirement: Spell Catalog And Roles

The grimoire SHALL provide the following spells, with at least one in each role: **Peek, Expose, Assay** (info); **Dampen, Surge, Quench** (volatility); **Cap, Halve, Redirect** (wards); **Double Down, Sour, Harvest** (score); **Skim, Forage, Hex** (economy/offense). Their effects are defined in design.

#### Scenario: Every role is represented

- **WHEN** the grimoire is assembled
- **THEN** it contains at least one spell in each of the five roles, totalling 15 distinct spells

### Requirement: Cauldron-Level Spell Effects Adjust The Running Total

Volatility spells (Dampen, Surge) SHALL adjust the **cauldron's running volatility total** rather than any single pot card; their adjustment SHALL be reflected in the explosion check but SHALL NOT alter individual ingredients' per-card volatility used by the detonator sort.

#### Scenario: Surge heats the cauldron without owning a card

- **WHEN** a player casts Surge (+volatility)
- **THEN** the cauldron's running total rises by the delta, affecting the explosion check, while no pot card's per-card volatility changes
