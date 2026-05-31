//! Wire codec: MessagePack for production, JSON as a debug fallback.
//!
//! These helpers are deliberately standalone (free functions over any
//! `Serialize`/`Deserialize` type) so the entire wire can be unit-tested with
//! neither a server nor a bot present.

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Errors that can arise encoding or decoding a wire message.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    /// MessagePack encoding failed.
    #[error("messagepack encode failed: {0}")]
    MsgpackEncode(#[from] rmp_serde::encode::Error),
    /// MessagePack decoding failed.
    #[error("messagepack decode failed: {0}")]
    MsgpackDecode(#[from] rmp_serde::decode::Error),
    /// JSON encoding/decoding failed.
    #[error("json codec failed: {0}")]
    Json(#[from] serde_json::Error),
}

/// Encode a message to MessagePack bytes (the production wire format).
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, CodecError> {
    Ok(rmp_serde::to_vec_named(value)?)
}

/// Decode a message from MessagePack bytes.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError> {
    Ok(rmp_serde::from_slice(bytes)?)
}

/// Encode a message to a JSON string (debug fallback).
pub fn encode_json<T: Serialize>(value: &T) -> Result<String, CodecError> {
    Ok(serde_json::to_string(value)?)
}

/// Decode a message from a JSON string (debug fallback).
pub fn decode_json<T: DeserializeOwned>(json: &str) -> Result<T, CodecError> {
    Ok(serde_json::from_str(json)?)
}
