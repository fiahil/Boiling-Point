//! Server bootstrap: parse the CLI, validate the content config, then serve the
//! WebSocket API.
//!
//! Fail-fast: an invalid content config aborts startup before any port is bound.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use boiling_point_protocol::AccountId;
use boiling_point_server::admin::{self, AdminProjection, AdminState, OperatorAuth};
use boiling_point_server::config::ContentConfig;
use boiling_point_server::lobby::accounts::{
    AccountRecord, AccountStore, DisabledOAuthVerifier, HttpOAuthVerifier, OAuthVerifier,
};
use boiling_point_server::lobby::{GroupRegistry, MatchQueue, SessionStore, SkillBased};
use boiling_point_server::observability;
use boiling_point_server::persistence;
use boiling_point_server::rating::{RatingStore, Skill};
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
    /// PostgreSQL connection URL enabling post-game persistence (results +
    /// replays). When unset, the server runs normally and persists nothing —
    /// persistence is optional infrastructure, not a precondition for play.
    #[arg(long, value_name = "URL", env = "DATABASE_URL")]
    database_url: Option<String>,
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

    // Optional persistence: when a database URL is configured, connect a pool
    // and apply pending migrations before accepting connections. Its absence
    // disables persistence cleanly (logged once here); a configured-but-
    // unreachable database is an operator error and aborts startup (fail fast,
    // matching the content-config gate).
    let pool = match &cli.database_url {
        Some(url) => {
            let pool = match boiling_point_server::persistence::connect(url).await {
                Ok(pool) => pool,
                Err(e) => {
                    eprintln!("failed to connect to database: {e}");
                    std::process::exit(1);
                }
            };
            if let Err(e) = boiling_point_server::persistence::run_migrations(&pool).await {
                eprintln!("failed to run database migrations: {e}");
                std::process::exit(1);
            }
            tracing::info!("persistence enabled; migrations applied before serving");
            Some(pool)
        }
        None => {
            tracing::info!(
                "no database configured (set --database-url / DATABASE_URL); \
                 persistence disabled — games still play normally"
            );
            None
        }
    };

    // The identity stack (`boom2-identity`). OAuth is the heaviest dependency and
    // opt-in: with `BP_OAUTH_ENABLED` unset, OAuth sign-in is disabled (the HTTP
    // verifier is never constructed) while device-bound and anonymous play work
    // unchanged. The account and rating stores are shared (one `Arc` each) across
    // the registry, queue, and transport so identity is consistent everywhere.
    let oauth_enabled = std::env::var("BP_OAUTH_ENABLED")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let verifier: Arc<dyn OAuthVerifier> = if oauth_enabled {
        tracing::info!("OAuth sign-in enabled (Google/Discord)");
        Arc::new(HttpOAuthVerifier::new())
    } else {
        tracing::info!("OAuth sign-in disabled (set BP_OAUTH_ENABLED=1 to enable)");
        Arc::new(DisabledOAuthVerifier)
    };
    let accounts = Arc::new(AccountStore::with_verifier(verifier).with_pool(pool.clone()));
    let ratings = Arc::new(RatingStore::default());

    // Hydrate accounts + ratings from durable storage so identities and skill
    // survive a restart. Best effort: a load failure logs and leaves the stores
    // empty (anonymous play is unaffected).
    if let Some(p) = &pool {
        match persistence::load_accounts(p).await {
            Ok(rows) => {
                for (id, player_id, kind, device_token, oauth_provider, oauth_subject) in rows {
                    accounts.hydrate(AccountRecord {
                        id,
                        player_id,
                        kind,
                        device_token,
                        oauth_provider,
                        oauth_subject,
                    });
                }
                tracing::info!(accounts = accounts.len(), "hydrated accounts");
            }
            Err(e) => tracing::error!(error = %e, "failed to load accounts"),
        }
        match persistence::load_ratings(p).await {
            Ok(rows) => {
                for (account_id, mu, sigma, games) in rows {
                    ratings.seed(
                        AccountId(account_id),
                        Skill { mu, sigma },
                        games.max(0) as u32,
                    );
                }
                tracing::info!(rated = ratings.len(), "hydrated ratings");
            }
            Err(e) => tracing::error!(error = %e, "failed to load ratings"),
        }
    }

    let groups = Arc::new(
        GroupRegistry::new(registry, config)
            .with_pool(pool.clone())
            .with_identity(accounts.clone(), ratings.clone()),
    );
    // Skill-based matchmaking (capability `lobby-and-matchmaking`): the queue uses
    // the skill policy, which groups rated players by skill and falls back to
    // first-come for unrated play (so anonymous matchmaking is unchanged).
    let queue = Arc::new(MatchQueue::with_identity(
        groups.clone(),
        Arc::new(SkillBased),
        accounts.clone(),
        ratings.clone(),
    ));
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
        pool: pool.clone(),
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
        pool,
        accounts,
        ratings,
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
