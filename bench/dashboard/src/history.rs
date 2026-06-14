//! The bench history schema (D5): one JSON record per `main` merge — the commit,
//! a timestamp, and that run's per-bench estimates — appended to the orphan
//! `bench-data` branch. The dashboard reads the full history and draws each
//! bench as a trend (the noise-killer: a regression is a sustained level shift
//! across records, never one run's delta — D2).

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::criterion::{BenchEstimate, collect};

/// The history record schema version (keys the dashboard's reader).
pub const SCHEMA_VERSION: u32 = 1;

/// One commit's bench measurements — the unit appended to `bench-data` per merge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryRecord {
    /// The record schema version.
    pub schema_version: u32,
    /// The commit these benches ran against (short SHA, or `unknown`).
    pub commit: String,
    /// When the run happened (Unix seconds) — the trend's x-axis.
    pub timestamp_unix: u64,
    /// Per-bench estimates with confidence bounds, sorted by id.
    pub benches: Vec<BenchEstimate>,
}

impl HistoryRecord {
    /// Build a record from a criterion directory, stamping the commit and now.
    pub fn build(criterion_dir: &Path, commit: String) -> Result<Self, String> {
        Ok(HistoryRecord {
            schema_version: SCHEMA_VERSION,
            commit,
            timestamp_unix: now_unix(),
            benches: collect(criterion_dir)?,
        })
    }

    /// Pretty JSON — the artifact appended to the history branch.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    }
}

/// Load every `*.json` history record in a directory, sorted oldest-first by
/// timestamp (then commit) so trends read left-to-right. Missing dir ⇒ empty.
pub fn load_dir(dir: &Path) -> Result<Vec<HistoryRecord>, String> {
    let mut records = Vec::new();
    if !dir.is_dir() {
        return Ok(records);
    }
    for entry in fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let json = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let record: HistoryRecord =
                serde_json::from_str(&json).map_err(|e| format!("{}: {e}", path.display()))?;
            records.push(record);
        }
    }
    records.sort_by(|a, b| {
        a.timestamp_unix
            .cmp(&b.timestamp_unix)
            .then_with(|| a.commit.cmp(&b.commit))
    });
    Ok(records)
}

/// Wall-clock now in Unix seconds.
fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records round-trip through JSON and load sorted by timestamp.
    #[test]
    fn history_records_round_trip_and_sort() {
        let dir = std::env::temp_dir().join(format!("bp-bench-history-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let mk = |commit: &str, ts: u64| HistoryRecord {
            schema_version: SCHEMA_VERSION,
            commit: commit.into(),
            timestamp_unix: ts,
            benches: vec![BenchEstimate {
                id: "engine/wave_resolution".into(),
                point_ns: 100.0,
                lower_ns: 95.0,
                upper_ns: 105.0,
            }],
        };
        fs::write(dir.join("b.json"), mk("bbb", 200).to_json()).unwrap();
        fs::write(dir.join("a.json"), mk("aaa", 100).to_json()).unwrap();
        let loaded = load_dir(&dir).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].commit, "aaa", "oldest first");
        assert_eq!(loaded[1].commit, "bbb");
        fs::remove_dir_all(&dir).ok();
    }

    /// A missing history directory is empty, not an error (first-ever run).
    #[test]
    fn missing_history_dir_is_empty() {
        let loaded = load_dir(Path::new("/no/such/bench/history")).unwrap();
        assert!(loaded.is_empty());
    }
}
