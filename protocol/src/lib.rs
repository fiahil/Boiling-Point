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
pub mod frame;
pub mod ids;
pub mod server;
pub mod vocab;

pub use client::{ClientMessage, PROTOCOL_VERSION, ProtocolVersion};
pub use codec::{CodecError, decode, decode_json, encode, encode_json};
pub use frame::{CastableSpell, PendingDecision, PlayableIngredient, TargetOptions};
pub use ids::{CardId, EmoteId, GroupCode, PlayerId};
pub use server::{Audience, Outbound, ServerMessage};
pub use vocab::{
    Brewer, Color, GrimoireBucket, HandIngredient, HandSpell, IngredientView, ModifierKind,
    PantryBucket, Recipe, SpellKind, SpellMode, SpellTarget, TargetKind,
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

    fn sample_recipe() -> Recipe {
        Recipe {
            pantry: vec![
                PantryBucket::Nightshade,
                PantryBucket::Saffron,
                PantryBucket::Bilberry,
            ],
            grimoire: vec![GrimoireBucket::Ironbark, GrimoireBucket::Brimstone],
            reserves: vec![SpellKind::Redirect],
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
            ClientMessage::CommitDefer,
            ClientMessage::PickBrewer {
                brewer: Brewer::Cinderwright,
            },
            ClientMessage::SubmitRecipe {
                recipe: sample_recipe(),
            },
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
            ServerMessage::DecisionFrame {
                round_number: 1,
                wave_number: 2,
                timer_ms: Some(15_000),
                decision: PendingDecision::WaveCommit {
                    playable: vec![PlayableIngredient {
                        ingredient: HandIngredient {
                            id: CardId(1),
                            view: sample_ingredient(),
                        },
                        colorless_allowed: true,
                    }],
                    can_pass: true,
                    spells: vec![
                        CastableSpell {
                            spell: CardId(2),
                            kind: SpellKind::Peek,
                            targets: TargetOptions::None,
                        },
                        CastableSpell {
                            spell: CardId(3),
                            kind: SpellKind::Hex,
                            targets: TargetOptions::Players { players: vec![p] },
                        },
                        CastableSpell {
                            spell: CardId(4),
                            kind: SpellKind::Sour,
                            targets: TargetOptions::Colors {
                                colors: Color::PLAYER_COLORS.to_vec(),
                            },
                        },
                    ],
                    can_defer: true,
                },
            },
            ServerMessage::DecisionFrame {
                round_number: 0,
                wave_number: 0,
                timer_ms: Some(20_000),
                decision: PendingDecision::BrewerPick {
                    options: vec![Brewer::Featherhand, Brewer::Lurker],
                },
            },
            ServerMessage::BrewersRevealed {
                brewers: vec![server::PlayerBrewer {
                    player: p,
                    brewer: Brewer::Broker,
                }],
            },
            ServerMessage::DecisionFrame {
                round_number: 0,
                wave_number: 0,
                timer_ms: Some(30_000),
                decision: PendingDecision::ApothecaryDraft {
                    pantry_options: PantryBucket::ALL.to_vec(),
                    grimoire_options: GrimoireBucket::ALL.to_vec(),
                    picks_min: 2,
                    picks_max: 3,
                    bonus_buckets: 1,
                    reserves_max: 2,
                    suggested: sample_recipe(),
                },
            },
            ServerMessage::RecipesRevealed {
                recipes: vec![server::PlayerRecipe {
                    player: p,
                    recipe: sample_recipe(),
                }],
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
                brewers: vec![server::PlayerBrewer {
                    player: p,
                    brewer: Brewer::Forager,
                }],
                recipes: vec![server::PlayerRecipe {
                    player: p,
                    recipe: sample_recipe(),
                }],
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
                brewers: vec![],
                recipes: vec![],
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

    /// A decision frame carries only recipient-permitted information: no
    /// `boiling_point` field exists anywhere in its shape, and the frame is
    /// private-only so it can never ride a broadcast.
    #[test]
    fn decision_frames_are_private_and_carry_no_secret() {
        let p = sample_player();
        let frame = ServerMessage::DecisionFrame {
            round_number: 2,
            wave_number: 1,
            timer_ms: Some(25_000),
            decision: PendingDecision::WaveCommit {
                playable: vec![PlayableIngredient {
                    ingredient: HandIngredient {
                        id: CardId(1),
                        view: sample_ingredient(),
                    },
                    colorless_allowed: true,
                }],
                can_pass: true,
                spells: vec![CastableSpell {
                    spell: CardId(2),
                    kind: SpellKind::Redirect,
                    targets: TargetOptions::Players { players: vec![p] },
                }],
                can_defer: false,
            },
        };
        assert!(frame.is_private_only());
        let json = encode_json(&frame).unwrap();
        assert!(
            !json.contains("boiling_point"),
            "a decision frame must never carry the boiling point: {json}"
        );
    }

    /// The frame's `permits_*` helpers accept exactly the enumerated actions.
    #[test]
    fn frame_permits_exactly_the_enumerated_actions() {
        let p = sample_player();
        let other = PlayerId(uuid::Uuid::from_u128(2));
        let decision = PendingDecision::WaveCommit {
            playable: vec![PlayableIngredient {
                ingredient: HandIngredient {
                    id: CardId(1),
                    view: sample_ingredient(),
                },
                colorless_allowed: true,
            }],
            can_pass: true,
            spells: vec![
                CastableSpell {
                    spell: CardId(2),
                    kind: SpellKind::Peek,
                    targets: TargetOptions::None,
                },
                CastableSpell {
                    spell: CardId(3),
                    kind: SpellKind::Hex,
                    targets: TargetOptions::Players { players: vec![p] },
                },
                CastableSpell {
                    spell: CardId(4),
                    kind: SpellKind::Sour,
                    targets: TargetOptions::Colors {
                        colors: Color::PLAYER_COLORS.to_vec(),
                    },
                },
            ],
            can_defer: false,
        };
        // Plays: the held card (either way), never an unlisted card.
        assert!(decision.permits_play(CardId(1), false));
        assert!(decision.permits_play(CardId(1), true));
        assert!(!decision.permits_play(CardId(99), false));
        assert!(decision.permits_pass());
        assert!(!decision.permits_defer(), "defer is off unless enumerated");
        assert!(
            !decision.permits_pick(Brewer::Lurker),
            "a wave commit never accepts a brewer pick"
        );
        // Casts: bare Peek, Hex at the listed player only, Sour at player
        // colours only — and never an unlisted spell.
        assert!(decision.permits_cast(CardId(2), None));
        assert!(!decision.permits_cast(CardId(2), Some(SpellTarget::Player { player: p })));
        assert!(decision.permits_cast(CardId(3), Some(SpellTarget::Player { player: p })));
        assert!(!decision.permits_cast(CardId(3), Some(SpellTarget::Player { player: other })));
        assert!(!decision.permits_cast(CardId(3), None));
        assert!(decision.permits_cast(CardId(4), Some(SpellTarget::Color { color: Color::Ruby })));
        assert!(!decision.permits_cast(CardId(4), Some(SpellTarget::Color { color: Color::Wild })));
        assert!(!decision.permits_cast(CardId(99), None));
    }

    /// Brewer metadata is total: exactly 12, unique names that round-trip
    /// through `by_name`, each with a non-empty one-sentence bent rule.
    #[test]
    fn brewer_metadata_is_total() {
        assert_eq!(Brewer::ALL.len(), 12);
        let mut names: Vec<&str> = Brewer::ALL.iter().map(|b| b.name()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 12, "brewer names must be unique");
        for brewer in Brewer::ALL {
            assert_eq!(Brewer::by_name(brewer.name()), Some(brewer));
            assert!(!brewer.bent_rule().is_empty());
        }
        assert_eq!(Brewer::by_name("Bartender"), None);
    }

    /// A brewer-pick frame permits exactly its two offered options and none of
    /// the wave-commit actions.
    #[test]
    fn brewer_pick_frame_permits_only_its_pair() {
        let decision = PendingDecision::BrewerPick {
            options: vec![Brewer::Featherhand, Brewer::Broker],
        };
        assert!(decision.permits_pick(Brewer::Featherhand));
        assert!(decision.permits_pick(Brewer::Broker));
        assert!(!decision.permits_pick(Brewer::Lurker));
        assert!(!decision.permits_pass());
        assert!(!decision.permits_play(CardId(1), false));
        assert!(!decision.permits_cast(CardId(1), None));
        assert!(!decision.permits_defer());
    }

    /// Bucket metadata is total: 12 pantry + 8 grimoire buckets, unique names
    /// that round-trip through `by_name`, non-empty blurbs, every grimoire
    /// role-group non-empty and inside the 15 kinds, the union covering all 15,
    /// and god-tier exactly the Eyebright + Ironbark families.
    #[test]
    fn bucket_metadata_is_total() {
        assert_eq!(PantryBucket::ALL.len(), 12);
        assert_eq!(GrimoireBucket::ALL.len(), 8);
        let mut names: Vec<&str> = PantryBucket::ALL.iter().map(|b| b.name()).collect();
        names.extend(GrimoireBucket::ALL.iter().map(|b| b.name()));
        let total = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), total, "bucket names must be unique");
        for bucket in PantryBucket::ALL {
            assert_eq!(PantryBucket::by_name(bucket.name()), Some(bucket));
            assert!(!bucket.blurb().is_empty());
        }
        assert_eq!(
            PantryBucket::ALL.iter().filter(|b| b.is_toolkit()).count(),
            2,
            "exactly Ochre and Wisp are toolkit"
        );
        let mut covered: Vec<SpellKind> = Vec::new();
        for bucket in GrimoireBucket::ALL {
            assert_eq!(GrimoireBucket::by_name(bucket.name()), Some(bucket));
            assert!(!bucket.blurb().is_empty());
            assert!(!bucket.spells().is_empty());
            covered.extend_from_slice(bucket.spells());
        }
        covered.sort_by_key(|k| format!("{k:?}"));
        covered.dedup();
        assert_eq!(covered.len(), 15, "the role-groups cover all 15 spells");
        for kind in GrimoireBucket::GOD_TIER_SPELLS {
            assert!(
                GrimoireBucket::ALL
                    .iter()
                    .filter(|b| b.is_god_tier())
                    .any(|b| b.spells().contains(&kind)),
                "{kind:?} must come from a god-tier bucket"
            );
        }
    }

    /// An Apothecary-draft frame permits exactly the legal recipes: distinct
    /// offered buckets, ledger counts within the allowance (the bonus bucket
    /// in one ledger only), reserves within the allowance and inside taken
    /// role-groups — and none of the other decision kinds' actions.
    #[test]
    fn draft_frame_permits_exactly_the_legal_recipes() {
        let frame = |bonus: u8, reserves_max: u8| PendingDecision::ApothecaryDraft {
            pantry_options: PantryBucket::ALL.to_vec(),
            grimoire_options: GrimoireBucket::ALL.to_vec(),
            picks_min: 2,
            picks_max: 3,
            bonus_buckets: bonus,
            reserves_max,
            suggested: sample_recipe(),
        };
        let plain = frame(0, 1);
        let base = sample_recipe();
        assert!(plain.permits_recipe(&base));
        // No other decision kind's submissions are legal on a draft frame.
        assert!(!plain.permits_pass());
        assert!(!plain.permits_play(CardId(1), false));
        assert!(!plain.permits_cast(CardId(1), None));
        assert!(!plain.permits_pick(Brewer::Lurker));
        assert!(!PendingDecision::BrewerPick { options: vec![] }.permits_recipe(&base));

        // Too few / too many buckets.
        let mut thin = base.clone();
        thin.pantry.truncate(1);
        assert!(!plain.permits_recipe(&thin));
        let mut fat = base.clone();
        fat.pantry.push(PantryBucket::Sage);
        assert!(!plain.permits_recipe(&fat), "a 4th bucket needs the bonus");
        assert!(
            frame(1, 1).permits_recipe(&fat),
            "the Connoisseur takes a 4th in one ledger"
        );
        let mut both_fat = fat.clone();
        both_fat.grimoire.push(GrimoireBucket::Farsight);
        both_fat.grimoire.push(GrimoireBucket::Mandrake);
        assert!(
            !frame(1, 1).permits_recipe(&both_fat),
            "the bonus bucket lands in ONE ledger, never both"
        );

        // Duplicates are never legal.
        let mut duped = base.clone();
        duped.pantry[1] = duped.pantry[0];
        assert!(!plain.permits_recipe(&duped));

        // Reserves: within the allowance, inside a taken role-group.
        let mut two_reserves = base.clone();
        two_reserves.reserves = vec![SpellKind::Redirect, SpellKind::Hex];
        assert!(!plain.permits_recipe(&two_reserves), "one reserve only");
        assert!(
            frame(0, 2).permits_recipe(&two_reserves),
            "the Reservist locks two"
        );
        let mut foreign_reserve = base.clone();
        foreign_reserve.reserves = vec![SpellKind::Peek];
        assert!(
            !plain.permits_recipe(&foreign_reserve),
            "a reserve must come from a taken bucket"
        );
        let mut no_reserve = base.clone();
        no_reserve.reserves.clear();
        assert!(plain.permits_recipe(&no_reserve), "the reserve is optional");

        // An unoffered bucket (a filtered Ironbark) is rejected.
        let no_ironbark = PendingDecision::ApothecaryDraft {
            pantry_options: PantryBucket::ALL.to_vec(),
            grimoire_options: GrimoireBucket::ALL
                .into_iter()
                .filter(|b| *b != GrimoireBucket::Ironbark)
                .collect(),
            picks_min: 2,
            picks_max: 3,
            bonus_buckets: 0,
            reserves_max: 1,
            suggested: sample_recipe(),
        };
        assert!(!no_ironbark.permits_recipe(&base));
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
