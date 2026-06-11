//! Boiling Point wire protocol.
//!
//! This crate is the narrow waist between the authoritative server and any
//! client or bot. It contains ONLY the message DTOs that cross the wire, the
//! public value-types they reference, and the [`codec`] helpers — no game logic
//! and no server-only secrets. In particular, the boiling point is never a field
//! of a general state message; in-round it surfaces only via the private
//! [`server::ServerMessage::PeekResult`], and post-round via the
//! [`server::ServerMessage::Depile`] (which reveals it **every** round, boom and
//! safe, per the boom2 combat core).

pub mod client;
pub mod codec;
pub mod ids;
pub mod server;
pub mod vocab;

pub use client::{ClientMessage, PROTOCOL_VERSION, ProtocolVersion};
pub use codec::{CodecError, decode, decode_json, encode, encode_json};
pub use ids::{CardId, EmoteId, GroupCode, PlayerId};
pub use server::{Audience, Outbound, ServerMessage};
pub use vocab::{
    Color, HandIngredient, HandSpell, IngredientView, ModifierKind, SpellKind, SpellMode,
    SpellTarget, TargetKind,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::*;
    use crate::vocab::*;

    fn sample_player() -> PlayerId {
        PlayerId(uuid::Uuid::from_u128(1))
    }

    fn sample_ingredient() -> IngredientView {
        IngredientView {
            color: Color::Ruby,
            volatility: 5,
            points: 3,
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
            ClientMessage::CommitIngredient {
                card: CardId(7),
                colorless: false,
            },
            ClientMessage::CommitIngredient {
                card: CardId(8),
                colorless: true,
            },
            ClientMessage::CastSpell {
                spell: CardId(9),
                target: None,
            },
            ClientMessage::CastSpell {
                spell: CardId(10),
                target: Some(SpellTarget::Player {
                    player: sample_player(),
                }),
            },
            ClientMessage::CastSpell {
                spell: CardId(11),
                target: Some(SpellTarget::Color {
                    color: Color::Emerald,
                }),
            },
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
                ingredients: vec![HandIngredient {
                    id: CardId(1),
                    view: sample_ingredient(),
                }],
                spells: vec![HandSpell {
                    id: CardId(2),
                    kind: SpellKind::Peek,
                }],
            },
            ServerMessage::WaveOpened {
                round_number: 1,
                wave_number: 1,
                timer_ms: 25_000,
                final_wave: false,
            },
            ServerMessage::SpellCast {
                player: p,
                spell: SpellKind::Surge,
                color_target: None,
            },
            ServerMessage::SpellCast {
                player: p,
                spell: SpellKind::DoubleDown,
                color_target: Some(Color::Ruby),
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
            ServerMessage::Exposed {
                player: p,
                ingredient: sample_ingredient(),
                colorless: true,
            },
            ServerMessage::EmoteBroadcast {
                from: p,
                emote: EmoteId(2),
            },
            ServerMessage::PeekResult { boiling_point: 26 },
            ServerMessage::AssayResult {
                dominant: Some(Color::Ruby),
                lead: 4,
            },
            ServerMessage::Depile {
                reveals: vec![DepileEntry {
                    player: p,
                    ingredient: sample_ingredient(),
                    colorless: false,
                    wave_number: 2,
                    running_volatility: 5,
                    liable: true,
                }],
                exploded: true,
                boiling_point: 24,
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
                fired: vec![SpellFire {
                    player: p,
                    spell: SpellKind::Harvest,
                    target: None,
                }],
            },
            ServerMessage::Explosion {
                pot_value: 9,
                detonators: vec![p],
                deltas: vec![PlayerScore {
                    player: p,
                    score: -9,
                }],
                fired: vec![SpellFire {
                    player: p,
                    spell: SpellKind::Redirect,
                    target: Some(p),
                }],
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
                your_ingredients: vec![HandIngredient {
                    id: CardId(1),
                    view: sample_ingredient(),
                }],
                your_spells: vec![HandSpell {
                    id: CardId(3),
                    kind: SpellKind::Hex,
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

    /// The boiling point must appear ONLY in `PeekResult` (private, in-round) and
    /// the `Depile` (post-round, every round). Serialising any other variant must
    /// not produce a `boiling_point` field.
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
            ServerMessage::SpellCast {
                player: p,
                spell: SpellKind::Surge,
                color_target: None,
            },
            ServerMessage::ScoreUpdate {
                scores: vec![PlayerScore {
                    player: p,
                    score: 0,
                }],
            },
            ServerMessage::StateSnapshot {
                group_code: GroupCode("BREW-7K3F".into()),
                your_player_id: p,
                round_number: 2,
                players: vec![],
                scores: vec![],
                active_modifiers: vec![],
                contributions: vec![],
                your_ingredients: vec![],
                your_spells: vec![],
            },
        ];
        for m in non_secret {
            let json = encode_json(&m).unwrap();
            assert!(
                !json.contains("boiling_point"),
                "non-secret message leaked a boiling point: {json}"
            );
        }
        // The two allowed carriers do contain it — including the SAFE depile,
        // which reveals the line every round (the near-miss payoff).
        let peek = encode_json(&ServerMessage::PeekResult { boiling_point: 26 }).unwrap();
        assert!(peek.contains("boiling_point"));
        let safe_depile = encode_json(&ServerMessage::Depile {
            reveals: vec![],
            exploded: false,
            boiling_point: 26,
            crossing_index: None,
        })
        .unwrap();
        assert!(safe_depile.contains("boiling_point"));
    }

    /// Spell metadata is total: every one of the 15 spells has a mode and a
    /// target kind, and Actives are exactly the ward/cash-in/curse five.
    #[test]
    fn spell_metadata_is_total_and_matches_design() {
        assert_eq!(SpellKind::ALL.len(), 15);
        let actives: Vec<SpellKind> = SpellKind::ALL
            .into_iter()
            .filter(|s| s.mode() == SpellMode::Active)
            .collect();
        assert_eq!(
            actives,
            vec![
                SpellKind::Cap,
                SpellKind::Halve,
                SpellKind::Redirect,
                SpellKind::Harvest,
                SpellKind::Hex,
            ]
        );
        for kind in SpellKind::ALL {
            match kind {
                SpellKind::Redirect | SpellKind::Hex => {
                    assert_eq!(kind.target_kind(), TargetKind::Player)
                }
                SpellKind::DoubleDown | SpellKind::Sour => {
                    assert_eq!(kind.target_kind(), TargetKind::Color)
                }
                _ => assert_eq!(kind.target_kind(), TargetKind::None),
            }
        }
    }

    /// Building an `Outbound` must classify audiences correctly.
    #[test]
    fn audience_routing_is_explicit() {
        let p = sample_player();
        let private = ServerMessage::YourHand {
            ingredients: vec![],
            spells: vec![],
        }
        .to(p);
        assert!(matches!(private.audience, Audience::Private(_)));
        let public = ServerMessage::SpellCast {
            player: p,
            spell: SpellKind::Peek,
            color_target: None,
        }
        .broadcast();
        assert!(matches!(public.audience, Audience::Broadcast));
        assert!(ServerMessage::PeekResult { boiling_point: 1 }.is_private_only());
        assert!(
            ServerMessage::AssayResult {
                dominant: None,
                lead: 0
            }
            .is_private_only()
        );
        assert!(
            !ServerMessage::SpellCast {
                player: p,
                spell: SpellKind::Peek,
                color_target: None,
            }
            .is_private_only()
        );
    }

    /// The secret-routing rail must be load-bearing: broadcasting a private-only
    /// message trips the `is_private_only()` debug-assert (debug builds), so a
    /// future edit that broadcasts a hand/peek/error fails loudly in tests.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "private-only")]
    fn broadcasting_a_private_message_trips_the_guard() {
        let _ = ServerMessage::Error {
            code: ErrorCode::Internal,
            message: "boom".into(),
        }
        .broadcast();
    }
}
