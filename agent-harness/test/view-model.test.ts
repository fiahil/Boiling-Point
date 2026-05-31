import { test } from "node:test";
import assert from "node:assert/strict";

import { applyServerMessage, createViewModel, currentEpochReveals } from "../src/net/view-model.ts";
import { assertNoSecretLeak } from "../src/net/secret-boundary.ts";
import type { CardView, ServerMessage } from "../src/protocol/messages.ts";

const RUBY: CardView = { color: "Ruby", volatility: 1, points: 2, effect: null };

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
      room_code: "BREW-7K3F",
      your_player_id: "me",
      your_color: "Ruby",
      players: [
        { id: "me", display_name: "Me", color: "Ruby", connected: true },
        { id: "opp", display_name: "Opp", color: "Sapphire", connected: true },
      ],
    },
    { type: "YourHand", cards: [{ id: 1, view: RUBY }] },
    { type: "WaveOpened", round_number: 1, wave_number: 1, timer_ms: 30000 },
    { type: "WaveResolved", played: ["me"], passed: ["opp"], cauldron_card_count: 1, contributions: [{ player: "me", count: 1 }] },
    { type: "ScoreUpdate", scores: [{ player: "me", score: 5 }, { player: "opp", score: 0 }] },
  ]);

  assert.equal(vm.self.playerId, "me");
  assert.equal(vm.self.color, "Ruby");
  assert.equal(vm.self.hand.length, 1);
  assert.equal(vm.pot.cardCount, 1);

  const me = vm.players.get("me");
  const opp = vm.players.get("opp");
  assert.equal(me?.score, 5);
  assert.equal(me?.contribution, 1);
  assert.equal(me?.committedLastWave, true);
  assert.equal(opp?.lockedOut, true);
});

test("never holds a boiling point on a safe brew", () => {
  const vm = feed([
    {
      type: "RoomJoined",
      room_code: "BREW-7K3F",
      your_player_id: "me",
      your_color: "Ruby",
      players: [{ id: "me", display_name: "Me", color: "Ruby", connected: true }],
    },
    { type: "WaveOpened", round_number: 1, wave_number: 1, timer_ms: 30000 },
    { type: "WaveResolved", played: ["me"], passed: [], cauldron_card_count: 1, contributions: [{ player: "me", count: 1 }] },
    {
      type: "Depile",
      reveals: [{ player: "me", card: RUBY, running_volatility: 1 }],
      exploded: false,
      boiling_point: null,
      crossing_index: null,
    },
  ]);
  assert.equal(vm.self.disclosedBoilingPoint, undefined);
  assert.equal(vm.revealHistory.length, 1);
});

test("boiling point enters only via own Peek or an exploded depile", () => {
  const vm = createViewModel();
  applyServerMessage(vm, { type: "PeekResult", boiling_point: 11 });
  assert.equal(vm.self.disclosedBoilingPoint, 11);
  assert.equal(vm.self.boilingPointSource, "peek");
  assertNoSecretLeak(vm);

  applyServerMessage(vm, {
    type: "Depile",
    reveals: [{ player: "opp", card: RUBY, running_volatility: 9 }],
    exploded: true,
    boiling_point: 9,
    crossing_index: 0,
  });
  assert.equal(vm.self.disclosedBoilingPoint, 9);
  assert.equal(vm.self.boilingPointSource, "explosion");
  assertNoSecretLeak(vm);
});

test("reshuffle starts a new counting epoch", () => {
  const vm = feed([
    { type: "WaveOpened", round_number: 1, wave_number: 1, timer_ms: 30000 },
    { type: "WaveResolved", played: ["me"], passed: [], cauldron_card_count: 1, contributions: [] },
    {
      type: "Depile",
      reveals: [{ player: "me", card: RUBY, running_volatility: 1 }],
      exploded: false,
      boiling_point: null,
      crossing_index: null,
    },
    { type: "DeckReshuffled" },
  ]);
  assert.equal(vm.shuffleEpoch, 1);
  assert.equal(vm.revealHistory.length, 1);
  assert.equal(currentEpochReveals(vm).length, 0, "prior-epoch reveals are out of the current count");
});
