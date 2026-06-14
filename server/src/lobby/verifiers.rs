//! Production credential verifiers (`boom2-identity`) — the network/crypto
//! implementations behind the [`super::accounts`] verifier seams. Constructed
//! only when their method is configured, so the headless tests (which use stub
//! verifiers) never touch the network.
//!
//! **OAuth** ([`HttpOAuthVerifier`]): Google/Apple/Microsoft are OpenID Connect,
//! verified by validating the **id token** (a JWT) against the provider's JWKS
//! with `jsonwebtoken` — signature, issuer, audience (our client id), expiry —
//! and reading **only** the `sub`. Apple in particular has no userinfo endpoint,
//! so id-token verification is the only option. Discord is plain OAuth2: a
//! `users/@me` call with the access token yields its `id`. No profile scopes are
//! requested and no name/email is ever read.
//!
//! **Passkey:** the WebAuthn ceremony (server-issued challenge → client
//! `navigator.credentials` → assertion → `webauthn-rs` verification) is inherently
//! client-coupled and lands with the web client (`adopt-pixi-client`), like the
//! OAuth *client* flow. The server-side account model and the
//! [`super::PasskeyVerifier`] seam ship here; the production verifier is wired in
//! at that point. Until then the server runs [`super::accounts::DisabledPasskeyVerifier`].

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use tokio::sync::RwLock;

use boiling_point_protocol::OAuthProvider;

use super::accounts::{OAuthIdentity, OAuthVerifier};

/// Per-provider OIDC parameters: the issuer (`iss` claim), the JWKS endpoint, and
/// the expected audience (our registered client id for that provider).
struct OidcProvider {
    issuer: &'static str,
    jwks_uri: &'static str,
    audience: String,
}

/// The minimal id-token claims we read: subject, audience, issuer. No name/email.
#[derive(Debug, Deserialize)]
struct IdTokenClaims {
    sub: String,
}

/// The production OAuth verifier. Holds the configured per-provider client ids
/// and a small JWKS cache (refreshed on an unknown key id).
pub struct HttpOAuthVerifier {
    client: reqwest::Client,
    /// Configured OIDC providers (those with a client id set). Discord is keyed
    /// separately since it is not OIDC.
    oidc: HashMap<&'static str, OidcProvider>,
    /// Cached JWKS per jwks_uri.
    jwks: RwLock<HashMap<&'static str, JwkSet>>,
}

/// Which providers are enabled, by their configured client ids (audience). A
/// provider with no client id configured is rejected at verification time.
#[derive(Debug, Clone, Default)]
pub struct OAuthConfig {
    /// Google OIDC client id (audience).
    pub google_client_id: Option<String>,
    /// Apple OIDC client id (the Services ID / audience).
    pub apple_client_id: Option<String>,
    /// Microsoft OIDC client id (audience).
    pub microsoft_client_id: Option<String>,
    /// Whether Discord (OAuth2 userinfo) sign-in is enabled.
    pub discord_enabled: bool,
}

impl OAuthConfig {
    /// Read the config from environment variables (`BP_OAUTH_*`). Absent values
    /// leave that provider disabled.
    pub fn from_env() -> Self {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        OAuthConfig {
            google_client_id: var("BP_OAUTH_GOOGLE_CLIENT_ID"),
            apple_client_id: var("BP_OAUTH_APPLE_CLIENT_ID"),
            microsoft_client_id: var("BP_OAUTH_MICROSOFT_CLIENT_ID"),
            discord_enabled: var("BP_OAUTH_DISCORD_ENABLED").is_some(),
        }
    }

    /// Whether any provider is enabled.
    pub fn any_enabled(&self) -> bool {
        self.google_client_id.is_some()
            || self.apple_client_id.is_some()
            || self.microsoft_client_id.is_some()
            || self.discord_enabled
    }
}

impl HttpOAuthVerifier {
    /// Build a verifier for the configured providers.
    pub fn new(config: OAuthConfig) -> Self {
        let mut oidc = HashMap::new();
        if let Some(audience) = config.google_client_id {
            oidc.insert(
                "google",
                OidcProvider {
                    issuer: "https://accounts.google.com",
                    jwks_uri: "https://www.googleapis.com/oauth2/v3/certs",
                    audience,
                },
            );
        }
        if let Some(audience) = config.apple_client_id {
            oidc.insert(
                "apple",
                OidcProvider {
                    issuer: "https://appleid.apple.com",
                    jwks_uri: "https://appleid.apple.com/auth/keys",
                    audience,
                },
            );
        }
        if let Some(audience) = config.microsoft_client_id {
            oidc.insert(
                "microsoft",
                OidcProvider {
                    // Microsoft is multi-tenant; the issuer carries the tenant id,
                    // so issuer validation is relaxed and audience is the anchor.
                    issuer: "",
                    jwks_uri: "https://login.microsoftonline.com/common/discovery/v2.0/keys",
                    audience,
                },
            );
        }
        HttpOAuthVerifier {
            client: reqwest::Client::new(),
            oidc,
            jwks: RwLock::new(HashMap::new()),
        }
    }

    /// Fetch (and cache) the JWKS for a provider, refreshing if `force`.
    async fn jwks_for(&self, jwks_uri: &'static str, force: bool) -> Result<JwkSet, String> {
        if !force && let Some(set) = self.jwks.read().await.get(jwks_uri) {
            return Ok(set.clone());
        }
        let set: JwkSet = self
            .client
            .get(jwks_uri)
            .send()
            .await
            .map_err(|e| format!("JWKS fetch failed: {e}"))?
            .json()
            .await
            .map_err(|e| format!("JWKS parse failed: {e}"))?;
        self.jwks.write().await.insert(jwks_uri, set.clone());
        Ok(set)
    }

    /// Verify an OIDC id token and return its subject.
    async fn verify_oidc(&self, label: &'static str, token: &str) -> Result<String, String> {
        let provider = self
            .oidc
            .get(label)
            .ok_or_else(|| format!("{label} sign-in is not configured"))?;
        let header = decode_header(token).map_err(|e| format!("bad id token header: {e}"))?;
        let kid = header.kid.ok_or("id token has no key id")?;

        // Try the cached JWKS, then a forced refresh on a miss (key rotation).
        let mut last_err = String::new();
        for force in [false, true] {
            let set = self.jwks_for(provider.jwks_uri, force).await?;
            let Some(jwk) = set.find(&kid) else {
                last_err = format!("no JWKS key for kid {kid}");
                continue;
            };
            let key = DecodingKey::from_jwk(jwk).map_err(|e| format!("bad JWKS key: {e}"))?;
            let mut validation = Validation::new(header.alg);
            validation.set_audience(&[&provider.audience]);
            if provider.issuer.is_empty() {
                validation.validate_aud = true;
                validation.iss = None; // Microsoft: tenant-scoped issuer; audience is the anchor.
            } else {
                validation.set_issuer(&[provider.issuer]);
            }
            return decode::<IdTokenClaims>(token, &key, &validation)
                .map(|data| data.claims.sub)
                .map_err(|e| format!("id token rejected: {e}"));
        }
        Err(last_err)
    }

    /// Verify a Discord access token via `users/@me` and return its `id`.
    async fn verify_discord(&self, access_token: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        struct DiscordUser {
            id: String,
        }
        let resp = self
            .client
            .get("https://discord.com/api/users/@me")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| format!("Discord request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!(
                "Discord rejected the token: HTTP {}",
                resp.status()
            ));
        }
        let user: DiscordUser = resp
            .json()
            .await
            .map_err(|e| format!("Discord response was not JSON: {e}"))?;
        Ok(user.id)
    }
}

#[async_trait]
impl OAuthVerifier for HttpOAuthVerifier {
    async fn verify(&self, provider: OAuthProvider, token: &str) -> Result<OAuthIdentity, String> {
        let subject = match provider {
            OAuthProvider::Google => self.verify_oidc("google", token).await?,
            OAuthProvider::Apple => self.verify_oidc("apple", token).await?,
            OAuthProvider::Microsoft => self.verify_oidc("microsoft", token).await?,
            OAuthProvider::Discord => self.verify_discord(token).await?,
        };
        if subject.is_empty() {
            return Err("provider returned an empty subject".into());
        }
        Ok(OAuthIdentity { provider, subject })
    }
}

/// Build the configured OAuth verifier, or `None` if no provider is enabled.
pub fn oauth_verifier(config: OAuthConfig) -> Option<Arc<dyn OAuthVerifier>> {
    config
        .any_enabled()
        .then(|| Arc::new(HttpOAuthVerifier::new(config)) as Arc<dyn OAuthVerifier>)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_enables_only_configured_providers() {
        let empty = OAuthConfig::default();
        assert!(!empty.any_enabled());
        let v = HttpOAuthVerifier::new(empty);
        assert!(v.oidc.is_empty());

        let cfg = OAuthConfig {
            google_client_id: Some("g-client".into()),
            apple_client_id: Some("a-client".into()),
            microsoft_client_id: None,
            discord_enabled: true,
        };
        assert!(cfg.any_enabled());
        let v = HttpOAuthVerifier::new(cfg);
        assert!(v.oidc.contains_key("google"));
        assert!(v.oidc.contains_key("apple"));
        assert!(!v.oidc.contains_key("microsoft"));
    }

    /// An unconfigured OIDC provider is rejected without any network call.
    #[tokio::test]
    async fn unconfigured_oidc_provider_is_rejected() {
        let v = HttpOAuthVerifier::new(OAuthConfig::default());
        let err = v.verify(OAuthProvider::Google, "any.jwt.here").await;
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("not configured"));
    }
}
