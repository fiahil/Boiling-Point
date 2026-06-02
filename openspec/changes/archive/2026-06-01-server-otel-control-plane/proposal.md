## Why

`admin-ui` makes the server's telemetry the **source** for its read surface: an
in-process projection consumes the span lifecycle to serve the room inspector, the
privileged reveal, replay, and live balance figures. That presupposes server-side
work the current server does not provide:

- Today's observability is **`tracing` (JSON logs) + Prometheus** — and it emits
  *events*, not spans. "Spans as a source" requires both a `tracing` →
  OpenTelemetry bridge **and** real span instrumentation of the game loop (which
  has none today).
- The privileged reveal requires **sensitive attributes** (boiling point, hands,
  committed cards, mid-round volatility, deck seed) to ride in spans. These may
  reach the trusted, operator-only trace backend; the trust boundary that matters
  is the **player wire**, which never carries them (the admin channel is a separate
  transport). There is therefore no export-time redaction — a deliberately simpler
  path (Constitution III) than the originally-proposed redacting exporter.
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
- **Sensitive attributes (no redaction):** carry sensitive game state in span
  attributes. Spans export as-is to the trusted operator-only trace backend; the
  player wire never carries them (enforced by the separate admin channel, not by
  attribute-level stripping). The redacting exporter the stub originally reserved
  was dropped for this simpler path.
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
  deferred per `admin-ui` R2). Spans export as-is — no redaction (the trace backend
  is operator-only; the player wire is the trust boundary).
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
  contract, the span-lifecycle `Layer`, span instrumentation across
  `room`/`session`/`transport`/`persistence`, and command primitives on the room
  registry / config holder.
- **New dependencies:** `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`,
  `tracing-opentelemetry` (bridging existing `tracing`).
- **Hard boundary (Constitution I):** sensitive game state never crosses the player
  `wire-protocol` (enforced by the separate admin channel); command primitives are
  reachable only via the admin command API, never the player wire.
- **Blocks:** `admin-ui`. **Depends on:** `server-release-1` (observability, room
  registry, content config) — now archived.
