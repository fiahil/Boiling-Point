// Persona archetypes (spec: Persona Shapes Playstyle Bias; design D5).
// A persona is an OPTIONAL layer on top of a difficulty preset — the two are independent
// axes. It biases playstyle via a system-prompt fragment (how Claude is told to play) and,
// secondarily, via biasFallback (how the local fallback leans when Claude is late). It
// grants NO capability. With no persona, the agent plays straight.

import type { Card } from "../protocol/messages.ts";
import type { Move } from "../agent/actions.ts";
import type { ViewModel } from "../net/view-model.ts";

export type Archetype = "gambler" | "turtle" | "bandwagoner" | "trickster";

export const ARCHETYPES_LIST: readonly Archetype[] = [
  "gambler",
  "turtle",
  "bandwagoner",
  "trickster",
];

export function isArchetype(value: string): value is Archetype {
  return (ARCHETYPES_LIST as readonly string[]).includes(value);
}

export interface Persona {
  archetype: Archetype;
  /** Appended to the agent system prompt to bias playstyle. */
  systemPromptFragment: string;
  /** Optional lean for the local fallback move; undefined means "no opinion". */
  biasFallback?: (vm: ViewModel) => Move | undefined;
}

const byHighestVolatility = (hand: Card[]): Card | undefined =>
  hand.length === 0 ? undefined : [...hand].sort((a, b) => b.volatility - a.volatility || b.points - a.points)[0];

const byLowestVolatility = (hand: Card[]): Card | undefined =>
  hand.length === 0 ? undefined : [...hand].sort((a, b) => a.volatility - b.volatility || a.points - b.points)[0];

const firstOfColor = (hand: Card[], color: string): Card | undefined =>
  hand.find((c) => c.color === color);

const firstDecoy = (hand: Card[], ownColor: string): Card | undefined =>
  hand.find((c) => c.color !== ownColor && c.color !== "Wild");

export const ARCHETYPES: Record<Archetype, Persona> = {
  gambler: {
    archetype: "gambler",
    systemPromptFragment:
      "You are The Gambler: bold and impatient. You love big pots and pushing your luck. " +
      "Lean toward committing high-volatility cards and staying in rather than passing.",
    biasFallback: (vm) => {
      const c = byHighestVolatility(vm.self.hand);
      return c ? { kind: "commit", cardId: c.id } : undefined;
    },
  },
  turtle: {
    archetype: "turtle",
    systemPromptFragment:
      "You are The Turtle: cautious and survival-minded. You fear explosions and protect your score. " +
      "Lean toward low-volatility cards, and pass when the pot is getting dangerous.",
    biasFallback: (vm) => {
      if (vm.pot.cardCount > 0 && (vm.wave?.number ?? 1) > 1) return { kind: "pass" };
      const c = byLowestVolatility(vm.self.hand);
      return c ? { kind: "commit", cardId: c.id } : { kind: "pass" };
    },
  },
  bandwagoner: {
    archetype: "bandwagoner",
    systemPromptFragment:
      "You are The Bandwagoner: you hate being left out and like to pile onto whatever is happening. " +
      "Lean toward staying in every wave; favor playing your own color to build presence.",
    biasFallback: (vm) => {
      const own = firstOfColor(vm.self.hand, vm.self.color);
      const card = own ?? vm.self.hand[0];
      return card ? { kind: "commit", cardId: card.id } : undefined;
    },
  },
  trickster: {
    archetype: "trickster",
    systemPromptFragment:
      "You are The Trickster: you play mind games. Early in a round you sow misdirection by playing " +
      "other players' colors as decoys; late you reveal your real intent with your own color.",
    biasFallback: (vm) => {
      const early = (vm.wave?.number ?? 1) <= 1;
      const card = early
        ? firstDecoy(vm.self.hand, vm.self.color) ?? vm.self.hand[0]
        : firstOfColor(vm.self.hand, vm.self.color) ?? vm.self.hand[0];
      return card ? { kind: "commit", cardId: card.id } : undefined;
    },
  },
};

export function getPersona(archetype?: Archetype): Persona | undefined {
  return archetype ? ARCHETYPES[archetype] : undefined;
}

/** System-prompt fragment for an optional persona; empty string for straight play. */
export function personaPrompt(archetype?: Archetype): string {
  return archetype ? ARCHETYPES[archetype].systemPromptFragment : "";
}
