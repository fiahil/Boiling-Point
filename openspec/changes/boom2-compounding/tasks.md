## 1. Protocol & ingredient tags

- [x] 1.1 Add compounding tags to ingredients in the `protocol` crate: count-threshold params and named-combo ids. *(`Compounding` enum — `CountThreshold { past, per_card }`, `Combo { combo, member }` over a named `ComboId` of 2–5-member sets with `size()` — on `IngredientView`; PROTOCOL_VERSION 7→8.)*
- [x] 1.2 Extend the depile messages to narrate which combos/thresholds fired and their contribution; regenerate client wire types. *(`CompoundingFire` (carries combo `size`) on `DepileEntry`. No TS codegen exists yet — `clients/web/` lands with `adopt-pixi-client`; the Rust wire types are the source.)*

## 2. Count-threshold scoring

- [x] 2.1 Evaluate count-threshold bonuses at resolution from the public pot card count. *(`game::compounding::apply_count_thresholds`, off `pot.cards.len()`.)*
- [x] 2.2 Wire the Distiller Brewer hook (treat the pot as 2 cards larger for thresholds). *(`DISTILLER_POT_BONUS` consulted via `CompoundingBrewers`.)*

## 3. Named-combo bonuses

- [x] 3.1 Implement combo detection (all members of a named 2–5 set in the pot) and apply the size-scaling bonus (points, plus volatility for an Alchemist). *(`game::compounding::apply_combos` + `combo_bonus(size)`; fires once per owner; credits the owner's colour — cross-colour safe.)*
- [x] 3.2 Guarantee a lone member plays as a normal ingredient with no penalty (bonus-not-requirement). *(Tested: `lone_combo_member_is_a_plain_card`.)*
- [x] 3.3 Wire the Herbalist (combo fires twice) and Alchemist (combo also adds volatility) Brewer hooks. *(`HERBALIST_COMBO_MULTIPLIER` / `ALCHEMIST_COMBO_VOLATILITY` consulted.)*

## 4. Effective volatility & color-synergy

- [x] 4.1 Feed compounding-adjusted **effective** volatility into the explosion check and the fatal-wave sort (per `boom-resolution`). *(`Round` recomputes compounding each wave; `PotIngredient::effective_volatility` adds the combo delta — feeds check, sort, depile climb.)*
- [x] 4.2 If color-synergy content is included, enforce its cap / Peek-gate so it cannot snowball in the hidden pot. *(No color-synergy content ships — the cap is structural; the harness `compounding_snowball` smell guards the pot-share if it is ever added. Documented in `game::compounding` + docs/06_boom2/02.)*

## 5. Content & buckets

- [x] 5.1 Author the Bramble (named 2–5 combos) and Honey (count-threshold) ingredient content surfaced by the Apothecary buckets. *(`content.toml` `[[pantry_bucket]]` Bramble roster: one 2, two 3s, one 4, one 5; Honey count-thresholds; `BucketCard`/realizer carry the tag.)*
- [x] 5.2 Keep reach-in manipulation (Double Down, Sour) in the grimoire, not on ingredients. *(Unchanged — no ingredient carries a reach-in effect.)*

## 6. Clients & balance (Principle IV)

- [ ] 6.1 Web client (`clients/web/`): depile narrates fired combos/thresholds and any effective-volatility shift that changed the detonator. *(Blocked: `clients/web/` does not exist yet — it lands with `adopt-pixi-client`. The protocol carries everything the renderer needs: `DepileEntry.compounding` and the printed-vs-running volatility gap.)*
- [x] 6.2 AI client harness (`clients/ai` harness mode, §IV): measure color-synergy snowball, lone-combo-member frequency (dead-draw rate), and threshold payoff; tune magnitudes; record in docs/07. *(Harness records combo/threshold fires, lone-combo-member rate, compounding pot-share + a `compounding_snowball` smell; 1,800-game derivation recorded in docs/06_boom2/02. Shipped conservative/untuned — share < 40% at the four-Chemist max; named 2–5 combos almost never complete on owner-unknown decks (a rare-jackpot lottery), so thresholds carry compounding — the watch item. Findings live in docs/06_boom2/02 alongside the other harness derivations rather than docs/07, the visual design system.)*
