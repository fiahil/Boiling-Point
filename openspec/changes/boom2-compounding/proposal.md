## Why

Ingredients in the combat core are independent atoms. The "brewing recipes" fantasy — and the depth the Bramble/Honey buckets promise — comes from letting ingredients **interact in the pot** ([docs/07](../../../docs/07_toward-a-v2-core.md), O3). This change adds **compounding**: count-threshold ingredients and named-ingredient combos, on top of `boom2-combat-core` and `boom2-apothecary`.

## What Changes

- **Count-threshold compounding (Honey):** ingredients that score more in big/late pots, keyed off the **public** card count — plannable and legible.
- **Named-ingredient combos (Bramble):** paired bonuses (e.g. Sage + Mint) that give the drafting names mechanical teeth. **Combos are bonuses, never requirements** — a card is always fine alone, better paired (avoids dead draws in an owner-unknown deck).
- **Effective volatility feeds the detonator sort.** A card's *effective* (post-compounding) volatility is what the explosion check and the fatal-wave sort use (already required by combat core; this change is the first source of cards whose effective volatility differs from printed).
- **Color-synergy compounding** (hidden, snowball-prone) is **capped or Peek-gated** if used at all.
- **Reach-in manipulation stays in the grimoire** (Double Down, Sour) — compounding does **not** bake table-reaching effects onto ingredients.
- Activates the three compounding Brewers from `boom2-brewers` (Herbalist, Distiller, Alchemist).

## Capabilities

### New Capabilities

- `boom-compounding` — in-pot ingredient interactions: count-threshold scoring, named-combo bonuses (bonus-not-requirement), the effective-volatility feed into resolution, and the color-synergy cap.

### Modified Capabilities

<!-- Compounding extends boom-cards (ingredients gain optional compounding tags)
     and relies on boom-resolution's "effective volatility drives the sort"
     requirement (already in combat core). Both are same-saga capabilities, so the
     interactions are specified here as boom-compounding requirements. -->

## Impact

- **Protocol crate:** compounding tags on ingredients (threshold params, combo-pair ids); the depile must narrate compounding that fired.
- **Server engine:** evaluate count-thresholds (public count), combo bonuses (both halves present), and effective-volatility recomputation feeding the explosion check + sort.
- **Clients:** the depile shows when a combo/threshold fired and its contribution.
- **Balance (IV):** the harness validates that compounding doesn't snowball (color-synergy cap) and that combos stay bonuses (no dead-draw feel-bad); activates Herbalist/Distiller/Alchemist hooks.
