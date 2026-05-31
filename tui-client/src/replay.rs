//! Deterministic record/replay of the inbound server-message stream.
//!
//! Recording writes each received [`ServerMessage`] as one JSON line; replay
//! reads them back and feeds them into the client with no socket, reproducing a
//! session exactly (research R5). This is what turns a reported bug into a
//! reproducible fixture.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use boiling_point_protocol::server::ServerMessage;

/// Append one message to an open replay file as a JSON line.
pub fn append(file: &mut File, msg: &ServerMessage) -> io::Result<()> {
    let line = serde_json::to_string(msg).map_err(io::Error::other)?;
    writeln!(file, "{line}")
}

/// Save a full message stream as JSON lines.
pub fn save(path: impl AsRef<Path>, msgs: &[ServerMessage]) -> io::Result<()> {
    let mut f = File::create(path)?;
    for m in msgs {
        append(&mut f, m)?;
    }
    Ok(())
}

/// Load a recorded JSON-lines stream back into messages.
pub fn load(path: impl AsRef<Path>) -> io::Result<Vec<ServerMessage>> {
    let f = File::open(path)?;
    let mut out = Vec::new();
    for line in BufReader::new(f).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let msg: ServerMessage = serde_json::from_str(&line).map_err(io::Error::other)?;
        out.push(msg);
    }
    Ok(out)
}
