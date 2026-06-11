## Why

Playtesting found the v1 "light" game **too simple** ([docs/06_boom2/02_toward-a-v2-core.md](../../../docs/06_boom2/02_toward-a-v2-core.md)). The committed direction is a deeper, more strategic, more political **v2 core** where the deep game *is* the product (not an opt-in mode). This change lands the **combat core** — the new card model, the detonator-only explosion, the wave loop, and the 15-spell grimoire — the foundation every other `boom2-` change builds on.

It ships **first** and on **simple fixed color-anchored decks** (no drafting yet), so the rebuilt blind-volatility economy can be harness-validated in isolation before Brewers, the Apothecary draft, and compounding layer on (Principle III — phase the rework, don't big-bang it).

## What Changes

- **BREAKING (game model):** replaces v1's *shared-deck, everyone-loses-the-pot* core with the v2 model below. The v1 specs it supersedes are listed under Modified Capabilities.
- **Two card types.** **Ingredients** go into the cauldron (color · volatility · points); **spells** are active effects that are **never** in the pot (no points, no volatility of their own). They live in two separate decks.
- **Volatility 0–7** (was 1–3), skewed low; the boiling point rescales accordingly (`[needs playtesting]`, ~20–32).
- **Points score only on colored Votes.** An ingredient played colorless (a wild / go-neutral) contributes volatility but **0** points.
- **Detonator-only explosion.** Only the **detonator(s)** lose **P** (= sum of colored Vote points), not the whole table. The detonator is found by sorting the **fatal wave** ascending by *effective* volatility: the trigger card + every heavier card in that wave **split −P**; equal volatilities are simultaneous. Folding before the fatal wave is safe.
- **Safe-brew scoring** stays winner-takes-all by color-point dominance (Alliance/Commune split, round down) — now driven purely by colored Votes.
- **Wave loop:** each wave you **must play an ingredient or pass** (pass = locked out) and **may** cast **up to 1 spell**. Ingredients **top up to 3** each wave; spells are **hoarded** (drawn at round start, not replenished in-round).
- **The 15-spell grimoire**, **visible when activated, else hidden**, in **Instant** vs **Active** modes.
- **The depile reveals by volatility every round and reveals the boiling point every round** (boom *and* safe), changing v1's "reveal-on-boom-only" rule.
- **Fixed color-anchored decks** stand in for drafting (deferred to `boom2-apothecary`).

## Capabilities

### New Capabilities

- `boom-cards` — the v2 card model: ingredient/spell split, attributes (volatility 0–7, points 0–3), points-on-colored-votes-only, fixed color-anchored decks, and dealing (ingredient top-up-to-3, spell hoarding).
- `boom-wave-loop` — the per-wave action (ingredient-or-pass + optional spell), pass-locks-out, and round termination.
- `boom-resolution` — pot value P and the P-symmetry, detonator identification (fatal-wave volatility sort, split −P), ward interaction, safe-brew dominance scoring, and the volatility-sorted depile that reveals the boiling point every round.
- `boom-grimoire` — the 15 spells, the visible-when-activated rule, and the Instant/Active timing modes.

### Modified Capabilities

<!-- These v1 specs hold the contracts the boom-* capabilities replace. They are
     SUPERSEDED by this saga and retired when boom2 archives; we do not edit them
     in place because v2 is a near-total rewrite, not a tweak, and v1/v2 cannot
     coexist as the core. -->
- `deck-and-dealing` — superseded by `boom-cards` (shared shoe → two color-anchored decks; refill-to-5/round → top-up-to-3/wave + spell hoard).
- `scoring-and-explosion` — superseded by `boom-resolution` (everyone-loses → detonator-only; points-everywhere → points-on-colored-votes).
- `round-engine` — superseded by `boom-wave-loop` (single-card waves → ingredient-or-pass + optional spell).
- `card-effects` — superseded by `boom-grimoire` (8 in-pot effects → 15 active spells, visible-when-activated).

## Impact

- **Protocol crate:** new card/spell/explosion/ depile message shapes (the canonical source the web client's TypeScript types regenerate from).
- **Server engine:** new deal/wave/resolution/explosion logic; the blind-volatility economy is re-derived from scratch by the **revived bot harness** (`archive/bot-harness/`, Principle IV) — the old tuning (boiling point 8–14, vol 1–3) does not carry over.
- **Clients:** the web client (`clients/web/`) renders the new states; depile becomes a volatility-sorted, boiling-point-revealing animation.
- **Adjacent v1 specs touched (follow-up, out of this scope):** `cauldron-modifiers` offsets rescale to the new boiling-point range; `deathmatch`'s "most-volatility = Detonator" now aligns with the main model.
- **Starting numbers** (all `[needs playtesting]`) are recorded in docs/07; this change carries them as the harness's first config, not as final values.
