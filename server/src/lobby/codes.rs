//! Human-readable invite codes like `BREW-7K3F`.

use rand::Rng;

use boiling_point_protocol::RoomCode;

/// Unambiguous alphabet (no `0/O`, `1/I`, etc.) for readable codes.
const ALPHABET: &[u8] = b"ACDEFGHJKLMNPQRSTUVWXYZ23456789";
/// Number of random characters after the `BREW-` prefix.
const SUFFIX_LEN: usize = 4;

/// Generate a fresh invite code. Callers should retry on the (rare) collision.
pub fn generate_code() -> RoomCode {
    let mut rng = rand::thread_rng();
    let suffix: String = (0..SUFFIX_LEN)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect();
    RoomCode(format!("BREW-{suffix}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_has_expected_shape() {
        let RoomCode(code) = generate_code();
        assert!(code.starts_with("BREW-"));
        let suffix = &code["BREW-".len()..];
        assert_eq!(suffix.len(), SUFFIX_LEN);
        assert!(suffix.bytes().all(|b| ALPHABET.contains(&b)));
    }
}
