//! End-to-end validation of the admin surface against a real running game loop
//! (`admin-ui` tasks 9.1 and 9.2). This is the in-repo stand-in for a bot-harness
//! session: it registers the admin projection as the span-lifecycle consumer,
//! drives a full four-player game through the authoritative room/game loop, and
//! asserts the inspector reflects the live room, the reveal matches authoritative
//! hidden state, the balance figures count every completed round, and an operator
//! kill-room command closes the loop *through telemetry* (the room leaving the
//! live registry).
//!
//! The whole thing runs in one test function so the process-global span subscriber
//! is installed exactly once and the two scenarios do not race a shared projection.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use uuid::Uuid;

use boiling_point_protocol::{CardId, ClientMessage, PlayerId, ServerMessage};

use boiling_point_server::admin::AdminProjection;
use boiling_point_server::admin::projection::RevealOutcome;
use boiling_point_server::config::ContentConfig;
use boiling_point_server::lobby::{RoomCommand, RoomRegistry};
use boiling_point_server::observability;

/// Install the global span subscriber feeding a fresh projection (once).
fn install_projection() -> Arc<AdminProjection> {
    observability::init("127.0.0.1:0".parse().expect("metrics addr"));
    let projection = Arc::new(AdminProjection::new());
    observability::lifecycle::register_consumer(projection.clone());
    projection
}

/// A registry with short wave timers so a full game completes quickly.
fn fast_registry() -> Arc<RoomRegistry> {
    let mut config = ContentConfig::from_toml(include_str!("../content.toml")).unwrap();
    config.timing.wave1_ms = 250;
    config.timing.wave_ms = 200;
    let registry = Arc::new(config.build_registry().unwrap());
    Arc::new(RoomRegistry::new(registry, Arc::new(config)))
}

/// A bot seated in a room: commits one hand card per wave (passing once empty),
/// driving the round to a natural end. Returns whether it saw `GameOver`.
async fn bot(
    player: PlayerId,
    tx: mpsc::Sender<RoomCommand>,
    mut out: mpsc::Receiver<ServerMessage>,
) -> bool {
    let mut hand: Vec<CardId> = Vec::new();
    let mut idx = 0usize;
    while let Some(msg) = out.recv().await {
        match msg {
            ServerMessage::YourHand { cards } => {
                hand = cards.iter().map(|c| c.id).collect();
                idx = 0;
            }
            ServerMessage::WaveOpened { .. } => {
                let action = if idx < hand.len() {
                    let card = hand[idx];
                    idx += 1;
                    ClientMessage::CommitCard { card }
                } else {
                    ClientMessage::CommitPass
                };
                let _ = tx
                    .send(RoomCommand::Action {
                        player,
                        msg: action,
                    })
                    .await;
            }
            ServerMessage::GameOver { .. } => return true,
            _ => {}
        }
    }
    false
}

/// Wait until `f` returns true or the deadline passes (the lifecycle consumer is
/// fed asynchronously on a drain thread).
async fn wait_until(mut f: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if f() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    f()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn admin_surface_reflects_a_live_game_and_closes_the_control_loop() {
    let projection = install_projection();
    let rooms = fast_registry();

    // ---- 9.1: a full game appears live, the reveal matches state, balance counts.
    let (game_code, tx) = rooms.create();
    let done = Arc::new(AtomicUsize::new(0));
    let mut bots = Vec::new();
    for i in 0..4 {
        let player = PlayerId(Uuid::new_v4());
        let (otx, orx) = mpsc::channel::<ServerMessage>(256);
        tx.send(RoomCommand::Join {
            player,
            name: format!("bot{i}"),
            out: otx,
        })
        .await
        .unwrap();
        let tx = tx.clone();
        let done = done.clone();
        bots.push(tokio::spawn(async move {
            let ok = bot(player, tx, orx).await;
            done.fetch_add(1, Ordering::SeqCst);
            ok
        }));
    }

    // Poll the projection while the game runs: the room must appear live and the
    // privileged reveal must return the round's (secret) boiling point.
    let mut saw_live_room = false;
    let mut saw_reveal_bp = false;
    let game_code_str = game_code.0.clone();
    let _ = wait_until(|| {
        let rooms_now = projection.rooms();
        if rooms_now
            .iter()
            .any(|r| r.room_code.as_deref() == Some(&game_code_str))
        {
            saw_live_room = true;
        }
        if let RevealOutcome::Revealed(reveal) = projection.reveal(&game_code_str)
            && reveal.boiling_point.is_some()
        {
            saw_reveal_bp = true;
        }
        // Stop once the game span has closed (a completed game folded in).
        projection.balance().games >= 1
    })
    .await;

    for b in bots {
        assert!(
            b.await.unwrap(),
            "every seated bot should have reached GameOver"
        );
    }

    assert!(
        saw_live_room,
        "the inspector never showed the live game room"
    );
    assert!(
        saw_reveal_bp,
        "the reveal never returned the round's boiling point from open spans"
    );

    let balance = projection.balance();
    assert!(balance.games >= 1, "a completed game should be counted");
    assert!(
        balance.rounds >= 5,
        "the unsampled aggregate should count all five completed rounds, got {}",
        balance.rounds
    );
    // The room left the live registry once the game ended (room.lifetime closed).
    assert!(
        wait_until(|| {
            !projection
                .rooms()
                .iter()
                .any(|r| r.room_code.as_deref() == Some(&game_code_str))
        })
        .await,
        "the finished game's room should leave the live view"
    );
    // The game is replayable from the buffer.
    assert!(
        !projection.replay_list().is_empty(),
        "a completed game should be retained for replay"
    );

    // ---- 9.2: a kill-room command is confirmed by telemetry (the loop closes).
    let (kill_code, _kill_tx) = rooms.create();
    let kill_code_str = kill_code.0.clone();
    assert!(
        wait_until(|| projection
            .rooms()
            .iter()
            .any(|r| r.room_code.as_deref() == Some(&kill_code_str)))
        .await,
        "the seeded room should appear in the live registry"
    );
    // Operator command goes through the authoritative registry, not the projection.
    assert!(
        rooms.kill_room(&kill_code, "e2e-operator"),
        "kill should be delivered"
    );
    assert!(
        wait_until(|| !projection
            .rooms()
            .iter()
            .any(|r| r.room_code.as_deref() == Some(&kill_code_str)))
        .await,
        "the killed room should disappear from the live view — the loop closes through telemetry"
    );
}
