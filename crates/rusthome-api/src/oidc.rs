//! OpenID Connect login against Authelia (cf. spec §8.2).
//!
//! Authorization code flow with PKCE; `state` and `nonce` are checked; the
//! resulting session lives in memory (a restart asks users to log in again).
//! Roles come from `preferred_username` through an explicit mapping: an
//! unknown user is refused.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use futures_util::future::BoxFuture;
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, IssuerUrl, Nonce, OAuth2TokenResponse as _, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope, TokenResponse as _,
};
use url::Url;

use crate::{Role, User};

/// Data kept between the redirect to Authelia and its callback.
#[derive(Debug, Clone)]
pub struct PendingLogin {
    pub csrf: String,
    pub pkce_verifier: String,
    pub nonce: String,
    /// Where to send the user back after login (an app path, never a URL).
    pub next: String,
}

/// Verified identity returned by the provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub username: String,
    pub display_name: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum OidcError {
    #[error("OIDC configuration: {0}")]
    Config(String),
    #[error("OIDC discovery failed: {0}")]
    Discovery(String),
    #[error("token exchange failed: {0}")]
    Exchange(String),
    #[error("invalid ID token: {0}")]
    InvalidToken(String),
}

/// What the HTTP layer needs from a provider. Implemented by [`OpenIdProvider`]
/// in production and by a fake in tests.
pub trait IdentityProvider: Send + Sync {
    fn authorize(&self, next: String) -> (Url, PendingLogin);
    fn finish(
        &self,
        code: String,
        pending: PendingLogin,
    ) -> BoxFuture<'_, Result<Identity, OidcError>>;
}

type DiscoveredClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

pub struct OpenIdProvider {
    client: DiscoveredClient,
    http: openidconnect::reqwest::Client,
}

impl OpenIdProvider {
    /// Discovers the provider (`/.well-known/openid-configuration`).
    pub async fn discover(
        issuer: &str,
        client_id: &str,
        client_secret: &str,
        redirect: &str,
    ) -> Result<Self, OidcError> {
        let http = openidconnect::reqwest::ClientBuilder::new()
            // No redirects: required to avoid SSRF through the provider (openidconnect docs).
            .redirect(openidconnect::reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| OidcError::Config(e.to_string()))?;
        let issuer =
            IssuerUrl::new(issuer.to_owned()).map_err(|e| OidcError::Config(e.to_string()))?;
        let metadata = CoreProviderMetadata::discover_async(issuer, &http)
            .await
            .map_err(|e| OidcError::Discovery(e.to_string()))?;
        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(client_id.to_owned()),
            Some(ClientSecret::new(client_secret.to_owned())),
        )
        .set_redirect_uri(
            RedirectUrl::new(redirect.to_owned()).map_err(|e| OidcError::Config(e.to_string()))?,
        );
        Ok(Self { client, http })
    }
}

impl IdentityProvider for OpenIdProvider {
    fn authorize(&self, next: String) -> (Url, PendingLogin) {
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, csrf, nonce) = self
            .client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("profile".into()))
            .set_pkce_challenge(challenge)
            .url();
        let pending = PendingLogin {
            csrf: csrf.secret().clone(),
            pkce_verifier: verifier.secret().clone(),
            nonce: nonce.secret().clone(),
            next,
        };
        (url, pending)
    }

    fn finish(
        &self,
        code: String,
        pending: PendingLogin,
    ) -> BoxFuture<'_, Result<Identity, OidcError>> {
        Box::pin(async move {
            let response = self
                .client
                .exchange_code(AuthorizationCode::new(code))
                .map_err(|e| OidcError::Config(e.to_string()))?
                .set_pkce_verifier(PkceCodeVerifier::new(pending.pkce_verifier))
                .request_async(&self.http)
                .await
                .map_err(|e| OidcError::Exchange(e.to_string()))?;
            let _ = response.access_token(); // not used: identity comes from the ID token
            let id_token = response
                .id_token()
                .ok_or_else(|| OidcError::InvalidToken("no ID token in response".into()))?;
            let claims = id_token
                .claims(&self.client.id_token_verifier(), &Nonce::new(pending.nonce))
                .map_err(|e| OidcError::InvalidToken(e.to_string()))?;
            let username = claims
                .preferred_username()
                .map(|u| u.as_str().to_owned())
                .ok_or_else(|| OidcError::InvalidToken("missing preferred_username".into()))?;
            let display_name = claims
                .name()
                .and_then(|n| n.get(None))
                .map(|n| n.as_str().to_owned());
            Ok(Identity {
                username,
                display_name,
            })
        })
    }
}

/// Maps usernames to roles. Spec §8.3 default: admin → admin, others listed → resident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleMap(HashMap<String, Role>);

impl RoleMap {
    /// Parses `admin=admin,hugo=resident,nspanel=display`.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut map = HashMap::new();
        for pair in raw.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (user, role) = pair
                .split_once('=')
                .ok_or_else(|| format!("`{pair}`: expected user=role"))?;
            let role = match role.trim() {
                "admin" => Role::Admin,
                "resident" => Role::Resident,
                "display" => Role::Display,
                other => return Err(format!("`{other}`: unknown role")),
            };
            map.insert(user.trim().to_owned(), role);
        }
        Ok(Self(map))
    }

    #[must_use]
    pub fn role_of(&self, username: &str) -> Option<Role> {
        self.0.get(username).copied()
    }
}

struct Session {
    user: User,
    expires: Instant,
}

/// Login state shared by the HTTP handlers.
pub struct OidcAuth {
    pub(crate) provider: Arc<dyn IdentityProvider>,
    pub(crate) roles: RoleMap,
    /// `true` when served over HTTPS: cookies get the `Secure` flag.
    pub(crate) secure_cookies: bool,
    pending: Mutex<HashMap<String, (PendingLogin, Instant)>>,
    sessions: Mutex<HashMap<String, Session>>,
    session_ttl: Duration,
}

pub(crate) const PENDING_TTL: Duration = Duration::from_secs(600);

impl OidcAuth {
    #[must_use]
    pub fn new(provider: Arc<dyn IdentityProvider>, roles: RoleMap, secure_cookies: bool) -> Self {
        Self {
            provider,
            roles,
            secure_cookies,
            pending: Mutex::default(),
            sessions: Mutex::default(),
            session_ttl: Duration::from_hours(7 * 24),
        }
    }

    pub(crate) fn remember_pending(&self, p: PendingLogin) {
        let mut map = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        map.retain(|_, (_, at)| now.duration_since(*at) < PENDING_TTL);
        map.insert(p.csrf.clone(), (p, now));
    }

    /// One-shot: a `state` can only be used once.
    pub(crate) fn take_pending(&self, csrf: &str) -> Option<PendingLogin> {
        let mut map = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        let (p, at) = map.remove(csrf)?;
        (at.elapsed() < PENDING_TTL).then_some(p)
    }

    pub(crate) fn open_session(&self, user: User) -> String {
        let id = CsrfToken::new_random().secret().clone(); // 32 random bytes, base64url
        let mut map = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        map.retain(|_, s| s.expires > now);
        map.insert(
            id.clone(),
            Session {
                user,
                expires: now + self.session_ttl,
            },
        );
        id
    }

    pub(crate) fn session_user(&self, id: &str) -> Option<User> {
        let map = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        map.get(id)
            .filter(|s| s.expires > Instant::now())
            .map(|s| s.user.clone())
    }

    pub(crate) fn close_session(&self, id: &str) {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(id);
    }
}

/// Only app-relative paths are accepted as `next` (no open redirect).
pub(crate) fn safe_next(next: Option<&str>) -> String {
    match next {
        Some(n) if n.starts_with('/') && !n.starts_with("//") && !n.contains('\\') => n.to_owned(),
        _ => "/".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_role_map() {
        let m = RoleMap::parse("admin=admin, hugo=resident ,nspanel=display").unwrap();
        assert_eq!(m.role_of("admin"), Some(Role::Admin));
        assert_eq!(m.role_of("hugo"), Some(Role::Resident));
        assert_eq!(m.role_of("nspanel"), Some(Role::Display));
        assert_eq!(m.role_of("intrus"), None);
        assert!(RoleMap::parse("hugo").is_err());
        assert!(RoleMap::parse("hugo=root").is_err());
    }

    #[test]
    fn next_is_never_an_external_url() {
        assert_eq!(
            safe_next(Some("/entites?etat=indisponible")),
            "/entites?etat=indisponible"
        );
        for bad in ["https://evil.example", "//evil.example", "/\\evil", "evil"] {
            assert_eq!(safe_next(Some(bad)), "/", "{bad}");
        }
        assert_eq!(safe_next(None), "/");
    }
}
