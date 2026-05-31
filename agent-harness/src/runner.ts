// Orchestrates one agent seat: connect → join → play to GameOver. Ties together the net
// layer, the view model + secret-boundary assertion, the wave lifecycle (deliberate at the
// previous wave's resolution, commit/lock-in at open, fast fallback on overrun — design
// D4), the Agent SDK session, and persona emotes. EDGE MODULE: the live-server timing and
// SDK control flow here are PROVISIONAL and verified once server-release-1 is committed.

import { Connection } from "./net/connection.ts";
import { applyServerMessage, createViewModel, currentEpochReveals, type ViewModel } from "./net/view-model.ts";
import { assertNoSecretLeak } from "./net/secret-boundary.ts";
import { WaveLifecycle } from "./net/wave-lifecycle.ts";
import type { WireMode } from "./protocol/codec.ts";
import type { CardId, ServerMessage } from "./protocol/messages.ts";

import { type Move, moveToClientMessage } from "./agent/actions.ts";
import { allowedToolsFor, modelFor, type Difficulty } from "./agent/difficulty.ts";
import { buildTurnContext, renderTurnContext } from "./agent/context.ts";
import { buildSystemPrompt } from "./agent/prompt.ts";
import { createBpToolServer, type ToolDeps } from "./agent/tools.ts";
import { runAgentTurn } from "./agent/session.ts";

import { fallbackMove } from "./timeliness/fallback.ts";
import { maybeBlunder } from "./personas/blunder.ts";
import { getPersona, type Archetype, type Persona } from "./personas/archetypes.ts";
import { emoteFor, type Situation } from "./personas/emotes.ts";

export interface RunnerConfig {
  url: string;
  room: string;
  name: string;
  difficulty: Difficulty;
  archetype?: Archetype;
  epsilon: number;
  mode: WireMode;
  /** Submit the fallback at this fraction of the wave timer if the agent is not done. */
  fallbackAt: number;
}

const FALLBACK_FRACTION_DEFAULT = 0.8;

export class AgentRunner {
  private readonly vm: ViewModel = createViewModel();
  private readonly conn: Connection;
  private readonly lifecycle = new WaveLifecycle();
  private readonly persona: Persona | undefined;
  private readonly server: ReturnType<typeof createBpToolServer>;

  private pending: Move | null = null;
  private decided = false;
  private deliberating = false;
  private fallbackTimer: ReturnType<typeof setTimeout> | undefined;
  private lastEmoteAt = 0;

  constructor(private readonly cfg: RunnerConfig) {
    this.conn = new Connection(cfg.url, cfg.mode);
    this.persona = getPersona(cfg.archetype);

    const deps: ToolDeps = {
      getViewModel: () => this.vm,
      decideMove: (move) => this.onAgentDecision(move),
      lockIn: () => this.conn.send({ type: "LockIn" }),
      pickTarget: (cardId: CardId) => this.conn.send({ type: "PickTarget", card_id: cardId }),
      sendEmote: (id) => this.conn.send({ type: "SendEmote", emote_id: id }),
      revealHistory: () => currentEpochReveals(this.vm),
    };
    this.server = createBpToolServer(deps);

    this.lifecycle.onResolve(() => this.startDeliberation());
    this.lifecycle.onOpen((_wave, timerMs) => this.onWaveOpen(timerMs));
  }

  async start(): Promise<void> {
    this.conn.onServerMessage((msg) => this.onMessage(msg));
    await this.conn.connectAndJoin(this.cfg.room, this.cfg.name);
  }

  // --- message handling -----------------------------------------------------

  private onMessage(msg: ServerMessage): void {
    applyServerMessage(this.vm, msg);
    assertNoSecretLeak(this.vm); // continuous secret-boundary check (spec)
    this.lifecycle.handle(msg);
    this.reactWithEmote(msg);
    if (msg.type === "GameOver") this.stop();
  }

  // --- timeliness: deliberate at resolve, commit at open, fallback on overrun

  /** Begin thinking about the upcoming wave (the public state is final at resolve, D4). */
  private startDeliberation(): void {
    if (this.deliberating || this.vm.gameOver) return;
    if (this.isLockedOut()) return;
    this.deliberating = true;
    this.decided = false;
    this.pending = null;

    const prompt = renderTurnContext(buildTurnContext(this.vm));
    const systemPrompt = buildSystemPrompt(this.cfg.difficulty, this.cfg.archetype);

    void runAgentTurn({
      server: this.server,
      allowedTools: allowedToolsFor(this.cfg.difficulty),
      systemPrompt,
      model: modelFor(this.cfg.difficulty),
      prompt,
      isDecided: () => this.decided,
    })
      .catch((err) => console.error("[agent] turn failed:", err))
      .finally(() => {
        this.deliberating = false;
      });
  }

  private onWaveOpen(timerMs: number): void {
    // If we have not started thinking yet (e.g. the first wave of the game), start now.
    if (!this.deliberating && this.pending === null && !this.decided) this.startDeliberation();

    // If the agent already decided during the gap, commit immediately.
    if (this.pending) {
      this.submit(this.pending);
      return;
    }

    // Otherwise arm the fast local fallback so the wave never stalls on a slow agent.
    const at = clamp01(this.cfg.fallbackAt || FALLBACK_FRACTION_DEFAULT);
    clearTimeout(this.fallbackTimer);
    this.fallbackTimer = setTimeout(() => {
      if (this.pending === null) this.submit(this.personaBiasedFallback());
    }, Math.max(0, Math.floor(timerMs * at)));
  }

  /** Called by the move tool when the agent commits/passes. */
  private onAgentDecision(move: Move): void {
    this.decided = true;
    if (this.isWaveOpen()) {
      this.submit(move);
    } else {
      // Decided during the inter-wave gap — hold and commit when the wave opens.
      this.pending = move;
    }
  }

  private submit(move: Move): void {
    clearTimeout(this.fallbackTimer);
    this.pending = null;
    const { move: final } = maybeBlunder(move, this.vm, this.cfg.epsilon);
    this.conn.send(moveToClientMessage(final));
    this.conn.send({ type: "LockIn" }); // lock in early so the table keeps moving (D4)
  }

  private personaBiasedFallback(): Move {
    const biased = this.persona?.biasFallback?.(this.vm);
    return biased ?? fallbackMove(this.vm);
  }

  // --- persona emotes -------------------------------------------------------

  private reactWithEmote(msg: ServerMessage): void {
    if (!this.persona) return;
    const situation = situationFor(msg, this.vm);
    if (!situation) return;
    const id = emoteFor(this.persona.archetype, situation);
    if (!id) return;
    // Respect the server's 100ms rate limit with margin; don't spam.
    const now = Date.now();
    if (now - this.lastEmoteAt < 1500) return;
    this.lastEmoteAt = now;
    this.conn.send({ type: "SendEmote", emote_id: id });
  }

  // --- helpers --------------------------------------------------------------

  private isWaveOpen(): boolean {
    return this.vm.wave?.open === true;
  }

  private isLockedOut(): boolean {
    return this.vm.players.get(this.vm.self.playerId)?.lockedOut === true;
  }

  private stop(): void {
    clearTimeout(this.fallbackTimer);
    this.conn.close();
  }
}

function situationFor(msg: ServerMessage, vm: ViewModel): Situation | undefined {
  switch (msg.type) {
    case "WaveOpened":
      return "wave_open";
    case "WaveResolved":
      return msg.pot_card_count >= Math.max(3, vm.players.size) ? "big_pot" : "rival_committed";
    case "Explosion":
      return "after_explosion";
    case "GameOver":
      return msg.winner === vm.self.playerId ? "after_win" : undefined;
    default:
      return undefined;
  }
}

function clamp01(x: number): number {
  if (!Number.isFinite(x) || x < 0) return 0;
  if (x > 1) return 1;
  return x;
}
