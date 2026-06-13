//! Seat-filler mode (capability `boom-seat-filler`): AI seats in real rooms
//! over the WebSocket wire — joining by invite code or matchmaking enqueue,
//! playing complete games with either brain, never holding up the table.
//!
//! Pre-game decisions default to **Delegated** (the brain genuinely decides —
//! D5's seat-filler posture; today that is the wave commit, and the Brewer
//! pick/draft kinds inherit the same default when they land). A transient
//! disconnect is survived per the protocol's reconnection contract: re-enter
//! with the held session token and group code, rebuild the view from the
//! server's snapshot, and keep playing; permanent failure exits that seat
//! cleanly without disturbing the process's other seats.

use std::sync::Arc;
use std::time::Duration;

use rand::SeedableRng;
use rand::rngs::StdRng;

use boiling_point_protocol::{ClientMessage, EmoteId};

use crate::ClientError;
use crate::agent::api::{HttpMessagesApi, MessagesApi};
use crate::agent::{AgentBrain, AgentSettings, ProcessSpend};
use crate::bot::rng::derive;
use crate::bot::{Archetype, BotBrain};
use crate::brain::Brain;
use crate::seat::{SeatConfig, SeatOutcome, run_seat};
use crate::transport::{Connection, EntryMode, WsConnection, enter};

/// Which brain a filler seat plays with.
#[derive(Debug, Clone)]
pub enum FillerBrain {
    /// The deterministic bot brain.
    Bot {
        /// Heuristic posture.
        archetype: Archetype,
        /// Blunder epsilon (0..=1).
        epsilon: f64,
        /// Seed for the seat's RNG stream (per-game streams derive from it).
        seed: u64,
    },
    /// The Claude-driven agent brain (one fresh brain per game, so the
    /// per-game spend cap resets with the game).
    Agent(Box<AgentSettings>),
}

/// The player-facing **familiar** name for a brain (Apothecary Ink flavor):
/// a bot seat presents as the witch's helper creature — "Timid Toad
/// (familiar)" — so nobody mistakes it for a human, and the name pool can't
/// collide with the ingredient/bucket vocabulary. Code-level identifiers stay
/// literal (`cautious`, `aggressive`, …); only display surfaces use these.
pub fn familiar_name(brain: &FillerBrain) -> String {
    let pairing = match brain {
        FillerBrain::Bot { archetype, .. } => match archetype {
            Archetype::Cautious => "Timid Toad",
            Archetype::Aggressive => "Brash Salamander",
            Archetype::Political => "Silver-tongued Raven",
            Archetype::Random => "Scatterbrained Moth",
        },
        // The artificial brewer itself.
        FillerBrain::Agent(_) => "Homunculus",
    };
    format!("{pairing} (familiar)")
}

/// One seat's configuration within a filler process.
#[derive(Debug, Clone)]
pub struct FillerSeatSettings {
    /// Display name at the table (the persona's face).
    pub display_name: String,
    /// How the seat enters the server.
    pub entry: EntryMode,
    /// The brain and its settings.
    pub brain: FillerBrain,
    /// Complete games to play before leaving the group (PlayAgain between).
    pub games: u32,
    /// Emote palette ids the persona may use for table presence (server
    /// content config; operator-supplied — empty means silent).
    pub emote_palette: Vec<u16>,
    /// Reconnection attempts after a transient disconnect before giving up.
    pub reconnect_attempts: u32,
}

impl Default for FillerSeatSettings {
    fn default() -> Self {
        let brain = FillerBrain::Bot {
            archetype: Archetype::Political,
            epsilon: 0.05,
            seed: rand::random(),
        };
        FillerSeatSettings {
            display_name: familiar_name(&brain),
            entry: EntryMode::Enqueue,
            brain,
            games: 1,
            emote_palette: Vec::new(),
            reconnect_attempts: 3,
        }
    }
}

/// How a filler seat's run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeatExit {
    /// Played its configured number of games and left the group.
    Completed,
    /// Reconnection ultimately failed; the seat exited cleanly.
    ConnectionLost(String),
    /// A server-side secret-boundary breach (always fatal, loudly).
    SecretLeak(String),
}

/// One filler seat's full report: per-game outcomes plus how it ended.
#[derive(Debug)]
pub struct FillerSeatReport {
    /// The seat's display name.
    pub display_name: String,
    /// Per-completed-game outcomes, in order.
    pub games: Vec<SeatOutcome>,
    /// How the seat ended.
    pub exit: SeatExit,
}

/// Build the per-game brain pair for a seat.
fn build_brains(
    settings: &FillerSeatSettings,
    game_index: u32,
    api: Option<&Arc<dyn MessagesApi>>,
    spend: &Arc<ProcessSpend>,
) -> (Box<dyn Brain>, BotBrain) {
    match &settings.brain {
        FillerBrain::Bot {
            archetype,
            epsilon,
            seed,
        } => {
            let game_seed = derive(*seed, game_index as u64);
            let brain =
                BotBrain::new(*archetype, StdRng::seed_from_u64(game_seed)).with_epsilon(*epsilon);
            let fallback = BotBrain::new(*archetype, StdRng::seed_from_u64(derive(game_seed, 1)));
            (Box::new(brain), fallback)
        }
        FillerBrain::Agent(agent) => {
            let rng_seed: u64 = rand::random();
            let fallback = BotBrain::new(
                agent.fallback_archetype,
                StdRng::seed_from_u64(derive(rng_seed, 1)),
            );
            let brain = AgentBrain::new(
                (**agent).clone(),
                api.expect("agent seats build an API client up front")
                    .clone(),
                spend.clone(),
                StdRng::seed_from_u64(rng_seed),
            );
            (Box::new(brain), fallback)
        }
    }
}

/// Run one filler seat to completion: enter, play `games` complete games
/// (reconnecting through transient drops), and leave. `api` overrides the
/// Messages API client (tests inject mocks; `None` builds the real one from
/// the environment when the seat runs an agent brain).
pub async fn run_filler_seat(
    server_url: &str,
    settings: FillerSeatSettings,
    api: Option<Arc<dyn MessagesApi>>,
    spend: Arc<ProcessSpend>,
) -> FillerSeatReport {
    let api = match (&settings.brain, api) {
        (FillerBrain::Agent(_), None) => match HttpMessagesApi::from_env() {
            Ok(client) => Some(Arc::new(client) as Arc<dyn MessagesApi>),
            Err(e) => {
                return FillerSeatReport {
                    display_name: settings.display_name,
                    games: Vec::new(),
                    exit: SeatExit::ConnectionLost(format!("agent auth: {e}")),
                };
            }
        },
        (_, api) => api,
    };

    let mut report = FillerSeatReport {
        display_name: settings.display_name.clone(),
        games: Vec::new(),
        exit: SeatExit::Completed,
    };

    // Entry: the first frame on the connection is the entry message.
    let mut conn = match WsConnection::connect(server_url).await {
        Ok(conn) => conn,
        Err(e) => {
            report.exit = SeatExit::ConnectionLost(e.to_string());
            return report;
        }
    };
    // The seat-filler plays anonymously (the one-tap default); accounts/rating
    // are for human players (`boom2-identity`), surfaced by the web client.
    let joined = match enter(
        &mut conn,
        &settings.entry,
        &settings.display_name,
        None,
        None,
    )
    .await
    {
        Ok(joined) => joined,
        Err(e) => {
            report.exit = SeatExit::ConnectionLost(e.to_string());
            return report;
        }
    };
    tracing::info!(
        seat = %settings.display_name,
        group = %joined.group_code.0,
        "filler seat joined"
    );

    let seat_cfg = SeatConfig {
        // Delegated-by-default pre-game and in-game decisions (D5).
        record_transcript: matches!(settings.brain, FillerBrain::Agent(_)),
        heartbeat_quiet: Some(Duration::from_secs(20)),
        emote_palette: settings.emote_palette.iter().map(|e| EmoteId(*e)).collect(),
        ..SeatConfig::default()
    };

    let mut brains: Option<(Box<dyn Brain>, BotBrain)> = None;
    let mut games_done: u32 = 0;
    loop {
        // Fresh brains at each game start; kept across mid-game reconnects.
        let (brain, fallback) =
            brains.get_or_insert_with(|| build_brains(&settings, games_done, api.as_ref(), &spend));

        match run_seat(
            &mut conn,
            joined.player,
            joined.color,
            brain.as_mut(),
            fallback,
            &seat_cfg,
        )
        .await
        {
            Err(ClientError::SecretLeak(detail)) => {
                report.exit = SeatExit::SecretLeak(detail);
                return report;
            }
            Err(e) => {
                report.exit = SeatExit::ConnectionLost(e.to_string());
                return report;
            }
            Ok(outcome) if outcome.observation.completed => {
                report.games.push(outcome);
                brains = None;
                games_done += 1;
                if games_done >= settings.games {
                    let _ = conn.send(&ClientMessage::LeaveGroup).await;
                    report.exit = SeatExit::Completed;
                    return report;
                }
                // Opt in to the next game with the same group.
                if conn.send(&ClientMessage::PlayAgain).await.is_err() {
                    report.exit = SeatExit::ConnectionLost("connection lost between games".into());
                    return report;
                }
            }
            Ok(_) => {
                // The stream ended before GameOver: a transient disconnect.
                // Reconnect per the protocol contract — re-enter with the held
                // session token and the group's invite code; the server
                // reattaches the seat and sends a state snapshot.
                let mut reattached = false;
                for attempt in 1..=settings.reconnect_attempts {
                    tokio::time::sleep(Duration::from_millis(500 * (1 << attempt.min(4)))).await;
                    let Ok(mut fresh) = WsConnection::connect(server_url).await else {
                        continue;
                    };
                    match enter(
                        &mut fresh,
                        &EntryMode::Join(joined.group_code.clone()),
                        &settings.display_name,
                        Some(joined.session_token.clone()),
                        None,
                    )
                    .await
                    {
                        Ok(rejoined) if rejoined.player == joined.player => {
                            tracing::info!(
                                seat = %settings.display_name,
                                attempt,
                                "filler seat reconnected"
                            );
                            conn = fresh;
                            reattached = true;
                            break;
                        }
                        Ok(_) | Err(_) => continue,
                    }
                }
                if !reattached {
                    // Permanent failure: exit THIS seat cleanly; other seats
                    // in the process are independent tasks and unaffected.
                    report.exit = SeatExit::ConnectionLost(format!(
                        "reconnection failed after {} attempts",
                        settings.reconnect_attempts
                    ));
                    return report;
                }
            }
        }
    }
}

/// Run several filler seats concurrently in one process, each with its own
/// brain, settings, and connection; returns when every seat has ended.
pub async fn run_filler_process(
    server_url: &str,
    seats: Vec<FillerSeatSettings>,
    api: Option<Arc<dyn MessagesApi>>,
) -> Vec<FillerSeatReport> {
    let spend = ProcessSpend::new();
    let mut tasks = Vec::with_capacity(seats.len());
    for settings in seats {
        let url = server_url.to_string();
        let api = api.clone();
        let spend = spend.clone();
        tasks.push(tokio::spawn(async move {
            run_filler_seat(&url, settings, api, spend).await
        }));
    }
    let mut reports = Vec::with_capacity(tasks.len());
    for task in tasks {
        match task.await {
            Ok(report) => reports.push(report),
            Err(e) => reports.push(FillerSeatReport {
                display_name: "<panicked seat>".into(),
                games: Vec::new(),
                exit: SeatExit::ConnectionLost(format!("seat task panicked: {e}")),
            }),
        }
    }
    reports
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Temperaments pair with their player-facing familiars; code ids stay
    /// literal and the agent presents as the Homunculus.
    #[test]
    fn familiars_pair_with_temperaments() {
        let bot = |archetype| FillerBrain::Bot {
            archetype,
            epsilon: 0.0,
            seed: 1,
        };
        assert_eq!(
            familiar_name(&bot(Archetype::Cautious)),
            "Timid Toad (familiar)"
        );
        assert_eq!(
            familiar_name(&bot(Archetype::Aggressive)),
            "Brash Salamander (familiar)"
        );
        assert_eq!(
            familiar_name(&bot(Archetype::Political)),
            "Silver-tongued Raven (familiar)"
        );
        assert_eq!(
            familiar_name(&bot(Archetype::Random)),
            "Scatterbrained Moth (familiar)"
        );
        assert_eq!(
            familiar_name(&FillerBrain::Agent(Box::default())),
            "Homunculus (familiar)"
        );
        // An unnamed seat presents as its brain's familiar by default.
        let defaults = FillerSeatSettings::default();
        assert_eq!(defaults.display_name, familiar_name(&defaults.brain));
    }
}
