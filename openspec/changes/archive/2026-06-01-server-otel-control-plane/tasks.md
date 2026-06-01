## 1. Dependencies & span-schema contract

- [x] 1.1 Add `opentelemetry = "0.31"`, `opentelemetry_sdk = "0.31"`, `opentelemetry-otlp = "0.31"`, `tracing-opentelemetry = "0.32"` (and `arc-swap` if used) to `server/Cargo.toml`; `cargo build` resolves.
- [x] 1.2 Create `observability::span_schema`: `SPAN_SCHEMA_VERSION`, the span-name constants and their parent→child hierarchy, the public attribute-key allow-list, and the secret-attribute set (boiling point, committed cards, hands, mid-round volatility) — all as one source of truth.
- [x] 1.3 Unit-test the contract: public and secret key sets are disjoint, the hierarchy names are internally consistent, and the version is exposed.

## 2. OTEL pipeline (tracing → OTEL bridge + OTLP export)

- [x] 2.1 Refactor `observability::init` to build a layered `tracing_subscriber::registry()` composed of the existing JSON `fmt` layer plus an `OpenTelemetryLayer`, keeping the Prometheus exporter wiring intact.
- [x] 2.2 Build the OTEL `TracerProvider`/exporter behind a configurable OTLP endpoint; ensure init never blocks on an unreachable backend and export failures are logged, not fatal.
- [x] 2.3 Test that `init` succeeds and the game loop runs with no OTLP endpoint reachable (Prometheus + logs still work).

## 3. Redacting exporter (security control)

- [x] 3.1 Implement `RedactingExporter` wrapping the OTLP `SpanExporter`: filter each span's attributes to the public allow-list (fail-closed), then delegate.
- [x] 3.2 Wire `RedactingExporter` into the export path so all exported spans pass through it.
- [x] 3.3 Security test: build span data populated with every key in the secret-attribute set, run it through `RedactingExporter` with a capturing inner exporter, and assert no secret key appears in the output while public keys survive.

## 4. Span instrumentation (emit the documented span tree)

- [x] 4.1 Instrument the room actor (`lobby::room::run`) with a `room.lifetime` span carrying the room code; assert it opens on create and ends on room close/kill.
- [x] 4.2 Instrument `session::run_game` with nested `game` → `round` → `wave` spans carrying the live-registry keys (game/round/wave ids + numbers) and the secret attributes (boiling point, volatility) in-process.
- [x] 4.3 Add `commit`, `resolve`, and `score` leaf spans under the correct parent, plus `ws.message` (in `transport`), `reconnect` (in `session`), and `db.write` (in `persistence`).
- [x] 4.4 Test that the documented spans are emitted with the expected names, nesting, and attribute keys (observed via the lifecycle consumer from §5).

## 5. Span-lifecycle hook (consumer seam)

- [x] 5.1 Define the `SpanEvent { id, name, parent_id, attributes, kind }` type and a consumer-registration seam (trait or bounded lossy channel) in `observability`.
- [x] 5.2 Implement the lifecycle `tracing` `Layer` (`on_new_span`/`on_record`/`on_close`) that reads attributes via a visitor and forwards `SpanEvent`s to the registered consumer via `try_send`, dropping/coalescing on a full buffer.
- [x] 5.3 Add the lifecycle layer to the subscriber registry so it observes 100% of spans, upstream of OTEL export sampling.
- [x] 5.4 Test: a registered consumer sees a span's start and end with attributes; a deliberately slow/full consumer does not block span emission or the game loop.

## 6. Command primitives

- [x] 6.1 Make `RoomRegistry`'s shared config/registry swappable (`ArcSwap`/`RwLock<Arc>`); `create()` snapshots the current config at spawn. Existing room/transport tests stay green.
- [x] 6.2 Implement `reload(toml, operator)` reusing `ContentConfig::from_toml`/`validate`/`build_registry`: atomic swap on success, unchanged config + returned `ConfigError` on failure.
- [x] 6.3 Implement `toggle_item(selector, enabled, operator)`: clone live config, flip one item's `enabled`, re-validate and swap; reject (unchanged) if it fails validation.
- [x] 6.4 Add `RoomCommand::Shutdown` and `RoomCommand::ForceStart` to the room actor and `kill_room`/`force_start`/`seed_room` on `RoomRegistry`, delivering commands through the room's channel.
- [x] 6.5 Emit an `admin.command` audit span (operator, action, target, outcome) for every primitive invocation.
- [x] 6.6 Tests: invalid reload/toggle rejected and config unchanged; valid reload/toggle applies and a new room reflects it; `kill_room` ends the `room.lifetime` span (observed via the lifecycle consumer); `force_start` starts the game; no `ClientMessage` path reaches any primitive.

## 7. Modified spec & docs

- [x] 7.1 Confirm the `persistence-and-observability` "Structured Tracing" delta matches the implemented span tree; update module docs to document the span tree and `SPAN_SCHEMA_VERSION`.

## 8. Verification

- [x] 8.1 `cargo build`, `cargo test`, `cargo clippy --all-targets`, and `cargo fmt --check` all pass for the server crate.
