# boom2 — the v2 core rework (design corpus)

This chapter groups the **design docs behind the `boom2` core rework** — the v2 work
that turns Boiling Point from a light filler into a deeper, more strategic, more
political game ([why](../02_game-design.md) it changed: the light game playtested as
*too simple*).

**Source-of-truth note.** Now that the rework is captured as OpenSpec changes, those
**changes are the authoritative requirements**; the two docs here are the **design
rationale and the decision record** that produced them (per the repo convention —
[`openspec/`](../../openspec/) holds the contracts, `docs/` holds the human-facing
rationale). When a change is implemented and archived, its rules are promoted into the
canonical [`02_game-design.md`](../02_game-design.md).

## The two design docs

| Doc | Role |
|---|---|
| [01_depth-and-complexity.md](01_depth-and-complexity.md) | **PROPOSAL / rationale** — the exploration of the three core-depth directions (Vote/Spell, Brewers, Ingredients), with the rejected and deferred alternatives. The *why*. |
| [02_toward-a-v2-core.md](02_toward-a-v2-core.md) | **DECISION LOG** — the locked v2 core: the boom model, pantry/grimoire split, the 15-spell grimoire, the Apothecary draft, the 12 Brewers, and the starting numbers. The *what*. |

Reading order: skim **01** for the direction and the alternatives, then **02** for the
committed design. Everything in 02 is tagged `[needs playtesting]` — the harness
re-derives the balance economy. (The §IV at-scale instrument is the **AI client's
harness mode** — [`clients/ai/`](../../clients/ai/README.md), change
[`boom2-ai-client`](../../openspec/changes/boom2-ai-client/), superseding the interim
bot harness that delivered the combat-core derivation (since returned to
`archive/bot-harness/`). Change
[`boom2-benchmarking`](../../openspec/changes/boom2-benchmarking/) folds its runs into
the benchmarking suite as the **balance study** instrument — on-demand, purely
observational — alongside the criterion server benchmarks.)

## Design → the boom2 core changes

The decision log (02) fans out into **four** apply-ready changes under
[`openspec/changes/`](../../openspec/changes/):

| Change | Sourced from (doc 02) | Covers |
|---|---|---|
| [`boom2-combat-core`](../../openspec/changes/boom2-combat-core/) | boom, cards, waves, grimoire | card model, detonator boom, wave loop, 15 spells, depile — on fixed decks |
| [`boom2-brewers`](../../openspec/changes/boom2-brewers/) | C3 — the 12 Brewers | public pick-1-of-2 Brewers + the pre-game brewer step |
| [`boom2-apothecary`](../../openspec/changes/boom2-apothecary/) | O4 — the Apothecary | bucket-draft deck-builder (30/20, availability realizer) |
| [`boom2-compounding`](../../openspec/changes/boom2-compounding/) | O3 — compounding | Honey count-thresholds + Bramble combos |

The live observability counterpart rides alongside:
[`boom2-observability`](../../openspec/changes/boom2-observability/) rebases the
whole operator read surface on the v2 core — span schema v2
([contract](../03_architecture/04_span-schema-contract.md)), the shared
`boom-balance-metrics` definitions (one definition, consumed by both the live
dashboard and the balance studies, targets seeded from doc 02's starting
numbers as `[needs playtesting]`), and the admin **command center** hosting the
balance dashboard, room inspector, and replays behind admin auth. Its
per-feature dashboard panels are phased behind `boom2-brewers` /
`boom2-apothecary` / `boom2-compounding`.

## Related

- [`../02_game-design.md`](../02_game-design.md) — the canonical (current) rules this rework replaces.
- [`../../CLAUDE.md`](../../CLAUDE.md) — the constitution the rework is checked against.
