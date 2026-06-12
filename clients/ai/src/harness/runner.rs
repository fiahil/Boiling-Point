//! The seeded batch runner (task 6.1): plays every cell of a sample spec to
//! completion and collects per-game records.
//!
//! The default transport is the server's headless in-process seam — byte
//! channels carrying encoded wire frames through the production codec (D2) —
//! driven from a deterministic RNG tree (root → cell → game → seat, D7). A
//! WebSocket transport is retained for transport-parity validation: the same
//! seeded scenario over an embedded real-wire server must produce identical
//! outcomes (task 2.2), pinned via the server's injectable game-seed source.
//! Agent seats are opt-in (`allow_agents`) and void reproducibility.

use std::sync::Arc;
use std::time::Duration;

use boiling_point_protocol::vocab::Color;
use boiling_point_protocol::{EmoteId, GroupCode, PlayerId};
use boiling_point_server::config::ContentConfig;
use boiling_point_server::headless::boot_headless_game;
use boiling_point_server::lobby::{GroupRegistry, MatchQueue, QueuedSeeds, SessionStore};
use boiling_point_server::transport::{AppState, app};

use crate::ClientError;
use crate::agent::api::{HttpMessagesApi, MessagesApi};
use crate::agent::{AgentBrain, AgentSettings, ProcessSpend};
use crate::bot::rng::{GameSeeds, derive};
use crate::bot::{Archetype, BotBrain};
use crate::brain::Brain;
use crate::observe::RoundObservation;
use crate::seat::{SeatConfig, run_seat};
use crate::transport::{ChannelConnection, EntryMode, WsConnection, enter};

use super::spec::{BrainSpec, CellSpec, SampleSpec, SeatSpec};

/// Salt for deriving a cell's seed from the root.
const CELL_SALT: u64 = 0x6365_6C6C_5345_4544; // "cellSEED"
/// Wall-clock guard per game: a game exceeding this is treated as hung.
const GAME_TIMEOUT: Duration = Duration::from_secs(60);

/// Which transport a batch runs over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    /// The headless in-process seam (default: reproducible, fast).
    InProcess,
    /// An embedded real-wire server (parity validation; slower).
    WebSocket,
}

impl TransportKind {
    /// The label reports carry.
    pub fn label(self) -> &'static str {
        match self {
            TransportKind::InProcess => "in-process",
            TransportKind::WebSocket => "websocket",
        }
    }
}

/// Run-level options beyond the spec itself.
#[derive(Debug, Clone, Copy)]
pub struct RunOptions {
    /// Transport to drive the games over.
    pub transport: TransportKind,
    /// Permit agent (Claude) seats — the explicit no-accidental-spend gate.
    pub allow_agents: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        RunOptions {
            transport: TransportKind::InProcess,
            allow_agents: false,
        }
    }
}

/// One seat's identity + accounting in one game.
#[derive(Debug, Clone)]
pub struct SeatRecord {
    /// The report label outcomes are attributed to.
    pub label: String,
    /// The seat's player id in this game.
    pub player: PlayerId,
    /// The seat's colour.
    pub color: Color,
    /// Decisions the seat answered.
    pub decisions: u32,
    /// Fallback commits (budget misses + illegal answers).
    pub fallbacks: u32,
}

/// A transport-comparable summary of one game's outcome (colours, not player
/// ids, so in-process and WebSocket runs of the same seed compare equal).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OutcomeSummary {
    /// Final score per seat colour, in seating order.
    pub scores_by_color: Vec<(String, i32)>,
    /// The winning colour(s).
    pub winner_colors: Vec<String>,
    /// How many rounds exploded.
    pub exploded_rounds: u32,
}

/// Everything one completed game contributes to the statistics.
#[derive(Debug, Clone)]
pub struct GameRecord {
    /// 0-based game index within its cell.
    pub index: u64,
    /// The four seats (label + identity + fallback accounting).
    pub seats: Vec<SeatRecord>,
    /// Per-round observations (from seat 0's broadcast view).
    pub rounds: Vec<RoundObservation>,
    /// The game's winner(s).
    pub winners: Vec<PlayerId>,
    /// The transport-comparable outcome summary.
    pub outcome: OutcomeSummary,
}

/// One cell's completed games.
#[derive(Debug, Clone)]
pub struct CellRun {
    /// The cell as specified.
    pub spec: CellSpec,
    /// Per-game records, in game order.
    pub games: Vec<GameRecord>,
}

/// A whole sample's results.
#[derive(Debug, Clone)]
pub struct SampleRun {
    /// The root seed the run was driven by.
    pub root_seed: u64,
    /// The transport used.
    pub transport: TransportKind,
    /// Whether any seat ran an agent brain (voids reproducibility).
    pub agent_seats_present: bool,
    /// Per-cell results, in spec order.
    pub cells: Vec<CellRun>,
}

/// The per-game brain pair for one seat: the configured brain plus the
/// instant fallback floor.
fn build_brains(
    seat_spec: &SeatSpec,
    seeds: &GameSeeds,
    seat: usize,
    api: Option<&Arc<dyn MessagesApi>>,
    spend: &Arc<ProcessSpend>,
) -> (Box<dyn Brain>, BotBrain) {
    // Fallback streams live beside the seat streams (seats 0..4 → 4..8) so
    // bot and fallback draws never alias.
    let fallback_rng = seeds.bot_rng(seat + 4);
    match &seat_spec.brain {
        BrainSpec::Bot { archetype, epsilon } => {
            let archetype = Archetype::by_name(archetype).expect("validated by the spec");
            let brain = BotBrain::new(archetype, seeds.bot_rng(seat)).with_epsilon(*epsilon);
            (Box::new(brain), BotBrain::new(archetype, fallback_rng))
        }
        BrainSpec::Agent { model, persona } => {
            let mut settings = AgentSettings::default();
            if let Some(model) = model {
                settings.model = model.clone();
            }
            if let Some(persona) = persona {
                settings.persona = persona.clone();
            }
            let fallback = BotBrain::new(settings.fallback_archetype, fallback_rng);
            let brain = AgentBrain::new(
                settings,
                api.expect("agent seats are gated on an API client").clone(),
                spend.clone(),
                seeds.bot_rng(seat),
            );
            (Box::new(brain), fallback)
        }
    }
}

/// The harness seat configuration: transcripts only for agent brains, the
/// content config's enabled emote palette (so batches keep exercising the one
/// comms channel), heartbeats only over the real wire.
fn seat_config(
    record_transcript: bool,
    heartbeat: Option<Duration>,
    emote_palette: Vec<EmoteId>,
) -> SeatConfig {
    SeatConfig {
        record_transcript,
        heartbeat_quiet: heartbeat,
        emote_palette,
        ..SeatConfig::default()
    }
}

/// The enabled emote ids of a content config.
fn emote_palette(config: &ContentConfig) -> Vec<EmoteId> {
    config
        .emote
        .iter()
        .filter(|e| e.enabled)
        .map(|e| EmoteId(e.id))
        .collect()
}

/// Assemble a [`GameRecord`] from the four seats' outcomes.
fn record_game(
    index: u64,
    identities: Vec<(String, PlayerId, Color)>,
    outcomes: Vec<crate::seat::SeatOutcome>,
) -> Result<GameRecord, ClientError> {
    let seat_zero = outcomes
        .first()
        .ok_or_else(|| ClientError::Incomplete("no seat outcomes".into()))?;
    if !seat_zero.observation.completed {
        return Err(ClientError::Incomplete(format!(
            "game {index} ended without GameOver"
        )));
    }
    for outcome in &outcomes {
        if let Some(error) = outcome.errors.first() {
            return Err(ClientError::Incomplete(format!(
                "game {index}: a frame-driven seat drew a server error: {error}"
            )));
        }
    }
    let color_of = |player: &PlayerId| -> String {
        identities
            .iter()
            .find(|(_, p, _)| p == player)
            .map(|(_, _, c)| format!("{c:?}"))
            .unwrap_or_else(|| "?".into())
    };
    let observation = &seat_zero.observation;
    let outcome = OutcomeSummary {
        scores_by_color: observation
            .final_scores
            .iter()
            .map(|(p, s)| (color_of(p), *s))
            .collect(),
        winner_colors: observation.winners.iter().map(color_of).collect(),
        exploded_rounds: observation.rounds.iter().filter(|r| r.exploded).count() as u32,
    };
    let seats = identities
        .into_iter()
        .zip(&outcomes)
        .map(|((label, player, color), o)| SeatRecord {
            label,
            player,
            color,
            decisions: o.decisions,
            fallbacks: o.fallbacks(),
        })
        .collect();
    Ok(GameRecord {
        index,
        seats,
        rounds: observation.rounds.clone(),
        winners: observation.winners.clone(),
        outcome,
    })
}

/// Run a whole sample spec.
pub async fn run_sample(
    config: &ContentConfig,
    spec: &SampleSpec,
    options: RunOptions,
) -> Result<SampleRun, ClientError> {
    spec.validate()?;
    let agent_seats_present = spec.has_agent_seats();
    if agent_seats_present && !options.allow_agents {
        return Err(ClientError::Config(
            "the spec contains agent seats; pass the explicit agent flag to spend real money"
                .into(),
        ));
    }
    // The API client is built once, only when agent seats are both present
    // and permitted — an all-bot batch makes zero Claude calls by construction.
    let api: Option<Arc<dyn MessagesApi>> = if agent_seats_present {
        Some(Arc::new(
            HttpMessagesApi::from_env().map_err(|e| ClientError::Config(e.to_string()))?,
        ))
    } else {
        None
    };
    let spend = ProcessSpend::new();

    let mut cells = Vec::with_capacity(spec.cells.len());
    for (cell_index, cell) in spec.cells.iter().enumerate() {
        let cell_seed = derive(spec.root_seed, derive(CELL_SALT, cell_index as u64));
        let games = match options.transport {
            TransportKind::InProcess => {
                run_cell_in_process(config, cell, cell_seed, api.as_ref(), &spend).await?
            }
            TransportKind::WebSocket => {
                run_cell_websocket(config, cell, cell_seed, api.as_ref(), &spend).await?
            }
        };
        cells.push(CellRun {
            spec: cell.clone(),
            games,
        });
    }
    Ok(SampleRun {
        root_seed: spec.root_seed,
        transport: options.transport,
        agent_seats_present,
        cells,
    })
}

/// One cell over the in-process seam.
async fn run_cell_in_process(
    config: &ContentConfig,
    cell: &CellSpec,
    cell_seed: u64,
    api: Option<&Arc<dyn MessagesApi>>,
    spend: &Arc<ProcessSpend>,
) -> Result<Vec<GameRecord>, ClientError> {
    let registry = Arc::new(
        config
            .build_registry()
            .map_err(|e| ClientError::Config(e.to_string()))?,
    );
    let palette = emote_palette(config);
    let config = Arc::new(config.clone());

    let mut games = Vec::with_capacity(cell.games as usize);
    for index in 0..cell.games {
        let seeds = GameSeeds::for_game(cell_seed, index);
        let names: [String; 4] = std::array::from_fn(|i| cell.seats[i].brain.label());
        let booted = boot_headless_game(registry.clone(), config.clone(), names, seeds.server);

        let mut identities = Vec::with_capacity(4);
        let mut futures = Vec::with_capacity(4);
        for (seat_index, seat) in booted.seats.into_iter().enumerate() {
            let seat_spec = &cell.seats[seat_index];
            identities.push((seat_spec.brain.label(), seat.player, seat.color));
            let (mut brain, mut fallback) = build_brains(seat_spec, &seeds, seat_index, api, spend);
            let cfg = seat_config(seat_spec.brain.is_agent(), None, palette.clone());
            let mut conn = ChannelConnection::new(seat.to_server, seat.from_server);
            let (player, color) = (seat.player, seat.color);
            futures.push(Box::pin(async move {
                run_seat(
                    &mut conn,
                    player,
                    color,
                    brain.as_mut(),
                    &mut fallback,
                    &cfg,
                )
                .await
            }));
        }

        let game = booted.game;
        let (outcomes, end) = tokio::time::timeout(GAME_TIMEOUT, async {
            tokio::join!(futures_util::future::join_all(futures), game)
        })
        .await
        .map_err(|_| ClientError::Incomplete(format!("game {index} timed out")))?;
        end.map_err(|e| ClientError::Incomplete(format!("game task: {e}")))?;
        let outcomes: Result<Vec<_>, _> = outcomes.into_iter().collect();
        games.push(record_game(index, identities, outcomes?)?);
    }
    Ok(games)
}

/// One cell over an embedded real-wire server, seeded via the server's
/// injectable game-seed source so outcomes are pinned to the same seeds the
/// in-process run used.
async fn run_cell_websocket(
    config: &ContentConfig,
    cell: &CellSpec,
    cell_seed: u64,
    api: Option<&Arc<dyn MessagesApi>>,
    spend: &Arc<ProcessSpend>,
) -> Result<Vec<GameRecord>, ClientError> {
    // Short timers keep the batch quick; waves still close on lock-in, the
    // timer only bounds a straggler.
    let mut config = config.clone();
    config.timing.wave1_ms = config.timing.wave1_ms.min(5_000);
    config.timing.wave_ms = config.timing.wave_ms.min(5_000);

    let registry = Arc::new(
        config
            .build_registry()
            .map_err(|e| ClientError::Config(e.to_string()))?,
    );
    let palette = emote_palette(&config);
    let config = Arc::new(config);
    let game_seeds: Vec<u64> = (0..cell.games)
        .map(|i| GameSeeds::for_game(cell_seed, i).server)
        .collect();
    let groups = Arc::new(
        GroupRegistry::new(registry, config.clone())
            .with_seed_source(Arc::new(QueuedSeeds::new(game_seeds))),
    );
    let queue = Arc::new(MatchQueue::new(groups.clone()));
    groups.set_queue(&queue);
    let state = AppState {
        sessions: Arc::new(SessionStore::new()),
        groups,
        queue,
        conn_timeout: Duration::from_secs(60),
        pool: None,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| ClientError::Transport(e.to_string()))?;
    let addr = listener
        .local_addr()
        .map_err(|e| ClientError::Transport(e.to_string()))?;
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app(state)).await;
    });
    let url = format!("ws://{addr}/ws");

    let mut games = Vec::with_capacity(cell.games as usize);
    for index in 0..cell.games {
        let seeds = GameSeeds::for_game(cell_seed, index);

        // Seat 0 creates the group; 1–3 join by code (the fourth starts it).
        let mut conns = Vec::with_capacity(4);
        let mut identities: Vec<(String, PlayerId, Color)> = Vec::with_capacity(4);
        let mut group_code: Option<GroupCode> = None;
        for (seat_index, seat_spec) in cell.seats.iter().enumerate() {
            let mut conn = WsConnection::connect(&url).await?;
            let mode = match &group_code {
                None => EntryMode::Create,
                Some(code) => EntryMode::Join(code.clone()),
            };
            let joined = enter(&mut conn, &mode, &seat_spec.brain.label(), None).await?;
            if seat_index == 0 {
                group_code = Some(joined.group_code.clone());
            }
            identities.push((seat_spec.brain.label(), joined.player, joined.color));
            conns.push(conn);
        }

        let mut futures = Vec::with_capacity(4);
        for (seat_index, mut conn) in conns.into_iter().enumerate() {
            let seat_spec = &cell.seats[seat_index];
            let (_, player, color) = identities[seat_index].clone();
            let (mut brain, mut fallback) = build_brains(seat_spec, &seeds, seat_index, api, spend);
            let cfg = seat_config(
                seat_spec.brain.is_agent(),
                Some(Duration::from_secs(20)),
                palette.clone(),
            );
            futures.push(Box::pin(async move {
                run_seat(
                    &mut conn,
                    player,
                    color,
                    brain.as_mut(),
                    &mut fallback,
                    &cfg,
                )
                .await
            }));
        }
        let outcomes = tokio::time::timeout(GAME_TIMEOUT, futures_util::future::join_all(futures))
            .await
            .map_err(|_| ClientError::Incomplete(format!("ws game {index} timed out")))?;
        let outcomes: Result<Vec<_>, _> = outcomes.into_iter().collect();
        games.push(record_game(index, identities, outcomes?)?);
    }
    server.abort();
    Ok(games)
}
