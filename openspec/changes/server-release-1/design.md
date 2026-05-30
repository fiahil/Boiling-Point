## Context

Boiling Point has a ratified game design (`knowledge/game-design.md`, canonical) and an approved server tech stack (Rust + Axum/Tokio, PostgreSQL, MessagePack/WebSocket). The earlier `server-architecture.md` brainstorm is half-stale: its infrastructure decisions (topology, concurrency, persistence, observability, anti-cheat, scaling) are approved and survive, but its game-mechanics sections describe the abandoned v1 (sequential turns, rumble/glow clues, last-player penalties, card-count majority) and are superseded by the final design's **simultaneous hidden waves, blind volatility, shared-loss explosions, points-based winner-takes-all, stacking modifiers, and Deathmatch**.

This change is the first code in the repo. It must stand up the workspace and deliver a feature-complete authoritative server plus the headless bot harness. The dominant design tension is **stability vs. churn**: the *rules engine and wire protocol* must be stable, while *content* (cards, effects, modifiers, counts, ratios) will churn constantly during balance playtesting. The architecture is organized around that seam.

## Goals / Non-Goals

**Goals:**
- A bot or agent can play a complete 4-player game end-to-end over WebSocket: lobby/queue → 5 rounds of waves → Deathmatch → `GameOver` → persistence.
- Faithful implementation of the final design's rules, including all 8 effects, 6 stacking modifiers, the depile, and full Deathmatch.
- A hard **content/engine boundary**: changing or toggling a card/effect/modifier, or retuning counts/ratios, never touches the protocol or the loop and is driven by a validated config file.
- A minimal table-talk channel (preset emotes — the design's only v1 comms) and connection smoke tests (handshake, heartbeat, join/leave) that prove the wire works end-to-end.
- In-process engine integration tests that drive complete games (including a Deathmatch) to validate R1 without needing the full bot harness (which is a separate change).
- Server-authoritative correctness: every action validated, no hidden information ever leaked (Constitution I).
- Readable code: well-known patterns where they help, and a purpose doc-comment on every module, type, and public function.

**Non-Goals:**
- No client (rendering is a separate effort).
- No full bot/balance harness in R1 — the headless game-playing bots, pluggable strategies, batch runner, and balance statistics are the separate `bot-balance-harness` change. R1 ships only connection smoke tests.
- No agent harness (L2) or visual tests (L3) — separate future specs.
- No OAuth, no FFA rating system (Elo/TrueSkill), no spectators, no chat.
- No crash recovery / checkpointing / event sourcing — persistence is post-game only.
- No horizontal scaling — single binary, logically modular.

## Decisions

### D-A1. Cargo workspace, the `protocol/` waist, and per-crate domain models

Three crates, mirroring the constitution's structure (client/agent-harness deferred):

```
boiling-point/                 (cargo workspace)
├── protocol/      ONLY the wire: ClientMessage/ServerMessage DTOs + msgpack
│                  encode/decode helpers. No game logic, no server secrets.
└── server/        authoritative engine + transport + persistence
                   owns the FULL domain model (incl. secrets: boiling point, deck)
```

*(The follow-on `bot-balance-harness` change adds a `bot-harness/` crate that speaks the same protocol and owns its OWN narrow, player-visible domain model — see the domain-model note below.)*

**The `protocol/` crate is a narrow waist, not a shared domain.** It contains only the
messages that cross the wire plus `encode`/`decode` helpers, so the *entire wire* is
unit-testable in isolation (round-trip every variant through MessagePack with no server
or bot present). It deliberately carries **no game-domain structs and no server-only
secrets** — most importantly, the **boiling point never appears in a protocol type**, so
it is structurally impossible to serialize it onto the wire by accident.

**Each side owns its own domain model — no shared domain crate.** The server's domain is
the authoritative truth (full cards, the deck, the cauldron *including its hidden boiling
point*, every hand). The bot's domain (added by the follow-on `bot-balance-harness` change)
is a separate, deliberately *narrower* model derived only from received messages — it has no
field for the boiling point, no other players'
hands, no draw deck. Leakage is prevented by construction: the bot literally cannot
represent a secret it is never sent. Public value-types that genuinely appear on the wire
(e.g. `Color`, `PlayerId`) live in `protocol/` and are reused by both sides; the rich,
behavior-bearing domain types are duplicated intentionally rather than shared, trading a
little duplication for an airtight secret boundary (Constitution I).

`server/` modules, drawing the content/engine line explicitly:

```
server/src/
├── main.rs               # bootstrap: load+validate config, build router, serve
├── config/               # config schema + fail-fast validation (startup)
├── content/              # ★ CONTENT — churns freely, never referenced by the loop directly
│   ├── card.rs           #   regular (color/points/volatility) card definitions
│   ├── effect.rs         #   the 8 effect behaviors (strategy objects)
│   ├── modifier.rs       #   the 6 cauldron-modifier behaviors
│   └── registry.rs       #   registry: id -> definition lookup, built from config
├── transport/            # ws upgrade, MessagePack codec, audience routing, rate limit
├── lobby/                # invite codes, room registry (DashMap), session auth
├── matchmaking/          # auto-match queue -> assemble 4 -> spawn room
├── game/                 # ★ ENGINE — stable, operates on abstractions only
│   ├── room.rs           #   room task; owns state; phase orchestration
│   ├── phase.rs          #   typestate phase machine (Idle..GameOver)
│   ├── wave.rs           #   simultaneous-commit wave loop + termination
│   ├── resolve.rs        #   fixed-order effect resolver + pre-wave snapshot
│   ├── scoring.rs        #   dominance, winner-takes-all, shared-loss, splits
│   ├── deck.rs           #   deck builder, deal-to-5, carryover
│   └── deathmatch.rs     #   tiebreaker mini state machine
├── persistence/          # sqlx (PostgreSQL): post-game writes, schema, anon auth
└── observability/        # tracing + metrics (Prometheus) setup
```

**Why:** the `content/` and `game/` split is the load-bearing boundary. `game/` depends on `content/` only through behavioral abstractions (traits + small enums) and the registry; it never names "Peek" or "Thin Ice". Alternatives rejected: a single `cards.rs` mixing all kinds (violates the "don't mix card types" constraint and couples the loop to content); putting content or domain types in `protocol/` (would let the wire drift with content churn and risk leaking secrets — the protocol crate must stay a thin, stable, secret-free waist).

### D-A2. Content/engine boundary via Strategy + Registry, with distinct kinds

- **Distinct kinds, never a union.** `Card`, `Effect`, and `Modifier` are separate types in separate files. A hand holds `Card`s; the modifier pool holds `Modifier`s; the two never mix in one collection. (Constraint #1, #2.)
- **Strategy pattern for behavior.** Effects implement an `Effect` trait (e.g. `fn resolve(&self, ctx: &mut WaveResolution)`); modifiers implement a `Modifier` trait exposing pure offsets/multipliers (`boiling_point_delta`, `start_volatility`, `pot_point_delta_per_card`, `pot_multiplier`, `reverses_dominance`). The loop calls trait methods — it never matches on a concrete effect/modifier. Adding a new effect = a new strategy object + a config line; the loop is untouched. (Constraint #1, #4.)
- **Registry pattern for lookup.** A `ContentRegistry`, built once at startup from validated config, maps stable content IDs → definitions and is the only thing the deck builder and resolver consult. (Constraint #4.)
- **Enabled flag.** Every content definition carries `enabled: bool`; disabled items are filtered out at registry-build time so they can never be dealt, drawn, or resolved. (Constraint #2.)

**Why:** Strategy keeps the open/closed boundary clean and readable; Registry centralizes the id→definition seam so the rest of the engine stays content-agnostic. Alternative rejected: `match card.kind { ... }` scattered through the loop — fast to write, but every content change ripples into engine code, exactly what the constraint forbids.

### D-A3. Config format, builder, and fail-fast validation

- **Format: TOML** for the content/balance config (human- and agent-editable, comment-friendly, ubiquitous in Rust via `serde`). RON was considered; TOML wins on familiarity and tooling.
- **Builder pattern** assembles the deck and modifier pool from config (`DeckBuilder` → `Deck`), keeping construction logic in one readable place and the resulting structures immutable.
- **Validation at startup, fail-fast.** `config::validate()` runs before the server binds a port and aborts with a specific error on any violation: counts sum to the declared deck size; color/wild/effect ratios within configured bounds; every rules-required effect present (enabled or explicitly disabled); modifier pool ≥ rounds that draw (4); and **deck large enough to last a full 4-player game under deal-to-5+carryover with no reshuffle** (D-R5). (Constraint #3.)

**Why:** balance is the project's main job (Constitution IV) and content will be edited constantly, often by an agent; a config typo must fail loudly at boot, not corrupt a game at 3am.

### D-A4. Phase machine as typestate; wave loop inside `Playing`

The room's lifecycle is a state machine: `Idle → Dealing → Playing → Depile → Scoring → (next round | GameOver | Deathmatch)`. We model phases with the **typestate pattern** so invalid transitions are compile errors (e.g. you cannot score before depiling). `Playing` contains the wave sub-loop:

```
              ┌──────────────────────── Playing (one round) ─────────────────────────┐
  Dealing ──► │  WaveOpen (timer 30s/10s; active players commit 0/1, HIDDEN)          │
              │     └─ close on timer expiry OR unanimous lock-in (D-R1) ─┐           │
              │  WaveResolve: reveal → resolve effects (fixed order,      │           │
              │     pre-wave snapshot) → ONE explosion check              │           │
              │     ├─ explosion ───────────────────────────► end round (boom)        │
              │     ├─ all remaining passed ────────────────► end round (settle)      │
              │     ├─ ≤1 active left ─► one final wave ─────► settle                  │
              │     └─ else ─────────────────────────────────► next WaveOpen          │
              └──────────────────────────────────────────────────────────────────────┘
                        │ (every round, boom or safe)
                        ▼
                     Depile (reverse-order reveal) ─► Scoring ─► next/GameOver
```

**Why typestate:** the constitution wants a server that can't enter illegal states; encoding phases in types makes whole categories of bug unrepresentable and documents the flow. Alternative rejected: a single `enum Phase` matched at runtime — simpler, but transition legality becomes runtime-checked and easy to get wrong as the machine grows (it already has a tiebreaker branch).

### D-A5. Effect resolution — fixed-order resolver over a pre-wave snapshot

`resolve.rs` implements the 7-step order (cards → volatility mods → color/identity → points → removal → information → explosion check) as an explicit pipeline. Before applying the wave, it freezes a **pre-wave snapshot** of the pot; Copycat/Double Down/Recall read that snapshot, so same-wave cards don't see each other. Same-kind modifiers in one wave each apply to the snapshot and **sum** (D-R3), making the result order-independent. This is essentially a small **Command** sequence resolved in a defined priority.

### D-A6. Concurrency — room task owns state (from approved §5)

One Tokio task per room is the sole owner of game state. Each player connection is a task: inbound WS → decode → `mpsc::Sender<PlayerAction>` to the room; outbound → a **bounded per-player `mpsc`** the room writes to (chosen over `broadcast` to avoid lag-drop of critical events, per the architecture doc's backpressure note). Rooms are found via a `DashMap<RoomCode, RoomHandle>` registry. Wave/phase timers use `tokio::time`. This keeps state single-owner and lock-free on the hot path.

### D-A7. Persistence & observability (from approved §8/§9)

`sqlx` against PostgreSQL; one write at `GameOver` (games + game_players + optional game_rounds). Anonymous session auth issues a UUID + token on join; the player row is written at game completion. `tracing` (JSON) + `metrics` with a Prometheus exporter; the balance metrics (explosion rate, durations, cards/round) are first-class because they drive Principle IV.

### Research decisions captured (D-R1 … D-R5)

These resolve rules ambiguities not pinned down by `game-design.md`. All balance values are `[needs playtesting]` starting points the bot harness will tune.

- **D-R1 — Wave close timing.** Run the full wave timer; a player's selection is changeable until close; if all active players lock in early, the wave closes early. *Rationale:* commits are hidden, so there is no in-wave information to react to — the timer is pure think-time, and early-close keeps 10s waves snappy. *Rejected:* resolve-on-first-submit (removes the ability to reconsider).
- **D-R2 — Bountiful Brew attribution.** `+1/card` inflates the **pot total only**, not any color's total. *Rationale:* matches "fattens the pot" — changes payout and blast size, not who wins. *Rejected:* attributing to a color (would let a scoring modifier silently swing dominance).
- **D-R3 — Stacked same-color Double Down.** Each applies to the frozen pre-wave snapshot and **sums** → two same-color Double Downs = ×3, order-independent. *Rejected:* sequential composition (×4) — order-dependent and harder to reason about.
- **D-R4 — "Deal 5" semantics.** Top up each hand **to** 5 at round start; unplayed cards carry over and count toward the 5. *Rationale:* keeping a Peek/Shield means drawing fewer fresh cards — that *is* the hoarding trade-off the design wants; also bounds hand size. *Rejected:* dealing 5 fresh on top of carryover (hands balloon, deck math explodes).
- **D-R5 — Reshuffle from discard on exhaustion.** When the draw deck empties, reshuffle the discard pile (all previously revealed/used cards) into a fresh draw deck. Refill demand is light — after round 1 the table only redraws what it spent — so this rarely fires; it is a safety net. *Rationale:* the updated `game-design.md` §13 chose this and resolved the card-counting worry — counting operates **per shuffle**, resetting transparently and equally for everyone, exactly like a real card shoe. *Rejected:* sizing the deck so it never reshuffles (brittle — one high-consumption game could still starve it, and it forces an artificially large deck). **(Supersedes the earlier no-reshuffle stance.)**

### Constitution Check

- **I. Server-Authoritative** — Met. The room task is the sole state owner; every action is validated (`transport` + `game`); audience-scoped messages guarantee no hidden info leaks; the bot harness consumes only player-permitted data, continuously testing the secret boundary.
- **II. Agent-Driven Development** — Met for R1's layer. R1 builds the protocol-over-WebSocket seam and connection smoke tests; the full Layer-1 bot harness attaches immediately after in the `bot-balance-harness` change, and Layers 2 (agent) and 3 (visual) later. All source is plain-text Rust/TOML, fully agent-writable.
- **III. Start Simple, Scale Later** — Met. Single binary with logical modules (not split services); post-game persistence (not event sourcing); invite links **and** the queue both ship because both are in-scope for R1, but with no host/settings to keep rooms trivial; the content/config boundary is the deliberately-designed seam for tomorrow's churn. *Justified complexity:* typestate phases (over a runtime enum) — justified by Principle I's "can't enter illegal states" goal and the growing machine (Deathmatch branch).
- **IV. Playtest-Driven Balance** — Met. Every numeric value lives in validated config, not code; all are tagged `[needs playtesting]`; the bot harness exists precisely to run thousands of games and surface degenerate strategies and the explosion-rate target before humans play.

## Risks / Trade-offs

- **Stale brainstorm misleads implementation** → The specs in this change are authoritative; `server-architecture.md` will be annotated as partially superseded, and the proposal enumerates exactly which sections die.
- **Content/engine boundary leaks under pressure** (a "just match on this one card" shortcut) → Encode behavior as traits + registry from day one; a code-review check that `game/` never names a concrete content item.
- **Effect-ordering / modifier-stacking edge cases are subtle** (e.g. Double-Down stacking, Reversal parity, Shield-in-Deathmatch cascade) → These are pinned as explicit spec scenarios and must be covered by unit tests in `resolve.rs`/`scoring.rs`/`deathmatch.rs`; the bot harness exercises them at volume.
- **Deck too small → frequent reshuffles** (each shuffle churns the card-count signal) → startup validation ensures the deck covers at least the initial deal with margin; the bot harness measures reshuffle frequency across thousands of games and flags a deck that shuffles too often.
- **Typestate adds boilerplate** → Accept it; the transition-safety and self-documentation pay for the verbosity in a state machine this central.
- **Wave full-timer adds latency on quiet waves** → Mitigated by unanimous early-close (D-R1); timers are config values to retune from playtest data.

## Migration Plan

Greenfield — no migration. Deployment steps: provision PostgreSQL and run the schema migration; ship the content config alongside the binary; the server validates config and connects to the DB at startup, failing fast if either is bad. Rollback is trivial (no prior version, no persisted in-progress state to reconcile).

## Open Questions

- Exact deck size and the full per-card volatility/points distributions — deferred to the bot harness (Principle IV); R1 ships sensible defaults that pass validation.
- Where in storage (if at all) per-round analytics rows are toggled on — defaulting to on, but cheap to make configurable.
- Whether `shared/` should also carry the content *types* (not data) for a future client to render cards — leaning no for R1 to keep the protocol crate minimal; revisit when the client lands.
