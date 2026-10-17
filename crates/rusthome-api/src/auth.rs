use std::sync::Arc;

use axum::extract::{Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};

use crate::ApiState;
use crate::oidc::{OidcAuth, safe_next};

/// Roles of spec §8.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Resident,
    Display,
}

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub name: String,
    pub role: Role,
}

#[derive(Clone)]
pub enum AuthMode {
    /// Every request is user `hugo` (admin). Refused in production by the binary.
    Dev,
    /// No authentication configured: everything is refused (safe default).
    Disabled,
    /// Login through Authelia (OpenID Connect).
    Oidc(Arc<OidcAuth>),
}

pub(crate) const SESSION_COOKIE: &str = "rusthome_session";

pub(crate) async fn require_user(
    State(state): State<ApiState>,
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> Response {
    let user = match &state.auth {
        AuthMode::Dev => Some(User {
            name: "hugo".into(),
            role: Role::Admin,
        }),
        AuthMode::Disabled => None,
        AuthMode::Oidc(oidc) => jar
            .get(SESSION_COOKIE)
            .and_then(|c| oidc.session_user(c.value())),
    };
    match user {
        Some(u) => {
            req.extensions_mut().insert(u);
            next.run(req).await
        }
        None => (StatusCode::UNAUTHORIZED, "authentication required").into_response(),
    }
}

#[derive(Deserialize)]
pub(crate) struct LoginQuery {
    next: Option<String>,
}

/// `GET /api/v1/auth/login`: redirects to Authelia.
pub(crate) async fn login(State(state): State<ApiState>, Query(q): Query<LoginQuery>) -> Response {
    match &state.auth {
        AuthMode::Oidc(oidc) => {
            let (url, pending) = oidc.provider.authorize(safe_next(q.next.as_deref()));
            oidc.remember_pending(pending);
            Redirect::to(url.as_str()).into_response()
        }
        // Dev: already "logged in"; Disabled: nothing to log in to.
        AuthMode::Dev => Redirect::to(&safe_next(q.next.as_deref())).into_response(),
        AuthMode::Disabled => {
            (StatusCode::NOT_FOUND, "authentication not configured").into_response()
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// `GET /api/v1/auth/callback`: Authelia sends the user back here.
pub(crate) async fn callback(
    State(state): State<ApiState>,
    jar: CookieJar,
    Query(q): Query<CallbackQuery>,
) -> Response {
    let AuthMode::Oidc(oidc) = &state.auth else {
        return (StatusCode::NOT_FOUND, "OIDC not enabled").into_response();
    };
    if let Some(err) = q.error {
        return (
            StatusCode::UNAUTHORIZED,
            format!("login refused by provider: {err}"),
        )
            .into_response();
    }
    let (Some(code), Some(csrf)) = (q.code, q.state) else {
        return (StatusCode::BAD_REQUEST, "missing code or state").into_response();
    };
    let Some(pending) = oidc.take_pending(&csrf) else {
        return (StatusCode::BAD_REQUEST, "unknown or expired login attempt").into_response();
    };
    let next = pending.next.clone();
    let identity = match oidc.provider.finish(code, pending).await {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!(error = %e, "OIDC login failed");
            return (StatusCode::UNAUTHORIZED, "login failed").into_response();
        }
    };
    let Some(role) = oidc.roles.role_of(&identity.username) else {
        tracing::warn!(user = %identity.username, "OIDC login refused: user has no RustHome role");
        return (
            StatusCode::FORBIDDEN,
            "this account has no access to RustHome",
        )
            .into_response();
    };
    tracing::info!(user = %identity.username, ?role, "user logged in");
    let session = oidc.open_session(User {
        name: identity.username,
        role,
    });
    let cookie = Cookie::build((SESSION_COOKIE, session))
        .path("/")
        .http_only(true)
        .secure(oidc.secure_cookies)
        .same_site(SameSite::Lax)
        .build();
    (jar.add(cookie), Redirect::to(&next)).into_response()
}

/// `GET /api/v1/auth/logout`: ends the RustHome session.
pub(crate) async fn logout(State(state): State<ApiState>, jar: CookieJar) -> Response {
    if let (AuthMode::Oidc(oidc), Some(c)) = (&state.auth, jar.get(SESSION_COOKIE)) {
        oidc.close_session(c.value());
    }
    (
        jar.remove(Cookie::build(SESSION_COOKIE).path("/")),
        Redirect::to("/"),
    )
        .into_response()
}
