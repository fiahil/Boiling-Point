//! Persistent player accounts (`boom2-identity`, capability `player-accounts`):
//! the durable cross-game identity an anonymous session can **optionally**
//! upgrade to, and the attachment point for rating.
//!
//! Two account kinds ([design D1], [roadmap "Identity"]): a **device-bound
//! anonymous** account (a durable token, no credentials — the lightest path) and
//! an **OAuth** account (Google/Discord — portable across devices). Both resolve
//! to the same durable [`PlayerId`]; an account *binds* an existing player id
//! rather than replacing it, so upgrading never disrupts a session. Anonymous
//! play stays the default and the fallback (Principle III).
//!
//! The store is in-memory and authoritative at runtime (like
//! [`super::SessionStore`]); durable persistence is layered on by write-through
//! when a database is configured, and hydrated on boot. With no database it
//! still works fully in memory, so the e2e suite needs no DB (Principle II) —
//! device tokens just don't survive a restart there, which is fine for tests.

use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use sqlx::PgPool;
use uuid::Uuid;

use boiling_point_protocol::{AccountId, AccountType, OAuthProvider, PlayerId};

/// A durable account: its id, the [`PlayerId`] it binds, and its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Account {
    /// The account's stable id.
    pub id: AccountId,
    /// The durable player identity bound to the account.
    pub player_id: PlayerId,
    /// The account kind (device-bound or OAuth).
    pub kind: AccountType,
}

/// Why an account operation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountError {
    /// A credential could not be verified (unknown token, provider rejected it,
    /// or OAuth is not configured on this server).
    AuthFailed(String),
    /// The operation conflicts with existing state (the identity is already
    /// bound elsewhere, or the player already has an account) — sign in instead.
    Conflict,
}

/// A verified OAuth identity: the provider plus its stable, opaque subject id
/// (the provider's permanent user identifier, e.g. Google's `sub`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthIdentity {
    /// The provider that issued the identity.
    pub provider: OAuthProvider,
    /// The provider's stable subject id for this user.
    pub subject: String,
}

/// Turns a provider access token into a verified [`OAuthIdentity`]. The seam that
/// keeps OAuth — the heaviest dependency ([design risks]) — out of the hot path
/// and out of the headless tests: production wires [`HttpOAuthVerifier`]; tests
/// and an OAuth-less server use [`DisabledOAuthVerifier`].
#[async_trait]
pub trait OAuthVerifier: Send + Sync {
    /// Verify `access_token` with `provider` and return the stable identity, or
    /// an error message if the token is invalid/unverifiable.
    async fn verify(
        &self,
        provider: OAuthProvider,
        access_token: &str,
    ) -> Result<OAuthIdentity, String>;
}

/// The default verifier: OAuth is not configured, so every OAuth attempt fails
/// cleanly (device-bound and anonymous play are unaffected).
pub struct DisabledOAuthVerifier;

#[async_trait]
impl OAuthVerifier for DisabledOAuthVerifier {
    async fn verify(
        &self,
        _provider: OAuthProvider,
        _access_token: &str,
    ) -> Result<OAuthIdentity, String> {
        Err("OAuth sign-in is not configured on this server".into())
    }
}

/// The production verifier: resolves a stable subject by calling the provider's
/// userinfo endpoint with the bearer access token (Google's OpenID userinfo,
/// Discord's `users/@me`). Constructed only when OAuth is configured, so tests
/// and OAuth-less deployments make no network calls.
pub struct HttpOAuthVerifier {
    client: reqwest::Client,
}

impl HttpOAuthVerifier {
    /// A verifier with a fresh HTTP client.
    pub fn new() -> Self {
        HttpOAuthVerifier {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for HttpOAuthVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OAuthVerifier for HttpOAuthVerifier {
    async fn verify(
        &self,
        provider: OAuthProvider,
        access_token: &str,
    ) -> Result<OAuthIdentity, String> {
        let (url, subject_field) = match provider {
            OAuthProvider::Google => ("https://openidconnect.googleapis.com/v1/userinfo", "sub"),
            OAuthProvider::Discord => ("https://discord.com/api/users/@me", "id"),
        };
        let resp = self
            .client
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| format!("provider request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!(
                "provider rejected the token: HTTP {}",
                resp.status()
            ));
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("provider response was not JSON: {e}"))?;
        let subject = body
            .get(subject_field)
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("provider response missing `{subject_field}`"))?
            .to_string();
        if subject.is_empty() {
            return Err("provider returned an empty subject".into());
        }
        Ok(OAuthIdentity { provider, subject })
    }
}

/// A persisted account row (for hydrate on boot / write-through). Plain data so
/// the persistence layer stays free of this module's types where it can.
#[derive(Debug, Clone)]
pub struct AccountRecord {
    /// Account id.
    pub id: Uuid,
    /// Bound player id.
    pub player_id: Uuid,
    /// `"device"` or `"oauth"`.
    pub kind: String,
    /// The device token, for device accounts.
    pub device_token: Option<String>,
    /// The provider label, for OAuth accounts.
    pub oauth_provider: Option<String>,
    /// The provider subject, for OAuth accounts.
    pub oauth_subject: Option<String>,
}

/// The concurrent account registry: the accounts plus the lookup indices that
/// resolve a credential (device token / OAuth subject) or a player id to an
/// account. In-memory and authoritative; an optional database backs durability.
pub struct AccountStore {
    accounts: DashMap<AccountId, Account>,
    by_device: DashMap<String, AccountId>,
    by_oauth: DashMap<(OAuthProvider, String), AccountId>,
    by_player: DashMap<PlayerId, AccountId>,
    verifier: Arc<dyn OAuthVerifier>,
    /// Optional durable backing. `Some` ⇒ accounts are written through on
    /// creation and survive a restart; `None` ⇒ in-memory only (fine for tests
    /// and OAuth-less local play, where device tokens last the process lifetime).
    pool: Option<PgPool>,
}

impl Default for AccountStore {
    fn default() -> Self {
        AccountStore {
            accounts: DashMap::new(),
            by_device: DashMap::new(),
            by_oauth: DashMap::new(),
            by_player: DashMap::new(),
            verifier: Arc::new(DisabledOAuthVerifier),
            pool: None,
        }
    }
}

impl AccountStore {
    /// An empty store with OAuth disabled (device-bound + anonymous still work).
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty store with the given OAuth verifier (production wires the HTTP one).
    pub fn with_verifier(verifier: Arc<dyn OAuthVerifier>) -> Self {
        AccountStore {
            verifier,
            ..Self::default()
        }
    }

    /// Attach the durable backing pool (write-through on creation). `None` leaves
    /// the store in-memory only.
    pub fn with_pool(mut self, pool: Option<PgPool>) -> Self {
        self.pool = pool;
        self
    }

    /// Write an account through to durable storage, if a pool is attached. Best
    /// effort: a failed write logs but does not fail the in-memory operation
    /// (the account is already live; durability is the only thing lost).
    async fn write_through(
        &self,
        account: Account,
        device_token: Option<&str>,
        oauth: Option<&OAuthIdentity>,
    ) {
        let Some(pool) = &self.pool else { return };
        let (kind, provider, subject) = match oauth {
            Some(id) => (
                "oauth",
                Some(id.provider.label()),
                Some(id.subject.as_str()),
            ),
            None => ("device", None, None),
        };
        if let Err(e) = crate::persistence::upsert_account(
            pool,
            account.id.0,
            account.player_id.0,
            kind,
            device_token,
            provider,
            subject,
        )
        .await
        {
            tracing::error!(account.id = %account.id.0, error = %e, "failed to persist account");
        }
    }

    /// Look up an account by id.
    pub fn account(&self, id: AccountId) -> Option<Account> {
        self.accounts.get(&id).map(|a| *a)
    }

    /// The account bound to `player`, if any.
    pub fn account_for_player(&self, player: PlayerId) -> Option<Account> {
        self.by_player
            .get(&player)
            .and_then(|id| self.accounts.get(&id).map(|a| *a))
    }

    /// Whether `player` already has a durable account.
    pub fn is_registered(&self, player: PlayerId) -> bool {
        self.by_player.contains_key(&player)
    }

    /// Number of accounts.
    pub fn len(&self) -> usize {
        self.accounts.len()
    }

    /// Whether there are no accounts.
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }

    /// Insert a fully-formed account into the in-memory indices (hydrate on boot;
    /// also the shared tail of create/link). Idempotent on the id.
    fn index(&self, account: Account, device_token: Option<String>, oauth: Option<OAuthIdentity>) {
        self.accounts.insert(account.id, account);
        self.by_player.insert(account.player_id, account.id);
        if let Some(t) = device_token {
            self.by_device.insert(t, account.id);
        }
        if let Some(id) = oauth {
            self.by_oauth.insert((id.provider, id.subject), account.id);
        }
    }

    /// Hydrate a persisted account row into the store (boot-time load).
    pub fn hydrate(&self, rec: AccountRecord) {
        let kind = if rec.kind == "oauth" {
            AccountType::OAuth
        } else {
            AccountType::DeviceBound
        };
        let account = Account {
            id: AccountId(rec.id),
            player_id: PlayerId(rec.player_id),
            kind,
        };
        let oauth = match (rec.oauth_provider.as_deref(), rec.oauth_subject) {
            (Some("google"), Some(sub)) => Some(OAuthIdentity {
                provider: OAuthProvider::Google,
                subject: sub,
            }),
            (Some("discord"), Some(sub)) => Some(OAuthIdentity {
                provider: OAuthProvider::Discord,
                subject: sub,
            }),
            _ => None,
        };
        self.index(account, rec.device_token, oauth);
    }

    /// **Upgrade** the current player to a fresh **device-bound** account, binding
    /// the existing `player_id`. Returns the new account and its durable token
    /// (the secret the client persists and replays). Errors [`AccountError::Conflict`]
    /// if the player already has an account (upgrade once).
    pub async fn create_device_account(
        &self,
        player_id: PlayerId,
    ) -> Result<(Account, String), AccountError> {
        if self.is_registered(player_id) {
            return Err(AccountError::Conflict);
        }
        let account = Account {
            id: AccountId::new(),
            player_id,
            kind: AccountType::DeviceBound,
        };
        let token = Uuid::new_v4().to_string();
        self.index(account, Some(token.clone()), None);
        self.write_through(account, Some(&token), None).await;
        Ok((account, token))
    }

    /// **Upgrade** the current player by linking an **OAuth** identity: verify the
    /// token, then bind the existing `player_id` to a fresh OAuth account. Errors
    /// [`AccountError::Conflict`] if that identity is already bound to a *different*
    /// account or the player already has an account (sign in at entry instead);
    /// idempotent when the same player re-links the same identity.
    pub async fn link_oauth(
        &self,
        player_id: PlayerId,
        provider: OAuthProvider,
        access_token: &str,
    ) -> Result<Account, AccountError> {
        let identity = self
            .verifier
            .verify(provider, access_token)
            .await
            .map_err(AccountError::AuthFailed)?;
        let key = (identity.provider, identity.subject.clone());
        if let Some(existing) = self.by_oauth.get(&key).map(|r| *r) {
            let account = self.accounts.get(&existing).map(|a| *a);
            return match account {
                Some(a) if a.player_id == player_id => Ok(a), // idempotent re-link
                _ => Err(AccountError::Conflict),             // belongs to someone else
            };
        }
        if self.is_registered(player_id) {
            return Err(AccountError::Conflict);
        }
        let account = Account {
            id: AccountId::new(),
            player_id,
            kind: AccountType::OAuth,
        };
        self.index(account, None, Some(identity.clone()));
        self.write_through(account, None, Some(&identity)).await;
        Ok(account)
    }

    /// **Sign in** with a credential at entry: resolve it to its account and that
    /// account's durable [`PlayerId`] (which the connection then adopts).
    ///
    /// - Device: resolve the token; an unknown token is [`AccountError::AuthFailed`].
    /// - OAuth: verify the token, then **find-or-create** — an existing identity
    ///   resolves to its account (cross-device portability); a brand-new identity
    ///   mints a fresh account bound to a new player id (sign-up via OAuth).
    pub async fn sign_in(&self, credential: &SignInCredential) -> Result<Account, AccountError> {
        match credential {
            SignInCredential::Device { account_token } => self
                .by_device
                .get(account_token)
                .and_then(|id| self.accounts.get(&id).map(|a| *a))
                .ok_or_else(|| AccountError::AuthFailed("unknown device account token".into())),
            SignInCredential::OAuth {
                provider,
                access_token,
            } => {
                let identity = self
                    .verifier
                    .verify(*provider, access_token)
                    .await
                    .map_err(AccountError::AuthFailed)?;
                let key = (identity.provider, identity.subject.clone());
                if let Some(existing) = self.by_oauth.get(&key).map(|r| *r)
                    && let Some(account) = self.accounts.get(&existing).map(|a| *a)
                {
                    return Ok(account);
                }
                // First sign-in for this identity, on this or any device: mint a
                // fresh durable player + account (find-or-create).
                let account = Account {
                    id: AccountId::new(),
                    player_id: PlayerId::new(),
                    kind: AccountType::OAuth,
                };
                self.index(account, None, Some(identity.clone()));
                self.write_through(account, None, Some(&identity)).await;
                Ok(account)
            }
        }
    }
}

/// A credential presented to [`AccountStore::sign_in`] — the server-side mirror
/// of the wire [`boiling_point_protocol::AccountCredential`], decoupled from the
/// transport so the store has no wire dependency beyond the id/provider types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInCredential {
    /// Resume a device-bound account by its durable token.
    Device {
        /// The durable device token.
        account_token: String,
    },
    /// Sign in with an OAuth access token (verified, then find-or-create).
    OAuth {
        /// The provider that issued the token.
        provider: OAuthProvider,
        /// The access token to verify.
        access_token: String,
    },
}

impl From<boiling_point_protocol::AccountCredential> for SignInCredential {
    fn from(c: boiling_point_protocol::AccountCredential) -> Self {
        match c {
            boiling_point_protocol::AccountCredential::Device { account_token } => {
                SignInCredential::Device { account_token }
            }
            boiling_point_protocol::AccountCredential::OAuth {
                provider,
                access_token,
            } => SignInCredential::OAuth {
                provider,
                access_token,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test verifier that maps any token to a fixed subject per provider, so
    /// the OAuth resolution logic is exercised with no network (the verifier is
    /// the external authority, not the system under test — not mocking).
    struct StubVerifier;

    #[async_trait]
    impl OAuthVerifier for StubVerifier {
        async fn verify(
            &self,
            provider: OAuthProvider,
            access_token: &str,
        ) -> Result<OAuthIdentity, String> {
            if access_token.is_empty() {
                return Err("empty token".into());
            }
            // The "subject" is the token's prefix before ':' — so two tokens can
            // resolve to the same identity (a returning user) or different ones.
            let subject = access_token.split(':').next().unwrap_or(access_token);
            Ok(OAuthIdentity {
                provider,
                subject: subject.to_string(),
            })
        }
    }

    #[test]
    fn device_account_survives_via_its_token() {
        let store = AccountStore::new();
        let player = PlayerId::new();
        let (account, token) = futures_block_on(store.create_device_account(player)).unwrap();
        assert_eq!(
            account.player_id, player,
            "the account binds the existing id"
        );
        assert_eq!(account.kind, AccountType::DeviceBound);

        // Resuming with the token resolves to the same durable identity.
        let resumed = futures_block_on(store.sign_in(&SignInCredential::Device {
            account_token: token,
        }))
        .unwrap();
        assert_eq!(resumed.id, account.id);
        assert_eq!(resumed.player_id, player);

        // An unknown token fails (not a silent new identity).
        let bad = futures_block_on(store.sign_in(&SignInCredential::Device {
            account_token: "nope".into(),
        }));
        assert!(matches!(bad, Err(AccountError::AuthFailed(_))));
    }

    #[test]
    fn creating_twice_conflicts() {
        let store = AccountStore::new();
        let player = PlayerId::new();
        futures_block_on(store.create_device_account(player)).unwrap();
        assert_eq!(
            futures_block_on(store.create_device_account(player)),
            Err(AccountError::Conflict),
            "a player upgrades to an account at most once"
        );
    }

    #[test]
    fn oauth_is_portable_across_devices() {
        let store = AccountStore::with_verifier(Arc::new(StubVerifier));
        // Device A: sign in (sign-up) — mints a fresh durable identity.
        let a = futures_block_on(store.sign_in(&SignInCredential::OAuth {
            provider: OAuthProvider::Google,
            access_token: "user-7:deviceA".into(),
        }))
        .unwrap();
        // Device B: same Google user (same subject prefix), different token —
        // resolves to the SAME account + player id.
        let b = futures_block_on(store.sign_in(&SignInCredential::OAuth {
            provider: OAuthProvider::Google,
            access_token: "user-7:deviceB".into(),
        }))
        .unwrap();
        assert_eq!(
            a.id, b.id,
            "same OAuth subject ⇒ same account across devices"
        );
        assert_eq!(a.player_id, b.player_id);

        // A different Google user is a different account.
        let c = futures_block_on(store.sign_in(&SignInCredential::OAuth {
            provider: OAuthProvider::Google,
            access_token: "user-9:deviceA".into(),
        }))
        .unwrap();
        assert_ne!(a.id, c.id);
    }

    #[test]
    fn link_oauth_binds_current_player_then_conflicts_on_reuse() {
        let store = AccountStore::with_verifier(Arc::new(StubVerifier));
        let player = PlayerId::new();
        let linked =
            futures_block_on(store.link_oauth(player, OAuthProvider::Discord, "u1:t")).unwrap();
        assert_eq!(
            linked.player_id, player,
            "link binds the existing player id"
        );

        // Re-linking the same identity to the same player is idempotent.
        let again =
            futures_block_on(store.link_oauth(player, OAuthProvider::Discord, "u1:t2")).unwrap();
        assert_eq!(again.id, linked.id);

        // A different player trying to link the same identity conflicts.
        let other = PlayerId::new();
        let conflict = futures_block_on(store.link_oauth(other, OAuthProvider::Discord, "u1:t3"));
        assert_eq!(conflict, Err(AccountError::Conflict));
    }

    #[test]
    fn disabled_verifier_rejects_oauth_but_devices_work() {
        let store = AccountStore::new(); // DisabledOAuthVerifier
        let player = PlayerId::new();
        assert!(futures_block_on(store.create_device_account(player)).is_ok());
        let err = futures_block_on(store.sign_in(&SignInCredential::OAuth {
            provider: OAuthProvider::Google,
            access_token: "anything".into(),
        }));
        assert!(matches!(err, Err(AccountError::AuthFailed(_))));
    }

    /// A tiny synchronous block-on for the store's async methods in unit tests
    /// (no Tokio runtime required — these futures never yield to a reactor).
    fn futures_block_on<F: std::future::Future>(fut: F) -> F::Output {
        // The store's futures complete without awaiting real I/O under the stub
        // verifier, so a trivial executor suffices.
        use std::task::{Context, Poll, Waker};
        let mut fut = Box::pin(fut);
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        loop {
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }
}
