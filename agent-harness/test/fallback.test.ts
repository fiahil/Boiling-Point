import { test } from "node:test";
import assert from "node:assert/strict";

import { createViewModel, type PlayerView } from "../src/net/view-model.ts";
import { fallbackMove } from "../src/timeliness/fallback.ts";
import type { HandCard } from "../src/protocol/messages.ts";

const card = (id: number, volatility: number, points: number, color = "Ruby" as const): HandCard => ({
  id,
  view: { color, volatility, points, effect: null },
});

const player = (id: string): PlayerView => ({
  info: { id, display_name: id, color: "Ruby", connected: true, guest: false },
  score: 0,
  contribution: 0,
  committedLastWave: false,
  lockedOut: false,
});

test("empty hand falls back to a pass", () => {
  const vm = createViewModel();
  assert.deepEqual(fallbackMove(vm), { kind: "pass" });
});

test("commits the lowest-volatility card by default", () => {
  const vm = createViewModel();
  vm.self.hand = [card(1, 3, 0), card(2, 1, 2), card(3, 2, 1)];
  assert.deepEqual(fallbackMove(vm), { kind: "commit", cardId: 2 });
});

test("passes when the only option is volatile and the pot is late and deep", () => {
  const vm = createViewModel();
  vm.players.set("a", player("a"));
  vm.players.set("b", player("b"));
  vm.wave = { number: 3, open: true };
  vm.pot.cardCount = 4; // >= players.size (2)
  vm.self.hand = [{ id: 9, view: { color: "Wild", volatility: 3, points: 3, effect: null } }];
  assert.deepEqual(fallbackMove(vm), { kind: "pass" });
});

test("commits the volatile card on the opening wave (not late/deep)", () => {
  const vm = createViewModel();
  vm.wave = { number: 1, open: true };
  vm.pot.cardCount = 0;
  vm.self.hand = [{ id: 9, view: { color: "Wild", volatility: 3, points: 3, effect: null } }];
  assert.deepEqual(fallbackMove(vm), { kind: "commit", cardId: 9 });
});
