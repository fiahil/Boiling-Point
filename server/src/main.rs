//! Server bootstrap: parse the CLI, validate the content config, then serve the
//! WebSocket API.
//!
//! Fail-fast: an invalid content config aborts startup before any port is bound.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use boiling_point_server::admin::{self, AdminProjection, AdminState, OperatorAuth};
use boiling_point_server::config::ContentConfig;
use boiling_point_server::lobby::{GroupRegistry, MatchQueue, SessionStore};
use boiling_point_server::observability;
use boiling_point_server::transport::{AppState, app};
use clap::Parser;

/// The default content config, embedded so the binary always has a valid baseline.
const DEFAULT_CONFIG: &str = include_str!("../content.toml");

/// Command-line options for the Boiling Point server.
#[derive(Debug, Parser)]
#[command(
    name = "boiling-point-server",
    version,
    about = "Boiling Point authoritative game server (player WebSocket + admin API + metrics)."
)]
struct Cli {
    /// Listen address for the player WebSocket wire.
    #[arg(long, value_name = "ADDR", default_value = "0.0.0.0:8080")]
    ws_addr: SocketAddr,
    /// Listen address for the Prometheus metrics exporter.
    #[arg(long, value_name = "ADDR", default_value = "0.0.0.0:9090")]
    metrics_addr: SocketAddr,
    /// Listen address for the operator-only admin API (isolated from the player wire).
    #[arg(long, value_name = "ADDR", default_value = "0.0.0.0:8081")]
    admin_addr: SocketAddr,
    /// Content config TOML to load. Defaults to the embedded baseline config.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// JSON-log verbosity (e.g. `info`, `debug`). Overrides `RUST_LOG` when set.
    #[arg(long, value_name = "LEVEL")]
    log_level: Option<String>,
    /// Idle/grace timeout for a player connection, in seconds.
    #[arg(long, value_name = "SECS", default_value_t = 90)]
    conn_timeout_secs: u64,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    observability::init(cli.metrics_addr, cli.log_level.as_deref());

    // The admin read projection consumes the span lifecycle (registered before the
    // game loop runs so it observes 100% of spans, upstream of export sampling).
    let projection = Arc::new(AdminProjection::new());
    observability::lifecycle::register_consumer(projection.clone());

    // Periodically reap open-span registry entries whose span-end was missed,
    // bounding projection memory (a generous multiple of any legitimate lifetime).
    {
        let projection = projection.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                tick.tick().await;
                let reaped = projection.reap_stale(std::time::Duration::from_secs(30 * 60));
                if reaped > 0 {
                    tracing::warn!(reaped, "reaped stale open spans (missed span-end)");
                }
            }
        });
    }

    let config_text = match &cli.config {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) => {
                eprintln!("failed to read content config {}: {e}", path.display());
                std::process::exit(1);
            }
        },
        None => DEFAULT_CONFIG.to_string(),
    };
    let config = match ContentConfig::from_toml(&config_text) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to parse content config: {e}");
            std::process::exit(1);
        }
    };
    // Build (and thereby validate) the registry; abort on an invalid config.
    let registry = match config.build_registry() {
        Ok(r) => Arc::new(r),
        Err(e) => {
            eprintln!("invalid content config: {e}");
            std::process::exit(1);
        }
    };
    let config = Arc::new(config);

    let groups = Arc::new(GroupRegistry::new(registry, config));
    let queue = Arc::new(MatchQueue::new(groups.clone()));
    // Wire the queue back into the registry so groups can request matchmaking fill.
    groups.set_queue(&queue);

    // Admin surface: served on an isolated port, distinct from the player wire.
    // Operators authenticate with bearer tokens from the environment, never the
    // anonymous player session tokens.
    let admin_auth = Arc::new(OperatorAuth::from_env());
    if admin_auth.is_empty() {
        tracing::warn!(
            "no admin operator tokens configured (set BP_ADMIN_TOKEN / \
             BP_ADMIN_OBSERVER_TOKEN); the admin API will reject every request"
        );
    }
    let admin_state = AdminState {
        projection: projection.clone(),
        auth: admin_auth,
        groups: groups.clone(),
    };
    let admin_addr = cli.admin_addr;
    match tokio::net::TcpListener::bind(admin_addr).await {
        Ok(listener) => {
            tracing::info!("Boiling Point admin API on http://{admin_addr}/admin/");
            tokio::spawn(async move {
                if let Err(e) = axum::serve(listener, admin::app(admin_state)).await {
                    tracing::error!(error = %e, "admin server error");
                }
            });
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to bind admin {admin_addr}; admin API disabled")
        }
    }

    let state = AppState {
        sessions: Arc::new(SessionStore::new()),
        groups,
        queue,
        conn_timeout: Duration::from_secs(cli.conn_timeout_secs),
    };

    let addr = cli.ws_addr;
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("failed to bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!("Boiling Point server listening on ws://{addr}/ws");
    if let Err(e) = axum::serve(listener, app(state)).await {
        tracing::error!(error = %e, "server error");
        std::process::exit(1);
    }
}
