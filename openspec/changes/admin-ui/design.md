## Context

Boiling Point's server is authoritative (Constitution I) and already required to
emit telemetry: `server-release-1`'s `persistence-and-observability` mandates
structured tracing spans for phase transitions, message handling, room lifecycle,
and DB writes, plus Prometheus balance metrics. Operators need to observe and
manage a running server and — for debugging — privately reveal hidden state
(boiling point, hands) that the player wire must never carry.

This change makes that telemetry the **source** for the admin read surface: an
in-process projection consumes the span lifecycle and serves the room inspector,
reveal, replay, and live balance figures; control is a separate command API. Four
forks were resolved in `research.md` (R1–R4 locked with the user; R5–R8 derived).
The server-owned prerequisites (OTEL upgrade, redaction, command primitives) are
reserved in the companion stub `server-otel-control-plane`.

Current constraint worth noting: today's observability spec says `tracing` (JSON
logs) + Prometheus — **not** OTEL. "Spans as a source" therefore presupposes a
server-side upgrade (the companion change), not just an admin choice.

## Goals / Non-Goals

**Goals:**
- One instrumentation surface feeding three consumers (Prometheus, OTLP export,
  admin projection); the admin UI is a pure read consumer.
- Live room inspector, privileged reveal, per-game replay, anomaly detection, and
  a balance dashboard — all span-sourced.
- A narrow, audited command API for control (reload, toggle, room lifecycle).
- Secrets never cross a trust boundary: in-process spans only, redacted at export.
- Accurate (unsampled) Principle IV balance numbers.

**Non-Goals:**
- No game logic and no change to player-facing server behavior or `wire-protocol`.
- No external trace backend deployment in v1 (the OTLP seam is designed, deferred).
- No multi-service split; admin lives as a module + separate routes in the monolith.
- No OAuth/account system for operators beyond a separate admin auth mechanism.

## Decisions

### D1. Hybrid source: spans for all reads (incl. reveal), command API for writes
Spans are a read/observe medium; control is a write telemetry cannot perform.
Everything readable — fleet/room listing, reveal, aggregates, replay — is served
from the projection; reload/toggle/kill/seed/force-start go through a separate
command API whose effects re-appear as spans. Rejected: routing control through
telemetry (twists the tool); a second direct read path for reveal (duplicates
reach into game internals). See R1.

### D2. In-process projection now; OTLP export seam, trace backend deferred
The projection holds an open-span registry (live state + reveal), unsampled
rolling aggregates (balance), and a bounded replay buffer (recent games). An OTLP
exporter is wired so a Tempo/Jaeger/ClickHouse backend can be added later for deep
history and for the post-service-split world — without reshaping the admin
surface. Rejected: trace-backend-as-source (sampling + ingest latency hurt live
and balance accuracy); both-from-day-one (premature infra). See R2, Constitution
III.

### D3. The span tree is the data model
The projection's fidelity is exactly the span hierarchy the server emits:

```
room.lifetime  {room_id, mode, config_version, created}
├─ lobby.wait  {players_joined, queue_pos}
├─ game        {game_id, players[], round_count}
│  ├─ round    {round_idx, boiling_point*, range, modifiers[]}
│  │  ├─ wave  {wave_idx, active_players, timer_s}
│  │  │  ├─ commit  {player, card_id*, volatility*}
│  │  │  └─ resolve {total_volatility*, exploded, revealed[]}
│  │  └─ score {dominant_color, pot_value, deltas[]}
│  └─ game_over {winner, final_scores, deathmatch}
├─ ws.message  {session, msg_type, latency_ms, rejected, reason}
├─ reconnect   {session, grace_used_ms, success}
└─ db.write    {table, rows, dur_ms}     (* = secret attribute)
```
Open spans = live state and reveal. Closed spans = aggregates + replay. This span
schema is a **versioned contract** the projection depends on (R5, R8).

### D4. Active-span enumeration via the span lifecycle, not exported traces
Long-lived spans are "this is happening now." The projection hooks span start/end
(a `tracing` `Layer`'s `on_new_span`/`on_close`, equivalently an OTEL
`SpanProcessor`'s `on_start`/`on_end`): start registers an open span; end removes
it and folds it into aggregates/replay. Standard OTLP exports only on end, so it
can never show in-flight rooms — the lifecycle hook is what makes the live view
possible. Instrumentation stays in `tracing`, bridged to OTEL via
`tracing-opentelemetry`. See R5.

### D5. Secrets in spans in-process only; allow-list redaction at the OTLP boundary
Secret attributes (boiling point, committed cards, hands, mid-round volatility)
ride in spans so the reveal is span-sourced and the projection holds them behind
admin auth. A redacting layer at the OTLP exporter strips them — **allow-list**
(only known-public attributes export), driven by the schema contract's secret set
(D3). Redaction is a tested security control. The trust boundary is the export
sink, not the instrumentation. See R3, Constitution I (extended).

```
 game loop ─spans(+secrets)→ tracing/OTEL ─┬→ AdminProjection (full, in-proc, admin auth)
                                           ├→ metrics/Prometheus (unsampled)
                                           └→ OTLP exporter (REDACT) → trace backend (no secrets)
```

### D6. Balance numbers from the unsampled stream, never a sampled trace
Principle IV figures cannot be sampled. The projection sits upstream of export
sampling and folds 100% of completed spans; Prometheus is likewise unsampled. The
embedded Grafana panels read Prometheus; the live "last N games" figures read the
projection. Never query a sampled trace backend for rates. See R6.

### D7. Admin UI = thin custom web app + embedded Grafana
Custom web app for the room inspector, reveal, activity feed, replay, and control
buttons (things Grafana cannot do), reading the projection over the admin API
(SSE/WebSocket for live). Balance dashboard = embedded Grafana panels over
Prometheus. Rejected: all-custom (rebuilds charting), Grafana-only (no reveal/
control), second TUI (weak for charts; revisit if the web stack is heavy). See R4.

### D8. Server-owned prerequisite in a companion change
`server-otel-control-plane` (stub) reserves: `tracing`→OTEL bridge, redacting OTLP
exporter, the in-process span-lifecycle hook the projection consumes, and command
primitives (kill/seed/force-start/reload) on the room registry. Kept out of the
player `wire-protocol`. May instead fold into `server-release-1` at promotion. See
R7.

## Risks / Trade-offs

- **Projection is more machinery than a registry read (Constitution III).** →
  Justified by free replay/forensics, decoupling (read side cannot race/mutate the
  game loop), and the future service-split story; tension recorded in R8. Revisit
  if it proves heavy; a direct registry read remains the fallback for live state.
- **Reveal lags true state by the flush interval.** → Reveal reads open-span
  attributes (effectively live); keep the door open to read authoritative state
  directly if exactness ever bites.
- **Redaction is security-critical and easy to regress.** → Allow-list (not
  deny-list) so new secret attributes are non-exporting by default; a test asserts
  no secret attribute key reaches exporter output; secret set lives in one schema
  contract.
- **Span schema becomes a contract the UI depends on.** → Version it; projection
  ignores unknown spans/attributes gracefully (forward/backward tolerant).
- **Projection memory growth (open-span leaks, replay buffer).** → Bounded replay
  buffer with oldest-eviction; open-span registry bounded by live rooms; reap
  entries whose end was missed via the stuck-room age check.
- **Projection backpressure could stall the game loop.** → Consumer must never
  block emission; drop/coalesce under load rather than backpressuring the game.

## Migration Plan

1. Land companion `server-otel-control-plane`: bridge `tracing`→OTEL, emit the D3
   span tree with secret attributes, add the redacting OTLP exporter (allow-list)
   and the in-process span-lifecycle hook + command primitives.
2. Build the projection (open-span registry, unsampled aggregates, bounded replay)
   against the lifecycle hook.
3. Build `admin-auth` + the admin API (read + command), served on isolated routes.
4. Build the thin web app (inspector, reveal, replay, activity, control) + embed
   Grafana for balance.
5. Verify the redaction control and the unsampled-aggregate accuracy with tests
   and a bot-harness run.

Rollback: the admin module and its routes are separable; disabling them leaves the
player server and the OTEL/Prometheus telemetry intact.

## Open Questions

- Web stack for the thin admin app (kept deliberately small) — not yet chosen; a
  second-TUI fallback stays on the table if the web stack proves heavy.
- Operator auth mechanism specifics (shared secret vs. OIDC vs. mTLS for the admin
  route) — to resolve when `server-otel-control-plane` is promoted.
- Whether the OTEL upgrade folds into `server-release-1` or stays a companion
  change (R7) — user's call at promotion.
- Exact "expected duration" thresholds for stuck-room detection — **needs
  playtesting** against real wave-timer budgets.
