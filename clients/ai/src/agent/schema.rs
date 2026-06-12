//! Decision-frame → tool-schema derivation and the response → answer mapping.
//!
//! The tool the model is forced to call is generated from the frame's legal
//! action set, resolving the `boom-decision-frame` design question as an
//! **action template with enumerated value lists**: card/spell ids and targets
//! appear as JSON-Schema `enum`s drawn verbatim from the frame, and the
//! description spells out each spell's legal targets. The model therefore
//! cannot *name* anything outside the frame; the cross-field combinations a
//! flat schema cannot express (this spell with that target) are enforced by
//! the mapping gate — every mapped answer passes [`Answer::is_legal`] before
//! submission, and anything unparseable or illegal triggers the fallback.

use boiling_point_protocol::CardId;
use boiling_point_protocol::PlayerId;
use boiling_point_protocol::frame::{PendingDecision, TargetOptions};
use boiling_point_protocol::vocab::{Color, SpellTarget, TargetKind};
use serde_json::{Value, json};

use crate::agent::api::ToolDefinition;
use crate::brain::{Answer, SpellCast, WaveAction};

/// The forced decision tool's name.
pub const TOOL_NAME: &str = "commit_wave";

/// A stable wire/report name for a colour (the schema's colour enum values).
fn color_name(color: Color) -> &'static str {
    match color {
        Color::Ruby => "Ruby",
        Color::Sapphire => "Sapphire",
        Color::Emerald => "Emerald",
        Color::Amethyst => "Amethyst",
        Color::Wild => "Wild",
    }
}

/// Parse a colour enum value back.
fn color_by_name(name: &str) -> Option<Color> {
    [
        Color::Ruby,
        Color::Sapphire,
        Color::Emerald,
        Color::Amethyst,
    ]
    .into_iter()
    .find(|c| color_name(*c) == name)
}

/// Derive the tool definition for a wave-commit frame: every enum value is
/// drawn from the frame's enumerated legal set.
pub fn tool_from_frame(decision: &PendingDecision) -> ToolDefinition {
    let PendingDecision::WaveCommit {
        playable,
        can_pass,
        spells,
    } = decision;

    let card_ids: Vec<u32> = playable.iter().map(|p| p.ingredient.id.0).collect();
    let mut actions: Vec<&str> = Vec::new();
    if !card_ids.is_empty() {
        actions.push("play");
    }
    if *can_pass {
        actions.push("pass");
    }

    let mut properties = serde_json::Map::new();
    properties.insert(
        "action".into(),
        json!({
            "type": "string",
            "enum": actions,
            "description": "Play an ingredient into the cauldron, or pass (passing locks you out for the rest of the round)."
        }),
    );
    if !card_ids.is_empty() {
        let card_lines: Vec<String> = playable
            .iter()
            .map(|p| {
                format!(
                    "card {} = {} ingredient, volatility {}, points {}",
                    p.ingredient.id.0,
                    color_name(p.ingredient.view.color),
                    p.ingredient.view.volatility,
                    p.ingredient.view.points,
                )
            })
            .collect();
        properties.insert(
            "card".into(),
            json!({
                "type": "integer",
                "enum": card_ids,
                "description": format!("Required when action is \"play\". Your hand: {}.", card_lines.join("; "))
            }),
        );
        properties.insert(
            "colorless".into(),
            json!({
                "type": "boolean",
                "description": "Play the card colorless: its volatility still enters the cauldron but it scores zero points and serves no colour. Default false."
            }),
        );
    }
    if !spells.is_empty() {
        let spell_ids: Vec<u32> = spells.iter().map(|s| s.spell.0).collect();
        let mut player_targets: Vec<String> = Vec::new();
        let mut color_targets: Vec<&str> = Vec::new();
        let spell_lines: Vec<String> = spells
            .iter()
            .map(|s| {
                let target_note = match &s.targets {
                    TargetOptions::None => "no target".to_string(),
                    TargetOptions::Players { players } => {
                        for p in players {
                            let id = p.0.to_string();
                            if !player_targets.contains(&id) {
                                player_targets.push(id);
                            }
                        }
                        format!(
                            "requires spell_target_player, one of: {}",
                            players
                                .iter()
                                .map(|p| p.0.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    }
                    TargetOptions::Colors { colors } => {
                        for c in colors {
                            let name = color_name(*c);
                            if !color_targets.contains(&name) {
                                color_targets.push(name);
                            }
                        }
                        format!(
                            "requires spell_target_color, one of: {}",
                            colors
                                .iter()
                                .map(|c| color_name(*c))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    }
                };
                format!("spell {} = {:?} ({target_note})", s.spell.0, s.kind)
            })
            .collect();
        properties.insert(
            "spell".into(),
            json!({
                "type": "integer",
                "enum": spell_ids,
                "description": format!(
                    "Optional: cast at most one spell this wave (alongside your action, never instead of it). Your castable spells: {}.",
                    spell_lines.join("; ")
                )
            }),
        );
        if !player_targets.is_empty() {
            properties.insert(
                "spell_target_player".into(),
                json!({
                    "type": "string",
                    "enum": player_targets,
                    "description": "The player id to aim a player-targeted spell at (see the spell list for which spells need this)."
                }),
            );
        }
        if !color_targets.is_empty() {
            properties.insert(
                "spell_target_color".into(),
                json!({
                    "type": "string",
                    "enum": color_targets,
                    "description": "The colour to aim a colour-targeted spell at (see the spell list for which spells need this)."
                }),
            );
        }
    }

    ToolDefinition {
        name: TOOL_NAME.into(),
        description: "Commit your decision for this wave of Boiling Point. You must choose to play exactly one ingredient or pass; you may additionally cast at most one spell with a legal target.".into(),
        input_schema: Value::Object(serde_json::Map::from_iter([
            ("type".to_string(), json!("object")),
            ("properties".to_string(), Value::Object(properties)),
            ("required".to_string(), json!(["action"])),
        ])),
    }
}

/// Map a tool-call input back to an [`Answer`], validating every field against
/// the frame. Returns `None` for anything unparseable or outside the legal set
/// — the caller falls back and logs.
pub fn answer_from_tool_input(decision: &PendingDecision, input: &Value) -> Option<Answer> {
    let PendingDecision::WaveCommit { spells, .. } = decision;

    let action = match input.get("action").and_then(Value::as_str)? {
        "pass" => WaveAction::Pass,
        "play" => {
            let card = CardId(input.get("card").and_then(Value::as_u64)? as u32);
            let colorless = input
                .get("colorless")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            WaveAction::Play { card, colorless }
        }
        _ => return None,
    };

    let spell = match input.get("spell").and_then(Value::as_u64) {
        None => None,
        Some(raw) => {
            let spell_id = CardId(raw as u32);
            let castable = spells.iter().find(|s| s.spell == spell_id)?;
            let target = match castable.kind.target_kind() {
                TargetKind::None => None,
                TargetKind::Player => {
                    let raw = input.get("spell_target_player").and_then(Value::as_str)?;
                    let uuid = uuid::Uuid::parse_str(raw).ok()?;
                    Some(SpellTarget::Player {
                        player: PlayerId(uuid),
                    })
                }
                TargetKind::Color => {
                    let raw = input.get("spell_target_color").and_then(Value::as_str)?;
                    Some(SpellTarget::Color {
                        color: color_by_name(raw)?,
                    })
                }
            };
            Some(SpellCast {
                spell: spell_id,
                target,
            })
        }
    };

    let answer = Answer::WaveCommit { action, spell };
    answer.is_legal(decision).then_some(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::frame::{CastableSpell, PlayableIngredient};
    use boiling_point_protocol::vocab::{HandIngredient, IngredientView, SpellKind};
    use uuid::Uuid;

    fn pid(n: u128) -> PlayerId {
        PlayerId(Uuid::from_u128(n))
    }

    fn frame() -> PendingDecision {
        PendingDecision::WaveCommit {
            playable: vec![
                PlayableIngredient {
                    ingredient: HandIngredient {
                        id: CardId(11),
                        view: IngredientView {
                            color: Color::Ruby,
                            volatility: 3,
                            points: 2,
                        },
                    },
                    colorless_allowed: true,
                },
                PlayableIngredient {
                    ingredient: HandIngredient {
                        id: CardId(12),
                        view: IngredientView {
                            color: Color::Wild,
                            volatility: 1,
                            points: 0,
                        },
                    },
                    colorless_allowed: true,
                },
            ],
            can_pass: true,
            spells: vec![
                CastableSpell {
                    spell: CardId(20),
                    kind: SpellKind::Peek,
                    targets: TargetOptions::None,
                },
                CastableSpell {
                    spell: CardId(21),
                    kind: SpellKind::Hex,
                    targets: TargetOptions::Players {
                        players: vec![pid(2), pid(3)],
                    },
                },
                CastableSpell {
                    spell: CardId(22),
                    kind: SpellKind::Sour,
                    targets: TargetOptions::Colors {
                        colors: Color::PLAYER_COLORS.to_vec(),
                    },
                },
            ],
        }
    }

    /// The schema's enums are exactly the frame's enumerated values.
    #[test]
    fn schema_enums_mirror_the_frame() {
        let tool = tool_from_frame(&frame());
        let props = &tool.input_schema["properties"];
        assert_eq!(props["action"]["enum"], json!(["play", "pass"]));
        assert_eq!(props["card"]["enum"], json!([11, 12]));
        assert_eq!(props["spell"]["enum"], json!([20, 21, 22]));
        assert_eq!(
            props["spell_target_player"]["enum"],
            json!([pid(2).0.to_string(), pid(3).0.to_string()])
        );
        assert_eq!(
            props["spell_target_color"]["enum"],
            json!(["Ruby", "Sapphire", "Emerald", "Amethyst"])
        );
        assert_eq!(tool.input_schema["required"], json!(["action"]));
    }

    /// A frame with no castable spells presents no spell fields at all.
    #[test]
    fn spent_spell_slot_drops_the_spell_fields() {
        let PendingDecision::WaveCommit {
            playable, can_pass, ..
        } = frame();
        let bare = PendingDecision::WaveCommit {
            playable,
            can_pass,
            spells: vec![],
        };
        let tool = tool_from_frame(&bare);
        let props = tool.input_schema["properties"].as_object().unwrap();
        assert!(!props.contains_key("spell"));
        assert!(!props.contains_key("spell_target_player"));
        assert!(!props.contains_key("spell_target_color"));
    }

    /// Well-formed responses map to legal answers.
    #[test]
    fn valid_inputs_map_to_legal_answers() {
        let f = frame();
        let answer = answer_from_tool_input(
            &f,
            &json!({"action": "play", "card": 11, "colorless": true, "spell": 21, "spell_target_player": pid(2).0.to_string()}),
        )
        .expect("maps");
        assert!(answer.is_legal(&f));
        let pass = answer_from_tool_input(&f, &json!({"action": "pass"})).expect("maps");
        assert_eq!(pass, Answer::pass());
    }

    /// Malformed or out-of-frame responses map to None (fallback + log).
    #[test]
    fn malformed_or_illegal_inputs_are_rejected() {
        let f = frame();
        // Unknown action.
        assert!(answer_from_tool_input(&f, &json!({"action": "explode"})).is_none());
        // Play without a card.
        assert!(answer_from_tool_input(&f, &json!({"action": "play"})).is_none());
        // A card outside the frame.
        assert!(answer_from_tool_input(&f, &json!({"action": "play", "card": 999})).is_none());
        // A spell outside the frame.
        assert!(answer_from_tool_input(&f, &json!({"action": "pass", "spell": 999})).is_none());
        // A player-targeted spell with a colour target.
        assert!(
            answer_from_tool_input(
                &f,
                &json!({"action": "pass", "spell": 21, "spell_target_color": "Ruby"})
            )
            .is_none()
        );
        // A player target outside the enumerated set.
        assert!(
            answer_from_tool_input(
                &f,
                &json!({"action": "pass", "spell": 21, "spell_target_player": pid(9).0.to_string()})
            )
            .is_none()
        );
        // Not even JSON-shaped right.
        assert!(answer_from_tool_input(&f, &json!("pass")).is_none());
    }
}
