//! Harness mode — the Principle IV reinstatement (capability
//! `boom-balance-harness`): seeded batch runs of complete v2 games over the
//! server's headless in-process seam, the matrix sample spec, and diffable
//! balance reports.
//!
//! This module (and the `balance_tester` binary) is the one sanctioned place the
//! server crate appears in the AI client's dependency graph (the opt-in
//! `harness` feature): the batch runner boots in-process games, but every
//! seat still talks to them exclusively through encoded wire frames (D2).
//! Batches default to bot brains — an all-bot run makes zero Claude calls by
//! construction; agent seats require the explicit flag and void the
//! reproducibility guarantee.

pub mod rating_sim;
pub mod report;
pub mod runner;
pub mod spec;
pub mod stats;

pub use rating_sim::{Matching, SimConfig, SimReport, run_simulation, run_with_params};
pub use report::{Report, Thresholds, fingerprint};
pub use runner::{OutcomeSummary, RunOptions, SampleRun, TransportKind, run_sample};
pub use spec::{BrainSpec, CellSpec, SampleSpec, SeatSpec};
pub use stats::CellStats;
