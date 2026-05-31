//! Canned message streams for the in-process mock server and for snapshot
//! tests, so the client can be driven end-to-end with no live server
//! (research R5; tasks 8.4/8.5/9.2).

use boiling_point_protocol::{
    PlayerId,
    ids::{CardId, EmoteId, RoomCode},
    server::{Contribution, DepileEntry, PlayerPublic, PlayerScore, ScoringOutcome, ServerMessage},
    vocab::{CardView, Color, EffectKind, HandCard, ModifierKind},
};
use uuid::Uuid;

/// A deterministic player id for seat `i` (0–3).
pub fn pid(i: u8) -> PlayerId {
    PlayerId(Uuid::from_u128(i as u128 + 1))
}

/// The four seated players (seat 0 is "you", Ruby).
pub fn players() -> Vec<PlayerPublic> {
    let names = ["you", "mistfox", "brewbaron", "cinder"];
    Color::PLAYER_COLORS
        .iter()
        .enumerate()
        .map(|(i, c)| PlayerPublic {
            id: pid(i as u8),
            display_name: names[i].into(),
            color: *c,
            connected: true,
        })
        .collect()
}

fn card(color: Color, volatility: u8, points: u8, effect: Option<EffectKind>) -> CardView {
    CardView {
        color,
        volatility,
        points,
        effect,
    }
}

/// A representative opening hand (includes a Peek and a Recall).
pub fn hand() -> Vec<HandCard> {
    vec![
        HandCard {
            id: CardId(1),
            view: card(Color::Ruby, 2, 1, None),
        },
        HandCard {
            id: CardId(2),
            view: card(Color::Sapphire, 1, 3, None),
        },
        HandCard {
            id: CardId(3),
            view: card(Color::Wild, 3, 0, Some(EffectKind::Peek)),
        },
        HandCard {
            id: CardId(4),
            view: card(Color::Emerald, 2, 2, None),
        },
        HandCard {
            id: CardId(5),
            view: card(Color::Amethyst, 1, 0, Some(EffectKind::Recall)),
        },
    ]
}

/// The `RoomJoined` message for this fixture table (seat 0's perspective).
pub fn room_joined() -> ServerMessage {
    ServerMessage::RoomJoined {
        room_code: RoomCode("BREW-7K3F".into()),
        your_player_id: pid(0),
        your_color: Color::Ruby,
        players: players(),
    }
}

/// A safe-brew depile of three cards (no explosion).
pub fn depile_safe() -> ServerMessage {
    ServerMessage::Depile {
        reveals: vec![
            DepileEntry {
                player: pid(0),
                card: card(Color::Ruby, 2, 1, None),
                running_volatility: 5,
            },
            DepileEntry {
                player: pid(1),
                card: card(Color::Sapphire, 1, 3, None),
                running_volatility: 3,
            },
            DepileEntry {
                player: pid(0),
                card: card(Color::Ruby, 2, 1, None),
                running_volatility: 2,
            },
        ],
        exploded: false,
        boiling_point: None,
        crossing_index: None,
    }
}

/// An exploded depile, with the boiling point revealed and the crossing marked.
pub fn depile_boom() -> ServerMessage {
    ServerMessage::Depile {
        reveals: vec![
            DepileEntry {
                player: pid(2),
                card: card(Color::Amethyst, 3, 0, Some(EffectKind::VolatileSurge)),
                running_volatility: 12,
            },
            DepileEntry {
                player: pid(1),
                card: card(Color::Sapphire, 3, 3, None),
                running_volatility: 9,
            },
            DepileEntry {
                player: pid(0),
                card: card(Color::Ruby, 3, 1, None),
                running_volatility: 6,
            },
        ],
        exploded: true,
        boiling_point: Some(10),
        crossing_index: Some(0),
    }
}

/// `GameStarting` for this fixture table.
pub fn game_starting() -> ServerMessage {
    ServerMessage::GameStarting {
        players: players(),
        round_count: 5,
    }
}

/// A `YourHand` carrying the fixture opening hand.
pub fn your_hand() -> ServerMessage {
    ServerMessage::YourHand { cards: hand() }
}

/// A Thin Ice modifier revealed for round 2.
pub fn modifier_thin_ice() -> ServerMessage {
    ServerMessage::ModifierRevealed {
        modifier: ModifierKind::ThinIce,
        round_number: 2,
    }
}

/// A wave-open message with explicit round/wave/timer.
pub fn wave_open(round: u8, wave: u8, timer_ms: u32) -> ServerMessage {
    ServerMessage::WaveOpened {
        round_number: round,
        wave_number: wave,
        timer_ms,
    }
}

/// A safe-brew Domination result (Sapphire takes the pot).
pub fn round_scored_domination() -> ServerMessage {
    ServerMessage::RoundScored {
        color_points: vec![(Color::Ruby, 2), (Color::Sapphire, 3)],
        outcome: ScoringOutcome::Domination {
            winner: Color::Sapphire,
        },
        awards: vec![PlayerScore {
            player: pid(1),
            score: 5,
        }],
    }
}

/// An explosion costing every player the pot.
pub fn explosion() -> ServerMessage {
    ServerMessage::Explosion {
        pot_value: 7,
        deltas: (0..4)
            .map(|i| PlayerScore {
                player: pid(i),
                score: -7,
            })
            .collect(),
        shielded: vec![],
    }
}

/// A game-over with Sapphire (seat 1) the sole winner.
pub fn game_over() -> ServerMessage {
    ServerMessage::GameOver {
        final_scores: (0..4)
            .map(|i| PlayerScore {
                player: pid(i),
                score: if i == 1 { 12 } else { -3 },
            })
            .collect(),
        winners: vec![pid(1)],
    }
}

/// A reconnection `StateSnapshot` mid-round-2 (scoped to seat 0's knowledge).
pub fn state_snapshot() -> ServerMessage {
    ServerMessage::StateSnapshot {
        room_code: RoomCode("BREW-7K3F".into()),
        your_player_id: pid(0),
        round_number: 2,
        players: players(),
        scores: (0..4)
            .map(|i| PlayerScore {
                player: pid(i),
                score: i as i32,
            })
            .collect(),
        active_modifiers: vec![ModifierKind::ThinIce],
        contributions: vec![Contribution {
            player: pid(1),
            count: 2,
        }],
        your_hand: hand(),
    }
}

/// A complete, modest game used by the in-process mock (`--mock`).
pub fn demo_game() -> Vec<ServerMessage> {
    let ps = players();
    let p = |i: u8| pid(i);
    vec![
        room_joined(),
        ServerMessage::GameStarting {
            players: ps.clone(),
            round_count: 5,
        },
        ServerMessage::YourHand { cards: hand() },
        ServerMessage::WaveOpened {
            round_number: 1,
            wave_number: 1,
            timer_ms: 30_000,
        },
        ServerMessage::EmoteBroadcast {
            from: p(2),
            emote: EmoteId(2),
        },
        ServerMessage::WaveResolved {
            played: vec![p(0), p(1)],
            passed: vec![],
            cauldron_card_count: 2,
            contributions: vec![
                Contribution {
                    player: p(0),
                    count: 1,
                },
                Contribution {
                    player: p(1),
                    count: 1,
                },
            ],
        },
        ServerMessage::WaveOpened {
            round_number: 1,
            wave_number: 2,
            timer_ms: 10_000,
        },
        ServerMessage::WaveResolved {
            played: vec![p(0)],
            passed: vec![p(2), p(3)],
            cauldron_card_count: 3,
            contributions: vec![
                Contribution {
                    player: p(0),
                    count: 2,
                },
                Contribution {
                    player: p(1),
                    count: 1,
                },
            ],
        },
        depile_safe(),
        ServerMessage::RoundScored {
            color_points: vec![(Color::Ruby, 2), (Color::Sapphire, 3)],
            outcome: ScoringOutcome::Domination {
                winner: Color::Sapphire,
            },
            awards: vec![PlayerScore {
                player: p(1),
                score: 5,
            }],
        },
        ServerMessage::ScoreUpdate {
            scores: vec![
                PlayerScore {
                    player: p(0),
                    score: 0,
                },
                PlayerScore {
                    player: p(1),
                    score: 5,
                },
                PlayerScore {
                    player: p(2),
                    score: 0,
                },
                PlayerScore {
                    player: p(3),
                    score: 0,
                },
            ],
        },
        ServerMessage::ModifierRevealed {
            modifier: ModifierKind::ThinIce,
            round_number: 2,
        },
        ServerMessage::YourHand { cards: hand() },
        ServerMessage::WaveOpened {
            round_number: 2,
            wave_number: 1,
            timer_ms: 30_000,
        },
        ServerMessage::WaveResolved {
            played: vec![p(0), p(1), p(2)],
            passed: vec![p(3)],
            cauldron_card_count: 3,
            contributions: vec![
                Contribution {
                    player: p(0),
                    count: 1,
                },
                Contribution {
                    player: p(1),
                    count: 1,
                },
                Contribution {
                    player: p(2),
                    count: 1,
                },
            ],
        },
        depile_boom(),
        ServerMessage::Explosion {
            pot_value: 7,
            deltas: vec![
                PlayerScore {
                    player: p(0),
                    score: -7,
                },
                PlayerScore {
                    player: p(1),
                    score: -7,
                },
                PlayerScore {
                    player: p(2),
                    score: -7,
                },
                PlayerScore {
                    player: p(3),
                    score: -7,
                },
            ],
            shielded: vec![],
        },
        ServerMessage::ScoreUpdate {
            scores: vec![
                PlayerScore {
                    player: p(0),
                    score: -7,
                },
                PlayerScore {
                    player: p(1),
                    score: -2,
                },
                PlayerScore {
                    player: p(2),
                    score: -7,
                },
                PlayerScore {
                    player: p(3),
                    score: -7,
                },
            ],
        },
        ServerMessage::GameOver {
            final_scores: vec![
                PlayerScore {
                    player: p(0),
                    score: -7,
                },
                PlayerScore {
                    player: p(1),
                    score: -2,
                },
                PlayerScore {
                    player: p(2),
                    score: -7,
                },
                PlayerScore {
                    player: p(3),
                    score: -7,
                },
            ],
            winners: vec![p(1)],
        },
    ]
}
