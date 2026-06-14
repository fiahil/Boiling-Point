//! The study configuration (task 3.2): the question-driven inputs to one study —
//! a seed set, a game count, knob values, and the matrix axes — parsed from TOML.
//!
//! The matrix axes (persona × Brewer × deck-archetype) and the seed set live in
//! the harness's own [`SampleSpec`] (reused, not re-modelled); the only thing a
//! *study* adds is a name, the question it answers, and which content config
//! (the "knob values") to run against.

use std::path::PathBuf;

use serde::Deserialize;

use boiling_point_ai_client::ClientError;
use boiling_point_ai_client::harness::SampleSpec;

/// One study's full configuration: the harness sample plus study metadata.
#[derive(Debug, Clone, Deserialize)]
pub struct StudyConfig {
    /// Short study name (appears in the report and its filename).
    pub name: String,
    /// The question this study answers (e.g. "does BP 31–43 hold ~45%?"). Prose,
    /// for the report header — every on-demand run is attributable to a hypothesis.
    #[serde(default)]
    pub question: Option<String>,
    /// Path to a content config TOML — the **knob values** under test (the
    /// boiling-point range, caps, magnitudes). Relative to the current directory.
    /// Absent ⇒ the server's embedded default content.
    #[serde(default)]
    pub content_config: Option<PathBuf>,
    /// The seeded matrix sample: root seed + the persona × Brewer × deck-archetype
    /// cells and their game counts. Reused verbatim from the harness.
    pub sample: SampleSpec,
}

impl StudyConfig {
    /// Parse and validate a TOML study config.
    pub fn from_toml(toml: &str) -> Result<Self, ClientError> {
        let config: StudyConfig =
            ::toml::from_str(toml).map_err(|e| ClientError::Config(format!("study: {e}")))?;
        config.sample.validate()?;
        Ok(config)
    }

    /// The default all-bot baseline study: the four archetypes head-to-head over
    /// the embedded content, for a quick on-demand sample with no config file.
    pub fn baseline(seed: u64, games: u64) -> Self {
        StudyConfig {
            name: "baseline".into(),
            question: Some("all-bot baseline over the embedded content".into()),
            content_config: None,
            sample: SampleSpec::baseline(seed, games),
        }
    }

    /// Total games across every cell — the study's scale, recorded in provenance.
    pub fn total_games(&self) -> u64 {
        self.sample.cells.iter().map(|c| c.games).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A study config parses: metadata plus the embedded harness sample.
    #[test]
    fn study_config_parses_with_an_embedded_sample() {
        let cfg = StudyConfig::from_toml(
            r#"
            name = "explosion-band"
            question = "does the BP 31-43 window hold ~45% explosions?"

            [sample]
            root_seed = 42

            [[sample.cells]]
            name = "field"
            games = 200
            seats = [
                { brain = "bot", archetype = "aggressive" },
                { brain = "bot", archetype = "cautious" },
                { brain = "bot", archetype = "political" },
                { brain = "bot", archetype = "random" },
            ]
            "#,
        )
        .expect("parses");
        assert_eq!(cfg.name, "explosion-band");
        assert_eq!(cfg.total_games(), 200);
        assert_eq!(cfg.sample.root_seed, 42);
        assert!(cfg.content_config.is_none());
    }

    /// The baseline study is a one-cell all-bot sample at the requested scale.
    #[test]
    fn baseline_study_is_all_bot() {
        let cfg = StudyConfig::baseline(7, 500);
        assert_eq!(cfg.total_games(), 500);
        assert!(!cfg.sample.has_agent_seats());
    }
}
