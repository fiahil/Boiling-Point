// A PERSISTENT Agent SDK session for one game (design D11). The earlier v0 spawned a fresh
// query() — and thus a cold Claude CLI subprocess (~20s) — every wave. Here a single query()
// runs for the whole game in streaming-input mode: we push the thin per-turn context as a
// user message each wave, the agent answers by calling a move tool (captured via the
// in-process MCP handler), and the subprocess stays warm so subsequent decisions are fast.
//
// EDGE MODULE: depends on the Agent SDK runtime.

import { query, type SDKUserMessage } from "@anthropic-ai/claude-agent-sdk";
import type { createBpToolServer } from "./tools.ts";
import { SERVER_NAME } from "./tool-names.ts";

export interface AgentSessionConfig {
  server: ReturnType<typeof createBpToolServer>;
  allowedTools: string[];
  systemPrompt: string;
  model: string;
}

function userMessage(text: string): SDKUserMessage {
  return { type: "user", message: { role: "user", content: text }, parent_tool_use_id: null };
}

/** Keeps one Agent SDK query() alive for the whole game; prompt() pushes one wave's turn. */
export class AgentSession {
  private queue: SDKUserMessage[] = [];
  private wake: (() => void) | null = null;
  private closed = false;
  private started = false;

  start(cfg: AgentSessionConfig): void {
    if (this.started) return;
    this.started = true;

    // A pushable async-iterable: yields queued user messages, blocking until one arrives.
    const self = this;
    const input = (async function* (): AsyncGenerator<SDKUserMessage> {
      while (!self.closed) {
        if (self.queue.length === 0) {
          await new Promise<void>((resolve) => {
            self.wake = resolve;
          });
          if (self.closed) return;
        }
        const next = self.queue.shift();
        if (next) yield next;
      }
    })();

    const stream = query({
      prompt: input,
      options: {
        model: cfg.model,
        systemPrompt: cfg.systemPrompt,
        mcpServers: { [SERVER_NAME]: cfg.server },
        allowedTools: cfg.allowedTools,
        // This agent only plays the game — no file/bash/web tools.
        disallowedTools: ["Bash", "Read", "Write", "Edit", "WebSearch", "WebFetch"],
      },
    });

    // Drain the output in the background. Decisions are captured by the MCP tool handlers
    // (deps.decideMove); we don't need the assistant text, just to keep the stream flowing.
    void (async () => {
      try {
        for await (const message of stream) {
          if (process.env.BP_DEBUG) console.error(`[session] <- ${message.type}`);
          if (self.closed) break;
        }
        if (process.env.BP_DEBUG) console.error("[session] stream ENDED");
      } catch (err) {
        if (!self.closed) console.error("[agent] session error:", err);
      }
    })();
  }

  /** Push one wave's thin context as a user turn; the agent answers with a tool call. */
  prompt(text: string): void {
    if (this.closed || !this.started) return;
    this.queue.push(userMessage(text));
    const w = this.wake;
    this.wake = null;
    if (w) w();
  }

  close(): void {
    this.closed = true;
    const w = this.wake;
    this.wake = null;
    if (w) w();
  }
}
