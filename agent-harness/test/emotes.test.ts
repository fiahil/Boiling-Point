import { test } from "node:test";
import assert from "node:assert/strict";

import { EMOTE_PALETTE, emoteFor, isPaletteEmote, type Situation } from "../src/personas/emotes.ts";
import { ARCHETYPES_LIST } from "../src/personas/archetypes.ts";

const SITUATIONS: Situation[] = ["wave_open", "big_pot", "after_explosion", "after_win", "rival_committed"];

test("every persona emote is a palette id (never free text)", () => {
  for (const archetype of ARCHETYPES_LIST) {
    for (const situation of SITUATIONS) {
      const id = emoteFor(archetype, situation);
      if (id !== undefined) {
        assert.ok(isPaletteEmote(id), `${archetype}/${situation} produced non-palette '${id}'`);
        assert.ok(EMOTE_PALETTE.includes(id));
      }
    }
  }
});

test("unknown emote ids are rejected", () => {
  assert.equal(isPaletteEmote(999), false);
  assert.equal(isPaletteEmote(0), false);
});
