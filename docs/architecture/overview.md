# Architecture Overview

Boiling Point is a Rust cargo workspace plus one Node/TS harness, organized around a
single principle: **the server is the only source of truth**. Everything else speaks
to it through one narrow wire protocol and receives only what a player is allowed to
see. (See the [constitution](../../CLAUDE.md) and, for the deeper infra rationale,
[server-architecture.md](server-architecture.md) and
[tech-stack-exploration.md](tech-stack-exploration.md).)

## Components

`protocol/` is the waist of the hourglass: every other component depends on it and
nothing else couples directly.

```
              ┌───────────────────────────────────────────────────────┐
              │                      protocol/                         │
              │   ClientMessage · ServerMessage · ids · vocab          │
              │   codec  (MessagePack on the wire, JSON for debugging) │
              │   — no game logic, no secrets —                        │
              └───────────────────────────────────────────────────────┘
                 ▲              ▲                ▲               ▲
   depends on ───┘              │                │               └─── depends on
                                │                │
   ┌───────────────┐   ┌────────┴────────┐   ┌───┴─────────────┐   ┌─────────────────┐
   │  tui-client/  │   │     server/     │   │  bot-harness/   │   │  agent-harness/ │
   │ ratatui       │   │ AUTHORITATIVE   │   │ headless bots   │   │ Node/TS         │
   │ renderer      │   │ engine + state  │   │ seeded batches  │   │ Claude-as-player│
   │ (untrusted)   │   │ (owns secrets)  │   │ balance stats   │   │ (untrusted)     │
   └───────┬───────┘   └────────┬────────┘   └───┬─────────────┘   └────────┬────────┘
           │ WebSocket          │                │ in-process OR             │ WebSocket
           │ (MessagePack)      │                │ WebSocket                 │ (MessagePack)
           └────────────────────┴────────────────┴───────────────────────────┘
                          all clients are untrusted; the server validates every action

   server/ also exposes, on isolated ports:  admin API (8081) · Prometheus metrics (9090)
```

Why this shape: clients (TUI, bots, Claude agents) hold **no game logic** and cannot
represent a secret (boiling point, opponents' hands, the deck) — leakage is prevented
by construction, not by discipline. The bot and agent harnesses are the two testing
layers the constitution requires (headless balance bots; Claude-as-player).

## Server internals

One process, cleanly separated modules (single-binary monolith — Principle III):

```
   AppState
   ├── sessions : SessionStore     anonymous session tokens, reconnect identity
   ├── rooms    : RoomRegistry      code → live room task (DashMap)
   └── queue    : MatchQueue        table-filling auto-match

   transport.rs   WebSocket upgrade, entry handshake, per-connection in/out tasks
        │  spawns / routes to
        ▼
   lobby/room.rs  ONE tokio task owns ONE room's state (no locks; mpsc commands in)
        │  runs
        ▼
   session.rs     game driver: deal → rounds → waves → depile → score → GameOver
        │  drives
        ▼
   game/*         the pure engine: runner, round, deck, scoring, modifiers, deathmatch
                  (the authoritative rules; holds every secret)

   observability.rs   JSON logs + OTEL span bridge + Prometheus + admin span feed
   admin/*            operator-only read/control API (isolated from the player wire)
   persistence.rs     Postgres schema + writers — BUILT, NOT WIRED at runtime
                      (review finding F4; being reworked in `persistence-and-replays`)
```

## Connection & game lifecycle

What flows between a client and the server, from connect to game over:

```
  CLIENT                                   SERVER
    │   ws connect /ws                       │
    │ ──────────────────────────────────▶    │
    │   CreateGroup | JoinGroup | EnqueueMatch (entry msg, carries protocol_version)
    │ ──────────────────────────────────▶    │  validate version; place in a room
    │            RoomJoined {code, you, …}    │
    │ ◀──────────────────────────────────    │
    │            (waiting — table fills to 4) │
    │            GameStarting                 │  ← 4th seat taken
    │ ◀──────────────────────────────────    │
    │  ┌── per round (×5) ───────────────────────────────────────────┐
    │  │   ModifierRevealed (rounds 2–5)      │                       │
    │  │   YourHand {your cards only}         │  (secret-routed)      │
    │  │  ┌── per wave ──────────────────────────────────────────┐   │
    │  │  │   WaveOpened {wave, timer}         │                  │   │
    │  │  │   CommitCard|CommitPass|LockIn ──▶ │  validate, apply │   │
    │  │  │   WaveResolved {who played/passed} │                  │   │
    │  │  └────────────────────────────────────────────────────────┘   │
    │  │   Depile {cards, crossing, bp?}      │  play-order reveal    │
    │  │   RoundScored | Explosion            │                       │
    │  └───────────────────────────────────────────────────────────────┘
    │            GameOver {standings, winners}│
    │ ◀──────────────────────────────────    │  → (future) persist match + replay
```

Secret discipline: the server **never** sends a value a player shouldn't have. The
boiling point is disclosed only via a Peek the player made, or revealed at an
explosion depile; opponents' hands and the deck never cross the wire.

## Client phase state machine (TUI)

The terminal client is a pure reducer over server messages — each `ServerMessage`
folds into the view model and advances a phase, and every screen is a pure function
of that state (so it snapshot-tests with no terminal and no server):

```
   Entry ──pick──▶ Connecting ─┐
     │  └─Join code─▶ JoinCode ─┤
     │  └─Quick match─▶ Queue ───┼──RoomJoined──▶ Lobby
     │                           │                  │ GameStarting
     ▼ (Error)                   │                  ▼
   (back to Entry)               │              RoundStart ◀───────────┐
                                 │                  │ WaveOpened        │ next round
                                 │                  ▼                   │
                                 │               Playing ──Depile──▶ Depile
                                 │                  ▲                   │
                                 │        WaveResolved│ (next wave)     ▼
                                 │                  └────────────  Scoring
                                 │                                     │ GameOver
                                 │                                     ▼
                                 └───────────────────────────────  GameOver ──r──▶ Queue
```

(Disconnect paints a reconnect overlay independent of phase; a `StateSnapshot` on
rejoin restores allowed state and locks the player out of the in-progress round.)

## Determinism (why replays are cheap)

The engine is driven by a single seeded RNG tree (`StdRng` from a root seed) over a
pinned content config. Given the same seed, config, and ordered player actions, a game
replays bit-for-bit — the basis for the `persistence-and-replays` replay format.
