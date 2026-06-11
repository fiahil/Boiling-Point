## Context

The full v2 design is locked in [docs/07_toward-a-v2-core.md](../../../docs/07_toward-a-v2-core.md) (rationale in [docs/06](../../../docs/06_depth-and-complexity.md)). This change implements the **combat core** only — the new card model, explosion, wave loop, and grimoire — on **fixed color-anchored decks**, so the rebuilt blind-volatility economy is harness-validated before Brewers (`boom2-brewers`), drafting (`boom2-apothecary`), and compounding (`boom2-compounding`) layer on. The server stays the single source of truth (§I); clients render the new states.

## Goals / Non-Goals

**Goals**
- The new pot economy: ingredients vs spells, volatility 0–7, points-on-colored-votes-only.
- The detonator-only explosion with the fatal-wave ascending-volatility sort and split −P.
- The wave loop (ingredient-or-pass + optional 1 spell, top-up-to-3, spell hoard).
- The 15-spell grimoire with the visible-when-activated rule.
- The volatility-sorted depile that reveals the boiling point every round.
- A harness config carrying the starting numbers, and a re-derived explosion-rate target.

**Non-Goals**
- Deck **drafting** (the Apothecary) — fixed decks stand in (→ `boom2-apothecary`).
- **Brewers** (→ `boom2-brewers`) and **compounding** combos/thresholds (→ `boom2-compounding`).
- Rebalancing `cauldron-modifiers` offsets / `deathmatch` beyond what the new ranges force (follow-up).
- Final numbers — all values here are `[needs playtesting]` first guesses for the harness.

## Decisions

### D1: The P-symmetry — one stake, two recipients

`P = Σ colored Vote points`. Safe brew → dominant color **+P**; explosion → detonator(s) **−P**; absent players **0**. Volatility's only jobs are the **trigger** (when it blows) and **who pays** (the sort). *Alternative rejected:* volatility- or overshoot-scaled blast (decouples prize from bomb) — unnecessary once the *who-pays* direction does the work, and it loses the clean "win it or pay it" symmetry.

### D2: Detonator = the heavy cards in the fatal wave

Only the fatal wave's cards are sorted (ascending, by **effective** volatility, on the hidden pre-wave base); trigger + heavier split −P; equal volatilities are simultaneous. *Alternative rejected:* whole-pot sort — it would keep early aggressors liable (an anti-Vulture property) but blurs causality; we chose immediacy and sharper per-wave decisions, accepting that folding before the fatal wave is safe.

### D3: Spells are active effects, never in the pot

Pulling effects out of the cauldron makes the spell cost an **action-economy** cost (you cast *instead of* coloring the pot is **not** required — you may do both, but spells are scarce: hoarded, drawn at round start, ≤1/wave). Visibility: **visible when activated, else hidden** — primed Actives are the bluff layer; Instants are public reads; deltas show, absolute totals don't (blind volatility holds).

### D4: The 15 spells

| Spell | Role | Mode | Effect (`[needs playtesting]`) |
|---|---|---|---|
| Peek | Info | Instant | Learn the exact boiling point. |
| Expose | Info | Instant | Reveal one face-down pot ingredient to the table. |
| Assay | Info | Instant | Privately learn the dominant color + its point lead. |
| Dampen | Vol | Instant | −3 cauldron volatility. |
| Surge | Vol | Instant | +3 cauldron volatility. |
| Quench | Vol | Instant | Cauldron cannot explode next wave (table-wide). |
| Cap | Ward | Active | As a detonator, eat at most −3. |
| Halve | Ward | Active | As a detonator, eat −½P. |
| Redirect | Ward | Active | As a detonator, shove −P onto a chosen player (cascades). |
| Double Down | Score | Instant | Double one color's points in the pot. |
| Sour | Score | Instant | Halve one chosen color's points in the pot. |
| Harvest | Cash-in | Active | If your color wins the pot, +3 bonus. |
| Skim | Economy | Instant | Discard your last-added ingredient (its points + volatility leave). |
| Forage | Economy | Instant | Draw 2 spells. |
| Hex | Offense | Active | A chosen player takes +5 extra damage on any explosion this round. |

### D5: The depile is now a resolution + reveal step

Reveals volatility-ascending every round and reveals the boiling point every round (changing v1 reveal-on-boom-only). On boom it also marks the crossing and the liable cards; wards/Hex/Harvest fire and are narrated here.

### D6: Starting numbers (first harness config)

Volatility 0–7 (mean ~3); boiling point ~20–32 (the load-bearing dial); points 0–3 → P≈10; 5 rounds; wave timers ~25s/~15s; **fixed** pantry 30 (~75% own), grimoire 20 spells, ingredient top-up 3, spells drawn 3/round (hoarded); explosion-rate target ~45% (higher than v1's 30–40% — only the detonator suffers). All `[needs playtesting]`.

## Constitution Check

| Principle | Compliance |
|---|---|
| **I — Server-authoritative** | All deal/wave/explosion/scoring/spell resolution is server-computed and validated; clients send intents (play ingredient / cast spell / pass) and render revealed state. The server never leaks the boiling point or pot contents except via Peek and the depile. |
| **II — Agent-driven** | All artifacts are source files. The protocol-bot harness and Claude-as-player harness drive the new loop headlessly; the TUI renders it for the visual layer. |
| **III — Start simple** | This is the smallest *playable* slice of the rework: **fixed decks** (no drafting), **no Brewers**, **no compounding** — those are separate later changes. **Justification for the rework's size:** v1 tested too simple; the deep core is the product (a documented amendment of the 5–10-min framing). **Rejected simpler alternative:** keep tweaking v1 with more modifiers/effects — raises content volume but not the skill ceiling or player identity, which is where the depth lives. |
| **IV — Playtest-driven** | Every number is `[needs playtesting]`; the bot harness re-derives the entire blind-volatility economy (boiling point ≈26 is the first sweep) and the ~45% explosion-rate target before human playtests. |

## Risks / Migration

- **The Vulture returns.** Detonator-only + fatal-wave-only makes folding before the fatal wave fully safe. Counter-pressure: you can't win without contesting pots; the round-end rules force resolution. **Harness must confirm rounds don't freeze.**
- **Peek economy is rebuilt.** Peek is now fired-less (costs a wave action) and (later) player-shaped; this change sets a fixed Peek supply and the harness re-derives blind-volatility tuning from scratch.
- **v1 supersession.** `deck-and-dealing`, `scoring-and-explosion`, `round-engine`, `card-effects` are retired when this saga archives; until then v2 is developed alongside but is the intended core.
