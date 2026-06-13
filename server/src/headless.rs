//! Headless in-process room boot (`boom2-ai-client` task 2.1): the seam the AI
//! client's batch harness drives thousands of seeded games through.
//!
//! The boundary between any seat and the server is a pair of channels carrying
//! **encoded wire frames** — the exact MessagePack bytes the WebSocket would
//! carry, produced and consumed by the production [`codec`] on both sides. No
//! domain object ever crosses (firewall D2): a client on the other end of a
//! [`HeadlessSeat`] is, byte for byte, a real client, and every batch game
//! exercises the codec. Undecodable inbound frames are skipped, mirroring the
//! WebSocket transport.
//!
//! The boot synthesizes the lobby's `GroupJoined`/`GameStarting` announcements
//! before driving [`session::run_game`] directly, so a client speaks the same
//! message sequence here as over the real wire (entry → roster → game), minus
//! the entry handshake the lobby owns.

use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use uuid::Uuid;

use boiling_point_protocol::server::PlayerPublic;
use boiling_point_protocol::vocab::Color;
use boiling_point_protocol::{GroupCode, PlayerId, ServerMessage, codec};

use crate::config::{ContentConfig, ROUND_COUNT};
use crate::content::ContentRegistry;
use crate::lobby::group::GroupCommand;
use crate::session::{GameEnd, SeatInfo, run_game};

/// One seat's byte-frame endpoints, handed to the client side. Sending on
/// `to_server` delivers an encoded `ClientMessage`; `from_server` yields encoded
/// `ServerMessage`s and closes when the game ends.
pub struct HeadlessSeat {
    /// The seat's player id (a client also learns it from `GroupJoined`).
    pub player: PlayerId,
    /// The seat's colour (also in `GroupJoined`).
    pub color: Color,
    /// Encoded client→server frames.
    pub to_server: mpsc::Sender<Vec<u8>>,
    /// Encoded server→client frames.
    pub from_server: mpsc::Receiver<Vec<u8>>,
}

/// A booted headless game: the four client-side seat endpoints and the handle
/// to the running game task.
pub struct HeadlessGame {
    /// The four seats, in seating (colour) order.
    pub seats: Vec<HeadlessSeat>,
    /// Resolves when the game completes.
    pub game: JoinHandle<GameEnd>,
}

/// Boot a complete four-player game headlessly with a fixed `seed`, returning
/// byte-frame endpoints for each seat. Player ids derive deterministically from
/// the seed so a re-run is byte-identical end to end.
pub fn boot_headless_game(
    registry: Arc<ContentRegistry>,
    config: Arc<ContentConfig>,
    names: [String; 4],
    seed: u64,
) -> HeadlessGame {
    let palette: HashSet<u16> = config
        .emote
        .iter()
        .filter(|e| e.enabled)
        .map(|e| e.id)
        .collect();
    let group_code = GroupCode(format!("HEADLESS-{seed:X}"));

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<GroupCommand>(256);

    // Roster, deterministic under the seed (outcomes are id-independent, but a
    // stable roster keeps replay payloads and logs byte-comparable).
    let roster: Vec<(PlayerId, Color, String)> = Color::PLAYER_COLORS
        .into_iter()
        .enumerate()
        .map(|(i, color)| {
            let id = PlayerId(Uuid::from_u128(((seed as u128) << 8) | (i as u128 + 1)));
            (id, color, names[i].clone())
        })
        .collect();
    let public: Vec<PlayerPublic> = roster
        .iter()
        .map(|(id, color, name)| PlayerPublic {
            id: *id,
            display_name: name.clone(),
            color: *color,
            connected: true,
            guest: false,
        })
        .collect();

    let mut seat_infos = Vec::with_capacity(4);
    let mut seats = Vec::with_capacity(4);
    for (player, color, name) in &roster {
        let (player, color) = (*player, *color);
        // Server-side ServerMessage channel → encoder task → client bytes.
        let (out_tx, mut out_rx) = mpsc::channel::<ServerMessage>(256);
        let (bytes_out_tx, bytes_out_rx) = mpsc::channel::<Vec<u8>>(256);
        // Client bytes → decoder task → group commands.
        let (bytes_in_tx, mut bytes_in_rx) = mpsc::channel::<Vec<u8>>(256);

        // The lobby announcements a real client would have seen by game start.
        let joined = ServerMessage::GroupJoined {
            group_code: group_code.clone(),
            your_player_id: player,
            your_color: color,
            session_token: String::new(),
            players: public.clone(),
        };
        let starting = ServerMessage::GameStarting {
            players: public.clone(),
            round_count: ROUND_COUNT,
        };

        let encoder_out = bytes_out_tx.clone();
        tokio::spawn(async move {
            for msg in [&joined, &starting] {
                let Ok(bytes) = codec::encode(msg) else {
                    return;
                };
                if encoder_out.send(bytes).await.is_err() {
                    return;
                }
            }
            while let Some(msg) = out_rx.recv().await {
                let Ok(bytes) = codec::encode(&msg) else {
                    break;
                };
                if encoder_out.send(bytes).await.is_err() {
                    break;
                }
            }
        });

        let decoder_cmd = cmd_tx.clone();
        tokio::spawn(async move {
            while let Some(bytes) = bytes_in_rx.recv().await {
                // Mirror the WebSocket transport: undecodable frames are skipped.
                if let Ok(msg) = codec::decode::<boiling_point_protocol::ClientMessage>(&bytes)
                    && decoder_cmd
                        .send(GroupCommand::Action { player, msg })
                        .await
                        .is_err()
                {
                    break;
                }
            }
            // The client hung up: free the seat like a transport disconnect.
            let _ = decoder_cmd.send(GroupCommand::Leave { player }).await;
        });

        seat_infos.push(SeatInfo {
            id: player,
            name: name.clone(),
            color,
            guest: false,
            out: out_tx,
        });
        seats.push(HeadlessSeat {
            player,
            color,
            to_server: bytes_in_tx,
            from_server: bytes_out_rx,
        });
    }
    drop(cmd_tx); // the decoder tasks hold the only senders now

    let game = tokio::spawn(async move {
        run_game(
            &registry,
            &config,
            group_code,
            seat_infos,
            &mut cmd_rx,
            &palette,
            seed,
            None,
        )
        .await
    });

    HeadlessGame { seats, game }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::server::PlayerScore;
    use boiling_point_protocol::{CardId, ClientMessage};

    fn content() -> (Arc<ContentRegistry>, Arc<ContentConfig>) {
        let mut cfg = ContentConfig::from_toml(include_str!("../content.toml")).unwrap();
        cfg.timing.wave1_ms = 2_000;
        cfg.timing.wave_ms = 2_000;
        cfg.timing.brewer_pick_ms = 2_000;
        cfg.timing.draft_ms = 2_000;
        let reg = cfg.build_registry().unwrap();
        (Arc::new(reg), Arc::new(cfg))
    }

    /// A minimal byte-level client: decodes every inbound frame through the
    /// production codec, plays its first hand card each wave (encoding its own
    /// frames), and returns the final scores it observed.
    async fn byte_client(mut seat: HeadlessSeat) -> Option<Vec<PlayerScore>> {
        let mut hand: Vec<CardId> = Vec::new();
        let mut passed = false;
        while let Some(bytes) = seat.from_server.recv().await {
            let msg: ServerMessage = codec::decode(&bytes).expect("every frame decodes");
            match msg {
                ServerMessage::YourHand { ingredients, .. } => {
                    hand = ingredients.iter().map(|c| c.id).collect();
                }
                ServerMessage::DecisionFrame {
                    decision: boiling_point_protocol::PendingDecision::BrewerPick { options },
                    ..
                } => {
                    let pick = ClientMessage::PickBrewer { brewer: options[0] };
                    let _ = seat.to_server.send(codec::encode(&pick).unwrap()).await;
                }
                ServerMessage::DecisionFrame {
                    decision:
                        boiling_point_protocol::PendingDecision::ApothecaryDraft { suggested, .. },
                    ..
                } => {
                    let submit = ClientMessage::SubmitRecipe { recipe: suggested };
                    let _ = seat.to_server.send(codec::encode(&submit).unwrap()).await;
                }
                ServerMessage::WaveOpened { wave_number, .. } => {
                    if wave_number == 1 {
                        passed = false;
                    }
                    if passed {
                        continue;
                    }
                    let action = match hand.first() {
                        Some(&card) => {
                            hand.remove(0);
                            ClientMessage::CommitIngredient {
                                card,
                                colorless: false,
                            }
                        }
                        None => {
                            passed = true;
                            ClientMessage::CommitPass
                        }
                    };
                    for msg in [action, ClientMessage::LockIn] {
                        let bytes = codec::encode(&msg).unwrap();
                        let _ = seat.to_server.send(bytes).await;
                    }
                }
                ServerMessage::WaveResolved { passed: p, .. } if p.contains(&seat.player) => {
                    passed = true;
                }
                ServerMessage::GameOver { final_scores, .. } => return Some(final_scores),
                _ => {}
            }
        }
        None
    }

    /// The seam plays a complete game over encoded frames, and the same seed
    /// reproduces the same outcome byte-channel to byte-channel.
    #[tokio::test]
    async fn headless_games_complete_and_reproduce_over_the_codec() {
        let (reg, cfg) = content();
        let play = |seed: u64| {
            let (reg, cfg) = (reg.clone(), cfg.clone());
            async move {
                let booted = boot_headless_game(
                    reg,
                    cfg,
                    ["a".into(), "b".into(), "c".into(), "d".into()],
                    seed,
                );
                let mut clients = booted.seats.into_iter().map(byte_client);
                let (c0, c1, c2, c3) = (
                    clients.next().unwrap(),
                    clients.next().unwrap(),
                    clients.next().unwrap(),
                    clients.next().unwrap(),
                );
                let (r0, r1, r2, r3) = tokio::join!(c0, c1, c2, c3);
                let _ = booted.game.await.expect("game task completes");
                let scores = r0.expect("seat 0 saw GameOver");
                for other in [r1, r2, r3] {
                    assert_eq!(other.expect("seat saw GameOver"), scores);
                }
                scores
            }
        };
        let first = tokio::time::timeout(std::time::Duration::from_secs(30), play(424242))
            .await
            .expect("first headless game completed");
        let again = tokio::time::timeout(std::time::Duration::from_secs(30), play(424242))
            .await
            .expect("second headless game completed");
        assert_eq!(first, again, "same seed, same outcome over the seam");
        let diverged = tokio::time::timeout(std::time::Duration::from_secs(30), play(424243))
            .await
            .expect("third headless game completed");
        assert_ne!(
            first, diverged,
            "a different seed should (overwhelmingly) diverge"
        );
    }
}
