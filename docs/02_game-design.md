# Boiling Point — Game Design (Final)

This is the **canonical game design** for Boiling Point — the single source of
truth for the rules, cards, scoring, and balance knobs. It consolidates the
earlier design brainstorms, which are not retained.

Per the [constitution](../CLAUDE.md), every game mechanic, scoring value,
threshold, and card effect is a **hypothesis until validated by playtesting**.
Unvalidated numbers are tagged **[needs playtesting]**. No balance number is
sacred — if the bot harness or structured player feedback says change it,
change it.

---

## 1. Elevator Pitch

Four players take turns secretly tossing ingredient cards into a shared,
unstable cauldron. Every card adds hidden **volatility** toward an unknown
**boiling point** — and carries a **color** (whose interests it serves) and a
**point value** (what the pot is worth). Push the brew too far and it explodes,
and **everyone** at the table eats the loss. Stop in time and the dominant color
scoops the entire pot.

Each card is simultaneously a **vote** (for a color's win), a **risk** (of the
boom), and a **signal** (of your intentions). The whole game is reading the
table, managing a scarce hand across rounds, and deciding — round after round —
whether to cooperate, betray, or push your luck one card too far.

It blends the quick gut-decisions of *Exploding Kittens* with the emergent
politics of *Cosmic Encounter* / *Sheriff of Nottingham*.

> **Status (2026-06-11) — the v2 combat core has shipped (`boom2-combat-core`).**
> The server now runs the deeper v2 core decided in
> [06_boom2/02_toward-a-v2-core.md](06_boom2/02_toward-a-v2-core.md): two card
> types (ingredients vs spells, two per-player decks), points scoring only on
> colored Votes, volatility 0–7 with a harness-derived boiling point of 31–43,
> a **detonator-only** explosion (the fatal wave's trigger + heavier cards split
> −P; §7's everyone-loses is superseded), the ingredient-or-pass + optional-spell
> wave, the 15-spell grimoire (§9's in-pot effects are superseded), and a
> volatility-sorted depile that reveals the boiling point **every** round
> (changing §10's reveal-on-boom-only). Superseded here: §3 (the card), §5 (wave
> contents), §6–7 (scoring/explosion), §9 (effects), §13 (deck composition);
> §§8, 11 (modifiers, deathmatch) survive with the new core, their magnitudes
> pending a rescale. The v2 rules are promoted into this document wholesale when
> the boom2 saga archives and the v1 specs retire; until then the decision log
> and the `openspec/changes/boom2-*` specs are the v2 source of truth.

---

## 2. Format

| | |
|---|---|
| **Players** | 4 (free-for-all). *(3-player support deferred.)* |
| **Rounds** | 5 per game **[needs playtesting]** |
| **Round length** | ~60–90 seconds |
| **Game length** | ~5–10 minutes |
| **Elimination** | None. Scores can go negative; trailing players keep full agency (this is what powers the Kingmaker dynamic). |
| **Player colors** | Ruby Red, Sapphire Blue, Emerald Green, Amethyst Purple |

The winner is the highest score after the final round. Ties are resolved by
**Deathmatch** (§11).

---

## 3. The Card

Every card has **three independent attributes**:

```
┌──────────────────────────┐
│  COLOR                   │   Ruby / Sapphire / Emerald / Amethyst / Wild
│                          │
│  VOLATILITY   1 · 2 · 3  │   how much explosion risk it adds
│                          │
│  POINTS       0 · 1 · 2 · 3 │   what it's worth for scoring
│                          │
│  [EFFECT]                │   ~1 in 4 cards has a special effect (§9)
└──────────────────────────┘
```

Points and volatility are **fully independent** — any combination exists:

| Archetype | Vol | Pts | Role |
|---|---|---|---|
| **Safe treasure** | low | high | the card everyone wants to land safely |
| **Pure danger** | high | low | a weapon — shoves the pot toward the edge for little reward |
| **Political ghost** | any | **0** | adds volatility and color *presence* but no scoring weight; under base rules doesn't increase the pot's value or its explosion damage. Pure signal. *(Bountiful Brew overrides this — see §8.)* |
| **Pot inflator** (wild, high pts) | any | high | fattens the pot for whoever wins it (and the explosion for everyone), but helps no color dominate |

Any player can draw and play **any** color from the shared deck — your hand is a
random mix of all four colors plus wilds. Playing someone else's color is a
deliberate political act (alliance, misdirection, or kingmaking).

---

## 4. The Cauldron & the Boiling Point

- Each round the cauldron starts empty (volatility 0), unless a modifier says
  otherwise (§8, Residue).
- A **hidden boiling point** is drawn secretly each round, in the range
  **8–14** **[needs playtesting]** (modifiers shift this range — §8).
- Cards added accumulate **volatility**. The instant total volatility
  **exceeds** the boiling point → **explosion**.

### Blind Volatility — No Cues

Players have **zero information** about the cauldron's volatility state during a
round. There is no "rumble," no "glow," no gauge. The only things you know:

- The boiling-point **range** (and how active modifiers shift it).
- The volatility of the cards **you** personally played.
- **How many** cards each other player has contributed (not what they were).

Every card is a leap of faith. The only window into the true state is the
**Peek** effect (§9), which makes Peek the single most informationally valuable
card in the game.

---

## 5. Turn Structure — Simultaneous Single-Card Waves

A round is a sequence of **waves**. In each wave, every still-active player
**secretly commits 0 or 1 card**, then all commits reveal at once.

```
        ┌─────────────────────────────────────────────┐
        │  WAVE                                        │
        │                                              │
        │  1. Shared timer counts down                 │
        │     • Wave 1: 30s  (read your new hand)       │
        │     • Waves 2+: 10s  [needs playtesting]      │
        │                                              │
        │  2. Each active player secretly picks         │
        │     0 or 1 card. Commits are HIDDEN —         │
        │     nobody sees who committed until reveal.   │
        │                                              │
        │  3. Reveal: all committed cards drop into the │
        │     cauldron at once (still face-DOWN to the  │
        │     table — see §10). Effects resolve (§9).   │
        │                                              │
        │  4. Table sees: who played, who passed, total │
        │     card count — NOT what was played.         │
        │                                              │
        │  5. Explosion check on the new total.         │
        └─────────────────────────────────────────────┘
```

**Pass = locked out.** Committing 0 cards in a wave permanently removes you from
the round. This creates the core commitment pressure: *"Do I burn a weak card
just to stay in the pot, or fold and lose all influence over it?"*

**Timer expiry = auto-pass = locked out.** If the timer ends and you haven't
committed, you're out for the round — no grace period. (An "are you still
there?" nudge may exist as a *connection-health* prompt, but it never changes
what happens to the pot. This also makes timer-expiry and disconnection the same
clean rule.)

### Round Termination — No Wave Cap

The field only ever **shrinks** (passing locks you out; an empty hand
effectively locks you out), so rounds always terminate without an artificial
cap. A round ends the instant **any** of these occurs:

```
  EXPLOSION            volatility crosses the boiling point → boom
  EVERYONE LOCKED OUT  all remaining active players pass in the same wave
  ONE PLAYER LEFT      the moment only one active player remains, they get
                       EXACTLY ONE more wave to play a card or pass, then the
                       pot settles regardless
```

The **one-player rule** is a pacing guardrail, not a reward: it prevents the
dead-time scenario where three players have folded and one keeps soloing card
after card while everyone watches. The survivor gets a single final say — tip
the color dominance their way, or walk away if the pot's already good (or too
hot) — then it's done. If that last player has an empty hand, the pot simply
settles immediately.

---

## 6. Scoring a Safe Brew — Winner Takes All

If the round ends without exploding, the pot is scored by **color dominance**.

**Dominance is decided by total POINTS, not card count.** The color with the
highest sum of point values among its cards wins. Fewer high-value cards beat
many cheap ones (2 Blue × 3 pts = 6 beats 3 Red × 1 pt = 3).

**The winning color takes ALL points in the pot** — every card, every color,
including wilds.

| Outcome | Condition | Award |
|---|---|---|
| **Domination** | one color has the strictly highest point total | that player takes **all** pot points |
| **Alliance** | two colors tied for the highest point total | those two players **split all pot points equally** |
| **Commune** | three+ colors tied for highest | those players **split all pot points equally** |
| **Absent** | you contributed 0 cards | **+0** (no risk taken, no reward) |

- **Splits round down**, and the scoreboard is **integer-only** (no fractions
  accumulate across the game). A 7-point alliance pays **3 each**; the leftover
  point evaporates. The rounding deliberately makes splitting *slightly* worse
  than the raw math — a thumb on the scale toward the more dramatic choice of
  going for solo domination.
- **No minimum pot.** Even a 1-point pot is scored and worth contesting. Small
  pots create the "cautious round" texture naturally, without a rule forcing it.
- **Wild points** have no color, so they never help a color win — but the
  winner scoops them as part of "winner takes all," and they swell the
  explosion (see §7).
- **0-point cards** contribute color *presence* and volatility but, under base
  rules, add nothing to the pot's value (and so add nothing to explosion
  damage). The lone exception is **Bountiful Brew** (§8), whose +1-per-card
  bonus applies regardless of a card's points.

### Scoring Sequence (with modifiers active)

When scoring modifiers (§8) are in play, the order matters. Determining *who*
wins uses color points only; computing *how much* the pot is worth layers in the
colorless bonus and the multiplier. General rule: **additive bonuses first, then
multipliers.**

```
SAFE BREW
  1. Read per-color point totals from the pot
     • Double Down already resolved in its wave (§9) — its doubling is
       baked into the color totals by now, so there's no conflict
  2. Determine the dominant color from those totals
     • Bountiful Brew's +1/card is COLORLESS — excluded from this step
     • Reversal flips "highest" → "lowest color present in the pot" (§8)
  3. Pot value = sum of all card points
     • + Bountiful Brew (+1 per card)        ← additive bonus
     • × Double Stakes (×2)                   ← multiplier, applied last
  4. Winner(s) take the pot value (Alliance/Commune split, round down)
```

---

## 7. Explosion — Everyone Loses the Pot

On explosion, **every player loses points equal to the total pot points** —
including spectators who contributed **0 cards**. There is no Detonator, no
blame role *(except in the Deathmatch tiebreaker — §11)*; the boom is a shared
catastrophe.

```
  Pot value P  =  sum of card points  + Bountiful Brew (+1/card)  × Double Stakes (×2)

  →  EVERY player: −P
```

(The same value calc as a safe brew, minus the dominance step — so "you lose
exactly what the pot was worth" stays true even under modifiers.)

- **No floor.** A tiny pot that blows costs almost nothing — that's fine. The
  loss is exactly the pot, whatever it's worth. Cheap early explosions are
  information, and they set up the scarier ones.
- **No ceiling.** A monster late-game pot (fattened by modifiers — §8) that
  detonates can crater the entire table for 15–20+ points. This is the
  **signature moment** of the game; capping it would neuter the whole
  escalation system.

**Why this model works:**

- **Kills the Vulture.** Sitting out no longer protects you — folding early
  dodges the *commitment* pressure but not the *blast*.
- **Makes risk genuinely collective.** Everyone is mutually incentivized to
  stop the pot from blowing — which is exactly what makes *choosing* to push it
  (or to bail) a real social decision.
- **Turns sabotage into suicide.** Deliberately blowing a fat pot hurts the
  saboteur as much as anyone.

---

## 8. Cauldron Modifiers — The Escalation Engine

**There are no round multipliers.** Escalation comes from **stacking cauldron
modifiers**, so that later rounds don't just have *bigger numbers* — they're a
*different game*. No two games draw the same modifier sequence, so no two games
arc the same way.

### Mechanics

- **Round 1 is always clean** (no modifier) — players learn the table first.
- At the **start of each subsequent round (2–5)**, one modifier is drawn from
  the pool and **revealed**. Reveal is at round start — pure reaction, no
  advance planning.
- Modifiers **stack cumulatively**: round 2 has 1 active, round 3 has 2, …,
  round 5 has 4. All active modifiers sit as icons by the cauldron; the newest
  is the surprise.
- The pool is **~20 cards across 6 modifier types** **[needs playtesting]**,
  weighted so dramatic modifiers are rarer. You might draw Reversal in round 2,
  or never.

### The Six Modifiers

Each does exactly **one thing** — one sentence, one icon, instantly readable.

| Modifier | Type | Effect | Intensity | Copies |
|---|---|---|---|---|
| **Residue** | Physics | Cauldron starts with **+3 volatility** | Mild | 4 |
| **Thin Ice** | Physics | Boiling point **−4** (explosions far more likely) | Medium | 4 |
| **Bountiful Brew** | Scoring | **+1 point to the pot per card** played (colorless — swells pot value & explosion damage, does **not** shift color dominance) | Medium | 4 |
| **Deep Cauldron** | Physics | Boiling point **+4** (explosions rare) | Medium | 3 |
| **Double Stakes** | Scoring | **All pot points ×2** — the win *and* the explosion loss | Spicy | 3 |
| **Reversal** | Scoring | The **lowest**-point color **present in the pot** wins instead of the highest | Wild | 2 |

*(All values **[needs playtesting]**.)*

**Reversal details:** "lowest" is evaluated **only among colors actually present
in the pot** — never an absent color (which would award the win to a color
nobody played and evaporate the pot). The player who committed the *fewest*
points still scoops the whole "winner takes all" pot — that's the intended
chaos. Edge cases fall out of the normal rules: a single color present is both
highest and lowest (Reversal is a no-op); a tie for lowest splits via the usual
Alliance/Commune rules (§6). Reversal is evaluated on **final per-color totals**,
after Double Down (§9) — Bountiful Brew's colorless bonus doesn't affect it.

### Stacking Composes Cleanly

Because every modifier is a single offset or multiplier, stacking — **including
contradictions** — just composes. No special-case rules:

```
  Thin Ice (−4) + Deep Cauldron (+4)   →  net 0, back to base range. They cancel.
  Thin Ice + Thin Ice (−8)             →  boiling point ~0–6. Apocalyptic.
  Bountiful Brew ×2                    →  +2 points per card. A greed engine.
  Double Stakes + Bountiful Brew       →  a huge, point-rich pot, doubled both ways.
  Reversal + Reversal                  →  double negative — highest color wins again.
```

"Two of the same stack; opposites cancel" is grokked in a single game. **Weird
combinations are a feature** — three stacked modifiers can produce states no one
specifically designed.

### The Arc Emerges From the Draw

Removing multipliers didn't kill the emotional arc — it made it *organic*. Each
modifier naturally evokes a classic dilemma, and the stack you happen to draw
shapes each round's feel (see §12). An illustrative game:

```
  Round 1   clean              → Stag Hunt   "Can I trust you?"
  Round 2   + Bountiful Brew    → Chicken     "Pile in... but it's getting hot"
  Round 3   + Thin Ice          → Chicken++   "Every card is a dare now"
  Round 4   + Deep Cauldron     → PD          "Threshold's safe again — politics decide it"
  Round 5   + Double Stakes      → Kingmaker   "Everything's doubled. Who do I crown?"
```

**Reversal is the chaos agent** — a player who's spent three rounds building Blue
dominance suddenly watches Blue *lose* the round. Alliances scramble in an instant.

---

## 9. Special Effects

Design principles (unchanged from v1): few effects, each memorable and
recognizable by icon; no effect wins the game alone; every effect still costs a
card slot and adds its volatility to the pot; effects should create stories.

### Catalog

All cards now carry points and volatility, so effects do too. **All values
[needs playtesting].**

| Effect | Icon | Vol | Pts | What It Does |
|---|---|---|---|---|
| **Peek** | Eye | 2 | 1 | Privately learn the exact boiling point. Others see only *"someone peeked."* The only window into volatility. |
| **Dampen** | Snowflake | 1 | 0 | Reduce total cauldron volatility by 2 (net −1). A safety play. |
| **Volatile Surge** | Lightning | 3 | 0 | Adds +2 extra volatility on top of its base 3 (5 effective). A weapon. |
| **Shield** | Shield | 2 | 0 | You take **no explosion penalty** this round — **but if the pot resolves safely, you forfeit ALL scoring for the round** (you opted out entirely). See balance note below. |
| **Expose** | Lantern | 1 | 0 | Reveal one random face-down card in the pot to the whole table. Information warfare. |
| **Copycat** | Mirror | 1 | 1 | This card's color becomes the highest-point color already in the pot from previous waves. Wild if the pot is empty. Bandwagon. |
| **Recall** | Hook | 1 | 0 | Retrieve one of **your own** previously-played cards from the pot back to your hand (you choose which). The Recall card itself stays in the pot. An undo button. |
| **Double Down** | Dice | 2 | 0 | Doubles the total point value of all cards of **its color** already in the pot from previous waves. Huge swing; adds 2 volatility and 0 points of its own. |

### Resolution — Immediate, with a Fixed Order

Effects resolve **immediately when their wave reveals** (not deferred to
end-of-round) — most effects exist to inform or reshape the *next* wave, so
deferral would break them. When multiple effects land in one wave, they resolve
in this fixed order against a settling pot:

```
  1. Cards added to pot          (colors, points, volatility all land)
  2. Volatility modifiers        Dampen (−), Volatile Surge (+)
  3. Color / identity effects    Copycat (adopt dominant color)
  4. Point effects               Double Down (multiply a color's points)
  5. Removal effects             Recall (pull your card back)
  6. Information effects         Peek, Expose (read the SETTLED pot, reported last)
  7. Explosion check             one check, on the true post-effect total
```

### Visibility — Silent by Default

Effects are **silent**: that an effect was played is revealed only at the
end-of-round depile, like every other card. This protects **blind volatility**
(§4) — announcing Dampen or Volatile Surge would leak that the pot just got
cooler/hotter, quietly reintroducing the cues we removed.

Three exceptions, all unavoidable or intentional:

| Effect | Visibility | Why |
|---|---|---|
| **Peek** | **Announced** — *"someone peeked"* (anonymous; not what they saw) | The paranoia is the point: someone knows something you don't. |
| **Expose** | **Announced** — reveals a card to the table | Public information warfare *is* its function. |
| **Recall** | **Partially visible** — the public contribution count drops by one | A card leaves the pot, and per §10 contribution counts are public. You can't hide that *a* card was reclaimed (though not which, or by exactly whom beyond the count change). A readable "I'm backing out" tell. |

Dampen, Volatile Surge, Copycat, and Double Down are fully silent until the depile.

*Minor known leak:* since "who played a card this wave" is public (§5) while Peek's
announcement is anonymous, the table can sometimes deduce *who* peeked if few
players acted that wave. Accepted — it's rare, low-stakes, and consistent with
the game's "your actions are readable" texture.

### Distribution & Balance Notes

For a ~90-card deck, ~25% are effect cards (~22 cards) **[needs playtesting]**:

| Effect | Copies | Note |
|---|---|---|
| Peek | **4** | **Primary balance knob.** With no cues, this is the *only* volatility info. Too few → pure roulette; too many → erodes the blind-commitment tension. Start at 4. |
| Shield | **1–2** | Deliberately rare (see below). |
| Dampen | 3 | |
| Volatile Surge | 3 | |
| Copycat | 3 | |
| Recall | 3 | |
| Expose | 2 | |
| Double Down | 2 | |

**Shield balance (the strongest card).** Under "everyone loses the pot,"
unconditional immunity would invert the core incentive — a shielded player
would *want* to blow a fat pot, since the boom hurts only their rivals. So Shield
carries a **double cost**: it's **rare (1–2 copies)** *and* it **forfeits all of
your scoring for the round if the pot resolves safely** (and it still adds its
own volatility to the pot). This turns Shield into a genuine **bet on the
explosion**: play it and you're declaring "this pot is going to blow, and I'm
sitting out the scoring to be safe when it does." Right → you're the only one
unhurt. Wrong → you wasted a card and scored zero in a round everyone else
profited from. (The forfeit applies to the **whole round** once played, so *when*
you shield matters.)

---

## 10. Information Architecture

### During a Round

| Information | Visible? |
|---|---|
| Your own hand | Private (only you) |
| Other players' hand sizes | Public |
| Cards in the cauldron | **Hidden** (face-down — color, points, volatility, effect all unknown) |
| How many cards each player contributed | Public — the key political signal: you see *who* played, not *what* |
| Exact volatility total | **Hidden** (only Peek reveals the boiling point) |
| Boiling point value | **Hidden** (range + active modifiers known) |
| Player scores | Public — everyone sees who's desperate, who's leading |
| Active modifiers | Public |
| Wave commits | **Hidden** until the wave resolves |
| That a special effect was played | **Hidden** by default — revealed at the depile (§9). Exceptions: Peek announces *"someone peeked"* (anonymous), Expose reveals a card, and Recall is inferable from the public contribution count dropping. |

### After Every Round — The Depile (Always Reveal)

After **every** round, success *or* explosion, all cards in the pot are revealed
one by one in **play order** (first-added first). Each flip shows color,
points, volatility, any effect, **and which player played it**, while the running
volatility climbs toward the boiling point like a lit fuse.

```
  …first card…→ flip → "…you started the whole mess."
  …            → flip → "You played Red — and you're Blue. Traitor."
  …last card……→ flip → "…and THAT was the one that tipped it over. Boom."
```

The depile is the game's dramatic spectacle and its sole information reveal. It
feeds directly into next-round politics, and — crucially — it makes **card
counting intentional and rewarded.** Players can track every card revealed across
rounds and deduce what's left in the deck, poker-style. Memory and attention are
a skill the design wants to pay off.

**Revealing the boiling point — on boom only.** On an **explosion**, the exact
boiling-point value is revealed, and the depile visually marks the moment the
running volatility total crossed it — the "crack" where it tipped over (the
satisfying "you needed 9, you hit 10" sting). On a **safe brew the boiling point
stays hidden** — you only learn the total stayed under it. This keeps Peek (§9)
valuable for navigating the common safe-brew case, while explosions stay the
game's big information moment.

### Table Talk — Preset Emotes Only

Politics need a signaling channel, but the MVP keeps it minimal and safe:
**preset emotes only** — a small curated palette of expressive icons
(e.g. truce 🤝, scheming 😈, fear 😱, taunt 😂, "watching you" 👀, "you're
done" 💀). No quick-phrases, no free text.

- **Language-neutral** (any international table), **zero moderation surface**,
  and a single tap fits the 10-second waves.
- **Non-binding by design.** An emote carries exactly as much weight as a
  spoken promise at a physical table — *none.* Flinging 🤝 and then dumping
  three cards into the pot is the bluff. The lie is the feature.
- Emotes are the **only** communication channel — **no free-text chat**
  (anonymous strangers + no moderation is the wrong first step).

---

## 11. Deathmatch — Tiebreaker

If 2+ players tie for the highest score after the final round, a **Deathmatch**
decides 1st place. It is **pure elimination** — main-game scores freeze; the
Deathmatch only orders the tied players.

```
  SETUP
    • Only tied players participate; an empty hand at start = eliminated
      immediately (placed last among the tied)
    • Clean slate: fresh hidden boiling point (8–14), NO modifiers
    • Every participant MUST commit 1 card per wave — no passing
    • Same simultaneous-wave timing as normal play
    • Color & points are IRRELEVANT here — only VOLATILITY matters

  RESOLUTION
    EXPLOSION  →  the participant who contributed the MOST volatility to the
                  pot is the "Detonator" and is ELIMINATED
                    • 1 survivor   → champion
                    • 2+ survivors → a fresh Deathmatch among them
                    • tie for most → all tied-for-most eliminated together;
                      if that's everyone → co-champions
                    • Detonator is Shielded → the blast is REDIRECTED to the
                      next-highest volatility contributor (cascade through any
                      further Shields; if everyone remaining is Shielded → no
                      casualty, fresh Deathmatch)
    NO EXPLOSION (hands exhausted)  →  all survivors are co-champions
```

Forced plays against an 8–14 boiling point mean it almost always explodes, so
the Detonator rule is the usual path. The **"most volatility = Detonator"** rule
is the *only* place the removed Detonator concept resurfaces — justified because
it's the one context where the shared-pain model can't produce a result. It's
thematically apt (you cooked it, you fall) and it **rewards whole-game hand
management** — the player who hoarded low-volatility cards for a possible
tiebreaker has a real edge.

**Special effects are allowed in Deathmatch**, and their value swings wildly:

| Effect | Deathmatch power |
|---|---|
| **Shield** | **God-like** — immune to being named Detonator. If the most-volatility player is Shielded, the blast is **redirected to the next-highest** volatility contributor (cascading through any further Shields). You don't just survive — you shove the elimination onto a rival. Consumed on use, and rare. |
| **Recall** | **Nice** — pull back your own high-volatility card to shed Detonator risk. |
| **Dampen / Volatile Surge** | **Useful** — tune the volatility math directly. |
| **Peek** | **Useful** — know exactly how close the edge is. |
| **Copycat / Double Down / Expose** | **Dead** — color and points don't matter here. |

A Shield saved for a possible tiebreaker is effectively a "survive one explosion
free" card — a memorable, *earned* payoff for drafting attention.

---

## 12. Game-Theory Textures (Emergent)

Players never need to know game theory — these feelings emerge from the rules
and the modifier draw. The four classic dilemmas the design aims to evoke:

| Dilemma | Feel | What surfaces it |
|---|---|---|
| **Stag Hunt** — *Trust Pact* | "Can I trust you to commit?" | Early/clean rounds; cooperation is best *if* both follow through. |
| **Chicken** — *Nerve Test* | "Who blinks first?" | Thin Ice / Bountiful Brew / Residue stacks — every card is a dare. |
| **Prisoner's Dilemma** — *Betrayal* | "Should I betray first?" | Deep Cauldron (safe to commit → politics dominate); round-down splits tempting solo domination. |
| **Kingmaker** — *Legacy Play* | "Who do I crown?" | No elimination + negative scores → a trailing player's cards can still decide the winner. Embraced as an emergent social moment, not a formal mechanic. |

### Strategy Archetypes

Fluid, not fixed — players slide between these as the table and modifiers shift:

| Archetype | Play pattern | v2 reality |
|---|---|---|
| **The Aggressor** | Floods one color, dares others to challenge | Now luck-dependent — you can't guarantee drawing your own color. |
| **The Diplomat** | Builds "everyone wins" pots, brokers (breakable) peace | Reading the depile and contribution counts is their craft. |
| **The Vulture** | Sits out, free-rides safe pots | **Largely dead** — explosions hit spectators too. Folding dodges commitment pressure, not the blast. |
| **The Saboteur** | Pushes volatility to blow someone's pot | Now a **suicide bomber** — the boom hurts them equally. |
| **The Misdirector** | Plays a color to fake out dominance, pivots in the final wave | New in v2 — enabled by any-color-from-deck + hidden commits. |

---

## 13. Deck Composition

~80–100 cards total **[needs playtesting]**. A ~90-card starting point:

| Group | Share | Count | Notes |
|---|---|---|---|
| Player-colored | ~60% | ~55 (≈14/color) | volatility 1–3, points 0–3, independent |
| Wild | ~15% | ~13 | points but no color; often higher volatility |
| Special effect | ~25% | ~22 | distributed across colors and wild (§9) |

**Distribution knobs (all [needs playtesting]):**

- **Volatility curve** — suggest skewing slightly low (more 1s and 2s than 3s)
  so the boiling point isn't trivially crossed; the bot harness should tune this
  toward a target explosion rate (~30–40%).
- **Points curve** — suggest high-point (3) cards are rarer, making "safe
  treasure" cards genuinely prized.
- **Hand size & carryover (refill-to-5 floor).** At each round start, top up
  every hand **to 5** — the refill only ever *adds*, it never forces a discard.
  Unplayed cards carry over, so a player who held 5 draws nothing and keeps
  them; one who held 2 draws 3. There is **no hand cap** — if an effect ever
  leaves a hand above 5, those cards are kept, never trimmed. Carryover keeps
  hoarding (a safe card, a Peek, a Shield) a real long-game decision, while the
  refill floor bounds how deep the deck is drawn.
  - *(Under current rules a hand can't actually exceed 5 at round start — cards
    only leave the hand by being played, and Recall is net-neutral — so the
    "no cap" clause is a graceful default, not something that fires today.)*
- **Deck exhaustion → reshuffle.** Refill demand is light (after round 1 the
  table only redraws what it spent), so the deck rarely empties in a 5-round
  game. As a safety net: when the draw deck runs out, the discard pile (all
  previously revealed/used cards) is **reshuffled into a fresh draw deck.** A
  reshuffle of the shared deck is a visible table event, so **card counting
  (§10) operates per-shuffle** — it resets transparently and equally for
  everyone at the reshuffle, exactly like a real card shoe.

The exact distributions are the primary balance surface and the main job of the
**bot harness** (run thousands of games to surface degenerate strategies and a
healthy explosion rate before human playtesting).

---

## 14. Groups, Matchmaking, Reconnection (Reference)

Mostly per prior decisions; see [server-infrastructure.md](03_architecture/02_server-infrastructure.md).

- **Groups:** players **join a group** (by invite link) and then go on **games**
  together; **auto-start at exactly 4 ready players.** No host, no settings,
  always 4 players. A group **persists across games** — after a game it returns to
  its lobby and the same table can **play again** (each seat opts in) without
  re-queuing; it is reclaimed after a 5-minute idle timeout once empty/idle.
- **Members vs guests.** A group holds at most 4 **members** (joined by invite, or
  the founding players of a quick-match table) — they persist and carry the group's
  standings. A **guest** is a player placed by matchmaking **fill** to complete a
  short table for **one game**; the guest is dropped when the group returns to its
  lobby, leaving the members intact.
- **Group fill.** A partial group (1–3 present members) can ask matchmaking to
  **fill** its empty seats — a visible "looking for a 4th…" state until guests
  arrive and the game starts, or a member cancels. The engine is fixed at 4, so a
  short group waits for a guest rather than starting under-strength.
- **Group standings.** Each group keeps a **live, in-memory** tally: per member,
  games played and wins (win-rate derived); plus an aggregate **guest** line so a
  guest's win is recorded against "guests" rather than vanishing. Standings live
  only as long as the group does — they are **not persisted** (durable career stats
  need accounts → roadmap).
- **Session connection:** a client connects once and keeps that connection across
  games and groups — it can join a group, leave back to a menu, and join another
  on the **same socket** (it is not torn down when a game or group ends).
- **Matchmaking — in v1.** A queue that assembles 4-player tables by simple
  fill (next open table / FIFO), **not skill-based** — it both forms fresh tables
  from solo players and **backfills partial groups** with guests. This works fine on
  anonymous sessions — no rating needed. Invite-link groups and the matchmaking
  queue both ship at launch.
- **Auth (v1):** anonymous session tokens (per the server doc) — no persistent
  accounts. The client persists its token and replays it so identity (and a held
  seat) survives a socket drop.
- **Deferred to v2:** player **rating** (FFA needs TrueSkill / Weng-Lin, not
  Elo), **persistent accounts**, and **skill-based matchmaking** that depends on
  them. See [05_roadmap.md](05_roadmap.md).
- **Reconnection:** 60-second grace; a disconnected player **auto-passes (locked
  out)** each wave; full state snapshot on rejoin (only what they're allowed to
  know).

---

## 15. Art Direction (Reference)

**"Arcane Punk Alchemy"** — *Hearthstone* meets *Arcane*. A frantic, slightly
dangerous workshop, not a dusty library. Dark brass/iron base; player colors
(Ruby, Sapphire, Emerald, Amethyst) pop dramatically. Card readability priority:
**Volatility > Color > Points > Effect.**

**Key update for the final design:** with cues removed, **the cauldron has no
"rumble" or "glow" states during play.** It must feel opaque and unpredictable
mid-round — tension comes from *not knowing*, and the explosion is a sudden
surprise. All the cauldron's drama is concentrated into the **depile** and the
**boom**.

---

## 16. Open Balance Knobs (for Playtesting)

Per constitution Principle IV, these are the explicit levers the bot harness and
playtests should tune. None is final.

- Boiling-point base range (8–14) and modifier offsets (±4, +3 Residue).
- Modifier pool size (~20), per-type copies, and the 6 modifiers' magnitudes.
- Wave timers (30s / 10s) and round count (5).
- Per-card volatility and points distributions; high-value rarity.
- Peek count (4) — the knob governing how "knowable" the cauldron is.
- Shield count (1–2) and whether the safe-resolution forfeit is the right cost.
- Effect-card counts and their individual vol/points values.
- Target explosion rate (~30–40%) the deck/threshold should produce.

---

## 17. Summary of Decisions

| Aspect | Decision |
|---|---|
| Players / rounds | 4-player FFA, 5 rounds, no elimination |
| Turn structure | Simultaneous single-card waves; commits hidden until reveal |
| Wave timers | 30s (wave 1), 10s (subsequent) |
| Pass | = locked out; timer expiry = auto-pass = locked out (no grace) |
| Round end | Explosion · everyone locked out · one-player-final-wave rule (no wave cap) |
| Card attributes | Color + Volatility (1–3) + Points (0–3), independent; any color from shared deck |
| Cauldron cues | **None** — blind volatility; Peek is the only window |
| Dominance | Highest **total points** of a color wins |
| Safe scoring | **Winner takes all** pot points; Alliance/Commune split equally, **round down**, integer-only; no minimum pot |
| Explosion | **Everyone** loses total pot points; **no floor, no ceiling** |
| Escalation | **No multipliers** — **stacking cauldron modifiers** instead (6 types, ~20 cards, 1/round from round 2, revealed at round start, round 1 clean) |
| Modifiers | Residue, Thin Ice, Bountiful Brew, Deep Cauldron, Double Stakes, Reversal — single-effect, compose cleanly, contradictions cancel |
| Effects | Peek, Dampen, Volatile Surge, Shield, Expose, Copycat, Recall, Double Down — resolve immediately, fixed order |
| Shield | Rare + forfeits all scoring on safe resolution (a bet on the boom) |
| Info reveal | Always reveal cards — dramatic play-order depile every round (volatility climbs to the boom); card counting rewarded. Exact boiling point revealed **on explosion only** (hidden on a safe brew) |
| Tiebreaker | Deathmatch — pure elimination, no modifiers, forced 1 card/wave, most-volatility = Detonator = out, co-champions if no boom; effects allowed (Shield god-like) |
| Deal / hand | Refill-to-5 floor at round start (never discards, no cap); unplayed cards carry over; deck reshuffles from discard if it empties |
| Comms | Preset emotes only (non-binding); no quick-phrases, no free-text chat |
| Matchmaking | v1 — invite links + table-filling queue (not skill-based); anonymous sessions. Rating, persistent accounts & skill-based matching → v2 |

---

## 18. Deferred Features

Conscious "not yet" decisions — out of scope **on purpose**, not by oversight.

| Deferred | Note |
|---|---|
| **Round objectives** (v1 brainstorm, Alt A) | Per-round scoring tweaks to nudge specific dilemmas. The modifier stack currently does the round-to-round variety job; revisit if rounds feel samey. The likeliest item to return. |
| **Spectator mode & replays** | Need an append-only event log; the server chose post-game persistence only (no event sourcing yet). |
| **Cauldron-modifier expansions** | The 6 modifiers are the launch set. Add more once the stacking system is validated by playtesting. |

Platform/post-launch deferrals (player rating, persistent accounts, skill-based
matchmaking) live in [05_roadmap.md](05_roadmap.md). Free-text chat (§10) and
3-player support (§2) are noted as out of v1 scope but aren't committed to a
later version.
