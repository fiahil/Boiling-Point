// The thin per-turn context handed to Claude. THIS IS THE GATING RULE (design D3):
// it carries only what every difficulty trivially has — the agent's own hand, public
// wave state, scores, active modifiers, and the (known, fixed) boiling-point range. It
// deliberately EXCLUDES the depile history and any pot card identities, so an agent whose
// preset omits the reveal_history tool can never obtain past identities.

import type { Color, HandCard, ModifierKind } from "../protocol/messages.ts";
import type { ViewModel } from "../net/view-model.ts";

/** The base boiling-point range is fixed game knowledge (server content.toml), not on the wire. */
export const BOILING_POINT_MIN = 8;
export const BOILING_POINT_MAX = 14;

export interface PublicPlayer {
  name: string;
  color: Color;
  score: number;
  contribution: number;
  committedLastWave: boolean;
  lockedOut: boolean;
  isYou: boolean;
}

export interface TurnContext {
  yourHand: HandCard[];
  round: { number: number; activeModifiers: ModifierKind[] } | null;
  wave: { number: number; timerMs?: number } | null;
  potCardCount: number;
  players: PublicPlayer[];
  /** Present only if THIS agent peeked or saw an explosion — never anyone else's. */
  yourDisclosedBoilingPoint?: number;
}

export function buildTurnContext(vm: ViewModel): TurnContext {
  const players: PublicPlayer[] = [];
  for (const p of vm.players.values()) {
    players.push({
      name: p.info.display_name,
      color: p.info.color,
      score: p.score,
      contribution: p.contribution,
      committedLastWave: p.committedLastWave,
      lockedOut: p.lockedOut,
      isYou: p.info.id === vm.self.playerId,
    });
  }

  const ctx: TurnContext = {
    yourHand: vm.self.hand,
    round: vm.round ? { number: vm.round.number, activeModifiers: [...vm.round.activeModifiers] } : null,
    wave: vm.wave ? { number: vm.wave.number, timerMs: vm.wave.timerMs } : null,
    potCardCount: vm.pot.cardCount,
    players,
  };
  if (vm.self.disclosedBoilingPoint !== undefined) {
    ctx.yourDisclosedBoilingPoint = vm.self.disclosedBoilingPoint;
  }
  return ctx;
}

/** Keys that must NEVER appear in the turn context (the revocable / hidden surface). */
export const FORBIDDEN_CONTEXT_KEYS: readonly string[] = [
  "revealHistory",
  "reveals",
  "potCards",
  "potIdentities",
  "deck",
  "remainingDeck",
];

function describeCard(c: HandCard): string {
  const eff = c.view.effect ? ` [${c.view.effect}]` : "";
  return `#${c.id} ${c.view.color} vol${c.view.volatility}/pts${c.view.points}${eff}`;
}

/** Human-readable rendering of the thin context for the agent prompt. */
export function renderTurnContext(ctx: TurnContext): string {
  const lines: string[] = [];
  if (ctx.round) {
    lines.push(`Round ${ctx.round.number}. Boiling point is hidden (normally ${BOILING_POINT_MIN}–${BOILING_POINT_MAX}).`);
    if (ctx.round.activeModifiers.length) {
      lines.push(`Active cauldron modifiers: ${ctx.round.activeModifiers.join(", ")}.`);
    }
  }
  if (ctx.wave) {
    const t = ctx.wave.timerMs ? ` (~${Math.round(ctx.wave.timerMs / 1000)}s)` : "";
    lines.push(`Wave ${ctx.wave.number} is open${t}. Commit one card or pass (passing locks you out of the round).`);
  }
  lines.push(`Pot holds ${ctx.potCardCount} face-down card(s).`);
  if (ctx.yourDisclosedBoilingPoint !== undefined) {
    lines.push(`You know the boiling point this round: ${ctx.yourDisclosedBoilingPoint}.`);
  }
  lines.push("Players:");
  for (const p of ctx.players) {
    const status = p.lockedOut ? "passed/locked-out" : p.committedLastWave ? "played last wave" : "still in";
    lines.push(`  - ${p.name} (${p.color})${p.isYou ? " [you]" : ""}: score ${p.score}, contributed ${p.contribution} this round, ${status}`);
  }
  lines.push("Your hand:");
  for (const c of ctx.yourHand) lines.push(`  - ${describeCard(c)}`);
  return lines.join("\n");
}
