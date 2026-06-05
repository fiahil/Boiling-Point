## Context

`group-model` made a group persist across games and the connection persist across
groups, but it treats all seats uniformly: at `GameOver` it keeps every non-disconnected
seat. Playtest feedback wants a sharper model — a group of friends should be able to
borrow a stranger for one game and keep its own roster (and a win tally) afterwards. So
the **group's membership** and a **game's roster** are now distinct:

```
  GROUP  members:[Alice,Bob,Carol]  standings:{A:2/3, B:1/3, C:1/3, guests:0/3}
    │  fill: need 1 more → matchmaking
    ▼
  GAME ROSTER (4) = members present + guests   [Alice,Bob,Carol] + Dave⟨guest⟩
    │  GameOver: Carol wins
    ▼
  GROUP  members:[Alice,Bob,Carol]  (Dave dropped)  standings:{…, C:2/4, guests:0/4}
```

## Goals / Non-Goals

**Goals:**
- A partial group (1–3 present members) can matchmake to fill to 4 with **guests**, with
  a visible "looking for a 4th…" state and a cancel.
- Guests are dropped at `GameOver`; members persist into the next game with the same code.
- A live, in-memory per-member win tally (games/wins/win-rate) + a guest aggregate,
  shown to members.
- Metrics expose live games and live groups.

**Non-Goals:**
- Persisting standings or any cross-session career stats (needs accounts — roadmap; also
  distinct from `persistence-and-replays`, which stores match history/replays).
- Groups larger than the table (no bench/rotation): **member cap = table size (4)**.
- Variable table sizes / 3-handed games (the engine is fixed at 4 — a group waits for a
  guest rather than starting short).
- Changing in-game rules, scoring, the round engine, or the session-router model.

## Decisions

- **D1 — Member cap = table size (4).** A group holds at most 4 members. With ≤4 members
  and a 4-seat table, every member always plays (no bench). A group of 4 members never
  needs a guest; a group of 1–3 members fills the rest with guests.
- **D2 — Membership rule: invite ⇒ member, fill ⇒ guest.** Joining by invite code (or
  forming a fresh quick-match table) makes you a **member**; being placed into an
  *existing* group by the fill queue makes you a **guest**. A seat carries this role.
- **D3 — Guests are one-game.** At `GameOver` the group returns to its lobby keeping only
  **members** (plus reconnected members); guest seats are pruned (alongside the existing
  `gone` pruning from `group-model`). The guest's session returns to the unbound menu.
- **D4 — Anchor-and-fill matchmaking.** Keep one solo queue. Groups requesting fill are
  *anchors*; waiting solos backfill an anchor's empty seats as guests (first-come). Solos
  with no anchor to fill still form a fresh quick-match group of 4 (all members), exactly
  as today. So fill prefers to top up real groups; pure-solo matchmaking is unchanged.
- **D5 — Fill is a member action with a visible searching state.** From a partial-group
  lobby a member requests fill; the group enters a `Lobby(searching)` sub-state and
  broadcasts "looking for N more"; it starts when the roster reaches 4, or a member
  cancels (back to the idle lobby). Idle cleanup still applies.
- **D6 — Standings live on the group actor, in memory.** Per member `{games_played,
  wins}` (+ derived win-rate) and an aggregate `{guest_games, guest_wins}`. Updated at
  `GameOver`: every member who played gets `+1 game`; each winner who is a member gets
  `+1 win`; if the roster had a guest, `guest_games += 1` and a guest win is
  `guest_wins += 1`. Co-champions (Deathmatch) each count as a win. Broadcast to members
  as a `StandingsUpdate` at game end and on roster changes. Standings die with the group.
- **D7 — Metrics.** Add a `games_active` gauge (up on game start, down on `GameOver`)
  beside the existing `groups_active`; surface both on the Grafana dashboard. Reword the
  metrics spec room→group (a `group-model` straggler).

## Risks / Trade-offs

- **[Risk] Returning a guest to the menu needs a server-initiated unbind.** Today
  `LeftGroup` is client-initiated. Options: (a) the guest **client** auto-leaves on
  `GameOver` (it knows it is a guest) and the server authoritatively prunes the guest
  seat regardless; (b) extend the router so the group can signal "you're unbound". Lean:
  (a) — server prunes the seat (authoritative), guest client returns to menu; a
  lingering bound guest is harmless (its non-member actions are already rejected).
- **[Risk] Fill-queue degenerate cases** (a group cancels mid-search; an anchor empties
  while waiting; a solo drops between assignment and game start) → define explicit queue
  transitions and reclaim an anchor that falls below 1 present member; cover with a test
  that fills a 3-member group with one solo and plays.
- **[Trade-off] Standings are ephemeral.** A server restart or a re-created group loses
  them. Accepted for v1 (persisting needs accounts); the live tally is the playtest ask.
- **[Trade-off] Adding `guest` to `PlayerPublic` is breaking** → justified: the table
  must render who is a guest, and it bumps the protocol version once alongside the new
  messages.

## Open Questions

- Naming of the fill messages — `FillGroup`/`CancelFill` vs `FindGuests`/`StopSearching`?
  (Lean: `FillGroup` / `CancelFill`.)
- Does a **guest** see the host group's standings, or only members? (Lean: members only;
  a guest sees the table but not the group's private tally.)
- Should standings also track **losses/last-place** or just wins + games (win-rate
  derived)? (Lean: games + wins + win-rate only, per the feedback.)
- When a fresh quick-match group of 4 solos forms, are all 4 members (so they can
  play-again together and accrue standings)? (Lean: yes — they formed the group together;
  only *fill* of an existing group yields guests.)

## Migration Plan

Single breaking release (version bump 2→3): protocol messages + `PlayerPublic.guest`,
then the matchmaking fill queue + member/guest seats + standings on the group actor, then
the metrics gauge + Grafana panels, then the three clients in lockstep, then validation
(`make check` + agent-harness). No data migration (standings are in-memory).
