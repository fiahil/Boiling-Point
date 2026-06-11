## Context

Brewers are committed in [docs/06_boom2/02](../../../docs/06_boom2/02_toward-a-v2-core.md) (C3) with the catalog and discipline fixed. This change adds them on top of `boom2-combat-core`, whose systems (the detonator sort, wards, grimoire, wave loop, dealing) are exactly what the 12 Brewers bend.

## Goals / Non-Goals

**Goals**
- Public, pick-1-of-2, unique-around-the-table identities via disjoint pairs (8 of 12 per game).
- The 12 one-rule Brewers, each hooking a different combat-core system.
- The pre-game brewer step, ordered before the deck.

**Non-Goals**
- The Leaked/Secret disclosure variants (Public only; the others are v-next).
- Deck drafting (its Brewer hooks, Connoisseur/Reservist, only fully bite once `boom2-apothecary` ships).

## Decisions

### D1: Public disclosure, disjoint-pair selection

Public is the lowest-risk, proven model and makes the politics richer without bolting on a deduction game. Disjoint pairs make uniqueness automatic and let all four players pick **simultaneously** (fits the auto-start lobby) with no draft-order unfairness. *Alternatives rejected:* a contested shared draft (slower, unfair to late pickers); Secret/Leaked disclosure (a different, riskier product — held for v-next).

### D2: One pool of 12, hooking nine systems

Each Brewer bends a *different* system (detonator sort, wards, draft, spell economy, ingredient hand, compounding ×3, info flow, scoring, wave commitment), so no two overlap. Three hook **compounding** (it's deep enough to carry reliability / threshold-timing / weaponization variants). Four are 🌶️ high-impact (Cinderwright, Channeler, Alchemist, Lurker) and get the most harness scrutiny.

### D3: The discipline is enforced, not just stated

The two hard guardrails (no free explosions beyond half; no free perfect info) are part of the spec, because they protect §7's shared-pain replacement and the Peek economy (§16). The borderline case, Eavesdropper, is *conditional* (learns nothing unless an opponent Peeks) and public (the table avoids feeding it) — flow, not the answer.

## Constitution Check

| Principle | Compliance |
|---|---|
| **I — Server-authoritative** | The server deals the disjoint pairs, collects picks, owns each Brewer's rule hook, and applies it during resolution; the client renders public identities and sends a pick intent. |
| **II — Agent-driven** | Brewers are data + server hooks (source files). The harness drives picks and measures the persona × Brewer matrix headlessly. |
| **III — Start simple** | **Public** disclosure only (Leaked/Secret deferred); a pool of **12**, not a sprawling set; one bent rule each. Layered onto the already-validated combat core rather than shipped with it. **Rejected simpler alternative:** ship no identities (pure symmetry) — but symmetry is exactly the "every seat identical" thinness the rework targets. |
| **IV — Playtest-driven** | All effects `[needs playtesting]`; the harness runs **persona × Brewer** over thousands of games to confirm no Brewer breaks a persona before humans see them; 🌶️ Brewers are explicit targets. |

## Risks / Migration

- **Mutual balance of 12** is a real content cost; the harness gates it.
- **Phasing gap:** Connoisseur/Reservist hook the *draft*, which doesn't exist until `boom2-apothecary`; until then they are inert (documented, not a bug).
- **Readability:** four public Brewers + (later) four public recipes is a lot to read pre-game; the UI must surface them cleanly.
