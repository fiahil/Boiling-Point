// Tracks the wave cycle so the runner can implement "deliberate in the gap, commit at
// open" (design D4): in simultaneous waves everything needed for wave N+1 is final the
// moment wave N resolves, so deliberation starts on RESOLVE and the chosen action is
// committed/locked-in on the next OPEN. This module only routes events; the runner owns
// the timers and the LLM call (kept here so the routing is pure and unit-testable).

import type { ServerMessage } from "../protocol/messages.ts";

export type OpenHandler = (waveNumber: number, timerMs: number) => void;
export type ResolveHandler = (waveNumber: number) => void;

export class WaveLifecycle {
  private openHandlers: OpenHandler[] = [];
  private resolveHandlers: ResolveHandler[] = [];
  private currentWave = 0;

  onOpen(handler: OpenHandler): void {
    this.openHandlers.push(handler);
  }

  onResolve(handler: ResolveHandler): void {
    this.resolveHandlers.push(handler);
  }

  get wave(): number {
    return this.currentWave;
  }

  /** Feed every received ServerMessage; fires handlers on wave open/resolve. */
  handle(msg: ServerMessage): void {
    if (msg.type === "WaveOpened") {
      this.currentWave = msg.wave_number;
      for (const h of this.openHandlers) h(msg.wave_number, msg.timer_ms);
    } else if (msg.type === "WaveResolved") {
      const resolved = this.currentWave;
      for (const h of this.resolveHandlers) h(resolved);
    }
  }
}
