import { test } from "node:test";
import assert from "node:assert/strict";

import { allowedToolsFor, canCountCards } from "../src/agent/difficulty.ts";
import { TOOL } from "../src/agent/tool-names.ts";

test("easy grants action tools but not the card-history capability", () => {
  const allowed = allowedToolsFor("easy");
  assert.ok(allowed.includes(TOOL.commit_card));
  assert.ok(allowed.includes(TOOL.pass));
  assert.ok(allowed.includes(TOOL.send_emote));
  assert.ok(!allowed.includes(TOOL.reveal_history), "easy must NOT be able to count cards");
  assert.equal(canCountCards("easy"), false);
});

test("hard grants the card-history capability on top of actions", () => {
  const allowed = allowedToolsFor("hard");
  assert.ok(allowed.includes(TOOL.commit_card));
  assert.ok(allowed.includes(TOOL.reveal_history), "hard may count cards");
  assert.equal(canCountCards("hard"), true);
});
