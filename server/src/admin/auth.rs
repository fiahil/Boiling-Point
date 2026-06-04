//! Operator authentication and role gating for the admin surface (`admin-auth`).
//!
//! This is **entirely separate** from anonymous player session tokens: operators
//! present a bearer token resolved against [`OperatorAuth`] (built from
//! configuration / environment), never the player [`crate::lobby::SessionStore`].
//! A player session token is meaningless here, so player credentials can never
//! reach an admin capability. Roles gate capability: [`OperatorRole::Observer`]
//! may perform **all reads**, including the hidden-state reveal (which is served
//! only over the admin channel, never a player connection); [`OperatorRole::Elevated`]
//! additionally may issue **control commands**.

use std::collections::HashMap;

/// An operator's role. Every control action requires
/// [`Elevated`](OperatorRole::Elevated); all reads (including the hidden-state
/// reveal) are available to any authenticated operator, down to `Observer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorRole {
    /// Read-only: fleet overview, group list, the hidden-state reveal, replay, and
    /// the balance dashboard.
    Observer,
    /// Everything an observer may do, plus the control commands (reload, toggle,
    /// group lifecycle).
    Elevated,
}

/// An authenticated operator principal.
#[derive(Debug, Clone)]
pub struct Operator {
    /// The operator's identity (used in command audit spans).
    pub name: String,
    /// The operator's role.
    pub role: OperatorRole,
}

impl Operator {
    /// Whether this operator may issue control commands.
    pub fn is_elevated(&self) -> bool {
        self.role == OperatorRole::Elevated
    }
}

/// The admin auth policy: a map of bearer token → (operator name, role). Tokens
/// are opaque secrets supplied out-of-band; this is deliberately simple
/// (Constitution III — a separate admin mechanism, not OAuth) and is the single
/// place admin credentials live.
#[derive(Default)]
pub struct OperatorAuth {
    tokens: HashMap<String, (String, OperatorRole)>,
}

impl OperatorAuth {
    /// An empty policy (no operator can authenticate).
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a token for an operator with a role (builder style).
    pub fn with_token(
        mut self,
        token: impl Into<String>,
        name: impl Into<String>,
        role: OperatorRole,
    ) -> Self {
        self.tokens.insert(token.into(), (name.into(), role));
        self
    }

    /// Build a policy from the environment: `BP_ADMIN_TOKEN` grants an elevated
    /// operator and `BP_ADMIN_OBSERVER_TOKEN` grants an observer. Unset vars are
    /// simply absent (with neither set, the admin surface authenticates no one).
    pub fn from_env() -> Self {
        let mut auth = Self::new();
        if let Ok(token) = std::env::var("BP_ADMIN_TOKEN") {
            auth = auth.with_token(token, "admin", OperatorRole::Elevated);
        }
        if let Ok(token) = std::env::var("BP_ADMIN_OBSERVER_TOKEN") {
            auth = auth.with_token(token, "observer", OperatorRole::Observer);
        }
        auth
    }

    /// Resolve a bearer token to an operator, or `None` if it is not recognized.
    pub fn authenticate(&self, token: &str) -> Option<Operator> {
        self.tokens.get(token).map(|(name, role)| Operator {
            name: name.clone(),
            role: *role,
        })
    }

    /// Whether any operator tokens are configured.
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_tokens_resolve_to_roles_unknown_are_denied() {
        let auth = OperatorAuth::new()
            .with_token("elev-secret", "root", OperatorRole::Elevated)
            .with_token("obs-secret", "watcher", OperatorRole::Observer);

        let elevated = auth
            .authenticate("elev-secret")
            .expect("elevated token resolves");
        assert!(elevated.is_elevated());
        assert_eq!(elevated.name, "root");

        let observer = auth
            .authenticate("obs-secret")
            .expect("observer token resolves");
        assert!(!observer.is_elevated());

        // A player session token (or any unknown string) authenticates no one.
        assert!(
            auth.authenticate("some-anonymous-player-session-uuid")
                .is_none()
        );
        assert!(auth.authenticate("").is_none());
    }
}
