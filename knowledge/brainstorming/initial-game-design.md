# Bubbling Point — Initial Game Design

## Core Loop

- **Players:** 3–4 players, free-for-all
- **Rounds:** 5–7 per game

Each round, a shared cauldron starts empty. Players have a hand of ingredient cards. Everyone takes turns adding ONE card face-down to the cauldron (or passing). Players can go around multiple times. The round ends when all players pass consecutively.

## Push-Your-Luck Layer

Every ingredient has a **volatility value (1–3)**. The cauldron has a **hidden bubbling threshold** drawn each round (between 8 and 14). Nobody knows exactly where it is — you only get clues:

- **"The cauldron rumbles"** when you cross 50%
- **"It's glowing"** at 75%

If total volatility exceeds the threshold → **BOOM**. The cauldron explodes:
- Whoever added the **last ingredient** before the bang **loses big**
- Everyone else **loses a little**

If the cauldron resolves safely → it brews a **potion**, and the potion's effect depends on the ingredient colors everyone contributed.

## Game Theory Layer — Color Politics

Ingredients come in **4 colors** (one per player). The potion's outcome follows a simple majority rule:

| Outcome | Who Scores |
|---|---|
| **One color dominates** (3+ of one color) | That player scores huge, everyone else gets nothing. Selfish play. |
| **Two colors tied** | Those two players split a good score. Informal alliance. |
| **All colors roughly equal** | Everyone gets a moderate score. Cooperative utopia. |
| **Your color is absent** | You score zero but took no explosion risk either. Free-rider play. |

Every card you play is simultaneously:
- A **vote** for your interests
- A **risk** of explosion
- A **signal** to other players

## Card Hand — Drafting + Scarcity

At the start of each round, deal **5 cards** to each player from a shared deck. Cards have:

- A **color** (your signature color + neutral "wild" cards)
- A **volatility value** (1, 2, or 3)
- A **special effect** (maybe 1 in 5 cards has one)

**Special effects:** "peek at the threshold," "swap the last ingredient played," "this card counts as 2 colors," "reduce volatility by 2 but score nothing."

You **keep unplayed cards between rounds**, so hand management matters. Burning a low-volatility card now means you won't have it for a safer play later.

## Emergent FFA Prisoner's Dilemma Strategies

| Strategy | Description |
|---|---|
| **The Aggressor** | Floods their color, dares others to challenge or bail |
| **The Diplomat** | Plays neutral/shared colors, tries to build "everyone wins" pots, brokers peace |
| **The Vulture** | Passes early, free-rides on safe pots, avoids all risk |
| **The Saboteur** | Intentionally pushes volatility high with no color commitment, trying to make someone else be the last player before the boom |

Alliances are fluid. Two players might silently cooperate on a color split for a few rounds, then one betrays. The Vulture strategy works until everyone notices and starts targeting low-contribution pots.

## Table Talk

Nothing stops players from negotiating out loud: "I'll stop adding if you do." "Let's both go blue this round." But all placements are **face-down**, so promises mean nothing. Pure bluffing territory.

## Game Theory Framework (Work in Progress)

The game should produce four distinct emotional textures depending on the round context (objective, score state, multiplier). Each maps to a classic dilemma, but players don't need to know game theory — the feelings emerge naturally.

### 1. Prisoner's Dilemma — "The Betrayal"

*Already the core of color politics.*

Two players are informally allied on a color split (+3 each). Either could pivot to domination (+5 for the betrayer, +0 for the loyal partner). If both betray simultaneously, neither dominates and they dilute each other — a third player might benefit.

**When it peaks:** Late rounds at ×2 multiplier. Domination is +10, alliance is +6. The gap tempts betrayal. Two cooperators who've been allied all game face the final round knowing the math favors defection.

**Design lever:** The round multiplier is the main amplifier. No new mechanics needed — the escalation does the work.

### 2. Chicken — "The Nerve Test"

*Already the core of the volatility mechanic.*

Two (or more) players keep adding cards, each daring the other to pass first. Passing concedes influence over the pot. Not passing risks being the Detonator. If nobody blinks, the cauldron explodes and the highest-volatility contributor eats -4 (×multiplier).

**When it peaks:** When the cauldron hits "rumble" (50%) and multiple players are still invested. Each additional card is a game of nerve — you want the OTHER person to stop.

**Design lever:** Objectives can isolate this. Examples:
- *"Nerve Test"* — the last player to add a card before all passes scores +3 (even if it later explodes). Rewards brinkmanship.
- *"Volatile Brew"* — bonus +2 to all scorers if the cauldron resolves above 75% threshold. Collective chicken: everyone wants to push close to the edge without going over.

### 3. Stag Hunt — "The Trust Pact"

*Implicit in alliances, should be sharpened.*

Both players *want* to cooperate. Unlike PD, there's no incentive to betray if you believe your partner will follow through — the fear is being the sucker who commits while the other pivots. The dilemma is trust under uncertainty, not temptation.

**When it peaks:** Early-to-mid game when alliances are forming. Player A and B have been splitting blue/green. Round 3: will they both commit again, or will one panic and go for solo domination "just in case"?

**Design lever:** An objective that makes cooperation strictly better than defection — removing the PD temptation and isolating the trust problem:
- *"Pact Round"* — if exactly two colors are tied for majority, those players score +5 each (instead of the normal +3). Alliance is now *better* than domination (+5 vs +5) — but only if both follow through. Trusting and being right is the best outcome. Trusting and being wrong is the worst.

**Interaction with PD:** Stag Hunt and PD pull in opposite directions. A "Pact Round" objective followed by a high-multiplier round creates whiplash — last round rewarded trust, this round rewards betrayal. Players who built trust now have to decide whether the relationship survives the incentive shift.

### 4. Kingmaker — "The Legacy Play"

*Emergent in late-game FFA, should be embraced.*

A player who can't win anymore still has power: their cards can decide who does win. The player at -6 in round 5 can flood one color and hand the game to an ally (or deny it to an enemy).

**When it peaks:** Final round, when at least one player is too far behind to win but has cards to play. Their motivation shifts from "maximize my score" to "choose the winner."

**Design levers:**
- This emerges naturally from the score-can-go-negative system — trailing players are never eliminated, so they always have agency.
- The Mimic card amplifies this: a kingmaker can boost someone's color without spending their own.
- Optional formalization: *"Legacy bonus: if you finish the game with the lowest score, the player you contributed alongside the most across all rounds gets +2."* Makes the kingmaker role visible and strategic — you might *want* to be in last place if it lets you crown your preferred winner. But also creates a perverse incentive to tank your own score, so this needs playtesting.

**Why not to over-formalize it:** Kingmaker is most fun when it's emergent and social. "I'm giving this to you because you didn't betray me in round 3" is a human moment. A mechanical bonus might reduce it to optimization. Consider keeping this as a natural consequence of the rules rather than an explicit mechanic.

### How They Interact Across a Game

A typical 5-round game might produce this emotional arc:

| Round | Multiplier | Dominant Dilemma | Typical Feel |
|---|---|---|---|
| 1 | ×1 | Stag Hunt | Cautious cooperation. "Can I trust you?" |
| 2 | ×1 | Chicken | Testing boundaries. "How far will you push?" |
| 3 | ×1.5 | PD emerges | Stakes rise. Alliances crack. "Should I betray first?" |
| 4 | ×1.5 | PD peaks | Betrayals happen. Scores diverge. "You backstabbed me!" |
| 5 | ×2 | Kingmaker + PD | Trailing players choose winners. Leaders play defense. "Who do I crown?" |

This isn't scripted — it emerges from the multiplier curve and human behavior. Round objectives can nudge specific dilemmas (a Pact objective in round 3 delays PD; a Nerve Test objective in round 2 accelerates chicken). The design goal is that no two games feel the same because the dilemma mix depends on who's playing and what the objectives are.

---

## Cauldron Modifiers (Work in Progress)

Separate from round objectives (which modify scoring), cauldron modifiers change the *physics* of the round — how the cauldron itself behaves. These could be drawn alongside or instead of objectives, or stacked in later rounds for escalation.

| Modifier | Effect | Dilemma It Sharpens |
|---|---|---|
| **Bountiful Brew** | If the pot resolves, all players score +1 per card in the pot (regardless of color), on top of normal scoring. | Tragedy of the Commons → Chicken. Everyone wants to pile cards in for the bonus, but more cards means more volatility. Greed vs. caution. |
| **Thin Ice** | Threshold range narrows to 6–9 (instead of 8–14). Explosions are much more likely. | Chicken. Every card is a dare. |
| **Deep Cauldron** | Threshold range widens to 12–18. Explosions are rare — but the penalty for detonating doubles (-8 instead of -4). | PD. Safe to commit, so the politics matter more — but the punishment for overcooking is devastating. |
| **Fog** | No rumble/glow warnings this round. Players get zero information about cauldron state. | Stag Hunt. Pure trust — you can't calculate, you can only commit or bail. |
| **Residue** | Cards from the previous round's explosion carry over into this round's cauldron (their volatility counts, but their colors don't). Starts the round already hot. | Chicken. Starting at 50%+ means every card is nerve-wracking from turn 1. |
| **Catalyst** | The first card played each wave has double volatility. | Chicken. Nobody wants to go first. Simultaneous play (Alt D) makes this especially tense. |

**Bountiful Brew as a late-game escalator:** In rounds 4–5, Bountiful Brew stacked with the ×2 multiplier creates explosive incentives. A successful 8-card pot scores +8 to everyone (×2 = +16) on top of color scoring. The total value is enormous — but so is the explosion risk. Players who've been cautious all game suddenly want to pile in. This could be the "gold rush" moment that makes final rounds feel distinct from early ones.

**Open questions:**
- Should modifiers be random (drawn from a deck) or follow a set progression (e.g., round 5 is always Bountiful Brew)?
- Can modifiers stack? Bountiful Brew + Thin Ice = "huge reward, tiny margin" — exciting but possibly too swingy.
- Should players see next round's modifier in advance? Advance knowledge lets you plan (save Dampen cards for Thin Ice round), which adds strategy. Surprise keeps rounds unpredictable.

---

## Scoring System (Work in Progress)

### Core Concept: Unified Score Track

One number per player. Starts at **0**. Can go negative. Highest score after the final round wins.

This is simultaneously your victory points and your "health." There's no elimination — a player at -5 is still in the game, just desperate. Desperation is good: it makes players take visible risks, which everyone else can read and exploit.

### Scoring a Successful Brew

When the cauldron resolves without exploding, score based on color distribution:

| Outcome | Condition | Score |
|---|---|---|
| **Domination** | Your color has strict majority (more than any other single color) | **+5** to you, **+0** to everyone else who contributed |
| **Alliance** | Two colors tied for most | **+3** each to those two players |
| **Commune** | Three or more colors tied, or no single color above 2 | **+2** to every player who contributed at least 1 card |
| **Absent** | You contributed 0 cards | **+0** (free-rider: no risk, no reward) |

Wild cards count toward no player's color (they're neutral filler that adds volatility without political weight).

### Explosion Penalties

| Role | Condition | Penalty |
|---|---|---|
| **The Detonator** | Contributed the most volatility in the round | **-4** |
| **Bystander** | Contributed at least 1 card | **-2** |
| **Spectator** | Contributed 0 cards | **-0** (dodged it entirely) |

Ties for Detonator: all tied players take -4. This is harsh on purpose — it discourages high-volatility arms races.

*Note: with simultaneous play (Alt D), "most volatility" replaces "last card before boom" as the blame mechanic. Works in both sequential and simultaneous modes.*

### Game Arc: Round Multiplier

All scoring (gains AND penalties) is multiplied by a round factor:

| Round | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|
| **Multiplier** | ×1 | ×1 | ×1.5 | ×1.5 | ×2 |

(For 7-round games: 1, 1, 1, 1.5, 1.5, 2, 2)

**What this creates:**
- **Rounds 1–2:** Low stakes. Players feel each other out, test alliances, learn who's aggressive.
- **Rounds 3–4:** Stakes rise. The cooperative pair from rounds 1-2 is now eyeing the ×1.5 domination bonus (+7.5 vs +4.5 for alliance). Betrayal starts to tempt.
- **Round 5:** ×2 everything. Domination is worth +10. An alliance scores +6 each. The prisoner's dilemma peaks — if both partners go for domination simultaneously, they split the pot and neither dominates, while a third player might sneak a commune +4. If one betrays and the other doesn't, the betrayer gets +10 and the loyal partner gets 0.

**Alternative escalation:** Instead of (or in addition to) a multiplier, the cauldron threshold range could tighten in later rounds (round 1: 8–14, round 5: 8–10), making explosions more likely. Higher rewards AND higher explosion risk. Needs playtesting to see if stacking both is too chaotic or perfectly chaotic.

### Open Scoring Questions

- Are these numbers right? Domination (+5) vs Alliance (+3) — is the +2 gap enough to tempt betrayal? Might need to be +6 vs +3.
- Should the Vulture (0 cards, 0 risk, 0 reward) get *something* if the pot explodes? "Scavenger bonus" +1 for watching others suffer? Or is the safety itself the reward?
- Does the explosion penalty scale with how many cards you played, or is it flat? Flat is simpler. Scaled is more "fair."
- Should there be a bonus for the player who contributed the *least* volatility in a successful brew? Rewards careful, measured play.

---

## Information Architecture (Work in Progress)

Clarity on what each player knows, and when, is critical — every design alternative (objectives, depile, simultaneous play) reshapes the information economy. This is a reference for evaluating those alternatives.

### During a Round

| Information | Visibility | Notes |
|---|---|---|
| Your own hand | Private | Only you see your cards |
| Other players' hand sizes | Public | You can see how many cards they hold, not which |
| Cards in the cauldron | **Hidden** | Played face-down — color, volatility, and effects are unknown |
| Number of cards each player contributed | Public | Key political signal — you see WHO played, not WHAT |
| Cauldron warnings (rumble/glow) | Public | Fires at 50%/75% of threshold — everyone sees them simultaneously |
| Exact volatility total | **Hidden** | You only get the two warning thresholds as clues |
| Exact threshold value | **Hidden** | Drawn secretly each round; range is known (8–14) |
| Player scores | Public | Always visible — you can see who's desperate, who's leading |
| Round multiplier | Public | Everyone knows the stakes |
| Round objective (if using Alt A) | Public | That's the whole point |

### After a Successful Brew

| Information | Revealed? | Notes |
|---|---|---|
| Color majority result | **Yes** | Everyone learns which color(s) dominated — this is the scoring basis |
| Individual cards | **No (base game)** | The pot brewed; you don't know exactly who played what. Bluffing persists. |
| Exact volatility total | **No** | You only know it didn't exceed the threshold |
| Threshold value | **No** | Could be revealed as a "fun fact" post-round? Low-priority. |

### After an Explosion

| Information | Revealed? | Notes |
|---|---|---|
| Who was Detonator (most volatility) | **Yes** | Required for penalty assignment |
| Individual cards | **Depends on variant** | Base game: No. With Depile (Alt E): Yes, revealed in reverse order. |
| Color distribution | **Depends** | Base: only Detonator is identified. Depile: full reveal. |

### Key Design Tension

**Less info revealed → more bluffing, more paranoia, more table talk.**
"Was that you with three reds? I think it was you." Accusations fly based on vibes, not facts.

**More info revealed → more strategic calculation, more targeted alliances.**
"You played two reds last round, I saw it in the depile. I'm not cooperating with you again."

The base game leans heavily toward hidden info. The Depile (Alt E) swings toward revealed info but *only on explosions*, creating an asymmetry: cooperation stays murky, but disasters expose everyone. That's a nice compromise — explosions are dramatic AND informative.

**Open questions:**
- Should successful brews ever reveal cards? A "fast-forward depile" on success would give full information but remove the bluffing metagame. Probably too much.
- Should players see *which* cards they themselves contributed (a personal history)? In a physical game you'd remember. Online, a subtle "your contributions this round" panel could help without revealing info to others.
- In simultaneous play (Alt D), should players see who committed cards *during* the timer countdown (like seeing chips slide to the center in poker)? Or only after the wave resolves? Showing live commits adds mind games ("they committed fast — they're confident") but adds UI complexity.

---

## Special Effects Catalog (Work in Progress)

Design principles:
- **Few effects, each memorable.** Target 8–10 unique effects in the deck. Players should recognize them by icon, not need to read text.
- **No effect wins the game on its own.** Effects are nudges, not bombs.
- **Every effect has a cost.** Usually: the card still goes in the cauldron with its volatility. Playing a special effect card is never "free."
- **Effects should create stories.** "Remember when you peeked at the threshold and STILL blew it up?" is the goal.

### The Catalog

| Effect | Icon | Card Volatility | What It Does |
|---|---|---|---|
| **Peek** | Eye | 2 | You privately see the exact threshold value. You still played a volatility-2 card to learn this. |
| **Dampen** | Snowflake | 1 | This card's volatility is 1, but it *also* reduces total cauldron volatility by 2. Net effect: -1 volatility. Safety play, but you used a card slot for defense instead of politics. |
| **Volatile Surge** | Lightning | 3 | Adds +2 extra volatility on top of its base 3 (total 5 effective). A weapon. Massive risk, but forces the pot closer to the edge. Pairs with Peek for a precision strike. |
| **Mimic** | Mirror | 1 | This card copies the color of the last card added before it (or wild if played first). Lets you reinforce someone's color without spending your own cards. Subtle alliance move. |
| **Swap** | Arrows | 2 | Swap this card with a random face-down card already in the cauldron. You get that card back into your hand. You don't know what you'll get — could rescue your own high-volatility mistake, or steal someone's political play. |
| **Shield** | Shield | 2 | If the cauldron explodes this round, you take no penalty (but you still don't score). Personal insurance. Other players don't know you have it, so they can't account for your reduced risk. |
| **Expose** | Lantern | 1 | Reveal one random face-down card in the cauldron to all players. Information warfare — might expose a hidden alliance or reveal how close to the threshold you are. Low volatility as tradeoff: low risk, high disruption. |
| **Amplify** | Megaphone | 2 | Double the color weight of your next card played this round. If you play a red card after this, it counts as 2 reds for majority purposes. Combo piece — useless alone. |

### Deck Distribution (Rough)

For a 4-player deck (~80–100 cards total):
- ~60% player-colored cards (15 per color), no special effect, volatility 1–3
- ~15% wild cards, no special effect, volatility 2–3
- ~25% special effect cards, distributed across colors and wild

*All numbers need playtesting. The ratio of specials to normals determines how "chaotic" vs "pure" the game feels.*

### Open Questions

- Should special effects trigger on play (face-down into cauldron, effect happens immediately but secretly) or on reveal (during depile or brew resolution)? Immediate is simpler. On-reveal creates more surprises but is harder to reason about.
- Can you hold special effect cards across rounds, or do they expire? Current design says all cards carry over — specials included. Hoarding a Peek for the final ×2 round is a valid strategy.
- Should Peek reveal to just you, or should other players see that *someone* peeked (without knowing what they learned)? "Someone peeked" adds paranoia; private peek is cleaner.
- Is Shield too strong? Guaranteed explosion immunity for 2 volatility is a good deal. Maybe limit to 1 Shield in the entire deck?

---

## What Makes It Click

- Only **1 decision per turn** (play a card or pass) — dead simple
- Push-your-luck tension builds with every card added
- Game theory is **emergent**, not rules-heavy — no formal alliances, just human behavior
- Rounds are fast (**60–90 seconds** each)
- The "boom" moments are **hilarious and memorable**

## Design Alternatives Under Consideration

### Alternative A: Public Round Objective

**Idea:** At the start of each round, flip a public objective card visible to all players. It applies a scoring tweak for that round only, giving everyone a shared piece of information to negotiate around (or bluff about).

**Example objectives:**

| Objective | Effect |
|---|---|
| **Volatile Brew** | Bonus 2 points to all scorers if the cauldron resolves above 75% threshold without exploding |
| **Purity** | If one color has 3+ cards in the pot, that player scores +3 bonus |
| **Harmony** | If no single color has more than 2 cards, all scorers get +2 |
| **The Vulture's Feast** | Whoever contributed the fewest cards (min 1) doubles their score this round |
| **Dead Calm** | If total volatility stays below 50% of the threshold, all scorers get +2 |
| **Betrayal Brew** | The last player to add a card before all passes gets +3 (even if the pot explodes — they still take the explosion penalty too) |

**What it adds:**
- A public information layer — currently all tension comes from hidden info (face-down cards, hidden threshold). This gives players something to openly discuss and scheme around.
- Round-to-round meta shifts — a "Purity" objective after two cooperative rounds forces everyone to reconsider. Prevents stale strategies.
- Richer table talk — "we should all go for Harmony this round" becomes a thing players can say (and then betray).

**Risks:**
- Extra cognitive load per round. Mitigated by keeping objectives to one sentence / one scoring tweak.
- Could overshadow the core cauldron tension if bonuses are too large. Numbers need playtesting.

**Open questions:**
- How many unique objectives in the deck? 10–15 feels right for variety without needing to memorize.
- Should some objectives be negative ("if X happens, everyone loses points")? Could create dread rounds.
- Should players see the next round's objective in advance, or only the current one?

---

### Alternative B: Card Draw Mechanics

Three options, from most conservative to most disruptive.

**Option B1: No Draw (Current Design)**

Players get 5 cards per round, keep unplayed cards. No way to draw more. Scarcity is the point — every card spent is a card you won't have later. The binary play/pass decision stays clean.

*Best if:* we want maximum simplicity and want the "saving cards" strategy to stay meaningful.

**Option B2: Draw as Special Effect**

No standalone draw action. Instead, ~2–3 cards in the deck have a special effect: "Draw 1 card when played" or "Draw 2, discard 1." Drawing is a resource you burn, not a free action — you still add the card to the cauldron (with its volatility and color).

*Best if:* we want occasional hand-refill moments without adding a third action type. Keeps the play/pass binary intact.

**Option B3: Draw as Third Action**

On your turn, you can: play a card, pass, or draw 1 card from the deck. Drawing doesn't end your participation in the round (you can still play later), but it signals weakness — everyone sees you needed more cards.

*Implications:*
- "All consecutive passes" end condition still works (drawing isn't passing).
- Weakens scarcity pressure — the Vulture strategy becomes stronger (pass, draw, hoard, dominate late rounds).
- Adds a "tempo" dimension: drawing costs you a turn where you could've influenced the pot.
- Need a hand limit (7? 8?) to prevent infinite hoarding.

*Best if:* we want more agency over hand composition and are okay with longer rounds.

---

### Alternative C: Objectives + Draw Combined

The objective card system could occasionally interact with drawing:

- **"Research Round"** objective: players who pass also draw 1 card. Rewards patience.
- **"Scarcity"** objective: no keeping unplayed cards this round — discard your hand at round end. Creates urgency to play everything.
- **"Surplus"** objective: everyone draws 2 extra cards at round start. More cards in play = higher explosion risk.

This lets draw mechanics exist without being a permanent third action — they show up as round-specific events that players can plan around.

---

### Alternative D: Online-First Turn Structure

The base game is sequential: each player takes a turn (play or pass), one at a time, round-robin. This works great at a physical table but creates problems online:

- **Dead time:** You're waiting for 2–3 other players before your next action. With a per-player timer (say 15s), a 4-player round could take minutes of mostly waiting.
- **Unpredictable pacing:** One slow player stalls everyone.
- **Disengagement:** Watching others play face-down cards you can't see isn't compelling on a screen.

Three options for fixing this:

**Option D1: Simultaneous Waves**

Each round is split into **2–3 waves**. In each wave, all players simultaneously and secretly select 0–N cards from their hand, then hit "commit." A shared timer (15–20s) counts down. When the timer ends (or everyone commits early):

1. All committed cards are added to the cauldron at once.
2. The cauldron state updates — rumble/glow warnings fire if thresholds are crossed.
3. Everyone sees: total volatility change, total cards added, and how many cards each player contributed (but not which cards — still face-down).
4. Next wave begins. Players who committed 0 cards in a wave are "locked out" for the rest of the round (equivalent to passing).

The round ends after a wave where every remaining player commits 0 cards, or after 3 waves max.

*What changes:*
- Round time drops from "N players × timer" to "2–3 waves × shared timer" — predictable, fast.
- You can't react to individual plays within a wave — you commit blind, which actually *strengthens* the bluffing/commitment feel.
- The "last card before boom" penalty needs rethinking. Option: the player who contributed the most volatility in the wave that caused the explosion takes the biggest hit. Or: the penalty is split proportionally to volatility contributed in the final wave.
- Push-your-luck escalation is preserved — you see cauldron state between waves and decide whether to keep going.

*Risks:*
- Loses the granular "one card at a time" tension. The incremental dread of watching each card go in is part of the magic.
- "Who caused the boom" is less clear when multiple cards land simultaneously.

**Option D2: Simultaneous Single Card**

Simpler variant: each wave, every player commits exactly **0 or 1 card**. Same simultaneous reveal, same shared timer. Closer to the base game's rhythm but still eliminates sequential waiting.

*What changes:*
- More waves per round (~4–5) but each is very fast (10s timer).
- "Last card before boom" maps cleanly — if the boom happens after a reveal, all players who played a card in that wave share the penalty (weighted by their card's volatility).
- Feels like the original game but parallelized.

*Risks:*
- More waves means more reveal animations — could feel repetitive if not visually polished.

**Option D3: Async Queuing (Batch Commit)**

Players build a **queue** of cards they plan to play, in order. When everyone has locked in their queue (or timer expires), the game plays them out one at a time in interleaved order (round-robin through queues). Players can set a "stop if cauldron reaches glow" auto-bail condition.

*What changes:*
- Fastest wall-clock time: one planning phase, then automated resolution.
- Auto-bail conditions add strategic depth — "I'll play 3 cards but stop if it gets too hot."
- Resolution phase is a spectacle: cards flip one by one, cauldron reacts, players watch the drama unfold like a replay.

*Risks:*
- No mid-round adaptation. You can't change your mind after seeing someone else's card.
- Auto-bail conditions add UI/rules complexity.
- Might feel less interactive — more like programming a bot than playing a game.

---

### Alternative E: Explosion Depile (Reverse Reveal)

**Idea:** When the cauldron explodes, instead of just "BOOM — done," the game enters a **depile phase**: all cards in the cauldron are revealed one by one in reverse order (last added → first added). Each card flips face-up, showing its color, volatility, and any special effect.

**What it adds:**
- **Spectacle:** The explosion becomes a dramatic moment instead of an instant. Players watch the cards unspool, tension building as blame is assigned in real time. "Oh no, that was MY card." "Three reds?! You said you were going easy!"
- **Information reveal:** This is the moment everyone learns what was actually played. In the base game, face-down cards are hidden — the depile is the only time the truth comes out. This feeds directly into the politics of the next round ("I can't trust you after that").
- **Blame escalation:** Revealing last-to-first naturally builds toward "who started this mess" — the early high-volatility cards that set up the chain.

**Variants:**

| Variant | Description |
|---|---|
| **Pure reveal** | Cards flip for information only. No gameplay effect — the depile is pure drama. |
| **Reverse effects** | Special effect cards trigger their effect in reverse as they're revealed. A "reduce volatility by 2" card that was played early now *adds* 2 back, twisting the knife. |
| **Blame scoring** | Each revealed card assigns a fraction of the explosion penalty to its owner. Last card = biggest share, first card = smallest share. More granular than "last player before boom loses big." |
| **Survival reveal** | Cards are revealed and removed one by one. If at any point the remaining volatility drops below the threshold, the cauldron "stabilizes" — remaining cards score as a successful potion. Creates a secondary drama: "will the pot recover?" |

**Survival reveal** is particularly interesting — it means the explosion isn't always total. If the last 2–3 cards pushed it over, removing them might save the brew. Players who contributed early low-volatility cards could still score. This rewards careful early contributions and punishes reckless late plays.

**Interaction with simultaneous play (Alt D):**
- If using waves (D1/D2), "last added" within a wave is ambiguous. Could use volatility as tiebreaker (highest volatility in the wave = "last") or reveal the entire final wave first, then previous waves.
- If using queue (D3), order is well-defined since queues are interleaved.

**Open questions:**
- How long should the depile animation take? Needs to be fast enough to not bore (1–2s per card?) but slow enough to build tension.
- Should the depile happen on successful brews too? Could be a quick "fast-forward" reveal so players always learn what was in the pot. Or only on explosion for maximum dramatic impact.
- Does revealing all cards change the game's information economy too much? If you always learn everything after explosions, bluffing about your cards becomes less viable. Maybe only reveal the top N cards?

---

### Comparison Matrix

| Dimension | Base Game | + Objectives (A) | + Draw FX (B2) | + Draw Action (B3) | Combined (C) | Sim. Waves (D1) | Sim. Single (D2) | Queue (D3) | Depile (E) |
|---|---|---|---|---|---|---|---|---|---|
| Decision complexity | Low | Low-Med | Low | Med | Med | Med | Low-Med | Med-High | Low |
| Round length | ~60–90s | ~60–90s | ~60–90s | ~90–120s | Varies | ~40–60s | ~50–70s | ~30–45s | +10–20s on boom |
| Strategic depth | Med | High | Med-High | High | High | High | Med-High | High | Med (spectacle) |
| Scarcity pressure | High | High | Med-High | Med | Varies | High | High | High | No change |
| Table talk potential | Med | High | Med | Med | High | Med-Low | Med-Low | Low | High (post-boom) |
| Online suitability | Low | Low-Med | Low | Low | Low-Med | **High** | **High** | **Very High** | Med-High |
| Spectacle value | Med | Med | Med | Med | Med | Med-High | Med | Med-High | **Very High** |
| Implementation complexity | Low | Low-Med | Low | Med | Med | Med-High | Med | High | Med |

---

## Comparable Games

Blends the quick decision-making of *Exploding Kittens* with the emerging game theory of *Cosmic Encounter* or *Sheriff of Nottingham*.

---

## Conceptual Art Direction

### Art Style: "Arcane Punk Alchemy"

Stylized, vibrant, and highly legible. Prioritizes clarity for fast rounds while retaining a sense of impending, chaotic doom. Think *Hearthstone* meets *Arcane*. Not a dusty medieval library — a frantic, slightly dangerous workshop where magic is harnessed with questionable safety standards.

**Color Palette:** Rich, dark base world — weathered brass, heavy iron, dim parchment. Player colors pop dramatically:
- Ruby Red
- Sapphire Blue
- Emerald Green
- Amethyst Purple

"Bubbling" effects transition from cool teals to fiery oranges as danger increases.

### The Cauldron (Centerpiece)

An arcane pressure cooker wired with tubes and gauges.

| State | Trigger | Visuals | UI Cues |
|---|---|---|---|
| **Empty/Calm** | Start of round | Dark interior, no liquid | Volatility gauge at zero |
| **Rumble** | 50% threshold | Low vibrations, swirling liquid, steam vents | Gauge needle in yellow zone, ambient vibration pulses |
| **Glow** | 75% threshold | Rapid bubbling, pulsating orange glow, sparks between electrodes | Gauge needle buried in red, visible arcs |
| **BOOM** | Threshold exceeded | Full-screen white/orange flash, colored smoke cloud matching contributed colors, magical residue rains down | — |

### Ingredient Cards

**Card Back:** Arcane eye design, "BUBBLING POINT" burned into aged leather. All cards look identical face-down.

**Card Front (Player Color):**
- Background dominated by player's color (fiery red vortex, deep purple magic lines, etc.)
- **Volatility (1–3):** Flaming "Skull & Potion" icons at the top — most crucial info, read instantly
- **Special Effect:** Box at bottom with unique icon (e.g., Eye icon for "Peek at the Threshold")

**Card Front (Wild/Neutral):**
- Swirling silver or gold background, clearly not a player color
- Often higher volatility (2 or 3) as tradeoff for versatility

### Player UI/Perspective

- **Player Identifier:** Signature color prominent in hand area and resource track
- **Hand Management:** 5 cards fan-folded at bottom; unplayed cards glow softly (persist between rounds)
- **Action UI:** Two large buttons — "ADD INGREDIENT" (drag card to pot) or "PASS"
- **Scoring Track (The Potion Shelf):** Glass vials on screen side. Scoring fills vials with liquid; explosion penalties corrode them with thick gunk

### Politics Visualization

- **Contribution Tracker:** Display near cauldron showing how many cards each player contributed (not what was played)
- **"Saboteur" Warning:** Players with high volatility history get sparking electricity around their portrait

### Art Director's Pitch Summary

- **Visual Pillar:** "Unstable Magic, Shared Doom."
- **Atmosphere:** High-tension, competitive, slightly comedic in its volatility.
- **Style:** Stylized Arcane Punk. Dark, industrial brass contrasting with brilliant, magical player colors.
- **Key Art Directive:** Make the push-your-luck feel visceral. Players should feel the cauldron vibrating as they place that third card. Card readability priority: Volatility > Color > Effect.

