//! The versioned study report schema (task 3.3): provenance + the shared §IV
//! metrics + flagged outliers, wrapping the full harness report.
//!
//! The report is **self-contained JSON** the dashboard reads directly: its own
//! provenance and §IV metrics on top, and the complete harness
//! [`Report`](boiling_point_ai_client::harness::Report) (per-cell statistics and
//! the persona × Brewer × deck-archetype matrices) embedded underneath. Reports
//! are versioned by [`SCHEMA_VERSION`]; a reader keys on it.

use serde::Serialize;

use boiling_point_ai_client::harness::Report;
use boiling_point_server::observability::balance_metrics::MetricValue;

use crate::config::StudyConfig;
use crate::metrics::{accumulate, study_metrics};

/// The study-report schema version. Bumped when the JSON shape changes so the
/// dashboard can read old records.
pub const SCHEMA_VERSION: u32 = 1;

/// What this study was and why it ran — the hypothesis the report answers.
#[derive(Debug, Clone, Serialize)]
pub struct StudyMeta {
    /// Short study name (also the report's filename stem).
    pub name: String,
    /// The question the study answers, if the config gave one.
    pub question: Option<String>,
}

/// Everything needed to attribute and reproduce a report (spec: versioned &
/// reproducible). Same provenance ⇒ same metrics — so a reader can re-run it.
#[derive(Debug, Clone, Serialize)]
pub struct Provenance {
    /// Root seed of the run's RNG tree.
    pub root_seed: u64,
    /// Total games across every cell — the study's scale.
    pub total_games: u64,
    /// FNV-1a fingerprint of the content config under test (the knob values).
    pub config_fingerprint: String,
    /// The engine commit the study ran against (`git rev-parse`, or `unknown`).
    pub engine_commit: String,
    /// The transport the batch ran over (`in-process` for a reproducible study).
    pub transport: String,
    /// False when an agent seat took part or the real wire was used — the same
    /// seeds will NOT reproduce the metrics.
    pub reproducible: bool,
    /// When the report was generated (Unix seconds) — display only, not compared.
    pub generated_unix: u64,
    /// Per-cell scale, in spec order.
    pub cells: Vec<CellProvenance>,
}

/// One cell's name and game count, recorded so the matrix is reproducible.
#[derive(Debug, Clone, Serialize)]
pub struct CellProvenance {
    /// The cell's name from the spec.
    pub name: String,
    /// Games played in this cell.
    pub games: u64,
}

/// One flagged finding lifted for the dashboard: an off-target rate or a
/// degenerate matrix cell, for a human to investigate (Principle IV). Purely
/// observational — a flag never fails anything.
#[derive(Debug, Clone, Serialize)]
pub struct Flag {
    /// The cell the finding was found in.
    pub cell: String,
    /// A short machine-stable category (mirrors the harness smell kinds).
    pub kind: String,
    /// A human-readable explanation including the numbers.
    pub detail: String,
}

/// A complete, versioned study report.
#[derive(Debug, Serialize)]
pub struct StudyReport {
    /// The schema version of this JSON shape.
    pub schema_version: u32,
    /// What ran and why.
    pub study: StudyMeta,
    /// How to attribute and reproduce it.
    pub provenance: Provenance,
    /// The shared §IV metric values (canonical definitions + targets).
    pub metrics: Vec<MetricValue>,
    /// Off-target rates and degenerate matrix cells, flagged for a human.
    pub flags: Vec<Flag>,
    /// The full harness report (per-cell stats + the persona × Brewer ×
    /// deck-archetype matrices) the metrics and flags were derived from.
    pub harness: Report,
}

impl StudyReport {
    /// Assemble a study report from a completed harness run and report.
    ///
    /// `harness` is the harness's own report (it already folded the run into
    /// per-cell statistics and detected the matrix smells); `config` carries the
    /// study metadata; `engine_commit` and `generated_unix` are the provenance the
    /// harness doesn't track. The §IV metrics are folded here from the same run.
    pub fn build(
        config: &StudyConfig,
        run: &boiling_point_ai_client::harness::SampleRun,
        harness: Report,
        engine_commit: String,
        generated_unix: u64,
    ) -> Self {
        let metrics = study_metrics(&accumulate(run));
        let flags = harness
            .smells
            .iter()
            .map(|s| Flag {
                cell: s.cell.clone(),
                kind: s.kind.clone(),
                detail: s.detail.clone(),
            })
            .collect();
        let provenance = Provenance {
            root_seed: harness.root_seed,
            total_games: config.total_games(),
            config_fingerprint: harness.config_fingerprint.clone(),
            engine_commit,
            transport: harness.transport.clone(),
            reproducible: harness.reproducible,
            generated_unix,
            cells: config
                .sample
                .cells
                .iter()
                .map(|c| CellProvenance {
                    name: c.name.clone(),
                    games: c.games,
                })
                .collect(),
        };
        StudyReport {
            schema_version: SCHEMA_VERSION,
            study: StudyMeta {
                name: config.name.clone(),
                question: config.question.clone(),
            },
            provenance,
            metrics,
            flags,
            harness,
        }
    }

    /// Serialise to pretty JSON — the versioned artifact the dashboard reads.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    }

    /// Render a human-readable markdown summary: the study header, provenance, the
    /// §IV metrics against their targets, the flags, then the full harness report.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Balance study — `{}`\n\n", self.study.name));
        if let Some(q) = &self.study.question {
            out.push_str(&format!("> {q}\n\n"));
        }
        let p = &self.provenance;
        out.push_str("## Provenance\n\n");
        out.push_str(&format!(
            "- Root seed: `{}` — {} games over {} cell(s)\n- Config fingerprint: `{}`\n- Engine commit: `{}`\n- Transport: {} — reproducible: {}\n",
            p.root_seed,
            p.total_games,
            p.cells.len(),
            p.config_fingerprint,
            p.engine_commit,
            p.transport,
            if p.reproducible { "yes" } else { "**NO**" },
        ));

        out.push_str("\n## §IV metrics (shared definitions)\n\n");
        out.push_str("| metric | value | target |\n|---|---|---|\n");
        for m in &self.metrics {
            let value = match m.value {
                Some(v) => format!("{v:.3}"),
                None => "—".into(),
            };
            let target = match &m.target {
                Some(t) => format!("{t:?}"),
                None => "—".into(),
            };
            out.push_str(&format!("| {} | {} | {} |\n", m.label, value, target));
        }

        out.push_str("\n## Flags\n\n");
        if self.flags.is_empty() {
            out.push_str("- None — every cell within threshold.\n");
        } else {
            for f in &self.flags {
                out.push_str(&format!(
                    "- ⚠️ `{}` **{}** — {}\n",
                    f.cell, f.kind, f.detail
                ));
            }
        }

        out.push_str("\n---\n\n");
        out.push_str(&self.harness.to_markdown());
        out
    }
}
