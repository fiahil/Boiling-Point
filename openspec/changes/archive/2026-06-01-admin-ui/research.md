## Open Questions

- R1: How far does "OTEL spans as a source" extend — observe-only, live reveal,
  or everything readable?
- R2: Where do spans live — an in-process projection, an external trace backend,
  or both?
- R3: Are secret game-state attributes (boiling point, hands, volatility) allowed
  inside spans at all?
- R4: What is the admin UI itself built as?
- R5: Standard OTLP export emits a span only on *end*. How do we get "live rooms
  right now" from spans?
- R6: Trace backends sample. How are Principle IV balance numbers kept accurate?
- R7: Where does the server-owned prerequisite (OTEL upgrade, redaction, command
  primitives) live?
- R8: Is a span projection actually justified over a direct read of the live room
  registry (Constitution III)?

## R1: How far does "spans as the source" extend?

**Decision:** **Hybrid.** Spans drive observation, aggregates, replay, **and** the
live godmode reveal (the reveal reads the attributes of a room's *open* spans).
Control (config reload, toggles, room lifecycle) is a **separate command API**,
never sourced from or routed through telemetry.

**Rationale:** Spans are a read/observe medium; control is a write — telemetry
fundamentally cannot perform writes, so forcing control through spans would twist
the tool. Everything *readable* fits one coherent substrate (the projection),
which keeps a single source of truth for the admin read surface. The reveal stays
"from spans" because the secret state already lives in open-span attributes.

**Alternatives Considered:**
- *Observe-only from spans* (reveal reads authoritative game state directly) —
  safer for reveal accuracy but introduces a second read path and a second place
  that reaches into game internals.
- *Spans for everything readable* (push even live listing/reveal through exported
  traces) — purest, but fights the tool: needs open-span export and accepts
  export staleness for the most latency-sensitive views.

**Key Details:** read/observe/aggregate/replay/reveal → projection;
reload/toggle/kill/seed/force-start → command API. The command API's effects
re-appear as spans, so the UI confirms actions through the same telemetry.

## R2: Where do spans live?

**Decision:** **In-process projection now; design the OTLP export seam, defer
standing up a trace backend.** The projection (open-span registry + rolling
aggregates + bounded replay buffer) is built from the in-process span lifecycle.
An OTLP exporter is wired but a Tempo/Jaeger deployment is not required for v1.

**Rationale:** Constitution III — the live view, reveal, and balance numbers are
all served from in-process state with no extra infrastructure to operate. The
OTLP seam is designed so deep historical forensics (and the eventual
service-split, where a direct registry read no longer works) can be added later
without reshaping the admin surface.

**Alternatives Considered:**
- *External trace backend (Tempo/Jaeger) as the source* — rich historical
  forensics out of the box, but sampling and ingest latency hurt both the live
  view and balance accuracy (see R6); also new infra to run now.
- *Both from day one* — most capable, most operational burden up front;
  premature.

**Key Details:** the bounded replay buffer keeps recent completed games
in-process for wave-by-wave replay; deep/long-retention history is the trace
backend's job when it lands.

## R3: Are secrets allowed in spans?

**Decision:** **Yes, in-process only — redact at the export boundary.** Boiling
point, committed cards, hands, and mid-round volatility totals ride in span
attributes inside the process (so the reveal is span-sourced and the projection
holds them behind admin auth). The **OTLP exporter strips these attributes**
before any span leaves the process.

**Rationale:** This keeps the reveal a pure projection read while satisfying
Constitution I — extended: a secret must never cross a trust boundary, neither to
the player wire **nor** to a third-party trace store. Redaction becomes a single,
auditable, security-critical control at one boundary rather than scattered
conditionals.

**Alternatives Considered:**
- *Never put secrets in spans* — smaller blast radius, but the reveal is no longer
  span-sourced and requires a separate direct read of game state (the second read
  path R1 rejected).

**Key Details:** redaction is allow-list-based (only known-public attributes
export) and **tested as a security control** — a test asserts no secret attribute
key reaches the exporter output. The set of secret attributes is enumerated in
the span-schema contract (`admin-span-projection`).

## R4: What is the admin UI built as?

**Decision:** **Thin custom web app + embedded Grafana.** A small custom web UI
serves the room inspector, godmode reveal, activity feed, and control buttons
(things Grafana cannot do); the balance dashboard is **embedded Grafana** panels
over Prometheus.

**Rationale:** Plays to each tool's strength — Grafana gives time-series charting
for free; the custom app gives live state, the privileged reveal, and command
actions that a dashboards tool cannot. Keeps custom code minimal (Constitution
III).

**Alternatives Considered:**
- *All-custom web dashboard* — consistent UX, but re-implements charting Grafana
  provides free.
- *Grafana-only* — almost no custom code, but no real live reveal or control.
- *Second TUI* — maximally agent-testable and consistent with `terminal-client`,
  but weak for charts and timelines; revisit if the web stack proves heavy.

**Key Details:** the custom app authenticates against `admin-auth`, reads the
projection over the admin API (SSE/WebSocket for live updates), and embeds Grafana
panels (signed embed / same-origin) for `balance-dashboard`.

## R5: Getting "live rooms" from spans when OTLP exports on end

**Decision:** Maintain the live view from the **in-process span lifecycle**, not
from exported traces. A custom consumer hooks span **start** and **end** (a
`tracing` `Layer`'s `on_new_span`/`on_close`, equivalently an OTEL
`SpanProcessor`'s `on_start`/`on_end`): `on_start` registers an open span;
`on_end` removes it and folds it into aggregates/replay.

**Rationale:** Long-lived spans (`room.lifetime`, `game`, `round`, `wave`) are
literally "this is happening now." Enumerating open spans *is* the live state.
Standard OTLP export only emits on end, so it can never show an in-flight room —
the lifecycle hook is the mechanism that makes the live view possible.

**Alternatives Considered:** poll an exported trace backend for recent spans —
stale by an ingest interval and blind to still-open spans; rejected.

**Key Details:** the open-span registry is keyed by room/game/round/wave ids
carried as span attributes; the reveal reads secret attributes off these open
spans. The registry is the same structure the live UI and the reveal both read.

## R6: Keeping balance numbers accurate despite sampling

**Decision:** Balance aggregates derive from the **unsampled in-process stream**
(the projection sees 100% of spans, before any export sampling) — never from a
sampled exported trace, and never from the trace backend.

**Rationale:** Principle IV numbers (explosion rate target ~30–40%, cards/round,
dominant-color rate) drive balance decisions and **cannot be sampled** without
bias. The in-process projection sits upstream of export sampling, so its
aggregates are exact.

**Alternatives Considered:** query the trace backend for rates — wrong by
construction once sampling is enabled; rejected for the balance numbers.

**Key Details:** Prometheus metrics (also unsampled) remain the durable
time-series store; the embedded Grafana panels read Prometheus. The projection's
in-memory aggregates serve the live "right now / last N games" figures.

## R7: Where does the server-owned prerequisite live?

**Decision:** A **companion change, `server-otel-control-plane`** (created as a
stub here, mirroring how `admin-ui` itself began), reserves the server-owned
work: bridge `tracing` → OTEL spans (today `persistence-and-observability`
specifies `tracing` JSON only), the **redacting OTLP exporter**, the in-process
**span-lifecycle hook** the projection consumes, and the **command-plane
primitives** (kill/seed/force-start/reload) on the room registry. `admin-ui`
depends on it.

**Rationale:** This is server-owned, not admin-surface-owned, and it must not
widen the player `wire-protocol` (Constitution I). It also touches
`server-release-1`'s observability, which is still **in-progress and unarchived**
— so a clean companion change is preferable to editing another active change's
specs. Folding it into `server-release-1` instead remains a valid option for the
user to choose at promotion time.

**Alternatives Considered:**
- *Fold into `server-release-1`* — fine, but enlarges an already-large in-progress
  change and couples the admin prerequisite to the player-server release.
- *Put it in `admin-ui`'s Modified Capabilities* — wrong owner; the OTEL upgrade
  and command primitives are server domain, not admin UI.

**Key Details:** `server-otel-control-plane` stays a stub until promoted; this
keeps the boundary designed-for without over-specifying server internals now.

## R8: Span projection vs. direct registry read (Constitution III)

**Decision:** Use the **span projection** as the read backbone, accepting it is
*more* machinery than reading the live `DashMap` room registry directly.

**Rationale:** The projection earns its complexity: (1) **free replay/forensics**
— a wave-by-wave timeline falls out of the span tree; a registry read has no
history; (2) **decoupling** — the admin surface is a pure consumer and cannot race
or mutate the game loop (Constitution I by construction), versus a second code
path reaching into game internals that must be kept in lockstep with every new
game concept; (3) **future service-split** — an OTLP/projection model keeps
working when the monolith splits, whereas a direct registry read does not; (4) the
**dogfooding** alignment — admin blind spots reveal instrumentation gaps.

**Alternatives Considered:** direct registry read for live state + Grafana for
balance — genuinely simpler for *live state alone*, and the right call **if replay,
decoupling, and the service-split story are not wanted**. This is the live tension;
the design records it so it can be revisited if the projection proves heavy.

**Key Details:** the one place a direct read stays attractive is the reveal's
*exactness* — a projection lags true state by the flush. Mitigated by reading
open-span attributes (effectively live) and by keeping the door open to read
authoritative state directly if accuracy ever bites.

## Summary

- **R1** Hybrid: spans drive observe + aggregate + replay + reveal; control is a
  separate command API.
- **R2** In-process projection now; OTLP export seam designed, trace backend
  deferred.
- **R3** Secrets in spans in-process only; redacted (allow-list, tested) at the
  export boundary.
- **R4** Thin custom web app for inspector/reveal/control + embedded Grafana for
  balance.
- **R5** Live view + reveal from the in-process span-lifecycle hook
  (`on_start`/`on_end`), not exported traces.
- **R6** Balance aggregates from the unsampled in-process stream / Prometheus —
  never a sampled trace.
- **R7** Server-owned prerequisite reserved in companion stub
  `server-otel-control-plane`; may instead fold into `server-release-1`.
- **R8** Span projection over direct registry read — justified by replay,
  decoupling, and the service-split story; tension recorded.
