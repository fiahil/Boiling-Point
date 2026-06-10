# Boiling Point — Toward a v2 Core

**Status: DECISION LOG (committing direction, pre-implementation).**

This file tracks the **v2 core rework** — the deliberate move from the tight
"light" game ([`02_game-design.md`](02_game-design.md)) toward a deeper, more
strategic, more political game. It is the live record of what is **committed**,
what is **leaning**, and what is **open**.

Exploratory rationale for each direction lives in
[`06_depth-and-complexity.md`](06_depth-and-complexity.md) — §A (Vote/Spell),
§B (Brewers), §C (Ingredients). When a decision here is validated and built, it
gets promoted into [`02_game-design.md`](02_game-design.md) and this log notes it.

Per the [constitution](../CLAUDE.md): every value is `[needs playtesting]`; the
bot harness (Principle IV) **re-derives the whole balance economy** for the new
core — the old tuning (boiling point 8–14, Peek count 4, vol 1–3) does not carry
over unexamined.

Opened 2026-06-08.

---

## Why a v2 core

Playtesting found the light game **too simple.** The decision is to grow Boiling
Point into a more **complex, strategic, and political** game — accepting a longer
session and a higher skill ceiling as *the point*, not a cost.

This **reverses** the original "keep it a 5–10 minute filler" framing
(§1/§2 canonical). The deep direction *becomes the core* — it is **not** an
opt-in "Deep Brew" mode beside a light default. There is one game now, and it is
the deeper one.

---

## Committed decisions

| # | Decision | Detail |
|---|---|---|
| **C1** | **Deeper core, not a mode** | The strategic/political game *is* the product. The light symmetric game is retired as the headline experience (it may survive only as a teaching/quickplay variant — TBD, not committed). |
| **C2** | **Session length ~15–20 min, possibly more** | Replaces the 5–10 min target (§2). Longer is acceptable in service of depth. Round count / wave timers re-open as knobs to hit this. |
| **C3** | **Brewers — 12 identities to create** | Asymmetric player identities. **Public** (known from turn 1). Each player **picks 1 of 2**. Identities are **unique around the table.** |
| **C4** | **Keep the wave mechanism** | Simultaneous single-card waves, hidden commits revealed together, **pass = locked out** (§5). The core turn loop is unchanged. |
| **C5** | **Deck drafting (Ingredients) — fast, novel, owner-unknown** | Players draft a **personal deck** quickly before the game and **do not know its full contents** — they **learn it as the game goes.** Implies a *procedural / probabilistic* build (you shape the odds, not the cards), a **personal shoe** (you draw only your own deck), and **no saved decks** (nothing to persist → stays in the anonymous-session world, §14). |

### Notes on the committed decisions

**C3 — the uniqueness math.** 4 players × (pick 1 of 2) × all-unique → deal **4
disjoint pairs** (8 Brewers in play), so any combination of picks is
automatically unique with no draft-order contention (everyone picks at once → fits
the auto-start ethos, §14). A pool of **12** means each game draws 8 of 12 and
pairs them → real cross-game variety. The cost is a balance commitment: **≥12
mutually balanced, one-sentence identities**, all vetted on the persona × Brewer
harness matrix before humans see them (§B.1 discipline applies).

**C5 — what "owner-unknown" buys.** Because even *you* don't know your realized
deck, disclosure resolves cleanly: **public recipe** (everyone sees your *intent*
— your cursor/dial settings) + **hidden realization** (nobody, including you,
knows the exact cards). The table reads your **range, never your hand**; you learn
your own deck as you draw it. This is *better* than a public deck-list — it keeps
the luck texture, blocks net-decking (you can't min-max a distribution), and needs
no accounts. Color-anchored deck + scarce off-color toolkit (per §C.1) so the
political pillar (§3) is *restored*, not removed.

---

## Leaning (strong, being refined alongside the open work)

| Decision | Detail | Why not yet "committed" |
|---|---|---|
| **L1 — Vote/Spell, *reshaped*** | The pantry/grimoire split (below) **retires "every card is two cards"** — a spell is its own object, not an ingredient played differently. What may survive: an **ingredient** playable as its color (a **Vote** — scores points) *or* as a colorless **wild** (volatility only, **no points** — a pure-danger / go-neutral push). Points still score **only** on colored Votes. | Whether the surviving ingredient wild-mode is kept is still open. |
| **L2 — Wider volatility range** | Expand volatility from 1–3 toward **0–5 / 0–7**, with a matching **boiling-point rescale** (a coupled knob — can't move one without the other). Gives the deck-posture dial room to express and makes single-card swings real. | Exact range + rescale is a harness number; tied to the explosion-damage model (open). |

---

## The boom & spells — leaning model

The explosion model **pivoted (2026-06-08)** away from §7's "everyone loses the
pot" toward **individual accountability**. It deliberately reverses three §7
pillars — **the Vulture returns** (passing is now safe), **risk becomes
individual**, and **sabotage is viable again** — in exchange for a tighter
hot-potato / chicken game and one clean symmetry:

```
   EVERY POT IS WORTH P   (P = sum of colored ingredient points)

        safe brew  →  dominant color   GAINS  +P
        explosion  →  the detonator(s)  LOSE  -P   (unless warded)
        passed / absent → 0 either way

   "Win the pot, or be the one who blows it and pay for it."
```

**Magnitude = points (P).** Volatility's job narrows to the *trigger* (when it
blows) plus identifying the *detonator* (who pays). The earlier damage-scaling
options (volatility-blast, overshoot) are **dropped** — directedness comes from
*who pays*, not from what scales the boom.

**Pantry / Grimoire split — two card types, two decks, two drafting dials:**

| | **Pantry (ingredients)** | **Grimoire (spells)** |
|---|---|---|
| Where it goes | **into the cauldron** | **active effect — never in the pot** |
| Carries | color · volatility · points | no points, no pot-volatility of its own |
| Does | builds P, arms the trigger | fires instantly, or stays active to react |
| Drafted by | Posture dial (aggressive↔safe) | what tricks you pack (god-tier Peek↔Shield budget) |

**Wards** are the protection spells behind the "−P *unless warded*" clause —
**Cap**, **Halve**, and **Redirect** — now part of the full grimoire below.

### Grimoire — the 15 spells

**Visibility rule (DECIDED):** a spell is **visible when activated, else hidden.**
A spell in hand is hidden; an **Active** spell primed face-down stays hidden (the
bluff — *"does the detonator have a ward?"*) until it **fires**, at which point the
whole table sees who cast what. An **Instant** spell activates on cast, so it is
visible immediately (caster + effect public). This trades §9's "silent effects" for
a richer signaling layer — and makes **Peek non-anonymous** (a change from §9's
"someone peeked"). Blind volatility (§4) survives: a visible Surge/Dampen shows its
*delta*, never the absolute pot total, which only Peek reveals.

**Two timing modes:** **Instant** (fires on cast, then spent) · **Active** (primed
face-down, fires on its trigger, then spent — an unfired Active is a wasted bet).

| # | Spell | Role | Mode | Effect |
|---|---|---|---|---|
| 1 | **Peek** | Info | Instant | Learn the exact boiling point. The god-tier info card. |
| 2 | **Expose** | Info | Instant | Reveal one face-down ingredient in the pot to the table. |
| 3 | **Assay** ✨ | Info | Instant | Privately learn the current dominant color + its point lead (read the *prize*, not the danger). |
| 4 | **Dampen** | Vol control | Instant | −X cauldron volatility (cool it). |
| 5 | **Surge** | Vol control / offense | Instant | +X cauldron volatility (heat it toward the edge). |
| 6 | **Quench** ✨ | Vol control | Instant | The cauldron **cannot explode next wave** (table-wide) — everyone piles in, the pot fattens, terror resumes after. High-impact. |
| 7 | **Cap** | Ward | Active | As a detonator, eat at most **−3**. |
| 8 | **Halve** | Ward | Active | As a detonator, eat **−½P**. |
| 9 | **Redirect** 🌶️ | Ward / offense | Active | As a detonator, shove your **−P onto a chosen player** (cascades through their wards). |
| 10 | **Double Down** | Score | Instant | **Double** one color's points already in the pot. |
| 11 | **Sour** ✨ *(Copycat rework)* | Score / offense | Instant | **Halve** one chosen color's points in the pot — tear down a leader. |
| 12 | **Harvest** ✨ | Cash-in | Active | If **your** color wins the pot → **+X bonus** on top of your take. |
| 13 | **Skim** ✨ *(Recall rework)* | Economy / defense | Instant | **Discard the last ingredient you added** — its points & volatility leave the pot (sheds detonator liability, cools the cauldron). |
| 14 | **Forage** ✨ | Economy | Instant | Draw **2 spells** from your grimoire — the only in-round replenisher (the O2 "unless a card lets you draw more" hook). |
| 15 | **Hex** ✨ 🌶️ | Offense / curse | Active | Choose a player; if the pot **explodes** this round they take **+X extra damage**, detonator or not — directed loss without detonating yourself. |

✨ = new this round. **Copycat → Sour** and **Recall → Skim**: both reworked away
from "reach into the hidden pot and pick a specific card" (fiddly, and you can't
see the pot anyway) toward clean total-based / self-targeted effects.

All values `[needs playtesting]`. Discipline (§9) holds: one-icon-readable, none
wins alone. The **god-tier** (Peek ↔ Wards, know vs survive) is drafted via the
Apothecary below.

## Deck-building — The Apothecary (DECIDED)

Two separate ledgers, drafted before the game. **You curate a small SET of named
buckets; a server-side realizer composes the actual deck.** Core model: **buckets
feed *availability*, not distribution** — a bucket makes a *family* of cards
*eligible*; it does **not** set how many. **No coins, no weighting.** You pick
**2–3 buckets per ledger** (one of each type, **no duplicates** — quantity is
meaningless under availability), and the realizer builds a **fixed-size, capped,
color-anchored** deck from the eligible pool, **re-rolled every game**. So the deck
is always the right size and always novel; the *number* of buckets only changes
**focus vs breadth** (2 → concentrated, 3 → varied), not size or power.

All **caps live in the realizer**, not the selection — color-anchor ~75% own,
toolkit ≤~25%, Treasure ≤~3, god-tier ≤~2 — so any legal set of picks yields a
legal deck. Toolkit is *optional*: pick no toolkit bucket → ~100% own color (pure
Loyalist); the Loyalist↔Diplomat axis is just "did you take a toolkit bucket?"

Public recipe = the buckets you took (the shopping-list table read). Realization
(the actual cards + draw order) stays hidden → you learn your deck as you draw (C5).

**Premium pick:** by default a bucket rolls *random cards within its family*; you
get **one "reserve" per grimoire** to lock a single *named* spell (e.g. *Redirect*,
not "a Ward"). Certainty for one card, breadth for the rest. Grimoire only — the
pantry is always pure-roll. `[needs playtesting]`

### Pantry buckets (pick 2–3 of 12) — fixed **30-card** shoe

| Group | Bucket | Eligible flavor it unlocks |
|---|---|---|
| Posture | 🌿 Sage | low-volatility own-color (safe pushes) |
| | 🍃 Mint | balanced mid-vol / mid-point own-color |
| | ☠️ Nightshade | high-volatility own-color weapons |
| | 🌺 Saffron | high-point own-color treasure *(realizer ≤~3)* |
| | 🪨 Chalk | 0-point "ghosts" — volatility + color presence, no prize |
| | 🍇 Bilberry | greedy own-color — high-vol **and** high-point |
| Toolkit *(opt, ≤25%)* | 🎨 Ochre | off-color cards (kingmake / misdirect) |
| | 🌫️ Wisp | wilds — colorless pure-danger / go-neutral |
| Chemistry | 🔗 Bramble | named-combo pairs (the O3 sage+mint synergies) |
| | 🍯 Honey | count-threshold cards — score more in big / late pots |
| Specialist | 🌱 Hellebore | ultra-low-vol "tiptoe" cards (dodge detonator liability) |
| | 🌶️ Embercap | escalating cards — volatility climbs the longer they sit |

### Grimoire buckets (pick 2–3 of 8) — fixed **20-spell** hoard

| Group | Bucket | Spells it makes eligible |
|---|---|---|
| God *(≤2)* | 👁 Eyebright | Peek |
| | 🛡 Ironbark | Cap · Halve · Redirect |
| Info | 🔍 Farsight | Expose · Assay |
| Offense | ⚡ Brimstone | Surge · Hex |
| Disruption | 🌀 Wormwood | Sour · Skim |
| Cash-in | 💰 Goldenseal | Harvest · Double Down |
| Defense | ❄️ Hoarfrost | Dampen · Quench |
| Tempo | 📜 Mandrake | Forage |

Names & exact card contents `[needs playtesting]`.

## The 12 Brewers (C3) & the pre-game flow

**Pre-game flow (DECIDED):** the start-of-game phase runs **Brewer first, then the
Apothecary** — pick your Brewer (1 of 2, from disjoint pairs so the table is unique,
public), *then* draft pantry + grimoire knowing who you are. Building the deck to
fit the Brewer is the **synergy hunt** — the richest skill expression (§C.3 Fork 3);
several Brewers hook straight into the draft.

**Design discipline (§B.1, unchanged):** one sentence, one bent rule, instantly
readable; must create **reads for the whole table**, never just a private stat;
**no free explosions** (half-damage is the absolute ceiling) and **no free perfect
information** (info-Brewers bend the *flow*, never the answer). Public, so the table
plays around them. 🌶️ = high-impact, needs care. Each hooks a different v2 system so
no two pull the same lever:

| Brewer | Hooks | One bent rule |
|---|---|---|
| **Featherhand** | Detonator sort (O1) | In the fatal-wave volatility sort, your cards count as the **lowest** at their value — you slip out of every tie. |
| **Cinderwright** 🌶️ | Wards / −P | When you're a detonator you take **half** damage — but you can **never** play a Ward. |
| **Connoisseur** | Apothecary draft | You draft a **4th bucket** in one ledger. |
| **Reservist** | Apothecary draft | Your grimoire holds **two reserves** — lock two exact spells, not one. |
| **Channeler** 🌶️ | Spell economy (O2) | You may play **two spells per wave**, not one. |
| **Forager** | Ingredient hand | You top up ingredients to **4** each wave, not 3. |
| **Herbalist** | Compounding (O3) | Your named combos fire from **a single half** — you never need both ingredients in the pot. |
| **Distiller** | Compounding (O3) | Your count-threshold cards treat the pot as **2 cards larger** — payoffs come online sooner. |
| **Alchemist** 🌶️ | Compounding (O3) | When one of your combos fires it also **adds volatility** to the pot — chemistry as a weapon. |
| **Eavesdropper** | Info flow | Whenever **anyone** casts Peek, you secretly learn the boiling point too (conditional, public — bends flow, not the answer). |
| **Broker** | Scoring | When you split a pot (Alliance/Commune) you round **up**, not down — everyone's preferred ally. |
| **Lurker** 🌶️ | Wave commitment | **Once per round** you may commit your card **after** the wave reveals (sees who/how-many, not volatilities). |

All effects `[needs playtesting]`. Harness matrix is **persona × Brewer ×
deck-archetype**; the ≥12 must be mutually balanced (draw 8 of 12 per game, disjoint
pairs → unique table).

## Open sub-questions

- **O1 — Detonator identification (the keystone). DECIDED.** At explosion, take
  the **fatal wave only** (not the whole pot), applied on top of the hidden
  pre-wave total, and sort *that wave's* cards **ascending by volatility**. The
  **trigger** is the first card that pushes the cumulative total past the boiling
  point; the trigger player **and everyone in that wave holding a higher-volatility
  card** are "tied" and **split −P** equally; **equal-volatility cards are
  simultaneous** (all liable if one triggers). Only players who committed in the
  fatal wave can be liable — so **folding before the fatal wave is safe** (a
  stronger fold-to-safety / Vulture dynamic than whole-pot would give, accepted in
  exchange for sharper per-wave decisions and immediate causality). **The depile
  sorts by volatility on *every* round, boom or safe** (O1.2) — the consistent
  "fuse climb" presentation; on a safe brew it climbs and stops short. **Resolved:**
  (a) the sort uses **effective** (post-compounding) *per-card* volatility —
  consistent with the trigger math; cauldron-level modifiers (Surge/Dampen) sit in
  the running total, not the per-card sort. (b) The **boiling point is revealed at
  the depile every round** (boom *and* safe) — the always-climb depile lands its
  line, so a safe brew gets the near-miss payoff ("22 — the line was 24"). This
  **changes §10** (which hid B on safe brews); Peek is unharmed — its value is
  in-round, the depile is post-round.
- **O2 — Spell economy. DECIDED.** Each wave you **must play an ingredient or
  pass** (pass = locked out, as §5); **playing a spell is optional**, up to **1
  per wave**, layered on an active turn — a spell never substitutes for the
  ingredient/pass choice and never keeps a folded player in. **Ingredients top up
  to 3 after every wave** `[needs playtesting]` (a leaner hand than the old
  refill-to-5: tighter, more readable choices, partial scarcity restored; folding
  is the escape when your hand is all wrong). **Spells are hoardable** — drawn at
  round start, **not replenished within a round**, unused ones **carry over**
  between rounds (the husbanding decision §13 wanted, moved to the grimoire). Open:
  spells-per-round draw count; boiling-point rescale for faster pot growth;
  spell-play visibility (does the table see *that* a spell was played?).
- **O3 — Ingredient compounding. DECIDED (direction).** Ship **count-threshold**
  compounding on ingredients (keys off the *public* card count — plannable,
  legible) plus **named-ingredient combo bonuses** (sage + mint… — the drafting
  names earn mechanical teeth). **Combos are bonuses, never requirements** (a card
  is always fine alone, better paired — avoids dead draws in a deck you don't fully
  know, C5). **Color-synergy** compounding (hidden, snowball-prone) is **capped or
  Peek-gated** if used at all. **Reach-in manipulation** (Double Down, copy-the-
  dominant-color) stays in the **grimoire** as spells, not baked onto ingredients.
  Effective (post-compounding) volatility feeds the O1 sort.
- **O4 — Deck-building / drafting. DECIDED → see *The Apothecary* above.** Two
  ledgers; pick a **set of 2–3 named buckets** per ledger (one of each, from **12**
  pantry / **8** grimoire); buckets feed *availability*, the realizer composes a
  fixed-size, capped, color-anchored deck (re-rolled each game); roll-within-bucket
  by default, **one *reserve* per grimoire** to lock an exact spell. Settled the
  three sub-forks: **two ledgers**, **roll + reserve**, **12 / 8** bucket types.

### Downstream re-opens (flagged, not yet scheduled)

- The **Peek economy** (§16's #1 knob) stops being a designer constant — it goes
  *player-shaped* (recipes) **and** *fired-less* (Vote/Spell). Whole blind-
  volatility tuning is re-derived.
- The **pre-game phase** (Brewer pick + recipe build) must fit a fast lobby with
  sensible defaults (§14 ethos), even at the new longer session length.
- Harness matrix grows to **persona × Brewer × deck-archetype × explosion-model**.
- **Localization lands with v2** ([05_roadmap.md — Localization](05_roadmap.md):
  EN/FR/ES/DE/IT + Latin flavor locale). The v2 content *is* the translation
  surface — 12 Brewers, 15 spells, 20 buckets, each a name + one-sentence rule
  text — and the §B.1 bar (*one sentence, instantly readable*) must hold in every
  shipped language, so naming/wording review happens per-language at design time,
  not as a post-hoc pass.

---

## Starting numbers (first harness targets — all `[needs playtesting]`)

Internally-coherent starting points, **not decisions** — the bot harness (IV) owns
the real values and re-derives the blind-volatility economy from scratch.

| Knob | Start | Note / coupling |
|---|---|---|
| Volatility / card | **0–7**, skewed low (mean ~3) | high-vol 5–7 = rare Nightshade weapons |
| Boiling-point range | **20–32** (mid ~26) | ~2× the old 8–14: higher mean vol **+** up-to-4-card waves; **the load-bearing dial** |
| Points / card | **0–3**, colored votes only | → typical pot **P ≈ 10**, a fat one ~18–20 |
| Rounds / game | **5** | the main length lever |
| Wave timer | **~25s** wave 1, **~15s** after | deeper decisions than the old 10s |
| Game length | ~90s draft + 5×~2.5min ≈ **15–18 min** | hits the C2 target |
| Explosion-rate target | **~45%** of rounds | *higher* than the old 30–40% — only the detonator(s) suffer |
| **Pantry** | **30 cards** | own ~75% (~22) · toolkit ≤25% (~7) · Treasure ≤3 |
| **Grimoire** | **20 spells** | god-tier ≤2 · 1 reserve |
| Ingredient top-up | **3** / wave | (decided, O2) |
| Spells drawn | **3** / round-start *(bumped for the 20-card grimoire)* | open (O2): ~15 drawn over the game; Forage +2 |
| Dampen / Surge | **−3 / +3** vol | scaled to the wider range |
| Cap / Halve / Redirect | **≤3 / −½P / full −P→target** | Cap is strong when P≈10 |
| Hex / Harvest | **+5 on boom / +3 on win** | the punish & the cash-in |
| Honey / Bramble | **+1 pt per card past the 5th / +2 when paired** | count-threshold & combo (O3) |

**Premium caps stay absolute** (Treasure ≤3, god-tier ≤2) — bigger decks add
*commons*, not more premium; in the 20-card grimoire god-tier is now proportionally
rarer (~10%), and the **reserve** guarantees ≥1 god-tier spell if you want it.

---

## Pointers

- Rationale & alternatives: [`06_depth-and-complexity.md`](06_depth-and-complexity.md)
- Canonical (current) rules this reworks: [`02_game-design.md`](02_game-design.md)
- Parked platform work (accounts, ranked): [`05_roadmap.md`](05_roadmap.md)
