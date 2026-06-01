## Context

`admin-ui` (research R1–R8) decided its entire read surface is **span-sourced**: an
in-process projection consumes the server's span lifecycle to serve the room
inspector, the godmode reveal, replay, and live balance figures, while control is a
separate command channel. That presupposes server-owned machinery the server does
not have today:

- `server/src/observability.rs` wires `tracing_subscriber::fmt().json()` and a
  Prometheus exporter. It emits **events** (`tracing::info!`) and metric counters —
  **there are no spans anywhere in the codebase**. "Spans as a source" therefore
  needs both a `tracing`→OTEL bridge *and* real span instrumentation.
- The game loop (`session::run_game`) holds the secret state the reveal needs
  (boiling point, hands, committed cards, mid-round volatility), but never attaches
  it to telemetry.
- Rooms are actor tasks reached only via `RoomCommand` (`Join`/`Leave`/`Action`) on
  an mpsc channel; `RoomRegistry` hands each room an `Arc<ContentConfig>` and
  `Arc<ContentRegistry>` captured at spawn. There is no control path and no way to
  swap config.

`server-release-1` is now **archived**, so `persistence-and-observability` is the
canonical capability in `openspec/specs/`; its **Structured Tracing** requirement is
modified here (the fork the stub left open is resolved in favour of a delta in this
change).

## Goals / Non-Goals

**Goals:**
- Bridge existing `tracing` to OpenTelemetry, additively (Prometheus + JSON logs
  keep working).
- Instrument the game loop to emit the documented span tree with stable, versioned
  attribute names, defined once in a span-schema contract.
- Carry secret state in span attributes in-process; redact it with an allow-list at
  the export boundary; prove it with a security test.
- Expose an in-process span-lifecycle seam (start/end + attributes) that sees 100%
  of spans and never backpressures the game loop.
- Provide server-owned command primitives (reload, toggle, seed/force-start/kill)
  with audit spans, callable by the admin command plane, unreachable from the
  player wire.

**Non-Goals:**
- The admin projection itself, the open-span registry, rolling aggregates, and the
  replay buffer — those are `admin-ui`'s `admin-span-projection`. This change ships
  the *seam* they consume, plus a minimal in-tree consumer for tests.
- The admin HTTP API, auth, and operator roles — `admin-ui`'s `admin-auth` /
  `admin-control`. This change ships the *primitives* the API calls.
- Standing up a Tempo/Jaeger backend (R2: export seam wired, backend deferred).
- Widening the player `wire-protocol`. No new `ClientMessage`/`ServerMessage`.

## Decisions

### D1: One instrumentation surface — `tracing` spans bridged to OTEL

Instrument with `tracing` spans (`info_span!` / `#[instrument]`) and bridge to OTEL
via `tracing-opentelemetry`'s `OpenTelemetryLayer`. `tracing` already exists in the
tree; adding a second native-OTEL API would split instrumentation.

- *Alternative — native `opentelemetry` API at call sites:* rejected; duplicates the
  instrumentation surface and abandons the existing `tracing` ergonomics/macros.

### D2: Layered subscriber with three independent consumers

Replace the single `fmt().json()` subscriber with a `tracing_subscriber::registry()`
composed of: (1) the JSON `fmt` layer (kept, for debugging), (2) the
`OpenTelemetryLayer` → OTLP export path (with its own sampler + redaction), and
(3) a custom **lifecycle `Layer`** that feeds the in-process consumer seam. Layering
lets each consumer have independent sampling: the lifecycle layer is upstream of
OTEL entirely, so it sees 100% of spans regardless of export sampling (R6).

- *Alternative — OTEL `SpanProcessor` for the lifecycle hook:* viable (research R5
  names it as equivalent), but a `SpanProcessor` only receives spans the OTEL
  `Sampler` kept, so "unsampled aggregates" would force `Sampler::AlwaysOn` and lose
  the ability to sample exports. A `tracing` `Layer` sits above OTEL sampling and is
  the cleaner fit for "100% in-process."

### D3: Span-schema contract as the single source of names + secret set

A new `observability::span_schema` module exposes `const SPAN_SCHEMA_VERSION`, the
span names, their hierarchy, the **public attribute allow-list**, and the
**secret-attribute set** as `&'static str` constants/sets. Redaction and (later) the
projection both read from here — no duplicated string literals.

### D4: Redaction = allow-list, fail-closed, at the export boundary

A `RedactingExporter` wraps the OTLP `SpanExporter`: before delegating, it filters
each span's attributes to those whose keys are on the public allow-list, dropping
everything else (secrets *and* anything unknown). Fail-closed satisfies R3's
security framing — a newly added secret stays redacted even before the schema is
updated. A security test (D8) asserts no secret key survives.

- *Alternative — deny-list (strip the enumerated secrets):* simpler and keeps OTEL
  infra attributes for trace UX, but fail-open (a new secret leaks until listed).
  Rejected for the security boundary; the cost is that OTEL infra attributes
  (`code.*`, `thread.*`, busy/idle) are also stripped unless added to the allow-list,
  which we accept for v1 (span names + hierarchy + public game attrs remain).

### D5: Lifecycle seam is a non-blocking, read-only consumer registration

The lifecycle `Layer` forwards `SpanEvent { id, name, parent_id, attributes, kind:
Start|End }` to a registered consumer behind a **bounded, lossy** channel
(`try_send`; drop/coalesce on full). The seam exposes only owned, observed data
(`SpanEvent`) — no handle to game state — so it is read-only by construction and
cannot backpressure the emitter (R5/R8, `admin-span-projection` "read-only",
"slow projection does not stall"). A trivial in-tree consumer is used in tests; the
real projection is `admin-ui`'s.

### D6: Command primitives on a swappable config holder + new RoomCommands

- **Config reload / toggle:** `RoomRegistry` stores the shared config/registry in
  `arc_swap::ArcSwap` (or `RwLock<Arc<…>>`) instead of a plain `Arc`. `create()`
  snapshots the *current* config when spawning a room. `reload(toml)` parses →
  `validate()` → `build_registry()`; on success swaps both atomically; on failure
  returns the existing `ConfigError` unchanged. `toggle_item(selector, enabled)`
  clones the live `ContentConfig`, flips one item's `enabled`, then runs the same
  validate→build→swap. Reuses the existing fail-fast validation verbatim — invalid
  never partially applies; effect is on **future** rooms (matches `admin-control`
  "subsequent deals exclude that card").
- **Room lifecycle:** add `RoomCommand::Shutdown` (kill) and `RoomCommand::ForceStart`
  to the room actor; `seed` reuses `create()`. `RoomRegistry` gains
  `kill_room(code)` / `force_start(code)` / `seed_room()` that look up the room's
  sender and deliver the command, so teardown/start run through the **authoritative
  loop** and the `room.lifetime` span ends naturally on kill (Constitution I,
  `admin-control` "kill ends room.lifetime span").

- *Alternative — mutate `DashMap`/config behind the loop:* rejected; races the actor
  and violates "act through the game loop."

### D7: Audit spans for every primitive

Each primitive opens an `admin.command` span carrying `operator`, `action`,
`target`, and records `outcome` (`ok` / rejection reason). Operator identity is a
parameter the primitive receives from the (admin-ui-owned) command API; this change
threads it into the span and defaults to a placeholder in tests.

### D8: Versions pinned to the 0.31 OTEL line

`opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp` = `0.31`;
`tracing-opentelemetry` = `0.32` (it runs one minor ahead and targets the 0.31 OTEL
line). All OTEL crates pinned to the same minor to avoid type-mismatch across the
0.x boundary.

## Constitution Check

- **I. Server-Authoritative:** Reinforced. Secrets ride in spans in-process only and
  are redacted at export (never to a trace store, never to the wire). Command
  primitives act only through the room actor; no game logic moves to any client; no
  `ClientMessage` can invoke a primitive. The lifecycle seam is read-only.
- **II. Agent-Driven Development:** All changes are source-level and testable
  headlessly: redaction has a unit security test; the lifecycle seam has a test
  consumer; command primitives have actor-level tests; span emission is asserted via
  the lifecycle consumer. No GUI-only state.
- **III. Start Simple, Scale Later:** In-process seam now; OTLP export *seam* wired
  but backend deferred (R2). We add OTEL deps and a swappable config holder — each
  justified against the simpler rejected alternative above (D1, D2, D4, D6). The
  projection/registry/replay and the HTTP/auth surface are explicitly **not** built
  here.
- **IV. Playtest-Driven Balance:** The unsampled lifecycle stream (D2) preserves
  exact balance aggregates upstream of sampling; Prometheus (also unsampled) stays
  the durable store. No balance numbers are introduced or changed.

*Deviations:* none. The one notable trade-off (allow-list strips OTEL infra
attributes, D4) is a documented risk, not a constitutional deviation.

## Risks / Trade-offs

- **Heavy new dependency tree (tonic/prost via opentelemetry-otlp)** → use the OTLP
  exporter's HTTP/protobuf or tonic transport with default features trimmed; tolerate
  longer first build. Mitigation: pin the 0.31 line (D8); keep OTEL setup in one
  module.
- **Allow-list strips OTEL infra attributes (D4)** → traces keep names, hierarchy,
  and public game attrs; if trace UX later needs `code.*`/`thread.*`, add them to the
  allow-list. The security property (no secret leaves) holds regardless.
- **Config swap only affects future rooms** → live rooms keep the config they
  captured at spawn. This matches the spec ("subsequent deals"), and avoids
  mutating a running game's content mid-round (Constitution I). Documented, not
  hidden.
- **Lifecycle layer reads span attributes via a visitor on every span** → cost is
  per-span allocation of the event. Mitigation: the bounded lossy channel (D5) caps
  consumer cost; the visitor only copies the small, fixed attribute set the schema
  defines.
- **OTLP batch exporter with no backend** → the batch processor retries/drops on
  connection failure. Mitigation: build the provider so init never blocks on a
  connection and export errors are logged, not fatal (otel-span-pipeline "runs with
  no backend").

## Migration Plan

1. Add deps; introduce `span_schema` + `RedactingExporter` + lifecycle `Layer`
   behind the existing `observability::init` signature (extended with an optional
   OTLP endpoint / consumer registration), so `main.rs` wiring changes minimally.
2. Instrument room/session/transport/persistence incrementally; each span lands with
   a test through the lifecycle consumer.
3. Make `RoomRegistry` config holder swappable and add the primitives; existing room
   tests stay green because `create()` keeps its signature.
4. No data migration. Rollback = drop the OTLP layer wiring; the JSON-log + metrics
   surface (the pre-change behaviour) is retained throughout.

## Open Questions

- **Operator identity source:** threaded as a parameter now; its real provenance
  (token/role) is `admin-ui`'s `admin-auth`. Placeholder until then.
- **Export sampler policy:** default `AlwaysOn` for v1 (no backend); a real sampler
  is chosen when a backend lands (R2). The lifecycle stream is unaffected either way.
- **`ArcSwap` vs `RwLock<Arc>`** for the config holder: `ArcSwap` is lock-free and
  preferred; if avoiding the dependency is desired, `RwLock<Arc<…>>` is an acceptable
  fallback. Decided at implementation by whether `arc-swap` is already pulled in.
