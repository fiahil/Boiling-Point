# Research: Boiling Point — Complete Game Design

**Feature**: `001-game-design` | **Date**: 2026-05-28

---

## R1: Escalation Mechanism (Multiplier vs. Threshold Tightening)

**Decision**: Replace round multipliers with **threshold tightening only**.

**Rationale**: The spec identifies double-stacking as the core concern — dynamic pot-based scoring already creates natural escalation (players save high-point cards, card counting improves, score pressure mounts). Adding multipliers on top risks making the final round dominate all strategy. Threshold tightening increases explosion *frequency* in later rounds without inflating *magnitude*, producing drama through danger rather than number inflation.

Per Constitution Principle III (Start Simple), a single escalation knob (threshold range) is preferred over two (threshold + multiplier). If playtesting shows late rounds lack sufficient swing, a small multiplier (×1.5 for round 5 only) can be layered on — the threshold tightening provides the foundation either way.

**Threshold schedule** (needs playtesting):

| Round | Threshold Range | Spread | Expected Midpoint |
|---|---|---|---|
| 1 | 10–16 | 7 | 13.0 |
| 2 | 9–15 | 7 | 12.0 |
| 3 | 8–13 | 6 | 10.5 |
| 4 | 7–12 | 6 | 9.5 |
| 5 | 6–10 | 5 | 8.0 |

**Explosion rate estimate**: With average card volatility ~1.7 and ~6–8 cards played per round, total volatility lands around 10–14. Early rounds (midpoint 12–13) survive most play patterns; late rounds (midpoint 8–9.5) explode under moderate aggression. Estimated overall rate: ~35%, within SC-003 target (30–40%). All values subject to bot harness validation.

**Alternatives considered**:
1. Keep multipliers as-is — rejected (double-stacking concern).
2. Remove all escalation — rejected (early-round leads become insurmountable).
3. Multiplier on success only — rejected (asymmetric rules add complexity without clear benefit).
4. Hybrid (tightening + small multiplier) — reserved as a playtesting fallback.

---

## R2: Shield Balance

**Decision**: **Scarcity AND no scoring on success.** 2 Shield cards in the 88-card deck. Shield prevents explosion penalty AND success scoring for the round.

**Rationale**: With shared-catastrophe explosions, Shield is insurance — you bet on explosion happening. If no explosion, you wasted a card slot and get nothing. This makes Shield a pure gamble: high-value when explosion hits (you dodge the penalty everyone else eats), zero-value when the cauldron brews safely. The V2 volatility cost means playing Shield also pushes the pot closer to the explosion you're betting on — a satisfying risk-reward loop.

Scarcity (2 copies in 88 cards) ensures Shield doesn't dominate strategy. Over 5 rounds with 20 cards drawn per round, the expected number of Shields drawn per game is ~2, meaning most games see 0–2 Shields played total.

**Alternatives considered**:
- Scarcity only (still scores on success) — rejected (too powerful — insurance + scoring is dominant).
- No scoring only (many copies) — rejected (frequent Shields dilute the gambling moment).

---

## R3: Deck Composition

**Decision**: 88-card deck with concrete distribution.

### Overview

| Category | Count | Percentage |
|---|---|---|
| Player-colored (plain) | 52 | 59% |
| Wild (plain) | 12 | 14% |
| Special effect | 24 | 27% |
| **Total** | **88** | 100% |

### Player-Colored Cards (52 total — 13 per color)

Each of the 4 colors (Red, Blue, Green, Purple) has identical distribution:

| Volatility | Points 0 | Points 1 | Points 2 | Points 3 | Subtotal |
|---|---|---|---|---|---|
| V1 | 1 | 3 | 2 | 1 | 7 |
| V2 | 1 | 2 | 1 | 1 | 5 |
| V3 | — | 1 | — | — | 1 |
| **Subtotal** | 2 | 6 | 3 | 2 | **13** |

Per-color stats: avg volatility 1.54, avg points 1.23, total volatility 20, total points 16.

### Wild Cards (12 total)

| Volatility | Points 0 | Points 1 | Points 2 | Points 3 | Subtotal |
|---|---|---|---|---|---|
| V1 | 1 | 2 | 2 | — | 5 |
| V2 | — | 2 | 2 | 1 | 5 |
| V3 | 1 | 1 | — | — | 2 |
| **Subtotal** | 2 | 5 | 4 | 1 | **12** |

Wild stats: avg volatility 1.75, avg points 1.17, total volatility 21, total points 14.

### Special Effect Cards (24 total)

| Effect | Copies | Volatility | Points | Color Distribution |
|---|---|---|---|---|
| **Peek** | 4 | 2 | 1 | 1 per color |
| **Dampen** | 3 | 1 (net -1) | 1 | Red, Blue, Green |
| **Volatile Surge** | 2 | 3 (+2 extra) | 1 | Blue, Purple |
| **Shield** | 2 | 2 | 0 | Wild, Wild |
| **Expose** | 4 | 1 | 1 | Red, Green, Purple, Wild |
| **Copycat** | 3 | 1 | 1 | Blue, Green, Purple |
| **Recall** | 3 | 1 | 0 | Red, Green, Wild |
| **Double Down** | 3 | 2 | 0 | Red, Blue, Purple |

Effect stats: avg volatility 1.50, avg points 0.63.

### Color Totals (plain + effect)

| Color | Plain | Effect | Total Cards | Aligned Effects |
|---|---|---|---|---|
| Red | 13 | 4 | 17 | Peek, Dampen, Expose, Recall, Double Down |
| Blue | 13 | 4 | 17 | Peek, Dampen, Volatile Surge, Copycat, Double Down |
| Green | 13 | 4 | 17 | Peek, Dampen, Expose, Copycat, Recall |
| Purple | 13 | 4 | 17 | Peek, Volatile Surge, Expose, Copycat, Double Down |
| Wild | 12 | 4 | 16 | Shield(2), Expose, Recall |
| **Total** | **64** | **24** | **88** | |

### Deck-Wide Statistics

- Total volatility in deck: 141 (avg 1.60/card)
- Total points in deck: 93 (avg 1.06/card)
- Per-round draw: 20 cards (5 per player) → ~32 volatility, ~21 points drawn
- Full deck cycles ~4.4 rounds before reshuffle (not all drawn cards get played)

### Design Rationale

- **Peek at 4 copies**: With no cauldron cues, Peek is the only volatility information source. At 4 copies (~4.5% of deck), roughly one Peek is drawn per round on average, making it scarce enough to be valuable but common enough to appear regularly.
- **V3 cards are rare** (4 plain + 2 Volatile Surge = 6 total, ~7%): High-volatility cards are dangerous and memorable. Scarcity makes them strategic choices rather than routine plays.
- **P3 cards are scarce** (8 plain + 0 effect = 8 total, ~9%): High-point cards are the highest-value targets in the game. Their scarcity creates hand management decisions ("save it or play it now?").
- **Shield is wild-only**: Shield opts out of scoring, so having it wild (no color contribution) is mechanically consistent — it's pure insurance.
- **Each color has 1 Peek**: Symmetric information access across colors.

---

## R4: Effect Card Point Values

**Decision**: Finalized point values for the 5 previously-TBD effects.

| Effect | Points | Rationale |
|---|---|---|
| **Peek** | 1 | Moderate contribution. You're paying V2 for information; P1 means it's not a dead card for scoring. |
| **Dampen** | 1 | Safety card with modest scoring. Net negative volatility (-1) is the main value. |
| **Volatile Surge** | 1 | Pure sabotage. P1 adds just enough to explosion damage to make it sting. |
| **Shield** | 0 | Insurance card. No scoring contribution is consistent with "opts out of round" design. |
| **Expose** | 1 | Information warfare at low cost. P1 keeps it relevant for pot value. |

The 3 already-specified effects remain unchanged: Copycat P1, Recall P0, Double Down P0.

---

## R5: Scoring Edge Cases

### Fractional Splits

**Decision**: **Floor division. Remainder is lost.**

Example: 7 points split between 2 alliance players → 3 each (1 point lost).

**Rationale**: Per Principle III (simplest viable), integer math with floor division. The lost remainder slightly devalues alliances relative to domination, which is a desirable property — it rewards the risk of going for solo dominance over the safety of sharing. The maximum lost amount is small (2 points in a 3-way split of an odd number).

### Zero-Point Pot — Brew

0 total points ÷ any number of players = 0. The dominant player scores +0. Mechanically correct and narratively appropriate: an empty brew produces nothing.

### Zero-Point Pot — Explosion

Everyone loses 0 points. Mechanically correct: an explosion with no payload. This can only happen if every card in the pot was P0, which requires deliberate effort (all political ghosts). It's a valid edge case that resolves naturally.

---

## R6: Wave Limits

**Decision**: **No explicit wave limit.** Rounds end via the three existing conditions: all players locked out, last-player final wave completes, or explosion.

**Rationale**: With 4 players and max 10 cards each, the theoretical maximum is 10 waves. In practice, strategic passing and explosion risk keep rounds to 4–6 waves. An artificial cap adds a rule players must learn without solving a real problem. If playtesting reveals round drag (>8 waves regularly), a soft cap can be added.

**Playtesting trigger**: If more than 10% of rounds exceed 7 waves in bot harness testing, introduce a cap.

---

## R7: Deathmatch Resolution

**Decision**: Brew uses standard color dominance scoring. Explosion results in **shared victory**.

| Deathmatch Outcome | Resolution |
|---|---|
| **Brew (no explosion)** | Standard color dominance scoring — winner takes all pot points. Ties resolved by re-entering Deathmatch. |
| **Explosion** | All tied players share the victory. |
| **All players run out of cards** | All tied players share the victory. |

**Deathmatch threshold**: Same range as Round 5 (6–10).

**No multiplier for Deathmatch**: The Deathmatch is already high-tension from the forced-commit rule (no passing). Adding a multiplier would be gratuitous escalation. The point is to break the tie or accept a shared outcome, not to blow up the scoreboard.

**Rationale**: The shared-catastrophe explosion model means all Deathmatch participants lose equally — the tie can't be broken by explosion damage alone. Rather than introducing a Deathmatch-specific rule (violating Principle III), we accept that explosion = "we all went down together" = shared victory. This creates an interesting dynamic: players in Deathmatch know that explosion means sharing, so the real competition is through brew dominance. Playing cautiously (low volatility) risks letting your opponent dominate the pot; playing aggressively risks the explosion that forces a shared result.

**Alternatives considered**:
- "Most volatility contributed loses" — rejected (introduces a Deathmatch-only rule, complex).
- Color dominance as explosion differentiator — rejected (unclear semantics when everyone loses).

---

## R8: Timer Behavior on Expiry

**Decision**: **Auto-pass (lock out) on timer expiry.**

When a wave timer expires and a player has not committed a card, they are treated as having passed and are locked out for the remainder of the round.

**Rationale**: This is the simplest interpretation of the existing rules ("a player who commits 0 cards passes and is locked out"). A grace period adds network-edge-case complexity. Auto-pass creates urgency — you must decide within the window or lose your opportunity. This strengthens the "simultaneous blind commitment" feel.

---

## Summary of Resolved Questions

| Question | Resolution | Validation |
|---|---|---|
| Multiplier system | Threshold tightening only (no multipliers) | Bot harness: explosion rate per round |
| Shield balance | 2 copies, wild-only, no scoring on success | Bot harness: Shield win-rate correlation |
| Deck composition | 88 cards (52 colored, 12 wild, 24 effect) | Bot harness: strategy diversity, explosion rate |
| Effect point values | Peek 1, Dampen 1, Surge 1, Shield 0, Expose 1 | Bot harness: effect play rates |
| Fractional splits | Floor division, remainder lost | N/A (deterministic rule) |
| Zero-point edge cases | 0÷N=0 for brew; 0 loss for explosion | N/A (deterministic rule) |
| Wave limits | No explicit limit | Bot harness: wave count distribution |
| Deathmatch | Brew = standard scoring; Explosion = shared victory | Playtest feedback |
| Timer expiry | Auto-pass (lock out) | Playtest feedback |
