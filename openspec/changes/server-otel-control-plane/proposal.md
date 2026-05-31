> **STATUS: STUB.** This proposal reserves the **server-owned** prerequisites that
> `admin-ui` depends on. It is intentionally light — capabilities are named but not
> yet specced. Do not generate research/specs/design/tasks until it is promoted
> from stub. **Alternative:** these may instead be folded into `server-release-1`
> (its `persistence-and-observability` is still in-progress); that is the user's
> call at promotion time (`admin-ui` research R7).

## Why

`admin-ui` makes the server's telemetry the **source** for its read surface: an
in-process projection consumes the span lifecycle to serve the room inspector, the
privileged reveal, replay, and live balance figures. That presupposes server-side
work `server-release-1` does not provide today:

- Its `persistence-and-observability` specifies **`tracing` (JSON logs)** +
  Prometheus — **not OTEL spans**. "Spans as a source" requires bridging
  `tracing` → OpenTelemetry.
- The privileged reveal requires **secret attributes** (boiling point, hands,
  mid-round volatility) to ride in spans **in-process only**, with redaction at
  the export boundary so they never reach a trace store (Constitution I, extended).
- The live view requires enumerating **open** spans, which standard OTLP export
  (emit-on-end) cannot provide — the server must expose an in-process
  **span-lifecycle hook**.
- Control (reload, toggle, room lifecycle) is a **write** telemetry cannot perform
  — the game loop must expose **command primitives**.

This stub reserves that scope so the boundary is designed for, not bolted on — and
so it stays **server-owned** and never widens the player `wire-protocol`.

## What Changes

- **OTEL span pipeline:** bridge the existing `tracing` instrumentation to
  OpenTelemetry (`tracing-opentelemetry` + `opentelemetry` + `opentelemetry-otlp`),
  emitting the documented span tree (room → game → round → wave → commit/resolve →
  score, plus `ws.message`, `reconnect`, `db.write`) with stable, versioned
  attribute names.
- **Secret attributes + redacting exporter:** carry secret game state in span
  attributes in-process; an **allow-list** redaction layer at the OTLP exporter
  strips them before any span leaves the process. Redaction is a tested,
  security-critical control with a single authoritative secret-attribute set.
- **In-process span-lifecycle hook:** expose span start/end (`on_start`/`on_end`,
  equivalently a `tracing` `Layer`) so `admin-ui`'s projection can maintain a live
  open-span registry and unsampled aggregates upstream of export sampling.
- **Command-plane primitives:** authoritative game-loop operations for
  validated config reload, per-item enable/disable, and room lifecycle
  (seed / force-start / kill), invoked only over the admin command API — never the
  player wire.

## Capabilities

### New Capabilities
> Stubs — to be detailed when this change is promoted.
- `otel-span-pipeline`: `tracing`→OTEL bridge emitting the versioned span tree and
  attributes; OTLP export wired (trace backend deferred per `admin-ui` R2).
- `telemetry-redaction`: allow-list redaction of secret span attributes at the
  export boundary; the authoritative secret-attribute set; tested as a security
  control.
- `span-lifecycle-hook`: an in-process consumer seam exposing span start/end for
  the `admin-ui` projection without backpressuring the game loop.
- `admin-command-primitives`: authoritative game-loop operations for reload,
  toggle, and room lifecycle, exposed only to the admin command API.

### Modified Capabilities
- `persistence-and-observability` (owned by `server-release-1`): its **Structured
  Tracing** requirement (today `tracing` JSON) becomes OTEL spans. Because that
  change is still in-progress and unarchived, the modification is reserved here and
  may instead be applied directly in `server-release-1` at promotion (R7).

## Impact

- **New code (server):** OTEL setup in the `observability/` module, the redacting
  exporter, the span-lifecycle hook, and command primitives on the room registry /
  config module.
- **New dependencies:** `opentelemetry`, `opentelemetry-otlp`,
  `tracing-opentelemetry` (bridging existing `tracing`).
- **Hard boundary (Constitution I):** secrets ride in spans in-process only and are
  redacted at export; command primitives are reachable only via the admin command
  API, never the player `wire-protocol`.
- **Blocks:** `admin-ui`. **Depends on:** `server-release-1` (observability, room
  registry, content config).
