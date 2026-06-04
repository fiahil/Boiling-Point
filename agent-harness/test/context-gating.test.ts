import { test } from "node:test";
import assert from "node:assert/strict";

import { applyServerMessage, createViewModel } from "../src/net/view-model.ts";
import { buildTurnContext, renderTurnContext, FORBIDDEN_CONTEXT_KEYS } from "../src/agent/context.ts";
import type { CardView, ServerMessage } from "../src/protocol/messages.ts";

const MY_CARD: CardView = { color: "Ruby", volatility: 1, points: 2, effect: null };
// A past-reveal card distinct from anything in hand — the kind a counter would exploit.
const PAST_REVEAL: CardView = { color: "Emerald", volatility: 3, points: 3, effect: "DoubleDown" };

function withHistory() {
  const vm = createViewModel();
  const msgs: ServerMessage[] = [
    {
      type: "GroupJoined",
      session_token: "test-session",
      group_code: "BREW-7K3F",
      your_player_id: "me",
      your_color: "Ruby",
      players: [
        { id: "me", display_name: "Me", color: "Ruby", connected: true, guest: false },
        { id: "opp", display_name: "Opp", color: "Sapphire", connected: true, guest: false },
      ],
    },
    { type: "YourHand", cards: [{ id: 1, view: MY_CARD }] },
    { type: "WaveOpened", round_number: 1, wave_number: 1, timer_ms: 30000, final_wave: false },
    { type: "WaveResolved", played: ["me", "opp"], passed: [], cauldron_card_count: 2, contributions: [] },
    {
      type: "Depile",
      reveals: [{ player: "opp", card: PAST_REVEAL, running_volatility: 3 }],
      exploded: false,
      boiling_point: null,
      crossing_index: null,
    },
    { type: "WaveOpened", round_number: 2, wave_number: 1, timer_ms: 30000, final_wave: false },
  ];
  for (const m of msgs) applyServerMessage(vm, m);
  return vm;
}

test("the turn context excludes reveal history and pot identities", () => {
  const vm = withHistory();
  assert.ok(vm.revealHistory.length > 0, "the model DOES hold the public history");

  const ctx = buildTurnContext(vm);
  for (const forbidden of FORBIDDEN_CONTEXT_KEYS) {
    assert.ok(!Object.keys(ctx).includes(forbidden), `context must not expose '${forbidden}'`);
  }
  // No depile entry leaked: running_volatility is a field unique to DepileEntry.
  assert.ok(!JSON.stringify(ctx).includes("running_volatility"));
});

test("a tool-starved agent's rendered context never leaks a past card identity", () => {
  const vm = withHistory();
  const rendered = renderTurnContext(buildTurnContext(vm));
  // The DoubleDown effect existed ONLY in the past reveal, never in this agent's hand.
  assert.ok(!rendered.includes("DoubleDown"), "past reveal must not appear in the thin context");
  // The agent's own hand (card #1) is fine to show.
  assert.ok(rendered.includes("#1"));
});
