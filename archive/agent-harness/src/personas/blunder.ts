// Optional blunder injection (spec: Optional Blunder Injection; design D5).
// With probability epsilon, the agent's chosen move is replaced by a uniformly random
// LEGAL move. This is the reliable difficulty knob, independent of model competence —
// the dependable skill-down lever if a "casual" persona still plays too well.
// Default epsilon is 0 (off) in v0.

import type { Move } from "../agent/actions.ts";
import { legalMoves } from "../agent/actions.ts";
import type { ViewModel } from "../net/view-model.ts";

export type Rng = () => number;

export interface BlunderResult {
  move: Move;
  blundered: boolean;
}

/** Returns the chosen move, or — with probability epsilon — a random legal substitute. */
export function maybeBlunder(
  chosen: Move,
  vm: ViewModel,
  epsilon: number,
  rng: Rng = Math.random,
): BlunderResult {
  const e = clamp01(epsilon);
  if (e <= 0) return { move: chosen, blundered: false };
  if (rng() >= e) return { move: chosen, blundered: false };

  const options = legalMoves(vm);
  if (options.length === 0) return { move: chosen, blundered: false };
  const idx = Math.min(options.length - 1, Math.floor(rng() * options.length));
  const random = options[idx] ?? chosen;
  return { move: random, blundered: true };
}

function clamp01(x: number): number {
  if (!Number.isFinite(x)) return 0;
  if (x < 0) return 0;
  if (x > 1) return 1;
  return x;
}
