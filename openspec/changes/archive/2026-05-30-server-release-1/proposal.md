## Why

Boiling Point has a ratified game design (`knowledge/game-design.md`) and an approved server tech stack, but **no code and no specifications** — and the existing `server-architecture.md` brainstorm predates the final design, so its game-mechanics sections (sequential turns, rumble/glow clues, last-player-blamed explosions, card-count majority) now contradict the canonical rules. Release 1 establishes the **authoritative game server** as the single source of truth (Constitution Principle I) and the **headless bot harness** that lets us validate balance before any client exists (Principles II & IV). Feature-complete means a bot or agent can play a *complete* game end-to-end over WebSocket.

## What Changes

- Stand up the cargo workspace: `protocol/` (wire messages + MessagePack codec helpers only — no domain types, no server secrets) and `server/` (authoritative engine, owns the full domain incl. secrets). R1 is validated by **connection smoke tests** (handshake, heartbeat ping/pong, join/leave) plus in-process engine integration tests. The **complete bot/balance harness is a separate change** (`bot-balance-harness`); the client, agent-harness (L2), and visual tests (L3) are out of scope.
- **Rebuild the game engine against the final design.** **BREAKING** vs. the brainstorm: replace sequential per-turn play with **simultaneous hidden single-card waves**; **DELETE the rumble/glow clue system** (blind volatility — Peek is the only window); replace last-player-blamed penalties with **shared-loss explosions** (everyone loses the pot); replace card-count majority with **winner-takes-all by total color points**.
- Add three systems the brainstorm never covered: the **8-effect resolution pipeline** (fixed order, pre-wave snapshot semantics), the **cauldron-modifier escalation engine** (6 stacking modifiers), and the **reverse-order depile** reveal every round.
- Add the **full Deathmatch** tiebreaker (forced waves, volatility-only, Detonator elimination, Shield-redirect cascade).
- Ship **invite-link codes _and_ the auto-match queue** (auto-start at 4 players; no host settings).
- Keep the brainstorm's **approved infrastructure**: single-binary/logical-module topology, `mpsc`-in / per-player-out concurrency with a `DashMap` room registry, post-game persistence with anonymous session auth, `tracing` + `metrics` + Prometheus observability, server-side validation on every action with a 100 ms rate limit, and 60 s reconnection grace with full state snapshots.
- Establish a **content/engine boundary** (Constitution III): all cards, effects, attributes, and modifiers are *content* defined in a separate module and driven by a **validated config file** with per-item enable/disable toggles, so changing the deck never touches the protocol or the loop.
- Align to the updated `game-design.md` consistency pass: effects are **silent until the depile** (exceptions: Peek announces anonymously, Expose reveals publicly, Recall shows as a contribution-count drop); the **boiling point is revealed on explosion only** (hidden on a safe brew); **Reversal** picks the lowest color *present in the pot*; and the **deck reshuffles from the discard** when the draw deck empties (card counting resets per shuffle).

## Capabilities

### New Capabilities
- `wire-protocol`: MessagePack message catalog (including the wave-open timer budget so clients/bots can render a countdown), the private-vs-broadcast audience split, the `JoinRoom`/`RoomJoined` version handshake, per-connection rate limiting, and standalone `encode`/`decode` helpers that make the entire wire unit-testable in isolation. Carries no domain structs and no server-only secrets (the boiling point never appears in a protocol type).
- `lobby-and-matchmaking`: invite codes (`BREW-7K3F`), the auto-match queue assembling groups of 4, room lifecycle (idle timeout, hostless auto-start at 4), and anonymous session-token auth on join.
- `round-engine`: the wave loop — hidden simultaneous commits, synchronized reveal, round-termination rules, wave timers, blind-volatility state, per-phase information visibility, and the depile.
- `card-effects`: the 8 special effects, their fixed 7-step resolution order, and pre-wave snapshot semantics for same-wave interactions.
- `cauldron-modifiers`: the escalation engine — drawing one modifier per round (2–5), cumulative stacking, and clean composition of offsets/multipliers (contradictions cancel).
- `scoring-and-explosion`: dominance by highest total color points, winner-takes-all payout, Alliance/Commune splits (round down, integer-only), shared-loss explosions (no floor/ceiling), and Shield's bet-on-the-boom forfeit.
- `deck-and-dealing`: deck composition, deal-to-5 (a refill *floor* — never discards) with unplayed-card carryover, and reshuffle-from-discard when the draw deck empties.
- `game-content-config`: the content/engine separation, distinct types per content kind (cards vs. effects vs. modifiers — never one union), per-item enable/disable toggles, and **fail-fast startup validation** of all counts/ratios/totals.
- `deathmatch`: the tiebreaker mini state machine — forced 1 card/wave, volatility-only resolution, most-volatility Detonator elimination, and the Shield-redirect cascade.
- `reconnection`: disconnect grace (60 s), auto-pass while absent, and full state snapshots scoped to what the player may know.
- `persistence-and-observability`: post-game writes (games, players, rounds), the schema, plus `tracing` spans and Prometheus game-balance metrics.
- `table-talk`: a fixed preset-emote palette broadcast to the room — non-binding, language-neutral, no free text and no quick-phrases (the only v1 communication channel).

### Modified Capabilities
- _None_ — this is the first change; `openspec/specs/` is empty.

## Impact

- **New code:** cargo workspace (`protocol/`, `server/`); server modules `transport/`, `lobby/`, `matchmaking/`, `config/`, `content/`, `game/{room,phase,wave,resolve,scoring,deck,deathmatch}`, `persistence/`, `observability/`, preset-emote handling, and `server/tests/` connection smoke tests. The server owns the authoritative domain (incl. secrets); `protocol/` holds only wire DTOs + codec helpers. The full bot/balance harness lands in the separate `bot-balance-harness` change.
- **New dependencies:** `axum`, `tokio`, `tower`, `serde`, `rmp-serde`, `dashmap`, `sqlx` (PostgreSQL), `tracing`/`tracing-subscriber`, `metrics`/`metrics-exporter-prometheus`, plus a config format (`toml` or `ron`) and `tokio-tungstenite` for the smoke-test WebSocket client.
- **New infrastructure:** a PostgreSQL instance and a content config file checked into the repo.
- **Docs:** `server-architecture.md` should be marked as partially superseded; the specs in this change become the authoritative server contract.
- **Constitution:** establishes the server-authoritative core (I) and the bot-harness testing layer (II, IV); the content/config boundary embodies start-simple-with-a-designed-seam (III).
