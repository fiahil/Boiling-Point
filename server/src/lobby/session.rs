//! Anonymous session authentication: a player presents an optional prior token
//! and receives a stable [`PlayerId`]. A fresh connection mints a new identity;
//! a known token resolves to the same identity (so reconnection keeps the seat).

use dashmap::DashMap;
use uuid::Uuid;

use boiling_point_protocol::PlayerId;

/// Maps opaque session tokens to player identities. Concurrent and lock-free on
/// the hot path.
#[derive(Default)]
pub struct SessionStore {
    tokens: DashMap<String, PlayerId>,
}

impl SessionStore {
    /// A new, empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve an identity: a known token returns its existing [`PlayerId`];
    /// otherwise a fresh identity and token are minted and stored.
    pub fn authenticate(&self, token: Option<&str>) -> (PlayerId, String) {
        if let Some(t) = token {
            if let Some(existing) = self.tokens.get(t) {
                return (*existing, t.to_string());
            }
        }
        let player = PlayerId::new();
        let token = Uuid::new_v4().to_string();
        self.tokens.insert(token.clone(), player);
        (player, token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_connection_mints_identity() {
        let store = SessionStore::new();
        let (p1, t1) = store.authenticate(None);
        let (p2, t2) = store.authenticate(None);
        assert_ne!(p1, p2);
        assert_ne!(t1, t2);
    }

    #[test]
    fn known_token_resolves_same_identity() {
        let store = SessionStore::new();
        let (p1, t1) = store.authenticate(None);
        let (p2, t2) = store.authenticate(Some(&t1));
        assert_eq!(p1, p2);
        assert_eq!(t1, t2);
    }

    #[test]
    fn unknown_token_mints_new_identity() {
        let store = SessionStore::new();
        let (p, _) = store.authenticate(Some("not-a-real-token"));
        let (p2, _) = store.authenticate(Some("not-a-real-token-either"));
        assert_ne!(p, p2);
    }
}
