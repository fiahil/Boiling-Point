//! Persistent player accounts (`boom2-identity`, capability `player-accounts`):
//! the durable cross-game identity an anonymous session can **optionally**
//! upgrade to, and the attachment point for rating.
//!
//! Account kinds ([design D1], [roadmap "Identity"]):
//! - **device-bound** — a durable token, no credentials (the lightest path; not
//!   portable across devices);
//! - **passkey** — a pseudonym plus a WebAuthn credential (portable, no password
//!   and no password backup);
//! - **OAuth** — one of Google/Apple/Microsoft/Discord (portable; same provider
//!   identity ⇒ same account).
//!
//! Privacy-first: an account carries **no email and no real name**. Every
//! account is auto-assigned a unique, themed pseudonym (e.g. `simmering-ruby-
//! newt`) which the player may change exactly **once**; OAuth/passkey contribute
//! only a stable opaque subject. An account is bound to a **single** identity of
//! its kind — there is no linking a second provider, and signing in never merges
//! into a current session. Players may **delete** their account (identity-only
//! erasure: the account, its rating, and its player record; shared game replays
//! are immutable anonymous records and are left intact).
//!
//! The store is in-memory and authoritative at runtime (like
//! [`super::SessionStore`]); durable persistence is layered on by write-through
//! when a database is configured, and hydrated on boot. With no database it
//! still works fully in memory, so the e2e suite needs no DB (Principle II).

use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use rand::Rng;
use rand::seq::SliceRandom;
use sqlx::PgPool;
use uuid::Uuid;

use boiling_point_protocol::{AccountId, AccountType, OAuthProvider, PlayerId};

/// A durable account: its id, the [`PlayerId`] it binds, its kind, and its
/// renameable pseudonym.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// The account's stable id.
    pub id: AccountId,
    /// The durable player identity bound to the account.
    pub player_id: PlayerId,
    /// The account kind.
    pub kind: AccountType,
    /// The account's current display name (a unique, themed pseudonym; for a
    /// passkey account, also its sign-in handle).
    pub display_name: String,
    /// How many display-name changes remain (1 fresh, 0 once the rename is spent).
    pub renames_remaining: u8,
}

/// Why an account operation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountError {
    /// A credential could not be verified (unknown token/pseudonym, provider or
    /// passkey rejected it, or the method is not configured on this server).
    AuthFailed(String),
    /// A requested display name is taken by another account or malformed.
    NameUnavailable,
    /// The account's single display-name change has already been spent.
    RenameLocked,
}

/// A verified OAuth identity: the provider plus its stable, opaque subject id
/// (the provider's permanent user identifier, e.g. an OIDC `sub`). No name, no
/// email — only the subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthIdentity {
    /// The provider that issued the identity.
    pub provider: OAuthProvider,
    /// The provider's stable subject id for this user.
    pub subject: String,
}

/// Turns a provider token into a verified [`OAuthIdentity`] — the seam that keeps
/// OAuth (the heaviest dependency) out of the headless tests. Production wires a
/// real verifier ([`super::verifiers::HttpOAuthVerifier`]); tests and an
/// OAuth-less server use [`DisabledOAuthVerifier`].
#[async_trait]
pub trait OAuthVerifier: Send + Sync {
    /// Verify `token` with `provider` and return the stable identity, or an error
    /// message. The verifier reads **only** the subject — no profile scopes.
    async fn verify(&self, provider: OAuthProvider, token: &str) -> Result<OAuthIdentity, String>;
}

/// The default OAuth verifier: OAuth is not configured, so every attempt fails
/// cleanly (device-bound, passkey, and anonymous play are unaffected).
pub struct DisabledOAuthVerifier;

#[async_trait]
impl OAuthVerifier for DisabledOAuthVerifier {
    async fn verify(&self, _p: OAuthProvider, _t: &str) -> Result<OAuthIdentity, String> {
        Err("OAuth sign-in is not configured on this server".into())
    }
}

/// Verifies WebAuthn passkey ceremonies — the seam that keeps the passkey crypto
/// (and the `webauthn-rs` dependency) out of the headless tests. The production
/// `webauthn-rs`-backed verifier lands with the web client (`adopt-pixi-client`),
/// which drives the challenge ceremony; until then a passkey-less server uses
/// [`DisabledPasskeyVerifier`] and tests use a stub.
#[async_trait]
pub trait PasskeyVerifier: Send + Sync {
    /// Verify a registration (attestation) and return the opaque credential
    /// record to store, or an error message.
    async fn verify_registration(&self, registration: &str) -> Result<String, String>;
    /// Verify an authentication assertion against a stored credential record.
    async fn verify_assertion(&self, credential: &str, assertion: &str) -> Result<(), String>;
}

/// The default passkey verifier: passkeys are not configured, so registration
/// and sign-in fail cleanly.
pub struct DisabledPasskeyVerifier;

#[async_trait]
impl PasskeyVerifier for DisabledPasskeyVerifier {
    async fn verify_registration(&self, _r: &str) -> Result<String, String> {
        Err("passkey sign-in is not configured on this server".into())
    }
    async fn verify_assertion(&self, _c: &str, _a: &str) -> Result<(), String> {
        Err("passkey sign-in is not configured on this server".into())
    }
}

/// A persisted account row (for hydrate on boot / write-through). Plain data so
/// the persistence layer stays decoupled from this module's types.
#[derive(Debug, Clone)]
pub struct AccountRecord {
    /// Account id.
    pub id: Uuid,
    /// Bound player id.
    pub player_id: Uuid,
    /// `"device"`, `"passkey"`, or `"oauth"`.
    pub kind: String,
    /// Current display name (unique pseudonym).
    pub display_name: String,
    /// Remaining renames (0 or 1).
    pub renames_remaining: i16,
    /// The device token, for device accounts.
    pub device_token: Option<String>,
    /// The provider label, for OAuth accounts.
    pub oauth_provider: Option<String>,
    /// The provider subject, for OAuth accounts.
    pub oauth_subject: Option<String>,
    /// The stored WebAuthn credential, for passkey accounts.
    pub passkey_credential: Option<String>,
}

/// A credential a client presents to [`AccountStore::sign_in`] — the server-side
/// mirror of the wire [`boiling_point_protocol::AccountCredential`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInCredential {
    /// Resume a device-bound account by its durable token.
    Device {
        /// The durable device token.
        account_token: String,
    },
    /// Sign in with an OAuth token (verified, then find-or-create by subject).
    OAuth {
        /// The provider that issued the token.
        provider: OAuthProvider,
        /// The token to verify (id token for OIDC, access token for Discord).
        token: String,
    },
    /// Sign in with a passkey: pseudonym lookup + assertion verification.
    Passkey {
        /// The account's pseudonym (its current display name).
        pseudonym: String,
        /// The WebAuthn authentication assertion.
        assertion: String,
    },
}

impl From<boiling_point_protocol::AccountCredential> for SignInCredential {
    fn from(c: boiling_point_protocol::AccountCredential) -> Self {
        use boiling_point_protocol::AccountCredential as W;
        match c {
            W::Device { account_token } => SignInCredential::Device { account_token },
            W::OAuth { provider, token } => SignInCredential::OAuth { provider, token },
            W::Passkey {
                pseudonym,
                assertion,
            } => SignInCredential::Passkey {
                pseudonym,
                assertion,
            },
        }
    }
}

/// The concurrent account registry: the accounts plus the lookup indices that
/// resolve a credential (device token / OAuth subject / passkey pseudonym) or a
/// player id to an account, the taken-name set (uniqueness), and the stored
/// passkey credentials. In-memory and authoritative; an optional database backs
/// durability.
pub struct AccountStore {
    accounts: DashMap<AccountId, Account>,
    by_device: DashMap<String, AccountId>,
    by_oauth: DashMap<(OAuthProvider, String), AccountId>,
    by_player: DashMap<PlayerId, AccountId>,
    /// display name → account (uniqueness, and the passkey sign-in handle).
    by_name: DashMap<String, AccountId>,
    /// account → stored passkey credential record (passkey accounts only).
    credentials: DashMap<AccountId, String>,
    oauth: Arc<dyn OAuthVerifier>,
    passkey: Arc<dyn PasskeyVerifier>,
    /// Optional durable backing. `Some` ⇒ write-through + last-login updates.
    pool: Option<PgPool>,
}

impl Default for AccountStore {
    fn default() -> Self {
        AccountStore {
            accounts: DashMap::new(),
            by_device: DashMap::new(),
            by_oauth: DashMap::new(),
            by_player: DashMap::new(),
            by_name: DashMap::new(),
            credentials: DashMap::new(),
            oauth: Arc::new(DisabledOAuthVerifier),
            passkey: Arc::new(DisabledPasskeyVerifier),
            pool: None,
        }
    }
}

impl AccountStore {
    /// An empty store with OAuth and passkeys disabled (device-bound + anonymous
    /// still work).
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach the OAuth verifier (production wires the HTTP one).
    pub fn with_oauth_verifier(mut self, verifier: Arc<dyn OAuthVerifier>) -> Self {
        self.oauth = verifier;
        self
    }

    /// Attach the passkey verifier (production wires the WebAuthn one).
    pub fn with_passkey_verifier(mut self, verifier: Arc<dyn PasskeyVerifier>) -> Self {
        self.passkey = verifier;
        self
    }

    /// Attach the durable backing pool (write-through + last-login). `None`
    /// leaves the store in-memory only.
    pub fn with_pool(mut self, pool: Option<PgPool>) -> Self {
        self.pool = pool;
        self
    }

    /// Look up an account by id.
    pub fn account(&self, id: AccountId) -> Option<Account> {
        self.accounts.get(&id).map(|a| a.clone())
    }

    /// The account bound to `player`, if any.
    pub fn account_for_player(&self, player: PlayerId) -> Option<Account> {
        self.by_player
            .get(&player)
            .and_then(|id| self.accounts.get(&id).map(|a| a.clone()))
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

    /// Insert a fully-formed account into the in-memory indices (the shared tail
    /// of hydrate/create). The name is registered for uniqueness.
    fn index(
        &self,
        account: Account,
        device_token: Option<String>,
        oauth: Option<OAuthIdentity>,
        credential: Option<String>,
    ) {
        self.by_player.insert(account.player_id, account.id);
        self.by_name
            .insert(account.display_name.clone(), account.id);
        if let Some(t) = device_token {
            self.by_device.insert(t, account.id);
        }
        if let Some(id) = oauth {
            self.by_oauth.insert((id.provider, id.subject), account.id);
        }
        if let Some(c) = credential {
            self.credentials.insert(account.id, c);
        }
        self.accounts.insert(account.id, account);
    }

    /// Hydrate a persisted account row into the store (boot-time load).
    pub fn hydrate(&self, rec: AccountRecord) {
        let kind = match rec.kind.as_str() {
            "oauth" => AccountType::OAuth,
            "passkey" => AccountType::Passkey,
            _ => AccountType::DeviceBound,
        };
        let account = Account {
            id: AccountId(rec.id),
            player_id: PlayerId(rec.player_id),
            kind,
            display_name: rec.display_name,
            renames_remaining: rec.renames_remaining.clamp(0, 1) as u8,
        };
        let oauth = match (
            provider_from_label(rec.oauth_provider.as_deref()),
            rec.oauth_subject,
        ) {
            (Some(provider), Some(subject)) => Some(OAuthIdentity { provider, subject }),
            _ => None,
        };
        self.index(account, rec.device_token, oauth, rec.passkey_credential);
    }

    /// A unique, themed pseudonym not currently taken by any account.
    fn fresh_name(&self) -> String {
        let mut rng = rand::thread_rng();
        for _ in 0..32 {
            let name = generate_name(&mut rng, 0);
            if !self.by_name.contains_key(&name) {
                return name;
            }
        }
        // Vanishingly unlikely fallthrough: widen with more entropy until unique.
        loop {
            let name = generate_name(&mut rng, 1_000);
            if !self.by_name.contains_key(&name) {
                return name;
            }
        }
    }

    /// **Upgrade** the current player to a device-bound account (binds the
    /// existing `player_id`), auto-naming it. Returns the account and, for a
    /// freshly minted one, its durable token (the secret the client persists).
    /// If the player already has an account it is returned unchanged (no new
    /// token) — there are no conflicts.
    pub async fn create_device_account(
        &self,
        player_id: PlayerId,
    ) -> Result<(Account, Option<String>), AccountError> {
        if let Some(existing) = self.account_for_player(player_id) {
            return Ok((existing, None));
        }
        let account = Account {
            id: AccountId::new(),
            player_id,
            kind: AccountType::DeviceBound,
            display_name: self.fresh_name(),
            renames_remaining: 1,
        };
        let token = Uuid::new_v4().to_string();
        self.index(account.clone(), Some(token.clone()), None, None);
        self.write_through(&account, Some(&token), None, None).await;
        Ok((account, Some(token)))
    }

    /// Create a **passkey** account from a completed registration (binds the
    /// current `player_id`), auto-naming it and storing the verified credential.
    /// If the player already has an account it is returned unchanged.
    pub async fn register_passkey(
        &self,
        player_id: PlayerId,
        registration: &str,
    ) -> Result<Account, AccountError> {
        if let Some(existing) = self.account_for_player(player_id) {
            return Ok(existing);
        }
        let credential = self
            .passkey
            .verify_registration(registration)
            .await
            .map_err(AccountError::AuthFailed)?;
        let account = Account {
            id: AccountId::new(),
            player_id,
            kind: AccountType::Passkey,
            display_name: self.fresh_name(),
            renames_remaining: 1,
        };
        self.index(account.clone(), None, None, Some(credential.clone()));
        self.write_through(&account, None, None, Some(&credential))
            .await;
        Ok(account)
    }

    /// **Sign in** with a credential at entry: resolve it to its account and that
    /// account's durable [`PlayerId`] (which the connection then adopts).
    ///
    /// - Device: resolve the token; unknown ⇒ [`AccountError::AuthFailed`].
    /// - OAuth: verify the token, then **find-or-create by (provider, subject)** —
    ///   an existing identity resolves to its account (portability); a new one
    ///   mints a fresh account bound to a new player. One account per identity.
    /// - Passkey: look the account up by pseudonym, then verify the assertion.
    ///
    /// On success the account's last-login timestamp is touched (durable only).
    pub async fn sign_in(&self, credential: &SignInCredential) -> Result<Account, AccountError> {
        let account = match credential {
            SignInCredential::Device { account_token } => self
                .by_device
                .get(account_token)
                .and_then(|id| self.accounts.get(&id).map(|a| a.clone()))
                .ok_or_else(|| AccountError::AuthFailed("unknown device account token".into()))?,
            SignInCredential::OAuth { provider, token } => {
                let identity = self
                    .oauth
                    .verify(*provider, token)
                    .await
                    .map_err(AccountError::AuthFailed)?;
                let key = (identity.provider, identity.subject.clone());
                if let Some(existing) = self.by_oauth.get(&key).map(|r| *r)
                    && let Some(account) = self.accounts.get(&existing).map(|a| a.clone())
                {
                    account
                } else {
                    let account = Account {
                        id: AccountId::new(),
                        player_id: PlayerId::new(),
                        kind: AccountType::OAuth,
                        display_name: self.fresh_name(),
                        renames_remaining: 1,
                    };
                    self.index(account.clone(), None, Some(identity.clone()), None);
                    self.write_through(&account, None, Some(&identity), None)
                        .await;
                    account
                }
            }
            SignInCredential::Passkey {
                pseudonym,
                assertion,
            } => {
                let account = self
                    .by_name
                    .get(pseudonym)
                    .and_then(|id| self.accounts.get(&id).map(|a| a.clone()))
                    .filter(|a| a.kind == AccountType::Passkey)
                    .ok_or_else(|| {
                        AccountError::AuthFailed("no passkey for that pseudonym".into())
                    })?;
                let credential = self
                    .credentials
                    .get(&account.id)
                    .map(|c| c.clone())
                    .ok_or_else(|| AccountError::AuthFailed("missing passkey credential".into()))?;
                self.passkey
                    .verify_assertion(&credential, assertion)
                    .await
                    .map_err(AccountError::AuthFailed)?;
                account
            }
        };
        self.touch_last_login(account.id).await;
        Ok(account)
    }

    /// Change `player`'s account display name — allowed **once**. Validates the
    /// name (well-formed + unique), applies it, frees the old name, decrements the
    /// rename allowance, and writes through. The single rename, once spent, locks.
    pub async fn set_display_name(
        &self,
        player: PlayerId,
        new_name: &str,
    ) -> Result<Account, AccountError> {
        let Some(mut account) = self.account_for_player(player) else {
            return Err(AccountError::AuthFailed(
                "not signed in to an account".into(),
            ));
        };
        if account.renames_remaining == 0 {
            return Err(AccountError::RenameLocked);
        }
        let name = new_name.trim().to_string();
        if !is_valid_name(&name) {
            return Err(AccountError::NameUnavailable);
        }
        // Unique unless it's already this account's name (a no-op rename still
        // spends the allowance, matching "once").
        if let Some(holder) = self.by_name.get(&name).map(|r| *r)
            && holder != account.id
        {
            return Err(AccountError::NameUnavailable);
        }
        self.by_name.remove(&account.display_name);
        self.by_name.insert(name.clone(), account.id);
        account.display_name = name;
        account.renames_remaining = 0;
        self.accounts.insert(account.id, account.clone());
        self.persist_name(&account).await;
        Ok(account)
    }

    /// **Delete** `player`'s account (identity-only erasure): remove it from every
    /// in-memory index and delete its durable rows (account, rating, player);
    /// shared game replays are left intact. Returns the deleted account (so the
    /// caller can also drop its in-memory rating). A no-op (`None`) if the player
    /// has no account.
    pub async fn delete_account(&self, player: PlayerId) -> Option<Account> {
        let account = self.account_for_player(player)?;
        self.accounts.remove(&account.id);
        self.by_player.remove(&account.player_id);
        self.by_name.remove(&account.display_name);
        self.credentials.remove(&account.id);
        self.by_device.retain(|_, v| *v != account.id);
        self.by_oauth.retain(|_, v| *v != account.id);
        if let Some(pool) = &self.pool
            && let Err(e) =
                crate::persistence::delete_account(pool, account.id.0, account.player_id.0).await
        {
            tracing::error!(account.id = %account.id.0, error = %e, "failed to delete account");
        }
        Some(account)
    }

    /// Write an account through to durable storage on creation, if a pool is
    /// attached. Best effort: a failed write logs but does not fail the in-memory
    /// operation.
    async fn write_through(
        &self,
        account: &Account,
        device_token: Option<&str>,
        oauth: Option<&OAuthIdentity>,
        credential: Option<&str>,
    ) {
        let Some(pool) = &self.pool else { return };
        let kind = match account.kind {
            AccountType::DeviceBound => "device",
            AccountType::Passkey => "passkey",
            AccountType::OAuth => "oauth",
        };
        let (provider, subject) = match oauth {
            Some(id) => (Some(id.provider.label()), Some(id.subject.as_str())),
            None => (None, None),
        };
        if let Err(e) = crate::persistence::upsert_account(
            pool,
            account.id.0,
            account.player_id.0,
            kind,
            &account.display_name,
            account.renames_remaining as i16,
            device_token,
            provider,
            subject,
            credential,
        )
        .await
        {
            tracing::error!(account.id = %account.id.0, error = %e, "failed to persist account");
        }
    }

    /// Persist a display-name change (best effort).
    async fn persist_name(&self, account: &Account) {
        let Some(pool) = &self.pool else { return };
        if let Err(e) = crate::persistence::update_account_name(
            pool,
            account.id.0,
            &account.display_name,
            account.renames_remaining as i16,
        )
        .await
        {
            tracing::error!(account.id = %account.id.0, error = %e, "failed to persist name");
        }
    }

    /// Touch an account's last-login timestamp (durable only; best effort).
    async fn touch_last_login(&self, id: AccountId) {
        let Some(pool) = &self.pool else { return };
        if let Err(e) = crate::persistence::touch_last_login(pool, id.0).await {
            tracing::error!(account.id = %id.0, error = %e, "failed to touch last_login");
        }
    }
}

/// The OIDC provider for a stored label, if recognised.
fn provider_from_label(label: Option<&str>) -> Option<OAuthProvider> {
    match label {
        Some("google") => Some(OAuthProvider::Google),
        Some("apple") => Some(OAuthProvider::Apple),
        Some("microsoft") => Some(OAuthProvider::Microsoft),
        Some("discord") => Some(OAuthProvider::Discord),
        _ => None,
    }
}

/// Themed pseudonym word-banks (apothecary/alchemy mood). The product is large
/// (20×12×16 = 3840 base combos) and uniqueness retries widen it further.
const ADJECTIVES: &[&str] = &[
    "simmering",
    "bubbling",
    "volatile",
    "gilded",
    "brisk",
    "molten",
    "candied",
    "briny",
    "smoky",
    "frosted",
    "spiced",
    "glowing",
    "restless",
    "humming",
    "drifting",
    "prowling",
    "cunning",
    "brazen",
    "velvet",
    "murky",
];
const COLORS: &[&str] = &[
    "ruby", "sapphire", "emerald", "amethyst", "amber", "cobalt", "crimson", "jade", "ochre",
    "ivory", "copper", "indigo",
];
const CREATURES: &[&str] = &[
    "newt", "toad", "raven", "moth", "adder", "otter", "ferret", "magpie", "lynx", "heron", "vole",
    "stoat", "weasel", "marten", "finch", "shrew",
];

/// Generate a themed `adjective-color-creature` pseudonym; `extra_entropy > 0`
/// appends a short numeric suffix to widen the space for the rare collision tail.
fn generate_name(rng: &mut impl Rng, extra_entropy: u32) -> String {
    let adj = ADJECTIVES.choose(rng).unwrap();
    let color = COLORS.choose(rng).unwrap();
    let creature = CREATURES.choose(rng).unwrap();
    if extra_entropy > 0 {
        format!(
            "{adj}-{color}-{creature}-{}",
            rng.gen_range(0..extra_entropy)
        )
    } else {
        format!("{adj}-{color}-{creature}")
    }
}

/// A user-chosen display name is well-formed: 3–32 chars of lowercase letters,
/// digits, and single hyphens (no leading/trailing/double hyphen). Keeps names
/// URL-safe and bounds impersonation surface; the auto-generated names conform.
fn is_valid_name(name: &str) -> bool {
    let len = name.chars().count();
    if !(3..=32).contains(&len) {
        return false;
    }
    if name.starts_with('-') || name.ends_with('-') || name.contains("--") {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test OAuth verifier mapping the token's prefix-before-':' to a subject,
    /// so two tokens can resolve to the same identity (a returning user).
    struct StubOAuth;
    #[async_trait]
    impl OAuthVerifier for StubOAuth {
        async fn verify(
            &self,
            provider: OAuthProvider,
            token: &str,
        ) -> Result<OAuthIdentity, String> {
            if token.is_empty() {
                return Err("empty token".into());
            }
            Ok(OAuthIdentity {
                provider,
                subject: token.split(':').next().unwrap_or(token).to_string(),
            })
        }
    }

    /// A test passkey verifier: registration yields a credential = the blob; an
    /// assertion verifies iff it equals the stored credential.
    struct StubPasskey;
    #[async_trait]
    impl PasskeyVerifier for StubPasskey {
        async fn verify_registration(&self, registration: &str) -> Result<String, String> {
            if registration.is_empty() {
                return Err("empty registration".into());
            }
            Ok(format!("cred:{registration}"))
        }
        async fn verify_assertion(&self, credential: &str, assertion: &str) -> Result<(), String> {
            if credential == format!("cred:{assertion}") {
                Ok(())
            } else {
                Err("bad assertion".into())
            }
        }
    }

    fn store() -> AccountStore {
        AccountStore::new()
            .with_oauth_verifier(Arc::new(StubOAuth))
            .with_passkey_verifier(Arc::new(StubPasskey))
    }

    #[test]
    fn auto_names_are_themed_and_unique() {
        let store = store();
        let mut names = std::collections::HashSet::new();
        for _ in 0..200 {
            let p = PlayerId::new();
            let (account, _) = block(store.create_device_account(p)).unwrap();
            assert!(
                is_valid_name(&account.display_name),
                "{}",
                account.display_name
            );
            let parts = account.display_name.split('-').count();
            assert!(
                (3..=4).contains(&parts),
                "themed shape: {}",
                account.display_name
            );
            assert!(
                names.insert(account.display_name.clone()),
                "names must be unique"
            );
            assert_eq!(account.renames_remaining, 1);
        }
    }

    #[test]
    fn device_account_resumes_and_create_is_idempotent() {
        let store = store();
        let player = PlayerId::new();
        let (account, token) = block(store.create_device_account(player)).unwrap();
        let token = token.expect("fresh device account returns a token");
        // Re-creating for the same player is a no-op (no conflict, no new token).
        let (again, again_token) = block(store.create_device_account(player)).unwrap();
        assert_eq!(again.id, account.id);
        assert!(again_token.is_none());
        // Resume by token → same account.
        let resumed = block(store.sign_in(&SignInCredential::Device {
            account_token: token,
        }))
        .unwrap();
        assert_eq!(resumed.id, account.id);
    }

    #[test]
    fn oauth_same_identity_same_account_no_conflict() {
        let store = store();
        // Two devices, same Google subject → SAME account (portable).
        let a = block(store.sign_in(&SignInCredential::OAuth {
            provider: OAuthProvider::Google,
            token: "sub7:devA".into(),
        }))
        .unwrap();
        let b = block(store.sign_in(&SignInCredential::OAuth {
            provider: OAuthProvider::Google,
            token: "sub7:devB".into(),
        }))
        .unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(a.player_id, b.player_id);
        // Same subject on a DIFFERENT provider is a different account (no merge).
        let apple = block(store.sign_in(&SignInCredential::OAuth {
            provider: OAuthProvider::Apple,
            token: "sub7:devA".into(),
        }))
        .unwrap();
        assert_ne!(a.id, apple.id);
    }

    #[test]
    fn passkey_register_then_sign_in_by_pseudonym() {
        let store = store();
        let player = PlayerId::new();
        let account = block(store.register_passkey(player, "attestation-1")).unwrap();
        assert_eq!(account.kind, AccountType::Passkey);
        // Sign in with the pseudonym + a matching assertion (stub: assertion == reg).
        let signed = block(store.sign_in(&SignInCredential::Passkey {
            pseudonym: account.display_name.clone(),
            assertion: "attestation-1".into(),
        }))
        .unwrap();
        assert_eq!(signed.id, account.id);
        // A bad assertion fails.
        let bad = block(store.sign_in(&SignInCredential::Passkey {
            pseudonym: account.display_name.clone(),
            assertion: "wrong".into(),
        }));
        assert!(matches!(bad, Err(AccountError::AuthFailed(_))));
        // An unknown pseudonym fails.
        let missing = block(store.sign_in(&SignInCredential::Passkey {
            pseudonym: "nobody-here-now".into(),
            assertion: "x".into(),
        }));
        assert!(matches!(missing, Err(AccountError::AuthFailed(_))));
    }

    #[test]
    fn rename_is_allowed_once_then_locks() {
        let store = store();
        let player = PlayerId::new();
        let (account, _) = block(store.create_device_account(player)).unwrap();
        let renamed = block(store.set_display_name(player, "spiced-amber-otter")).unwrap();
        assert_eq!(renamed.display_name, "spiced-amber-otter");
        assert_eq!(renamed.renames_remaining, 0);
        // The old name is freed; the new one is taken.
        assert!(!store.by_name.contains_key(&account.display_name));
        assert!(store.by_name.contains_key("spiced-amber-otter"));
        // A second rename is locked.
        assert_eq!(
            block(store.set_display_name(player, "molten-jade-lynx")),
            Err(AccountError::RenameLocked)
        );
    }

    #[test]
    fn rename_rejects_taken_and_malformed_names() {
        let store = store();
        let (a, _) = block(store.create_device_account(PlayerId::new())).unwrap();
        let p2 = PlayerId::new();
        block(store.create_device_account(p2)).unwrap();
        // Taking another account's name fails.
        assert_eq!(
            block(store.set_display_name(p2, &a.display_name)),
            Err(AccountError::NameUnavailable)
        );
        // Malformed names fail.
        for bad in [
            "ab",
            "Has-Caps",
            "trailing-",
            "double--hyphen",
            "space here",
        ] {
            assert_eq!(
                block(store.set_display_name(p2, bad)),
                Err(AccountError::NameUnavailable),
                "{bad}"
            );
        }
    }

    #[test]
    fn delete_account_erases_identity() {
        let store = store();
        let player = PlayerId::new();
        let (account, token) = block(store.create_device_account(player)).unwrap();
        let token = token.unwrap();
        let deleted = block(store.delete_account(player)).expect("deleted");
        assert_eq!(deleted.id, account.id);
        assert!(!store.is_registered(player));
        assert!(store.account(account.id).is_none());
        assert!(
            !store.by_name.contains_key(&account.display_name),
            "name freed"
        );
        // The device token no longer resolves.
        assert!(matches!(
            block(store.sign_in(&SignInCredential::Device {
                account_token: token
            })),
            Err(AccountError::AuthFailed(_))
        ));
        // Deleting again is a no-op.
        assert!(block(store.delete_account(player)).is_none());
    }

    /// A trivial executor for the store's futures in unit tests (they complete
    /// without real I/O under the stub verifiers and `pool: None`).
    fn block<F: std::future::Future>(fut: F) -> F::Output {
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
