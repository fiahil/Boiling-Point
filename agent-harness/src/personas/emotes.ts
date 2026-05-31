// Persona-driven emote selection (spec: Persona-Driven Emote Selection; design D5).
// Personas express ONLY through the server's preset-emote palette — table-talk has no
// free text. This maps an archetype + game situation to a numeric EmoteId (or to silence).
//
// Palette mirrors server/content.toml (the only comms channel):
//   1 truce · 2 scheming · 3 fear · 4 taunt · 5 watching · 6 youre_done

import type { EmoteId } from "../protocol/messages.ts";
import type { Archetype } from "./archetypes.ts";

export const EMOTE = {
  truce: 1,
  scheming: 2,
  fear: 3,
  taunt: 4,
  watching: 5,
  youre_done: 6,
} as const;

export const EMOTE_PALETTE: readonly EmoteId[] = Object.values(EMOTE);

export type Situation =
  | "wave_open"
  | "big_pot"
  | "after_explosion"
  | "after_win"
  | "rival_committed";

export function isPaletteEmote(id: EmoteId): boolean {
  return EMOTE_PALETTE.includes(id);
}

const MAP: Record<Archetype, Partial<Record<Situation, EmoteId>>> = {
  gambler: {
    wave_open: EMOTE.taunt,
    big_pot: EMOTE.youre_done,
    after_explosion: EMOTE.taunt,
    after_win: EMOTE.youre_done,
    rival_committed: EMOTE.watching,
  },
  turtle: {
    wave_open: EMOTE.truce,
    big_pot: EMOTE.fear,
    after_explosion: EMOTE.fear,
    rival_committed: EMOTE.watching,
  },
  bandwagoner: {
    rival_committed: EMOTE.truce,
    big_pot: EMOTE.watching,
    after_win: EMOTE.truce,
  },
  trickster: {
    wave_open: EMOTE.scheming,
    big_pot: EMOTE.watching,
    after_explosion: EMOTE.taunt,
    rival_committed: EMOTE.scheming,
  },
};

/**
 * The emote a persona would send in a situation, or undefined to stay silent.
 * Guaranteed to be a palette id (or undefined) — never free text.
 */
export function emoteFor(archetype: Archetype, situation: Situation): EmoteId | undefined {
  const row = MAP[archetype];
  const id = row ? row[situation] : undefined;
  if (id === undefined) return undefined;
  return isPaletteEmote(id) ? id : undefined;
}
