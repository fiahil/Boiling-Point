import { test } from "node:test";
import assert from "node:assert/strict";

import { applyServerMessage, createViewModel } from "../src/net/view-model.ts";
import { buildTurnContext, renderTurnContext, FORBIDDEN_CONTEXT_KEYS } from "../src/agent/context.ts";
import type { ServerMessage } from "../src/protocol/messages.ts";

// A card id that appears ONLY in a past depile reveal — never in the agent's hand.
const SECRET_PAST_ID = 9999;

function withHistory() {
  const vm = createViewModel();
  const msgs: ServerMessage[] = [
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
    { type: "WaveResolved", committed: ["me", "opp"], passed: [], pot_card_count: 2 },
    {
      // A prior round's reveal — the kind of identity a card-counter would use.
      type: "RoundRevealed",
      reveals: [{ player_id: "opp", card: { id: SECRET_PAST_ID, color: "Emerald", volatility: 3, points: 3 } }],
      outcome: { kind: "Domination", winner: "opp" },
    },
    { type: "RoundStarted", round_number: 2, threshold_min: 8, threshold_max: 14, multiplier: 1 },
    { type: "WaveOpened", wave_number: 1, timer_ms: 30000 },
  ];
  for (const m of msgs) applyServerMessage(vm, m);
  return vm;
}

test("the turn context excludes reveal history and pot identities", () => {
  const vm = withHistory();
  assert.ok(vm.revealHistory.length > 0, "the model DOES hold the public history");

  const ctx = buildTurnContext(vm);
  const keys = Object.keys(ctx);
  for (const forbidden of FORBIDDEN_CONTEXT_KEYS) {
    assert.ok(!keys.includes(forbidden), `context must not expose '${forbidden}'`);
  }
});

test("a tool-starved agent's rendered context never leaks a past card identity", () => {
  const vm = withHistory();
  const rendered = renderTurnContext(buildTurnContext(vm));
  assert.ok(
    !rendered.includes(String(SECRET_PAST_ID)),
    "past reveal identity must not appear in the thin context — it is reachable only via reveal_history",
  );
  // The agent's own hand (id 1) is fine to show.
  assert.ok(rendered.includes("#1"));
});
