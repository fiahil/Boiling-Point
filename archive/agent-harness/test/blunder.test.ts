import { test } from "node:test";
import assert from "node:assert/strict";

import { createViewModel } from "../src/net/view-model.ts";
import { maybeBlunder } from "../src/personas/blunder.ts";
import { isMoveLegal, type Move } from "../src/agent/actions.ts";

function vmWithHand() {
  const vm = createViewModel();
  vm.self.hand = [
    { id: 1, view: { color: "Ruby", volatility: 1, points: 0, effect: null } },
    { id: 2, view: { color: "Sapphire", volatility: 3, points: 3, effect: null } },
  ];
  return vm;
}

/** A scripted RNG that yields the given values in order, then 0. */
function scriptedRng(values: number[]): () => number {
  let i = 0;
  return () => values[i++] ?? 0;
}

test("epsilon 0 never overrides the chosen move", () => {
  const vm = vmWithHand();
  const chosen: Move = { kind: "commit", cardId: 1 };
  for (let n = 0; n < 100; n++) {
    const r = maybeBlunder(chosen, vm, 0);
    assert.equal(r.blundered, false);
    assert.deepEqual(r.move, chosen);
  }
});

test("epsilon 1 always substitutes a legal move", () => {
  const vm = vmWithHand();
  const chosen: Move = { kind: "commit", cardId: 1 };
  for (let n = 0; n < 100; n++) {
    const r = maybeBlunder(chosen, vm, 1);
    assert.equal(r.blundered, true);
    assert.ok(isMoveLegal(vm, r.move), "blunder must still be legal");
  }
});

test("blunder is deterministic under a seeded rng", () => {
  const vm = vmWithHand();
  const chosen: Move = { kind: "pass" };
  // First draw 0.1 (< epsilon 0.5 → blunder), second draw 0.99 → last legal option.
  const r = maybeBlunder(chosen, vm, 0.5, scriptedRng([0.1, 0.99]));
  assert.equal(r.blundered, true);
  // legalMoves = [pass, commit#1, commit#2]; index floor(0.99*3)=2 → commit#2
  assert.deepEqual(r.move, { kind: "commit", cardId: 2 });
});

test("a draw above epsilon keeps the chosen move", () => {
  const vm = vmWithHand();
  const chosen: Move = { kind: "commit", cardId: 1 };
  const r = maybeBlunder(chosen, vm, 0.3, scriptedRng([0.9]));
  assert.equal(r.blundered, false);
  assert.deepEqual(r.move, chosen);
});
