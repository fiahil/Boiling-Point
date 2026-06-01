//! The seeded batch runner: plays many complete games and collects per-game
//! records (tasks 5.2/5.3).
//!
//! The default [`TransportKind::InProcess`] backend spawns the server's own game
//! loop over in-memory channels and runs everything on a single-threaded executor
//! with a fixed poll order, so a `(seed, strategy assignment, content config)`
//! triple reproduces a run exactly (D4). [`TransportKind::WebSocket`] stands up an
//! embedded server and plays a smaller batch over the real socket to keep the
//! harness honest about the wire — but the server picks its own game seed there,
//! so WebSocket runs are *not* reproducible and are meant for protocol coverage,
//! not balance numbers.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use boiling_point_protocol::client::PROTOCOL_VERSION;
use boiling_point_protocol::vocab::Color;
use boiling_point_protocol::{ClientMessage, EmoteId, PlayerId, RoomCode, ServerMessage};
use boiling_point_server::config::ContentConfig;
use boiling_point_server::lobby::{MatchQueue, RoomRegistry, SessionStore};
use boiling_point_server::session::{SeatInfo, run_game};
use boiling_point_server::transport::{AppState, app};

use crate::HarnessError;
use crate::bot::{GameObservation, RoundObservation, run_bot};
use crate::rng::GameSeeds;
use crate::strategy::Strategy;
use crate::transport::{BotConnection, InProcess, WebSocket};

/// Players per table — fixed at four.
const SEATS: usize = 4;
/// Per-game wall-clock guard: a game exceeding this is treated as hung.
const GAME_TIMEOUT: Duration = Duration::from_secs(30);

/// Which transport backend a batch runs over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    /// Drive the server's game loop over in-memory channels (default; reproducible).
    InProcess,
    /// Drive a real WebSocket against an embedded server (honesty check, smaller batch).
    WebSocket,
}

impl TransportKind {
    /// The label used in reports.
    pub fn label(self) -> &'static str {
        match self {
            TransportKind::InProcess => "in-process",
            TransportKind::WebSocket => "websocket",
        }
    }
}

/// Parameters that fully define a batch run.
#[derive(Debug, Clone)]
pub struct BatchParams {
    /// Number of complete games to play.
    pub games: u64,
    /// Root seed for the whole run's RNG tree.
    pub seed: u64,
    /// Transport backend.
    pub transport: TransportKind,
    /// Strategy assigned to each of the four seats, in order.
    pub strategy_names: Vec<String>,
}

/// One seat's identity and assigned strategy, for win attribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatRecord {
    /// The seat's player id.
    pub player_id: PlayerId,
    /// The seat's colour.
    pub color: Color,
    /// The strategy assigned to the seat.
    pub strategy: String,
}

/// Everything one completed game contributes to the batch statistics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRecord {
    /// 0-based game index within the batch.
    pub index: u64,
    /// The four seats (identity + strategy), for attributing wins.
    pub seats: Vec<SeatRecord>,
    /// Per-round observations (from a single seat's broadcast view — identical across seats).
    pub rounds: Vec<RoundObservation>,
    /// The game's winner(s).
    pub winners: Vec<PlayerId>,
    /// Whether the game reached `GameOver`.
    pub completed: bool,
}

/// Build the bot-facing emote palette (the enabled preset emotes).
fn emote_palette(config: &ContentConfig) -> Vec<EmoteId> {
    config
        .emote
        .iter()
        .filter(|e| e.enabled)
        .map(|e| EmoteId(e.id))
        .collect()
}

/// Run a batch and return the per-game records, dispatching on the transport.
pub async fn run_batch(
    config: &ContentConfig,
    params: &BatchParams,
) -> Result<Vec<GameRecord>, HarnessError> {
    if params.strategy_names.len() != SEATS {
        return Err(HarnessError::Config(format!(
            "need exactly {SEATS} strategies, got {}",
            params.strategy_names.len()
        )));
    }
    let strategies = crate::strategy::assignment_from_names(&params.strategy_names)
        .map_err(HarnessError::Config)?;

    match params.transport {
        TransportKind::InProcess => run_in_process(config, params, &strategies).await,
        TransportKind::WebSocket => run_websocket(config, params, &strategies).await,
    }
}

/// The reproducible in-process batch: each game drives `session::run_game` over
/// channels alongside four cooperatively-scheduled bots.
async fn run_in_process(
    config: &ContentConfig,
    params: &BatchParams,
    strategies: &[Box<dyn Strategy>],
) -> Result<Vec<GameRecord>, HarnessError> {
    let registry = config
        .build_registry()
        .map_err(|e| HarnessError::Config(e.to_string()))?;
    let palette_vec = emote_palette(config);
    let palette_set: HashSet<u16> = palette_vec.iter().map(|e| e.0).collect();

    let mut records = Vec::with_capacity(params.games as usize);
    for index in 0..params.games {
        let seeds = GameSeeds::for_game(params.seed, index);
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(256);

        let mut seat_infos = Vec::with_capacity(SEATS);
        let mut conns = Vec::with_capacity(SEATS);
        let mut seat_records = Vec::with_capacity(SEATS);
        for (seat, strategy) in strategies.iter().enumerate() {
            let (out_tx, out_rx) = tokio::sync::mpsc::channel::<ServerMessage>(256);
            let player = PlayerId(Uuid::from_u128(((index as u128) << 4) | (seat as u128 + 1)));
            let color = Color::PLAYER_COLORS[seat];
            seat_infos.push(SeatInfo {
                id: player,
                name: format!("seat{seat}"),
                color,
                out: out_tx,
            });
            conns.push(InProcess::new(player, cmd_tx.clone(), out_rx));
            seat_records.push(SeatRecord {
                player_id: player,
                color,
                strategy: strategy.name().to_string(),
            });
        }
        drop(cmd_tx); // bots hold the only senders now

        let conns: [InProcess; SEATS] = conns
            .try_into()
            .map_err(|_| HarnessError::Config("seat count mismatch".into()))?;
        let [c0, c1, c2, c3] = conns;
        let room_code = RoomCode(format!("BATCH-{index}"));

        let game_fut = run_game(
            &registry,
            config,
            room_code,
            seat_infos,
            &mut cmd_rx,
            &palette_set,
            seeds.server,
        );
        let b0 = run_bot(
            c0,
            seat_records[0].player_id,
            seat_records[0].color,
            strategies[0].as_ref(),
            seeds.bot_rng(0),
            &palette_vec,
        );
        let b1 = run_bot(
            c1,
            seat_records[1].player_id,
            seat_records[1].color,
            strategies[1].as_ref(),
            seeds.bot_rng(1),
            &palette_vec,
        );
        let b2 = run_bot(
            c2,
            seat_records[2].player_id,
            seat_records[2].color,
            strategies[2].as_ref(),
            seeds.bot_rng(2),
            &palette_vec,
        );
        let b3 = run_bot(
            c3,
            seat_records[3].player_id,
            seat_records[3].color,
            strategies[3].as_ref(),
            seeds.bot_rng(3),
            &palette_vec,
        );

        let joined = tokio::time::timeout(GAME_TIMEOUT, async {
            tokio::join!(game_fut, b0, b1, b2, b3)
        })
        .await
        .map_err(|_| HarnessError::Incomplete(format!("game {index} timed out")))?;

        let (_game, r0, r1, r2, r3) = joined;
        // Propagate any secret-boundary breach; drain the rest so they aren't dropped silently.
        let obs0 = r0?;
        let (_, _, _) = (r1?, r2?, r3?);
        records.push(record_from(index, seat_records, obs0)?);
    }
    Ok(records)
}

/// The WebSocket honesty-check batch: stand up an embedded server, then for each
/// game connect four bots (one creates the room, three join by code) and play.
async fn run_websocket(
    config: &ContentConfig,
    params: &BatchParams,
    strategies: &[Box<dyn Strategy>],
) -> Result<Vec<GameRecord>, HarnessError> {
    let url = spawn_embedded_server(config).await?;
    let palette_vec = emote_palette(config);

    let mut records = Vec::with_capacity(params.games as usize);
    for index in 0..params.games {
        let seeds = GameSeeds::for_game(params.seed, index);

        // Seat 0 creates the room; learn the invite code from its RoomJoined.
        let (c0, id0, color0, code) = ws_join(
            &url,
            ClientMessage::CreateRoom {
                protocol_version: PROTOCOL_VERSION,
                display_name: "seat0".into(),
                session_token: None,
            },
        )
        .await?;
        let code = code
            .ok_or_else(|| HarnessError::WebSocket("create did not return a room code".into()))?;

        // Seats 1–3 join by code (the fourth join starts the game).
        let (c1, id1, color1, _) = ws_join(&url, join_msg("seat1", &code)).await?;
        let (c2, id2, color2, _) = ws_join(&url, join_msg("seat2", &code)).await?;
        let (c3, id3, color3, _) = ws_join(&url, join_msg("seat3", &code)).await?;

        let seat_records = vec![
            seat_record(id0, color0, strategies[0].as_ref()),
            seat_record(id1, color1, strategies[1].as_ref()),
            seat_record(id2, color2, strategies[2].as_ref()),
            seat_record(id3, color3, strategies[3].as_ref()),
        ];

        let b0 = run_bot(
            c0,
            id0,
            color0,
            strategies[0].as_ref(),
            seeds.bot_rng(0),
            &palette_vec,
        );
        let b1 = run_bot(
            c1,
            id1,
            color1,
            strategies[1].as_ref(),
            seeds.bot_rng(1),
            &palette_vec,
        );
        let b2 = run_bot(
            c2,
            id2,
            color2,
            strategies[2].as_ref(),
            seeds.bot_rng(2),
            &palette_vec,
        );
        let b3 = run_bot(
            c3,
            id3,
            color3,
            strategies[3].as_ref(),
            seeds.bot_rng(3),
            &palette_vec,
        );

        let joined = tokio::time::timeout(GAME_TIMEOUT, async { tokio::join!(b0, b1, b2, b3) })
            .await
            .map_err(|_| HarnessError::Incomplete(format!("ws game {index} timed out")))?;
        let (r0, r1, r2, r3) = joined;
        let obs0 = r0?;
        let (_, _, _) = (r1?, r2?, r3?);
        records.push(record_from(index, seat_records, obs0)?);
    }
    Ok(records)
}

/// Assemble a `GameRecord` from a seat's observation, failing if the game did not
/// complete.
fn record_from(
    index: u64,
    seats: Vec<SeatRecord>,
    obs: GameObservation,
) -> Result<GameRecord, HarnessError> {
    if !obs.completed {
        return Err(HarnessError::Incomplete(format!(
            "game {index} ended without GameOver"
        )));
    }
    Ok(GameRecord {
        index,
        seats,
        rounds: obs.rounds,
        winners: obs.winners,
        completed: obs.completed,
    })
}

fn seat_record(player_id: PlayerId, color: Color, strategy: &dyn Strategy) -> SeatRecord {
    SeatRecord {
        player_id,
        color,
        strategy: strategy.name().to_string(),
    }
}

fn join_msg(name: &str, code: &RoomCode) -> ClientMessage {
    ClientMessage::JoinRoom {
        protocol_version: PROTOCOL_VERSION,
        display_name: name.into(),
        session_token: None,
        room_code: code.clone(),
    }
}

/// Connect, send the entry message, and read until `RoomJoined`, returning the
/// connection plus the seat's confirmed identity (and the room code, on create).
async fn ws_join(
    url: &str,
    entry: ClientMessage,
) -> Result<(WebSocket, PlayerId, Color, Option<RoomCode>), HarnessError> {
    let mut conn = WebSocket::connect(url)
        .await
        .map_err(|e| HarnessError::WebSocket(e.to_string()))?;
    conn.send(entry).await;
    loop {
        match conn.recv().await {
            Some(ServerMessage::RoomJoined {
                room_code,
                your_player_id,
                your_color,
                ..
            }) => return Ok((conn, your_player_id, your_color, Some(room_code))),
            Some(ServerMessage::Error { message, .. }) => {
                return Err(HarnessError::WebSocket(message));
            }
            Some(_) => continue,
            None => {
                return Err(HarnessError::WebSocket(
                    "connection closed before RoomJoined".into(),
                ));
            }
        }
    }
}

/// Stand up an embedded server bound to an ephemeral localhost port and return its
/// `/ws` URL. Wave timers are shortened (the per-connection rate limit drops a
/// bot's `LockIn`, so waves close on the timer here) — timing does not affect
/// outcomes, only how fast the batch runs.
async fn spawn_embedded_server(config: &ContentConfig) -> Result<String, HarnessError> {
    let mut config = config.clone();
    config.timing.wave1_ms = 250;
    config.timing.wave_ms = 200;
    let registry = Arc::new(
        config
            .build_registry()
            .map_err(|e| HarnessError::Config(e.to_string()))?,
    );
    let config = Arc::new(config);
    let rooms = Arc::new(RoomRegistry::new(registry, config));
    let queue = Arc::new(MatchQueue::new(rooms.clone()));
    let state = AppState {
        sessions: Arc::new(SessionStore::new()),
        rooms,
        queue,
        conn_timeout: Duration::from_secs(60),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| HarnessError::WebSocket(e.to_string()))?;
    let addr = listener
        .local_addr()
        .map_err(|e| HarnessError::WebSocket(e.to_string()))?;
    tokio::spawn(async move {
        let _ = axum::serve(listener, app(state)).await;
    });
    Ok(format!("ws://{addr}/ws"))
}
