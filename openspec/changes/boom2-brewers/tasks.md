## 1. Protocol & identity

- [x] 1.1 Define the Brewer identity enum (12) and its public per-player state in the `protocol` crate.
- [x] 1.2 Define the pre-game messages: the dealt 2-of-pair offer and the player's pick intent. *(The offer rides the existing decision-frame machinery as `PendingDecision::BrewerPick`; the intent is `ClientMessage::PickBrewer`; the public table arrives in `BrewersRevealed`. Protocol v6.)*
- [x] 1.3 Regenerate client wire types. *(No-op today: no generated client types exist yet — the TypeScript codegen lands with `adopt-pixi-client`, whose generator will pick the v6 vocabulary up from the `protocol` crate.)*

## 2. Pre-game brewer phase

- [x] 2.1 Deal **disjoint pairs** (8 of 12) so any picks are unique; collect simultaneous picks.
- [x] 2.2 Order the phase **before** the deck step; publish all four chosen Brewers to the table. *(The phase runs before `Game::with_brewers`, whose deck building consults the picks; `BrewersRevealed` broadcasts before the first deal.)*
- [x] 2.3 Handle disconnect/timeout (auto-pick or default) within the lobby ethos. *(Deterministic first-of-pair auto-pick at `timing.brewer_pick_ms`; the phase closes early once all four picks land.)*

## 3. Brewer rule hooks (one per system)

- [x] 3.1 Detonator-sort hooks: Featherhand (lowest-in-ties), Cinderwright (half damage, no Ward), Alchemist (combo adds volatility). *(Alchemist's hook is an inert seam until `boom2-compounding` lands combos.)*
- [x] 3.2 Spell/economy hooks: Channeler (2 spells/wave), Reservist (2 reserves), Eavesdropper (piggyback Peek), Forager (top up to 4).
- [x] 3.3 Draft hooks (inert until `boom2-apothecary`): Connoisseur (4th bucket), Reservist (reserves). *(Constants defined and tested in `game::brewers`; nothing consults them yet — the documented phasing gap.)*
- [x] 3.4 Compounding hooks (full effect with `boom2-compounding`): Herbalist (combo from one half), Distiller (+2 pot size for thresholds). *(Inert seams, same as 3.3; `inert_brewers_change_nothing_yet` asserts bit-identical games.)*
- [x] 3.5 Scoring/commitment hooks: Broker (round up on split), Lurker (commit after reveal, once/round). *(The Lurker's defer is a staged wave: interim reveal, then a card-only post-reveal commit into the same wave — one explosion check, full liability; replays re-run it as simultaneous.)*
- [x] 3.6 Enforce the discipline guardrails in code (≤half damage; conditional info only). *(Cinderwright takes `ceil(share/2)` — never immunity; the Eavesdropper learns nothing on a Peek-free wave; both are tested invariants.)*

## 4. Clients & harness

- [ ] 4.1 Web client (`clients/web/`): render the 2-of-pair pick and the four public Brewers at the table. *(Blocked: `clients/web/` does not exist yet — it lands with `adopt-pixi-client`. The protocol carries everything the renderer needs, including each Brewer's one-sentence `bent_rule()`.)*
- [x] 4.2 AI client harness (`clients/ai` harness mode, §IV — supersedes the interim `bot-harness/`): land the Brewer-pick decision kind in `PendingDecision`, lift the sample-spec `brewer` axis rejection, and emit the **persona × Brewer** win/break matrix from `balance_tester`. *(The spec `brewer` axis is a pick preference; the matrix keys on actual picks, with a `brewer_outlier` smell over the [15%, 35%] per-seat win-rate band.)*

## 5. Balance (Principle IV)

- [x] 5.1 Run persona × Brewer over thousands of games; confirm no Brewer breaks a persona or a combat-core invariant. *(12,000 games: 3 × 2,000 baseline at seeds 42/7/1234 + 4 × 1,500 🌶️-concentration cells. All 12 within [22.0%, 33.5%] per-seat win rate; explosion rate 45.5–47.5% in every cell; no `brewer_outlier` smell.)*
- [x] 5.2 Tune the 🌶️ Brewers (Cinderwright, Channeler, Alchemist, Lurker) to spec discipline; record results in docs/07. *(Data says ship untuned — all four inside the band. Cinderwright (~32%, consistent top) recorded as the watch item; results + instrument caveats in docs/06_boom2/02 §"Brewers as built".)*
