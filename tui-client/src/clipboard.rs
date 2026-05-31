//! Best-effort clipboard copy via the OSC 52 terminal escape.
//!
//! This avoids a native-clipboard dependency (and its platform build cost): the
//! escape is understood by iTerm2, kitty, WezTerm, and tmux (with
//! `set -g set-clipboard on`). Where a terminal ignores OSC 52 the copy simply
//! has no effect — hence "best effort".

use std::io::{self, Write};

/// Copy `text` to the terminal clipboard using OSC 52. Errors only if stdout
/// cannot be written.
pub(crate) fn copy(text: &str) -> io::Result<()> {
    let seq = format!("\x1b]52;c;{}\x07", base64(text.as_bytes()));
    let mut out = io::stdout();
    out.write_all(seq.as_bytes())?;
    out.flush()
}

/// Minimal standard-alphabet base64 encoder (no external dependency).
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
