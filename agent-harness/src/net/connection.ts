// WebSocket client that connects exactly like a real client (spec: Claude-Driven
// Protocol Client). It sends an entry message (CreateRoom/JoinRoom/EnqueueMatch) carrying
// the protocol_version and awaits RoomJoined, surfacing an incompatible-version Error
// instead of crashing through. The server accepts ONLY binary MessagePack frames.
// EDGE MODULE: depends on `ws` + the wire codec.

import WebSocket from "ws";
import {
  PROTOCOL_VERSION,
  type ClientMessage,
  type Color,
  type PlayerId,
  type PlayerPublic,
  type RoomCode,
  type ServerMessage,
} from "../protocol/messages.ts";
import { decodeServer, encodeClient, type WireMode } from "../protocol/codec.ts";

export type EntryKind = "join" | "create" | "enqueue";

export interface EntryConfig {
  kind: EntryKind;
  displayName: string;
  roomCode?: RoomCode; // required for "join"
  sessionToken?: string | null;
}

export interface JoinResult {
  roomCode: RoomCode;
  yourPlayerId: PlayerId;
  yourColor: Color;
  players: PlayerPublic[];
}

export class ProtocolVersionError extends Error {}

type MessageHandler = (msg: ServerMessage) => void;

function entryMessage(cfg: EntryConfig): ClientMessage {
  const session_token = cfg.sessionToken ?? null;
  switch (cfg.kind) {
    case "join":
      if (!cfg.roomCode) throw new Error("join entry requires a room code");
      return { type: "JoinRoom", protocol_version: PROTOCOL_VERSION, display_name: cfg.displayName, session_token, room_code: cfg.roomCode };
    case "create":
      return { type: "CreateRoom", protocol_version: PROTOCOL_VERSION, display_name: cfg.displayName, session_token };
    case "enqueue":
      return { type: "EnqueueMatch", protocol_version: PROTOCOL_VERSION, display_name: cfg.displayName, session_token };
  }
}

export class Connection {
  private ws: WebSocket | undefined;
  private handlers: MessageHandler[] = [];
  private closeHandlers: Array<() => void> = [];

  onClose(handler: () => void): void {
    this.closeHandlers.push(handler);
  }

  private readonly url: string;
  private readonly mode: WireMode;

  constructor(url: string, mode: WireMode = "msgpack") {
    this.url = url;
    this.mode = mode;
  }

  onServerMessage(handler: MessageHandler): void {
    this.handlers.push(handler);
  }

  /** Open the socket, send the entry message, and await RoomJoined. */
  connectAndEnter(cfg: EntryConfig): Promise<JoinResult> {
    return new Promise<JoinResult>((resolve, reject) => {
      const ws = new WebSocket(this.url);
      this.ws = ws;
      ws.binaryType = "arraybuffer";

      let joined = false;

      ws.on("open", () => this.send(entryMessage(cfg)));

      ws.on("message", (data: WebSocket.RawData, isBinary: boolean) => {
        let msg: ServerMessage;
        try {
          msg = decodeServer(toBytes(data, isBinary), this.mode);
        } catch (err) {
          reject(new Error(`failed to decode server frame: ${String(err)}`));
          return;
        }

        if (!joined) {
          if (msg.type === "RoomJoined") {
            joined = true;
            resolve({
              roomCode: msg.room_code,
              yourPlayerId: msg.your_player_id,
              yourColor: msg.your_color,
              players: msg.players,
            });
            // fall through so RoomJoined also reaches handlers
          } else if (msg.type === "Error") {
            reject(
              msg.code === "VersionMismatch"
                ? new ProtocolVersionError(msg.message)
                : new Error(`server error before join: ${msg.code} — ${msg.message}`),
            );
            return;
          }
        }

        for (const h of this.handlers) h(msg);
      });

      ws.on("error", (err: Error) => {
        if (!joined) reject(err);
      });
      ws.on("close", () => {
        if (!joined) reject(new Error("connection closed before join"));
        else for (const h of this.closeHandlers) h();
      });
    });
  }

  send(msg: ClientMessage): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    const encoded = encodeClient(msg, this.mode);
    // The server accepts only binary frames; send msgpack bytes as binary.
    this.ws.send(encoded);
  }

  close(): void {
    this.ws?.close();
  }
}

function toBytes(data: WebSocket.RawData, isBinary: boolean): Uint8Array | string {
  if (!isBinary && typeof data === "string") return data;
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (Array.isArray(data)) return Buffer.concat(data);
  return data as Uint8Array;
}
