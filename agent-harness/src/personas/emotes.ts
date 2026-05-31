// Persona-driven emote selection (spec: Persona-Driven Emote Selection; design D5).
// Personas express ONLY through the server's preset-emote palette — table-talk has no
// free text. This module maps an archetype + game situation to a palette id (or to
// silence). It can never produce free text, which the server would reject anyway.
//
// PROVISIONAL palette: the real emote ids come from server-release-1's content config.
// Replace EMOTE_PALETTE with the configured ids when that lands.

import type { Archetype } from "./archetypes.ts";

export const EMOTE_PALETTE: readonly string[] = [
  "taunt",
  "sweat",
  "laugh",
  "think",
  "cheer",
  "skull",
  "shrug",
  "eyes",
];

export type Situation =
  | "wave_open"
  | "big_pot"
  | "after_explosion"
  | "after_win"
  | "rival_committed";

export function isPaletteEmote(id: string): boolean {
  return EMOTE_PALETTE.includes(id);
}

const MAP: Record<Archetype, Partial<Record<Situation, string>>> = {
  gambler: {
    wave_open: "taunt",
    big_pot: "cheer",
    after_explosion: "laugh",
    after_win: "cheer",
    rival_committed: "eyes",
  },
  turtle: {
    big_pot: "sweat",
    after_explosion: "skull",
    rival_committed: "think",
  },
  bandwagoner: {
    rival_committed: "cheer",
    big_pot: "eyes",
    after_win: "cheer",
  },
  trickster: {
    wave_open: "shrug",
    big_pot: "eyes",
    after_explosion: "laugh",
    rival_committed: "think",
  },
};

/**
 * The emote a persona would send in a situation, or undefined to stay silent.
 * Guaranteed to be a palette id (or undefined) — never free text.
 */
export function emoteFor(archetype: Archetype, situation: Situation): string | undefined {
  const row = MAP[archetype];
  const id = row ? row[situation] : undefined;
  if (id === undefined) return undefined;
  return isPaletteEmote(id) ? id : undefined;
}
