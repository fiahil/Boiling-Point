// Wire codec. MessagePack by default (matching the server's rmp-serde, with the
// `#[serde(tag="type")]` discriminant our message types mirror); JSON exists only as a
// debug fallback and is NOT accepted on the wire by the server. EDGE MODULE: depends on
// @msgpack/msgpack.
//
// Uuid quirk: the Rust `uuid` crate serializes as a STRING in human-readable formats but as
// 16 RAW BYTES in MessagePack. So `PlayerId` arrives as a Uint8Array, not a string. Since
// the protocol has no other bytes-typed fields, we canonicalize every decoded Uint8Array to
// a stable hex string — making PlayerId usable as a Map key / Set member / `includes` value.

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
  return canonicalizeBytes(mpDecode(bytes)) as ServerMessage;
}

/** Recursively replace Uint8Array values (uuids) with stable lowercase-hex strings. */
function canonicalizeBytes(value: unknown): unknown {
  if (value instanceof Uint8Array) return toHex(value);
  if (Array.isArray(value)) return value.map(canonicalizeBytes);
  if (value !== null && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value as Record<string, unknown>)) out[k] = canonicalizeBytes(v);
    return out;
  }
  return value;
}

function toHex(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += b.toString(16).padStart(2, "0");
  return s;
}
