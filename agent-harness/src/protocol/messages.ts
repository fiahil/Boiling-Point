// PROVISIONAL protocol surface — REGENERATE FROM THE RUST `protocol/` CRATE VIA ts-rs.
//
// `server-release-1` (which owns `protocol/`) is not yet committed, so these types are
// hand-authored from the documented message catalog: server-architecture.md §4, the
// server-release-1 `wire-protocol` / `round-engine` specs, and game-design-v2. They are
// aligned to the v2 design (simultaneous hidden single-card waves, blind volatility,
// shared-loss explosions, winner-takes-all by total color points) — NOT the v1 brainstorm
// (sequential turns, rumble/glow). Variant names use the server's `#[serde(tag="type")]`
// discriminant. When `protocol/` lands, replace this file with the ts-rs output; the rest
// of the harness depends only on these shapes.

export const PROTOCOL_VERSION = 1 as const;

export type PlayerId = string;
export type CardId = number;
export type RoomCode = string;

export type Color = "Ruby" | "Sapphire" | "Emerald" | "Amethyst" | "Wild";

export type EffectKind =
  | "Peek"
  | "Dampen"
  | "VolatileSurge"
  | "Shield"
  | "Expose"
  | "Copycat"
  | "Recall"
  | "DoubleDown";

/** A card as the owning player sees it (in their own hand) or as revealed in a depile. */
export interface Card {
  id: CardId;
  color: Color;
  volatility: 1 | 2 | 3;
  points: 0 | 1 | 2 | 3;
  effect?: EffectKind;
}

export interface PlayerInfo {
  id: PlayerId;
  name: string;
  color: Color;
  connected: boolean;
}

export type RoundOutcome =
  | { kind: "Domination"; winner: PlayerId }
  | { kind: "Alliance"; winners: PlayerId[] }
  | { kind: "Commune"; winners: PlayerId[] }
  | { kind: "Explosion" };

/** One card peeled during the reverse-order depile (public information). */
export interface RevealedCard {
  player_id: PlayerId;
  card: Card;
}

// ---------------------------------------------------------------------------
// Client → Server
// ---------------------------------------------------------------------------

export type ClientMessage =
  | { type: "JoinRoom"; room_code: RoomCode; player_name: string; protocol_version: number }
  | { type: "LeaveRoom" }
  /** Tentatively commit one card to the open wave; changeable until the wave closes. */
  | { type: "CommitCard"; card_id: CardId }
  /** Commit to passing this wave (locks you out of the round). */
  | { type: "Pass" }
  /** Finalize the current selection; the server closes the wave early when all active players lock in. */
  | { type: "LockIn" }
  /** Resolve a targeted effect (e.g. Recall): choose one of your own cards in the pot. */
  | { type: "PickTarget"; card_id: CardId }
  /** Send a preset emote from the configured palette (the only comms channel). */
  | { type: "SendEmote"; emote_id: string }
  | { type: "Heartbeat" };

// ---------------------------------------------------------------------------
// Server → Client  (audience noted; PRIVATE reaches only this player)
// ---------------------------------------------------------------------------

export type ServerMessage =
  // PRIVATE
  | { type: "RoomJoined"; room_id: string; players: PlayerInfo[]; your_player_id: PlayerId; your_color: Color }
  | { type: "YourHand"; cards: Card[] }
  | { type: "PeekResult"; threshold_value: number } // discloses the boiling point to THIS player only
  | { type: "Error"; code: string; message: string }
  | { type: "HeartbeatAck" }
  // BROADCAST (public only — never face-down identities, never the boiling point)
  | { type: "PlayerJoined"; player: PlayerInfo }
  | { type: "PlayerLeft"; player_id: PlayerId }
  | { type: "GameStarting"; round_count: number; player_order: PlayerId[] }
  | { type: "RoundStarted"; round_number: number; threshold_min: number; threshold_max: number; multiplier: number }
  | { type: "WaveOpened"; wave_number: number; timer_ms: number }
  /** Wave resolved: who committed / who passed, and the running pot card count. No identities. */
  | { type: "WaveResolved"; committed: PlayerId[]; passed: PlayerId[]; pot_card_count: number }
  /** An effect fired. Most effects are silent until the depile; Peek announces anonymously, Expose reveals publicly. */
  | { type: "EffectAnnounced"; effect: EffectKind; revealed?: RevealedCard }
  /** Reverse-order depile every round (last-added first). Full identities — public. */
  | { type: "RoundRevealed"; reveals: RevealedCard[]; outcome: RoundOutcome }
  /** Only ever sent on an explosion — the boiling point is revealed here and nowhere else publicly. */
  | { type: "Explosion"; boiling_point: number; total_volatility: number; crossing_player: PlayerId }
  | { type: "RoundScored"; scores: Record<PlayerId, number>; deltas: Record<PlayerId, number> }
  /** Draw deck reshuffled from discard — card counting resets here (a new shuffle epoch). */
  | { type: "DeckReshuffled" }
  | { type: "EmoteBroadcast"; player_id: PlayerId; emote_id: string }
  | { type: "GameOver"; final_scores: Record<PlayerId, number>; winner: PlayerId }
  | { type: "StateSnapshot"; snapshot: StateSnapshot };

/** Reconnection payload — scoped to what the player may know. */
export interface StateSnapshot {
  round_number: number;
  wave_number: number;
  your_hand: Card[];
  scores: Record<PlayerId, number>;
  pot_card_count: number;
  players: PlayerInfo[];
}

export type ServerMessageType = ServerMessage["type"];
export type ClientMessageType = ClientMessage["type"];
