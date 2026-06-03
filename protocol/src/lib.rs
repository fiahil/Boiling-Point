//! Boiling Point wire protocol.
//!
//! This crate is the narrow waist between the authoritative server and any
//! client or bot. It contains ONLY the message DTOs that cross the wire, the
//! public value-types they reference, and the [`codec`] helpers — no game logic
//! and no server-only secrets. In particular, the boiling point is never a field
//! of a general state message; it surfaces only via [`server::ServerMessage::PeekResult`]
//! (private) and an exploded [`server::ServerMessage::Depile`].

pub mod client;
pub mod codec;
pub mod ids;
pub mod server;
pub mod vocab;

pub use client::{ClientMessage, PROTOCOL_VERSION, ProtocolVersion};
pub use codec::{CodecError, decode, decode_json, encode, encode_json};
pub use ids::{CardId, EmoteId, GroupCode, PlayerId};
pub use server::{Audience, Outbound, ServerMessage};
pub use vocab::{CardView, Color, EffectKind, HandCard, ModifierKind};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::*;
    use crate::vocab::*;

    fn sample_player() -> PlayerId {
        PlayerId(uuid::Uuid::from_u128(1))
    }

    fn sample_card() -> CardView {
        CardView {
            color: Color::Ruby,
            volatility: 2,
            points: 3,
            effect: Some(EffectKind::Peek),
        }
    }

    /// Every client message must survive a MessagePack round-trip.
    #[test]
    fn client_messages_roundtrip_msgpack() {
        let msgs = vec![
            ClientMessage::JoinGroup {
                protocol_version: PROTOCOL_VERSION,
                display_name: "alice".into(),
                session_token: None,
                group_code: GroupCode("BREW-7K3F".into()),
            },
            ClientMessage::CreateGroup {
                protocol_version: PROTOCOL_VERSION,
                display_name: "bob".into(),
                session_token: Some("tok".into()),
            },
            ClientMessage::EnqueueMatch {
                protocol_version: PROTOCOL_VERSION,
                display_name: "cara".into(),
                session_token: None,
            },
            ClientMessage::CommitCard { card: CardId(7) },
            ClientMessage::CommitPass,
            ClientMessage::LockIn,
            ClientMessage::Emote { emote: EmoteId(3) },
            ClientMessage::PlayAgain,
            ClientMessage::FillGroup,
            ClientMessage::CancelFill,
            ClientMessage::LeaveGroup,
            ClientMessage::Heartbeat,
        ];
        for m in msgs {
            let bytes = encode(&m).expect("encode");
            let back: ClientMessage = decode(&bytes).expect("decode");
            assert_eq!(m, back);
        }
    }

    /// Every server message must survive both MessagePack and JSON round-trips.
    #[test]
    fn server_messages_roundtrip_both_formats() {
        let p = sample_player();
        let msgs = vec![
            ServerMessage::GroupJoined {
                group_code: GroupCode("BREW-7K3F".into()),
                your_player_id: p,
                your_color: Color::Sapphire,
                session_token: "session-token".into(),
                players: vec![PlayerPublic {
                    id: p,
                    display_name: "alice".into(),
                    color: Color::Sapphire,
                    connected: true,
                    guest: false,
                }],
            },
            ServerMessage::LeftGroup,
            ServerMessage::GroupSearching { needed: 1 },
            ServerMessage::StandingsUpdate {
                members: vec![MemberStanding {
                    player: p,
                    games_played: 3,
                    wins: 2,
                }],
                guest_games: 1,
                guest_wins: 0,
            },
            ServerMessage::YourHand {
                cards: vec![HandCard {
                    id: CardId(1),
                    view: sample_card(),
                }],
            },
            ServerMessage::WaveOpened {
                round_number: 1,
                wave_number: 1,
                timer_ms: 30_000,
                final_wave: false,
            },
            ServerMessage::WaveResolved {
                played: vec![p],
                passed: vec![],
                cauldron_card_count: 1,
                contributions: vec![Contribution {
                    player: p,
                    count: 1,
                }],
            },
            ServerMessage::ModifierRevealed {
                modifier: ModifierKind::ThinIce,
                round_number: 2,
            },
            ServerMessage::SomeonePeeked,
            ServerMessage::Exposed {
                card: sample_card(),
            },
            ServerMessage::DeckReshuffled,
            ServerMessage::EmoteBroadcast {
                from: p,
                emote: EmoteId(2),
            },
            ServerMessage::PeekResult { boiling_point: 11 },
            ServerMessage::Depile {
                reveals: vec![DepileEntry {
                    player: p,
                    card: sample_card(),
                    running_volatility: 2,
                }],
                exploded: true,
                boiling_point: Some(10),
                crossing_index: Some(0),
            },
            ServerMessage::RoundScored {
                color_points: vec![(Color::Ruby, 3)],
                outcome: ScoringOutcome::Domination {
                    winner: Color::Ruby,
                },
                awards: vec![PlayerScore {
                    player: p,
                    score: 3,
                }],
            },
            ServerMessage::Explosion {
                pot_value: 9,
                deltas: vec![PlayerScore {
                    player: p,
                    score: -9,
                }],
                shielded: vec![],
            },
            ServerMessage::ScoreUpdate {
                scores: vec![PlayerScore {
                    player: p,
                    score: 3,
                }],
            },
            ServerMessage::GameOver {
                final_scores: vec![PlayerScore {
                    player: p,
                    score: 3,
                }],
                winners: vec![p],
            },
            ServerMessage::Error {
                code: ErrorCode::WrongPhase,
                message: "not now".into(),
            },
            ServerMessage::PlayerConnectionChanged {
                player: p,
                connected: false,
            },
            ServerMessage::StateSnapshot {
                group_code: GroupCode("BREW-7K3F".into()),
                your_player_id: p,
                round_number: 3,
                players: vec![],
                scores: vec![PlayerScore {
                    player: p,
                    score: 5,
                }],
                active_modifiers: vec![ModifierKind::ThinIce],
                contributions: vec![Contribution {
                    player: p,
                    count: 2,
                }],
                your_hand: vec![HandCard {
                    id: CardId(1),
                    view: sample_card(),
                }],
            },
            ServerMessage::DeathmatchStarted {
                participants: vec![p],
            },
            ServerMessage::Heartbeat,
        ];
        for m in msgs {
            let bytes = encode(&m).expect("msgpack encode");
            assert_eq!(m, decode::<ServerMessage>(&bytes).expect("msgpack decode"));
            let json = encode_json(&m).expect("json encode");
            assert_eq!(m, decode_json::<ServerMessage>(&json).expect("json decode"));
        }
    }

    /// The boiling point must appear ONLY in `PeekResult` and an exploded `Depile`.
    /// Serialising any other variant must not produce a `boiling_point` field.
    #[test]
    fn boiling_point_only_in_allowed_messages() {
        let p = sample_player();
        // A representative set of non-secret messages that must never carry it.
        let non_secret = vec![
            ServerMessage::WaveResolved {
                played: vec![p],
                passed: vec![],
                cauldron_card_count: 3,
                contributions: vec![],
            },
            ServerMessage::ScoreUpdate {
                scores: vec![PlayerScore {
                    player: p,
                    score: 0,
                }],
            },
            ServerMessage::Depile {
                reveals: vec![],
                exploded: false,
                boiling_point: None, // safe brew: must be None
                crossing_index: None,
            },
            ServerMessage::StateSnapshot {
                group_code: GroupCode("BREW-7K3F".into()),
                your_player_id: p,
                round_number: 2,
                players: vec![],
                scores: vec![],
                active_modifiers: vec![],
                contributions: vec![],
                your_hand: vec![],
            },
        ];
        for m in non_secret {
            let json = encode_json(&m).unwrap();
            assert!(
                !json.contains("boiling_point") || json.contains("\"boiling_point\":null"),
                "non-secret message leaked a boiling point: {json}"
            );
        }
        // The two allowed carriers do contain it.
        let peek = encode_json(&ServerMessage::PeekResult { boiling_point: 12 }).unwrap();
        assert!(peek.contains("boiling_point"));
    }

    /// Building an `Outbound` must classify audiences correctly.
    #[test]
    fn audience_routing_is_explicit() {
        let p = sample_player();
        let private = ServerMessage::YourHand { cards: vec![] }.to(p);
        assert!(matches!(private.audience, Audience::Private(_)));
        let public = ServerMessage::SomeonePeeked.broadcast();
        assert!(matches!(public.audience, Audience::Broadcast));
        assert!(ServerMessage::PeekResult { boiling_point: 1 }.is_private_only());
        assert!(!ServerMessage::SomeonePeeked.is_private_only());
    }
}
