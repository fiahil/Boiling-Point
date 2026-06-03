// Wire protocol — a faithful TypeScript mirror of the Rust `protocol/` crate
// (protocol/src/{client,server,vocab,ids}.rs). Newtypes serialize transparently
// (PlayerId→uuid string, CardId/EmoteId→number, GroupCode→string); unit enums serialize
// as their variant name; `rmp_serde::to_vec_named` matches this object shape.
//
// This is now an exact hand-mirror, not the earlier guess. The proper long-term step is a
// feature-gated `ts-rs` derive on the Rust crate feeding `npm run gen:protocol`; that
// remains a documented follow-up (the crate does not derive ts-rs today).

export const PROTOCOL_VERSION = 2 as const;

export type PlayerId = string; // PlayerId(Uuid) — transparent uuid string
export type CardId = number; //   CardId(u32)
export type EmoteId = number; //  EmoteId(u16)
export type GroupCode = string; // GroupCode(String)

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

export type ModifierKind =
  | "Residue"
  | "ThinIce"
  | "DeepCauldron"
  | "BountifulBrew"
  | "DoubleStakes"
  | "Reversal";

export type ErrorCode =
  | "VersionMismatch"
  | "UnknownGroup"
  | "NotYourCard"
  | "WrongPhase"
  | "LockedOut"
  | "InvalidEmote"
  | "Internal";

/** Fully-revealed card attributes (in your own hand, or at the depile). */
export interface CardView {
  color: Color;
  volatility: number; // 1–3
  points: number; //     0–3
  effect: EffectKind | null;
}

export interface HandCard {
  id: CardId;
  view: CardView;
}

export interface PlayerPublic {
  id: PlayerId;
  display_name: string;
  color: Color;
  connected: boolean;
}

export interface PlayerScore {
  player: PlayerId;
  score: number; // i32, may be negative
}

export interface Contribution {
  player: PlayerId;
  count: number;
}

export interface DepileEntry {
  player: PlayerId;
  card: CardView;
  running_volatility: number;
}

export type ScoringOutcome =
  | { kind: "Domination"; winner: Color }
  | { kind: "Split"; colors: Color[] };

// ---------------------------------------------------------------------------
// Client → Server  (#[serde(tag = "type")])
// ---------------------------------------------------------------------------

export type ClientMessage =
  | { type: "JoinGroup"; protocol_version: number; display_name: string; session_token: string | null; group_code: GroupCode }
  | { type: "CreateGroup"; protocol_version: number; display_name: string; session_token: string | null }
  | { type: "EnqueueMatch"; protocol_version: number; display_name: string; session_token: string | null }
  | { type: "CommitCard"; card: CardId }
  | { type: "CommitPass" }
  | { type: "LockIn" }
  | { type: "Emote"; emote: EmoteId }
  | { type: "PlayAgain" }
  | { type: "LeaveGroup" }
  | { type: "Heartbeat" };

// ---------------------------------------------------------------------------
// Server → Client  (#[serde(tag = "type")])
// ---------------------------------------------------------------------------

export type ServerMessage =
  | { type: "GroupJoined"; group_code: GroupCode; your_player_id: PlayerId; your_color: Color; session_token: string; players: PlayerPublic[] }
  | { type: "GameStarting"; players: PlayerPublic[]; round_count: number }
  | { type: "YourHand"; cards: HandCard[] }
  | { type: "WaveOpened"; round_number: number; wave_number: number; timer_ms: number; final_wave: boolean }
  | { type: "WaveResolved"; played: PlayerId[]; passed: PlayerId[]; cauldron_card_count: number; contributions: Contribution[] }
  | { type: "ModifierRevealed"; modifier: ModifierKind; round_number: number }
  | { type: "SomeonePeeked" }
  | { type: "Exposed"; card: CardView }
  | { type: "DeckReshuffled" }
  | { type: "EmoteBroadcast"; from: PlayerId; emote: EmoteId }
  | { type: "PeekResult"; boiling_point: number } // private, secret
  | { type: "Depile"; reveals: DepileEntry[]; exploded: boolean; boiling_point: number | null; crossing_index: number | null }
  | { type: "RoundScored"; color_points: Array<[Color, number]>; outcome: ScoringOutcome; awards: PlayerScore[] }
  | { type: "Explosion"; pot_value: number; deltas: PlayerScore[]; shielded: PlayerId[] }
  | { type: "ScoreUpdate"; scores: PlayerScore[] }
  | { type: "GameOver"; final_scores: PlayerScore[]; winners: PlayerId[] }
  | { type: "DeathmatchStarted"; participants: PlayerId[] }
  | { type: "Error"; code: ErrorCode; message: string }
  | { type: "PlayerConnectionChanged"; player: PlayerId; connected: boolean }
  | { type: "LeftGroup" }
  | {
      type: "StateSnapshot";
      group_code: GroupCode;
      your_player_id: PlayerId;
      round_number: number;
      players: PlayerPublic[];
      scores: PlayerScore[];
      active_modifiers: ModifierKind[];
      contributions: Contribution[];
      your_hand: HandCard[];
    }
  | { type: "Heartbeat" };

export type ServerMessageType = ServerMessage["type"];
export type ClientMessageType = ClientMessage["type"];
