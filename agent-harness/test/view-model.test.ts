import { test } from "node:test";
import assert from "node:assert/strict";

import { applyServerMessage, createViewModel, currentEpochReveals } from "../src/net/view-model.ts";
import { assertNoSecretLeak } from "../src/net/secret-boundary.ts";
import type { ServerMessage } from "../src/protocol/messages.ts";

function feed(msgs: ServerMessage[]) {
  const vm = createViewModel();
  for (const m of msgs) {
    applyServerMessage(vm, m);
    assertNoSecretLeak(vm);
  }
  return vm;
}

test("builds self + opponents and tracks scores, contribution, lockout, pot", () => {
  const vm = feed([
    {
      type: "RoomJoined",
      room_id: "r1",
      your_player_id: "me",
      your_color: "Ruby",
      players: [
        { id: "me", name: "Me", color: "Ruby", connected: true },
        { id: "opp", name: "Opp", color: "Sapphire", connected: true },
      ],
    },
    { type: "YourHand", cards: [{ id: 1, color: "Ruby", volatility: 1, points: 2 }] },
    { type: "RoundStarted", round_number: 1, threshold_min: 8, threshold_max: 14, multiplier: 1 },
    { type: "WaveOpened", wave_number: 1, timer_ms: 30000 },
    { type: "WaveResolved", committed: ["me"], passed: ["opp"], pot_card_count: 1 },
    { type: "RoundScored", scores: { me: 5, opp: 0 }, deltas: { me: 5, opp: 0 } },
  ]);

  assert.equal(vm.self.playerId, "me");
  assert.equal(vm.self.color, "Ruby");
  assert.equal(vm.self.hand.length, 1);
  assert.equal(vm.pot.cardCount, 1);

  const me = vm.players.get("me");
  const opp = vm.players.get("opp");
  assert.equal(me?.score, 5);
  assert.equal(me?.contribution, 1);
  assert.equal(me?.committedThisWave, true);
  assert.equal(opp?.lockedOut, true);
});

test("never holds a boiling point on a safe brew", () => {
  const vm = feed([
    {
      type: "RoomJoined",
      room_id: "r1",
      your_player_id: "me",
      your_color: "Ruby",
      players: [{ id: "me", name: "Me", color: "Ruby", connected: true }],
    },
    { type: "RoundStarted", round_number: 1, threshold_min: 8, threshold_max: 14, multiplier: 1 },
    { type: "WaveOpened", wave_number: 1, timer_ms: 30000 },
    { type: "WaveResolved", committed: ["me"], passed: [], pot_card_count: 1 },
    {
      type: "RoundRevealed",
      reveals: [{ player_id: "me", card: { id: 1, color: "Ruby", volatility: 1, points: 2 } }],
      outcome: { kind: "Domination", winner: "me" },
    },
  ]);
  assert.equal(vm.self.disclosedBoilingPoint, undefined);
  assert.equal(vm.revealHistory.length, 1);
});

test("boiling point enters only via own Peek or an explosion", () => {
  const vm = createViewModel();
  applyServerMessage(vm, { type: "PeekResult", threshold_value: 11 });
  assert.equal(vm.self.disclosedBoilingPoint, 11);
  assert.equal(vm.self.boilingPointSource, "peek");
  assertNoSecretLeak(vm);

  applyServerMessage(vm, {
    type: "Explosion",
    boiling_point: 9,
    total_volatility: 10,
    crossing_player: "opp",
  });
  assert.equal(vm.self.disclosedBoilingPoint, 9);
  assert.equal(vm.self.boilingPointSource, "explosion");
  assertNoSecretLeak(vm);
});

test("reshuffle starts a new counting epoch", () => {
  const vm = feed([
    {
      type: "RoomJoined",
      room_id: "r1",
      your_player_id: "me",
      your_color: "Ruby",
      players: [{ id: "me", name: "Me", color: "Ruby", connected: true }],
    },
    { type: "RoundStarted", round_number: 1, threshold_min: 8, threshold_max: 14, multiplier: 1 },
    { type: "WaveOpened", wave_number: 1, timer_ms: 30000 },
    { type: "WaveResolved", committed: ["me"], passed: [], pot_card_count: 1 },
    {
      type: "RoundRevealed",
      reveals: [{ player_id: "me", card: { id: 1, color: "Ruby", volatility: 1, points: 2 } }],
      outcome: { kind: "Domination", winner: "me" },
    },
    { type: "DeckReshuffled" },
  ]);
  assert.equal(vm.shuffleEpoch, 1);
  assert.equal(vm.revealHistory.length, 1);
  assert.equal(currentEpochReveals(vm).length, 0, "prior-epoch reveals are out of the current count");
});
