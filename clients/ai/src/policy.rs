//! The host decision policy (design D5): per decision kind, **Scripted** (the
//! host supplies the answer) or **Delegated** (the brain decides).
//!
//! "What is controlled" is a host property, never a brain property: the
//! harness scripts the experimental variables (today a scripted wave answer;
//! the Apothecary draft and Brewer pick when those decision kinds land) while
//! the seat-filler delegates everything — without either brain knowing the
//! difference. A scripted answer still passes the same legality gate as a
//! brain's; an illegal script falls back exactly like an illegal brain answer.

use boiling_point_protocol::frame::PendingDecision;

use crate::brain::Answer;

/// Who answers a given decision kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// The host supplies this fixed answer; the brain never sees the frame.
    Scripted(Answer),
    /// The brain decides.
    Delegated,
}

/// The host's per-decision-kind policy table. Grows a field per decision kind
/// as the v2 surface lands (Brewer pick, Apothecary draft) — each defaulting
/// to `Delegated`, the seat-filler posture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostPolicy {
    /// Who answers wave-commit frames.
    pub wave_commit: Policy,
}

impl Default for HostPolicy {
    fn default() -> Self {
        HostPolicy {
            wave_commit: Policy::Delegated,
        }
    }
}

impl HostPolicy {
    /// The policy routing for `decision`'s kind.
    pub fn route(&self, decision: &PendingDecision) -> Policy {
        match decision {
            PendingDecision::WaveCommit { .. } => self.wave_commit,
        }
    }
}
