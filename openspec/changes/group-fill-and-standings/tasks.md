## 1. Protocol (BREAKING)

- [x] 1.1 Add client messages `FillGroup` (request matchmaking fill for my group) and `CancelFill`
- [x] 1.2 Add `PlayerPublic.guest: bool` (members `false`, guests `true`)
- [x] 1.3 Add server messages: `GroupSearching { needed: u8 }` (the "looking for a 4th…" state) and `StandingsUpdate { entries, guest }`
- [x] 1.4 Bump `PROTOCOL_VERSION` (2→3)

## 2. Member / guest seats

- [x] 2.1 Add a per-seat role to the group (`member` vs `guest`); invite/quick-match join ⇒ member, fill placement ⇒ guest
- [x] 2.2 Mark `guest` on the public table (`PlayerPublic`) in `GroupJoined`/`GameStarting`/snapshots
- [x] 2.3 At `GameOver`, prune guest seats when returning to the lobby (alongside the existing `gone` pruning); the guest's session returns to the unbound menu
- [x] 2.4 Enforce member cap = table size (4): a 4-member group never requests fill

## 3. Group matchmaking (fill)

- [x] 3.1 Add a `FillGroup` path: a partial-group member request puts the group into a `Lobby(searching)` sub-state and registers it with the queue for `4 − present` seats
- [x] 3.2 Rework `MatchQueue` to anchor-and-fill: waiting solos backfill a searching group as guests (first-come); solos with no anchor still form a fresh 4-member quick-match group
- [x] 3.3 Broadcast `GroupSearching { needed }`; start the game when the roster reaches 4
- [x] 3.4 `CancelFill` returns the group to its idle lobby and deregisters it from the queue; reclaim an anchor that drops below 1 present member

## 4. Group standings (NEW, live)

- [x] 4.1 Add an in-memory standings struct on the group actor: per member `{games_played, wins}` + aggregate `{guest_games, guest_wins}`
- [x] 4.2 Update at `GameOver` (members who played `+1 game`; member winners `+1 win`; guest game/win rolls into the aggregate; Deathmatch co-champions each count)
- [x] 4.3 Broadcast `StandingsUpdate` at game end and on roster changes; derive win-rate for display
- [x] 4.4 Standings are dropped when the group ends (no persistence)

## 5. Metrics & dashboard

- [x] 5.1 Add a `games_active` gauge (up on game start, down on `GameOver`) beside `groups_active`
- [x] 5.2 Surface live games and live groups on the Grafana balance dashboard (`ops/grafana/dashboards/`)
- [x] 5.3 Reword the metrics requirement room→group (a `group-model` straggler)

## 6. Clients (lockstep)

- [x] 6.1 `tui-client/`: a "fill group / looking for a 4th… / cancel" flow; render guests on the table; a standings panel
- [x] 6.2 `agent-harness/`: update the TS protocol mirror (fill/cancel, `PlayerPublic.guest`, searching/standings); handle the new messages
- [x] 6.3 `bot-harness/`: tolerate the new messages (a bot can fill as a guest; `guest`/standings are no-ops for play)

## 7. Docs & validation

- [x] 7.1 Update game-design §14 to describe member vs guest, fill, and standings
- [x] 7.2 Integration test: a 3-member group fills with one solo guest, plays, and returns to 3 members with standings updated and the guest dropped
- [x] 7.3 Integration test: a member win and a guest win land in the right standings buckets
- [x] 7.4 `make check` green across the workspace + agent-harness typecheck/tests
