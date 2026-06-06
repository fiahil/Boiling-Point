# Boiling Point — Depth & Complexity (Core-Adjustment Proposals)

**Status: PROPOSAL — not canonical.** This document captures recommendations for
growing Boiling Point from a tight one-game experience into a game with a higher
skill ceiling and long-term replayability. It is **exploratory**; nothing here
supersedes [`02_game-design.md`](02_game-design.md) until ratified and promoted into it.

Per the [constitution](../CLAUDE.md): every value below is a **hypothesis until
validated by playtesting** — explicit `[needs playtesting]` tags throughout — and
every adjustment must respect Principle I (server-authoritative), Principle III
(start simple, justify complexity), and Principle IV (data-informed balance via
the bot harness).

Explored 2026-06-05.

---

## 0. The diagnosis (why touch the core at all)

The v1 design is already deep on **one** axis — *in-the-moment hidden-information
politics*. Where it is thin is everything **across** a single game:

```
   RICH (within one game)            THIN (across games / players)
   • in-round social reads           • pre-game agency (cards are pure luck)
   • push-your-luck tension          • player identity (every seat is identical)
   • emergent game theory            • mastery curve & reasons to return
   • modifier-driven variety         • a single fixed game shape
   • the depile spectacle
```

Two core adjustments attack the "thin" column **without** adding a meta-game,
accounts, or a content treadmill (those — ranked/rating/seasons — are deliberately
out of scope here; see [`05_roadmap.md`](05_roadmap.md)):

| # | Adjustment | What it deepens | Risk |
|---|---|---|---|
| **A** | **Vote vs Spell** — every card is playable as its color *or* as wild | Per-card decision depth; effects gain opportunity cost | Threatens the Peek economy + adds decision load to fast waves |
| **B** | **Brewers** — asymmetric player identities | Player identity, replayability, "read the person not just the cards" | Balance/readability vs §III |

They **compound**: Vote/Spell creates a new rules surface that Brewers can bend
(§C).

---

## A. Vote vs Spell — every card is two cards

### A.1 The rule

Today an effect card is a free rider: playing it adds its color **and** points
**and** fires its effect, all at once. The proposal decouples *color* from
*effect* and makes the player choose, **per card, at play time**:

```
        EVERY CARD IS NOW TWO CARDS

   ┌─────────────────────┐        ┌─────────────────────┐
   │   PLAY AS COLOR      │        │   PLAY AS WILD       │
   │   = a VOTE           │   OR   │   = a SPELL          │
   │                      │        │                      │
   │  backs its color for │        │  effect FIRES        │
   │  dominance (§6)      │        │  colorless to the    │
   │  effect DORMANT      │        │  dominance read      │
   └─────────────────────┘        └─────────────────────┘
        (printed color is RETAINED in both modes)
```

- **Vote (color mode):** contributes color + points + volatility to the pot; the
  card's effect does **not** fire.
- **Spell (wild mode):** the effect fires; the card contributes **no color** to
  dominance. Points still drop into the pot as **colorless points** — exactly like
  today's printed Wilds (§6: they fatten the pot and the blast but never help a
  color win). `[needs playtesting]` — see fork A.4.
- **Volatility always counts**, in both modes. Only *color* and *effect* toggle.
- **Printed color is always retained** for the depile, card-counting (§10), and
  any effect that reads color (see A.3 — this is load-bearing, not cosmetic).

A 5-card hand stops being 5 options and becomes **up to 10**.

### A.2 Why it deepens the game

Effects stop being automatic. **Firing an effect now costs you a vote** — a Peek
means you are deliberately *not* coloring the pot this wave. That opportunity cost
is what turns an automatic play into a real decision.

It also generalizes two fixed archetypes (§3) into on-demand modes available to
*any* card:

```
  "Political ghost" (presence, no scoring) ─┐
                                            ├─► just "play any card as a Spell"
  "Pot inflator"   (points + danger)      ──┘
```

Anyone can now choose to **go neutral** on a wave — add danger and pot value
without backing a side. A misdirection button, a "don't read me" button, and a
Reversal-dodge, all from one rule.

### A.3 Interactions (where "keep original color" earns its keep)

Two existing effects (§9) *cannot function* unless a Spell retains its printed
color — which is the mechanical justification for that clause:

| Effect | Interaction under Vote/Spell |
|---|---|
| **Double Down** | "Doubles the points of *its* color in the pot." Played as a Spell it is colorless for dominance — so it doubles its **printed** color. Sharp choice: play the Red Double-Down as a **vote** (no doubling) or as a **Spell** (colorless vote, but doubles all the Red already in the pot). |
| **Copycat** | "Becomes the highest-point color in the pot." Played as a Spell, it fires and *adopts* a color — a card that is wild-until-it-resolves-then-colored. The one effect that turns a Spell back into a Vote. |
| **Printed Wilds** | Reframe a printed Wild as "a card whose *only* mode is Spell." No contradiction; just needs an explicit decision. |
| **Reversal** (modifier, §8) | More Spells played → fewer colors present → Reversal swingier. A texture note, not a problem. |

### A.4 Risks & open forks

1. **Threatens the single most important balance knob.** §16 names **Peek count
   (4)** as *the* lever — "too few → pure roulette." Under Vote/Spell, players fire
   Peek *less* (it now costs a vote), so fewer Peeks reach the cauldron and blind
   volatility (§4) drifts toward luck. Likely mitigations `[needs playtesting]`:
   rebalance the deck toward **more effect cards**, or carve an exception (info
   effects fire "free"?). This is squarely a bot-harness question.
2. **Decision load in a 10-second wave.** §5's waves are fast. "Pick a card"
   becoming "pick a card **and** its mode" doubles the snap-decision. The UI must
   make it **one gesture** (e.g. tap = Vote, long-press/flip = Spell) or the
   elegance becomes stress.
3. **Fork — do Spells keep their points?** Default **yes** (colorless points, like
   Wilds — more consistent, more interesting). Alternative: Spell = pure effect, no
   points (cheaper but weaker effects). `[needs playtesting]`.

### A.5 Recommendation

**Ship Vote/Spell as a selectable mode ("Wild Brew") first, not as an immediate
core replacement.** Rationale (Principle III + IV): it materially changes deck math
and the already-tuned Peek economy, so let the **bot harness run it head-to-head**
against the canonical ruleset (explosion rate, Peek-fire rate, effect usage,
degenerate strategies) before deciding whether it *becomes* the core. The seam is
the same toggle Brewers will use (§B.4). If the data is healthy, promote it to core
and fold it into 02_game-design.md §3/§5/§9.

---

## B. Brewers — asymmetric player identities

Today every seat is **mechanically identical** (confirmed: the existing
`agent-personas` are *playstyle biases* that grant **no capability**). The proposal
gives each player a **Brewer** — an alchemist identity with one bent rule — drawn
or picked at game start. This is the *Cosmic Encounter* move: the highest
depth-per-unit-of-complexity addition available to a symmetric political FFA game.

### B.1 Design discipline (non-negotiable)

A Brewer must obey the same discipline as the modifiers (§8): **one sentence, one
bent rule, instantly readable.** The acceptance test:

> A great Brewer creates **decisions and reads for the whole table**, not just a
> stat edge for its owner.

The Cinderwright (below) is fun not because *its owner* takes less damage, but
because *everyone else* must now play the fat-pot staring contest knowing it
doesn't blink. The power radiates outward.

**Two hard guardrails, both derived from existing design decisions:**

```
  ✗ Must NOT make explosions free for its owner.
      → resurrects the suicide-bomber that §7 deliberately killed.
        Half-damage is the absolute ceiling, and even that is spicy.
  ✗ Must NOT hand out free perfect information.
      → "you see the boiling point" vaporizes the Peek economy (§4/§16).
        Info-Brewers bend the FLOW of information, never the answer.

  Also avoid: pure stat leads ("+1 point/round" — a lead, not a decision) and
  powers with no counterplay (asymmetry the table can't perceive is just variance).
```

### B.2 Candidate pool (organized by the system each one hooks)

All effects `[needs playtesting]`. 🌶️ = high-impact, needs care.

**Info economy** (the richest vein):

| Brewer | One-line rule |
|---|---|
| **The Eavesdropper** | When anyone plays Peek, you secretly learn what they learned. |
| **The Diviner** | You start every game holding a Peek that does not fill a hand slot. |
| **The Cipher** 🌶️ | Your contribution count shows as "?" to everyone else (erases the game's #1 political signal — §10). |

**Hand / deck economy** (quiet, compounding, deathmatch-relevant):

| Brewer | One-line rule |
|---|---|
| **The Magpie** | You refill to 6, not 5. |
| **The Forager** | At round start, look at the top 2 deck cards; swap one into your hand. |

**Effect-modifier class** (deepens cards you already designed):

| Brewer | One-line rule |
|---|---|
| **The Scavenger** | Your Recall can pull *any* card from the pot, not just your own. |
| **The Echo** 🌶️ | The first effect card you play each round resolves twice. |

**Scoring / dominance:**

| Brewer | One-line rule |
|---|---|
| **The Broker** | When you split a pot (Alliance/Commune), you round **up**, not down (§6) — making you everyone's preferred ally. |
| **The Purist** | Win a pot **solo** (no split) → +2 bonus (an anti-alliance identity). |
| **The Turncoat** 🌶️ | Once per game, after a depile, change which color "you are." |

**Wave / commitment** (spiciest — these rules are load-bearing, §5):

| Brewer | One-line rule |
|---|---|
| **The Phantom** 🌶️ | The first time you pass each round, you are **not** locked out. |
| **The Lurker** 🌶️ | Once per round, you may commit your card **after** the wave reveals. |

**Escalation engine** (§8):

| Brewer | One-line rule |
|---|---|
| **The Stormcaller** | Two modifiers are drawn each round; you privately choose which becomes active. |

**Social layer** (thematically delicious in a game built on lies):

| Brewer | One-line rule |
|---|---|
| **The Honest Broker** | Once per game, reveal a card from your hand to the table — and you **must** play it next wave (the only *binding* promise in a game where every emote is a bluff — §10). |

### B.3 The pivotal fork — disclosure

Whether Brewers are known decides the feature's entire feel:

```
   PUBLIC                    LEAKED                     SECRET
   (Cosmic Encounter)        (the hybrid)               (social deduction)

   Known from turn 1.        Hidden at start; your      Nobody knows; deduced
   Politics form around      play + the depile leak     over 5 rounds.
   known incentives.         it over the game.
   Legible, lowest risk,     Uses the depile as the     Adds a deduction layer
   fits §III.                identity-reveal engine.    on top. Highest risk,
                                                        most against §III.
```

### B.4 Recommendations

- **Disclosure: ship PUBLIC first.** Lowest risk, proven model, makes the politics
  richer without bolting on a deduction game. **Leaked** is the uniquely *Boiling
  Point* version (it weaponizes the depile) — hold it as the v-next evolution once
  Public is validated. Secret is a different product.
- **Pool: launch ~6 Brewers, not 16.** A small, hand-tuned set beats a sprawling
  one. Keep each to one sentence.
- **Gate behind a toggle.** The pure-symmetric game stays the default/baseline (and
  the future ranked baseline). Same seam as Vote/Spell (§A.5).
- **Assignment: a 2-pick-1 draft** `[needs playtesting]` — chaos of a random draw,
  but a sliver of pre-game agency (the thin column, §0). Pure-random is the simpler
  fallback.
- **Validate with the bot harness (§IV).** The key matrix is **persona × Brewer**:
  do the existing playstyle archetypes (Gambler/Turtle/Bandwagoner/Trickster) break
  any Brewer? Thousands of games before a human sees it.

---

## C. Why A and B compound

Vote/Spell (§A) creates a brand-new rules surface — *the color/effect toggle* —
that Brewers (§B) can bend. Brewers that exist **only because** of Vote/Spell:

| Brewer | One-line rule |
|---|---|
| **The Polymath** 🌶️ | Your effects fire in **color mode too** — you never sacrifice the vote. |
| **The Witch** | Your Spells still count their color at **half** points — a half-vote *and* a spell at once. |

And the **Echo** Brewer (effect resolves twice) gets sharper under Vote/Spell,
because each fire already costs more. The two proposals are not parallel features;
they multiply.

---

## D. Constitution check

| Principle | This proposal |
|---|---|
| **I — Server-authoritative** | Card mode (Vote/Spell), Brewer identity, and all effect resolution are computed and validated **server-side**. The client renders a mode toggle and sends an intent; it never decides outcomes. |
| **III — Start simple** | Both ship **behind a toggle**, leaving the canonical symmetric ruleset as the untouched default. Vote/Spell ships as a *mode* before any core replacement. Brewers launch as a *small* pool, Public disclosure (the simplest), before the spicier Leaked variant. |
| **IV — Playtest-driven** | Every number is `[needs playtesting]`. The bot harness runs Vote/Spell head-to-head vs. canonical, and the persona × Brewer matrix, **before** human playtests. The Peek-economy risk (A.4) is an explicit data target. |

**Rejected simpler alternative:** deepen the game purely by adding more modifiers
and effect cards (the "natural growth" path). Rejected as the *primary* lever — it
raises content volume but not the skill ceiling or player identity, which is where
"cute indie → game with depth" actually lives. It remains a fine *secondary* lever.

---

## E. If adopted — impact on `02_game-design.md`

A non-binding map of which canonical sections each adjustment would touch:

| Adjustment | Sections to revise |
|---|---|
| **Vote/Spell** | §3 (the card — add the mode), §5 (turn structure — the play gesture), §6 (scoring — colorless Spell points), §9 (effects — fire-on-Spell + Double Down/Copycat clauses), §13 (deck — rebalance effect-card share), §16 (balance knobs — Peek-fire rate). |
| **Brewers** | New section (player identities), §2 (format), §11 (Deathmatch — per-Brewer tiebreaker value), §12 (archetypes — Brewers vs personas). |

---

## F. Next steps (when ready to leave explore mode)

1. Pick the **first** target — recommend **Vote/Spell as a "Wild Brew" mode** (the
   sharper core question; bot-harness can answer it fastest).
2. Open an OpenSpec change (`openspec/changes/…`) with proposal + design + specs.
3. Run the bot-harness comparison; let the data set the deck/Peek numbers.
4. Promote validated rules into `02_game-design.md`; park the rest here.
