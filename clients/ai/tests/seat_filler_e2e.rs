//! Seat-filler end-to-end (task 7.4): four filler seats with mixed brains
//! auto-match through the real matchmaking queue and play a complete v2 game
//! to GameOver over the real WebSocket wire, with zero missed deadlines.
//!
//! The agent seat runs against a deliberately failing mock Messages API, so
//! its every decision exercises the degrade path — proving the liveness
//! contract (a seat never stalls the table) without spending a cent.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use boiling_point_ai_client::agent::AgentSettings;
use boiling_point_ai_client::agent::api::{
    ApiError, MessagesApi, MessagesRequest, MessagesResponse,
};
use boiling_point_ai_client::bot::Archetype;
use boiling_point_ai_client::filler::{
    FillerBrain, FillerSeatSettings, SeatExit, run_filler_process,
};
use boiling_point_ai_client::transport::EntryMode;
use boiling_point_server::config::ContentConfig;
use boiling_point_server::lobby::{GroupRegistry, MatchQueue, SessionStore};
use boiling_point_server::transport::{AppState, app};

/// A Messages API that always fails — every agent decision degrades to the
/// internal bot, on time.
struct AlwaysDown;

#[async_trait]
impl MessagesApi for AlwaysDown {
    async fn call(&self, _req: &MessagesRequest) -> Result<MessagesResponse, ApiError> {
        Err(ApiError::Transport("e2e mock: api unreachable".into()))
    }
}

/// Boot a real server (WS router, lobby, matchmaking) on an ephemeral port.
async fn start_server() -> String {
    let mut config = ContentConfig::default_content();
    // Real-wire pacing: short enough for a fast test, long enough that four
    // paced seats always commit well inside the window.
    config.timing.wave1_ms = 4_000;
    config.timing.wave_ms = 4_000;
    let registry = Arc::new(config.build_registry().unwrap());
    let config = Arc::new(config);
    let groups = Arc::new(GroupRegistry::new(registry, config));
    let queue = Arc::new(MatchQueue::new(groups.clone()));
    groups.set_queue(&queue);
    let state = AppState {
        sessions: Arc::new(SessionStore::new()),
        groups,
        queue,
        conn_timeout: Duration::from_secs(60),
        pool: None,
        accounts: Default::default(),
        ratings: Default::default(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app(state)).await;
    });
    format!("ws://{addr}/ws")
}

fn bot_seat(name: &str, archetype: Archetype, seed: u64) -> FillerSeatSettings {
    FillerSeatSettings {
        display_name: name.into(),
        entry: EntryMode::Enqueue,
        brain: FillerBrain::Bot {
            archetype,
            epsilon: 0.0,
            seed,
        },
        games: 1,
        emote_palette: vec![1],
        reconnect_attempts: 2,
    }
}

/// Four filler seats — three bot archetypes and one (degraded) agent — enqueue
/// into matchmaking, get assembled into a table, and play a complete game to
/// GameOver with no server errors and no fallback at any bot seat.
#[tokio::test]
async fn four_mixed_filler_seats_automatch_and_complete_a_game() {
    let url = start_server().await;

    let agent_seat = FillerSeatSettings {
        display_name: "Madame Wick".into(),
        entry: EntryMode::Enqueue,
        brain: FillerBrain::Agent(Box::new(AgentSettings {
            persona: "a theatrical chandler".into(),
            ..AgentSettings::default()
        })),
        games: 1,
        emote_palette: Vec::new(),
        reconnect_attempts: 2,
    };
    let seats = vec![
        bot_seat("Sage Bramble", Archetype::Political, 11),
        bot_seat("Old Copperpot", Archetype::Cautious, 22),
        bot_seat("Vex the Bold", Archetype::Aggressive, 33),
        agent_seat,
    ];

    let reports = tokio::time::timeout(
        Duration::from_secs(120),
        run_filler_process(&url, seats, Some(Arc::new(AlwaysDown))),
    )
    .await
    .expect("the table completed before the timeout");

    assert_eq!(reports.len(), 4);
    for report in &reports {
        assert_eq!(
            report.exit,
            SeatExit::Completed,
            "seat '{}' must complete cleanly",
            report.display_name
        );
        assert_eq!(report.games.len(), 1, "seat '{}'", report.display_name);
        let game = &report.games[0];
        assert!(game.observation.completed, "seat '{}'", report.display_name);
        // Zero missed deadlines: a missed deadline surfaces as a server error
        // (LockedOut/StaleFrame on the late submission) — none may occur.
        assert!(
            game.errors.is_empty(),
            "seat '{}' drew server errors: {:?}",
            report.display_name,
            game.errors
        );
        assert!(game.decisions > 0, "seat '{}'", report.display_name);
        assert_eq!(
            game.observation.rounds.len(),
            5,
            "a complete v2 game runs five rounds (seat '{}')",
            report.display_name
        );
    }
    // The bot seats never need their fallback (instant + legal by construction).
    for report in reports.iter().take(3) {
        assert_eq!(
            report.games[0].fallbacks(),
            0,
            "bot seat '{}' must not fall back",
            report.display_name
        );
    }
    // All four seats agree on the winners.
    let winners: Vec<_> = reports
        .iter()
        .map(|r| r.games[0].observation.winners.clone())
        .collect();
    assert!(winners.windows(2).all(|w| w[0] == w[1]));
}

/// Permanent connection failure exits the seat cleanly instead of hanging or
/// panicking (task 7.3's failure half — the reconnect contract's happy path
/// is the session-token re-entry exercised by the server's own tests).
#[tokio::test]
async fn unreachable_server_exits_the_seat_cleanly() {
    let reports = run_filler_process(
        "ws://127.0.0.1:9/ws", // nothing listens on the discard port
        vec![bot_seat("Lonely Bot", Archetype::Cautious, 5)],
        None,
    )
    .await;
    assert_eq!(reports.len(), 1);
    assert!(
        matches!(reports[0].exit, SeatExit::ConnectionLost(_)),
        "the seat exits cleanly instead of hanging: {:?}",
        reports[0].exit
    );
    assert!(reports[0].games.is_empty());
}
