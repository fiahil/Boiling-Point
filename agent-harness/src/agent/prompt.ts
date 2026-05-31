// System-prompt assembly (pure). Rules + optional persona fragment + a difficulty note.
// The thin per-turn context (context.ts) is sent as the user turn, NOT here — that keeps
// the gating rule intact (design D3).

import type { Difficulty } from "./difficulty.ts";
import { canCountCards } from "./difficulty.ts";
import { personaPrompt, type Archetype } from "../personas/archetypes.ts";

export const RULES = `You are playing Boiling Point, a 4-player blind-commitment card game.

Each round, players add cards to a shared cauldron over a series of simultaneous "waves".
In each wave you secretly commit ONE card or pass; commits reveal together when the wave closes.
Passing locks you out for the rest of the round.

Every card has a COLOR (a player color, or Wild), a VOLATILITY (1-3, explosion risk), and
POINTS (0-3). The cauldron has a hidden boiling point (a volatility threshold). If total
volatility exceeds it, the cauldron EXPLODES and EVERY player loses points equal to the pot.
If it never explodes, the color with the highest TOTAL POINTS in the pot wins ALL the pot's
points (ties split). You only ever learn the boiling point if you Peek or it explodes.

You act ONLY by calling tools: commit_card, pass, lock_in, pick_target, send_emote.
Decide deliberately, then lock_in so the table is not kept waiting. Emotes (preset only) are
table-talk: bluff, taunt, or mislead freely — they carry no mechanical weight.`;

export function buildSystemPrompt(difficulty: Difficulty, archetype?: Archetype): string {
  const parts = [RULES];

  parts.push(
    canCountCards(difficulty)
      ? "You may call reveal_history to review every card revealed in past depiles this shuffle epoch — count cards and deduce what remains."
      : "You do NOT track the depile reveals; play from the public state and your own hand on instinct.",
  );

  const persona = personaPrompt(archetype);
  if (persona) parts.push(persona);

  return parts.join("\n\n");
}
