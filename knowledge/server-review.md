# Boiling Point — Server Code Review

A thorough review of the Rust server (`server/`) and its protocol crate
(`protocol/`), written as both an architecture reference and an evaluation. It
covers how the server is wired, walks each subsystem, and records findings,
risks, and prioritized recommendations against the
[constitution](../CLAUDE.md) and the [game design](game-design.md).

Reviewed against `main` @ `27cc398` (2026-05-31). The full workspace test suite
was green at review time: **51 passed, 0 failed, 2 ignored** (both ignored tests
require a live PostgreSQL). Balance numbers tagged **[needs playtesting]** match
the game design's convention — they are hypotheses, not settled values.

**Overall:** a clean, genuinely server-authoritative implementation with an
unusually disciplined content/loop separation and strong unit-test coverage of
the game engine. The main gaps are not correctness bugs but *wiring* gaps: a
second, less-tested copy of the game loop drives real play; persistence is built
but never called; and a well-designed secret-routing safety rail goes unused.

---

## 1. Architecture at a Glance

### Crates

Two-crate workspace (`Cargo.toml` — `members = ["protocol", "server"]`,
edition 2024):

- **`protocol/`** — the *narrow waist*: only the DTOs that cross the wire, the
  public value-types they reference, and the codec. **No game logic, no
  secrets** (`protocol/src/lib.rs:1-8`).
- **`server/`** — the authoritative domain model (deck, hands, boiling point),
  the game loop, transport, lobby, persistence, and observability. Depends on
  `protocol` only for wire vocabulary (`server/src/lib.rs:1-21`).

### Module map (`server/src/`)

| Module | Responsibility |
|---|---|
| `main.rs` | Bootstrap: validate config → bind `:8080` → serve. Fail-fast. |
| `transport.rs` | Axum app, WebSocket upgrade, per-connection read/write tasks, version handshake, rate limiting. |
| `lobby/` | `session` (anonymous auth), `codes` (invite codes), `registry` (live rooms), `room` (per-room task), `matchmaking` (4-player queue). |
| `session.rs` | `run_game`: the **async, networked** game loop driving one full game over the wire. |
| `game/` | The **synchronous** engine: `runner` (`Game::play_out`), `round`, `resolve`, `scoring`, `deathmatch`, `deck`, `pot`, `modifiers`, `state`, `card`. |
| `content/` | Config-driven content behind the registry: `card`, `effect`, `modifier`, `registry`. Strategy + Registry patterns. |
| `config.rs` | TOML schema + fail-fast validation + registry assembly. |
| `persistence.rs` | Post-game PostgreSQL writes (sqlx). |
| `observability.rs` | JSON tracing + Prometheus metrics. |

### Request / game flow

```
WebSocket /ws  (transport::handle_socket)
  │  first frame must be an entry message (CreateRoom | JoinRoom | EnqueueMatch)
  │  + matching PROTOCOL_VERSION, else Error and close
  ▼
RoomCommand channel  ──►  lobby/room.rs::run   (one async task = sole owner of a room)
                              │  lobby loop: Join/Leave/Heartbeat/Emote, idle timeout 300s
                              │  at 4 seats → GameStarting, then…
                              ▼
                         session.rs::run_game   (owns the room rx for the whole game)
                              │  per round: refill hands → draw modifier (r≥2)
                              │  per wave:  broadcast WaveOpened → collect_wave (timed,
                              │             hidden commits) → resolve_wave → broadcast
                              │  depile → score → ScoreUpdate
                              ▼
                         GameOver  (+ Deathmatch on a tie for the lead)
```

### Concurrency model

The strongest structural choice: **single-task ownership of game state, no locks
on the game path.**

- Each connection has its own outbound `mpsc::Sender<ServerMessage>`; a dedicated
  writer task serialises to the socket (`transport.rs:80-89`).
- Each room is one async task that *exclusively* owns its state; connections only
  talk to it via `RoomCommand`s (`lobby/room.rs:1-8`). During a game,
  `run_game` takes the room's receiver directly (`lobby/room.rs:203`), so all
  commits funnel through one task — no shared mutable game state, no `Mutex`.
- The only shared structures are concurrent maps and one short-lived lock:
  `RoomRegistry` and `SessionStore` are `DashMap`-backed (`lobby/registry.rs:21`,
  `lobby/session.rs:14`); `MatchQueue` uses a `Mutex<Vec<…>>` held only for the
  drain and **never across an await** (`lobby/matchmaking.rs:52-66`).
- Wire format: MessagePack primary, JSON fallback for debugging, via
  `protocol::codec` (`transport.rs:63,82`).

---

## 2. Subsystem Walkthrough

### Startup & the fail-fast config gate

`main.rs` initialises observability, parses the embedded `content.toml`
(`include_str!`), and **builds (thus validates) the registry before binding any
port** — an invalid config calls `std::process::exit(1)` rather than starting
(`main.rs:23-37`). `config.rs::validate` is comprehensive: deck-size sum, deal
coverage, effect/wild ratio bands, presence of every rules-required effect,
modifier-pool minimum, boiling-point range, and emote-palette uniqueness — each
with a specific `ConfigError` (`config.rs:135-313`). The default deck is 90
cards with 8 effects and a 20-copy modifier pool (`config.rs:361-373`).

### Transport (`transport.rs`)

A single route, `GET /ws` (`transport.rs:47-51`). Handshake requires a matching
`PROTOCOL_VERSION` and one of three entry messages, else an `Error` is sent and
the writer is drained so the client actually receives it before close
(`transport.rs:91-102`, `136-197`). The action loop enforces a 100 ms rate limit
(`RATE_LIMIT`, `transport.rs:30,117-119`) and a heartbeat-driven idle timeout
(`conn_timeout`, default 90 s — `main.rs:46`, `transport.rs:111-115`). On exit it
sends `RoomCommand::Leave`. Transport has a solid set of live-WebSocket
integration tests (handshake, matchmaking, heartbeat, abandonment, a full
4-client game) at `transport.rs:337-628`.

### Lobby, rooms, matchmaking (`lobby/`)

- **Auth** is anonymous: a presented token resolves to a stable `PlayerId`; a
  fresh connection mints both. Known tokens return the same id, enabling
  reconnection (`lobby/session.rs:25-35`).
- **Rooms** are created collision-safe (retry on duplicate code) and self-
  deregister when they end (`lobby/registry.rs:50-67`, `lobby/room.rs:256`).
  Codes are human-readable `BREW-XXXX` from an unambiguous alphabet
  (`lobby/codes.rs`).
- **Matchmaking** parks players on a `oneshot` until a fourth arrives, then
  creates a room and joins all four (`lobby/matchmaking.rs:45-81`). This *is*
  wired end-to-end (`main.rs:41`, `transport.rs:170-186`, test at
  `transport.rs:337`).
- **Resilience**: a `Leave` mid-lobby drops the seat; if the room empties the
  task ends (`lobby/room.rs:211-223`). Mid-game, a disconnected player auto-
  passes and a reconnect reattaches the channel and receives a scoped
  `StateSnapshot` (`session.rs:443-449,494-508,198-212`).

### The game loop (`session.rs::run_game`)

For each of `ROUND_COUNT` (5) rounds: draw a cumulative modifier from round 2
(`session.rs:126-139`), refill every hand to 5 and send each player their private
`YourHand` (`session.rs:142-157`), roll the hidden boiling point and apply
modifier offsets (`session.rs:159-160`), then run waves until the round ends. A
wave broadcasts `WaveOpened`, collects hidden commits for the timer window
(`collect_wave`, `session.rs:431-538`), resolves through the engine, routes the
Peek/Expose tells, and broadcasts a `WaveResolved` that carries counts but never
card identities (`session.rs:235-262`). The round ends with a depile (boiling
point disclosed only on explosion — `session.rs:266-290`) and scoring; a tie for
the lead after round 5 is broken by a Deathmatch (`session.rs:368-399`).

### Game engine (`game/`)

The synchronous heart, fully testable in-process:

- **`Round`** owns the pot and active/locked-out bookkeeping and decides when a
  round ends: explosion, everyone-passed, or the *one-player final wave* guard
  (a lone survivor gets exactly one more wave, then settles —
  `round.rs:170-195`). `depile` walks cumulative volatility to mark the crossing
  card (`round.rs:200-224`).
- **`resolve_wave`** freezes a pre-wave snapshot, adds the committed cards, then
  resolves effects in a fixed category order against that snapshot, then does a
  *single* explosion check (`resolve.rs:128-196`). Snapshot semantics mean
  same-wave cards don't see each other and duplicate effects sum (two same-colour
  Double Downs → ×3, order-independent — `resolve.rs:5-9`, test
  `resolve.rs:245-280`). Effects are Strategy objects keyed by tag; the loop
  never matches on a concrete effect (`content/effect.rs:1-58`). The eight
  effects and their categories (State → Volatility → Identity → Points → Removal
  → Information) live in `content/effect.rs:60-202`.
- **`scoring`** computes `pot_value = (Σ points + Bountiful·per-card) ×
  Double-Stakes` and awards it to the dominant colour(s), splitting ties and
  rounding down; Reversal flips dominance to the lowest present colour; a
  shielded winner forfeits (`scoring.rs:54-117`). Explosion makes every
  non-shielded player lose the full pot value (`scoring.rs:119-138`).
- **`deathmatch`** is a volatility-only elimination: forced commits each wave,
  the highest-volatility non-shielded contributor(s) detonate, shields cascade
  the blast, all-shielded means no casualty, exhausted hands mean co-champions
  (`deathmatch.rs:149-262`).
- **`modifiers`** compose by plain arithmetic — deltas and start-volatility sum,
  multipliers multiply, reversal is parity — so contradictions cancel with no
  special cases (`game/modifiers.rs:32-61`). Default magnitudes (all **[needs
  playtesting]**): Residue +3 start, Thin Ice −4 BP, Deep Cauldron +4 BP,
  Bountiful +1/card, Double Stakes ×2 (`content/modifier.rs:36-41`); Dampen −2,
  Volatile Surge +2 (`content/effect.rs:60-62`).
- **`deck`** builds seeded with sequential `CardId`s, refills to a floor of 5
  with carryover, and reshuffles the discard when the draw pile empties — a
  visible table event that resets card counting (`game/deck.rs`).

### Determinism

Everything stochastic is seeded from one `u64` per game (`lobby/room.rs:197`):
deck build/shuffle, modifier pile, boiling-point rolls, and a derived deathmatch
seed. The sync runner has an explicit determinism test (`runner.rs:419-428`) and
a 300-game no-panic stress test (`runner.rs:405-417`). Expose's "random" card is
in fact *deterministic* (earliest non-self card) on purpose, so games stay
reproducible (`resolve.rs:79-85`).

### Persistence (`persistence.rs`)

Write-once-at-`GameOver` design over sqlx with a 5-connection pool
(`persistence.rs:61-66`), an embedded idempotent schema
(`migrations/0001_init.sql`, applied via `run_migrations`), and a single
transactional `persist_game` writing players, game, per-player results, and
per-round analytics (`persistence.rs:77-133`). **See finding F4: this module is
not actually called by the running server.**

### Observability (`observability.rs`)

JSON `tracing` plus a Prometheus exporter on `:9090` (`observability.rs:13-27`).
Counters/gauges are named in one place: `rooms_created_total`, `rooms_active`,
`games_started_total`, `games_completed_total`, `rounds_total`, and
`round_explosions_total` — the last feeds the ~30–40 % explosion-rate target
(`observability.rs:31-59`).

### Protocol & the secret boundary (`protocol/`)

The boiling point appears in exactly two messages — `PeekResult` (private) and
an exploded `Depile` — and a unit test asserts no other serialized message
carries it (`protocol/src/server.rs:1-8`,
`protocol/src/lib.rs:204-248`). `WaveResolved`, `StateSnapshot`, and the depile
deliberately omit hidden state. The crate also defines an `Outbound`/`Audience`
routing type with `is_private_only()` and a `broadcast()` debug-assert that
*would* catch broadcasting a private message (`protocol/src/server.rs:250-303`).
**See finding F3: the server does not use this rail.**

---

## 3. Findings & Risks

### F1 — Invalid in-wave actions are silently dropped, not error-replied *(compliance, medium)*

Constitution §I: *"Invalid actions receive an error response with no state
change."* In `collect_wave`, the match arms for `CommitCard`, `CommitPass`,
`LockIn`, and `Emote` are all guarded; anything that fails a guard — a card not
in hand, an action from a passed/locked-out player, an off-palette emote — falls
through to `_ => {}` and is **silently ignored** (`session.rs:461-485`). The
"no state change" half holds; the "error response" half does not. The protocol
even defines the right codes (`NotYourCard`, `LockedOut`,
`protocol/src/server.rs:79-84`), and the handshake/lobby paths *do* reply with
errors (`VersionMismatch` at `transport.rs:200-212`; `InvalidEmote`/`WrongPhase`
at `lobby/room.rs:144-151,241-249`) — only the in-wave action path stays silent.

Note the resulting inconsistency: an off-palette emote returns
`Error{InvalidEmote}` *in the lobby* but is dropped *during a wave*.

This may be a deliberate choice — replying "that card isn't yours" or "you're
locked out" during a hidden-commit wave leaks information and timing. **Decide
and record it:** either emit the existing error codes, or document silent-drop as
an intentional anti-leak measure. (The same invalid-card→pass shortcut exists in
the sync runner at `runner.rs:234-235`, where the decider is trusted code, so it
is benign there.)

### F2 — The async loop reimplements orchestration the tested engine already has *(robustness, medium)*

There are **two implementations of the round/wave/scoring/deathmatch flow**:
`game/runner.rs` (`Game::play_out`, synchronous) and `session.rs::run_game`
(async). They share the low-level pieces (`Round`, `resolve_wave`, `score_safe`,
`explosion`, `run_deathmatch`) but each *re-derives* the orchestration: hand
refill, modifier draw, the wave loop, scoring application, and the tie-break.
The synchronous runner is the one with deep tests — determinism, the 300-game
stress run, and `tie_routes_into_deathmatch` (`runner.rs:391-463`). The async
`run_game` that actually drives real games is exercised only by the
coarser-grained transport integration tests.

Consequences: the two can drift (they already differ — e.g. the runner counts
`cards_played` and builds analytics, the async path does not), and the path that
ships is the less-tested one. Consider having `run_game` delegate to a
shared core (e.g. drive `Game` via a network-backed `Decider`, surfacing the
broadcasts it needs), or at minimum add engine-level tests over the async path.

### F3 — The secret-routing safety rail is unused; no end-to-end no-leak assertion *(robustness / coverage, medium)*

The server sends `ServerMessage` directly down per-connection channels using
hand-written `broadcast(&players, …)` / `send_to(&players, id, …)` helpers
(`session.rs:75-85`), and **never constructs `Outbound` or calls `.broadcast()`/
`.to()`**. So the `is_private_only()` debug-assert that exists precisely to stop
a private message (`YourHand`, `PeekResult`, `StateSnapshot`, `Error`) from being
broadcast is dead code. The manual discipline is correct everywhere I checked —
every private message goes via `send_to`/the per-player `out` channel — but
nothing *enforces* it, so a future edit could broadcast a hand with no test or
assertion firing.

Relatedly, the full-game integration test only asserts everyone reaches
`GameOver` (`transport.rs:547-590`); the no-secret check runs against a single
lobby message (`transport.rs:409-413`). There is no test that scans an entire
game's broadcast stream for leaked secrets. Routing the server's sends through
`Outbound` (or adding a stream-scanning e2e test) would close both gaps.

### F4 — Persistence is fully built but never invoked at runtime *(wiring, medium)*

`persistence.rs` is complete and tested, and `runner.rs` even has a
`to_game_result` bridge (`runner.rs:302-351`) — but nothing on the live path
calls any of it. `AppState` has no `PgPool` (`transport.rs:32-44`), `main.rs`
never calls `connect`/`run_migrations`, and `run_game` ends at the `GameOver`
broadcast with no persistence (`session.rs:403-411`). `to_game_result` is only
reachable from the sync runner's `#[ignore]`d DB test (`runner.rs:467-496`).

**Net effect: completed games are not saved.** This is the designed "post-game
persistence" seam (Constitution §III) left unconnected rather than a bug, but it
means leaderboards/match-history don't function yet, and the async path would
also need a `to_game_result`-equivalent (it currently tracks neither
`cards_played` nor per-round analytics). Worth recording explicitly so it isn't
mistaken for working.

### F5 — Minor notes *(low)*

- **Production `unwrap()` in the async loop.** `hands.get_mut(id).unwrap()`
  (`session.rs:145`), `*scores.get_mut(player).unwrap()` (`session.rs:304,325`),
  and the deathmatch shed closure's `min_by_key(…).unwrap()` (`session.rs:383`)
  are all invariant-guarded today but would read better as
  `entry().or_insert(0)` / `expect("invariant: …")`. The deck's `expect(…)` calls
  are guarded by prior emptiness checks (`game/deck.rs:60-71`); the deathmatch
  forced-commit `expect` is guarded by the `is_empty` short-circuit
  (`deathmatch.rs:161-177`).
- **Logging level isn't tunable.** `observability::init` installs a JSON
  subscriber with no `EnvFilter` (`observability.rs:14-18`), so `RUST_LOG` is
  ignored. Fine for now; add a filter when you need to quiet/verbose logs in
  prod.
- **Doc nit.** `resolve.rs` calls the order a "fixed 7-step order"
  (`resolve.rs:2,159`) but `EffectCategory` has six variants
  (`content/effect.rs:12-26`).
- `contributions()` is recomputed each wave (O(players × pot)); negligible at
  this scale (`session.rs:413-426`).

---

## 4. Prioritized Recommendations

1. **Decide F1 (compliance).** Either reply with the existing
   `NotYourCard`/`LockedOut`/`WrongPhase` codes for in-wave invalid actions, or
   record in this doc / the wire-protocol spec that silent-drop is an
   intentional anti-leak choice. Resolve the lobby-vs-wave emote inconsistency
   the same way.
2. **Wire or explicitly defer persistence (F4).** Add a `PgPool` to `AppState`,
   call `run_migrations` at startup, and have `run_game` build a `GameResult`
   and call `persist_game` at `GameOver` — or, if it's intentionally deferred,
   say so in `knowledge/` so it isn't read as functional.
3. **Converge the two loops (F2).** Have `run_game` delegate to a shared engine
   core, or add engine-level tests directly over the async path, so the shipping
   path inherits the determinism/stress coverage.
4. **Make the secret boundary enforce itself (F3).** Route server sends through
   `Outbound`/`Audience`, and/or add an end-to-end test that scans a whole
   game's broadcast stream for leaked secrets.
5. **Polish (F5).** Replace the four invariant `unwrap()`s with explicit
   `expect`/`entry`, add an `EnvFilter`, fix the "7-step" doc nit.

None of these block correctness today; #1 and #4 are the constitution-facing
ones, #2 is the most visible functionality gap.

---

## 5. Constitution Compliance Matrix

| Principle | Verdict | Evidence | Deviation |
|---|---|---|---|
| **I. Server-Authoritative** | Strong | Full state (deck, hands, boiling point) is server-only; `CardId` is opaque and commits are validated against the hand (`session.rs:462-464`); all RNG server-side; secret boundary tested (`protocol/src/lib.rs:204-248`). | **F1**: invalid in-wave actions get no error response (the "no state change" half holds). |
| **II. Agent-Driven Development** | Strong (with a gap) | Sync engine is fully agent-testable via `Decider`/`DeathmatchDecider` traits (`runner.rs:60-69`, `deathmatch.rs:35-44`), deterministic seeds, 300-game stress test; `protocol` is a clean narrow waist for bots. | The protocol bot harness and Claude-as-player harness are *designed* (openspec changes) but not yet crates in the workspace (`members = ["protocol","server"]`). |
| **III. Start Simple, Scale Later** | Strong | Single binary; anonymous token auth; invite codes + simple 4-player matchmaking; embedded config; one fixed game mode; persistence is post-game-only by design. | Persistence seam built but not connected (**F4**) — start-simple, but record it. |
| **IV. Playtest-Driven Balance** | Strong (with a gap) | Every tunable is externalised to `content.toml` and tagged **[needs playtesting]** in code (`content/effect.rs:60-62`, `content/modifier.rs:36-41`); `round_explosions_total` tracks the 30–40 % target. | The bot harness that would run *thousands* of games to validate balance isn't built yet, so at-scale validation can't run. |
| **Typestate (accepted deviation)** | Documented | Phase/round transitions use `Option<RoundEnd>` + `is_open()` rather than compile-time typestate (`round.rs:82-133`). | This is the previously-accepted deviation in the project history. |
