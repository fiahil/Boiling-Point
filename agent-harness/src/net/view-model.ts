// The agent's narrow, player-visible view of the game, built solely from received
// ServerMessages. There is deliberately NO field for the boiling point (except the
// guarded disclosure below), opponents' hand contents, or face-down cauldron identities —
// leakage is prevented by construction (spec: Player-Visible View Model; design D6).
//
// The depile history IS held here because it is public, but it is EXCLUDED from the thin
// turn context and surfaced only via the reveal_history capability tool — the difficulty
// gate (design D3), not a secret boundary.

import type {
  Color,
  DepileEntry,
  HandCard,
  ModifierKind,
  PlayerId,
  PlayerPublic,
  ServerMessage,
} from "../protocol/messages.ts";
import { discloseBoilingPoint } from "./secret-boundary.ts";

export interface SelfState {
  playerId: PlayerId;
  color: Color;
  hand: HandCard[];
  /** Set ONLY through discloseBoilingPoint() — own Peek or an exploded depile. */
  disclosedBoilingPoint?: number;
  boilingPointSource?: "peek" | "explosion";
}

export interface PlayerView {
  info: PlayerPublic;
  score: number;
  /** Cards contributed to the pot this round (from WaveResolved contributions). */
  contribution: number;
  /** Committed a card in the wave that just resolved. */
  committedLastWave: boolean;
  /** Passed → locked out for the rest of the round. */
  lockedOut: boolean;
}

export interface RoundView {
  number: number;
  /** Modifiers active this game so far (cumulative; ThinIce/DeepCauldron shift the boiling point). */
  activeModifiers: ModifierKind[];
}

export interface WaveView {
  number: number;
  open: boolean;
  timerMs?: number;
}

export interface PotView {
  cardCount: number;
  // No field for card identities: the pot is face-down until the depile.
}

export interface RevealRecord {
  round: number;
  reveals: DepileEntry[];
  exploded: boolean;
  /** Disclosed only when the round exploded. */
  boilingPoint?: number;
  epoch: number;
}

export interface ViewModel {
  self: SelfState;
  players: Map<PlayerId, PlayerView>;
  round?: RoundView;
  wave?: WaveView;
  pot: PotView;
  roundCount?: number;
  /** Public depile history (gated behind reveal_history, not put in the turn context). */
  revealHistory: RevealRecord[];
  /** Increments on DeckReshuffled; the current card-counting epoch. */
  shuffleEpoch: number;
  gameOver?: { winners: PlayerId[]; finalScores: Array<{ player: PlayerId; score: number }> };
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

function ensurePlayer(vm: ViewModel, info: PlayerPublic): PlayerView {
  const existing = vm.players.get(info.id);
  if (existing) {
    existing.info = info;
    return existing;
  }
  const view: PlayerView = {
    info,
    score: 0,
    contribution: 0,
    committedLastWave: false,
    lockedOut: false,
  };
  vm.players.set(info.id, view);
  return view;
}

function activeModifiers(vm: ViewModel): ModifierKind[] {
  if (!vm.round) vm.round = { number: 0, activeModifiers: [] };
  return vm.round.activeModifiers;
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
    case "GameStarting": {
      vm.roundCount = msg.round_count;
      for (const info of msg.players) ensurePlayer(vm, info);
      break;
    }
    case "YourHand": {
      vm.self.hand = msg.cards;
      break;
    }
    case "WaveOpened": {
      const newRound = msg.wave_number === 1;
      if (!vm.round) vm.round = { number: msg.round_number, activeModifiers: [] };
      vm.round.number = msg.round_number;
      vm.wave = { number: msg.wave_number, open: true, timerMs: msg.timer_ms };
      if (newRound) {
        vm.pot.cardCount = 0;
        for (const p of vm.players.values()) {
          p.contribution = 0;
          p.committedLastWave = false;
          p.lockedOut = false;
        }
      }
      break;
    }
    case "WaveResolved": {
      if (vm.wave) vm.wave.open = false;
      const playedSet = new Set(msg.played);
      const passedSet = new Set(msg.passed);
      for (const p of vm.players.values()) {
        p.committedLastWave = playedSet.has(p.info.id);
        if (passedSet.has(p.info.id)) p.lockedOut = true;
      }
      for (const c of msg.contributions) {
        const p = vm.players.get(c.player);
        if (p) p.contribution = c.count;
      }
      vm.pot.cardCount = msg.cauldron_card_count;
      break;
    }
    case "ModifierRevealed": {
      activeModifiers(vm).push(msg.modifier);
      break;
    }
    case "SomeonePeeked":
    case "Exposed": {
      // Public effect signals. Exposed reveals a single pot card to all, but v0 does not
      // fold it into the model (it is not part of the depile history / card count).
      break;
    }
    case "PeekResult": {
      discloseBoilingPoint(vm, msg.boiling_point, "peek");
      break;
    }
    case "Depile": {
      if (msg.exploded && msg.boiling_point != null) {
        discloseBoilingPoint(vm, msg.boiling_point, "explosion");
      }
      vm.revealHistory.push({
        round: vm.round?.number ?? 0,
        reveals: msg.reveals,
        exploded: msg.exploded,
        ...(msg.boiling_point != null ? { boilingPoint: msg.boiling_point } : {}),
        epoch: vm.shuffleEpoch,
      });
      break;
    }
    case "RoundScored": {
      for (const a of msg.awards) {
        const p = vm.players.get(a.player);
        if (p) p.score = a.score;
      }
      break;
    }
    case "Explosion": {
      // The boiling point arrives via Depile, not here; this carries the score deltas.
      break;
    }
    case "ScoreUpdate": {
      for (const s of msg.scores) {
        const p = vm.players.get(s.player);
        if (p) p.score = s.score;
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
    case "PlayerConnectionChanged": {
      const p = vm.players.get(msg.player);
      if (p) p.info.connected = msg.connected;
      break;
    }
    case "GameOver": {
      vm.gameOver = { winners: msg.winners, finalScores: msg.final_scores };
      break;
    }
    case "StateSnapshot": {
      vm.self.playerId = msg.your_player_id;
      vm.self.hand = msg.your_hand;
      if (!vm.round) vm.round = { number: msg.round_number, activeModifiers: [] };
      vm.round.number = msg.round_number;
      vm.round.activeModifiers = [...msg.active_modifiers];
      for (const info of msg.players) ensurePlayer(vm, info);
      for (const s of msg.scores) {
        const p = vm.players.get(s.player);
        if (p) p.score = s.score;
      }
      for (const c of msg.contributions) {
        const p = vm.players.get(c.player);
        if (p) p.contribution = c.count;
      }
      break;
    }
    case "Error":
    case "Heartbeat": {
      break;
    }
    default: {
      const _never: never = msg;
      void _never;
    }
  }
  return vm;
}

/** Reveal history for the CURRENT shuffle epoch only (counting resets on reshuffle). */
export function currentEpochReveals(vm: ViewModel): RevealRecord[] {
  return vm.revealHistory.filter((r) => r.epoch === vm.shuffleEpoch);
}
