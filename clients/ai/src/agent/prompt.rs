//! Prompt assembly for the agent brain (task 5.2): persona + difficulty
//! framing, the running transcript of events the seat legitimately observed,
//! and a compact summary of the current decision.
//!
//! Everything here is built **exclusively** from the structurally secret-free
//! [`SeatView`] (its fields and its transcript) and the decision frame, so the
//! prompt cannot know more than the player does. Transcript growth is bounded:
//! only the most recent `transcript_limit` lines are included (drop-oldest
//! compaction), and [`PromptStats`] reports the sizes so growth stays measured.

use boiling_point_protocol::frame::PendingDecision;

use crate::view::{FrameContext, SeatView};

/// How hard the persona tries — prompt framing only; it can never alter the
/// legal action set, the view, or the timeliness contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    /// Casual table presence: plays naturally, not ruthlessly.
    Relaxed,
    /// Solid, sensible play.
    Standard,
    /// Plays to win.
    Sharp,
}

impl Difficulty {
    /// The framing line for the system prompt.
    fn framing(self) -> &'static str {
        match self {
            Difficulty::Relaxed => {
                "Play casually and in character. You enjoy the game more than you need to win; the occasional sentimental or showy play is in character."
            }
            Difficulty::Standard => {
                "Play soundly and in character: take sensible risks, avoid obvious blunders."
            }
            Difficulty::Sharp => {
                "Play to win. Weigh detonation liability, fold timing, and spell economy carefully on every wave."
            }
        }
    }
}

/// Measured sizes of one assembled prompt (the growth instrumentation).
#[derive(Debug, Clone, Copy, Default)]
pub struct PromptStats {
    /// Transcript lines available on the view.
    pub transcript_lines_total: usize,
    /// Transcript lines actually included after drop-oldest compaction.
    pub transcript_lines_used: usize,
    /// Characters in the assembled user message.
    pub user_chars: usize,
}

/// The fixed rules summary every prompt carries (public rules only).
const RULES: &str = "Boiling Point rules: 4 players, 5 rounds. Each round, waves of simultaneous commits: \
play one ingredient from your hand into the shared cauldron (face-down) or pass (pass = locked out for the round). \
You may also cast at most one spell per wave. Each ingredient adds hidden volatility; if the cauldron's total \
volatility crosses the round's hidden boiling point, it EXPLODES and only the detonators (the heaviest cards of \
the fatal wave) split the loss. If everyone left passes, the pot settles safely and the dominant colour's owner \
scores its points. Colorless plays add volatility but score nothing. Folding early is safe but forfeits the pot.";

/// Build the system prompt: persona, difficulty framing, rules.
pub fn system_prompt(persona: &str, difficulty: Difficulty) -> String {
    format!(
        "You are playing the card game Boiling Point at a live table, in persona: {persona}. \
{framing}\n\n{RULES}\n\nYou will receive the game so far and your current legal options, and you must \
answer by calling the commit_wave tool with exactly one decision. Choose only from the options listed in the tool.",
        framing = difficulty.framing(),
    )
}

/// Build the user message: bounded transcript + current standing + the ask.
/// Returns the message and its measured sizes.
pub fn user_prompt(
    view: &SeatView,
    frame: &FrameContext,
    transcript_limit: usize,
) -> (String, PromptStats) {
    let transcript = view.transcript();
    let start = transcript.len().saturating_sub(transcript_limit);
    let recent = &transcript[start..];

    let mut out = String::with_capacity(2048);
    if start > 0 {
        out.push_str(&format!(
            "(Earlier events compacted: {start} older lines omitted.)\n"
        ));
    }
    if recent.is_empty() {
        out.push_str("The game has just begun.\n");
    } else {
        out.push_str("Game so far:\n");
        for line in recent {
            out.push_str(line);
            out.push('\n');
        }
    }

    out.push_str(&format!(
        "\nIt is round {}, wave {}. The cauldron holds {} cards. ",
        frame.round_number, frame.wave_number, view.cauldron_card_count
    ));
    match view.known_boiling_point() {
        Some(bp) => out.push_str(&format!("You know the boiling point is exactly {bp}. ")),
        None => out.push_str("The boiling point is unknown to you. "),
    }
    if let Some(score) = view.my_score() {
        out.push_str(&format!("Your score: {score}"));
        if let Some(best) = view.best_opponent_score() {
            out.push_str(&format!(" (best opponent: {best})"));
        }
        out.push_str(". ");
    }
    if let Some(timer) = frame.timer_ms {
        out.push_str(&format!("You have about {} seconds. ", timer / 1000));
    }

    let spells = match &frame.decision {
        PendingDecision::WaveCommit { spells, .. } => spells.as_slice(),
        // The Brewer pick never reaches the prompt: the agent brain answers it
        // deterministically without an API call.
        PendingDecision::BrewerPick { .. } => &[],
    };
    out.push_str("\nDecide now: play one of your hand ingredients (listed in the tool) or pass");
    if spells.is_empty() {
        out.push_str(". No spell is castable this wave.");
    } else {
        out.push_str(", and optionally cast one spell.");
    }
    out.push_str(" Answer ONLY by calling commit_wave.");

    let stats = PromptStats {
        transcript_lines_total: transcript.len(),
        transcript_lines_used: recent.len(),
        user_chars: out.len(),
    };
    (out, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::frame::PendingDecision;
    use boiling_point_protocol::vocab::Color;
    use boiling_point_protocol::{PlayerId, ServerMessage};
    use uuid::Uuid;

    fn frame() -> FrameContext {
        FrameContext {
            round_number: 2,
            wave_number: 1,
            timer_ms: Some(15_000),
            decision: PendingDecision::WaveCommit {
                playable: vec![],
                can_pass: true,
                spells: vec![],
                can_defer: false,
            },
        }
    }

    /// The prompt knows the boiling point only after a sanctioned disclosure —
    /// the no-secrets audit at the prompt seam.
    #[test]
    fn prompt_carries_no_undisclosed_boiling_point() {
        let mut view = SeatView::new(PlayerId(Uuid::from_u128(1)), Color::Ruby).with_transcript();
        let (before, _) = user_prompt(&view, &frame(), 100);
        assert!(before.contains("boiling point is unknown"));
        assert!(!before.contains("exactly"));

        view.observe(&ServerMessage::PeekResult { boiling_point: 27 });
        let (after, _) = user_prompt(&view, &frame(), 100);
        assert!(
            after.contains("exactly 27"),
            "a Peek the seat cast may surface"
        );
    }

    /// Transcript inclusion is bounded with drop-oldest compaction, and the
    /// growth is measured.
    #[test]
    fn transcript_is_bounded_and_measured() {
        let mut view = SeatView::new(PlayerId(Uuid::from_u128(1)), Color::Ruby).with_transcript();
        for round in 1..=30u8 {
            view.observe(&ServerMessage::WaveOpened {
                round_number: round,
                wave_number: 1,
                timer_ms: 1000,
                final_wave: false,
            });
        }
        let (text, stats) = user_prompt(&view, &frame(), 10);
        assert!(stats.transcript_lines_total > 10);
        assert_eq!(stats.transcript_lines_used, 10);
        assert!(text.contains("older lines omitted"));
        assert_eq!(stats.user_chars, text.len());
    }

    /// Persona and difficulty land in the system prompt (framing only).
    #[test]
    fn system_prompt_carries_persona_and_difficulty() {
        let s = system_prompt("Sage Bramble, a cryptic herbalist", Difficulty::Sharp);
        assert!(s.contains("Sage Bramble"));
        assert!(s.contains("Play to win"));
        assert!(s.contains("commit_wave"));
    }
}
