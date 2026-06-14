//! Balance study (change `boom2-benchmarking`, capability `balance-study`).
//!
//! A **thin study wrapper** over the AI client's harness mode
//! ([`boiling_point_ai_client::harness`], change `boom2-ai-client`): study
//! configs in, versioned reproducible reports out. The harness already owns the
//! transports, the seeded batch runner, the per-cell statistics, and the
//! markdown/JSON report — this crate does **not** duplicate any of it. It adds
//! exactly the two things a *study* needs on top:
//!
//! 1. **Provenance** ([`report::Provenance`]) — seed set, game count, content
//!    config hash, and engine commit, so a report is attributable to a hypothesis
//!    and reproducible from its own header (spec: versioned & reproducible).
//! 2. **The shared §IV fold** ([`metrics`]) — the same
//!    [`boiling_point_server::observability::balance_metrics`] definitions the
//!    live pipeline evaluates, folded over the harness's bot games (design D3:
//!    one definition, two populations — "does live play match the harness?" is a
//!    direct comparison, never a reconciliation). The study never re-derives a
//!    formula.
//!
//! **Observational, never gating (D1):** off-target metrics are *flagged* for a
//! human ([`report::Flag`], Principle IV); nothing here blocks CI or a deploy.

pub mod config;
pub mod metrics;
pub mod report;
pub mod runner;

pub use config::StudyConfig;
pub use report::StudyReport;
pub use runner::run_study;
