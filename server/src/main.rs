//! Server bootstrap: validate the content config, then serve the WebSocket API.
//!
//! Fail-fast: an invalid content config aborts startup before any port is bound.

use std::sync::Arc;

use boiling_point_server::config::ContentConfig;
use boiling_point_server::lobby::{MatchQueue, RoomRegistry, SessionStore};
use boiling_point_server::observability;
use boiling_point_server::transport::{AppState, app};

/// The default content config, embedded so the binary always has a valid baseline.
const DEFAULT_CONFIG: &str = include_str!("../content.toml");

#[tokio::main]
async fn main() {
    observability::init(
        "0.0.0.0:9090"
            .parse()
            .expect("valid metrics listen address"),
    );

    let config = match ContentConfig::from_toml(DEFAULT_CONFIG) {
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

    let rooms = Arc::new(RoomRegistry::new(registry, config));
    let queue = Arc::new(MatchQueue::new(rooms.clone()));
    let state = AppState {
        sessions: Arc::new(SessionStore::new()),
        rooms,
        queue,
        conn_timeout: std::time::Duration::from_secs(90),
    };

    let addr = "0.0.0.0:8080";
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
