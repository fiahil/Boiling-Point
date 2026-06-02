> **Proposal only.** These tasks are the implementation plan for when this change is
> scheduled; none are started (no code lands from this change yet).

## 1. Protocol rename (BREAKING)

- [ ] 1.1 Rename `RoomCode`→`GroupCode` (`protocol/src/ids.rs`)
- [ ] 1.2 Rename entry/response messages: `CreateRoom`/`JoinRoom`/`RoomJoined` → `CreateGroup`/`JoinGroup`/`GroupJoined`; field `room_code`→`group_code`
- [ ] 1.3 Rename `ErrorCode::UnknownRoom`→`UnknownGroup`
- [ ] 1.4 Bump `PROTOCOL_VERSION`

## 2. Server rename

- [ ] 2.1 Rename `RoomRegistry`/`RoomCommand`/`RoomHandle` and `lobby/room.rs`→`lobby/group.rs`
- [ ] 2.2 Rename `create_room`/`join_room`/`handle_entry` arms and `lobby/codes.rs` helpers
- [ ] 2.3 Rename observability metrics `rooms_*`→`groups_*` (and update the span schema / admin projection)
- [ ] 2.4 Update doc comments referencing "room"

## 3. Persistent group lifecycle

- [ ] 3.1 Add a group state machine: `Lobby → InGame → Lobby(results) → …`, decoupling group lifetime from a single `run_game`
- [ ] 3.2 On `GameOver`, return players to the group lobby instead of deregistering the group
- [ ] 3.3 Add a "play again" path (re-deal with the same table) and a "leave" path (free the seat); generalize hostless auto-start to "4 ready players in the group"
- [ ] 3.4 Reclaim idle/empty *groups* (replaces idle-room cleanup)

## 4. Clients (lockstep)

- [ ] 4.1 `tui-client/`: renamed messages; a between-games "play again / leave" screen
- [ ] 4.2 `agent-harness/`: regenerate the TS protocol mirror (do not hand-edit); update entry flow
- [ ] 4.3 `bot-harness/`: renamed entry messages

## 5. Docs & validation

- [ ] 5.1 Reword game-design §14 "Rooms"→"Groups"
- [ ] 5.2 Integration test: one group plays two back-to-back games via "play again"
- [ ] 5.3 `make check` green across the workspace + agent-harness typecheck/tests
