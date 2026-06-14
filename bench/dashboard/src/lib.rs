//! Bench dashboard (change `boom2-benchmarking`, capability `bench-dashboard`).
//!
//! Two jobs, both plain data → file transforms (Principles II/III):
//!
//! - [`criterion`] + [`history`] — `collect`: read a criterion run's per-bench
//!   `estimates.json` and emit **one** [`history::HistoryRecord`] (commit,
//!   timestamp, per-bench point estimate + confidence bounds). The per-merge CI
//!   job appends each record to the orphan `bench-data` branch (D5).
//! - [`render`] — read the full history + any balance-study reports and emit a
//!   **single self-contained** `benches.html` (inline data, inline styles and
//!   scripts, hand-rolled SVG) that renders fully from disk with zero external
//!   requests (D4).
//!
//! The dashboard only ever *shows* numbers — it never gates (D1). It reads study
//! reports as generic JSON (the versioned report shape is the contract) so it
//! stays decoupled from the study/harness/server crates.

pub mod criterion;
pub mod history;
pub mod render;

pub use history::HistoryRecord;
