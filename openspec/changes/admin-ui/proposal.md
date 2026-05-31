## Why

Operators and developers need to **observe and manage a running server** —
inspect live rooms, sessions, and the auto-match queue; watch the balance
metrics that drive Principle IV; reload or toggle content/balance config; and,
for debugging, **privately reveal hidden game state** (boiling point, opponents'
hands) that the player wire must never carry. During the `terminal-client`
exploration we decided this privileged "godmode" view does **not** belong in the
player client (Constitution I) — it belongs behind a separate admin surface
talking to a separate server control plane.

The insight that shapes this change: the server already **must** emit telemetry
(`persistence-and-observability` requires structured tracing spans for phase
transitions, message handling, room lifecycle, and DB writes). Rather than build
a *second* read path that reaches into live game internals, **the admin read
surface is a projection of that span stream.** One instrumentation surface feeds
three consumers — Prometheus, the trace export, and the admin UI. If the admin
UI is blind to something, that thing isn't instrumented — which means production
observability is blind to it too. The admin UI becomes a forcing function for
telemetry quality, and a pure read projection that **cannot, by construction,
race or mutate the game loop** (Constitution I).

## What Changes

- Add a **separate admin interface** — a **thin custom web app** for the room
  inspector, godmode reveal, and control actions, with **embedded Grafana** for
  the balance dashboard — talking to a **server-side admin/control API distinct
  from the player protocol**. It never uses, widens, or shares the player
  `protocol/` wire.
- **Spans are the read source.** An in-process **admin projection** consumes the
  server's OTEL span lifecycle (`on_start`/`on_end`) and maintains: a live
  **open-span registry** (current rooms/games/rounds/waves), **rolling balance
  aggregates** (folded from completed spans), and a **bounded replay buffer**
  (recent completed games, wave-by-wave). The admin UI is a read-only consumer of
  this projection.
- **Read surfaces, all span-sourced:** fleet overview, live room list, per-room
  detail, the **privileged hidden-state reveal** (boiling point, hands,
  volatility totals — read from the attributes of a room's *open* spans), live
  activity feed, anomaly/stuck-room detection, per-game replay, and the balance
  dashboard.
- **Control is the deliberate write-side exception.** Telemetry cannot perform
  writes, so config reload, per-item enable/disable, and room lifecycle actions
  (seed / force-start / kill) are issued over an explicit admin **command API** —
  never through spans. Each command's effect then re-appears in the span stream,
  so the UI confirms it through the same telemetry (the loop closes).
- **Secrets ride in spans in-process only.** Boiling point, hands, and mid-round
  volatility live in span attributes *inside the process* so the reveal is
  span-sourced; the **OTLP exporter redacts them** before anything crosses the
  trust boundary to a trace backend. Redaction is a security-critical, tested
  control (Constitution I extended: never leak secrets to the player wire *or* a
  third-party store).
- **Balance numbers come from the unsampled in-process stream**, never a sampled
  exported trace — Principle IV numbers cannot be sampled.
- All of the above behind **admin authentication** separate from anonymous player
  session tokens, with role-based gating (the reveal requires an elevated role).

This change adds **no game logic** and modifies **no player-facing server
behavior**. The admin surface is a read projection plus a narrow command channel.

## Capabilities

### New Capabilities
- `admin-auth`: operator authentication/authorization separate from player
  session tokens, role-based capability gating, admin-channel isolation from the
  player WebSocket. Gates every admin capability.
- `admin-span-projection`: the in-process read model built solely by consuming
  the server's OTEL span lifecycle — open-span registry (live state), unsampled
  rolling aggregates, bounded replay buffer, and a versioned span-schema contract.
  Read-only by construction.
- `room-inspector`: live room/session/queue listing, the admin-only hidden-state
  reveal for a chosen room (from open-span attributes), stuck-room detection, and
  per-game replay — all from the projection.
- `balance-dashboard`: the Principle IV metrics (explosion rate, durations,
  cards/round, dominant-color rate, timeout/reconnection rate, reshuffle
  frequency) derived from the unsampled source and visualized via embedded
  Grafana.
- `admin-control`: the separate command API — validated config reload, per-item
  enable/disable toggles, and room lifecycle actions — explicitly *not* telemetry,
  with every action audited and observable back through the span stream.

### Modified Capabilities
- _None in this change._ The admin surface is all-new and alters no
  `server-release-1` player requirement. The server-owned prerequisites it
  depends on — upgrading observability to **OTEL spans** (today's
  `persistence-and-observability` specifies `tracing` JSON), the **redacting OTLP
  exporter**, the in-process **span-lifecycle hook**, and the **command-plane
  primitives** in the game loop — are reserved in the companion change
  `server-otel-control-plane` (see Impact). They are server-owned and must not
  widen the player `wire-protocol`.

## Impact

- **New surface:** an admin web app (thin custom UI + embedded Grafana) and a
  server-side admin/control API + auth, deployed and routed separately from the
  player WebSocket. In the single-binary monolith this is a separate module and
  separate routes/port — not a separate service yet (Constitution III).
- **New code:** an `admin/` server module (projection, admin API, command
  handlers, auth, redaction at the export boundary) and an admin web client.
  Builds on `server-release-1`'s room registry, content config, and observability.
- **New dependencies:** the OTEL stack on the server side
  (`opentelemetry`, `opentelemetry-otlp`, `tracing-opentelemetry`) bridging the
  existing `tracing` instrumentation; Grafana for the embedded balance panels;
  a web stack for the admin client (TBD in design — kept thin).
- **Hard boundary (Constitution I):** the admin path must never widen the player
  protocol or leak privileged data onto a player connection. The hidden-state
  reveal is admin-channel-only by construction, and secrets are redacted at the
  OTLP export boundary so they never reach a trace store.
- **Depends on:** `server-release-1` (rooms, sessions, content config,
  observability) **and** the companion `server-otel-control-plane` (OTEL span
  pipeline + redaction + command primitives). Sequenced **after** a usable server
  and the player client.
- **Constitution:** advances Principle II (one instrumentation surface dogfooded
  by the admin UI; Claude reads the same spans) and Principle IV (accurate,
  unsampled balance numbers). Principle I is *strengthened* — the read side is a
  projection that cannot mutate state, and secrets never cross a trust boundary.
  Principle III is the live tension (see `design.md`): a span projection is more
  machinery than a direct registry read, justified by free replay, decoupling,
  and the future service-split story.
