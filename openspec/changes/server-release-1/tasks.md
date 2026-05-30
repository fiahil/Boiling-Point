## 1. Workspace & Scaffolding

- [x] 1.1 Create the cargo workspace with `protocol/`, `server/`, `bot-harness/` member crates
- [x] 1.2 Add core dependencies (axum, tokio, tower, serde, rmp-serde, dashmap, sqlx, tracing, tracing-subscriber, metrics, metrics-exporter-prometheus, toml, tokio-tungstenite) and pin versions
- [x] 1.3 Set up workspace-level lints requiring `missing_docs` so every module/type/public fn carries a purpose doc-comment (Constraint #5)
- [x] 1.4 Add a CI/check task (fmt, clippy, test) and a README note on running it

## 2. Wire Protocol Crate (`protocol/`)

- [x] 2.1 In `protocol/`, define ONLY the public value-types that appear on the wire (PlayerId, RoomCode, Color) — no behavior-bearing domain structs and no server-only secrets (the boiling point must not exist as a protocol type)
- [x] 2.2 Define `ClientMessage` enum (JoinRoom w/ protocol_version, CreateRoom, EnqueueMatch, CommitCard, CommitPass, LockIn, Emote, Heartbeat) — enum-tagged for JSON fallback
- [x] 2.3 Define `ServerMessage` enum with explicit audience (RoomJoined, GameStarting, YourHand[private], WaveOpened, WaveResolved, ModifierRevealed, SomeonePeeked, Exposed, DeckReshuffled, EmoteBroadcast, PeekResult[private], Depile, RoundScored, Explosion, ScoreUpdate, GameOver, Error[private], StateSnapshot[private], Heartbeat[private], Player(Dis)connected)
- [x] 2.4 Encode the private-vs-broadcast audience at the type level (separate enums or an Audience wrapper) per `wire-protocol`
- [x] 2.5 Provide standalone `encode`/`decode` helpers (MessagePack, JSON-fallback) in `protocol/` so the entire wire is testable with neither server nor bot present
- [x] 2.6 Unit-test round-trip encode/decode for every message variant, and assert no message type carries the boiling point or other secret fields

## 3. Content Module & Config (`server/content/`, `server/config/`)

- [x] 3.1 Define distinct content kinds in separate files: `Card`, `Effect`, `Modifier` — never a shared union (Constraint #1, spec `game-content-config`)
- [x] 3.2 Define the `Effect` trait (strategy) with a `resolve(&self, ctx: &mut WaveResolution)` method and stub the 8 effect strategy objects
- [x] 3.3 Define the `Modifier` trait exposing pure offsets/multipliers (boiling_point_delta, start_volatility, pot_point_delta_per_card, pot_multiplier, reverses_dominance) and the 6 modifier objects
- [x] 3.4 Add `enabled: bool` to every content definition (Constraint #2)
- [x] 3.5 Implement the `ContentRegistry` (registry pattern): build id→definition maps from config, filtering out disabled items
- [x] 3.6 Define the TOML config schema (deck composition, per-effect copies, modifier pool weights, deck size, ratio bounds, wave timers, boiling-point range) and a checked-in default config
- [x] 3.7 Implement `config::validate()` fail-fast checks: counts sum to deck size, ratio bounds, required effects present, modifier pool ≥ 4, deck large enough for the initial deal (4×5) with margin (reshuffle covers later exhaustion); abort startup with a specific error on any failure (Constraint #3)
- [x] 3.8 Unit-test validation: a malformed/undersized config aborts; a valid config builds the registry

## 4. Transport Layer (`server/transport/`)

- [ ] 4.1 Implement the Axum WebSocket upgrade and per-connection read/write tasks
- [ ] 4.2 Implement the MessagePack codec with JSON-fallback debug mode
- [ ] 4.3 Implement audience routing: deliver Private messages to one connection, Broadcast to all in a room; assert no secret payloads on Broadcast
- [ ] 4.4 Implement the protocol-version handshake (accept compatible, reject incompatible before any state shared)
- [ ] 4.5 Implement per-connection rate limiting (1 action / 100 ms, silently drop excess)

## 5. Lobby, Matchmaking & Session Auth (`server/lobby/`, `server/matchmaking/`)

- [ ] 5.1 Implement anonymous session auth: issue UUID + session token on join; re-resolve token to the same identity on reconnect
- [ ] 5.2 Implement the `DashMap<RoomCode, RoomHandle>` room registry and invite-code generation (BREW-7K3F) + UUID internal key
- [ ] 5.3 Implement create/join-by-code with unknown-code rejection
- [ ] 5.4 Implement the auto-match queue assembling groups of exactly 4 and spawning a room
- [ ] 5.5 Implement hostless auto-start at 4 (no host role, no settings, no manual start) and the 5-minute idle-room cleanup

## 6. Game Core: Domain Model, Phase Machine, Deck & Dealing (`server/game/`)

- [ ] 6.1 Define the server's authoritative domain model (Card, Deck, discard pile, Cauldron incl. the hidden boiling_point, Pot, Hand, GameState) — owned by the server crate, never placed in `protocol/`
- [ ] 6.2 Implement the room task: sole state owner, bounded per-player `mpsc` out + `mpsc` in, timer plumbing via `tokio::time`
- [ ] 6.3 Implement the typestate phase machine (Idle→Dealing→Playing→Depile→Scoring→next/GameOver/Deathmatch) so illegal transitions are compile errors (D-A4)
- [x] 6.4 Implement the `DeckBuilder` (builder pattern) assembling the shared deck from the registry; mixed colors + wild + enabled effects
- [x] 6.5 Implement deal-to-5 as a refill floor with carryover (D-R4): top up each hand to 5, never discard, no hand cap
- [x] 6.6 Implement reshuffle-from-discard when a refill empties the draw deck (D-R5), announced to all players so card counting resets per shuffle

## 7. Wave Loop (`server/game/wave.rs`)

- [ ] 7.1 Implement hidden simultaneous commits: each active player commits 0/1 card, hidden until reveal; latest selection wins; changeable until close
- [ ] 7.2 Implement the shared wave timer (30s wave 1 / 10s subsequent) with unanimous lock-in early close (D-R1), and emit the wave's timer budget in the `WaveOpened` broadcast so clients/bots can render a countdown (server stays authoritative on close)
- [ ] 7.3 Implement synchronized reveal: cards enter face-down at once; broadcast who played / who passed / new count, never identities
- [ ] 7.4 Implement pass = permanent lockout, with timer-expiry and empty-hand treated identically
- [ ] 7.5 Implement round termination: explosion / all-remaining-passed / one-player-final-wave, no wave cap
- [ ] 7.6 Enforce blind volatility — never emit any running-volatility or proximity cue
- [ ] 7.7 Unit-test each termination path and the early-close path

## 8. Effect Resolution (`server/game/resolve.rs`)

- [x] 8.1 Implement the pre-wave pot snapshot and the fixed 7-step resolver pipeline (cards → volatility mods → color/identity → points → removal → information → one explosion check)
- [x] 8.2 Implement the 8 effect behaviors against the resolver: Peek (private), Dampen, Volatile Surge, Expose, Copycat, Recall, Double Down, Shield
- [x] 8.3 Implement same-wave same-kind summing against the snapshot (two same-color Double Downs → ×3, order-independent) (D-R3)
- [x] 8.4 Implement Peek privacy (peeker gets the value; others see an anonymous "someone peeked")
- [x] 8.5 Implement Shield round-scope state (immunity + safe-resolution scoring forfeit) and thread it to scoring
- [ ] 8.6 Implement silent-by-default effect visibility: emit no play notification except SomeonePeeked (anonymous), Exposed (public card reveal), and the Recall-driven contribution-count drop; Dampen/Volatile Surge/Copycat/Double Down stay fully silent until the depile
- [ ] 8.7 Unit-test resolution ordering, snapshot semantics, the Double Down stacking edge case, and that silent effects emit no leak

## 9. Cauldron Modifiers (`server/game/` + `content/modifier.rs`)

- [ ] 9.1 Implement round-1-clean + draw-one-per-round (2–5) from the weighted pool, revealed at round start
- [ ] 9.2 Implement cumulative stacking (round N has N−1 active) and public visibility
- [x] 9.3 Implement clean composition of all offsets/multipliers; verify Thin Ice+Deep Cauldron cancel and Reversal×2 reverts
- [x] 9.4 Wire the six modifier effects into boiling-point / starting-volatility / pot-value / dominance computation, with Bountiful Brew inflating pot total only and colorless across all cards (D-R2), and Reversal selecting the lowest color *present in the pot*
- [x] 9.5 Unit-test composition, cancellation, Reversal parity plus its edge cases (single color = no-op, tie-for-lowest splits, never an absent color), and Double Stakes scaling both directions

## 10. Scoring & Explosion (`server/game/scoring.rs`)

- [x] 10.1 Implement dominance by highest total color points (wild counts to pot total, not to any color)
- [x] 10.2 Implement the scoring sequence: decide the winner on per-color totals first (Reversal picks the lowest present color, Bountiful excluded), then pot value = sum of card points + Bountiful (additive) × Double Stakes (multiplier) → award/deduct
- [x] 10.3 Implement winner-takes-all and Alliance/Commune splits (round down, integer-only, leftover evaporates)
- [x] 10.4 Implement shared-loss explosion (every player incl. spectators loses pot value; no floor/ceiling) honoring Shield immunity
- [x] 10.5 Implement absent-player +0 and Shield safe-resolution forfeit
- [x] 10.6 Unit-test domination, two/three-way splits with rounding, shared loss, Shield outcomes, and Reversal scoring

## 11. Depile & Information Visibility (`server/game/`)

- [ ] 11.1 Implement the reverse-order depile every round (full attributes + contributing player per card)
- [ ] 11.2 Mark the boiling-point crossing card on explosion depiles
- [ ] 11.3 Reveal the exact boiling point on explosion depiles only; keep it hidden on a safe brew
- [ ] 11.4 Implement and unit-test per-phase information visibility (own hand private; counts/scores/modifiers public; cauldron identities hidden until depile; boiling point hidden except via Peek and the explosion depile)

## 12. Deathmatch (`server/game/deathmatch.rs`)

- [ ] 12.1 Implement trigger + setup (tied players only, frozen main scores, fresh BP 8–14, no modifiers, empty-hand-at-start eliminated last)
- [ ] 12.2 Implement forced 1-card/wave commits (no passing), volatility-only
- [ ] 12.3 Implement Detonator elimination (most volatility out; tie-for-most all out; 1 survivor champion; 2+ fresh Deathmatch; all-out → co-champions)
- [ ] 12.4 Implement the Shield redirect cascade (redirect to next-highest; all-shielded → no casualty, fresh Deathmatch)
- [ ] 12.5 Implement no-explosion co-champions and unit-test all Deathmatch outcomes

## 13. Reconnection (`server/`)

- [ ] 13.1 Implement 60s disconnect grace with auto-pass while absent (identical to timer-expiry lockout)
- [ ] 13.2 Implement the StateSnapshot on rejoin, scoped strictly to player-permitted info
- [ ] 13.3 Implement abandonment after grace (auto-pass future waves, keep tracking score) and all-disconnected room cleanup
- [ ] 13.4 Unit-test snapshot scoping (no hidden data leaks) and the abandon-then-continue path

## 14. Persistence (`server/persistence/`)

- [ ] 14.1 Write the PostgreSQL schema migration (players, games, game_players, game_rounds)
- [ ] 14.2 Implement the single post-game write at `GameOver` (game + per-player results + optional per-round detail); no mid-game writes
- [ ] 14.3 Implement anonymous player-record creation from session UUID
- [ ] 14.4 Integration-test the completion write and per-player result retrieval

## 15. Observability (`server/observability/`)

- [ ] 15.1 Set up `tracing` (JSON) spans for phase transitions, message handling, room lifecycle, DB writes
- [ ] 15.2 Set up `metrics` + Prometheus exporter and emit balance metrics (active rooms, durations, explosion rate, timeout rate, cards/round, reconnection rate)

## 16. Table Talk — Preset Emotes (`server/`)

- [ ] 16.1 Define the fixed preset-emote palette in config (id → emote) and validate it at startup; no free text, no quick-phrases
- [ ] 16.2 Handle inbound `Emote`: accept only palette ids, reject others with an `Error`, and broadcast `EmoteBroadcast` (sender + id) to the room in any phase
- [ ] 16.3 Ensure emotes change no game state and are subject to the 100 ms rate limit
- [ ] 16.4 Unit-test palette validation, non-binding behavior (no state change), and rate-limited spam

## 17. Connection Smoke Tests (`server/tests/`)

- [ ] 17.1 Smoke test: a WebSocket client connects and completes the protocol-version handshake; an incompatible version is rejected
- [ ] 17.2 Smoke test: heartbeat keepalive holds a connection live; a missing heartbeat routes into disconnect handling
- [ ] 17.3 Smoke test: create/join a room by code and leave; graceful and abrupt disconnect are handled
- [ ] 17.4 Assert the smoke client receives only player-permitted messages (no secret fields on the wire)

## 18. End-to-End Integration & Doc Sync

- [ ] 18.1 Engine-level integration test: drive a complete 5-round game in-process (scripted commits) to `GameOver` and assert the persistence write
- [ ] 18.2 Engine-level integration test: a forced score tie routes into a full Deathmatch and produces a champion (include a Shield-redirect case)
- [ ] 18.3 Property/integration test: many in-process games run without illegal-state panics and exercise a reshuffle at least once
- [ ] 18.4 Annotate `server-architecture.md` as partially superseded, pointing to this change's specs as authoritative
