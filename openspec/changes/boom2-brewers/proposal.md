## Why

In the v1 game every seat is mechanically identical — there is no **player identity** and no **pre-game agency** ([docs/06_boom2/02](../../../docs/06_boom2/02_toward-a-v2-core.md), C3). Brewers give each player a public, asymmetric identity with one bent rule — the *Cosmic Encounter* move, the highest depth-per-complexity addition available to a symmetric political FFA. This change adds the **12 Brewers** and the pre-game brewer pick, on top of `boom2-combat-core`.

## What Changes

- **Each player has a Brewer** — a public, asymmetric identity, known from turn 1.
- **Selection: pick 1 of 2, unique around the table.** Each player is dealt a **disjoint pair** (8 of the 12 Brewers in play per game), so any combination of picks is automatically unique with no draft-order contention; everyone picks simultaneously.
- **A pool of 12** (draw 8 per game) for cross-game variety.
- **Pre-game ordering: Brewer first, then the deck.** The brewer step runs before deck construction so a player builds to suit who they are (the synergy hunt). (Deck *drafting* itself arrives in `boom2-apothecary`; until then the brewer simply precedes the fixed-deck deal.)
- **Each Brewer bends exactly one rule**, hooking a different combat-core system so no two pull the same lever, under the discipline: one readable sentence, **reads for the whole table**, **no free explosions** (half-damage ceiling), **no free perfect information**.

## Capabilities

### New Capabilities

- `boom-brewers` — public asymmetric player identities: the disclosure rule, the pick-1-of-2-unique selection, the design discipline, the 12-Brewer catalog, and the pre-game brewer-before-deck ordering.

### Modified Capabilities

<!-- Brewers bend rules defined by boom2-combat-core capabilities (boom-resolution,
     boom-wave-loop, boom-grimoire, boom-cards). Those live in the same unarchived
     saga, so the bends are specified here as boom-brewers requirements that
     reference those rules, rather than as MODIFIED deltas against not-yet-archived
     sibling specs. -->

## Impact

- **Protocol crate:** brewer identity + the 2-of-pair offer/pick messages; each player's brewer is public state.
- **Server engine:** a pre-game brewer phase (deal disjoint pairs, collect picks); per-brewer rule hooks into the wave/resolution/grimoire/deal logic.
- **Clients:** the TUI renders the brewer pick and shows all four public brewers at the table.
- **Balance (IV):** the harness matrix grows to **persona × Brewer**; the ≥12 must be mutually balanced (no Brewer breaks any persona) before humans play. The two 🌶️-flagged wave/detonator Brewers (Channeler, Lurker, Cinderwright, Alchemist) get the most scrutiny.
