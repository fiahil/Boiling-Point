## Why

`admin-ui` makes the server's telemetry the **source** for its read surface: an
in-process projection consumes the span lifecycle to serve the room inspector, the
privileged reveal, replay, and live balance figures. That presupposes server-side
work the current server does not provide:

- Today's observability is **`tracing` (JSON logs) + Prometheus** — and it emits
  *events*, not spans. "Spans as a source" requires both a `tracing` →
  OpenTelemetry bridge **and** real span instrumentation of the game loop (which
  has none today).
- The privileged reveal requires **secret attributes** (boiling point, hands,
  committed cards, mid-round volatility) to ride in spans **in-process only**, with
  redaction at the export boundary so they never reach a trace store
  (Constitution I, extended).
- The live view requires enumerating **open** spans, which standard OTLP export
  (emit-on-end) cannot provide — the server must expose an in-process
  **span-lifecycle hook** upstream of export sampling.
- Control (reload, toggle, room lifecycle) is a **write** telemetry cannot perform
  — the game loop must expose **command primitives**.

This change builds those server-owned prerequisites so the boundary is designed
for, not bolted on — and so it stays **server-owned** and never widens the player
`wire-protocol`.

## What Changes

- **OTEL span pipeline:** bridge the existing `tracing` instrumentation to
  OpenTelemetry (`tracing-opentelemetry` + `opentelemetry` + `opentelemetry-otlp`)
  and **instrument the game loop** to emit the documented span tree (room → game →
  round → wave → commit/resolve → score, plus `ws.message`, `reconnect`,
  `db.write`) with stable, versioned attribute names.
- **Secret attributes + redacting exporter:** carry secret game state in span
  attributes in-process; an **allow-list** redaction layer at the OTLP exporter
  strips them before any span leaves the process. Redaction is a tested,
  security-critical control with a single authoritative secret-attribute set.
- **In-process span-lifecycle hook:** expose span start/end (a `tracing` `Layer`,
  equivalently an OTEL `SpanProcessor`'s `on_start`/`on_end`) so `admin-ui`'s
  projection can maintain a live open-span registry and unsampled aggregates
  upstream of export sampling, without backpressuring the game loop.
- **Command-plane primitives:** authoritative game-loop operations for
  validated config reload, per-item enable/disable, and room lifecycle
  (seed / force-start / kill), exposed as a server API for the admin command plane
  to call — never reachable from the player wire.

## Capabilities

### New Capabilities
- `otel-span-pipeline`: `tracing`→OTEL bridge plus game-loop instrumentation
  emitting the versioned span tree and attributes; OTLP export wired (trace backend
  deferred per `admin-ui` R2).
- `telemetry-redaction`: allow-list redaction of secret span attributes at the
  export boundary; the authoritative secret-attribute set; tested as a security
  control.
- `span-lifecycle-hook`: an in-process consumer seam exposing span start/end for
  the `admin-ui` projection without backpressuring the game loop.
- `admin-command-primitives`: authoritative game-loop operations for reload,
  toggle, and room lifecycle, exposed only to the admin command API.

### Modified Capabilities
- `persistence-and-observability`: its **Structured Tracing** requirement (today
  `tracing` JSON events) becomes OTEL spans emitted from a documented, versioned
  span tree. `server-release-1` is now archived, so this modification is applied as
  a delta against the canonical capability here (resolving the fork the stub left
  open).

## Impact

- **New code (server):** OTEL setup in the `observability/` module, the span-schema
  contract, the redacting exporter, the span-lifecycle `Layer`, span instrumentation
  across `room`/`session`/`transport`/`persistence`, and command primitives on the
  room registry / config holder.
- **New dependencies:** `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`,
  `tracing-opentelemetry` (bridging existing `tracing`).
- **Hard boundary (Constitution I):** secrets ride in spans in-process only and are
  redacted at export; command primitives are reachable only via the admin command
  API, never the player `wire-protocol`.
- **Blocks:** `admin-ui`. **Depends on:** `server-release-1` (observability, room
  registry, content config) — now archived.
