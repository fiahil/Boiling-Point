## Context

Compounding is the O3 decision in [docs/06_boom2/02](../../../docs/06_boom2/02_toward-a-v2-core.md): ship count-threshold + named-combo content, keep it legible, and don't overload the hidden pot. It depends on `boom2-combat-core` (effective-volatility feed; the Bramble/Honey buckets' *effects*) and `boom2-apothecary` (those buckets' *availability*). It is sequenced **last** because it's the spiciest for the hidden-pot legibility and the harness.

## Goals / Non-Goals

**Goals**
- Count-threshold scoring keyed off the public card count (plannable).
- Named-combo **bonuses** (never requirements) that give the drafting names teeth.
- Effective volatility feeding the explosion check + detonator sort.
- A cap/gate on color-synergy so nothing snowballs in the hidden pot.

**Non-Goals**
- Reach-in manipulation on ingredients (stays grimoire — Double Down, Sour).
- Deep, opaque content-based compounding that turns the hidden pot into noise.

## Decisions

### D1: Knowability is the axis

**Knowable** compounding (public-count thresholds) is the safe, plannable first class. **Hidden** compounding (content-based color synergy) is spicy and capped/Peek-gated. Combos are *knowable to the drafter* (you took the Bramble bucket) but *opportunistic in realization* (you don't know your exact cards), so they must be **bonuses, never requirements** — otherwise an owner-unknown deck produces dead-draw feel-bad.

### D2: Compounding on ingredients, manipulation in the grimoire

Passive/self/count/combo effects may live on ingredients (legible building blocks); anything that reaches into another color's totals is a **spell**. This keeps ingredients readable and concentrates swingy table-reaching power in the scarce, gated grimoire.

### D3: Effective volatility is the shared contract

Combo-added volatility flows through the **same** effective value the combat core already sorts/checks on — so a combo can change the detonator, consistently. This is why combat core specified "effective volatility drives the sort" up front.

## Constitution Check

| Principle | Compliance |
|---|---|
| **I — Server-authoritative** | The server evaluates thresholds (public count), combos (both halves present), and effective-volatility recomputation; the client only renders what the depile narrates. No compounding logic is client-side. |
| **II — Agent-driven** | Compounding is ingredient data + server evaluation; the harness stress-tests snowball and dead-draw rates headlessly. |
| **III — Start simple** | Sequenced **last**; ships the **legible** classes (count-threshold, combo-bonus) and **caps/gates** the snowball-prone one; explicitly keeps reach-in effects out of ingredients. **Rejected simpler alternative:** no compounding — but that leaves the Bramble/Honey buckets and three Brewers (Herbalist/Distiller/Alchemist) without teeth, and drops the "brewing chemistry" the design wants. |
| **IV — Playtest-driven** | All magnitudes `[needs playtesting]`; the harness validates no color-synergy snowball, combos remain bonuses (no dead-draw penalty), and the threshold values reward big/late pots without trivializing safe-treasure. |

## Risks / Migration

- **Hidden-pot legibility:** content-based synergy is opaque; the cap/Peek-gate and the depile narration (showing what fired) are the mitigations.
- **Dead-draw feel-bad:** enforced away by the bonus-not-requirement rule; the harness measures lone-half frequency.
- **Detonator surprises:** combo-added volatility can shift liability; acceptable and intended (the Alchemist Brewer weaponizes exactly this), but the depile must explain it.
