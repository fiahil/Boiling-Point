//! The matrix sample specification (task 6.1): which persona × Brewer ×
//! deck-archetype cells to run and how many games per cell — never only full
//! factorial runs.
//!
//! All three axes are live. The persona axis is the bot archetype + epsilon
//! (or an agent persona). A seat's `brewer` is a pick *preference*: the server
//! deals random disjoint pairs, so the seat picks the named Brewer whenever it
//! is offered (else the first option), and the persona × Brewer matrix
//! aggregates by the **actual** picks — every cell contributes signal
//! regardless of the deal. A seat's `deck_archetype` (`boom2-apothecary`)
//! names a planned recipe ([`crate::bot::DeckArchetype`]) the bot legalizes
//! against its own draft frame — the matrix keys on the configured plan.

use serde::Deserialize;

use boiling_point_protocol::vocab::{Brewer, Recipe};

use crate::ClientError;
use crate::bot::{Archetype, DeckArchetype};

/// One seat's experimental assignment within a cell.
#[derive(Debug, Clone, Deserialize)]
pub struct SeatSpec {
    /// The brain for this seat.
    #[serde(flatten)]
    pub brain: BrainSpec,
    /// Brewer pick preference (bot seats only): taken whenever the dealt pair
    /// offers it. Must name one of the 12 Brewers.
    #[serde(default)]
    pub brewer: Option<String>,
    /// Deck-archetype plan (bot seats only): the named recipe this seat drafts
    /// (legalized against its frame). Must name one of the
    /// [`DeckArchetype`] presets.
    #[serde(default)]
    pub deck_archetype: Option<String>,
}

impl SeatSpec {
    /// The parsed Brewer preference (validated by [`SampleSpec::validate`]).
    pub fn brewer_preference(&self) -> Option<Brewer> {
        self.brewer.as_deref().and_then(Brewer::by_name)
    }

    /// The parsed deck plan (validated by [`SampleSpec::validate`]).
    pub fn deck_plan(&self) -> Option<Recipe> {
        self.deck_archetype
            .as_deref()
            .and_then(DeckArchetype::by_name)
            .map(|a| a.recipe())
    }
}

/// Which brain a seat runs.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "brain", rename_all = "snake_case")]
pub enum BrainSpec {
    /// The deterministic bot brain.
    Bot {
        /// Archetype name (`cautious`, `aggressive`, `political`, `random`).
        archetype: String,
        /// Blunder epsilon (0..=1).
        #[serde(default)]
        epsilon: f64,
    },
    /// The Claude-driven agent brain (opt-in: requires the explicit
    /// `--allow-agents` flag; voids the reproducibility guarantee).
    Agent {
        /// Model id (defaults to the agent brain's default).
        #[serde(default)]
        model: Option<String>,
        /// Persona prose.
        #[serde(default)]
        persona: Option<String>,
    },
}

impl BrainSpec {
    /// The label outcomes are attributed to in reports.
    pub fn label(&self) -> String {
        match self {
            BrainSpec::Bot { archetype, epsilon } if *epsilon > 0.0 => {
                format!("{archetype}(ε={epsilon})")
            }
            BrainSpec::Bot { archetype, .. } => archetype.clone(),
            BrainSpec::Agent { model, .. } => {
                format!("agent:{}", model.as_deref().unwrap_or("default"))
            }
        }
    }

    /// Whether this is an agent (Claude) seat.
    pub fn is_agent(&self) -> bool {
        matches!(self, BrainSpec::Agent { .. })
    }
}

/// One matrix cell: a named seat configuration run for `games` games.
#[derive(Debug, Clone, Deserialize)]
pub struct CellSpec {
    /// Report label for the cell.
    pub name: String,
    /// Complete games to play in this cell.
    pub games: u64,
    /// The four seats, in seating (colour) order.
    pub seats: Vec<SeatSpec>,
}

/// A whole sample: the root seed plus the cells to run.
#[derive(Debug, Clone, Deserialize)]
pub struct SampleSpec {
    /// Root seed for the run's RNG tree (root → cell → game → seat).
    pub root_seed: u64,
    /// The cells to run, in order.
    pub cells: Vec<CellSpec>,
}

impl SampleSpec {
    /// Parse and validate a TOML sample spec.
    pub fn from_toml(toml: &str) -> Result<Self, ClientError> {
        let spec: SampleSpec =
            ::toml::from_str(toml).map_err(|e| ClientError::Config(format!("spec: {e}")))?;
        spec.validate()?;
        Ok(spec)
    }

    /// The default all-bot baseline cell: the four archetypes head-to-head.
    pub fn baseline(root_seed: u64, games: u64) -> Self {
        SampleSpec {
            root_seed,
            cells: vec![CellSpec {
                name: "baseline".into(),
                games,
                seats: Archetype::ALL
                    .into_iter()
                    .map(|a| SeatSpec {
                        brain: BrainSpec::Bot {
                            archetype: a.name().into(),
                            epsilon: 0.0,
                        },
                        brewer: None,
                        deck_archetype: None,
                    })
                    .collect(),
            }],
        }
    }

    /// Whether any seat anywhere runs an agent brain.
    pub fn has_agent_seats(&self) -> bool {
        self.cells
            .iter()
            .flat_map(|c| &c.seats)
            .any(|s| s.brain.is_agent())
    }

    /// Validate cell shapes, archetype names, and the not-yet-landed axes.
    pub fn validate(&self) -> Result<(), ClientError> {
        if self.cells.is_empty() {
            return Err(ClientError::Config("spec has no cells".into()));
        }
        for cell in &self.cells {
            if cell.seats.len() != 4 {
                return Err(ClientError::Config(format!(
                    "cell '{}' needs exactly 4 seats, has {}",
                    cell.name,
                    cell.seats.len()
                )));
            }
            if cell.games == 0 {
                return Err(ClientError::Config(format!(
                    "cell '{}' runs zero games",
                    cell.name
                )));
            }
            for seat in &cell.seats {
                if let BrainSpec::Bot { archetype, epsilon } = &seat.brain {
                    if Archetype::by_name(archetype).is_none() {
                        return Err(ClientError::Config(format!(
                            "cell '{}': unknown archetype '{archetype}'",
                            cell.name
                        )));
                    }
                    if !(0.0..=1.0).contains(epsilon) {
                        return Err(ClientError::Config(format!(
                            "cell '{}': epsilon {epsilon} outside 0..=1",
                            cell.name
                        )));
                    }
                }
                if let Some(name) = &seat.brewer {
                    if Brewer::by_name(name).is_none() {
                        return Err(ClientError::Config(format!(
                            "cell '{}': unknown brewer '{name}' (one of: {})",
                            cell.name,
                            Brewer::ALL
                                .iter()
                                .map(|b| b.name())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )));
                    }
                    if seat.brain.is_agent() {
                        return Err(ClientError::Config(format!(
                            "cell '{}': a brewer preference requires a bot brain (the agent picks for itself)",
                            cell.name
                        )));
                    }
                }
                if let Some(name) = &seat.deck_archetype {
                    if DeckArchetype::by_name(name).is_none() {
                        return Err(ClientError::Config(format!(
                            "cell '{}': unknown deck archetype '{name}' (one of: {})",
                            cell.name,
                            DeckArchetype::ALL
                                .iter()
                                .map(|a| a.name())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )));
                    }
                    if seat.brain.is_agent() {
                        return Err(ClientError::Config(format!(
                            "cell '{}': a deck archetype requires a bot brain (the agent drafts for itself)",
                            cell.name
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A targeted cell spec parses with labels intact.
    #[test]
    fn specs_parse_and_label() {
        let spec = SampleSpec::from_toml(
            r#"
            root_seed = 7

            [[cells]]
            name = "aggressive-vs-field"
            games = 500
            seats = [
                { brain = "bot", archetype = "aggressive" },
                { brain = "bot", archetype = "cautious", epsilon = 0.1 },
                { brain = "bot", archetype = "political" },
                { brain = "bot", archetype = "random" },
            ]
            "#,
        )
        .expect("parses");
        assert_eq!(spec.cells[0].games, 500);
        assert_eq!(spec.cells[0].seats[0].brain.label(), "aggressive");
        assert_eq!(spec.cells[0].seats[1].brain.label(), "cautious(ε=0.1)");
        assert!(!spec.has_agent_seats());
    }

    /// The Brewer axis is live: a valid preference parses; an unknown name or
    /// an agent-seat preference fails loudly.
    #[test]
    fn brewer_axis_is_live_and_validated() {
        let spec = SampleSpec::from_toml(
            r#"
            root_seed = 7
            [[cells]]
            name = "x"
            games = 1
            seats = [
                { brain = "bot", archetype = "cautious", brewer = "Cinderwright" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "cautious" },
            ]
            "#,
        )
        .expect("a live brewer axis parses");
        assert_eq!(
            spec.cells[0].seats[0].brewer_preference(),
            Some(Brewer::Cinderwright)
        );

        let unknown = SampleSpec::from_toml(
            r#"
            root_seed = 7
            [[cells]]
            name = "x"
            games = 1
            seats = [
                { brain = "bot", archetype = "cautious", brewer = "Bartender" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "cautious" },
            ]
            "#,
        )
        .unwrap_err();
        assert!(unknown.to_string().contains("unknown brewer"), "{unknown}");

        let agent_pref = SampleSpec::from_toml(
            r#"
            root_seed = 7
            [[cells]]
            name = "x"
            games = 1
            seats = [
                { brain = "agent", brewer = "Lurker" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "cautious" },
            ]
            "#,
        )
        .unwrap_err();
        assert!(agent_pref.to_string().contains("bot brain"), "{agent_pref}");
    }

    /// The deck-archetype axis is live (`boom2-apothecary`): a preset name
    /// parses to its planned recipe; an unknown name or an agent-seat plan
    /// fails loudly.
    #[test]
    fn deck_archetype_axis_is_live_and_validated() {
        let spec = SampleSpec::from_toml(
            r#"
            root_seed = 7
            [[cells]]
            name = "x"
            games = 1
            seats = [
                { brain = "bot", archetype = "cautious", deck_archetype = "warlord" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "cautious" },
            ]
            "#,
        )
        .expect("a live deck-archetype axis parses");
        assert_eq!(
            spec.cells[0].seats[0].deck_plan(),
            Some(DeckArchetype::Warlord.recipe())
        );
        assert_eq!(spec.cells[0].seats[1].deck_plan(), None);

        let unknown = SampleSpec::from_toml(
            r#"
            root_seed = 7
            [[cells]]
            name = "x"
            games = 1
            seats = [
                { brain = "bot", archetype = "cautious", deck_archetype = "rush" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "cautious" },
            ]
            "#,
        )
        .unwrap_err();
        assert!(
            unknown.to_string().contains("unknown deck archetype"),
            "{unknown}"
        );

        let agent_plan = SampleSpec::from_toml(
            r#"
            root_seed = 7
            [[cells]]
            name = "x"
            games = 1
            seats = [
                { brain = "agent", deck_archetype = "fortress" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "cautious" },
            ]
            "#,
        )
        .unwrap_err();
        assert!(agent_plan.to_string().contains("bot brain"), "{agent_plan}");
    }

    /// Bad shapes are rejected: seat counts, unknown archetypes, agent flags.
    #[test]
    fn malformed_specs_are_rejected() {
        assert!(SampleSpec::from_toml("root_seed = 1\ncells = []").is_err());
        let bad_archetype = r#"
            root_seed = 1
            [[cells]]
            name = "x"
            games = 1
            seats = [
                { brain = "bot", archetype = "bold" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "cautious" },
            ]
        "#;
        assert!(SampleSpec::from_toml(bad_archetype).is_err());
    }

    /// Agent seats parse (gating to the explicit flag happens in the runner).
    #[test]
    fn agent_seats_are_detected() {
        let spec = SampleSpec::from_toml(
            r#"
            root_seed = 7
            [[cells]]
            name = "agent-vs-bots"
            games = 2
            seats = [
                { brain = "agent", model = "claude-haiku-4-5" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "political" },
                { brain = "bot", archetype = "random" },
            ]
            "#,
        )
        .expect("parses");
        assert!(spec.has_agent_seats());
        assert_eq!(
            spec.cells[0].seats[0].brain.label(),
            "agent:claude-haiku-4-5"
        );
    }
}
