// WebSocket client that connects exactly like a real client (spec: Claude-Driven
// Protocol Client). It performs the JoinRoom / protocol_version handshake and surfaces an
// incompatible-version Error instead of crashing through. EDGE MODULE: depends on `ws`
// and the wire codec; verified against the real server once server-release-1 lands.

import WebSocket from "ws";
import {
  PROTOCOL_VERSION,
  type ClientMessage,
  type Color,
  type PlayerId,
  type PlayerInfo,
  type ServerMessage,
} from "../protocol/messages.ts";
import { decodeServer, encodeClient, type WireMode } from "../protocol/codec.ts";

export interface JoinResult {
  roomId: string;
  yourPlayerId: PlayerId;
  yourColor: Color;
  players: PlayerInfo[];
}

export class ProtocolVersionError extends Error {}

type MessageHandler = (msg: ServerMessage) => void;

export class Connection {
  private ws: WebSocket | undefined;
  private handlers: MessageHandler[] = [];

  constructor(
    private readonly url: string,
    private readonly mode: WireMode = "msgpack",
  ) {}

  onServerMessage(handler: MessageHandler): void {
    this.handlers.push(handler);
  }

  /** Open the socket, send JoinRoom with our protocol_version, and await RoomJoined. */
  connectAndJoin(roomCode: string, playerName: string): Promise<JoinResult> {
    return new Promise<JoinResult>((resolve, reject) => {
      const ws = new WebSocket(this.url);
      this.ws = ws;
      ws.binaryType = "arraybuffer";

      let joined = false;

      ws.on("open", () => {
        this.send({
          type: "JoinRoom",
          room_code: roomCode,
          player_name: playerName,
          protocol_version: PROTOCOL_VERSION,
        });
      });

      ws.on("message", (data: WebSocket.RawData, isBinary: boolean) => {
        let msg: ServerMessage;
        try {
          msg = decodeServer(toBytesOrText(data, isBinary), this.mode);
        } catch (err) {
          reject(new Error(`failed to decode server frame: ${String(err)}`));
          return;
        }

        if (!joined) {
          if (msg.type === "RoomJoined") {
            joined = true;
            resolve({
              roomId: msg.room_id,
              yourPlayerId: msg.your_player_id,
              yourColor: msg.your_color,
              players: msg.players,
            });
            // fall through so the RoomJoined also reaches handlers
          } else if (msg.type === "Error") {
            const err =
              /version/i.test(msg.message) || /version/i.test(msg.code)
                ? new ProtocolVersionError(msg.message)
                : new Error(`server error before join: ${msg.code} ${msg.message}`);
            reject(err);
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
      });
    });
  }

  send(msg: ClientMessage): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    this.ws.send(encodeClient(msg, this.mode));
  }

  close(): void {
    this.ws?.close();
  }
}

function toBytesOrText(data: WebSocket.RawData, isBinary: boolean): Uint8Array | string {
  if (!isBinary && typeof data === "string") return data;
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (Array.isArray(data)) return Buffer.concat(data);
  return data as Uint8Array;
}
