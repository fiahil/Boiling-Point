> **STATUS: STUB.** This proposal is a placeholder to capture scope and the
> server-side dependency. It is intentionally light — capabilities are named but
> not yet specced. Do not generate research/specs/design/tasks until it is
> promoted from stub.

## Why

Operators and developers need a way to **observe and manage a running server** —
inspect live rooms, sessions, and the auto-match queue; watch the balance metrics
that drive Principle IV; reload or toggle content/balance config; and, for
debugging, **privately reveal hidden game state** (boiling point, opponents'
hands) that the player wire must never carry. During the terminal-client
exploration we explicitly decided this privileged "godmode" view does **not**
belong in the player client (Constitution I) — it belongs behind a separate admin
surface talking to a separate server control plane. This stub reserves that
scope so the boundary is designed for, not bolted on.

## What Changes

- Add a **separate admin interface** (web dashboard or a second TUI — TBD) that
  talks to a **server-side admin/control API distinct from the player
  protocol**. It never uses, widens, or shares the player `protocol/` wire.
- **Read-only first:** list active rooms, players/sessions, and the queue; live
  balance metrics (explosion rate, round durations, cards/round, reshuffle
  frequency) sourced from the existing Prometheus/observability stack.
- **Then control:** validated content/balance **config reload**, per-item
  enable/disable toggles, and room lifecycle actions (seed a room, force-start,
  kill an idle/stuck room).
- **Privileged debug reveal:** a strictly admin-gated view of hidden game state
  for a selected live room, served over the admin channel only — **never**
  reachable from a player connection.
- All of the above behind **admin authentication** separate from anonymous
  player session tokens.

## Capabilities

### New Capabilities
> Stubs — to be detailed when this change is promoted.
- `admin-auth`: authentication/authorization for operators, separate from player
  session tokens; gates every admin capability.
- `room-inspector`: read-only live view of rooms, players, sessions, and the
  auto-match queue, plus the admin-only hidden-state reveal for a chosen room.
- `balance-dashboard`: surfacing the server's balance metrics (Principle IV) for
  observation during bot runs and playtests.
- `content-config-admin`: validated reload and per-item enable/disable of the
  cards/effects/modifiers config, reusing the server's fail-fast validation.

### Modified Capabilities
- _None yet._ A promoted version will require a **server-side admin/control API**
  that `server-release-1` does not provide; that is a server-owned dependency to
  be specced there or in a companion change — not a change to the player
  `wire-protocol`.

## Impact

- **New surface:** an admin client (stack TBD) and a server-side admin/control
  API + auth, deployed and routed separately from the player WebSocket.
- **Hard boundary:** the admin path must never widen the player protocol or leak
  privileged data onto a player connection (Constitution I). The
  hidden-state reveal is admin-channel-only by construction.
- **Depends on:** `server-release-1` (rooms, sessions, observability metrics,
  content config) and a new server-side admin API. Sequenced **after** the player
  client and a usable server.
