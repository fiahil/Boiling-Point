## ADDED Requirements

### Requirement: Two Drafting Ledgers, Picked As Bucket Sets

Deck construction SHALL use two separate ledgers — a **Pantry** and a **Grimoire** — each drafted by selecting a **set of 2–3 named buckets** (one of each type, **no duplicates**). This step SHALL run in the pre-game phase **after** the Brewer pick.

#### Scenario: A player drafts both ledgers

- **WHEN** a player reaches the draft step
- **THEN** they select 2–3 distinct pantry buckets and 2–3 distinct grimoire buckets, with no duplicate bucket in either ledger

### Requirement: Buckets Feed Availability, Not Distribution

A bucket SHALL only make a **family of cards eligible** for the deck; it SHALL NOT set how many of those cards appear. There is **no coin budget and no weighting**. The number of buckets taken SHALL change only the deck's **focus vs breadth**, never its size.

#### Scenario: Bucket count does not change deck size

- **WHEN** one player takes 2 pantry buckets and another takes 3
- **THEN** both receive a full fixed-size pantry; the 2-bucket deck is more concentrated, the 3-bucket deck more varied

### Requirement: The Realizer Composes A Fixed-Size, Capped, Color-Anchored Deck

A server-side **realizer** SHALL build each player's deck from the eligible pool, **re-rolled every game**, to a **fixed size** (pantry 30, grimoire 20) while enforcing all caps: color-anchor **~75% own color**, **toolkit ≤25%**, **Treasure ≤3**, **god-tier ≤2**. Premium caps SHALL be **absolute** (independent of deck size). Any legal pick-set SHALL yield a legal deck.

#### Scenario: Realized deck respects the caps regardless of picks

- **WHEN** a player picks only toolkit and treasure buckets
- **THEN** the realized pantry still holds ~75% own color, ≤25% toolkit, and ≤3 Treasure cards

#### Scenario: God-tier stays absolutely capped

- **WHEN** a player picks both god-tier grimoire buckets (Eyebright + Ironbark)
- **THEN** the realized grimoire contains at most 2 god-tier spells, regardless of the 20-card size

### Requirement: Public Recipe, Hidden Realization

The buckets a player took (their **recipe**) SHALL be **public** to the table. The realized cards and their draw order SHALL be **hidden from everyone, including the owner**, who learns their deck as they draw it.

#### Scenario: The table reads intent, not the hand

- **WHEN** a player finishes drafting
- **THEN** all players can see which buckets they took, but no one (not even the owner) can see the realized cards before they are drawn

### Requirement: The Reserve — One Guaranteed Grimoire Spell

By default a grimoire bucket SHALL roll a **random** spell within its role-group. A player SHALL have **one reserve** to **lock a single named spell** instead; the pantry SHALL always be pure-roll (no reserve).

#### Scenario: Reserve guarantees a specific spell

- **WHEN** a player spends their reserve on Redirect
- **THEN** the realized grimoire is guaranteed to contain Redirect, while the rest rolls within the chosen buckets

### Requirement: The Bucket Rosters

The Pantry SHALL offer **12** buckets (Sage, Mint, Nightshade, Saffron, Chalk, Bilberry, Ochre, Wisp, Bramble, Honey, Hellebore, Embercap) and the Grimoire **8** reagent buckets (Eyebright→Peek; Ironbark→a Ward; Farsight→Expose/Assay; Brimstone→Surge/Hex; Wormwood→Sour/Skim; Goldenseal→Harvest/Double Down; Hoarfrost→Dampen/Quench; Mandrake→Forage).

#### Scenario: Toolkit is optional

- **WHEN** a player takes no toolkit bucket (no Ochre, no Wisp)
- **THEN** their realized pantry is ~100% own color (the pure Loyalist), well within the anchor
