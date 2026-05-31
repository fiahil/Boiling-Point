// Fully-qualified MCP tool names. The Agent SDK names in-process tools
// `mcp__{server}__{tool}`; difficulty presets gate on these exact strings, and
// the in-process MCP server registers the matching short names (see tools.ts).

export const SERVER_NAME = "bp";

const q = (name: string): string => `mcp__${SERVER_NAME}__${name}`;

export const TOOL_SHORT = {
  commit_card: "commit_card",
  pass: "pass",
  lock_in: "lock_in",
  pick_target: "pick_target",
  send_emote: "send_emote",
  reveal_history: "reveal_history",
} as const;

export const TOOL = {
  commit_card: q(TOOL_SHORT.commit_card),
  pass: q(TOOL_SHORT.pass),
  lock_in: q(TOOL_SHORT.lock_in),
  pick_target: q(TOOL_SHORT.pick_target),
  send_emote: q(TOOL_SHORT.send_emote),
  reveal_history: q(TOOL_SHORT.reveal_history),
} as const;

/** Action tools — always granted; these are how the agent moves. */
export const ACTION_TOOLS: readonly string[] = [
  TOOL.commit_card,
  TOOL.pass,
  TOOL.lock_in,
  TOOL.pick_target,
  TOOL.send_emote,
];

/** Capability tools — the difficulty dial. Withholding one removes the capability. */
export const CAPABILITY_TOOLS = {
  reveal_history: TOOL.reveal_history,
} as const;
