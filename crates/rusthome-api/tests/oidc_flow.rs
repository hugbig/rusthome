//! Full login flow over HTTP with a fake identity provider (no network).

// Test harness: panicking on unexpected input is the desired behaviour.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use futures_util::future::BoxFuture;
use http_body_util::BodyExt;
use rusthome_api::oidc::{Identity, IdentityProvider, OidcAuth, OidcError, PendingLogin, RoleMap};
use rusthome_api::{ApiState, AuthMode, router};
use rusthome_core::{Hub, SystemClock};
use tower::ServiceExt;

struct FakeProvider {
    n: AtomicU32,
}

impl IdentityProvider for FakeProvider {
    fn authorize(&self, next: String) -> (url::Url, PendingLogin) {
        let csrf = format!("csrf-{}", self.n.fetch_add(1, Ordering::SeqCst));
        let url = url::Url::parse(&format!("https://auth.example/authorize?state={csrf}")).unwrap();
        (
            url,
            PendingLogin {
                csrf,
                pkce_verifier: "v".into(),
                nonce: "n".into(),
                next,
            },
        )
    }

    fn finish(
        &self,
        code: String,
        pending: PendingLogin,
    ) -> BoxFuture<'_, Result<Identity, OidcError>> {
        Box::pin(async move {
            assert_eq!(
                pending.pkce_verifier, "v",
                "PKCE verifier travels to the exchange"
            );
            match code.as_str() {
                "code-hugo" => Ok(Identity {
                    username: "hugo".into(),
                    display_name: None,
                }),
                "code-intrus" => Ok(Identity {
                    username: "intrus".into(),
                    display_name: None,
                }),
                _ => Err(OidcError::Exchange("bad code".into())),
            }
        })
    }
}

fn app() -> Router {
    let oidc = OidcAuth::new(
        Arc::new(FakeProvider {
            n: AtomicU32::new(0),
        }),
        RoleMap::parse("admin=admin,hugo=resident,sokhna=resident").unwrap(),
        true,
    );
    let hub = Hub::new(Arc::new(SystemClock));
    router(
        ApiState {
            hub,
            recorder: None,
            auth: AuthMode::Oidc(Arc::new(oidc)),
            automations: None,
            dashboards: None,
            system: serde_json::Value::Null,
            display_dashboard: None,
            zigbee: None,
            frigate: None,
        },
        None,
    )
}

async fn get(
    app: &Router,
    uri: &str,
    cookie: Option<&str>,
) -> (StatusCode, Option<String>, Option<String>, String) {
    let mut req = Request::get(uri);
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    let res = app
        .clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let h = |n| res.headers().get(n).map(|v| v.to_str().unwrap().to_owned());
    let (status, location, set_cookie) = (res.status(), h(header::LOCATION), h(header::SET_COOKIE));
    let body =
        String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    (status, location, set_cookie, body)
}

#[tokio::test]
async fn login_callback_session_and_logout() {
    let app = app();
    assert_eq!(
        get(&app, "/api/v1/me", None).await.0,
        StatusCode::UNAUTHORIZED
    );

    let (status, location, ..) = get(&app, "/api/v1/auth/login?next=/entites", None).await;
    assert!(status.is_redirection());
    assert_eq!(
        location.as_deref(),
        Some("https://auth.example/authorize?state=csrf-0")
    );

    let (status, location, cookie, _) = get(
        &app,
        "/api/v1/auth/callback?code=code-hugo&state=csrf-0",
        None,
    )
    .await;
    assert!(status.is_redirection());
    assert_eq!(
        location.as_deref(),
        Some("/entites"),
        "back to the requested page"
    );
    let cookie = cookie.unwrap();
    assert!(cookie.starts_with("rusthome_session="));
    for flag in ["HttpOnly", "Secure", "SameSite=Lax", "Path=/"] {
        assert!(cookie.contains(flag), "cookie misses {flag}: {cookie}");
    }
    let session = cookie.split(';').next().unwrap();

    let (status, _, _, body) = get(&app, "/api/v1/me", Some(session)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("\"name\":\"hugo\"") && body.contains("\"role\":\"resident\""),
        "{body}"
    );

    // A state can only be used once (replay).
    assert_eq!(
        get(
            &app,
            "/api/v1/auth/callback?code=code-hugo&state=csrf-0",
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );

    let (status, _, cleared, _) = get(&app, "/api/v1/auth/logout", Some(session)).await;
    assert!(status.is_redirection());
    assert!(cleared.unwrap().starts_with("rusthome_session="));
    assert_eq!(
        get(&app, "/api/v1/me", Some(session)).await.0,
        StatusCode::UNAUTHORIZED,
        "session closed"
    );
}

#[tokio::test]
async fn refusals() {
    let app = app();
    // Unknown user: authenticated by Authelia but no RustHome role.
    get(&app, "/api/v1/auth/login", None).await;
    let (status, _, cookie, _) = get(
        &app,
        "/api/v1/auth/callback?code=code-intrus&state=csrf-0",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(cookie.is_none());

    // Forged state.
    assert_eq!(
        get(
            &app,
            "/api/v1/auth/callback?code=code-hugo&state=forged",
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );

    // Provider rejects the code.
    get(&app, "/api/v1/auth/login", None).await;
    assert_eq!(
        get(&app, "/api/v1/auth/callback?code=nope&state=csrf-1", None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    // Error returned by the provider (user cancelled).
    assert_eq!(
        get(&app, "/api/v1/auth/callback?error=access_denied", None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    // Open redirect attempt: `next` is forced back to the app.
    get(&app, "/api/v1/auth/login?next=https://evil.example", None).await;
    let (_, location, ..) = get(
        &app,
        "/api/v1/auth/callback?code=code-hugo&state=csrf-2",
        None,
    )
    .await;
    assert_eq!(location.as_deref(), Some("/"));

    // Forged session cookie.
    assert_eq!(
        get(&app, "/api/v1/me", Some("rusthome_session=forged"))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}
