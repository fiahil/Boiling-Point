## Context

The Apothecary is fully specified in [docs/06_boom2/02](../../../docs/06_boom2/02_toward-a-v2-core.md) (the "Deck-building — The Apothecary" section + O4). It replaces `boom2-combat-core`'s fixed-deck placeholder and depends on `boom2-brewers` for ordering (draft runs after the Brewer pick) and for the Connoisseur/Reservist hooks.

## Goals / Non-Goals

**Goals**
- A fast, procedural, owner-unknown draft: pick 2–3 named buckets per ledger; the realizer rolls the rest.
- Availability-not-distribution model; caps in the realizer; public recipe / hidden realization.
- Pantry 30 / grimoire 20; 12 pantry + 8 grimoire buckets; one grimoire reserve.

**Non-Goals**
- Saved decks / accounts (explicitly out — nothing persists; stays anonymous-session).
- A coin/weight economy or freeform deckbuilding (rejected — see D1).
- Compounding content (Bramble/Honey buckets exist as availability here; their *effects* land in `boom2-compounding`).

## Decisions

### D1: Availability, not distribution — and no coins

A bucket makes a *family eligible*; the realizer decides amounts to a fixed size with caps. This blocks net-decking (you can't min-max a distribution), guarantees a legal right-sized deck from any pick-set, and keeps the draft a fast set-selection. *Alternatives rejected:* coin-budget weighting (a quantity economy — more knobs, solvable optimum); freeform deckbuilder (drags in saved decks → accounts → out of scope).

### D2: Caps live in the realizer, and premium caps are absolute

color-anchor ~75%, toolkit ≤25%, Treasure ≤3, god-tier ≤2 — enforced at realization, so any picks are legal. Premium caps are **absolute**: a bigger deck adds commons, not premium, so god-tier is *proportionally rarer* in the 20-card grimoire (~10%) — the structural Peek-economy protection. The **reserve** guarantees ≥1 god-tier if wanted.

### D3: Public recipe / hidden realization

The buckets are public (intent read at the table — "Red went Nightshade + Saffron = aggressive bruiser"); the realized cards are hidden even from the owner (learn-as-you-draw, C5). This is *better* than a public deck-list: it keeps the luck texture and the political legibility at once.

### D4: Rosters and the archetype space

12 pantry buckets (posture / toolkit / chemistry / specialist groups) and 8 grimoire reagents (incl. two god-tier). Picking 2–3 yields legible archetypes (Warlord = Nightshade+Saffron+Bilberry / Ironbark+Brimstone; Fortress; Kingmaker). The Bramble/Honey buckets are *available* here; their compounding effects ship in `boom2-compounding`.

## Constitution Check

| Principle | Compliance |
|---|---|
| **I — Server-authoritative** | The realizer runs **server-side** and owns the hidden realized deck; the client sends a recipe (bucket set + reserve) and renders public recipes. The server never reveals a player's realized cards. |
| **II — Agent-driven** | Buckets, caps, and the realizer are data + server code. The harness drafts recipes programmatically and sweeps the deck-archetype axis. |
| **III — Start simple** | Procedural buckets, **no saved decks → no accounts** (stays in v1's anonymous-session world); a fast set-selection, not a freeform builder; layered after the validated combat core + Brewers. **Rejected simpler alternative:** keep fixed decks — but that leaves pre-game agency at zero, the exact thinness this targets. |
| **IV — Playtest-driven** | All sizes/caps `[needs playtesting]`; the harness adds the **deck-archetype** axis (persona × Brewer × archetype) and tunes the realizer caps — god-tier ≤2 being the load-bearing Peek-economy dial. |

## Risks / Migration

- **Realizer caps are the balance crux** — especially god-tier ≤2 and Treasure ≤3; the harness owns them.
- **Supersession:** removes `boom-cards`'s fixed-deck deal as the deck source (same saga); fixed decks remain only as a harness fallback/teaching mode if wanted.
- **Pre-game time:** Brewer + two-ledger draft must stay fast (sane defaults, a quick-pick) to honor the lobby ethos.
