//! Server bootstrap: validate the content config, then serve the WebSocket API.
//!
//! Fail-fast: an invalid content config aborts startup before any port is bound.

use std::sync::Arc;

use boiling_point_server::config::ContentConfig;
use boiling_point_server::lobby::{RoomRegistry, SessionStore};
use boiling_point_server::transport::{app, AppState};

/// The default content config, embedded so the binary always has a valid baseline.
const DEFAULT_CONFIG: &str = include_str!("../content.toml");

#[tokio::main]
async fn main() {
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

    let state = AppState {
        sessions: Arc::new(SessionStore::new()),
        rooms: Arc::new(RoomRegistry::new(registry, config)),
    };

    let addr = "0.0.0.0:8080";
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("failed to bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    println!("Boiling Point server listening on ws://{addr}/ws");
    if let Err(e) = axum::serve(listener, app(state)).await {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
}
