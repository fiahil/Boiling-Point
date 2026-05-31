import { test } from "node:test";
import assert from "node:assert/strict";

import { createViewModel } from "../src/net/view-model.ts";
import { fallbackMove } from "../src/timeliness/fallback.ts";

test("empty hand falls back to a pass", () => {
  const vm = createViewModel();
  assert.deepEqual(fallbackMove(vm), { kind: "pass" });
});

test("commits the lowest-volatility card by default", () => {
  const vm = createViewModel();
  vm.self.hand = [
    { id: 1, color: "Ruby", volatility: 3, points: 0 },
    { id: 2, color: "Sapphire", volatility: 1, points: 2 },
    { id: 3, color: "Emerald", volatility: 2, points: 1 },
  ];
  assert.deepEqual(fallbackMove(vm), { kind: "commit", cardId: 2 });
});

test("passes when the only option is volatile and the pot is late and deep", () => {
  const vm = createViewModel();
  vm.players.set("a", { info: { id: "a", name: "A", color: "Ruby", connected: true }, score: 0, contribution: 0, committedThisWave: false, lockedOut: false });
  vm.players.set("b", { info: { id: "b", name: "B", color: "Sapphire", connected: true }, score: 0, contribution: 0, committedThisWave: false, lockedOut: false });
  vm.wave = { number: 3, open: true };
  vm.pot.cardCount = 4; // >= players.size (2)
  vm.self.hand = [{ id: 9, color: "Wild", volatility: 3, points: 3 }];
  assert.deepEqual(fallbackMove(vm), { kind: "pass" });
});

test("commits the volatile card on the opening wave (not late/deep)", () => {
  const vm = createViewModel();
  vm.wave = { number: 1, open: true };
  vm.pot.cardCount = 0;
  vm.self.hand = [{ id: 9, color: "Wild", volatility: 3, points: 3 }];
  assert.deepEqual(fallbackMove(vm), { kind: "commit", cardId: 9 });
});
