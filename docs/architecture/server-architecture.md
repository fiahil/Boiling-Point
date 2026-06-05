# Boiling Point — Server Architecture

> **STATUS: Partially superseded.** This brainstorm predates the final game
> design. Its **game-mechanics** sections are obsolete — §3 (sequential-turn
> state machine), §4 (turn-based protocol), §6 (rumble/glow clue system), and
> §12 (per-turn timing) were replaced by simultaneous hidden waves, blind
> volatility, shared-loss explosions, and points-based scoring. The
> **infrastructure** sections (§1 topology, §2 rooms, §5 concurrency, §7
> reconnection, §8 persistence, §9 observability, §10 anti-cheat, §11 scaling)
> remain valid and were implemented. The authoritative server contract now lives
> in `openspec/changes/server-release-1/specs/`.

The server stack is decided: **Rust (Axum + Tokio), PostgreSQL, MessagePack over WebSocket** (see [tech-stack-exploration.md](tech-stack-exploration.md)). This document explores the open architectural questions — how rooms live and die, what messages flow on the wire, how secrets are managed, what happens on disconnect, and more.

Nothing here is final. Each section presents options with tradeoffs and a "start here" recommendation.

---

## 1. Server Topology

**The question:** one binary or many?

| Option | Description | Tradeoff |
|---|---|---|
| **Single binary monolith** | One Axum process: HTTP endpoints for auth/lobby + WebSocket upgrade for game rooms | Simplest to deploy and reason about. Vertical scaling only. Good enough for thousands of concurrent rooms on one box |
| **Logical separation, single binary** | Same binary but internal modules cleanly split: `lobby`, `matchmaking`, `room_engine`, `persistence`. Could be split later along these seams | Best of both — deploy simplicity now, split option later |
| **Split binaries: lobby + game-server** | Lobby handles HTTP/auth/matchmaking, hands off a "room ticket" to a game-server process that only runs WebSocket rooms | Enables independent scaling. Adds inter-process coordination |

**Start here:** Logical separation within a single binary. The game's scale (3–4 players per room, rooms lasting minutes) means a single process handles thousands of concurrent games. Design the internal module boundaries so a future split is a deployment change, not a rewrite.

**Approved**

```
src/
├── main.rs            # Axum router setup, config
├── lobby-matchmaking/ # queue management and room creation
├── game/              # game state machine, room task
├── protocol/          # message types (or in shared crate)
├── persistence/       # PostgreSQL queries
└── observability/     # metrics, tracing setup
```

---

## 2. Room Lifecycle

### Creation

| Option | Description | When |
|---|---|---|
| **Invite-link only** | Creator gets a short code (e.g., `BREW-7K3F`), shares it. Others join via code | MVP — no matchmaking server needed |
| **Matchmaking queue** | Players enqueue with preferences (3p or 4p). Server assembles groups and creates rooms | When player base justifies it |
| **Both** | Ship invite links first, add matchmaking later | Phased approach |

**Room ID scheme:** Short human-readable codes for invite links (`BREW-7K3F`), UUIDs internally for storage and logging.

**MY INPUT**: let's do both, invite link to group up and then matchmaking (launch automatically if 4 players). no settings, no hosts, always 4 players. 

### Lifecycle Diagram

```
  Player creates room
         │
         ▼
    ┌──────────┐   timeout (5 min)
    │   Idle   │──────────────────────► Room destroyed
    │ (lobby)  │                         (no game started)
    └────┬─────┘
         │ Host sends StartGame
         │ + min 3 players
         ▼
    ┌──────────┐
    │ Drafting │◄─────────────────────┐
    └────┬─────┘                      │
         │                            │
         ▼                            │
    ┌──────────┐                      │
    │ Playing  │                      │
    └────┬─────┘                      │
         │ all pass / boom            │
         ▼                            │
    ┌──────────┐                      │
    │Revealing │                      │
    └────┬─────┘                      │
         │                            │
         ▼                            │
    ┌──────────┐                      │
    │Resolving │                      │
    └────┬─────┘                      │
         │                            │
         ▼                            │
    ┌──────────┐   not final round    │
    │ Scoring  │──────────────────────┘
    └────┬─────┘
         │ final round
         ▼
    ┌──────────┐
    │ GameOver │──► Persist results ──► Room destroyed
    └──────────┘
```

### Cleanup Rules

- **Idle timeout:** Room sitting in `Idle` for 5 minutes with no game start → destroyed
- **Disconnect grace:** If all players disconnect mid-game → hold room for 60 seconds, then destroy
- **Post-game:** After `GameOver` + persistence write → room task terminates

---

## 3. Detailed State Machine

### Rust Enum Sketch

```rust
enum GamePhase {
    Idle { players: Vec<Player>, host: PlayerId },
    Drafting { hands: HashMap<PlayerId, Vec<Card>>, threshold: u8 },
    Playing {
        cauldron: Vec<(PlayerId, Card)>,
        volatility_total: u8,
        threshold: u8,
        turn_order: Vec<PlayerId>,
        active_player_idx: usize,
        passed_since_last_play: HashSet<PlayerId>,
    },
    Revealing { cauldron: Vec<(PlayerId, Card)> },
    Resolving { outcome: RoundOutcome },
    Scoring { scores: HashMap<PlayerId, i32>, round: u8 },
}

enum RoundOutcome {
    Explosion { last_player: PlayerId, threshold: u8, total: u8 },
    PotionBrewed { color_counts: HashMap<Color, u8>, scoring: ScoringOutcome },
}

enum ScoringOutcome {
    Dominated { player: PlayerId },
    Split { players: Vec<PlayerId> },
    Cooperative,
}
```

### Phase Details

**Idle**
- Track connected players and their assigned colors
- Host can send `StartGame` when 3–4 players present
- Transition: `StartGame` received + min players met → `Drafting`

**Drafting**
- Generate hidden threshold for the round (random 8–14)
- Deal 5 cards to each player from the shared deck (plus any cards kept from previous rounds)
- Send each player their hand via private `YourHand` message
- Initialize cauldron (empty, volatility = 0)
- Transition: all players acknowledge cards (or 5-second auto-ack timer) → `Playing`

**Playing** (the core loop — needs the most detail)
- **Turn order:** Clockwise from a rotating starting player (shifts each round)
- **Active player can:** `PlayCard(card_id)` or `Pass`
- **On `PlayCard`:**
  1. Validate card is in player's hand
  2. Remove from hand, add face-down to cauldron
  3. Increment volatility total (server-side only)
  4. Check threshold: if exceeded → transition to `Revealing` (boom path)
  5. Check clue thresholds:
     - `total >= threshold * 0.50` and rumble not yet sent → broadcast `CauldronClue::Rumble`
     - `total >= threshold * 0.75` and glow not yet sent → broadcast `CauldronClue::Glow`
  6. Clear `passed_since_last_play` set (a new card was played, so the pass cycle resets)
  7. Advance to next player
- **On `Pass`:**
  1. Add player to `passed_since_last_play`
  2. If `passed_since_last_play` contains ALL players → transition to `Revealing` (safe brew path)
  3. Otherwise advance to next player
- **Turn timer:** 20 seconds per turn. On timeout → auto-pass
- **Special effects:** Resolve immediately when played:
  - *Peek:* send private message with exact threshold value
  - *Swap:* remove last card from cauldron, place this one instead. Recalculate volatility
  - *Dual-color:* card counts as both player's color and one other
  - *Reduce volatility:* subtract 2 from volatility total (but card scores nothing)
- Transition: all pass consecutively → `Revealing` / threshold exceeded → `Revealing`

**Key question — "all pass consecutively":** This means a full rotation where every player passes without anyone playing a card in between. If Player A passes, then Player B plays a card, Player A's pass is "reset" — they get to decide again when their turn comes around.

**Revealing**
- All face-down cards are flipped — broadcast full cauldron contents (color, volatility, effects, who played what)
- This phase exists for dramatic effect
- Transition: 3–5 second timer (or all players acknowledge) → `Resolving`

**Resolving**
- **Boom path:** Threshold exceeded. Identify last player who added a card. Calculate penalties (configurable — e.g., -5 for last player, -1 for everyone else). Broadcast explosion event with full details
- **Safe brew path:** Count colors in cauldron. Apply majority rules:
  - One color ≥ 3 cards → that player gets big score (e.g., +10), others get 0
  - Two colors tied for most → those players split (e.g., +5 each)
  - All colors roughly equal → everyone gets moderate score (e.g., +3 each)
  - Absent color → that player gets 0
- Broadcast potion result with scoring breakdown
- Transition: immediate → `Scoring`

**Scoring**
- Apply score changes to persistent leaderboard
- Broadcast updated scores
- Check if this was the final round
- Transition: not final → `Drafting` (next round) / final → `GameOver`

---

## 4. Protocol Design

### ClientMessage (client → server)

| Message | Phase | Payload | Notes |
|---|---|---|---|
| `JoinRoom` | Pre-game | `room_code, player_name` | |
| `LeaveRoom` | Any | — | |
| `StartGame` | Idle | — | Host only |
| `AckCards` | Drafting | — | Player confirms hand received |
| `PlayCard` | Playing | `card_id: u32` | |
| `Pass` | Playing | — | |
| `AckReveal` | Revealing | — | Optional, timer handles it |
| `Heartbeat` | Any | — | Keepalive |
| `ChatMessage` | Any | `text: String` | Table talk |

### ServerMessage (server → client)

| Message | Phase | Audience | Payload |
|---|---|---|---|
| `RoomJoined` | Idle | Private | `room_id, players, your_color, is_host` |
| `PlayerJoined` | Idle | Broadcast | `player_name, player_color` |
| `PlayerLeft` | Any | Broadcast | `player_id` |
| `GameStarting` | Idle → Drafting | Broadcast | `round_count, player_order` |
| `YourHand` | Drafting | **Private** | `cards: Vec<Card>` |
| `RoundStarted` | Drafting | Broadcast | `round_number, starting_player` |
| `YourTurn` | Playing | **Private** | `time_limit_ms, server_timestamp_ms` |
| `CardPlayed` | Playing | Broadcast | `player_id, cards_in_cauldron_count` |
| `CauldronClue` | Playing | Broadcast | `Rumble` or `Glow` |
| `PlayerPassed` | Playing | Broadcast | `player_id` |
| `TurnTimeout` | Playing | Broadcast | `player_id` (auto-passed) |
| `SpecialEffectTriggered` | Playing | Broadcast | `player_id, effect_type` |
| `PeekResult` | Playing | **Private** | `threshold_value` |
| `CardsRevealed` | Revealing | Broadcast | `Vec<(player_id, Card)>` |
| `Explosion` | Resolving | Broadcast | `threshold, total_volatility, last_player, penalties` |
| `PotionBrewed` | Resolving | Broadcast | `color_counts, scoring_outcome, points_awarded` |
| `ScoreUpdate` | Scoring | Broadcast | `scores: HashMap<PlayerId, i32>` |
| `GameOver` | Scoring | Broadcast | `final_scores, winner` |
| `Error` | Any | **Private** | `code, message` |
| `PlayerDisconnected` | Any | Broadcast | `player_id` |
| `PlayerReconnected` | Any | Broadcast | `player_id` |
| `StateSnapshot` | Any | **Private** | Full game state (for reconnection) |
| `Heartbeat` | Any | **Private** | — |

### Design Notes

- **Private vs Broadcast:** Most messages are broadcast. Only `YourHand`, `YourTurn`, `PeekResult`, `Error`, and `StateSnapshot` are private. Could enforce this distinction at the type level with separate enums
- **Message encoding:** `serde` + `rmp-serde` (MessagePack) on the wire. Tag variants with `#[serde(tag = "type")]` for debuggability in JSON fallback mode
- **Versioning:** Not needed for MVP. When needed, add a `protocol_version` field to the initial `JoinRoom`/`RoomJoined` handshake
- **No request-response:** All messages are fire-and-forget. The server broadcasts state changes; the client reacts. If a `PlayCard` is invalid, the server sends an `Error` and doesn't change state

---

## 5. Concurrency Architecture

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
                 (mpsc) │  │    │  │      │  │ (broadcast)
                        │  │    │  │      │  │
                     ┌──▼──┴────▼──┴──────▼──┴──┐
                     │       Room Task          │
                     │   (game state owner)     │
                     │                          │
                     │  mpsc::Receiver<Action>  │
                     │  broadcast::Sender<Event> │
                     └──────────────────────────┘
```

**Each player WebSocket connection** is a separate Tokio task that:
1. Reads from WebSocket → deserializes → sends `PlayerAction` into room's `mpsc::Sender`
2. Subscribes to room's `broadcast::Receiver` → serializes → writes to WebSocket

**The room task** is the sole owner of game state. It:
1. Reads from `mpsc::Receiver` (player actions)
2. Validates and applies state transitions
3. Broadcasts events via `broadcast::Sender`
4. Manages turn timers via `tokio::time::sleep`

### Room Registry

How does a joining player find their room's `mpsc::Sender`?

| Option | Tradeoff |
|---|---|
| `DashMap<RoomCode, RoomHandle>` | Concurrent hashmap, lock-free reads. Simple. Contention only on create/destroy (rare) |
| `RwLock<HashMap<...>>` | Simpler, but writer blocks all readers during room create/destroy |
| Lobby actor task with its own `mpsc` channel | Clean encapsulation, but adds a hop and a mailbox |

**Start here:** `DashMap`. A `RoomHandle` contains the room's `mpsc::Sender<PlayerAction>` and metadata (player count, phase, created_at).

### Backpressure

If the `broadcast` channel lags (slow client, network stall), Tokio's broadcast drops the oldest messages. For a game server, missing a `CardPlayed` event breaks the client's state.

**Options:**
- **Bounded `mpsc` per player** instead of broadcast — more control over per-player flow, slightly more memory
- **Large broadcast buffer** — game events are small and infrequent (~50–100 per round). A buffer of 256 is generous
- **Detect `RecvError::Lagged`** — force-disconnect the slow client with a reconnection prompt, they'll get a `StateSnapshot` on rejoin

**Start here:** Bounded `mpsc` per player

---

## 6. Secret Management

**Core principle: the server never sends information a player shouldn't have.**

This is simpler than most multiplayer games because:
- Cards are face-down for **everyone** — nobody sees anyone's cards until `Revealing`
- The threshold is hidden from **all** players until `Resolving`

### Information Flow by Phase

| Phase | What players know | What's hidden |
|---|---|---|
| Drafting | Their own hand | Other players' hands, threshold |
| Playing | Their own hand, card count in cauldron, clues (rumble/glow), who played/passed | Card identities in cauldron, threshold exact value |
| Revealing | All cards revealed | Threshold (until Resolving) |
| Resolving | Everything | Nothing |

### Clue Computation

```rust
fn check_clues(&self) -> Option<CauldronClue> {
    let ratio = self.volatility_total as f32 / self.threshold as f32;
    if ratio >= 0.75 && !self.glow_sent {
        self.glow_sent = true;
        Some(CauldronClue::Glow)
    } else if ratio >= 0.50 && !self.rumble_sent {
        self.rumble_sent = true;
        Some(CauldronClue::Rumble)
    } else {
        None
    }
}
```

### Peek Effect

Player who plays a "peek" card receives a private `PeekResult { threshold_value: u8 }` message. Only they learn the exact number. Other players see `SpecialEffectTriggered { effect: Peek }` — they know someone peeked but not what they saw.

### Open Game Design Questions

- **Clue precision:** Should clues be exact (50%, 75%) or fuzzy ("somewhere between 40–60%")? Fuzzy adds uncertainty but complexity. Start with exact thresholds, tune via playtesting.
- **Swap effect — blind or informed?** When a player swaps the last ingredient, do they see the card they're replacing? Blind swap is simpler and preserves more hidden info. Informed swap gives a strategic advantage worth the information leak. **Needs playtesting to decide.**

---

## 7. Reconnection & Disconnect

### Scenarios

| Scenario | Handling | Notes |
|---|---|---|
| **Player disconnects during `Playing`** | Turn timer continues. If it's their turn → auto-pass on timeout. Seat held for 60s | Indistinguishable from a slow player until grace period expires |
| **Player disconnects during other phases** | Seat held, their acks are auto-triggered by timer | Game doesn't stall |
| **Player reconnects within 60s** | Send `StateSnapshot` with full current game state | Seamless rejoin |
| **Player gone > 60s** | Mark as abandoned. Auto-pass all future turns. Score still tracked | Game continues 3→2 players |
| **All players disconnect** | Hold room for 60s, then destroy | No point keeping an empty room |
| **Host disconnects in `Idle`** | Migrate host to next player in join order | Room survives |

### State Snapshot Contents

Sent to reconnecting player — everything they're allowed to know:

```rust
struct StateSnapshot {
    phase: GamePhase,         // current phase (without hidden data)
    your_hand: Vec<Card>,
    scores: HashMap<PlayerId, i32>,
    round_number: u8,
    cauldron_card_count: u8,
    clues_revealed: Vec<CauldronClue>,
    players: Vec<PlayerInfo>,  // name, color, connected status
    active_player: PlayerId,
    turn_time_remaining_ms: u32,
    cards_contributed: HashMap<PlayerId, u8>, // contribution tracker
}
```

---

## 8. Persistence Strategy

### What Gets Persisted and When

| Strategy | Persists | Recovery | Complexity |
|---|---|---|---|
| **Post-game only** | Final scores, match history, player stats | No crash recovery — games are lost | Minimal. One write per completed game |
| **Checkpoint per round** | Round results after each `Scoring` phase | Resume from last completed round | Moderate. ~7 writes per game |
| **Full event sourcing** | Every `PlayerAction` and `GameEvent` | Full replay from any point | High. Many writes, replay logic needed |

**Start here:** Post-game only. Games are short (5–10 minutes). Losing a game to a server crash is annoying but not catastrophic. Add checkpoint-per-round if crash recovery matters later. Full event sourcing only if replay/spectating features are wanted.

**Approved**

### Schema Sketch

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

Anonymous with a session token. Player picks a display name, gets a UUID + session token. No email/password. Add OAuth (Google, Discord) later for account persistence across devices.

---

## 9. Observability

### Game-Specific Metrics

| Metric | Type | Why It Matters |
|---|---|---|
| `rooms_active` | Gauge | Capacity planning |
| `rooms_created_total` | Counter | Growth tracking |
| `players_connected` | Gauge | Load indicator |
| `game_duration_seconds` | Histogram | Is the game too long/short? |
| `round_duration_seconds` | Histogram | Are rounds hitting the 60–90s target? |
| `explosion_rate` | Ratio | Game balance — should be ~30–40% |
| `turn_timeout_rate` | Ratio | UX indicator — are timers too short? |
| `ws_message_latency_ms` | Histogram | Player experience |
| `reconnection_rate` | Ratio | Network quality |
| `cards_per_round` | Histogram | Engagement — are players passing too early? |
| `dominant_strategy_rate` | Ratio | Balance — is one color/strategy winning disproportionately? |

### Stack

- **Structured logging:** `tracing` crate + `tracing-subscriber` (JSON output). Already ecosystem standard for Tokio/Axum
- **Metrics:** `metrics` crate with Prometheus exporter (`metrics-exporter-prometheus`)
- **Tracing spans:** WebSocket message receive-to-response, phase transitions, DB writes, room lifecycle events
- **Distributed tracing:** Not needed for single-binary monolith. Add Jaeger/Tempo when/if services split

**Start here:** `tracing` + `metrics` + Prometheus. Structured JSON logs to stdout. Good enough for a long time.

**Approved**

---

## 10. Anti-Cheat & Validation

### Server Validates on Every Action

**On `PlayCard`:**
1. Is it this player's turn?
2. Is the game in `Playing` phase?
3. Does the player actually have this card in their hand?
4. Has the player already passed this cycle?

**On `Pass`:**
1. Is it this player's turn?
2. Is the game in `Playing` phase?

**On `StartGame`:**
1. Is the sender the host?
2. Are there 3–4 players?
3. Is the room in `Idle` phase?

Invalid actions → `Error` message, no state change.

### What's Impossible by Design

| Cheat | Why it can't happen |
|---|---|
| See other players' cards | Server never sends them |
| Know the threshold | Server never sends it (except via Peek) |
| Play a card you don't have | Server checks hand |
| Act out of turn | Server ignores out-of-turn messages |
| Modify the cauldron | Server is the only writer |
| Inflate your score | Server computes all scores |

### What's Still Possible

- **Collusion:** Two players on a voice call sharing hand info. Can't prevent in a social game. Not worth engineering against
- **Multiple accounts:** One person playing two seats. Mitigate with IP rate limiting + account verification (later)
- **Timing analysis:** Could a player infer info from server response times? Negligible risk for this game. Add random delay to responses if ever concerned

### Rate Limiting

Cap at **1 action per 100ms** per player. No legitimate reason to send faster. Drop excess messages silently.

---

## 11. Scaling Path

### Back-of-Envelope Capacity

- Each room: ~4 players, ~2KB game state, one Tokio task
- Memory per room: ~50KB (state + channel buffers + task overhead)
- 4GB allocated to rooms → **~80,000 concurrent rooms**
- Messages per room: ~50–350 per game (10–50 per round × 5–7 rounds)
- 1,000 concurrent rooms × 4,000 players → ~5,000 messages/second. Trivial for Tokio

**A single server handles tens of thousands of concurrent players comfortably.**

### Scaling Stages

| Stage | Approach | Trigger |
|---|---|---|
| **Single server** | One binary, one box | 0 – ~10K concurrent players |
| **Vertical** | Bigger box | ~10K – 50K |
| **Horizontal** | Split lobby from game servers. Lobby assigns rooms to game servers via least-loaded | ~50K+ |
| **Regional** | Game servers in multiple regions, route players to nearest | When latency matters globally |

### Session Stickiness

WebSocket connections are inherently sticky (long-lived TCP). The challenge is reconnection — a reconnecting player must reach the same game server hosting their room.

**Options:** Room-to-server mapping in Redis/lobby, consistent hashing on room ID, or WebSocket-aware load balancer.

**Start here:** Don't think about this. Single server. Revisit when you have 10K+ concurrent players.

---

## 12. Turn Timing

### Per-Turn Timer (Recommended)

Each player gets **20 seconds** when it's their turn. Simple, predictable, keeps the game moving.

Alternative: a round-wide timer (entire round has a budget). More pressure but harder to reason about. Start with per-turn.

### Timeout Behavior

1. Timer expires → **auto-pass**
2. After **2 consecutive auto-passes** → server sends `InactivityWarning` (client shows "Are you still there?")
3. After **3 consecutive auto-passes** → treat as disconnect, apply disconnect handling

### Clock Sync

Server sends `YourTurn { time_limit_ms: 20000, server_timestamp_ms }`. Client starts local countdown adjusted by estimated one-way latency (computed at connection handshake via ping).

For a 20-second timer, the ~100–500ms network discrepancy is negligible. Don't over-engineer this.

### Phase Timers

| Phase | Duration | Trigger to advance early |
|---|---|---|
| Drafting | 5s | All `AckCards` received |
| Revealing | 4s | All `AckReveal` received |
| Resolving | Instant (computed) | Displayed client-side for 3–5s |
| Scoring | 5s | — |

**Not Approved, wait for design updates**

---

## 13. Open Questions

Questions that span multiple sections or need game design input before server work:

- **Spectator mode:** Should rooms support watchers? What do they see? (All public info, possibly delayed by one phase to prevent cheating via screen-sharing.) Affects protocol and channel topology
- **Room configuration:** Configurable settings (round count, timer duration, threshold range)? Set at creation by host?
- **Card deck composition:** Where is the deck defined? Hardcoded? Config file? Database table? Affects Drafting phase and balance iteration speed
- **FFA rating system:** Elo works for 2-player. For 3–4 player FFA, need TrueSkill or Weng-Lin. Affects `players` table schema
- **In-game bots:** The bot-harness runs headless bots for testing. Should the server also support in-game bots to fill empty seats? Different concern — the bot harness tests the server, in-game bots need embedded AI
- **Replay / spectating:** Event sourcing enables replays but costs persistence complexity. Worth it? Could be added later if events are logged to a separate append-only store
- **Chat moderation:** Table talk is part of the game. Text chat? Predefined emotes only? Both? If text chat, any filtering?

---

## 14. Summary — Start Here

| Topic | Start Here | Revisit When |
|---|---|---|
| Topology | Single binary, logically separated modules | Need independent scaling |
| Room creation | Invite links (short codes) | Player base justifies matchmaking |
| State machine | Full 6-phase model as described above | Playtesting reveals phase issues |
| Protocol | MessagePack, enum-tagged messages, private/broadcast split | Need protocol versioning |
| Concurrency | `mpsc` inbound + `broadcast` outbound per room, `DashMap` registry | Broadcast lag becomes an issue |
| Secrets | Never send what players shouldn't know. Clues at exact thresholds | Balance needs fuzzier clues |
| Reconnection | Auto-pass + hold seat 60s + full state snapshot on rejoin | Need bot replacement |
| Persistence | Post-game only, anonymous auth | Need crash recovery or replays |
| Observability | `tracing` + `metrics` + Prometheus | Distributed tracing for multi-service |
| Anti-cheat | Server-authoritative, validate every action, rate limit 100ms | Collusion / multi-account problems |
| Scaling | Single server, don't think about it | 10K+ concurrent players |
| Turn timing | 20s per turn, auto-pass, server timestamps | Playtesting says too fast/slow |
