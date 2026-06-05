# Boiling Point — Server Code Review

A thorough review of the Rust server (`server/`) and its protocol crate
(`protocol/`), written as both an architecture reference and an evaluation. It
covers how the server is wired, walks each subsystem, and records findings,
risks, and prioritized recommendations against the
[constitution](../../CLAUDE.md) and the [game design](../game-design.md).

Originally reviewed against `main` @ `27cc398` (2026-05-31); re-reviewed
2026-06-02, then **refreshed 2026-06-05** after the `group-model`,
`group-fill-and-standings`, `converge-game-loops`, and `persistence-and-replays`
changes landed. The full workspace test suite is green (engine + transport,
bot-harness, tui-client; a few ignored tests require a live PostgreSQL). Balance
numbers tagged **[needs playtesting]** match the game design's convention — they
are hypotheses, not settled values.

> **Re-review status (refreshed 2026-06-05).** Every finding below is now
> **resolved**; each finding's Status note records the change that closed it.
> Findings are tracked as OpenSpec changes rather than loose advice:
> - **F1, F3, F5 → resolved** by **`review-remediation`** (2026-06-03).
> - **F2 → resolved** by **`converge-game-loops`** (2026-06-04): `run_game` now
>   drives the tested `Game` engine through one orchestration core, and a
>   sync==async parity test pins their final scores together.
> - **F4 → resolved** by **`persistence-and-replays`** (2026-06-03): persistence is
>   wired on the live path (match results + timeless replays in one post-game write)
>   and degrades cleanly when no database is configured.
> - Two structural changes this refresh also folds in: the **room→group rename +
>   persistent groups** (`group-model`) and **group matchmaking fill + members/guests
>   + live standings** (`group-fill-and-standings`). The architecture sections below
>   describe the current (group) shape.
> - Workspace: the bot harness (`bot-harness/`), terminal client (`tui-client/`), and
>   the Node/TS Claude-as-player harness (`agent-harness/`) each have a dedicated
>   review in this folder.

**Overall:** a clean, genuinely server-authoritative implementation with an
unusually disciplined content/loop separation and strong test coverage of the
game engine. The *wiring* gaps the first review flagged are now closed: one
orchestration core backs both the in-process and the networked loop (**F2**),
post-game persistence and timeless replays are wired on the live path (**F4**),
and the secret-routing rail is enforced on every server send (**F3**). What
remains is balance validation (Principle IV) and the usual product surface, not
correctness or wiring debt.

---

## 1. Architecture at a Glance

### Crates

Four-crate workspace (`Cargo.toml` — `members = ["protocol", "server",
"tui-client", "bot-harness"]`, edition 2024), plus the Node/TS `agent-harness/`.
This review focuses on the two below; the others have dedicated reviews in this
folder:

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
| `transport.rs` | Axum app, WebSocket upgrade, per-connection read/write tasks, version handshake, rate limiting, the durable per-socket session router. |
| `lobby/` | `session` (anonymous auth), `codes` (invite codes), `registry` (live groups + optional `PgPool`), `group` (per-group task — persists across games), `matchmaking` (4-player queue + group fill). |
| `session.rs` | `run_game`: the **async, networked** loop that drives one full game over the wire by stepping the `Game` engine. |
| `game/` | The engine core: `runner` (`Game` — owns the round/wave/scoring/deathmatch steps, drives both loops), `round`, `resolve`, `scoring`, `deathmatch`, `deck`, `pot`, `modifiers`, `state`, `card`. |
| `replay.rs` | Timeless replays: encode a finished game to `seed + action_log`, verify integrity, and reconstruct its public event stream by re-running the pinned engine. |
| `content/` | Config-driven content behind the registry: `card`, `effect`, `modifier`, `registry`. Strategy + Registry patterns. |
| `config.rs` | TOML schema + fail-fast validation + registry assembly. |
| `persistence.rs` | Post-game PostgreSQL writes (sqlx): results + replay payload in one transaction. |
| `admin/` | Operator command-plane + the live open-span projection behind the admin API. |
| `observability.rs` | JSON tracing, the span lifecycle/projection, and Prometheus metrics. |

### Request / game flow

```
WebSocket /ws  (transport::handle_socket — one durable session per socket)
  │  first frame must be an entry message (CreateGroup | JoinGroup | EnqueueMatch)
  │  + matching PROTOCOL_VERSION, else Error and close
  ▼
GroupCommand channel ──► lobby/group.rs::run  (one async task = sole owner of a group)
                              │  persistent lobby: Join/Leave/Heartbeat/Emote/PlayAgain/
                              │  FillGroup, idle timeout 300s; standings kept across games
                              │  at 4 ready seats → GameStarting, then…
                              ▼
                         session.rs::run_game  (owns the group rx for the whole game;
                              │  drives a Game through begin_round → resolve_wave →
                              │  settle_round, adding only wire I/O + spans)
                              │  per round: refill hands → draw modifier (r≥2)
                              │  per wave:  broadcast WaveOpened → collect_wave (timed,
                              │             hidden commits) → Game::resolve_wave → broadcast
                              │  depile → score → ScoreUpdate
                              ▼
                         GameOver (+ Deathmatch on a tie) → optional post-game write
                              │  (results + replay) → return survivors to the group lobby
```

### Concurrency model

The strongest structural choice: **single-task ownership of game state, no locks
on the game path.**

- Each connection has its own outbound `mpsc::Sender<ServerMessage>`; a dedicated
  writer task serialises to the socket (`transport.rs`).
- Each group is one async task that *exclusively* owns its state; connections only
  talk to it via `GroupCommand`s (`lobby/group.rs`). During a game, `run_game`
  takes the group's receiver directly, so all commits funnel through one task — no
  shared mutable game state, no `Mutex`. A group **outlives a single game**: it
  serves its lobby, runs a game when four seats are ready, then returns the
  survivors to the lobby (`group-model`).
- The only shared structures are concurrent maps and one short-lived lock:
  `GroupRegistry` and `SessionStore` are `DashMap`-backed (`lobby/registry.rs`,
  `lobby/session.rs`); `MatchQueue` uses a `Mutex<Vec<…>>` held only for the
  drain and **never across an await** (`lobby/matchmaking.rs`).
- Wire format: MessagePack primary, JSON fallback for debugging, via
  `protocol::codec` (`transport.rs`).

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

A single route, `GET /ws` (`transport.rs`). The socket is a **durable session**:
it authenticates once on the first entry message, then routes — entry messages
bind it to a group, `LeaveGroup` returns it to the unbound menu, table actions
forward to the bound group, and the socket survives a game or group ending
(`group-model` D5). The handshake requires a matching `PROTOCOL_VERSION` and one
of three entry messages, else an `Error` is sent and the writer is drained so the
client receives it before close. The action loop enforces a 100 ms rate limit
(`RATE_LIMIT`) on table actions and a heartbeat-driven idle timeout
(`conn_timeout`). Transport has a solid set of live-WebSocket integration tests
(handshake, matchmaking, heartbeat, abandonment, a full 4-client game, leave/
re-join, two games via play-again, the secret-leak scan, and the span tree) in
`transport.rs`'s test module.

### Lobby, groups, matchmaking (`lobby/`)

- **Auth** is anonymous: a presented token resolves to a stable `PlayerId`; a
  fresh connection mints both. Known tokens return the same id, enabling
  reconnection (`lobby/session.rs`).
- **Groups** are created collision-safe (retry on duplicate code) and self-
  deregister when they empty, idle out (300 s), or an operator kills them
  (`lobby/registry.rs`, `lobby/group.rs`). A group **persists across games** and
  keeps a live, in-memory **standings** tally — per-member games/wins plus a guest
  aggregate (`group-fill-and-standings`). It distinguishes **members** (joined by
  invite/quick-match) from **guests** (placed by matchmaking fill, dropped at
  `GameOver`). Codes are human-readable `BREW-XXXX` from an unambiguous alphabet
  (`lobby/codes.rs`).
- **Matchmaking** parks players on a `oneshot` until a fourth arrives, then
  forms a group and joins all four; a partial group can also `FillGroup` to top
  itself up with guests from the queue (`lobby/matchmaking.rs`). Wired
  end-to-end (`main.rs`, `transport.rs`, transport tests).
- **Resilience**: a `Leave` mid-lobby drops the seat; if the group empties the
  task ends. Mid-game, a disconnected player auto-passes and a reconnect
  reattaches the channel and receives a scoped `StateSnapshot` (`session.rs`).

### The game loop (`session.rs::run_game`)

`run_game` no longer re-derives the game flow — it **drives the tested `Game`
engine** (see F2). For each of `ROUND_COUNT` (5) rounds it calls
`Game::begin_round` (draw the round's modifier from round 2, refill hands to 5,
roll the hidden boiling point), broadcasts `ModifierRevealed`/`DeckReshuffled` and
each player's private `YourHand`, then runs waves until `Game` reports the round
closed. A wave broadcasts `WaveOpened`, collects hidden commits for the timer
window (`collect_wave`) as raw intents, calls `Game::resolve_wave` (which validates
against hands, takes the committed cards, applies the wave, returns recalled cards
to their owners, and records the replay action log), routes the Peek/Expose tells,
re-sends a private `YourHand` to any owner whose hand a recall grew (D3), and
broadcasts a `WaveResolved` that carries counts but never card identities. The
round ends with `Game::settle_round` — a depile (boiling point disclosed only on
explosion) and scoring; a tie for the lead after round 5 is broken by
`Game::break_tie`'s Deathmatch. At `GameOver`, if a database is configured, the
results and a timeless replay are written in one transaction.

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

Everything stochastic is seeded from one `u64` per game (`lobby/group.rs`):
deck build/shuffle, modifier pile, boiling-point rolls, and a derived deathmatch
seed (`seed ^ 0xD3A7_4A7C`). The `Game` engine has an explicit determinism test
and a 300-game no-panic stress test (`runner.rs`), and — since `converge-game-loops`
— a `sync==async` parity test asserts the networked `run_game` reproduces
`Game::play_out`'s final scores for fixed seeds (`session::tests`). Because the
engine is deterministic from `seed + action_log`, a finished game is **replayable**
end to end (`replay.rs`). Expose's "random" card is in fact *deterministic*
(earliest non-self card) on purpose, so games stay reproducible (`resolve.rs`).

### Persistence & replays (`persistence.rs`, `replay.rs`)

Write-once-at-`GameOver` design over sqlx, an embedded idempotent schema
(`migrations/0001_init.sql` + `0002_replays.sql`, applied under an advisory lock
via `run_migrations`), and a single transactional `persist_game` writing players,
game, per-player results, per-round analytics, **and** the timeless replay payload.
The shared `build_game_result` (`runner.rs`) produces the result identically for
both loops. `replay.rs` encodes a finished game as `seed + action_log` (with an
integrity hash and a content fingerprint) and `reconstruct`s its public event
stream by re-running the pinned engine via `Game::play_out_with_events`. **F4 is
resolved:** `--database-url`/`DATABASE_URL` connects a `PgPool` threaded
`GroupRegistry → run_game`; with no URL configured the post-game write is a clean
no-op (logged once) — persistence is optional infrastructure, never a precondition
for play.

### Observability (`observability.rs`, `admin/`)

JSON `tracing` plus a Prometheus exporter. Counters/gauges are named in one place
(`observability.rs`): `groups_created_total`/`groups_active`, `games_started_total`/
`games_completed_total`/`games_active`, `rounds_total`, `round_explosions_total`
(feeding the ~30–40 % explosion-rate target), `round_dominations_total`/
`round_splits_total`, `waves_total`/`wave_timeouts_total`, `cards_committed_total`,
`deck_reshuffles_total`, and `player_reconnects_total`. A structured span tree
(`game → round → wave → resolve/commit/score`, plus `hand`/`reconnect`) carries the
secret in-flight state as in-process-only attributes; the `admin/` projection
serves a privileged live-state reveal off the open spans, never the player wire.

### Protocol & the secret boundary (`protocol/`)

The boiling point appears in exactly two messages — `PeekResult` (private) and
an exploded `Depile` — and a unit test asserts no other serialized message
carries it (`protocol/src/server.rs`, `protocol/src/lib.rs`). `WaveResolved`,
`StateSnapshot`, and the depile deliberately omit hidden state. The crate also
defines an `Outbound`/`Audience` routing type with `is_private_only()` and a
`broadcast()` debug-assert that catches broadcasting a private message. **F3 is
resolved:** every server→client send in `run_game` now routes through a single
`dispatch` egress that constructs the `Outbound`, so the debug-assert guards every
broadcast, and an end-to-end test scans a whole game's frames for leaked secrets.

---

## 3. Findings & Risks

### F1 — Invalid in-wave actions are silently dropped, not error-replied *(compliance, medium)*

> **Status (resolved 2026-06-03 by `review-remediation`).** `collect_wave` now replies
> with the existing error codes instead of dropping: `NotYourCard` (a card not in hand),
> `LockedOut` (a commit/pass/lock-in from a passed/locked-out player), `InvalidEmote`
> (an off-palette emote — resolving the lobby-vs-wave inconsistency the same way the
> lobby does), and `WrongPhase` (an entry message mid-game). Each carries only the
> reason — never pot/volatility/boiling-point — and applies no state change. Unit tests
> (`session::tests`) assert each code and that game state is unchanged.

Constitution §I: *"Invalid actions receive an error response with no state
change."* In `collect_wave`, the match arms for `CommitCard`, `CommitPass`,
`LockIn`, and `Emote` are all guarded; anything that fails a guard — a card not
in hand, an action from a passed/locked-out player, an off-palette emote — falls
through to `_ => {}` and is **silently ignored** (`session.rs:461-485`). The
"no state change" half holds; the "error response" half does not. The protocol
even defines the right codes (`NotYourCard`, `LockedOut`,
`protocol/src/server.rs:79-84`), and the handshake/lobby paths *do* reply with
errors (`VersionMismatch` at `transport.rs:200-212`; `InvalidEmote`/`WrongPhase`
in `lobby/group.rs`) — only the in-wave action path stays silent.

Note the resulting inconsistency: an off-palette emote returns
`Error{InvalidEmote}` *in the lobby* but is dropped *during a wave*.

This may be a deliberate choice — replying "that card isn't yours" or "you're
locked out" during a hidden-commit wave leaks information and timing. **Decide
and record it:** either emit the existing error codes, or document silent-drop as
an intentional anti-leak measure. (The same invalid-card→pass shortcut exists in
the sync runner at `runner.rs:234-235`, where the decider is trusted code, so it
is benign there.)

### F2 — The async loop reimplements orchestration the tested engine already has *(robustness, medium)*

> **Status (resolved 2026-06-04 by `converge-game-loops`).** There is now a **single
> orchestration core**: `Game` (`game/runner.rs`) owns the hands, deck, scores,
> modifiers, RNG, and round bookkeeping and exposes the round/wave/scoring/deathmatch
> steps (`begin_round` / `resolve_wave` / `settle_round` / `leaders` / `break_tie`).
> Both `Game::play_out` (sync) and `session.rs::run_game` (async, shipping) drive those
> same steps — `run_game` no longer re-derives the flow; it adds only the wire I/O
> (collect commits within the wave timer, broadcast the public outcome) and the
> observability spans. The two documented divergences are reconciled: the seeds are
> aligned on the sync runner's derivation (`rng = seed`, deathmatch `seed ^ 0xD3A7_4A7C`),
> and a recall now re-sends the owner a private `YourHand` (D3). A
> `session::tests::async_path_matches_sync_runner_for_fixed_seeds` test asserts the async
> path produces the **same final scores as `Game::play_out`** across several seeds (the
> safety net that would fire loudly on any future drift), and
> `async_path_completes_across_many_seeds_without_panicking` keeps the no-panic stress
> coverage. The analytics the async path used to drop (`cards_played`, per-round
> `RoundLog`) are now populated by the shared core, so the converged path can feed
> `to_game_result`. No observable wire-behavior change (the transport integration tests
> stay green).

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

> **Status (resolved 2026-06-03 by `review-remediation`).** Every server→client send in
> `run_game` now routes through a single `dispatch` egress that constructs an
> `Outbound` via `ServerMessage::broadcast`/`.to(..)`, so the `is_private_only()`
> debug-assert guards every broadcast (a protocol `#[should_panic]` test confirms it
> fires). A new end-to-end test (`transport::tests::full_game_broadcasts_never_leak_secrets`)
> plays a full four-player game and scans every frame each client receives, asserting the
> boiling point appears only in a private `PeekResult` or an exploded `Depile` — and that
> a safe-brew `Depile` hides it.

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

> **Status (2026-06-03): RESOLVED by the `persistence-and-replays` change.** The
> rework landed: `--database-url`/`DATABASE_URL` connects a `PgPool` and runs the
> (advisory-locked) migrations at boot; the pool lives in `AppState` and is
> threaded `GroupRegistry → run_game`; at `GameOver` the live path builds the
> `GameResult` (via the shared `build_game_result`, now populating `cards_played`
> and per-round analytics) **and** a timeless replay payload, persisting both in
> one `db.write` transaction. With no URL configured the server plays normally
> and skips the write (logged once). The original description below is retained as
> the pre-change state.

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

> **Status (resolved 2026-06-03 by `review-remediation`).** The four invariant
> `unwrap()`s are now `expect("invariant: …")` (hand refill, deathmatch shed) or
> `entry().or_insert(0)` (score accumulation), and the "fixed 7-step order" doc nit in
> `resolve.rs` now reads "fixed 6-category order". The logging-level sub-item was already
> resolved (`--log-level`, see the re-review banner above).

- **Production `unwrap()` in the async loop.** `hands.get_mut(id).unwrap()`
  (`session.rs:145`), `*scores.get_mut(player).unwrap()` (`session.rs:304,325`),
  and the deathmatch shed closure's `min_by_key(…).unwrap()` (`session.rs:383`)
  are all invariant-guarded today but would read better as
  `entry().or_insert(0)` / `expect("invariant: …")`. The deck's `expect(…)` calls
  are guarded by prior emptiness checks (`game/deck.rs:60-71`); the deathmatch
  forced-commit `expect` is guarded by the `is_empty` short-circuit
  (`deathmatch.rs:161-177`).
- **Logging level isn't tunable.** *(Resolved 2026-06-02.)* `observability::init`
  now takes an `EnvFilter` seeded from the new `--log-level` flag (falling back to
  `RUST_LOG`, then `info`) — `server/src/main.rs`, `observability.rs`.
- **Doc nit.** `resolve.rs` calls the order a "fixed 7-step order"
  (`resolve.rs:2,159`) but `EffectCategory` has six variants
  (`content/effect.rs:12-26`).
- `contributions()` is recomputed each wave (O(players × pot)); negligible at
  this scale (`session.rs:413-426`).

---

## 4. Prioritized Recommendations

All five recommendations from the original review are now **done** — retained
here as a record of what closed each finding.

1. **Decide F1 (compliance) — done.** In-wave invalid actions reply with the
   existing `NotYourCard`/`LockedOut`/`InvalidEmote`/`WrongPhase` codes (reason
   only, no state change), resolving the lobby-vs-wave emote inconsistency the
   same way (`review-remediation`).
2. **Rework persistence (F4) — done.** `persistence-and-replays` wired match
   results + timeless replays on the live path (`PgPool` in `AppState`, migrations
   at startup, results + replay persisted in one transaction at `GameOver`),
   degrading cleanly when no database is configured.
3. **Converge the two loops (F2) — done.** `run_game` drives the tested `Game`
   engine through one orchestration core; a `sync==async` parity test pins the
   shipping path to the engine's scores, and the analytics/replay log flow off the
   shared `Game` (`converge-game-loops`).
4. **Make the secret boundary enforce itself (F3) — done.** Every server send
   routes through `Outbound`/`Audience`, and an end-to-end test scans a whole
   game's broadcast stream for leaked secrets (`review-remediation`).
5. **Polish (F5) — done.** Invariant `unwrap()`s became `expect`/`entry`, the log
   level is tunable (`--log-level` + `EnvFilter`), and the "7-step" doc nit is
   fixed (`review-remediation`).

The remaining open work is **Principle IV** (data-informed balance validation via
the bot harness), not correctness or wiring debt.

---

## 5. Constitution Compliance Matrix

| Principle | Verdict | Evidence | Deviation |
|---|---|---|---|
| **I. Server-Authoritative** | Strong | Full state (deck, hands, boiling point) is server-only; `CardId` is opaque and commits are validated against the hand (`session.rs:462-464`); all RNG server-side; secret boundary tested (`protocol/src/lib.rs:204-248`) and now routed through `Outbound` end-to-end (**F3**). | **F1 closed (2026-06-03):** invalid in-wave actions now receive an error response (`NotYourCard`/`LockedOut`/`InvalidEmote`/`WrongPhase`) with no state change. |
| **II. Agent-Driven Development** | Strong (with a gap) | Sync engine is fully agent-testable via `Decider`/`DeathmatchDecider` traits (`runner.rs:60-69`, `deathmatch.rs:35-44`), deterministic seeds, 300-game stress test; `protocol` is a clean narrow waist for bots. | **Closed (2026-06-02):** the protocol bot harness (`bot-harness/`) and terminal client (`tui-client/`) are now workspace crates, and the Claude-as-player harness ships in `agent-harness/`. |
| **III. Start Simple, Scale Later** | Strong | Single binary; anonymous token auth; invite codes + simple 4-player matchmaking with group fill; embedded config; one fixed game mode; persistence is post-game-only and optional. | **F4 closed:** persistence + timeless replays are now wired on the live path, and degrade to a clean no-op with no database configured. |
| **IV. Playtest-Driven Balance** | Strong (with a gap) | Every tunable is externalised to `content.toml` and tagged **[needs playtesting]** in code (`content/effect.rs:60-62`, `content/modifier.rs:36-41`); `round_explosions_total` tracks the 30–40 % target. | **Closed (2026-06-02):** the `bot-harness/` seeded batch runner now runs thousands of games for at-scale balance validation. |
| **Typestate (accepted deviation)** | Documented | Phase/round transitions use `Option<RoundEnd>` + `is_open()` rather than compile-time typestate (`round.rs:82-133`). | This is the previously-accepted deviation in the project history. |
