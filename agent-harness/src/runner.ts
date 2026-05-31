// Orchestrates one agent seat: connect → enter → play to GameOver. Ties together the net
// layer, the view model + secret-boundary assertion, the wave lifecycle (deliberate at the
// previous wave's resolution, commit/lock-in at open, fast fallback on overrun — design
// D4), the Agent SDK session, and persona emotes.
//
// Two "brains": `claude` drives decisions through the Agent SDK; `fallback` plays the local
// heuristic only (no LLM) — a zero-cost seat-filler and the protocol integration harness.

import { Connection, type EntryConfig, type JoinResult } from "./net/connection.ts";
import { applyServerMessage, createViewModel, currentEpochReveals, type ViewModel } from "./net/view-model.ts";
import { assertNoSecretLeak } from "./net/secret-boundary.ts";
import { WaveLifecycle } from "./net/wave-lifecycle.ts";
import type { WireMode } from "./protocol/codec.ts";
import type { EmoteId, PlayerId, PlayerScore, ServerMessage } from "./protocol/messages.ts";

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

export type Brain = "claude" | "fallback";

export interface RunnerConfig {
  url: string;
  entry: EntryConfig;
  difficulty: Difficulty;
  archetype?: Archetype;
  epsilon: number;
  mode: WireMode;
  brain: Brain;
  /** Submit the fallback at this fraction of the wave timer if the agent is not done. */
  fallbackAt: number;
}

const FALLBACK_FRACTION_DEFAULT = 0.8;
/** Spacing after a commit before sending LockIn, to clear the server's 100ms rate limit. */
const LOCK_IN_DELAY_MS = 150;

export interface GameResult {
  winners: PlayerId[];
  finalScores: PlayerScore[];
  /** True if the connection closed before a GameOver was seen. */
  aborted: boolean;
}

export class AgentRunner {
  private readonly vm: ViewModel = createViewModel();
  private readonly conn: Connection;
  private readonly lifecycle = new WaveLifecycle();
  private readonly persona: Persona | undefined;
  private readonly server: ReturnType<typeof createBpToolServer>;

  private pending: Move | null = null;
  private decided = false;
  private deliberating = false;
  private submittedWave = -1;
  private fallbackTimer: ReturnType<typeof setTimeout> | undefined;
  private lastEmoteAt = 0;
  private finished = false;
  private doneResolve!: (r: GameResult) => void;
  private readonly donePromise = new Promise<GameResult>((res) => {
    this.doneResolve = res;
  });

  private readonly cfg: RunnerConfig;

  constructor(cfg: RunnerConfig) {
    this.cfg = cfg;
    this.conn = new Connection(cfg.url, cfg.mode);
    this.persona = getPersona(cfg.archetype);

    const deps: ToolDeps = {
      getViewModel: () => this.vm,
      decideMove: (move) => this.onAgentDecision(move),
      lockIn: () => this.conn.send({ type: "LockIn" }),
      sendEmote: (emote: EmoteId) => this.conn.send({ type: "Emote", emote }),
      revealHistory: () => currentEpochReveals(this.vm),
    };
    this.server = createBpToolServer(deps);

    this.lifecycle.onResolve(() => this.onWaveResolve());
    this.lifecycle.onOpen((_wave, timerMs) => this.onWaveOpen(timerMs));
    this.conn.onClose(() => this.finish({ winners: [], finalScores: [], aborted: true }));
  }

  async start(): Promise<JoinResult> {
    this.conn.onServerMessage((msg) => this.onMessage(msg));
    return this.conn.connectAndEnter(this.cfg.entry);
  }

  /** Resolves when the game ends (GameOver) or the connection closes (aborted). */
  whenDone(): Promise<GameResult> {
    return this.donePromise;
  }

  // --- message handling -----------------------------------------------------

  private onMessage(msg: ServerMessage): void {
    applyServerMessage(this.vm, msg);
    assertNoSecretLeak(this.vm); // continuous secret-boundary check (spec)
    this.lifecycle.handle(msg);
    this.reactWithEmote(msg);
    if (msg.type === "GameOver") {
      this.finish({ winners: msg.winners, finalScores: msg.final_scores, aborted: false });
    }
  }

  private finish(result: GameResult): void {
    if (this.finished) return;
    this.finished = true;
    this.stop();
    this.doneResolve(result);
  }

  // --- timeliness: deliberate at resolve, commit at open, fallback on overrun

  private onWaveResolve(): void {
    // The public state is final at resolve (D4) — start thinking about the next wave now.
    if (this.cfg.brain === "claude") this.startDeliberation();
  }

  private startDeliberation(): void {
    if (this.deliberating || this.vm.gameOver || this.isLockedOut()) return;
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
    if (this.isLockedOut()) return;

    if (this.cfg.brain === "fallback") {
      // No LLM: play the (persona-biased) heuristic promptly so the table flows.
      clearTimeout(this.fallbackTimer);
      this.fallbackTimer = setTimeout(() => this.submit(this.personaBiasedFallback()), 100);
      return;
    }

    // claude brain
    if (!this.deliberating && this.pending === null && !this.decided) this.startDeliberation();
    if (this.pending) {
      this.submit(this.pending);
      return;
    }
    const at = clamp01(this.cfg.fallbackAt || FALLBACK_FRACTION_DEFAULT);
    clearTimeout(this.fallbackTimer);
    this.fallbackTimer = setTimeout(() => {
      if (this.submittedWave !== this.vm.wave?.number) {
        if (process.env.BP_DEBUG) console.error(`[fallback] agent late — playing heuristic (wave ${this.vm.wave?.number})`);
        this.submit(this.personaBiasedFallback());
      }
    }, Math.max(0, Math.floor(timerMs * at)));
  }

  /** Called by the move tool when the agent commits/passes. */
  private onAgentDecision(move: Move): void {
    this.decided = true;
    if (process.env.BP_DEBUG) console.error(`[agent] chose ${describe(move)} (wave ${this.vm.wave?.number})`);
    if (this.isWaveOpen()) this.submit(move);
    else this.pending = move; // decided in the inter-wave gap — commit when the wave opens
  }

  private submit(move: Move): void {
    const wave = this.vm.wave?.number ?? -1;
    if (this.submittedWave === wave || !this.isWaveOpen() || this.isLockedOut()) return;
    this.submittedWave = wave;
    clearTimeout(this.fallbackTimer);
    this.pending = null;

    const { move: final } = maybeBlunder(move, this.vm, this.cfg.epsilon);
    this.conn.send(moveToClientMessage(final));
    // Lock in early to close the wave (D4), but space it past the 100ms rate limit.
    setTimeout(() => this.conn.send({ type: "LockIn" }), LOCK_IN_DELAY_MS);
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
    const emote = emoteFor(this.persona.archetype, situation);
    if (emote === undefined) return;
    const now = Date.now();
    if (now - this.lastEmoteAt < 1500) return; // don't spam / collide with the rate limit
    this.lastEmoteAt = now;
    this.conn.send({ type: "Emote", emote });
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
      return msg.cauldron_card_count >= Math.max(3, vm.players.size) ? "big_pot" : "rival_committed";
    case "Explosion":
      return "after_explosion";
    case "GameOver":
      return msg.winners.includes(vm.self.playerId) ? "after_win" : undefined;
    default:
      return undefined;
  }
}

function clamp01(x: number): number {
  if (!Number.isFinite(x) || x < 0) return 0;
  if (x > 1) return 1;
  return x;
}

function describe(move: Move): string {
  return move.kind === "commit" ? `commit #${move.cardId}` : "pass";
}
