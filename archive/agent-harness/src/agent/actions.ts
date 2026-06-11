// The move a player makes in a wave: commit exactly one card, or pass.
// (Lock-in and emotes are separate tool calls / messages, not "the move".)

import type { CardId, ClientMessage } from "../protocol/messages.ts";
import type { ViewModel } from "../net/view-model.ts";

export type Move = { kind: "commit"; cardId: CardId } | { kind: "pass" };

export function moveToClientMessage(move: Move): ClientMessage {
  return move.kind === "commit"
    ? { type: "CommitCard", card: move.cardId }
    : { type: "CommitPass" };
}

/** Every legal move from the current hand: pass, or commit any single held card. */
export function legalMoves(vm: ViewModel): Move[] {
  const moves: Move[] = [{ kind: "pass" }];
  for (const card of vm.self.hand) moves.push({ kind: "commit", cardId: card.id });
  return moves;
}

export function isMoveLegal(vm: ViewModel, move: Move): boolean {
  if (move.kind === "pass") return true;
  return vm.self.hand.some((c) => c.id === move.cardId);
}
