//! The identity/account wire vocabulary (`boom2-identity`): the durable
//! account types, the credentials a client presents to resolve one, and the
//! public rating readout.
//!
//! Accounts are an **additive** upgrade over the anonymous session ([02 §14],
//! roadmap "Identity"): an anonymous [`crate::PlayerId`] keeps working with no
//! account, and an account merely *binds* an existing player id so it persists
//! across sessions and devices. The server owns the account store, the
//! credential resolution, and the rating math (Principle I); these are only the
//! DTOs that cross the wire — no secrets (a device token is the one secret the
//! server mints and the client persists, carried once on creation).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable, server-issued identity for a durable **account** (distinct from the
/// per-game [`PlayerId`] it binds).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountId(pub Uuid);

impl AccountId {
    /// Mint a fresh random account id.
    pub fn new() -> Self {
        AccountId(Uuid::new_v4())
    }
}

impl Default for AccountId {
    fn default() -> Self {
        Self::new()
    }
}

/// The account kinds (roadmap "Identity"). The lightest path is a device-bound
/// anonymous account (a durable token, no credentials); the portable paths are a
/// **passkey** (a pseudonym plus a WebAuthn credential, no password) and
/// **OAuth**. An account is bound to **one** identity of its kind — there is no
/// linking a second provider/passkey to the same account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    /// A durable token tied to one device, with no external credential. Survives
    /// a session on the same device; not portable across devices.
    DeviceBound,
    /// A passkey (WebAuthn) bound to the account's pseudonym; portable, no
    /// password and no password backup.
    Passkey,
    /// An OAuth identity (one of [`OAuthProvider`]). Portable: signing in on a
    /// new device with the same provider identity resolves to the same account.
    OAuth,
}

/// The supported OAuth providers. All but Discord are OpenID Connect (verified
/// by id-token); Discord is plain OAuth2 (verified by a userinfo call). The
/// server requests **no profile scopes** and never reads a real name or email —
/// only the provider's stable subject id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthProvider {
    /// Google sign-in (OIDC).
    Google,
    /// Apple sign-in (OIDC; id-token only — Apple has no userinfo endpoint).
    Apple,
    /// Microsoft sign-in (OIDC).
    Microsoft,
    /// Discord sign-in (OAuth2; userinfo via `users/@me`).
    Discord,
}

impl OAuthProvider {
    /// The stable lowercase label stored/serialised for this provider.
    pub fn label(self) -> &'static str {
        match self {
            OAuthProvider::Google => "google",
            OAuthProvider::Apple => "apple",
            OAuthProvider::Microsoft => "microsoft",
            OAuthProvider::Discord => "discord",
        }
    }

    /// Every supported provider (for enumeration in config/admin surfaces).
    pub const ALL: [OAuthProvider; 4] = [
        OAuthProvider::Google,
        OAuthProvider::Apple,
        OAuthProvider::Microsoft,
        OAuthProvider::Discord,
    ];
}

/// A credential a client presents (optionally) on an entry message to **sign in**
/// to — or resume — an existing account, adopting that account's durable player
/// identity. Absent ⇒ the connection authenticates anonymously (the default).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AccountCredential {
    /// Resume a device-bound account by its durable token (minted by the server
    /// on [`crate::ServerMessage::AccountEstablished`] and persisted by the client).
    Device {
        /// The durable device account token.
        account_token: String,
    },
    /// Sign in with an OAuth token; the server verifies it with the provider
    /// (an OIDC id token for Google/Apple/Microsoft, an access token for
    /// Discord) and resolves (or creates, on first sign-in) the matching
    /// account by its stable subject. One account per provider identity.
    OAuth {
        /// The provider that issued the token.
        provider: OAuthProvider,
        /// The token to verify (an id token for OIDC providers, else an access
        /// token). Only the subject is read — no profile scopes, no real name.
        token: String,
    },
    /// Sign in with a **passkey**: the account's pseudonym plus a WebAuthn
    /// assertion the server verifies against the stored credential (no
    /// password). The pseudonym is the account's current display name.
    Passkey {
        /// The account's pseudonym (its unique display name) to look up.
        pseudonym: String,
        /// The serialized WebAuthn authentication assertion.
        assertion: String,
    },
}

/// The public rating readout for one account: a single conservative number to
/// show, the rated-game count, and a "still settling" hint. The server owns the
/// underlying skill estimate (the FFA model's `mu`/`sigma`) and exposes only
/// this rounded, integer view on the wire — so the readout stays `Eq` like every
/// other message, and clients never reason about the model's internals
/// (Principle I).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RatingView {
    /// The displayed, conservative rating (the skill estimate discounted by its
    /// own uncertainty, rounded: higher is better, and it firms up with games).
    pub display: i32,
    /// Rated games this account has completed.
    pub games_played: u32,
    /// Whether the rating is still provisional (too few games / too uncertain to
    /// trust the display value yet).
    pub provisional: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{decode, decode_json, encode, encode_json};

    #[test]
    fn account_wire_types_roundtrip() {
        let creds = vec![
            AccountCredential::Device {
                account_token: "dev-tok-123".into(),
            },
            AccountCredential::OAuth {
                provider: OAuthProvider::Google,
                token: "id.token.google".into(),
            },
            AccountCredential::OAuth {
                provider: OAuthProvider::Apple,
                token: "id.token.apple".into(),
            },
            AccountCredential::OAuth {
                provider: OAuthProvider::Microsoft,
                token: "id.token.ms".into(),
            },
            AccountCredential::OAuth {
                provider: OAuthProvider::Discord,
                token: "disc.access".into(),
            },
            AccountCredential::Passkey {
                pseudonym: "simmering-ruby-newt".into(),
                assertion: "webauthn.assertion".into(),
            },
        ];
        for c in creds {
            assert_eq!(c, decode(&encode(&c).unwrap()).unwrap());
            assert_eq!(c, decode_json(&encode_json(&c).unwrap()).unwrap());
        }

        let rating = RatingView {
            display: 18,
            games_played: 3,
            provisional: true,
        };
        assert_eq!(rating, decode(&encode(&rating).unwrap()).unwrap());
        assert_eq!(rating, decode_json(&encode_json(&rating).unwrap()).unwrap());
    }

    #[test]
    fn account_id_binds_a_player() {
        // An account id and a player id are distinct identities.
        let a = AccountId::new();
        let p = crate::ids::PlayerId::new();
        assert_ne!(a.0, p.0);
    }
}
