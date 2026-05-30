//! Observability: structured JSON tracing and Prometheus game-balance metrics.
//!
//! [`init`] wires both up at startup. The [`metric`] helpers name the counters
//! and gauges the rest of the server increments, so instrumentation call sites
//! stay terse and the metric names live in one place.

use std::net::SocketAddr;

use metrics_exporter_prometheus::PrometheusBuilder;

/// Initialise JSON tracing and install a Prometheus exporter listening on
/// `metrics_addr` (e.g. `0.0.0.0:9090`). Call once at startup.
pub fn init(metrics_addr: SocketAddr) {
    tracing_subscriber::fmt()
        .json()
        .with_target(true)
        .with_current_span(true)
        .init();
    if let Err(e) = PrometheusBuilder::new()
        .with_http_listener(metrics_addr)
        .install()
    {
        tracing::warn!(error = %e, "failed to install Prometheus exporter");
    } else {
        tracing::info!(%metrics_addr, "metrics exporter listening");
    }
}

/// The game-balance metrics, named in one place. These compile to no-ops when no
/// recorder is installed (e.g. in tests).
pub mod metric {
    /// A room was created.
    pub fn room_created() {
        metrics::counter!("rooms_created_total").increment(1);
        metrics::gauge!("rooms_active").increment(1.0);
    }

    /// A room task ended.
    pub fn room_closed() {
        metrics::gauge!("rooms_active").decrement(1.0);
    }

    /// A game began.
    pub fn game_started() {
        metrics::counter!("games_started_total").increment(1);
    }

    /// A game completed.
    pub fn game_completed() {
        metrics::counter!("games_completed_total").increment(1);
    }

    /// A round resolved; `exploded` feeds the explosion-rate (~30–40% target).
    pub fn round_resolved(exploded: bool) {
        metrics::counter!("rounds_total").increment(1);
        if exploded {
            metrics::counter!("round_explosions_total").increment(1);
        }
    }
}
