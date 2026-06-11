## 1. Protocol & ingredient tags

- [ ] 1.1 Add compounding tags to ingredients in the `protocol` crate: count-threshold params and named-combo pair ids.
- [ ] 1.2 Extend the depile messages to narrate which combos/thresholds fired and their contribution; regenerate client wire types.

## 2. Count-threshold scoring

- [ ] 2.1 Evaluate count-threshold bonuses at resolution from the public pot card count.
- [ ] 2.2 Wire the Distiller Brewer hook (treat the pot as 2 cards larger for thresholds).

## 3. Named-combo bonuses

- [ ] 3.1 Implement pair detection (both halves in the pot) and apply the bonus (points and/or volatility per definition).
- [ ] 3.2 Guarantee a lone half plays as a normal ingredient with no penalty (bonus-not-requirement).
- [ ] 3.3 Wire the Herbalist (combo fires from one half) and Alchemist (combo also adds volatility) Brewer hooks.

## 4. Effective volatility & color-synergy

- [ ] 4.1 Feed compounding-adjusted **effective** volatility into the explosion check and the fatal-wave sort (per `boom-resolution`).
- [ ] 4.2 If color-synergy content is included, enforce its cap / Peek-gate so it cannot snowball in the hidden pot.

## 5. Content & buckets

- [ ] 5.1 Author the Bramble (combo pairs) and Honey (count-threshold) ingredient content surfaced by the Apothecary buckets.
- [ ] 5.2 Keep reach-in manipulation (Double Down, Sour) in the grimoire, not on ingredients.

## 6. Clients & balance (Principle IV)

- [ ] 6.1 TUI: depile narrates fired combos/thresholds and any effective-volatility shift that changed the detonator.
- [ ] 6.2 Harness: measure color-synergy snowball, lone-combo-half frequency (dead-draw rate), and threshold payoff; tune magnitudes; record in docs/07.
