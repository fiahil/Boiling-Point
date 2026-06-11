## Why

`boom2-combat-core` ships on **fixed** decks, so pre-game agency is still zero ([docs/06_boom2/02](../../../docs/06_boom2/02_toward-a-v2-core.md), C5/O4). The Apothecary replaces fixed decks with a **fast, procedural, owner-unknown** draft: players curate a small set of named buckets, and a realizer rolls a novel deck they learn as they play. This is the lever that turns a Brewer identity into a *build*, and it converts color access from luck into pre-committed strategy without saved decks or accounts.

## What Changes

- **Two drafting ledgers**, drafted in the pre-game phase **after** the Brewer pick.
- **Buckets feed *availability*, not distribution.** No coins, no weighting: you pick a **set of 2–3 buckets per ledger** (one of each type, no duplicates), and a server-side **realizer** composes a **fixed-size, capped, color-anchored** deck from the eligible pool, **re-rolled every game**. Deck size and legality never depend on how many buckets you took; bucket *count* only trades **focus vs breadth**.
- **Pantry: 30 cards**, drafted from **12** buckets (Sage, Mint, Nightshade, Saffron, Chalk, Bilberry, Ochre, Wisp, Bramble, Honey, Hellebore, Embercap).
- **Grimoire: 20 spells**, drafted from **8** reagent buckets (Eyebright, Ironbark, Farsight, Brimstone, Wormwood, Goldenseal, Hoarfrost, Mandrake), each rolling a random spell within a role-group.
- **Caps live in the realizer:** color-anchor ~75% own, toolkit ≤25%, Treasure ≤3, **god-tier ≤2** — so any legal pick-set yields a legal deck, and premium caps stay **absolute** (bigger decks add commons, not premium).
- **Public recipe, hidden realization:** the buckets you took are public (the table reads your intent); the actual cards and draw order are hidden, even from you.
- **Premium pick:** **one reserve per grimoire** locks a single named spell; everything else rolls within its bucket. Pantry is always pure-roll.

## Capabilities

### New Capabilities

- `boom-apothecary` — the bucket-drafting deck-builder: the two ledgers, the availability-not-distribution model, the 12+8 bucket rosters, the realizer (fixed size, caps, color-anchor, re-roll), public-recipe/hidden-realization, and the reserve.

### Modified Capabilities

<!-- This supersedes the "Fixed Color-Anchored Decks" placeholder requirement
     from boom2-combat-core's boom-cards. Both live in the same unarchived saga,
     so the replacement is specified here (the realizer becomes the deck source)
     rather than as a MODIFIED delta against the sibling spec. -->
- `boom-cards` (same saga) — the fixed-deck deal is replaced by the Apothecary realizer as the source of each player's pantry and grimoire.

## Impact

- **Protocol crate:** the bucket rosters, the per-player recipe (public), and the reserve pick; realized decks stay server-side/hidden.
- **Server engine:** the realizer (compose a fixed-size, capped, color-anchored deck from an eligible pool, re-rolled per game) replaces the fixed-deck deal; a pre-game draft step after the Brewer.
- **Clients:** the draft UI (pick 2–3 buckets per ledger, set a reserve); the table shows each player's public recipe.
- **Balance (IV):** the harness matrix grows to **persona × Brewer × deck-archetype**; the realizer caps (esp. god-tier ≤2) are the structural Peek-economy protection and a primary tuning target.
- **Brewer payoff:** activates the Connoisseur (4th bucket) and Reservist (2 reserves) hooks from `boom2-brewers`.
