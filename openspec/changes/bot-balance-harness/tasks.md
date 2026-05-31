## 1. Crate Setup

- [x] 1.1 Add the `bot-harness/` crate to the workspace, depending on the `protocol/` crate (and a seeded `rand`)
- [x] 1.2 Define a transport abstraction with two backends: in-process server handle (default) and WebSocket (`tokio-tungstenite`)

## 2. Bot Core & Domain Model

- [x] 2.1 Define the bot's player-visible domain model, built only from received `ServerMessage`s — no boiling point, no opponents' hands, no draw deck (D2)
- [x] 2.2 Implement the message loop: decode `ServerMessage`, update the model, ask the strategy for an action, encode the `ClientMessage`
- [x] 2.3 Implement complete-game play (join → deal → 5 rounds of waves → any Deathmatch → `GameOver`) emitting only valid actions
- [x] 2.4 Assert at runtime/test that the bot never receives a secret field (secret-boundary contract test)

## 3. Strategies

- [x] 3.1 Define the `Strategy` trait covering commit/pass, card choice, effect targeting, and emotes — a pure function of the bot's model + its seeded RNG (D3)
- [x] 3.2 Implement baseline strategies: cautious, aggressor, diplomat, random
- [x] 3.3 Support per-seat strategy assignment for a game

## 4. Determinism

- [x] 4.1 Implement a single root-seed RNG tree deriving per-game and per-bot streams (D4)
- [x] 4.2 Forbid non-seeded sources on the decision path; add a same-seed-same-result reproducibility test

## 5. Batch Runner

- [x] 5.1 Implement a CLI batch runner: N games, strategy assignment, content-config path, root seed
- [x] 5.2 Run thousands of complete games headlessly via the in-process backend; support a smaller WS-mode batch
- [x] 5.3 Collect per-game results (outcome, scores, pot values, explosions, waves, modifiers drawn)

## 6. Statistics & Reporting

- [x] 6.1 Aggregate batch statistics: explosion rate, win distribution by color and strategy, avg pot value, avg cards/round, avg waves/round, modifier-draw frequency
- [x] 6.2 Emit a structured report (human summary + machine-readable file) keyed to the content-config version under test
- [x] 6.3 Implement balance-smell detection against configurable thresholds: dominant strategy/color, off-target explosion rate, runaway pots — flag with supporting numbers

## 7. Validation

- [x] 7.1 Test: a seeded batch reproduces identical outcomes and statistics on re-run
- [x] 7.2 Test: an intentionally skewed strategy is flagged as a degenerate-strategy candidate
- [x] 7.3 Run a real batch against the default content config and record the baseline balance report
