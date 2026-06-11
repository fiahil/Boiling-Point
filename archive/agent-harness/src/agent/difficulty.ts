// Difficulty IS the granted tool set (spec: Difficulty Is the Granted Tool Set; design D3).
// A preset is exactly the allowedTools array passed to the Agent SDK. Withholding a
// capability tool removes the capability — and because the turn context is thin, an
// agent that cannot call reveal_history never receives past card identities at all.

import { ACTION_TOOLS, CAPABILITY_TOOLS } from "./tool-names.ts";

export type Difficulty = "easy" | "hard";

export const DIFFICULTIES: readonly Difficulty[] = ["easy", "hard"];

export function isDifficulty(value: string): value is Difficulty {
  return (DIFFICULTIES as readonly string[]).includes(value);
}

/** The capability tools each preset grants on top of the always-on action tools. */
const CAPABILITIES_BY_DIFFICULTY: Record<Difficulty, readonly string[]> = {
  easy: [], // actions only — no card-counting, plays on the thin public view alone
  hard: [CAPABILITY_TOOLS.reveal_history], // may count cards within the current shuffle epoch
};

/** The exact `allowedTools` list for a preset. */
export function allowedToolsFor(difficulty: Difficulty): string[] {
  return [...ACTION_TOOLS, ...CAPABILITIES_BY_DIFFICULTY[difficulty]];
}

/** Default model per preset — Easy may also lean on a smaller model (design D5). */
export function modelFor(difficulty: Difficulty): string {
  return difficulty === "easy" ? "claude-haiku-4-5-20251001" : "claude-sonnet-4-6";
}

export function canCountCards(difficulty: Difficulty): boolean {
  return CAPABILITIES_BY_DIFFICULTY[difficulty].includes(CAPABILITY_TOOLS.reveal_history);
}
