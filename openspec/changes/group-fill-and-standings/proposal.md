## Why

Playtest feedback: "I create a group with my 3 friends — but we're only 3, not 4. I
want the **group** to enter matchmaking and pick up a 4th. When the game ends I come
back to the same group **without** that 4th person. And I want a **running tally of wins
per person** in my group." This makes the **Group** and the **Game** complementary, not
the same thing (which `group-model` had collapsed): a *group* is the persistent social
unit (your friends + their standings); a *game* is one assembled table of 4 that may
borrow a stranger to fill an empty seat.

`group-model` shipped persistent groups and the session connection, but treated every
seated player identically — there is no notion of **who belongs to the group** vs **who
is just filling a seat**, no way for a partial group to matchmake for the rest, and no
standings. This change adds those three things.

## What Changes

- **Group matchmaking (fill).** A group with fewer than 4 present members can enqueue to
  **fill** its empty seats from the matchmaking queue, showing a "looking for a 4th…"
  state until enough guests arrive (then the game starts) or a member cancels.
- **Members vs guests.** A group has **members** (joined by invite code — persistent,
  carry standings) and **guests** (arrived via matchmaking fill — present for **one
  game**, then dropped when the group returns to its lobby). A player's role is public on
  the table.
- **Group standings (NEW, live).** Each group keeps an **in-memory** running tally —
  per member: games played, wins, win-rate; plus an aggregate **guest** bucket (guest
  wins don't vanish, they roll into "guests"). Standings live only as long as the group
  does; they are not persisted (that needs accounts — roadmap).
- **Metrics.** Track **live games in progress** (a new `games_active` gauge) alongside
  the existing **live groups** gauge, and surface both on the Grafana balance dashboard.
  Also corrects the lingering "rooms" wording in the metrics requirement to "groups".

## Capabilities

### New Capabilities
- `group-standings`: the live per-member win tally (games/wins/win-rate), the guest
  aggregate, when it updates, and how it is conveyed to members.

### Modified Capabilities
- `lobby-and-matchmaking`: the auto-match queue gains **partial-group fill** (a group
  requests guests; solos backfill it); the group lifecycle gains a member/guest
  distinction (guests are dropped at game end, members persist) and a member cap equal
  to the table size (4).
- `wire-protocol`: new client messages to request/cancel fill; a server "searching"
  state and a standings update; `PlayerPublic` marks whether a seat is a guest
  **(BREAKING — bumps `PROTOCOL_VERSION`)**.
- `persistence-and-observability`: the metrics requirement adds an **active games** gauge
  and is reworded room→group.
- `balance-dashboard`: surfaces live games and live groups.

## Impact

- **BREAKING wire change** — bumps `PROTOCOL_VERSION` (2→3); all clients update in
  lockstep (TUI, agent-harness TS mirror, bot-harness).
- **Code:** `protocol/src/{client,server}.rs` (fill/cancel + standings messages,
  `PlayerPublic.guest`); `server/src/lobby/{matchmaking,group,registry}.rs` (partial-fill
  queue, member/guest seats, guest pruning, standings on the group actor),
  `server/src/session.rs` (carry role/standings through the game),
  `server/src/observability.rs` + `ops/grafana/dashboards/` (`games_active` gauge,
  panels); clients render the guest, the "looking for a 4th…" state, and standings.
- **No persistence/DB impact** — standings are live-only and die with the group;
  durable career stats remain a roadmap item (depends on accounts), distinct from
  `persistence-and-replays` (which stores match history + replays, not live standings).
- **Builds on `group-model`** (members vs roster is the refinement of its open question
  "does a group persist its roster each game?": it persists its *members*; the roster is
  reassembled each game).
