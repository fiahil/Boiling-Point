## 1. Protocol rename (BREAKING)

- [x] 1.1 Rename `RoomCode`→`GroupCode` (`protocol/src/ids.rs`)
- [x] 1.2 Rename entry/response messages: `CreateRoom`/`JoinRoom`/`RoomJoined` → `CreateGroup`/`JoinGroup`/`GroupJoined`; field `room_code`→`group_code`
- [x] 1.3 Rename `ErrorCode::UnknownRoom`→`UnknownGroup`
- [x] 1.4 Add `LeaveGroup` (client) and `LeftGroup` (server) messages for the menu/leave flow
- [x] 1.5 Bump `PROTOCOL_VERSION`

## 2. Server rename

- [x] 2.1 Rename `RoomRegistry`/`RoomCommand`/`RoomHandle` and `lobby/room.rs`→`lobby/group.rs`
- [x] 2.2 Rename `create_room`/`join_room`/`handle_entry` arms and `lobby/codes.rs` helpers
- [x] 2.3 Rename observability metrics `rooms_*`→`groups_*` (and update the span schema / admin projection)
- [x] 2.4 Update doc comments referencing "room"

## 3. Persistent group lifecycle

- [x] 3.1 Add a group state machine: `Lobby → InGame → Lobby(results) → …`, decoupling group lifetime from a single `run_game`
- [x] 3.2 On `GameOver`, return players to the group lobby instead of deregistering the group
- [x] 3.3 Add a "play again" path (re-deal with the same table) and a "leave" path (free the seat); generalize hostless auto-start to "4 ready players in the group"
- [x] 3.4 Reclaim idle/empty *groups* (replaces idle-room cleanup)

## 4. Session connection (re-bindable transport)

- [x] 4.1 Reshape `transport.rs::handle_socket` into a session router: negotiate version/identity once, hold a `current_binding: Option<group_tx>`, forward table actions to it
- [x] 4.2 On `LeaveGroup`: free the seat in the bound group, clear the binding, reply `LeftGroup`, keep the socket open (menu state)
- [x] 4.3 Allow re-entry — a `CreateGroup`/`JoinGroup`/`EnqueueMatch` on an established session binds to a new group without renegotiating the version
- [x] 4.4 Service `Heartbeat` in the unbound menu state (don't reap a heartbeating menu connection); reject table actions when unbound
- [x] 4.5 Persist & replay `session_token` client-side so identity (and the held-seat reconnection path) survives a socket drop

## 5. Clients (lockstep)

- [x] 5.1 `tui-client/`: renamed messages; a main-menu (unbound) state; between-games "play again / leave to menu" flow; persist the session token
- [x] 5.2 `agent-harness/`: update the hand-mirrored TS protocol (the crate does not derive ts-rs today); add the entry/leave/play-again messages and bump `PROTOCOL_VERSION`
- [x] 5.3 `bot-harness/`: renamed entry messages

## 6. Docs & validation

- [x] 6.1 Reword game-design §14 "Rooms"→"Groups"
- [x] 6.2 Integration test: one group plays two back-to-back games via "play again"
- [x] 6.3 Integration test: one socket joins a group, plays, leaves to the menu, and joins a *second* group (session connection)
- [x] 6.4 `make check` green across the workspace + agent-harness typecheck/tests
