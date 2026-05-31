// Fast local fallback (spec: Timely Commitment Within the Wave; design D4).
// When Claude has not produced an action as the wave deadline approaches, the harness
// commits this move so the table never stalls waiting on a slow agent. It is a cheap,
// explainable, safe-leaning heuristic computed with NO LLM call — the TS analogue of a
// baseline bot strategy. A persona may override the bias (see archetypes.biasFallback).

import type { Card } from "../protocol/messages.ts";
import type { Move } from "../agent/actions.ts";
import type { ViewModel } from "../net/view-model.ts";

/**
 * Safe-leaning baseline:
 *  - empty hand → pass;
 *  - otherwise commit the lowest-volatility, then lowest-points card;
 *  - but if the only candidate is highly volatile (vol 3) and the pot is already deep
 *    (this is a later wave with several cards down), prefer to pass rather than risk a boom.
 */
export function fallbackMove(vm: ViewModel): Move {
  const hand = vm.self.hand;
  if (hand.length === 0) return { kind: "pass" };

  const safest = [...hand].sort(compareSafety)[0] as Card;

  const lateAndDeep = (vm.wave?.number ?? 1) > 1 && vm.pot.cardCount >= vm.players.size;
  if (safest.volatility >= 3 && lateAndDeep) return { kind: "pass" };

  return { kind: "commit", cardId: safest.id };
}

function compareSafety(a: Card, b: Card): number {
  if (a.volatility !== b.volatility) return a.volatility - b.volatility;
  if (a.points !== b.points) return a.points - b.points;
  return a.id - b.id;
}
