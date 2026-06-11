## 1. Protocol & recipe

- [ ] 1.1 Define the 12 pantry + 8 grimoire bucket identities and the per-player **recipe** (bucket set + reserve) in the `protocol` crate; recipe is public, realized deck is not.
- [ ] 1.2 Define the draft messages (offer buckets, submit recipe, set reserve); regenerate client wire types.

## 2. The realizer

- [ ] 2.1 Map each bucket to its eligible card family (pantry flavors, grimoire role-groups).
- [ ] 2.2 Implement the realizer: from a recipe's eligible pool, compose a fixed-size deck (pantry 30 / grimoire 20), re-rolled each game.
- [ ] 2.3 Enforce caps in the realizer: color-anchor ~75%, toolkit ≤25%, Treasure ≤3 (absolute), god-tier ≤2 (absolute).
- [ ] 2.4 Implement the reserve: lock one named grimoire spell; roll the remainder; pantry stays pure-roll.
- [ ] 2.5 Keep the realized deck server-side and hidden from the owner (deal/learn-as-you-draw).

## 3. Pre-game draft step

- [ ] 3.1 Run the two-ledger draft **after** the Brewer pick; publish each player's recipe to the table.
- [ ] 3.2 Provide sane defaults / quick-pick so the draft stays inside the lobby time budget.
- [ ] 3.3 Wire the Brewer hooks: Connoisseur (4th bucket allowed), Reservist (2 reserves).

## 4. Replace fixed decks

- [ ] 4.1 Switch the deck source from `boom-cards`' fixed decks to the realizer output.
- [ ] 4.2 Keep fixed decks available as a harness/teaching fallback only.

## 5. Clients & harness

- [ ] 5.1 Web client (`clients/web/`): draft UI (pick 2–3 buckets per ledger, set reserve) and a public-recipe readout at the table.
- [ ] 5.2 Bot harness (revived from `archive/bot-harness/`, §IV): programmatic recipes; add the **deck-archetype** axis to the matrix.

## 6. Balance (Principle IV)

- [ ] 6.1 Sweep realizer caps (god-tier ≤2, Treasure ≤3, toolkit ≤25%) across persona × Brewer × archetype.
- [ ] 6.2 Confirm no degenerate archetype dominates; confirm the Peek economy holds under player-shaped supply; record in docs/07.
