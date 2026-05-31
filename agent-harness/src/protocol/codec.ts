// Wire codec. MessagePack by default (matching the server's rmp-serde, with the
// `#[serde(tag="type")]` discriminant our message types mirror); JSON is the optional
// debug fallback (server-release-1 `wire-protocol`). Claude never sees raw frames — it
// sees the curated thin context — so the wire format is purely an internal concern here
// (design D6). EDGE MODULE: depends on @msgpack/msgpack; verified once the server lands.

import { decode as mpDecode, encode as mpEncode } from "@msgpack/msgpack";
import type { ClientMessage, ServerMessage } from "./messages.ts";

export type WireMode = "msgpack" | "json";

export function encodeClient(msg: ClientMessage, mode: WireMode): Uint8Array | string {
  return mode === "json" ? JSON.stringify(msg) : mpEncode(msg);
}

export function decodeServer(data: Uint8Array | ArrayBuffer | string, mode: WireMode): ServerMessage {
  if (mode === "json") {
    const text = typeof data === "string" ? data : new TextDecoder().decode(data as ArrayBuffer);
    return JSON.parse(text) as ServerMessage;
  }
  const bytes = data instanceof Uint8Array ? data : new Uint8Array(data as ArrayBuffer);
  return mpDecode(bytes) as ServerMessage;
}
