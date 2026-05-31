// Runs ONE decision through the Agent SDK: a long-lived game uses many of these, each fed
// the thin per-turn context as the user turn. The agent decides by calling a move tool,
// which sets a latch the runner watches; we stop consuming the stream once it decides.
// EDGE MODULE: depends on the Agent SDK runtime; the exact query() option surface is
// PROVISIONAL and verified once deps are installed (design D1).

import { query } from "@anthropic-ai/claude-agent-sdk";
import type { createBpToolServer } from "./tools.ts";
import { SERVER_NAME } from "./tool-names.ts";

export interface AgentTurnOptions {
  server: ReturnType<typeof createBpToolServer>;
  allowedTools: string[];
  systemPrompt: string;
  model: string;
  /** The thin per-turn context, rendered for the user turn. */
  prompt: string;
  /** True once a move tool has fired — we can stop consuming. */
  isDecided: () => boolean;
}

/** Drive the agent until it commits a move (via a tool) or the run ends. */
export async function runAgentTurn(opts: AgentTurnOptions): Promise<void> {
  const stream = query({
    prompt: opts.prompt,
    options: {
      model: opts.model,
      systemPrompt: opts.systemPrompt,
      mcpServers: { [SERVER_NAME]: opts.server },
      allowedTools: opts.allowedTools,
      // This agent only plays the game — no file/bash/web tools.
      disallowedTools: ["Bash", "Read", "Write", "Edit", "WebSearch", "WebFetch"],
    },
  });

  for await (const _message of stream) {
    if (opts.isDecided()) break;
  }
}
