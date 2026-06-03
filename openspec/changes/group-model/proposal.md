## Why

Playtest wording feedback: players "do not create a room or join one — they **join a
group** and then go on **games** together." Two things follow. (1) The pervasive "room"
vocabulary should become "group" across the protocol, server, and clients. (2) More
substantively, a *room* today **is** a single game that closes at `GameOver`; a *group*
should **persist across games** so the same table can play again without re-queuing.

This change is **proposal only** — it captures the design and the breaking-rename scope.
No code lands from it yet (per the maintainer). The TUI's player-facing wording was
already updated to "group" as a stopgap; the protocol/server identifiers are unchanged
until this change is scheduled.

## What Changes

- **BREAKING — rename room → group** across the wire and server: `RoomCode`→`GroupCode`,
  `CreateRoom`/`JoinRoom`/`RoomJoined` → `CreateGroup`/`JoinGroup`/`GroupJoined`,
  `ErrorCode::UnknownRoom` → `UnknownGroup`, and the server internals
  (`RoomRegistry`/`RoomCommand`/`RoomHandle`, `create_room`/`join_room`, the
  `rooms_*` metrics, ~35–50 identifiers). The invite-code format (`BREW-XXXX`) is
  unchanged.
- **Persistent group lifecycle.** A group survives across games: lobby → game → back to
  the group lobby → "play again", decoupling the group's lifetime from a single game's.
  The group keeps its code and roster between games; a finished game returns players to
  the group rather than tearing the group down.
- **Clients** (`tui-client/`, `agent-harness/`, `bot-harness/`) update to the renamed
  messages and gain a between-games "play again / leave" flow.

## Capabilities

### New Capabilities
<!-- none — this reshapes existing capabilities rather than adding new ones. -->

### Modified Capabilities
- `lobby-and-matchmaking`: room→group rename **(BREAKING)** and a new persistent-group
  lifecycle that hosts multiple sequential games.
- `wire-protocol`: the entry/handshake messages and the invite-code type are renamed
  room→group **(BREAKING)**.

## Impact

- **BREAKING wire change** — bumps `PROTOCOL_VERSION`; every client (TUI, agent-harness
  TS mirror, bot-harness) must update in lockstep.
- **Code:** `protocol/src/{client,server,ids}.rs`; `server/src/lobby/{room,registry,codes,matchmaking,session}.rs`, `transport.rs`, `observability.rs`, `session.rs`; `tui-client/`, `bot-harness/`, `agent-harness/`.
- **Docs/specs:** this change's deltas update `lobby-and-matchmaking` and `wire-protocol`; the game-design "Rooms" reference (§14) should be reworded to "Groups" when scheduled.
- **No persistence/replay impact** (that's `persistence-and-replays`), though a persistent group implies a group may own several `game_id`s over its life — a forward-looking note for that schema.
