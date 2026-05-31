// In-process MCP tool server (spec: Actions and Capabilities as In-Process MCP Tools).
// Every move Claude can make is a tool; analytical capabilities are separate tools gated
// by difficulty's allowedTools. Tools run in THIS process, so handlers read the live view
// model and validate before forwarding a ClientMessage. EDGE MODULE: depends on the Agent
// SDK + zod; verified against the SDK runtime once deps are installed.

import { createSdkMcpServer, tool } from "@anthropic-ai/claude-agent-sdk";
import { z } from "zod";

import type { CardId } from "../protocol/messages.ts";
import type { Move } from "./actions.ts";
import { isMoveLegal } from "./actions.ts";
import type { ViewModel, RevealRecord } from "../net/view-model.ts";
import { isPaletteEmote } from "../personas/emotes.ts";
import { SERVER_NAME, TOOL_SHORT } from "./tool-names.ts";

export interface ToolDeps {
  getViewModel(): ViewModel;
  /** Forward the chosen move (commit a card / pass) and record the decision for the runner. */
  decideMove(move: Move): void;
  lockIn(): void;
  pickTarget(cardId: CardId): void;
  sendEmote(emoteId: string): void;
  /** Reveal history for the current shuffle epoch — the gated card-counting capability. */
  revealHistory(): RevealRecord[];
}

const ok = (text: string) => ({ content: [{ type: "text" as const, text }] });
const fail = (text: string) => ({ content: [{ type: "text" as const, text }], isError: true });

export function createBpToolServer(deps: ToolDeps) {
  const commitCard = tool(
    TOOL_SHORT.commit_card,
    "Commit exactly one card from your hand to the open wave (changeable until you lock in).",
    { card_id: z.number().int() },
    async ({ card_id }) => {
      const move: Move = { kind: "commit", cardId: card_id };
      if (!isMoveLegal(deps.getViewModel(), move)) return fail(`Card ${card_id} is not in your hand.`);
      deps.decideMove(move);
      return ok(`Committed card ${card_id}.`);
    },
  );

  const pass = tool(
    TOOL_SHORT.pass,
    "Pass this wave. WARNING: passing locks you out for the rest of the round.",
    {},
    async () => {
      deps.decideMove({ kind: "pass" });
      return ok("Passed (locked out for the round).");
    },
  );

  const lockIn = tool(
    TOOL_SHORT.lock_in,
    "Finalize your current selection. When all active players lock in, the wave closes early.",
    {},
    async () => {
      deps.lockIn();
      return ok("Locked in.");
    },
  );

  const pickTarget = tool(
    TOOL_SHORT.pick_target,
    "Resolve a targeted effect (e.g. Recall): choose one of your own cards currently in the pot.",
    { card_id: z.number().int() },
    async ({ card_id }) => {
      deps.pickTarget(card_id);
      return ok(`Picked target ${card_id}.`);
    },
  );

  const sendEmote = tool(
    TOOL_SHORT.send_emote,
    "Send a preset emote (the only communication channel). Must be a palette id; free text is not allowed.",
    { emote_id: z.string() },
    async ({ emote_id }) => {
      if (!isPaletteEmote(emote_id)) return fail(`'${emote_id}' is not a palette emote.`);
      deps.sendEmote(emote_id);
      return ok(`Sent emote ${emote_id}.`);
    },
  );

  // CAPABILITY TOOL — the difficulty dial. Registered here, but only callable when the
  // preset's allowedTools includes it. Returns only CURRENT-epoch reveals (counting resets
  // on reshuffle). Because it is the sole source of past identities, an Easy preset that
  // omits it never receives them (the turn context excludes them too).
  const revealHistory = tool(
    TOOL_SHORT.reveal_history,
    "Card counting: list every card revealed in past depiles this shuffle epoch (color, points, volatility, who played it).",
    {},
    async () => {
      const records = deps.revealHistory();
      const lines = records.flatMap((r) =>
        r.reveals.map(
          (rev) =>
            `round ${r.round}: ${rev.player_id} played #${rev.card.id} ${rev.card.color} vol${rev.card.volatility}/pts${rev.card.points}${rev.card.effect ? ` [${rev.card.effect}]` : ""}`,
        ),
      );
      return ok(lines.length ? lines.join("\n") : "No cards revealed yet this shuffle epoch.");
    },
  );

  return createSdkMcpServer({
    name: SERVER_NAME,
    version: "0.0.0",
    tools: [commitCard, pass, lockIn, pickTarget, sendEmote, revealHistory],
  });
}
