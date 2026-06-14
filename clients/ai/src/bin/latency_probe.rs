//! The agent-brain latency probe (`boom2-ai-client` task 5.5): one isolated
//! decision against a fixture frame — no server needed — to measure a model's
//! decision latency and per-decision cost before trusting it with a seat.

use std::sync::Arc;
use std::time::Instant;

use clap::Parser;
use rand::SeedableRng;
use rand::rngs::StdRng;
use uuid::Uuid;

use boiling_point_ai_client::agent::api::HttpMessagesApi;
use boiling_point_ai_client::agent::prompt::Difficulty;
use boiling_point_ai_client::agent::{AgentBrain, AgentSettings, ProcessSpend, SpendCaps};
use boiling_point_ai_client::brain::Brain;
use boiling_point_ai_client::view::{FrameContext, SeatView};
use boiling_point_protocol::frame::{
    CastableSpell, PendingDecision, PlayableIngredient, TargetOptions,
};
use boiling_point_protocol::vocab::{Color, HandIngredient, IngredientView, SpellKind};
use boiling_point_protocol::{CardId, PlayerId, ServerMessage};

/// Measure one agent-brain decision against a fixture wave-commit frame.
#[derive(Parser)]
#[command(name = "latency_probe")]
struct Cli {
    /// Model id to probe (e.g. claude-opus-4-8, claude-haiku-4-5).
    #[arg(long, default_value = "claude-opus-4-8")]
    model: String,
    /// Decisions to run (latency varies; probe a few).
    #[arg(long, default_value_t = 3)]
    runs: u32,
    /// Wave timer the fixture frame advertises, in milliseconds.
    #[arg(long, default_value_t = 15_000)]
    timer_ms: u32,
}

/// A mid-game fixture: three hand cards, two castable spells, two opponents.
fn fixture() -> (SeatView, FrameContext) {
    let me = PlayerId(Uuid::from_u128(1));
    let mut view = SeatView::new(me, Color::Ruby).with_transcript();
    // Seed a little observed history so the prompt is representative.
    view.observe(&ServerMessage::WaveOpened {
        round_number: 2,
        wave_number: 1,
        timer_ms: 25_000,
        final_wave: false,
    });
    view.observe(&ServerMessage::WaveResolved {
        played: vec![PlayerId(Uuid::from_u128(2)), PlayerId(Uuid::from_u128(3))],
        passed: vec![],
        cauldron_card_count: 5,
        contributions: vec![],
    });
    let card = |id: u32, color, vol, pts| PlayableIngredient {
        ingredient: HandIngredient {
            id: CardId(id),
            view: IngredientView {
                color,
                volatility: vol,
                points: pts,
                compounding: None,
            },
        },
        colorless_allowed: true,
    };
    let frame = FrameContext {
        round_number: 2,
        wave_number: 2,
        timer_ms: Some(15_000),
        decision: PendingDecision::WaveCommit {
            playable: vec![
                card(11, Color::Ruby, 5, 3),
                card(12, Color::Ruby, 1, 1),
                card(13, Color::Wild, 2, 0),
            ],
            can_pass: true,
            spells: vec![
                CastableSpell {
                    spell: CardId(21),
                    kind: SpellKind::Peek,
                    targets: TargetOptions::None,
                },
                CastableSpell {
                    spell: CardId(22),
                    kind: SpellKind::Hex,
                    targets: TargetOptions::Players {
                        players: vec![PlayerId(Uuid::from_u128(2)), PlayerId(Uuid::from_u128(3))],
                    },
                },
            ],
            can_defer: false,
        },
    };
    (view, frame)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();
    let api = match HttpMessagesApi::from_env() {
        Ok(api) => Arc::new(api),
        Err(e) => {
            eprintln!("latency_probe: {e}");
            std::process::exit(2);
        }
    };
    let settings = AgentSettings {
        model: cli.model.clone(),
        difficulty: Difficulty::Standard,
        spend: SpendCaps {
            per_game_usd: 1.0,
            per_process_usd: 1.0,
        },
        ..AgentSettings::default()
    };
    let mut brain = AgentBrain::new(settings, api, ProcessSpend::new(), StdRng::seed_from_u64(0));
    let (view, mut frame) = fixture();
    frame.timer_ms = Some(cli.timer_ms);

    println!(
        "probing {} with a fixture wave-commit frame ({} runs)…",
        cli.model, cli.runs
    );
    for run in 1..=cli.runs {
        let started = Instant::now();
        let answer = brain.decide(&view, &frame).await;
        let elapsed = started.elapsed();
        println!(
            "run {run}: {elapsed:>8.1?}  answer = {answer:?}  (legal: {})",
            answer.is_legal(&frame.decision)
        );
    }
    let stats = brain.stats();
    println!(
        "api decisions: {}; degraded: {}; spend: ${:.4}; max prompt: {} chars",
        stats.api_decisions,
        stats.degraded.len(),
        stats.game_spend_usd,
        stats.max_prompt_chars,
    );
}
