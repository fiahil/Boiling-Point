//! Observability: structured JSON tracing, a `tracing`→OpenTelemetry span bridge,
//! Prometheus game-balance metrics, and the in-process span-lifecycle hook.
//!
//! [`init`] composes a layered `tracing` subscriber:
//! 1. an env-filtered JSON `fmt` layer (the existing structured logs),
//! 2. an OpenTelemetry layer exporting to OTLP (wired only when an endpoint is
//!    configured — backend deferred, R2), and
//! 3. the [`lifecycle`] layer feeding the in-process consumer seam — upstream of any
//!    export sampling, so the `admin-ui` projection sees 100% of spans.
//!
//! The span tree and attribute names live in [`span_schema`] (version
//! [`span_schema::SPAN_SCHEMA_VERSION`]); the projection reads from there. Spans
//! carry sensitive game state and are exported as-is to the trusted, operator-only
//! trace backend — the trust boundary that matters is the player wire, which the
//! admin channel never touches, so there is no export-time redaction.

use std::net::SocketAddr;
use std::sync::OnceLock;

use metrics_exporter_prometheus::PrometheusBuilder;
use tracing_subscriber::prelude::*;

pub mod balance_metrics;
pub mod lifecycle;
pub mod span_schema;

/// Env var naming the OTLP endpoint (e.g. `http://localhost:4317`). When unset, the
/// OTLP export layer is not installed — spans still flow to logs, Prometheus, and
/// the in-process lifecycle consumer, but nothing is exported (R2: backend deferred).
const OTLP_ENDPOINT_ENV: &str = "BP_OTLP_ENDPOINT";

/// Holds the tracer provider for the process lifetime so its batch exporter keeps
/// running (dropping it would shut the export pipeline down).
static PROVIDER: OnceLock<opentelemetry_sdk::trace::SdkTracerProvider> = OnceLock::new();

/// Initialise logging, the OTEL span bridge, the lifecycle hook, and Prometheus.
/// Call once at startup. Never fails on a missing trace backend.
///
/// `log_level`, when `Some`, sets the JSON-log verbosity (e.g. `"info"`,
/// `"debug"`, `"boiling_point_server=debug,info"`) and takes precedence over the
/// `RUST_LOG` environment variable; when `None` the level falls back to `RUST_LOG`
/// and then to `info`. Invalid directives are ignored (lossy parse) rather than
/// aborting startup.
pub fn init(metrics_addr: SocketAddr, log_level: Option<&str>) {
    use tracing_subscriber::Layer as _;
    use tracing_subscriber::filter::LevelFilter;

    let env_filter = match log_level {
        Some(level) => tracing_subscriber::EnvFilter::new(level),
        None => tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
    };

    // The OTEL layer is present only when an OTLP endpoint is configured; an
    // `Option<Layer>` is itself a `Layer` (a no-op when `None`).
    let otel_layer =
        build_otel_tracer().map(|tracer| tracing_opentelemetry::layer().with_tracer(tracer));

    // The `RUST_LOG` level filter gates the JSON **logs only** (attached per-layer to
    // the fmt layer). The in-process lifecycle hook must observe 100% of the server's
    // spans regardless of log verbosity — the admin projection's unsampled accuracy
    // (Principle IV) and the live inspector depend on it, so `RUST_LOG=warn` must not
    // blind the admin surface. The lifecycle layer therefore carries its own always-on
    // `INFO` filter (every game span is `info_span!`), independent of `RUST_LOG`.
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_filter(env_filter),
        )
        .with(otel_layer)
        .with(
            lifecycle::global_handle()
                .layer()
                .with_filter(LevelFilter::INFO),
        )
        .init();

    install_prometheus(metrics_addr);
}

/// Install the Prometheus exporter (unchanged from the pre-OTEL behaviour).
fn install_prometheus(metrics_addr: SocketAddr) {
    if let Err(e) = PrometheusBuilder::new()
        .with_http_listener(metrics_addr)
        .install()
    {
        tracing::warn!(error = %e, "failed to install Prometheus exporter");
    } else {
        tracing::info!(%metrics_addr, "metrics exporter listening");
    }
}

/// Build the OTLP-backed tracer. Returns `None` (and logs) when no endpoint is
/// configured or the exporter cannot be built — startup proceeds regardless
/// (otel-span-pipeline: runs with no backend).
fn build_otel_tracer() -> Option<opentelemetry_sdk::trace::SdkTracer> {
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_otlp::WithExportConfig as _;

    let endpoint = std::env::var(OTLP_ENDPOINT_ENV).ok()?;

    let exporter = match opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&endpoint)
        .build()
    {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = %e, endpoint, "OTLP exporter unavailable; spans not exported");
            return None;
        }
    };

    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_resource(
            opentelemetry_sdk::Resource::builder()
                .with_service_name("boiling-point-server")
                .build(),
        )
        .with_batch_exporter(exporter)
        .build();

    let tracer = provider.tracer("boiling-point-server");
    let _ = PROVIDER.set(provider);
    tracing::info!(endpoint, "OTLP span export enabled");
    Some(tracer)
}

/// The live Prometheus emitter, re-pointed at the [`balance_metrics`]
/// definitions: every series it writes is named in
/// [`balance_metrics::series`], and every balance figure is derived from a
/// [`balance_metrics::BalanceEvent`] — no formula lives here. The v1 balance
/// series ([`balance_metrics::RETIRED_V1_SERIES`]) are never emitted. These
/// compile to no-ops when no recorder is installed (e.g. in tests).
pub mod metric {
    use super::balance_metrics::{BalanceEvent, series};

    /// A group was created (fleet figures, carried over from v1).
    pub fn group_created() {
        metrics::counter!(series::GROUPS_CREATED).increment(1);
        metrics::gauge!(series::GROUPS_ACTIVE).increment(1.0);
    }

    /// A group task ended.
    pub fn group_closed() {
        metrics::gauge!(series::GROUPS_ACTIVE).decrement(1.0);
    }

    /// A game began.
    pub fn game_started() {
        metrics::counter!(series::GAMES_STARTED).increment(1);
        metrics::gauge!(series::GAMES_ACTIVE).increment(1.0);
    }

    /// A player connection opened (the connected-players fleet gauge).
    pub fn connection_opened() {
        metrics::gauge!(series::PLAYERS_CONNECTED).increment(1.0);
    }

    /// A player connection closed.
    pub fn connection_closed() {
        metrics::gauge!(series::PLAYERS_CONNECTED).decrement(1.0);
    }

    /// Record one balance event onto its Prometheus series. The same events feed
    /// the admin projection's aggregates, so the dashboard's two sources (these
    /// series via Grafana, the projection via the admin API) cannot disagree
    /// about what an event means.
    pub fn record(ev: &BalanceEvent) {
        match ev {
            BalanceEvent::GameCompleted { duration_ms } => {
                metrics::counter!(series::GAMES_COMPLETED).increment(1);
                metrics::gauge!(series::GAMES_ACTIVE).decrement(1.0);
                metrics::histogram!(series::GAME_DURATION).record(*duration_ms as f64 / 1000.0);
            }
            BalanceEvent::RoundSettled {
                boomed,
                frozen,
                duration_ms,
            } => {
                metrics::counter!(series::ROUNDS).increment(1);
                if *boomed {
                    metrics::counter!(series::ROUNDS_BOOMED).increment(1);
                }
                if *frozen {
                    metrics::counter!(series::ROUNDS_FROZEN).increment(1);
                }
                metrics::histogram!(series::ROUND_DURATION).record(*duration_ms as f64 / 1000.0);
            }
            BalanceEvent::Boom { detonators } => {
                metrics::counter!(series::BOOM_DETONATORS).increment(*detonators);
            }
            BalanceEvent::WaveResolved {
                timed_out,
                commits,
                folds,
                duration_ms,
            } => {
                metrics::counter!(series::WAVES).increment(1);
                if *timed_out {
                    metrics::counter!(series::WAVE_TIMEOUTS).increment(1);
                }
                metrics::counter!(series::WAVE_COMMITS).increment(*commits);
                metrics::counter!(series::WAVE_FOLDS).increment(*folds);
                metrics::histogram!(series::WAVE_DURATION).record(*duration_ms as f64 / 1000.0);
            }
            BalanceEvent::SpellCast { kind } => {
                metrics::counter!(series::SPELL_CASTS, series::SPELL_LABEL => kind.clone())
                    .increment(1);
            }
            BalanceEvent::PlayerReconnected => {
                metrics::counter!(series::PLAYER_RECONNECTS).increment(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With no OTLP endpoint configured, the export layer is simply absent — the
    /// server still logs, exports metrics, and feeds the lifecycle consumer. (The
    /// full "game runs with no backend" path is covered by the transport tests,
    /// which run complete games without ever wiring OTLP.)
    #[test]
    fn no_otlp_layer_without_endpoint() {
        if std::env::var(OTLP_ENDPOINT_ENV).is_err() {
            assert!(
                build_otel_tracer().is_none(),
                "no endpoint configured, so no tracer should be built"
            );
        }
    }
}
