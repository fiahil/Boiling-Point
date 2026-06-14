//! Reading criterion output: walk a `target/criterion` tree for each bench's
//! `new/estimates.json` and extract the mean point estimate with its confidence
//! bounds — the per-bench figures one history record carries.
//!
//! Criterion writes `<criterion_dir>/<group>/<bench>/new/estimates.json` (and a
//! `change/` sibling on reruns, plus `report/` HTML we ignore). The bench id is
//! the path between `<criterion_dir>` and `/new/`, e.g. `engine/wave_resolution`.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// One bench's measured cost: the mean estimate and its confidence interval, in
/// nanoseconds. The band is what makes the 6–12% rerun noise visible against a
/// real shift (D2) — a single point is never read alone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchEstimate {
    /// The bench id (criterion group/bench path), e.g. `engine/wave_resolution`.
    pub id: String,
    /// Mean point estimate (nanoseconds).
    pub point_ns: f64,
    /// Lower confidence bound (nanoseconds).
    pub lower_ns: f64,
    /// Upper confidence bound (nanoseconds).
    pub upper_ns: f64,
}

/// The slice of criterion's `estimates.json` we read: the mean and its interval.
#[derive(Debug, Deserialize)]
struct Estimates {
    mean: Estimate,
}

/// One criterion estimate: a point value and its confidence interval.
#[derive(Debug, Deserialize)]
struct Estimate {
    point_estimate: f64,
    confidence_interval: ConfidenceInterval,
}

/// A criterion confidence interval (nanoseconds).
#[derive(Debug, Deserialize)]
struct ConfidenceInterval {
    lower_bound: f64,
    upper_bound: f64,
}

/// Parse one `estimates.json`'s mean estimate into `(point, lower, upper)`.
pub fn parse_estimates(json: &str) -> Result<(f64, f64, f64), String> {
    let e: Estimates = serde_json::from_str(json).map_err(|e| format!("estimates.json: {e}"))?;
    Ok((
        e.mean.point_estimate,
        e.mean.confidence_interval.lower_bound,
        e.mean.confidence_interval.upper_bound,
    ))
}

/// Collect every bench estimate under a criterion directory, sorted by id.
pub fn collect(criterion_dir: &Path) -> Result<Vec<BenchEstimate>, String> {
    let mut out = Vec::new();
    if criterion_dir.is_dir() {
        walk(criterion_dir, criterion_dir, &mut out)?;
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// Recurse `dir`, picking up each `new/estimates.json` and deriving its id
/// relative to `root` (the criterion directory).
fn walk(root: &Path, dir: &Path, out: &mut Vec<BenchEstimate>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else if path.file_name().and_then(|n| n.to_str()) == Some("estimates.json")
            && path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                == Some("new")
            && let Some(id) = bench_id(root, &path)
        {
            let json = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let (point_ns, lower_ns, upper_ns) = parse_estimates(&json)?;
            out.push(BenchEstimate {
                id,
                point_ns,
                lower_ns,
                upper_ns,
            });
        }
    }
    Ok(())
}

/// The bench id for an `estimates.json` path: the components between `root` and
/// the trailing `new/estimates.json`, joined with `/`.
fn bench_id(root: &Path, estimates: &Path) -> Option<String> {
    // .../<id...>/new/estimates.json → .../<id...>
    let bench_dir = estimates.parent()?.parent()?;
    let rel = bench_dir.strip_prefix(root).ok()?;
    let id = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    (!id.is_empty()).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mean estimate and its bounds are read out of a criterion fixture.
    #[test]
    fn parses_a_criterion_estimates_blob() {
        let json = r#"{
            "mean": {
                "confidence_interval": { "confidence_level": 0.95, "lower_bound": 90.0, "upper_bound": 110.0 },
                "point_estimate": 100.0,
                "standard_error": 5.0
            },
            "median": { "confidence_interval": { "confidence_level": 0.95, "lower_bound": 1, "upper_bound": 2 }, "point_estimate": 1.5, "standard_error": 0.1 }
        }"#;
        assert_eq!(parse_estimates(json).unwrap(), (100.0, 90.0, 110.0));
    }

    /// The bench id is the path between the criterion dir and `/new/`.
    #[test]
    fn derives_the_bench_id_from_the_path() {
        let root = Path::new("/t/target/criterion");
        let est = Path::new("/t/target/criterion/engine/wave_resolution/new/estimates.json");
        assert_eq!(
            bench_id(root, est).as_deref(),
            Some("engine/wave_resolution")
        );
    }

    /// A real on-disk tree is walked end to end.
    #[test]
    fn collects_estimates_from_a_tree() {
        let dir = std::env::temp_dir().join(format!("bp-bench-collect-{}", std::process::id()));
        let bench = dir.join("engine/modifier_stacking/new");
        fs::create_dir_all(&bench).unwrap();
        fs::write(
            bench.join("estimates.json"),
            r#"{"mean":{"confidence_interval":{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":3.0},"point_estimate":2.0,"standard_error":0.5}}"#,
        )
        .unwrap();
        let found = collect(&dir).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "engine/modifier_stacking");
        assert_eq!(found[0].point_ns, 2.0);
        fs::remove_dir_all(&dir).ok();
    }
}
