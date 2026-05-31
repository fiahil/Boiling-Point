// One-shot probe of the Agent SDK path: synthesize a view model, run a single agent turn,
// and report whether the agent called a move tool (and which). No game server involved —
// this isolates auth + SDK + in-process MCP tools. Costs one cheap (Haiku) decision.
//
//   node --experimental-strip-types scripts/probe-agent.ts

import { configureAuth } from "../src/auth.ts";
import { applyServerMessage, createViewModel, currentEpochReveals } from "../src/net/view-model.ts";
import { buildTurnContext, renderTurnContext } from "../src/agent/context.ts";
import { buildSystemPrompt } from "../src/agent/prompt.ts";
import { allowedToolsFor, modelFor } from "../src/agent/difficulty.ts";
import { createBpToolServer, type ToolDeps } from "../src/agent/tools.ts";
import { runAgentTurn } from "../src/agent/session.ts";
import type { Move } from "../src/agent/actions.ts";

configureAuth();

const vm = createViewModel();
applyServerMessage(vm, {
  type: "RoomJoined",
  room_code: "PROBE",
  your_player_id: "me",
  your_color: "Ruby",
  players: [{ id: "me", display_name: "Probe", color: "Ruby", connected: true }],
});
applyServerMessage(vm, {
  type: "YourHand",
  cards: [
    { id: 10, view: { color: "Ruby", volatility: 1, points: 2, effect: null } },
    { id: 11, view: { color: "Sapphire", volatility: 3, points: 3, effect: null } },
    { id: 12, view: { color: "Wild", volatility: 2, points: 0, effect: "Peek" } },
  ],
});
applyServerMessage(vm, { type: "WaveOpened", round_number: 1, wave_number: 1, timer_ms: 30000 });

let decided: Move | undefined;
const deps: ToolDeps = {
  getViewModel: () => vm,
  decideMove: (m) => {
    decided = m;
  },
  lockIn: () => {},
  sendEmote: () => {},
  revealHistory: () => currentEpochReveals(vm),
};

const server = createBpToolServer(deps);
const difficulty = "easy";

console.error("[probe] running one agent turn (Haiku)…");
const t0 = Date.now();
try {
  await runAgentTurn({
    server,
    allowedTools: allowedToolsFor(difficulty),
    systemPrompt: buildSystemPrompt(difficulty),
    model: modelFor(difficulty),
    prompt: renderTurnContext(buildTurnContext(vm)),
    isDecided: () => decided !== undefined,
  });
} catch (err) {
  console.error("[probe] FAILED:", err);
  process.exit(2);
}
const dt = Date.now() - t0;

if (decided) {
  console.error(`[probe] OK — agent decided in ${dt}ms:`, decided);
  process.exit(0);
} else {
  console.error(`[probe] agent finished without calling a move tool (${dt}ms)`);
  process.exit(1);
}
