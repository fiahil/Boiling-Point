## 1. Protocol & identity

- [ ] 1.1 Define the Brewer identity enum (12) and its public per-player state in the `protocol` crate.
- [ ] 1.2 Define the pre-game messages: the dealt 2-of-pair offer and the player's pick intent.
- [ ] 1.3 Regenerate client wire types.

## 2. Pre-game brewer phase

- [ ] 2.1 Deal **disjoint pairs** (8 of 12) so any picks are unique; collect simultaneous picks.
- [ ] 2.2 Order the phase **before** the deck step; publish all four chosen Brewers to the table.
- [ ] 2.3 Handle disconnect/timeout (auto-pick or default) within the lobby ethos.

## 3. Brewer rule hooks (one per system)

- [ ] 3.1 Detonator-sort hooks: Featherhand (lowest-in-ties), Cinderwright (half damage, no Ward), Alchemist (combo adds volatility).
- [ ] 3.2 Spell/economy hooks: Channeler (2 spells/wave), Reservist (2 reserves), Eavesdropper (piggyback Peek), Forager (top up to 4).
- [ ] 3.3 Draft hooks (inert until `boom2-apothecary`): Connoisseur (4th bucket), Reservist (reserves).
- [ ] 3.4 Compounding hooks (full effect with `boom2-compounding`): Herbalist (combo from one half), Distiller (+2 pot size for thresholds).
- [ ] 3.5 Scoring/commitment hooks: Broker (round up on split), Lurker (commit after reveal, once/round).
- [ ] 3.6 Enforce the discipline guardrails in code (≤half damage; conditional info only).

## 4. Clients & harness

- [ ] 4.1 Web client (`clients/web/`): render the 2-of-pair pick and the four public Brewers at the table.
- [ ] 4.2 AI client harness (`clients/ai` harness mode, §IV — supersedes the interim `bot-harness/`): land the Brewer-pick decision kind in `PendingDecision`, lift the sample-spec `brewer` axis rejection, and emit the **persona × Brewer** win/break matrix from `bp-harness`.

## 5. Balance (Principle IV)

- [ ] 5.1 Run persona × Brewer over thousands of games; confirm no Brewer breaks a persona or a combat-core invariant.
- [ ] 5.2 Tune the 🌶️ Brewers (Cinderwright, Channeler, Alchemist, Lurker) to spec discipline; record results in docs/07.
