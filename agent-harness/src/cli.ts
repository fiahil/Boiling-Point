#!/usr/bin/env -S node --experimental-strip-types
// bp-bot — launch ONE agent seat (spec: Per-Seat Process; design D7). Each invocation is
// its own process with its own connection, session, difficulty, and persona. To fill a
// table, run several and join as the remaining seat.
//
//   bp-bot --room BREW-7K3F --difficulty hard --persona gambler [--epsilon 0.1]

import { parseArgs } from "node:util";
import { configureAuth } from "./auth.ts";
import { AgentRunner } from "./runner.ts";
import { isDifficulty, DIFFICULTIES } from "./agent/difficulty.ts";
import { isArchetype, ARCHETYPES_LIST } from "./personas/archetypes.ts";
import type { WireMode } from "./protocol/codec.ts";

function main(): void {
  const { values } = parseArgs({
    options: {
      room: { type: "string" },
      difficulty: { type: "string", default: "hard" },
      persona: { type: "string" },
      epsilon: { type: "string", default: "0" },
      url: { type: "string", default: "ws://127.0.0.1:8080/ws" },
      name: { type: "string" },
      mode: { type: "string", default: "msgpack" },
      "fallback-at": { type: "string", default: "0.8" },
    },
  });

  const room = values.room;
  if (!room) fatal("missing --room <code>");

  const difficulty = values.difficulty ?? "hard";
  if (!isDifficulty(difficulty)) fatal(`--difficulty must be one of: ${DIFFICULTIES.join(", ")}`);

  let archetype;
  if (values.persona !== undefined) {
    if (!isArchetype(values.persona)) fatal(`--persona must be one of: ${ARCHETYPES_LIST.join(", ")}`);
    archetype = values.persona;
  }

  const epsilon = Number(values.epsilon);
  if (!Number.isFinite(epsilon) || epsilon < 0 || epsilon > 1) fatal("--epsilon must be between 0 and 1");

  const mode = values.mode === "json" ? "json" : "msgpack";

  const auth = configureAuth();
  if (auth.path === "none") fatal("no usable credentials — see the [auth] message above");

  const name = values.name ?? `bot-${difficulty}${archetype ? `-${archetype}` : ""}`;

  const runner = new AgentRunner({
    url: values.url ?? "ws://127.0.0.1:8080/ws",
    room,
    name,
    difficulty,
    archetype,
    epsilon,
    mode: mode as WireMode,
    fallbackAt: Number(values["fallback-at"]) || 0.8,
  });

  console.error(`[bp-bot] joining ${room} as ${name} (${difficulty}${archetype ? `/${archetype}` : ""}, ε=${epsilon})`);
  runner.start().catch((err) => fatal(`runner failed: ${String(err)}`));
}

function fatal(msg: string): never {
  console.error(`[bp-bot] ${msg}`);
  process.exit(1);
}

main();
