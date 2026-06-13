//! The host decision policy (design D5): per decision kind, **Scripted** (the
//! host supplies the answer) or **Delegated** (the brain decides).
//!
//! "What is controlled" is a host property, never a brain property: the
//! harness scripts the experimental variables (a scripted wave answer; the
//! deck-archetype draft rides the bot brain's *plan* rather than a script,
//! like the Brewer preference) while the seat-filler delegates everything —
//! without either brain knowing the difference. A scripted answer still
//! passes the same legality gate as a brain's; an illegal script falls back
//! exactly like an illegal brain answer.

use boiling_point_protocol::frame::PendingDecision;

use crate::brain::Answer;

/// Who answers a given decision kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Policy {
    /// The host supplies this fixed answer; the brain never sees the frame.
    Scripted(Answer),
    /// The brain decides.
    Delegated,
}

/// The host's per-decision-kind policy table. Grows a field per decision kind
/// as the v2 surface lands — each defaulting to `Delegated`, the seat-filler
/// posture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPolicy {
    /// Who answers wave-commit frames.
    pub wave_commit: Policy,
    /// Who answers the pre-game brewer pick (`boom2-brewers`). The harness's
    /// brewer axis rides the bot brain's *preference* rather than a script
    /// (the dealt pair is random, so a fixed scripted pick would mostly be
    /// illegal); this stays `Delegated` unless an experiment pins it.
    pub brewer_pick: Policy,
    /// Who answers the pre-game Apothecary draft (`boom2-apothecary`). The
    /// harness's deck-archetype axis rides the bot brain's *plan* (the bot
    /// legalizes the named recipe against its own frame — the Cinderwright
    /// loses Ironbark, for instance — where a fixed script would go illegal);
    /// this stays `Delegated` unless an experiment pins it.
    pub apothecary_draft: Policy,
}

impl Default for HostPolicy {
    fn default() -> Self {
        HostPolicy {
            wave_commit: Policy::Delegated,
            brewer_pick: Policy::Delegated,
            apothecary_draft: Policy::Delegated,
        }
    }
}

impl HostPolicy {
    /// The policy routing for `decision`'s kind.
    pub fn route(&self, decision: &PendingDecision) -> &Policy {
        match decision {
            PendingDecision::WaveCommit { .. } => &self.wave_commit,
            PendingDecision::BrewerPick { .. } => &self.brewer_pick,
            PendingDecision::ApothecaryDraft { .. } => &self.apothecary_draft,
        }
    }
}
