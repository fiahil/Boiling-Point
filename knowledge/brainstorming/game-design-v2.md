# Boiling Point — Game Design v2

This document captures the design decisions made during the v2 brainstorming session (2026-05-28), building on the [initial game design](game-design-initial.md). Each section describes what changed from v1, the reasoning, and remaining open questions.

---

## Turn Structure: Simultaneous Single-Card Waves

**Changed from:** Sequential turns (one player at a time, round-robin).

**New design:** Each round is split into waves. In each wave, all players simultaneously and secretly commit 0 or 1 card, then all results are revealed at once.

- **Wave 1 timer:** 30 seconds (players need time to evaluate their new hand).
- **Subsequent wave timers:** 10 seconds (faster decisions as context builds).
- **Commits are hidden** during the timer — no player sees who else has committed until the wave resolves.
- **Pass = locked out.** Committing 0 cards in a wave permanently removes you from the round. This creates commitment pressure: "Do I play a weak card just to stay in?"
- **Last player standing** gets one final wave to play or pass, then the round ends regardless. Small reward for outlasting everyone.
- **Wave results** show: who played, who passed, total cards in pot, but NOT what was played (cards are face-down until end-of-round reveal).

**Why:** Sequential turns have a dead-time problem online — you're watching 2-3 other players place face-down cards with zero information feedback between your turns. Simultaneous waves eliminate dead time (everyone acts every wave) and strengthen the blind-commitment feel.

**Open questions:**
- Is there a maximum wave count per round (e.g., 5)? Or does the round only end when all players are locked out / last-player rule triggers?
- Timer values (30s/10s) are starting points — need playtesting.
- Should uncommitted players when the timer expires be auto-passed (locked out) or given a grace period?

---

## Scoring: Dynamic Pot-Based, Winner Takes All

**Changed from:** Fixed values (Domination +5, Alliance +3, Commune +2).

**New design:** Each card has a **point value (0–3)** in addition to color and volatility. Scoring is determined by the total points in the pot.

### Card Attributes

Every card now has three independent attributes:
- **Color:** One of 4 player colors (Ruby Red, Sapphire Blue, Emerald Green, Amethyst Purple) or Wild.
- **Volatility (1–3):** How much explosion risk the card adds.
- **Points (0–3):** How much the card is worth for scoring.

Points and volatility are **independent** — any combination is possible. A card can be high-points/low-volatility (safe treasure), low-points/high-volatility (pure danger), or anything in between. This creates more varied and surprising cards than a correlated model.

### Color Dominance

**Changed from:** Card count determines dominance.

**New rule:** The color with the highest **total point value** in the pot dominates. Fewer high-value cards can beat many low-value ones.

Example: 2 Blue cards worth 3pts each (6pts total) beat 3 Red cards worth 1pt each (3pts total). Blue dominates despite fewer cards.

### Successful Brew Scoring

**Winner takes all:** The dominant color's player receives ALL points in the pot — every card, every color, including wilds.

| Outcome | Condition | Score |
|---|---|---|
| **Domination** | One color has strictly highest point total | That player takes ALL pot points (×multiplier) |
| **Alliance** | Two colors tied for highest point total | Those two players split ALL pot points equally (×multiplier) |
| **Commune** | Three or more colors tied | Those players split ALL pot points equally (×multiplier) |
| **Absent** | Player contributed 0 cards | +0 |

Wild cards have points but no color. They inflate the pot total without helping any color's dominance. The winner scoops wild points as part of "winner takes all."

### Explosion Penalty

**Changed from:** Detonator -4, Bystander -2, Spectator 0.

**New rule:** On explosion, **EVERY player** (including spectators who contributed 0 cards) loses points equal to the **total pot points** (×multiplier). There is no Detonator role — everyone suffers equally.

Example: Pot has 10 points, round multiplier is ×1.5. Every player loses 15 points.

**Why this model:**
- Kills the Vulture strategy — sitting out doesn't protect you from explosions.
- Makes explosions genuinely catastrophic shared events, not targeted penalties.
- Creates collective risk management: everyone is incentivized to prevent explosions.
- The Saboteur strategy becomes a suicide bombing — blowing up a big pot hurts you just as much.

### Notable Card Dynamics

- **0-point cards** are "political ghosts" — they add volatility (explosion risk) and contribute to color presence, but have no scoring impact and add nothing to the pot's point total (which means they also don't increase explosion damage). Pure political signal with volatility cost.
- **High-point wild cards** are "pot inflators" — they increase the pot value for whoever dominates and increase explosion damage for everyone, but don't help any color win.
- **Playing another player's color** is a deliberate political move: alliance-building, kingmaking, or misdirection. Since any player can draw any color, this happens naturally.

**Open questions:**
- Are the scoring numbers right? With winner-takes-all, a dominated 10-point pot in round 5 (×2) gives one player +20. Is that too swingy?
- Should there be a minimum pot size for scoring to activate? (E.g., if the pot only has 2 points, is it worth the explosion risk?)
- How should fractional splits be handled? (E.g., 7 points split between 2 alliance players = 3.5 each.) Round down? Round up? Keep fractional?
- Should the explosion penalty scale with pot size (as decided) or have a floor/ceiling to prevent extreme swings?

---

## Information Architecture: Blind Rounds, Full Reveals

### No Cauldron Cues

**Changed from:** Rumble at 50% threshold, Glow at 75%.

**New rule:** No warnings at all. Players have **zero information** about the cauldron's volatility state during a round. The only things you know are:
- The threshold range is 8–14
- What cards YOU played (and their volatility)
- How many cards each other player contributed (but not their volatility or points)

**Why:** Pure blind commitment. Every card is a leap of faith. The game feels like Russian roulette with cards — no calculation possible, only gut feeling and card counting. This makes the Peek special effect card the ONLY window into cauldron state, dramatically increasing its value.

**Implication:** The Fog cauldron modifier (deferred) is now redundant in the base game, since there are no cues to remove. Fog would need redesign if modifiers are un-deferred.

### Always Reveal (Dramatic Depile)

**Changed from:** Hidden info on success, full reveal on explosion (brainstorming v1).

**New rule:** After every round (success or explosion), ALL cards in the pot are revealed one by one in reverse order (last-added first). Each card shows:
- Color
- Points
- Volatility
- Special effect (if any)
- Which player played it

**Why:** Creates a dramatic spectacle every round. The depile builds narrative tension: "Oh, THAT was your card... and you played Red even though you're Blue? Traitor!" Feeds directly into next-round politics.

**Card counting is intentional and rewarded.** Players can track every card played across rounds and deduce what's left in the deck. This rewards memory and attention, similar to poker.

**Open questions:**
- How long should each card flip take in the depile animation? (1-2 seconds per card?)
- Should there be a "summary view" after the depile for players who want to study the results?
- On explosion, should the depile highlight the moment volatility crossed the threshold? (E.g., a visual "crack" when the running total exceeds the threshold value.)

---

## Card Colors: Any Color from Shared Deck

**Changed from:** Implicit assumption that players mostly play their own "signature color."

**New rule:** Any player can draw and play any color from the shared deck. A player's hand may contain cards of their own color, other players' colors, and wilds. The deck is shuffled and dealt randomly.

**Why:** Richer politics. Playing another player's color is a deliberate political move:
- Playing someone's color = helping them toward domination (alliance move)
- Playing your OWN color of another player's cards = misdirection
- Playing a rival's color while they're losing = kingmaking

**Implication for strategies:**
- **Aggressor** (flood your color) becomes luck-dependent — you can't guarantee drawing enough of your own color.
- **Alliance** is harder to read — did they play your color to help you, or to set you up for betrayal?
- **Misdirection** is a new viable strategy — play Red cards to make everyone think Red is dominating, then play your actual color in the final wave.

---

## Special Effects: Redesigned for Simultaneous Play

### Effects That Work Unchanged

| Effect | Volatility | Points | What It Does |
|---|---|---|---|
| **Peek** | 2 | TBD | Privately learn exact threshold value. Others see "someone peeked." Result delivered before next wave's timer starts |
| **Dampen** | 1 | TBD | Reduce total cauldron volatility by 2 (net -1 vol). Safety play |
| **Volatile Surge** | 3 | TBD | Add +2 extra volatility on top of base 3 (total 5 effective). A weapon |
| **Shield** | 2 | TBD | Immune to explosion penalty this round. The only protection against shared explosion damage |
| **Expose** | 1 | TBD | Reveal one random face-down card in the pot to all players. Information warfare |

### New Effects (Replacing Mimic, Swap, Amplify)

| Effect | Volatility | Points | What It Does |
|---|---|---|---|
| **Copycat** | 1 | 1 | This card's color becomes the color with the highest total points already in the pot from previous waves. If played in Wave 1 (empty pot), it's wild. Bandwagon effect |
| **Recall** | 1 | 0 | Retrieve one of YOUR OWN previously played cards from the pot back to your hand. You choose which. The Recall card stays in pot. An undo button |
| **Double Down** | 2 | 0 | Doubles the total point value of all cards of its color currently in the pot from previous waves. Huge swing potential, but adds 2 volatility and 0 points of its own |

**Why Mimic, Swap, and Amplify were replaced:** All three assumed sequential turn order (referencing "the last card played" or "your next card"). Simultaneous waves have no "last card" within a wave.

**Open questions:**
- What point values should the 5 original effect cards carry? They need points (0-3) since all cards now have points.
- Should special effects resolve immediately when the wave resolves, or at end of round?
- With no cues, Peek is the ONLY source of volatility info. Should there be more Peek cards in the deck? (Currently ~2-3 in a 90-card deck.)
- **Shield balance:** With everyone-loses-pot-points explosions, Shield is potentially the most valuable card in the game. Options:
  - Limit to 1-2 Shields in the entire deck (scarcity as balance).
  - Shield also prevents scoring on success (you're opting out entirely — a bet on explosion).
  - Both (rare AND no scoring on success).

---

## Round Multipliers

**Status: OPEN — needs further discussion.**

The current multiplier curve from v1:

| Round | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|
| **Multiplier** | ×1 | ×1 | ×1.5 | ×1.5 | ×2 |

### The Tension

With dynamic pot-based scoring, there's already natural escalation built in:
- Players save high-point cards for later rounds → bigger pots → bigger rewards/penalties.
- Card counting improves over time → more informed play → bigger commitments.
- Score gaps widen → desperation play → bigger risks.

The multiplier on top of this **double-stacks escalation**. A 12-point pot in round 5 at ×2 means the winner gets +24 and an explosion costs everyone -24. A single round could decide the entire game, potentially making rounds 1-4 feel pointless.

### Arguments For Keeping Multipliers
- Guarantees late-game drama — a player who dominates rounds 1-3 can't coast to victory.
- Creates the emotional arc: cautious exploration → rising tension → climactic finish.
- Without multipliers, conservative play early could build an insurmountable lead.

### Arguments Against (or for Rethinking)
- Dynamic scoring + hand management already creates natural escalation.
- The explosion model (everyone loses pot points) is already harsh enough to keep things dramatic.
- Double-stacking escalation might make the game too swingy — the final round dominates all strategy.
- The brainstorming v1 noted: "multipliers could be attached to stacking cauldron modifiers" — suggesting rules-based escalation rather than number inflation.

### Options to Explore
1. **Keep as-is.** The multiplier is the simplest escalation tool. If playtesting shows it's too swingy, reduce the values (e.g., ×1, ×1, ×1, ×1.25, ×1.5).
2. **Remove multipliers.** Trust the natural escalation from hand management, card counting, and score pressure.
3. **Apply multiplier only to success, not explosions.** Bigger rewards late, but explosion damage stays consistent. Makes late rounds high-reward rather than high-risk.
4. **Replace with threshold tightening.** Instead of inflating numbers, narrow the threshold range in later rounds (Round 1: 8-14, Round 5: 8-10). More explosions, not bigger explosions.
5. **Hybrid: threshold tightening + small multiplier.** Tighter thresholds create more explosions, small multiplier (×1 → ×1.5 max) amplifies without dominating.

**Decision needed before spec finalization.**

---

## Deck Composition

**Status: OPEN — needs detailed design.**

### Known Constraints
- ~80–100 cards total
- ~60% player-colored (15 per color), ~15% wild, ~25% special effect
- 3 independent attributes per card: color, volatility (1-3), points (0-3)
- 8 special effects distributed across colors and wild

### Open Questions
- What's the exact distribution of volatility values within each color? (Even? Weighted toward low?)
- What's the exact distribution of point values? (Should high-point cards be rarer?)
- How many of each special effect card? (Peek might need more copies given its increased importance with no cues.)
- Are effect cards evenly distributed across colors, or are some effects color-locked?
- What volatility/point values do each special effect card carry?

---

## Tiebreaker: Deathmatch

**Kept from v1 with adjustments for new system.**

If two or more players tie for highest score after the final round:
- Only tied players participate.
- A new hidden threshold is generated.
- Each tied player MUST play one card per wave (no passing).
- The round uses simultaneous single-card waves (same as regular rounds).
- Ends when the cauldron explodes or all tied players run out of cards.

**On explosion:** The Detonator concept doesn't exist in v2 — everyone loses total pot points. In Deathmatch, this means all tied players lose equally on explosion. Need an alternative tiebreaker rule for Deathmatch explosions.

**Open questions:**
- What multiplier does Deathmatch use? (×2? ×3? None?)
- With "everyone loses pot points equally" on explosion, how does Deathmatch determine a loser? Options:
  - Revert to "most volatility contributed loses" for Deathmatch only.
  - Deathmatch uses color domination scoring on explosion too — the player whose color dominated the pot before explosion loses less?
  - Deathmatch simply ends on explosion with all tied players sharing the loss — game ends in shared victory.
- If a tied player has 0 cards at Deathmatch start, they're eliminated immediately.

---

## Room Management

**Unchanged from v1.** Invite links, auto-start at 4 players, no host, no settings, 5-minute idle timeout, anonymous auth.

---

## Reconnection

**Unchanged from v1.** 60-second grace period, auto-pass behavior, full state snapshot on reconnect. In the simultaneous wave model, a disconnected player auto-passes (locked out) for each wave.

---

## Art Direction

**Unchanged from v1.** Arcane Punk Alchemy. See [initial game design](initial-game-design.md) for full details. Key update: the cauldron no longer has "rumble" or "glow" visual states during play (since cues are removed). The cauldron should feel unpredictable and opaque during rounds — tension comes from NOT knowing, with the explosion being a sudden surprise.

---

## Deferred Features

- **Cauldron modifiers** — Fog needs redesign (base game already has no cues). Others (Bountiful Brew, Thin Ice, Deep Cauldron, Residue, Catalyst) may need rebalancing for the new scoring model.
- **Round objectives** — Still relevant for nudging specific game theory dilemmas. The emotional arc (Stag Hunt → Chicken → PD → Kingmaker) currently relies solely on multipliers and human behavior.
- **Simultaneous multi-card waves (D1)** — Could be an advanced mode if D2 feels too constrained.
- **Async queuing (D3)** — Fastest wall-clock time but least interactive. Deprioritized.
- **Spectator mode, replays, chat/emotes, rating/matchmaking, 3-player support** — All still deferred.

---

## Summary of v1 → v2 Changes

| Aspect | v1 | v2 |
|---|---|---|
| Turn structure | Sequential | Simultaneous single-card waves |
| Cauldron cues | Rumble (50%) + Glow (75%) | None — blind volatility |
| Card attributes | Color + Volatility + optional Effect | Color + Volatility + Points (0-3) + optional Effect |
| Points/volatility correlation | Implied | Independent (any combination) |
| Scoring model | Fixed (Domination +5, Alliance +3, Commune +2) | Dynamic — winner takes all pot points |
| Dominance rule | Card count | Color's total point value |
| Explosion penalty | Detonator -4, Bystander -2, Spectator 0 | Everyone loses total pot points |
| Info reveal | Debated (hidden vs revealed) | Always reveal after every round (dramatic depile) |
| Card colors | Signature color implied | Any color from shared deck |
| Pass mechanic | Can pass and play later | Pass = locked out |
| Commit visibility | N/A | Hidden until wave resolves |
| Last player rule | N/A | Gets one final wave |
| Wave timers | 20s per turn | 30s wave 1, 10s subsequent |
| Mimic / Swap / Amplify | Original designs | Replaced: Copycat / Recall / Double Down |
| Card counting | Not discussed | Intentional, rewarded |
| Multipliers | ×1, ×1, ×1.5, ×1.5, ×2 | **OPEN — under discussion** |
