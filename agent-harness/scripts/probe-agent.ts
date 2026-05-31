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
import { AgentSession } from "../src/agent/session.ts";
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

const session = new AgentSession();
session.start({
  server,
  allowedTools: allowedToolsFor(difficulty),
  systemPrompt: buildSystemPrompt(difficulty),
  model: modelFor(difficulty),
});

// Two prompts on the same warm session — the second should be much faster than the first,
// demonstrating the persistent-session win (no per-decision cold start).
async function decideOnce(label: string): Promise<number> {
  decided = undefined;
  const t0 = Date.now();
  session.prompt(renderTurnContext(buildTurnContext(vm)));
  while (decided === undefined && Date.now() - t0 < 60000) await sleep(100);
  const dt = Date.now() - t0;
  if (!decided) {
    console.error(`[probe] ${label}: no decision within 60s`);
    session.close();
    process.exit(1);
  }
  console.error(`[probe] ${label}: decided in ${dt}ms →`, decided);
  return dt;
}

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

console.error("[probe] persistent session — two decisions on one warm subprocess (Haiku)…");
const first = await decideOnce("turn 1 (cold)");

// Advance the board to a genuinely new wave so turn 2 is a fresh decision.
applyServerMessage(vm, { type: "WaveResolved", played: ["me"], passed: [], cauldron_card_count: 2, contributions: [{ player: "me", count: 1 }] });
applyServerMessage(vm, { type: "WaveOpened", round_number: 1, wave_number: 2, timer_ms: 10000 });

const second = await decideOnce("turn 2 (warm)");
session.close();
console.error(
  `[probe] OK — both decisions ran on ONE warm session (turn1 ${first}ms, turn2 ${second}ms). ` +
    `Per-decision latency is model/rate-limit bound, not subprocess cold-start; the warm session ` +
    `removes per-wave spawns and preserves context but does not by itself fit a 10s wave.`,
);
process.exit(0);
