## Context

Today the lobby model is one-room-one-game: `lobby/room.rs` spawns a task that runs a
single `session::run_game` and the room self-deregisters at `GameOver`
(`lobby/registry.rs`). "Room" terminology is pervasive (~35–50 identifiers across
`protocol/` and `server/`). The playtest ask is twofold — reword to "group", and make a
group outlive a single game so a table can replay together. Exploration surfaced a third,
entangled layer: the WebSocket is welded to one room for its whole life
(`transport.rs::handle_socket` runs `handle_entry` once, binds a single `room_tx`, and
forwards to it), so the socket dies *as a side effect* when the room task ends at
`GameOver`. Outliving a game therefore has two layers — the group must persist (server
lifecycle) **and** the connection must persist independently of any one group
(transport). This is the kind of breaking protocol change OpenSpec exists to stage; this
doc captures intent, not committed code.

## Goals / Non-Goals

**Goals:**
- A clean room→group rename across the wire and server, in one breaking version bump.
- A group that persists across games (play-again without re-queuing), keeping its code
  and roster between games.
- A connection that persists across *groups*: one authenticated socket can join a group,
  leave back to a menu, and join another — never torn down because a game or group ended.

**Non-Goals:**
- Persistence/replays (separate change), accounts/profiles (roadmap), or matchmaking
  policy changes.
- Any change to in-game rules, scoring, or the round engine.
- Implementing this now — it is authored for later scheduling.

## Decisions

- **D1 — One breaking rename, version-bumped.** Do the rename atomically and bump
  `PROTOCOL_VERSION`; all clients (TUI, agent-harness TS mirror, bot-harness) update in
  lockstep. Keep the `BREW-XXXX` code format to minimize churn and preserve muscle
  memory.
- **D2 — Separate group lifetime from game lifetime.** Introduce a group state machine:
  `Lobby → InGame(game) → Lobby (results shown) → …`. A group owns the registry entry,
  the code, the roster, and the seat→player mapping; a *game* is a bounded session the
  group runs. `GameOver` returns players to the group lobby instead of destroying it.
  (The socket carrying those players is itself durable — see D5.)
- **D3 — Play-again / leave flow.** From the post-game group lobby, players choose
  "play again" (re-deal with the same table) or leave (freeing the seat). The
  hostless/auto-start rule generalizes: a game starts when the group holds 4 ready
  players. Idle cleanup now reclaims a *group* that sits empty/idle, not a one-shot room.
- **D4 — Forward note for persistence.** A persistent group implies one group may
  produce several `game_id`s over its life; the `persistence-and-replays` schema should
  expect a nullable group/session linkage when this lands (not built here).
- **D5 — The connection is a durable session, not a per-group bridge.** Today
  `handle_socket` runs `handle_entry` exactly once, binds the socket to a single
  `room_tx`, and forwards every later message to it; the socket dies when that room's
  task ends. Reshape the connection into a long-lived **session** that owns the
  authenticated `PlayerId` and a *current binding* `Option<group_tx>`, acting as a small
  router:
  - Version is negotiated and identity authenticated **once per connection** (the first
    message).
  - Entry messages (`CreateGroup`/`JoinGroup`/`EnqueueMatch`) **set** the binding;
    table/game messages (`CommitCard`/`CommitPass`/`LockIn`/`Emote`/`Heartbeat`) are
    **forwarded** to it. With no binding, `Heartbeat` is serviced locally and table
    actions are rejected.
  - A new `LeaveGroup` **clears** the binding and returns the connection to an unbound
    **menu** state with the socket still open; from there the player may bind to a
    different group — all on one socket.
  - The connection closes only on transport drop / client close — never because a game or
    group ended.

  This nests the three lifetimes: **connection ⊇ group ⊇ game** (D5 ⊇ D2 ⊇ a single
  `run_game`). Two real sockets (a separate "game connection") were rejected: the usual
  drivers — horizontal scaling, a low-latency game transport, separate trust domains —
  don't apply to a turn-based card game on a single server (Constitution §III), and the
  router *is* the seam where a split-out game transport could later attach.
- **D6 — Durable identity is the session's prerequisite.** A session connection presumes
  the client replays its `session_token` so a dropped-and-reconnected socket resolves to
  the same `PlayerId`. The server's `SessionStore` already does token→identity, but the
  TUI sends `session_token: None` everywhere — so reconnection cannot reclaim a held seat
  today and the reconnect overlay is effectively cosmetic. Wire the client to persist the
  minted token and replay it on every entry; this also makes the existing `reconnection`
  grace/snapshot path functional end-to-end.

## Risks / Trade-offs

- **[Risk] Breaking the wire affects three clients at once** → mitigated by the lockstep
  version bump and the agent-harness's ts-rs generation seam (regenerate, don't
  hand-edit); land protocol + all clients in one change.
- **[Risk] Lifecycle bugs (a group leaking when everyone leaves mid-results)** → define
  explicit transitions and an idle/empty-group reaper; cover with the existing
  transport integration-test style (a group plays two back-to-back games).
- **[Risk] Connection-router state bugs** — a menu (zero-group) connection must still be
  heartbeated or it is reaped by the idle `conn_timeout`; an entry message that arrives
  while already bound, or a table action that arrives while unbound, must be handled
  (rebind vs. error) rather than mis-routed → define the router's accepted transitions
  explicitly and cover with a transport test that joins, plays, leaves to the menu, and
  joins a *second* group on one socket.
- **[Trade-off] Bigger blast radius than a pure rename** → justified: the rename and the
  persistent-group behavior are the same conceptual shift ("group, not room"); splitting
  them would mean two breaking passes over the same identifiers.

## Migration Plan

Single breaking release: rename across `protocol/` + `server/` + all clients together,
bump `PROTOCOL_VERSION`, regenerate the agent-harness types. No data migration (no
persisted room/group rows today). Sequence when scheduled: (1) protocol rename +
version bump (incl. the new `LeaveGroup`/`LeftGroup` messages), (2) server identifiers +
group state machine, (2b) reshape `transport.rs` into a session router (rebindable
binding, menu state), (3) clients + session-token persistence + main-menu/leave +
play-again flow, (4) reword game-design §14 "Rooms"→"Groups".

## Open Questions

- Does a group persist its roster across games by default, or re-confirm "ready" each
  game? (Lean: show results, then each seat opts "play again"; a seat that doesn't
  opt-in within the idle window is freed.)
- Should a persistent group expose its game history to members between games (ties into
  `persistence-and-replays`)? Defer until replay playback exists.
- Does `LeaveGroup` need an explicit `LeftGroup` ack, or may the client transition to the
  menu optimistically? (Lean: explicit ack — the server is authoritative and the menu
  must reflect the freed seat.)
