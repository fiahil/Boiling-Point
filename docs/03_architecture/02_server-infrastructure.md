# Boiling Point — Server Infrastructure Design

> **Living document — infrastructure only.** This page records the server's
> infrastructure design *as built*: topology, room lifecycle, concurrency,
> reconnection, persistence, observability, anti-cheat, and the scaling path.
>
> It began as a broader brainstorm. The game-mechanics sections — a
> sequential-turn state machine, a turn-based protocol, the rumble/glow clue
> system, and per-turn timing — were **cut** once the final design landed; the
> game now runs on **simultaneous hidden waves, blind volatility, shared-loss
> explosions, and points-based scoring**. Those rules live in
> [02_game-design.md](../02_game-design.md); the authoritative server contract
> (wire protocol, round engine, scoring, deck, reconnection, …) lives in the
> resolved capability specs under [`openspec/specs/`](../../openspec/specs/).

The server stack is decided: **Rust (Axum + Tokio), PostgreSQL, MessagePack over
WebSocket** (see [03_tech-stack-exploration.md](03_tech-stack-exploration.md)). The
sections below document how rooms live and die, how game state is owned and mutated,
what happens on disconnect, how state is persisted and observed, and how the system
scales.

---

## 1. Server Topology

**The question:** one binary or many?

| Option | Description | Tradeoff |
|---|---|---|
| **Single binary monolith** | One Axum process: HTTP endpoints for auth/lobby + WebSocket upgrade for game rooms | Simplest to deploy and reason about. Vertical scaling only. Good enough for thousands of concurrent rooms on one box |
| **Logical separation, single binary** | Same binary but internal modules cleanly split: `lobby`, `matchmaking`, `room_engine`, `persistence`. Could be split later along these seams | Best of both — deploy simplicity now, split option later |
| **Split binaries: lobby + game-server** | Lobby handles HTTP/auth/matchmaking, hands off a "room ticket" to a game-server process that only runs WebSocket rooms | Enables independent scaling. Adds inter-process coordination |

**Decision:** logical separation within a single binary (Constitution §III — start
simple, design the seam). The game's scale (4 players per room, rooms lasting minutes)
means a single process handles thousands of concurrent games. The internal module
boundaries are drawn so a future split is a deployment change, not a rewrite.

```
src/
├── main.rs            # Axum router setup, config
├── lobby-matchmaking/ # queue management and room creation
├── game/              # game state machine, room task
├── protocol/          # message types (shared `protocol` crate)
├── persistence/       # PostgreSQL queries
└── observability/     # metrics, tracing setup
```

---

## 2. Room Lifecycle

### Creation

| Option | Description | When |
|---|---|---|
| **Invite-link only** | Creator gets a short code (e.g., `BREW-7K3F`), shares it. Others join via code | MVP — no matchmaking server needed |
| **Matchmaking queue** | Players enqueue. Server assembles groups of four and creates rooms | When player base justifies it |
| **Both** | Ship invite links first, add matchmaking later | Phased approach |

**Decision:** both. Players group up via an invite code, and a partial table fills its
empty seats from the matchmaking queue; the game **starts automatically when the fourth
seat fills**. Tables are **always 4 players** — no hosts, no per-room settings. (The
social/standings layer that sits above a game is the *group*; see the
`lobby-and-matchmaking` and `group-standings` specs.)

**Room ID scheme:** short human-readable codes for invite links (`BREW-7K3F`), UUIDs
internally for storage and logging.

### Lifecycle Diagram

```
  Table forms (invite code, or matchmaking fills empty seats)
         │
         ▼
    ┌──────────┐   idle timeout (5 min, never fills)
    │   Idle   │──────────────────────────► Room destroyed
    │ (lobby)  │
    └────┬─────┘
         │ 4th seat fills → game starts automatically
         ▼
    ┌────────────────────────────┐
    │          Playing           │   rounds → hidden waves → depile →
    │   (server-owned game loop)  │   reveal → score, looping per round
    └────┬───────────────────────┘   (mechanics: see 02_game-design.md)
         │ final round scored
         ▼
    ┌──────────┐
    │ GameOver │──► persist results ──► Room destroyed
    └──────────┘
```

### Cleanup Rules

- **Idle timeout:** room sitting in `Idle` for 5 minutes without filling → destroyed.
- **Disconnect grace:** if all players disconnect mid-game → hold the room for 60
  seconds, then destroy.
- **Post-game:** after `GameOver` + persistence write → the room task terminates.

---

## 3. Concurrency Architecture

### Channel Topology

```
                         ┌──────────────────┐
                         │   Axum Router    │
                         │  (HTTP + WS)     │
                         └──┬────┬────┬─────┘
                            │    │    │
                     WS upgrade  │    WS upgrade
                            │    │    │
                     ┌──────▼┐ ┌─▼───┐ ┌▼───────┐
                     │Player A│ │Pl. B│ │Player C│   per-connection
                     │WS Task │ │Task │ │WS Task │   Tokio tasks
                     └──┬──▲─┘ └┬──▲─┘ └─┬──▲───┘
                        │  │    │  │      │  │
              send(Act) │  │    │  │      │  │ recv(Event)
                 (mpsc) │  │    │  │      │  │ (per-player)
                        │  │    │  │      │  │
                     ┌──▼──┴────▼──┴──────▼──┴──┐
                     │       Room Task          │
                     │   (game state owner)     │
                     │                          │
                     │  mpsc::Receiver<Action>  │
                     │  per-player event senders │
                     └──────────────────────────┘
```

**Each player WebSocket connection** is a separate Tokio task that:
1. reads from the WebSocket → deserializes → sends a `PlayerAction` into the room's
   `mpsc::Sender`;
2. receives events on its own channel → serializes → writes to the WebSocket.

**The room task** is the sole owner of game state. It:
1. reads from its `mpsc::Receiver` (player actions);
2. validates and applies state transitions;
3. fans events out to the per-player senders (audience-scoped — see §7 and the
   `wire-protocol` spec);
4. manages wave/phase timers via `tokio::time`.

### Room Registry

How does a joining player find their room's `mpsc::Sender`?

| Option | Tradeoff |
|---|---|
| `DashMap<RoomCode, RoomHandle>` | Concurrent hashmap, lock-free reads. Simple. Contention only on create/destroy (rare) |
| `RwLock<HashMap<...>>` | Simpler, but writer blocks all readers during room create/destroy |
| Lobby actor task with its own `mpsc` channel | Clean encapsulation, but adds a hop and a mailbox |

**Decision:** `DashMap`. A `RoomHandle` carries the room's `mpsc::Sender<PlayerAction>`
and metadata (player count, phase, created_at).

### Backpressure

If a player's outbound channel lags (slow client, network stall), missing a state
event breaks that client's view.

**Decision:** a **bounded `mpsc` per player** rather than a shared broadcast bus —
per-player flow control, and a slow client is force-disconnected (it receives a full
`StateSnapshot` on reconnect, §4) instead of silently dropping events for everyone.
Game events are small and infrequent (tens to a few hundred per game), so buffers are
cheap.

---

## 4. Reconnection & Disconnect

### Scenarios

| Scenario | Handling | Notes |
|---|---|---|
| **Player disconnects mid-wave** | The wave timer keeps running. If they have not committed for the current wave, they **auto-pass** (commit nothing) when it expires. Seat held for 60s | Indistinguishable from a slow player until the grace period expires |
| **Player disconnects between waves/phases** | Seat held; any acks are auto-triggered by the timer | Game doesn't stall |
| **Player reconnects within 60s** | Send a `StateSnapshot` with the full current state (scoped to what they may know) | Seamless rejoin |
| **Player gone > 60s** | Mark as abandoned. Auto-pass all remaining waves. Score still tracked | Game continues with the remaining players |
| **All players disconnect** | Hold the room for 60s, then destroy | No point keeping an empty room |

### State Snapshot Contents

Sent to a reconnecting player — everything they're allowed to know, and nothing they
aren't (the boiling point and opponents' hands never ride the wire). Illustrative; the
authoritative shape is in the `reconnection` and `wire-protocol` specs:

```rust
struct StateSnapshot {
    phase: Phase,                              // current phase (no hidden data)
    your_hand: Vec<Card>,                      // private — only the reconnecting player
    scores: HashMap<PlayerId, i32>,
    round_number: u8,
    cauldron_card_count: u8,                   // public count only, not identities
    contributions: HashMap<PlayerId, u8>,      // public per-player contribution counts
    players: Vec<PlayerInfo>,                  // name, color, connected status
    wave_deadline_ms: u32,                     // time left in the current wave
}
```

---

## 5. Persistence Strategy

### What Gets Persisted and When

| Strategy | Persists | Recovery | Complexity |
|---|---|---|---|
| **Post-game only** | Final scores, match history, player stats | No crash recovery — in-flight games are lost | Minimal. A few writes per completed game |
| **Checkpoint per round** | Round results after each scoring phase | Resume from the last completed round | Moderate. ~5 writes per game |
| **Full event sourcing** | Every action and event | Full replay from any point | High. Many writes, replay logic needed |

**Decision:** post-game writes for match history and stats; a **replay record** is
written alongside so completed games can be played back (see the `match-replays` and
`persistence-and-observability` specs). Games are short (5–10 minutes); losing an
in-flight game to a crash is annoying, not catastrophic, so live checkpointing is not
worth its cost yet.

### Schema Sketch

Illustrative — the authoritative schema lives in the `persistence-and-observability`
spec. (Player rating/`elo` is a v2 concern; see [05_roadmap.md](../05_roadmap.md).)

Two tables: anonymous `players`, and a single consolidated `game_replays` row per
completed game. The game row carries queryable metadata + denormalized `stats_*`
summary columns + the MessagePack replay payload. Per-round detail is not stored as
rows — it is recoverable by reconstructing the replay (seed + action log re-run the
pinned engine). The payload additionally carries a timestamped log of every raw input
players sent (commits, passes, lock-ins, emotes), so playback can show pacing and emotes.

```sql
CREATE TABLE players (
    id           UUID PRIMARY KEY,
    display_name TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE game_replays (
    game_id             UUID PRIMARY KEY,
    started_at          TIMESTAMPTZ NOT NULL,
    ended_at            TIMESTAMPTZ NOT NULL,
    player_ids          UUID[]      NOT NULL,           -- seating order
    winner_ids          UUID[],                         -- NULL = no winner; array for ties
    scores              JSONB       NOT NULL,           -- per-player breakdown
    stats_round_count   SMALLINT    NOT NULL,
    stats_player_count  SMALLINT    NOT NULL,
    stats_explosions    SMALLINT    NOT NULL,
    stats_cards_played  INT         NOT NULL,
    stats_high_score    INT         NOT NULL,
    stats_low_score     INT         NOT NULL,
    stats_deathmatch    BOOLEAN     NOT NULL,
    payload             BYTEA       NOT NULL,           -- raw MessagePack replay body
    format_version      SMALLINT    NOT NULL,
    engine_version      SMALLINT    NOT NULL,
    config_fingerprint  TEXT        NOT NULL,
    integrity_hash      TEXT        NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### Auth for MVP

Anonymous with a session token: a player picks a display name and gets a UUID + session
token. No email/password. OAuth (Google, Discord) for cross-device persistence is a v2
item ([05_roadmap.md](../05_roadmap.md)).

---

## 6. Observability

### Balance metrics — defined once (`boom-balance-metrics`)

Every v2 balance metric is defined **exactly once**, in
`server/src/observability/balance_metrics.rs` — id, formula over v2 engine
events, unit, and its `[needs playtesting]` target seeded from the
[decision log](../06_boom2/02_toward-a-v2-core.md). The live pipeline (the
Prometheus emitters and the admin projection's unsampled aggregates) and the
benchmarking suite's balance studies (`boom2-benchmarking`) evaluate the **same
definitions**, so live play and harness runs compare directly. The v1 figures
(explosion rate vs ~30–40%, cards per round, dominant-colour rate, reshuffle
frequency) retired with the v1 core; their historical Prometheus series stay in
storage, unqueried.

| Metric (definition id) | Unit | Target (all `[needs playtesting]`) |
|---|---|---|
| `boom_rate` | ratio | ~45% (harness-confirmed 44.8% at BP 31–43) |
| `freeze_rate` | ratio | 0% — rounds must not freeze |
| `detonators_per_boom` | per boom | — |
| `fold_rate` | ratio | — |
| `wave_depth` | waves/round | — |
| `wave_duration_seconds` | seconds | — |
| `round_duration_seconds` | seconds | ~150 s |
| `game_duration_seconds` | seconds | 900–1080 s (15–18 min) |
| `spell_cast_rate` (+ per-spell) | casts/round | — |
| `wave_timeout_rate` | ratio | — |
| `reconnection_rate` | per game | — |

Fleet/ops figures carry over from v1 with unchanged identity: `groups_active`,
`groups_created_total`, `games_active`, `games_started_total`,
`games_completed_total`, `players_connected`, `waves_total`,
`wave_timeouts_total`, `rounds_total`, `player_reconnects_total`, and the
duration histograms.

The balance metrics are the Principle IV signal; the operator-facing dashboard
that consumes them lives inside the **admin command center**
(`admin-command-center` / `balance-dashboard` specs, described in
[`ops/README.md`](../../ops/README.md)) — embedded Grafana renders the
Prometheus series, and the projection-backed cards render the same definition
ids, all behind admin auth.

### Stack

- **Structured logging:** `tracing` + `tracing-subscriber` (JSON output) — the
  ecosystem standard for Tokio/Axum.
- **Metrics:** the `metrics` crate with a Prometheus exporter, emitting only the
  series named in `balance_metrics::series`.
- **Tracing spans:** the v2 span tree (group → game → round → wave →
  commit/spell-cast/resolve, with the round's depile/score) is a first-class
  contract — it is *also* the admin read model and the in-process balance
  aggregator. See [04_span-schema-contract.md](04_span-schema-contract.md) and
  the `otel-span-pipeline` / `admin-span-projection` specs (rebased on v2 by
  change `boom2-observability`).
- **Distributed tracing:** not needed for a single-binary monolith; add Jaeger/Tempo if
  services ever split.

**Decision:** `tracing` + `metrics` + Prometheus, structured JSON logs to stdout.

---

## 7. Anti-Cheat & Validation

The server validates **every** action before applying it (Constitution §I); invalid
actions get an `Error` reply and **no state change**.

**On a wave commit (play/change/pass):**
1. Is the game in a phase that accepts commits?
2. Is the player a seated participant in this room?
3. Do they actually hold the card(s) they are committing?
4. Have they already locked a commit for this wave that may not be changed?

**On lobby actions (join/leave):**
1. Does the room exist and have a free seat (join)?
2. Is the actor a member of the room (leave)?

### What's Impossible by Design

| Cheat | Why it can't happen |
|---|---|
| See other players' cards | The server never sends them |
| Know the boiling point | The server never sends it (except the private Peek effect) |
| Play a card you don't hold | The server checks the hand |
| Commit outside the wave window | The server ignores it |
| Modify the cauldron directly | The server is the only writer |
| Inflate your score | The server computes all scores |

### What's Still Possible

- **Collusion:** two players sharing information out-of-band. Unpreventable in a social
  game; not worth engineering against.
- **Multiple accounts:** one person on two seats. Mitigate with rate limiting + account
  verification (later).

### Rate Limiting

Cap inbound actions per player (no legitimate reason to flood); drop excess silently.

---

## 8. Scaling Path

### Back-of-Envelope Capacity

- Each room: 4 players, ~2KB of game state, one Tokio task.
- Memory per room: ~50KB (state + channel buffers + task overhead).
- 4GB allocated to rooms → **~80,000 concurrent rooms**.
- Messages per room: tens to a few hundred per game.
- Thousands of concurrent rooms → a few thousand messages/second. Trivial for Tokio.

**A single server handles tens of thousands of concurrent players comfortably.**

### Scaling Stages

| Stage | Approach | Trigger |
|---|---|---|
| **Single server** | One binary, one box | 0 – ~10K concurrent players |
| **Vertical** | Bigger box | ~10K – 50K |
| **Horizontal** | Split lobby from game servers; lobby assigns rooms to the least-loaded game server | ~50K+ |
| **Regional** | Game servers in multiple regions; route players to the nearest | When latency matters globally |

### Session Stickiness

WebSocket connections are inherently sticky (long-lived TCP). The challenge is
reconnection — a reconnecting player must reach the same game server hosting their room.
Options when the time comes: a room-to-server map in Redis/lobby, consistent hashing on
room ID, or a WebSocket-aware load balancer.

**Decision:** single server; revisit at ~10K+ concurrent players.

---

## 9. Open Questions — and Where They Landed

The brainstorm raised cross-cutting questions; most are now resolved by shipped specs.

| Question | Status |
|---|---|
| Room configuration (round count, timers, threshold range) | **Resolved** — no per-room settings; balance lives in `server/content.toml`, validated at startup (`game-content-config`) |
| Card deck composition — hardcoded, config, or DB? | **Resolved** — content config (`game-content-config`); editable without recompiling |
| Replay / spectating | **Resolved** — post-game replays (`match-replays`); live spectating remains deferred |
| Chat moderation | **Resolved** — preset emotes only, no free-text chat (`table-talk`) |
| In-game bots to fill empty seats | **Partially** — matchmaking fills seats from the queue (`lobby-and-matchmaking`); the bot/agent harnesses fill seats for testing |
| FFA rating system (TrueSkill / Weng-Lin, not Elo) | **Deferred to v2** ([05_roadmap.md](../05_roadmap.md)) |
| Live spectator mode (what watchers see, anti-screen-share delay) | **Open / deferred** — affects protocol and channel topology |

---

## 10. Summary — Decisions at a Glance

| Topic | Decision | Revisit When |
|---|---|---|
| Topology | Single binary, logically separated modules | Need independent scaling |
| Room creation | Invite codes + matchmaking fill; always 4, auto-start, no hosts | — |
| Concurrency | `mpsc` inbound + per-player outbound, `DashMap` registry | Outbound lag becomes an issue |
| Reconnection | Auto-pass + hold seat 60s + full state snapshot on rejoin | Need bot replacement |
| Persistence | Post-game writes + replay record, anonymous session auth | Need crash recovery |
| Observability | `tracing` + `metrics` + Prometheus; span tree is the read model | Distributed tracing for multi-service |
| Anti-cheat | Server-authoritative, validate every action, rate limit | Collusion / multi-account problems |
| Scaling | Single server | 10K+ concurrent players |
