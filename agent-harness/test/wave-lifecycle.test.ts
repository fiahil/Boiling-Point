import { test } from "node:test";
import assert from "node:assert/strict";

import { WaveLifecycle } from "../src/net/wave-lifecycle.ts";

test("fires open with the timer budget and resolve on the matching events", () => {
  const lc = new WaveLifecycle();
  const opens: Array<{ wave: number; timer: number }> = [];
  const resolves: number[] = [];
  lc.onOpen((wave, timer) => opens.push({ wave, timer }));
  lc.onResolve((wave) => resolves.push(wave));

  lc.handle({ type: "WaveOpened", round_number: 1, wave_number: 1, timer_ms: 30000 });
  lc.handle({ type: "WaveResolved", played: ["me"], passed: [], cauldron_card_count: 1, contributions: [] });
  lc.handle({ type: "WaveOpened", round_number: 1, wave_number: 2, timer_ms: 10000 });

  assert.deepEqual(opens, [
    { wave: 1, timer: 30000 },
    { wave: 2, timer: 10000 },
  ]);
  assert.deepEqual(resolves, [1], "resolve carries the wave that just closed");
  assert.equal(lc.wave, 2);
});

test("ignores unrelated messages", () => {
  const lc = new WaveLifecycle();
  let fired = 0;
  lc.onOpen(() => fired++);
  lc.onResolve(() => fired++);
  lc.handle({ type: "Heartbeat" });
  lc.handle({ type: "ScoreUpdate", scores: [] });
  assert.equal(fired, 0);
});
