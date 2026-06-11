## 1. Protocol & card model

- [ ] 1.1 Define ingredient vs spell card types in the `protocol` crate (color, volatility 0–7, points 0–3; spells carry no points/volatility), with serde derives.
- [ ] 1.2 Define the wire messages for: wave commit (ingredient-or-pass + optional spell), reveal, explosion result (detonator set + per-player −P), safe-brew scoring, and the volatility-sorted depile (incl. revealed boiling point).
- [ ] 1.3 Regenerate client wire types from the crate; ensure no hand-written drift.

## 2. Decks & dealing (fixed, color-anchored)

- [ ] 2.1 Build the fixed color-anchored pantry (~75% own color + toolkit) and fixed grimoire as content config.
- [ ] 2.2 Implement ingredient top-up-to-3 each wave (refill-only) and per-player draw from own pantry.
- [ ] 2.3 Implement spell draw at round start (fixed count), no in-round replenish (except Forage), carryover between rounds.

## 3. Wave loop

- [ ] 3.1 Implement the secret wave commit: ingredient-or-pass + optional ≤1 spell; reveal simultaneously.
- [ ] 3.2 Enforce pass = locked out (incl. timer-expiry auto-pass); a spell never keeps a passed player in.
- [ ] 3.3 Implement round termination (explosion · all pass · one-player-final-wave).

## 4. Resolution — explosion & scoring

- [ ] 4.1 Compute pot value P = Σ colored Vote points; colorless/wild plays score 0.
- [ ] 4.2 Safe-brew dominance: winner-takes-all, Alliance/Commune split round-down, integer-only.
- [ ] 4.3 Explosion check on the running total (incl. cauldron-level spell deltas).
- [ ] 4.4 Detonator identification: fatal-wave ascending **effective**-volatility sort → trigger + heavier split −P; equal-volatility simultaneous; non-fatal-wave players exempt.
- [ ] 4.5 Apply wards at resolution (Cap / Halve / Redirect with cascade) and Hex (+extra on any explosion).

## 5. The 15-spell grimoire

- [ ] 5.1 Implement Instant spells (Peek, Expose, Assay, Dampen, Surge, Quench, Double Down, Sour, Skim, Forage).
- [ ] 5.2 Implement Active spells (Cap, Halve, Redirect, Harvest, Hex): prime face-down, fire on trigger, consume.
- [ ] 5.3 Enforce visibility: hidden in hand / while primed; visible on activation (Instant on cast, Active on fire); reveal deltas not absolute totals.
- [ ] 5.4 Server validation: ≤1 spell/wave; reject illegal targets/timing.

## 6. The depile

- [ ] 6.1 Volatility-ascending reveal every round; reveal the boiling point every round (boom and safe).
- [ ] 6.2 On boom, mark the crossing point and the liable cards; narrate fired wards/Hex/Harvest.

## 7. Clients & harnesses

- [ ] 7.1 Web client (`clients/web/`): render the new wave actions (ingredient/spell/pass), the grimoire, and the volatility-sorted, boiling-point-revealing depile.
- [ ] 7.2 Bot harness (revived from `archive/bot-harness/`, §IV): drive the new loop headlessly; emit explosion-rate, detonator-distribution, Peek-fire-rate, and freeze (all-pass) statistics.
- [ ] 7.3 Claude-as-player harness (optional revival from `archive/agent-harness/`): expose the new intents over the structured JSON interface.

## 8. Balance (Principle IV)

- [ ] 8.1 Sweep the boiling-point range (~20–32) against the ~45% explosion-rate target; confirm rounds don't freeze (Vulture check).
- [ ] 8.2 Tune volatility/points curves and the fixed-deck composition; re-derive the blind-volatility / Peek economy.
- [ ] 8.3 Record validated numbers back into docs/07 and promote stable rules into docs/02.
