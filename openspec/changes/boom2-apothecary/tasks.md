## 1. Protocol & recipe

- [x] 1.1 Define the 12 pantry + 8 grimoire bucket identities and the per-player **recipe** (bucket set + reserve) in the `protocol` crate; recipe is public, realized deck is not. *(`vocab::PantryBucket`/`GrimoireBucket`/`Recipe`; grimoire role-groups are static metadata. Recipe rides public `RecipesRevealed`/`StateSnapshot`; the realized deck never crosses the wire.)*
- [x] 1.2 Define the draft messages (offer buckets, submit recipe, set reserve); regenerate client wire types. *(`PendingDecision::ApothecaryDraft` carries the rosters + allowances + a suggested quick-pick; the intent is `ClientMessage::SubmitRecipe`; protocol v7. Client codegen is a no-op today — it lands with `adopt-pixi-client`, picking up the v7 vocabulary.)*

## 2. The realizer

- [x] 2.1 Map each bucket to its eligible card family (pantry flavors, grimoire role-groups). *(Pantry families are file-driven `[[pantry_bucket]]` in `content.toml`, colour slot derived from the bucket; grimoire role-groups are `GrimoireBucket::spells()` protocol metadata.)*
- [x] 2.2 Implement the realizer: from a recipe's eligible pool, compose a fixed-size deck (pantry 30 / grimoire 20), re-rolled each game. *(`game::realizer`, seeded per deck off the per-seat seed stream.)*
- [x] 2.3 Enforce caps in the realizer: color-anchor ~75%, toolkit ≤25%, Treasure ≤3 (absolute), god-tier ≤2 (absolute). *(`ApothecaryConfig` caps; a capped-out slot falls through to commons — Sage/Mint own-colour for the pantry, non-god role-groups for the grimoire — so any pick-set yields a legal deck and premium stays absolute. Swept across pick-sets/seeds in `realizer::tests`.)*
- [x] 2.4 Implement the reserve: lock one named grimoire spell; roll the remainder; pantry stays pure-roll. *(Reserves placed first in `realize_grimoire` — guaranteed unless excluded/capped; the Reservist locks two. Pantry has no reserve.)*
- [x] 2.5 Keep the realized deck server-side and hidden from the owner (deal/learn-as-you-draw). *(Realized cards live in the engine's `Pantry`/`Grimoire`; only the public recipe is broadcast; the owner learns the deck via `YourHand` as they draw — the existing learn-as-you-draw path, unchanged.)*

## 3. Pre-game draft step

- [x] 3.1 Run the two-ledger draft **after** the Brewer pick; publish each player's recipe to the table. *(`session::run_draft_phase` runs after `run_brewer_phase`, before `Game::with_recipes`; `RecipesRevealed` broadcasts before the first deal. `draft` span landed additively, no schema bump.)*
- [x] 3.2 Provide sane defaults / quick-pick so the draft stays inside the lobby time budget. *(`session::suggested_recipe` — a seeded 3+3 quick-pick rides every frame as `suggested` (the one-tap default) and is applied to stragglers at `timing.draft_ms` (30s); the phase closes early once all four recipes land.)*
- [x] 3.3 Wire the Brewer hooks: Connoisseur (4th bucket allowed), Reservist (2 reserves). *(`brewers::extra_buckets`/`reserve_allowance` feed the frame's `bonus_buckets`/`reserves_max`; `excluded_buckets` keeps Ironbark off the Cinderwright's grimoire roster — the no-Ward rule extended to the draft. The seams the brewers change defined are now consulted and tested.)*

## 4. Replace fixed decks

- [x] 4.1 Switch the deck source from `boom-cards`' fixed decks to the realizer output. *(`Game::with_recipes` realizes each reciped player's decks; the shipping `session::run_game` always supplies recipes. Replay format v5 / engine v4 carry the recipes as a reconstruction input — guarded by `replay_with_recipes_round_trips`.)*
- [x] 4.2 Keep fixed decks available as a harness/teaching fallback only. *(A recipeless player falls back to the fixed colour-anchored deal, so the brewerless/recipeless `Game::new` still deals the classic decks — sync tests, teaching games, and pre-draft replays all keep working.)*

## 5. Clients & harness

- [ ] 5.1 Web client (`clients/web/`): draft UI (pick 2–3 buckets per ledger, set reserve) and a public-recipe readout at the table. *(Blocked: `clients/web/` does not exist yet — it lands with `adopt-pixi-client`. The protocol carries everything the renderer needs: the `ApothecaryDraft` frame's rosters/allowances/blurbs and the public `RecipesRevealed`.)*
- [x] 5.2 AI client harness (`clients/ai` harness mode, §IV — supersedes the interim `bot-harness/`): land the draft decision kind, lift the sample-spec `deck_archetype` axis rejection (the scripted-draft policy is already in place), and add the **deck-archetype** axis to the matrix. *(`Answer::ApothecaryDraft` + `bot::decks` (four presets, frame-legalized like the Brewer preference); the spec's `deck_archetype` axis is live and validated; `balance_tester` emits the persona × deck-archetype matrix with an `archetype_outlier` smell over the same per-seat band.)*

## 6. Balance (Principle IV)

- [x] 6.1 Sweep realizer caps (god-tier ≤2, Treasure ≤3, toolkit ≤25%) across persona × Brewer × archetype. *(20,000 games: a 4-cell same-persona mirror sweep (`specs/deck-archetype-sweep.toml`) + a 4-cell archetype-vs-mixed-field sweep (`specs/deck-archetype-vs-field.toml`). Caps held in every realized deck across both; results in docs/06_boom2/02 §"Apothecary as built".)*
- [x] 6.2 Confirm no degenerate archetype dominates; confirm the Peek economy holds under player-shaped supply; record in docs/07. *(The Peek economy structurally holds — the absolute god-tier cap (≤2/grimoire) bounds even an all-Loyalist (4× guaranteed-Peek) table, the designed info-vs-survival tradeoff. The archetype CONTENTS are not yet mutually balanced (Warlord strong, Kingmaker weak) — recorded as the watch item, the first content-tuning target, with the mirror-freeze and bot-blind-estimate caveats; the realizer mechanism + caps are the validated deliverable. Full write-up + rerun commands in docs/06_boom2/02 §"Apothecary as built".)*
