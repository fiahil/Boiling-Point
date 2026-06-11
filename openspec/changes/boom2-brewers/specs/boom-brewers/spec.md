## ADDED Requirements

### Requirement: Each Player Has A Public Brewer

Every player SHALL have a **Brewer** — an asymmetric identity with one bent rule — that is **public** and known to the whole table from the start of the game.

#### Scenario: Brewers are visible from turn 1

- **WHEN** a game begins
- **THEN** every player's chosen Brewer is shown to all players before the first wave

### Requirement: Selection Is Pick-One-Of-Two, Unique Around The Table

At game start the engine SHALL deal each player a **disjoint pair** of Brewers (8 distinct Brewers across the four players, drawn from the pool of 12) and have each player **pick one** simultaneously. Because the pairs are disjoint, any combination of picks SHALL be unique around the table with no contention.

#### Scenario: Disjoint pairs guarantee uniqueness

- **WHEN** four players are each dealt a disjoint pair and each picks one
- **THEN** all four chosen Brewers are distinct, regardless of who picked which

#### Scenario: Cross-game variety

- **WHEN** successive games are set up
- **THEN** the 8 Brewers offered are drawn from the pool of 12, so the available identities vary between games

### Requirement: Brewer Effects Obey The Design Discipline

Each Brewer SHALL bend **exactly one** rule, expressible in one sentence, that creates **reads for the whole table** (not merely a private stat). No Brewer SHALL make explosions free for its owner (**half-damage is the absolute ceiling**) and no Brewer SHALL grant free perfect information (info-Brewers bend the *flow* of information, never reveal the answer for free).

#### Scenario: No free explosions

- **WHEN** a Brewer reduces its owner's explosion cost
- **THEN** the reduction is at most to half damage, and is never full immunity

#### Scenario: No free perfect information

- **WHEN** a Brewer grants information
- **THEN** that information is conditional or partial (e.g. piggybacks on another player's action), never an unconditional reveal of the boiling point from turn 1

### Requirement: The Twelve Brewers

The pool SHALL be these 12 Brewers, each hooking a different combat-core system:

| Brewer | Bent rule |
|---|---|
| Featherhand | In the fatal-wave volatility sort, your cards count as the lowest at their value (you slip out of ties). |
| Cinderwright | When you are a detonator you take half damage — but you can never play a Ward. |
| Connoisseur | You draft a 4th bucket in one ledger. |
| Reservist | Your grimoire holds two reserves (lock two exact spells). |
| Channeler | You may play two spells per wave. |
| Forager | You top up ingredients to 4 each wave. |
| Herbalist | Your named combos fire from a single half. |
| Distiller | Your count-threshold cards treat the pot as 2 cards larger. |
| Alchemist | When one of your combos fires it also adds volatility to the pot. |
| Eavesdropper | Whenever anyone casts Peek, you secretly learn the boiling point too. |
| Broker | When you split a pot you round up, not down. |
| Lurker | Once per round you may commit your card after the wave reveals. |

#### Scenario: A Brewer bends its hooked rule

- **WHEN** the Cinderwright is a detonator
- **THEN** they take half of −P and cannot have had an active Ward

#### Scenario: Draft-hook Brewers degrade gracefully before drafting ships

- **WHEN** a draft-hook Brewer (Connoisseur, Reservist) is in play before `boom2-apothecary`
- **THEN** its effect applies once drafting exists; with fixed decks it has no draft to bend and is inert (a known phasing gap, not an error)

### Requirement: Pre-Game Ordering — Brewer Before Deck

The brewer pick SHALL occur **before** deck construction in the start-of-game phase, so a player chooses their deck knowing their Brewer.

#### Scenario: Brewer precedes the deck step

- **WHEN** the start-of-game phase runs
- **THEN** brewer selection completes before the deck (drafted or fixed) is finalized
