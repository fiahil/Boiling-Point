#!/usr/bin/env -S node --experimental-strip-types
// bp-bot — launch ONE agent seat (spec: Per-Seat Process; design D7). Each invocation is
// its own process with its own connection, session, difficulty, and persona. To fill a
// table, run several and join as the remaining seat.
//
//   bp-bot --create                       # bot opens a group, prints the invite code
//   bp-bot --group BREW-7K3F --persona gambler
//   bp-bot --enqueue --brain fallback     # zero-cost seat filler via matchmaking

import { parseArgs } from "node:util";
import { configureAuth } from "./auth.ts";
import { AgentRunner, type Brain } from "./runner.ts";
import { isDifficulty, DIFFICULTIES } from "./agent/difficulty.ts";
import { isArchetype, ARCHETYPES_LIST } from "./personas/archetypes.ts";
import type { EntryConfig } from "./net/connection.ts";
import type { WireMode } from "./protocol/codec.ts";

function main(): void {
  const { values } = parseArgs({
    options: {
      group: { type: "string" },
      create: { type: "boolean", default: false },
      enqueue: { type: "boolean", default: false },
      difficulty: { type: "string", default: "hard" },
      persona: { type: "string" },
      epsilon: { type: "string", default: "0" },
      brain: { type: "string", default: "claude" },
      url: { type: "string", default: "ws://127.0.0.1:8080/ws" },
      name: { type: "string" },
      mode: { type: "string", default: "msgpack" },
      "fallback-at": { type: "string", default: "0.8" },
    },
  });

  const difficulty = values.difficulty ?? "hard";
  if (!isDifficulty(difficulty)) fatal(`--difficulty must be one of: ${DIFFICULTIES.join(", ")}`);

  let archetype;
  if (values.persona !== undefined) {
    if (!isArchetype(values.persona)) fatal(`--persona must be one of: ${ARCHETYPES_LIST.join(", ")}`);
    archetype = values.persona;
  }

  const brain = (values.brain ?? "claude") as Brain;
  if (brain !== "claude" && brain !== "fallback") fatal("--brain must be 'claude' or 'fallback'");

  const epsilon = Number(values.epsilon);
  if (!Number.isFinite(epsilon) || epsilon < 0 || epsilon > 1) fatal("--epsilon must be between 0 and 1");

  if (values.mode === "json") {
    fatal("--mode json is not supported on the wire (the server accepts only binary MessagePack)");
  }
  const mode: WireMode = "msgpack";

  const name = values.name ?? `bot-${difficulty}${archetype ? `-${archetype}` : ""}`;

  // Resolve the entry mode.
  let entry: EntryConfig;
  if (values.create) entry = { kind: "create", displayName: name };
  else if (values.enqueue) entry = { kind: "enqueue", displayName: name };
  else if (values.group) entry = { kind: "join", displayName: name, groupCode: values.group };
  else fatal("specify one of --group <code>, --create, or --enqueue");

  // Auth is only needed for the LLM brain.
  if (brain === "claude") configureAuth();

  const runner = new AgentRunner({
    url: values.url ?? "ws://127.0.0.1:8080/ws",
    entry,
    difficulty,
    archetype,
    epsilon,
    mode,
    brain,
    fallbackAt: Number(values["fallback-at"]) || 0.8,
  });

  console.error(`[bp-bot] ${entry.kind} as ${name} (${brain}/${difficulty}${archetype ? `/${archetype}` : ""}, ε=${epsilon})`);
  runner
    .start()
    .then(async (joined) => {
      console.error(`[bp-bot] joined group ${joined.groupCode} as color ${joined.yourColor} (id ${joined.yourPlayerId})`);
      const result = await runner.whenDone();
      if (result.aborted) {
        console.error("[bp-bot] connection closed before game over");
        process.exit(1);
      }
      const won = result.winners.includes(joined.yourPlayerId);
      console.error(`[bp-bot] game over — winners: ${result.winners.join(", ")}${won ? " (you won!)" : ""}`);
      process.exit(0);
    })
    .catch((err) => fatal(`runner failed: ${String(err)}`));
}

function fatal(msg: string): never {
  console.error(`[bp-bot] ${msg}`);
  process.exit(1);
}

main();
