# Feature Specification: Boiling Point — Complete Game Design

**Feature Branch**: `001-game-design`

**Created**: 2026-05-28

**Status**: Draft

**Goal**: Produce the final, complete game design document for Boiling Point — a free-for-all card game where 4 players add ingredients to a shared cauldron, balancing blind push-your-luck volatility against color-based political scoring.

---

## Game Overview

Boiling Point is a 4-player free-for-all card game played over 5 rounds. Each round, players simultaneously commit ingredient cards face-down into a shared cauldron. The cauldron either brews a potion — awarding all points to the dominant color's player — or explodes, punishing everyone. Players navigate blind volatility risk, color-based politics, and shifting alliances to finish with the highest score.

**Core identity:** Blind commitment during rounds. Full transparency after. Commit in the dark, face the light.

---

## Players & Setup

- **Player count:** Exactly 4.
- **Player colors:** Ruby Red, Sapphire Blue, Emerald Green, Amethyst Purple. Assigned at game start.
- **Starting score:** 0 for all players. Scores can go negative. Highest score after 5 rounds wins.
- **Starting hand:** 5 cards dealt from the shared deck at the start of round 1.

---

## Card System

### Card Attributes

Every card has three independent attributes:

| Attribute | Range | Description |
|---|---|---|
| **Color** | 4 player colors or Wild | Determines political allegiance for scoring |
| **Volatility** | 1, 2, or 3 | How much explosion risk the card adds to the cauldron |
| **Points** | 0, 1, 2, or 3 | How much the card is worth for scoring and explosion penalty |

Volatility and points are **independent** — any combination is possible. A card can be high-points/low-volatility (safe treasure), low-points/high-volatility (pure sabotage), or anything in between.

### Card Colors

Any player can draw and play **any color** from the shared deck. A Red player may hold Blue, Green, Purple, and Wild cards alongside their own Red cards. Playing another player's color is a deliberate political move — alliance-building, kingmaking, or misdirection.

Wild cards have points and volatility but no color. They inflate the pot's total value without helping any color's dominance.

### Deck Composition

~80–100 cards total:
- **~60%** player-colored cards (evenly distributed across all 4 colors), no special effect, volatility 1–3, points 0–3.
- **~15%** wild cards, no special effect, volatility 1–3, points 0–3.
- **~25%** special effect cards, distributed across colors and wild.

*Exact distribution of volatility/point values across the deck is TBD — see Open Questions.*

### Hand Management

- At the start of each round, each player is dealt **5 new cards** from the shared deck.
- **Unplayed cards persist** between rounds. Saving a strong card for later is a deliberate strategy.
- **Maximum hand size: 10 cards.** If a player's hand would exceed 10 after being dealt new cards, they must discard cards of their choice down to 10 before the round begins.
- After each round's reveal phase, all revealed cauldron cards go to a **discard pile**. When the draw deck is empty, the discard pile is shuffled to form a new draw deck.

---

## Round Structure

Each round follows this sequence:

### 1. Drafting

- A hidden **volatility threshold** is generated for the round (random integer, 8–14 inclusive).
- Each player is dealt 5 cards from the shared deck (added to any cards kept from previous rounds).
- If any player's hand exceeds 10, they discard down to 10.

### 2. Playing (Simultaneous Waves)

The round is played in **waves**. Each wave:

1. All **active** players simultaneously and secretly choose to commit 1 card or pass (commit 0).
2. All choices are hidden until the wave resolves — no player sees others' choices during the decision window.
3. When the wave resolves (all players committed or timer expires):
   - All committed cards are added face-down to the cauldron.
   - All players see: who played, who passed, total card count in pot — but NOT what was played.
   - The system checks if cumulative volatility exceeds the threshold. If yes → explosion (skip to Reveal). If no → next wave.
4. A player who commits 0 cards (passes) is **locked out** for the remainder of the round. They cannot play again this round.

**Wave timers:** First wave has a longer timer (~30s) for players to evaluate their hand. Subsequent waves are faster (~10s). Exact values subject to playtesting.

**Last player standing:** When only one player remains active, they get **one final wave** to play or pass, then the round ends regardless.

**Round end conditions:**
- All players are locked out (all passed).
- The last-player final wave completes.
- The cauldron explodes (volatility exceeds threshold).

### 3. Revealing (Dramatic Depile)

After every round — whether the cauldron brewed or exploded — all cards are revealed one by one in **reverse order** (last-added first). Each card shows:
- Color
- Point value
- Volatility value
- Special effect (if any)
- Which player played it

This depile is a dramatic moment, building the narrative of the round: who contributed what, who bluffed, who betrayed.

### 4. Scoring

See Scoring System below.

### 5. Next Round (or Game Over)

- If this was not the final round → return to Drafting for the next round.
- If this was the final round → game ends, highest score wins.

---

## Scoring System

### Successful Brew (No Explosion)

**Color dominance** is determined by the **total point value** of each color's cards in the pot. The color with the strictly highest point total dominates.

**Winner takes all:** The dominant player receives ALL points in the pot (sum of every card's point value, including wild cards), multiplied by the round multiplier.

| Outcome | Condition | Score |
|---|---|---|
| **Domination** | One color has strictly highest point total | That player takes ALL pot points ×multiplier |
| **Alliance** | Two colors tied for highest point total | Those two players split ALL pot points equally ×multiplier |
| **Commune** | Three or more colors tied for highest | Those players split ALL pot points equally ×multiplier |
| **Absent** | Player contributed 0 cards this round | +0 |

**Example:** Pot contains Red(3pts) + Red(2pts) + Blue(3pts) + Wild(2pts) = 10 total points. Red has 5pts, Blue has 3pts. Red dominates → Red player takes +10 (×multiplier).

### Explosion

When cumulative volatility exceeds the hidden threshold, the cauldron explodes.

**EVERY player** — including those who contributed 0 cards — loses points equal to the **total pot points**, multiplied by the round multiplier. There is no "Detonator" role. Explosions are shared catastrophes.

**Example:** Pot has 8 total points, round multiplier is ×1.5. Every player loses 12 points.

**Why spectators aren't safe:** This kills the Vulture strategy (sit out, take zero risk). There is no safe seat — you're affected by the group's recklessness whether you participate or not.

### Round Multiplier

All scoring (gains AND penalties) is multiplied by a round factor:

| Round | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|
| **Multiplier** | ×1 | ×1 | ×1.5 | ×1.5 | ×2 |

*(For 7-round games: ×1, ×1, ×1, ×1.5, ×1.5, ×2, ×2)*

*Note: The multiplier system is under active design review. See Open Questions.*

### Tiebreaker: Deathmatch

If two or more players tie for highest score after the final round:

- Only tied players participate. A new hidden threshold is generated.
- Each tied player MUST commit one card per wave — **no passing allowed**.
- Simultaneous wave rules apply (same as regular rounds).
- Ends when the cauldron explodes or all tied players run out of cards.
- If all tied players run out of cards without an explosion → shared victory.
- Deathmatch-specific explosion scoring rules are TBD (see Open Questions).

---

## Push-Your-Luck: Blind Volatility

### No Cauldron Cues

There are **no warnings** about the cauldron's volatility state. No rumble at 50%, no glow at 75%. Players commit blind.

The only information available about cauldron volatility is:
- The threshold range is 8–14 (public knowledge).
- You know the volatility of cards YOU played.
- You can see how many cards others played (but not their volatility values).
- If you used a **Peek** card, you know the exact threshold.

Every card committed is a leap of faith. The explosion, when it comes, is a surprise.

### Card Counting

Card counting is **intentional and rewarded**. Since all cards are revealed after every round, attentive players can track what's been played and deduce what remains in the deck. Over a 5-round game, skilled players build a mental model of the remaining deck composition — which colors, point values, and volatilities are still in play.

---

## Special Effects

### Design Principles

- **Few effects, each memorable.** 8 unique effects in the deck.
- **No effect wins the game on its own.** Effects are nudges, not bombs.
- **Every effect has a cost.** The card still goes in the cauldron with its volatility and points. Playing a special effect card is never "free."
- **Effects should create stories.** "Remember when you peeked and STILL blew it up?" is the goal.
- **All effects resolve when the wave resolves.** In simultaneous play, effects trigger after all cards in a wave are committed.

### Effect Catalog

| Effect | Volatility | Points | What It Does |
|---|---|---|---|
| **Peek** | 2 | TBD | Privately learn the exact threshold value. All other players see "someone peeked." Result delivered before next wave starts. The ONLY source of volatility information in the game |
| **Dampen** | 1 | TBD | Reduce total cauldron volatility by 2 (net -1 volatility). Safety play — use a card slot for defense instead of politics |
| **Volatile Surge** | 3 | TBD | Add +2 extra volatility on top of base 3 (total 5 effective). A weapon. Pushes the pot closer to the edge |
| **Shield** | 2 | TBD | Immune to explosion penalty this round. The ONLY protection against shared explosion damage. (Balance TBD — see Open Questions) |
| **Expose** | 1 | TBD | Reveal one random face-down card in the pot to all players. Information warfare — might expose a hidden alliance or reveal dangerous volatility |
| **Copycat** | 1 | 1 | This card's color becomes the color with the highest total points in the pot from previous waves. If played in wave 1 (empty pot), it's wild. Bandwagon effect |
| **Recall** | 1 | 0 | Retrieve one of YOUR OWN previously played cards from the pot back to your hand. You choose which card. The Recall card stays in the pot. An undo button |
| **Double Down** | 2 | 0 | Doubles the total point value of all cards of its color currently in the pot from previous waves. Huge swing potential |

### Notable Card Dynamics

- **0-point cards** are "political ghosts" — they add volatility but have no scoring impact and contribute nothing to explosion damage (since explosion damage = total pot points). Pure political signal with explosion risk.
- **High-point wild cards** are "pot inflators" — they increase the pot value for whoever dominates and increase explosion damage for everyone, without helping any color win.
- **Peek is king.** With no cauldron cues, Peek is the only window into volatility state. A Peek played in wave 2+ lets you calculate exactly how much room is left before explosion.

---

## Information Architecture

### During a Round

| Information | Visibility |
|---|---|
| Your own hand | Private |
| Other players' hand sizes | Public |
| Cards in the cauldron | **Hidden** (face-down) |
| Who committed / who passed each wave | Public |
| Total card count in cauldron | Public |
| Cumulative volatility | **Hidden** (no cues) |
| Exact threshold value | **Hidden** (unless Peek used) |
| Player scores | Public |
| Round multiplier | Public |

### After a Round (Depile)

| Information | Revealed? |
|---|---|
| Every card's color, points, volatility, effect | **Yes** |
| Which player played each card | **Yes** |
| Exact threshold value | **Yes** |
| Full scoring breakdown | **Yes** |

### Key Design Tension

All information is hidden during the round → maximum bluffing, maximum uncertainty.
All information is revealed after the round → maximum accountability, maximum political fallout.

This creates a game where you act on faith and face consequences in the light. Every round's reveal rewrites the political landscape for the next.

---

## Emergent Strategies

These strategies emerge naturally from the rules — no formal alliance mechanics exist.

| Strategy | Description |
|---|---|
| **The Aggressor** | Plays high-point cards of their own color. Aims to dominate pots. Risk: luck-dependent (can't guarantee drawing your own color) |
| **The Diplomat** | Plays other players' colors to build alliances. Brokers cooperation. Risk: trusting a partner who might betray |
| **The Vulture** | Passes early, avoids commitment. Risk: explosions still hurt even spectators; missing out on positive scoring rounds |
| **The Saboteur** | Plays high-volatility/low-point cards to push the pot toward explosion. Risk: explosions hurt you too |
| **The Kingmaker** | A trailing player who can't win uses their cards to decide who does, by flooding another player's color |
| **The Misdirector** | Plays cards of one color to suggest they're building an alliance, then pivots to their own color in the final wave |

---

## Game Theory Framework

The game produces four distinct emotional textures depending on round context, score state, and player behavior. Each maps to a classic dilemma:

### 1. Prisoner's Dilemma — "The Betrayal"
Two players informally allied on a color split. Either could pivot to domination. If both betray, neither dominates and a third player benefits. **Peaks in late rounds** when multipliers (if kept) amplify the gap between alliance and domination payoffs.

### 2. Chicken — "The Nerve Test"
Multiple players keep committing cards wave after wave, each daring others to pass first. Passing concedes influence. Not passing risks explosion. With blind volatility (no cues), every wave is nerve-wracking. **Peaks when the pot grows large** — big points at stake.

### 3. Stag Hunt — "The Trust Pact"
Both players want to cooperate — the fear is being the sucker who commits while the other pivots. Unlike PD, there's no incentive to betray if you trust your partner. **Peaks early** when alliances are forming and trust hasn't been tested.

### 4. Kingmaker — "The Legacy Play"
A trailing player who can't win still has power: their cards decide who does. **Peaks in the final round** when score gaps are clear and the trailing player chooses a winner.

### Typical Emotional Arc

| Round | Dominant Dilemma | Typical Feel |
|---|---|---|
| 1 | Stag Hunt | Cautious cooperation. "Can I trust you?" |
| 2 | Chicken | Testing boundaries. "How far will you push?" |
| 3 | PD emerges | Stakes rise. Alliances crack. "Should I betray first?" |
| 4 | PD peaks | Betrayals happen. Scores diverge. "You backstabbed me!" |
| 5 | Kingmaker + PD | Trailing players choose winners. Leaders play defense. |

This arc isn't scripted — it emerges from the scoring dynamics and human behavior. No two games feel the same.

---

## Success Criteria

- **SC-001**: A complete 5-round game finishes in under 15 minutes.
- **SC-002**: Individual rounds resolve in 60–90 seconds (including waves and depile).
- **SC-003**: Explosion rate averages 30–40% of rounds (balanced threshold tuning).
- **SC-004**: No single strategy wins more than 35% of games over a large sample.
- **SC-005**: At least 3 of the 4 game theory dilemmas emerge naturally in a typical game.
- **SC-006**: "Boom" moments are memorable — the blind explosion is a surprise, the depile reveal builds narrative.

---

## Open Design Questions

### Multiplier System (Active Discussion)
The multiplier curve (×1, ×1, ×1.5, ×1.5, ×2) may double-stack escalation with the already-dynamic pot-based scoring. Options under consideration:
1. Keep as-is.
2. Remove multipliers entirely (trust natural escalation).
3. Apply multiplier only to success, not explosions.
4. Replace with threshold tightening (narrower range in later rounds → more explosions).
5. Hybrid: threshold tightening + smaller multiplier.

### Shield Balance
With everyone-loses-pot-points explosions, Shield is potentially the strongest card. Options:
- Limit to 1-2 Shields in the entire deck (scarcity).
- Shield also prevents scoring on success (you're opting out entirely — a bet on explosion).
- Both (rare AND no scoring).

### Deck Composition
Exact distribution of colors, volatilities, points, and effects across the deck needs detailed design. Key sub-questions:
- Distribution of volatility values per color (even? weighted toward low?).
- Distribution of point values (should high-point cards be rarer?).
- How many of each special effect? (Peek may need more copies given its importance with no cues.)
- Are effects tied to specific colors, or distributed evenly?
- What point values should the 5 original effect cards carry?

### Scoring Edge Cases
- How to handle fractional splits (e.g., 7 points ÷ 2 alliance players = 3.5). Round? Keep fractional?
- What if the pot has 0 total points (all 0-point cards) and brews successfully? No one scores (0 ÷ anyone = 0).
- What if the pot has 0 total points and explodes? Everyone loses 0 — effectively no penalty.

### Wave Limits
Is there a maximum wave count per round, or do rounds only end via lock-out / last-player rule / explosion?

### Deathmatch
- What multiplier does Deathmatch use?
- How does the "everyone loses pot points" explosion model work as a tiebreaker? If all tied players lose equally, the tie persists.
- Options: revert to "most volatility contributed loses" for Deathmatch only, or use color dominance as the differentiator, or declare shared victory on explosion.

---

## Assumptions

- The initial release targets 4-player games only; 3-player support is deferred.
- Cauldron modifiers and round objectives are deferred — the core game ships without them. The Fog modifier needs redesign (base game already has no cues).
- Scoring values and deck composition are starting points subject to balance tuning through playtesting.
- The art style ("Arcane Punk Alchemy") is established but visual asset creation is out of scope for this spec.
- Spectator mode, replays, chat/emotes, and rating/matchmaking are out of scope.
