## Context

Today the lobby model is one-room-one-game: `lobby/room.rs` spawns a task that runs a
single `session::run_game` and the room self-deregisters at `GameOver`
(`lobby/registry.rs`). "Room" terminology is pervasive (~35–50 identifiers across
`protocol/` and `server/`). The playtest ask is twofold — reword to "group", and make a
group outlive a single game so a table can replay together. This is the kind of breaking
protocol change OpenSpec exists to stage; this doc captures intent, not committed code.

## Goals / Non-Goals

**Goals:**
- A clean room→group rename across the wire and server, in one breaking version bump.
- A group that persists across games (play-again without re-queuing), keeping its code
  and roster between games.

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
- **D3 — Play-again / leave flow.** From the post-game group lobby, players choose
  "play again" (re-deal with the same table) or leave (freeing the seat). The
  hostless/auto-start rule generalizes: a game starts when the group holds 4 ready
  players. Idle cleanup now reclaims a *group* that sits empty/idle, not a one-shot room.
- **D4 — Forward note for persistence.** A persistent group implies one group may
  produce several `game_id`s over its life; the `persistence-and-replays` schema should
  expect a nullable group/session linkage when this lands (not built here).

## Risks / Trade-offs

- **[Risk] Breaking the wire affects three clients at once** → mitigated by the lockstep
  version bump and the agent-harness's ts-rs generation seam (regenerate, don't
  hand-edit); land protocol + all clients in one change.
- **[Risk] Lifecycle bugs (a group leaking when everyone leaves mid-results)** → define
  explicit transitions and an idle/empty-group reaper; cover with the existing
  transport integration-test style (a group plays two back-to-back games).
- **[Trade-off] Bigger blast radius than a pure rename** → justified: the rename and the
  persistent-group behavior are the same conceptual shift ("group, not room"); splitting
  them would mean two breaking passes over the same identifiers.

## Migration Plan

Single breaking release: rename across `protocol/` + `server/` + all clients together,
bump `PROTOCOL_VERSION`, regenerate the agent-harness types. No data migration (no
persisted room/group rows today). Sequence when scheduled: (1) protocol rename +
version bump, (2) server identifiers + group state machine, (3) clients + play-again
flow, (4) reword game-design §14 "Rooms"→"Groups".

## Open Questions

- Does a group persist its roster across games by default, or re-confirm "ready" each
  game? (Lean: show results, then each seat opts "play again"; a seat that doesn't
  opt-in within the idle window is freed.)
- Should a persistent group expose its game history to members between games (ties into
  `persistence-and-replays`)? Defer until replay playback exists.
