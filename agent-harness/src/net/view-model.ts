// The agent's narrow, player-visible view of the game, built solely from received
// ServerMessages. There is deliberately NO field for the boiling point (except the
// guarded disclosure below), opponents' hand contents, or the draw deck — leakage is
// prevented by construction (spec: Player-Visible View Model; design D6).
//
// The reveal history IS held here because it is public (the depile is shown to everyone),
// but it is EXCLUDED from the thin turn context and surfaced only via the reveal_history
// capability tool — that is the difficulty gate (design D3), not a secret boundary.

import type {
  Card,
  Color,
  PlayerId,
  PlayerInfo,
  RevealedCard,
  RoundOutcome,
  ServerMessage,
} from "../protocol/messages.ts";
import { discloseBoilingPoint } from "./secret-boundary.ts";

export interface SelfState {
  playerId: PlayerId;
  color: Color;
  hand: Card[];
  /** Set ONLY through discloseBoilingPoint() — own Peek or an explosion depile. */
  disclosedBoilingPoint?: number;
  boilingPointSource?: "peek" | "explosion";
}

export interface PlayerView {
  info: PlayerInfo;
  score: number;
  /** Cards this player has added to the pot this round. Reset each round. */
  contribution: number;
  /** Committed a card in the current wave. Reset each wave. */
  committedThisWave: boolean;
  /** Passed → locked out for the rest of the round. */
  lockedOut: boolean;
}

export interface RoundView {
  number: number;
  thresholdMin: number;
  thresholdMax: number;
  multiplier: number;
}

export interface WaveView {
  number: number;
  open: boolean;
  timerMs?: number;
  /** Local wall-clock deadline estimate, set by the net layer on WaveOpened. Informational only. */
  deadlineTs?: number;
}

export interface PotView {
  cardCount: number;
  // No field for card identities: the pot is face-down until the depile.
}

export interface RevealRecord {
  round: number;
  reveals: RevealedCard[];
  outcome: RoundOutcome;
  /** The shuffle epoch this round belonged to — counting is only valid within one epoch. */
  epoch: number;
}

export interface ViewModel {
  self: SelfState;
  players: Map<PlayerId, PlayerView>;
  round?: RoundView;
  wave?: WaveView;
  pot: PotView;
  /** Public depile history (gated behind reveal_history, not put in the turn context). */
  revealHistory: RevealRecord[];
  /** Increments on DeckReshuffled; the current card-counting epoch. */
  shuffleEpoch: number;
  gameOver?: { winner: PlayerId; finalScores: Record<PlayerId, number> };
}

export function createViewModel(): ViewModel {
  return {
    self: { playerId: "", color: "Wild", hand: [] },
    players: new Map(),
    pot: { cardCount: 0 },
    revealHistory: [],
    shuffleEpoch: 0,
  };
}

function ensurePlayer(vm: ViewModel, info: PlayerInfo): PlayerView {
  const existing = vm.players.get(info.id);
  if (existing) {
    existing.info = info;
    return existing;
  }
  const view: PlayerView = {
    info,
    score: 0,
    contribution: 0,
    committedThisWave: false,
    lockedOut: false,
  };
  vm.players.set(info.id, view);
  return view;
}

/** Reduce a received ServerMessage into the view model. Returns the model for chaining. */
export function applyServerMessage(vm: ViewModel, msg: ServerMessage): ViewModel {
  switch (msg.type) {
    case "RoomJoined": {
      vm.self.playerId = msg.your_player_id;
      vm.self.color = msg.your_color;
      for (const info of msg.players) ensurePlayer(vm, info);
      break;
    }
    case "YourHand": {
      vm.self.hand = msg.cards;
      break;
    }
    case "PlayerJoined": {
      ensurePlayer(vm, msg.player);
      break;
    }
    case "PlayerLeft": {
      const p = vm.players.get(msg.player_id);
      if (p) p.info.connected = false;
      break;
    }
    case "GameStarting": {
      break;
    }
    case "RoundStarted": {
      vm.round = {
        number: msg.round_number,
        thresholdMin: msg.threshold_min,
        thresholdMax: msg.threshold_max,
        multiplier: msg.multiplier,
      };
      vm.pot.cardCount = 0;
      vm.wave = undefined;
      for (const p of vm.players.values()) {
        p.contribution = 0;
        p.committedThisWave = false;
        p.lockedOut = false;
      }
      break;
    }
    case "WaveOpened": {
      vm.wave = { number: msg.wave_number, open: true, timerMs: msg.timer_ms };
      for (const p of vm.players.values()) p.committedThisWave = false;
      break;
    }
    case "WaveResolved": {
      if (vm.wave) vm.wave.open = false;
      for (const p of vm.players.values()) p.committedThisWave = false;
      for (const id of msg.committed) {
        const p = vm.players.get(id);
        if (p) {
          p.committedThisWave = true;
          p.contribution += 1;
        }
      }
      for (const id of msg.passed) {
        const p = vm.players.get(id);
        if (p) p.lockedOut = true;
      }
      vm.pot.cardCount = msg.pot_card_count;
      break;
    }
    case "PeekResult": {
      discloseBoilingPoint(vm, msg.threshold_value, "peek");
      break;
    }
    case "EffectAnnounced": {
      // Most effects are silent until the depile; Peek arrives privately (PeekResult).
      // Expose's public reveal could aid counting, but v0 does not fold it into the model.
      break;
    }
    case "RoundRevealed": {
      vm.revealHistory.push({
        round: vm.round?.number ?? 0,
        reveals: msg.reveals,
        outcome: msg.outcome,
        epoch: vm.shuffleEpoch,
      });
      break;
    }
    case "Explosion": {
      discloseBoilingPoint(vm, msg.boiling_point, "explosion");
      break;
    }
    case "RoundScored": {
      for (const [id, score] of Object.entries(msg.scores)) {
        const p = vm.players.get(id);
        if (p) p.score = score;
      }
      break;
    }
    case "DeckReshuffled": {
      vm.shuffleEpoch += 1;
      break;
    }
    case "EmoteBroadcast": {
      break; // table-talk has no game-state effect
    }
    case "GameOver": {
      vm.gameOver = { winner: msg.winner, finalScores: msg.final_scores };
      break;
    }
    case "StateSnapshot": {
      const s = msg.snapshot;
      vm.self.hand = s.your_hand;
      vm.pot.cardCount = s.pot_card_count;
      for (const info of s.players) ensurePlayer(vm, info);
      for (const [id, score] of Object.entries(s.scores)) {
        const p = vm.players.get(id);
        if (p) p.score = score;
      }
      break;
    }
    case "Error":
    case "HeartbeatAck": {
      break;
    }
    default: {
      // Exhaustiveness guard — a new variant must be handled explicitly.
      const _never: never = msg;
      void _never;
    }
  }
  return vm;
}

/** Reveal history for the CURRENT shuffle epoch only (card counting resets on reshuffle). */
export function currentEpochReveals(vm: ViewModel): RevealRecord[] {
  return vm.revealHistory.filter((r) => r.epoch === vm.shuffleEpoch);
}
