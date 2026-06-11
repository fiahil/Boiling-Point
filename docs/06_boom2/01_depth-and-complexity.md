# Boiling Point — Depth & Complexity (Core-Adjustment Proposals)

**Status: PROPOSAL — not canonical.** This document captures recommendations for
growing Boiling Point from a tight one-game experience into a game with a higher
skill ceiling and long-term replayability. It is **exploratory**; nothing here
supersedes [`02_game-design.md`](../02_game-design.md) until ratified and promoted into it.

Per the [constitution](../../CLAUDE.md): every value below is a **hypothesis until
validated by playtesting** — explicit `[needs playtesting]` tags throughout — and
every adjustment must respect Principle I (server-authoritative), Principle III
(start simple, justify complexity), and Principle IV (data-informed balance via
the bot harness).

Explored 2026-06-05. Extended 2026-06-06 (Ingredients / deck-building — §C).

---

## Executive summary — how Boiling Point grows from here

Boiling Point is deep **within** a single game and thin **across** games (§0).
Three core-adjustment directions attack that thin column. Each ships behind a
toggle over the canonical symmetric shared-deck baseline (which stays the default
and the future ranked baseline), and all three **compound** (§D).

```
   DIRECTION         DEEPENS                          THIN-COLUMN LINE IT FIXES
   A · Vote/Spell     in-wave decision (1 card → 2)    (mostly within-game depth)
   B · Brewers        player identity                  "every seat is identical"
   C · Ingredients    pre-game strategic agency        "cards are pure luck"  ◄ only C
```

**Brewers + Ingredients** together are the *identity-&-agency* path — who you
**are** (Brewer) and what you **bring** (your deck). They are the two highest
depth-per-complexity moves for turning a tight indie game into one with a real
skill ceiling, and — critically — they are **low-risk to the delicate
Peek / blind-volatility economy** the canonical tuning rests on (§16). Vote/Spell
is the sharpest *in-the-moment* deepener but is the one direction that can quietly
break that Peek economy (A.4).

**Personal recommendation** *(this revises the original §G "Vote/Spell first" call)*:

```
   1. BREWERS       Public disclosure, ~6 identities, 2-pick-1 draft.
                    Highest depth/complexity; proven model (Cosmic Encounter).

   2. INGREDIENTS   Procedural "Recipe" decks, COLOR-ANCHORED with a scarce
                    off-color toolkit. Restores AND deepens the politics
                    (the Loyalist↔Diplomat axis), kills the "pure luck" line,
                    and needs no saved decks → no accounts (stays in v1 scope).

   3. VOTE/SPELL    Ship as a bot-harness-vetted "Wild Brew" mode. Deepest, but
                    the only one that threatens the tuned Peek knob — sequence it
                    last, behind data, not as the thing you lead with.
```

Why the reorder: the goal here is restoring **political & strategic** depth and
**player identity** — which weights squarely toward B + C, the two directions that
*don't* endanger the canonical core. Vote/Spell stays valuable, but as the change
you **de-risk with the harness**, not the one you open with.

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
out of scope here; see [`05_roadmap.md`](../05_roadmap.md)):

| # | Adjustment | What it deepens | Risk |
|---|---|---|---|
| **A** | **Vote vs Spell** — every card is playable as its color *or* as wild | Per-card decision depth; effects gain opportunity cost | Threatens the Peek economy + adds decision load to fast waves |
| **B** | **Brewers** — asymmetric player identities | Player identity, replayability, "read the person not just the cards" | Balance/readability vs §III |
| **C** | **Ingredients** — players stock the personal deck they draw from | **Pre-game strategic agency** — converts color access from luck → choice; restores cross-color politics as a *budgeted* resource | Touches the shared-deck political pillar (§3); widens the balance surface |

They **compound** (§D): Vote/Spell creates a new rules surface that Brewers can
bend, and Ingredients hands every player the lever that makes a Brewer identity
*expressible* (build the deck that suits who you are).

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

## C. Ingredients — players stock the deck they draw from

### C.1 The rule

Today every hand is a random draw from one shared shoe — so **pre-game agency is
zero** (§0: "cards are pure luck"). Ingredients gives each player a **personal
deck** — their *Pantry* — stocked before the game and drawn from only by them. The
Pantry is **anchored in the player's own color**, but **budgets a scarce minority
of off-color, wild, and effect cards**: its *toolkit*.

```
   A PANTRY  (one player's personal shoe, ~18–24 cards [needs playtesting])

   ┌──────────────────────────────────────────────────────────┐
   │  ~70–80%  your OWN color    → identity reads clearly,       │
   │                               the Aggressor can finally     │
   │                               COMMIT to red                 │
   │  ~20–30%  TOOLKIT budget    → THE politics knob:            │
   │            • off-color cards   (ally · kingmake · misdirect) │
   │            • wilds             (pot inflators · go neutral)  │
   │            • effects           (capped — protects Peek, §16) │
   └──────────────────────────────────────────────────────────┘
        Everyone gets an EQUAL construction budget → fair, like
        TCG archetypes or MOBA picks (equivalent, not identical).
```

The pot, dominance, scoring, and explosion (§6/§7) are **untouched** — Ingredients
only changes *what each player can draw*. Clean seam onto the existing core.

### C.2 How it puts the politics back — and deepens them

The naïve version ("you only ever draw your own color") **amputates** the political
core: with 4 players = 4 colors, if you can only play your own color you can only
ever help yourself — kingmaking, cross-color alliances, and misdirection all die
(§3 calls these the central acts). The **color-anchored deck with a budgeted
toolkit** does the opposite — it converts cross-color play from a *lucky draw* into
a *pre-committed strategic resource*, which makes it **deeper**, not absent:

**1 · Cross-color cards become pre-committed political capital.** You decide
*before round 1* how much capital to pack. If decks are **public** (C.3, fork 4),
the table reads your intent before a card drops — *"Red built a Cosmopolitan deck;
they're not trying to win, they're trying to decide who does."* The metagame now
starts at the draft.

**2 · Scarcity makes every off-color play scream intent.** Three Blue cards drawn
by luck are noise; one Blue card you *chose* to pack, out of only four off-color
cards all game, is a loud, legible act. Misdirection and kingmaking become more
deliberate **and** more readable — the Diplomat's craft (§12) with a sharper edge.

**3 · Your deck is the one thing you can't lie about.** Every emote is non-binding
(§10); the only binding promise was the Honest Broker Brewer. A *public* deck adds
a second structural truth — it bounds your **capabilities**. But your individual
plays stay hidden (blind volatility + hidden commits, §4/§5), so the table knows
your **range**, never your **hand**. This is poker: it *raises* the reading skill
instead of removing it.

**4 · Card-counting upgrades to range-reading.** §10's "what's left in the shared
shoe" becomes *"Red has burned both Peeks and the Shield — they're out of safety
now, they're exposed."* The memory skill the design already rewards gets **deeper**.

**5 · Alliances and sabotage gain a pre-game dimension.** A toolkit budget
*telegraphs* an intended alliance you can then honor or betray; a high-volatility
build is a *committed* Saboteur — though §7's shared-pain still makes blowing the
pot hurt you too, so pure aggression stays self-limiting (the anti-degenerate
guardrail holds).

These collapse into a clean, legible **2×2 of strategic identities** — each one
creating reads for the *whole table* (the same discipline test a Brewer must pass,
§B.1):

```
                        AGGRESSIVE  (high vol · weapons · greedy points)
                              │
        The Warlord ──────────┼────────── The Provocateur
        (mono-color, pushes   │        (off-color weapons — sows chaos
         own color to win)    │         in others' pots; a built Saboteur)
   ─────────────────────────  ┼  ─────────────────────────
        LOYALIST              │              DIPLOMAT
        (anchored, self-win)  │        (off-color + wild toolkit)
                              │
        The Fortress ─────────┼────────── The Kingmaker
        (mono-color · Dampen/ │        (Copycat / Double Down / wilds —
         Shield / Peek · endure)│        decides who wins, can't itself)
                              │
                        PROTECTIVE  (low vol · safety · information)
```

The **Loyalist ↔ Diplomat** axis *is* the politics knob — the dial deciding how
much of your game is "win on my own color" vs "broker everyone else's outcome." It
did not exist as a *choice* before; color access was pure luck. The
**Aggressive ↔ Protective** axis is the original idea (build to push or to endure).
Together they make a deck a **strategic identity**, not a stat lead.

### C.3 The four forks (your open questions)

**Fork 1 — Shared shoe vs personal draw** *(your Q1 "shuffle all together" + Q2
"only draw your own color" are the same axis):*

| Model | What happens | Verdict |
|---|---|---|
| **Personal deck, personal draw** | Each player draws only their own stocked shoe | ✅ **Recommended.** Decks *mean* something; identity is legible. |
| **Shuffle all together, blind draw** | You may draw cards an *opponent* stocked | ⚠️ Anti-synergy — a strong deck *feeds your enemies*. Avoid unless you want exactly that chaos. |
| **Shuffle together, owner-tagged** | One physical pile, but you still only draw *yours* | = personal draw; a pure implementation detail, no design difference. |

Note the trap in "only draw your own **color**" ≠ "draw your own **deck**." The
former amputates politics (C.2); the latter, anchored, *restores* them.

**Fork 2 — Construction method** *(your Q4: pick cards vs. set randomizer
parameters)* — Principle III has strong opinions here:

```
SIMPLEST ──────────────────────────────────────────────────────► MOST COMPLEX
Preset archetypes      RECIPE dials             Constrained        Freeform
(2–4 fixed decks:      (Posture: Aggro↔Protect; draft (pick N        deckbuilder
 Warlord / Fortress /  Allegiance: Loyalist↔     from a pool         (Hearthstone)
 Kingmaker …)          Diplomat; +capped Tooling within a budget)
                       → PROCEDURAL deck,
                         novel every game
   ▲ fine v0               ▲ RECOMMENDED                              ▲
                       novelty + agency + bounded                 saved decks →
                       (generator enforces caps,                  accounts → OUT
                        so no degenerate min-max)                 of v1 scope (§14)
```

Your "set the parameters of a randomizer that builds a novel deck each game"
instinct is the sweet spot: **more replayable** than fixed presets, **far safer**
than freeform (the generator *enforces* the balance caps, so no degenerate
min-maxing), and there is **nothing to save** — which keeps the whole feature
inside the anonymous-session world (§14). Freeform is a trap: it drags in saved
decks → accounts → a content treadmill, all explicitly out of scope.

**Fork 3 — Ordering vs the Brewer pick** *(your Q3):*

```
   BREWER → DECK     build to suit the identity (the synergy hunt) — richest skill
                     expression; RECOMMENDED.
   DECK → BREWER     the Brewer is a wildcard topping.
   BUNDLED "KIT"     Brewer + signature deck as ONE pick — Hearthstone-class,
                     simplest & most legible; a fine v0.
```

**Fork 4 — Disclosure** *(mirrors the Brewer fork, §B.3):*

```
   PUBLIC             LEAKED                       SECRET
   deck comp known    hidden; the depile reveals   nobody knows; deduced over
   from the draft.    it over the game (weaponizes  5 rounds (a deduction game
   Richest legible    the depile — the uniquely     bolted on top).
   reads; RECOMMEND.  Boiling-Point variant).      Highest risk, most vs §III.
```

Public is the recommended launch — it is what makes C.2's pre-game reads exist at
all. Leaked is the v-next evolution (same move the doc recommends for Brewers).

### C.4 Risks & open forks

1. **The Peek / effect economy.** §16 names Peek count (4) as *the* balance knob.
   Letting players stock their own effects ("2 Peeks + a Shield") is the spiciest
   lever and the biggest risk to blind-volatility tuning. Mitigation: the Recipe
   generator **caps effect density per deck** (freeform can't) — a bot-harness
   target. `[needs playtesting]`
2. **Determinism worry.** Color-anchored decks could make dominance predictable
   ("whoever pushes their own color hardest wins"). Counter: the off-color toolkit,
   wilds, Copycat/Double Down, **Reversal** (which swings *harder* when fewer colors
   are present — same logic the doc notes for Spells, A.3), and the existing hidden
   commits / blind volatility all keep the pot's color outcome live. `[needs playtesting]`
3. **The pre-game phase vs the lobby.** §14 is "no settings, auto-start at exactly
   4, ~5–10 min." A 2-pick-1 draft or a few dial-twists survives that ethos; a
   freeform builder does not. Keep the build step **fast**.
4. **Saved decks = accounts = out of scope.** Presets and procedural Recipes have
   nothing to persist — a major Principle-III point for the procedural route.
5. **Balance surface.** Ingredients widens the harness matrix to **persona ×
   Brewer × deck-archetype** — large but *bounded* by the procedural caps. This is
   exactly the bot harness's job (§IV).

### C.5 Recommendation

Ship Ingredients as the **procedural "Recipe"** route: a few dials
(**Posture** Aggressive↔Protective, **Allegiance** Loyalist↔Diplomat, plus a capped
Tooling slider) that seed a **novel, color-anchored personal deck** each game,
chosen **after the Brewer**, with **public** deck composition, behind the **same
toggle** as Brewers and Vote/Spell. This restores *and deepens* the politics
(C.2), directly fixes the "cards are pure luck" line (§0), needs **no accounts**,
and hands the harness a bounded surface to tune. All values `[needs playtesting]`.

---

## D. Why A, B, and C compound

These are not parallel features; they **multiply**.

**A × B — Vote/Spell creates a rules surface Brewers can bend.** Brewers that exist
**only because** of Vote/Spell (§A):

| Brewer | One-line rule |
|---|---|
| **The Polymath** 🌶️ | Your effects fire in **color mode too** — you never sacrifice the vote. |
| **The Witch** | Your Spells still count their color at **half** points — a half-vote *and* a spell at once. |

The **Echo** Brewer (effect resolves twice) also sharpens under Vote/Spell, because
each fire already costs more.

**B × C — your Brewer makes your deck, your deck expresses your Brewer.** Ingredients
is the lever that turns a Brewer from a *stat bent* into a *build*:

| Pairing | Why it deepens |
|---|---|
| **Brewer + deck = "class + loadout"** | Pick the Echo Brewer, then stock an effect-rich deck to abuse the double-fire — the synergy hunt is the skill ceiling. |
| **Deck-economy Brewers get teeth** | The Forager (look at top 2, swap one) and Magpie (refill to 6) are far richer when you *know your own deck* — look-ahead becomes a real plan, not a coin flip. |
| **De-dup needed** | The Diviner (start holding a Peek) overlaps with "just stock a Peek." Re-point or cut such Brewers, or make them *deck-archetype enablers* instead. |

**A × C — Vote/Spell doubles the utility of every toolkit card.** Under Vote/Spell,
your scarce off-color card can be played as a colored **Vote** (kingmake) *or* a
colorless **Spell** (just the effect) — so the Allegiance dial (C.2) and the
color/effect toggle (A.1) interact: even a Loyalist deck can "go neutral" on demand.

---

## E. Constitution check

| Principle | This proposal |
|---|---|
| **I — Server-authoritative** | Card mode (Vote/Spell), Brewer identity, deck construction, and all effect resolution are computed and validated **server-side**. The server builds the Pantry from the chosen Recipe, owns the deck, and deals; the client renders a mode toggle / dial UI and sends intents, never deciding outcomes. |
| **III — Start simple** | All three ship **behind a toggle**, leaving the canonical symmetric shared-deck ruleset as the untouched default. Vote/Spell ships as a *mode* before any core replacement. Brewers launch as a *small* Public pool before the spicier Leaked variant. Ingredients ships as **procedural Recipes** (no saved decks → no accounts) before any freeform builder, and **color-anchored** so the shared-deck political pillar (§3) is restored, not removed. |
| **IV — Playtest-driven** | Every number is `[needs playtesting]`. The bot harness runs Vote/Spell head-to-head vs. canonical, the persona × Brewer matrix, and the persona × Brewer × deck-archetype matrix **before** human playtests. The Peek-economy risk (A.4 / C.4) is an explicit data target for both Vote/Spell *and* Ingredients' effect-density caps. |

**Rejected simpler alternative:** deepen the game purely by adding more modifiers
and effect cards (the "natural growth" path). Rejected as the *primary* lever — it
raises content volume but not the skill ceiling or player identity, which is where
"cute indie → game with depth" actually lives. It remains a fine *secondary* lever.

---

## F. If adopted — impact on `02_game-design.md`

A non-binding map of which canonical sections each adjustment would touch:

| Adjustment | Sections to revise |
|---|---|
| **Vote/Spell** | §3 (the card — add the mode), §5 (turn structure — the play gesture), §6 (scoring — colorless Spell points), §9 (effects — fire-on-Spell + Double Down/Copycat clauses), §13 (deck — rebalance effect-card share), §16 (balance knobs — Peek-fire rate). |
| **Brewers** | New section (player identities), §2 (format), §11 (Deathmatch — per-Brewer tiebreaker value), §12 (archetypes — Brewers vs personas). |
| **Ingredients** | §3 (any-color-from-shared-deck → color-anchored personal deck + toolkit), §13 (deck composition → per-player Pantry sizing, reshuffle, effect caps), §10 (card-counting → per-opponent range-reading), §12 (archetypes — the deck-identity 2×2), §14 (a fast pre-game Recipe step in the lobby flow), §16 (new knobs — toolkit %, effect caps, deck size). |

---

## G. Next steps (when ready to leave explore mode)

1. Pick the **first** target. The original call here was Vote/Spell; the
   **Executive summary** (top of page) revises that to **Brewers first, then
   Ingredients, then Vote/Spell** — leading with the two identity-&-agency
   directions that *don't* threaten the tuned Peek economy.
2. Open an OpenSpec change (`openspec/changes/…`) per target, with proposal +
   design + specs. (Brewers and Ingredients can share the pre-game-setup seam.)
3. Run the bot-harness comparison; let the data set the deck / Peek / toolkit
   numbers and validate the persona × Brewer × deck-archetype matrix.
4. Promote validated rules into `02_game-design.md`; park the rest here.
