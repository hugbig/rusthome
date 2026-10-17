//! HTTP API: REST + WebSocket (cf. spec §8.1).
//!
//! M1: read endpoints (states, registries, history), live WebSocket, and
//! authentication (`dev` mode or OIDC through Authelia).

mod auth;
pub mod oidc;
mod ws;

use std::path::PathBuf;

use axum::extract::{Path, Query, State as AxState};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router, middleware};
use chrono::{DateTime, Duration, Utc};
use rusthome_core::Hub;
use rusthome_model::EntityId;
use rusthome_recorder::Recorder;
use serde::Deserialize;
use serde_json::json;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

pub use auth::{AuthMode, Role, User};

#[derive(Clone)]
pub struct ApiState {
    pub hub: Hub,
    pub recorder: Option<Recorder>,
    pub auth: AuthMode,
}

/// Builds the router. `static_dir`, when set, serves the built frontend
/// with SPA fallback to `index.html`.
pub fn router(state: ApiState, static_dir: Option<PathBuf>) -> Router {
    let protected = Router::new()
        .route("/me", get(me))
        .route("/states", get(states))
        .route("/states/{entity_id}", get(state_of))
        .route("/registry/{kind}", get(registry))
        .route("/history/{entity_id}", get(history))
        .route("/ws", get(ws::handler))
        .route("/services", get(list_services))
        .route(
            "/services/{domain}/{service}",
            axum::routing::post(call_service),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_user,
        ));
    // Public: health (container probes) and the login flow itself.
    let api = Router::new()
        .route("/health", get(health))
        .route("/auth/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .route("/auth/logout", get(auth::logout))
        .merge(protected)
        .with_state(state);

    let mut app = Router::new().nest("/api/v1", api);
    if let Some(dir) = static_dir {
        let index = dir.join("index.html");
        app = app.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)));
    }
    app.layer(TraceLayer::new_for_http())
}

enum ApiError {
    NotFound(String),
    BadRequest(String),
    Forbidden(String),
    /// The target refuses to act (read-only mode, entity driven by HA…).
    Conflict(String),
    Internal(String),
}

impl From<rusthome_core::ServiceError> for ApiError {
    fn from(e: rusthome_core::ServiceError) -> Self {
        use rusthome_core::ServiceError as E;
        match e {
            E::NotFound(m) => ApiError::NotFound(format!("unknown service {m}")),
            E::InvalidData(m) => ApiError::BadRequest(m),
            E::Refused(m) => ApiError::Conflict(m),
            E::Failed(m) => ApiError::Internal(m),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (code, msg) = match self {
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Forbidden(m) => (StatusCode::FORBIDDEN, m),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, m),
            ApiError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        };
        (code, Json(json!({ "error": msg }))).into_response()
    }
}

fn parse_id(raw: &str) -> Result<EntityId, ApiError> {
    EntityId::parse(raw).map_err(|e| ApiError::BadRequest(e.to_string()))
}

async fn health(AxState(s): AxState<ApiState>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "entities": s.hub.states.len(),
        "recorder": s.recorder.is_some(),
    }))
}

async fn me(axum::Extension(user): axum::Extension<User>) -> Json<User> {
    Json(user)
}

async fn states(AxState(s): AxState<ApiState>) -> Json<serde_json::Value> {
    Json(json!(
        s.hub.states.all().iter().map(|x| &**x).collect::<Vec<_>>()
    ))
}

async fn state_of(
    AxState(s): AxState<ApiState>,
    Path(raw): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = parse_id(&raw)?;
    let st = s
        .hub
        .states
        .get(&id)
        .ok_or_else(|| ApiError::NotFound(format!("{id} not found")))?;
    Ok(Json(json!(*st)))
}

async fn registry(
    AxState(s): AxState<ApiState>,
    Path(kind): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let r = &s.hub.registries;
    Ok(Json(match kind.as_str() {
        "floors" => json!(r.floors()),
        "areas" => json!(r.areas()),
        "labels" => json!(r.labels()),
        "devices" => json!(r.devices()),
        "entities" => json!(r.entities()),
        other => return Err(ApiError::NotFound(format!("unknown registry {other}"))),
    }))
}

async fn list_services(AxState(s): AxState<ApiState>) -> Json<serde_json::Value> {
    Json(json!(s.hub.services.list()))
}

/// `POST /api/v1/services/{domain}/{service}` with the service data as body.
async fn call_service(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path((domain, service)): Path<(String, String)>,
    body: Option<Json<serde_json::Value>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Spec §8.3: the wall display gets a whitelist later; no commands until then.
    if user.role == Role::Display {
        return Err(ApiError::Forbidden(
            "display accounts cannot send commands".into(),
        ));
    }
    let data = body.map_or_else(|| json!({}), |Json(v)| v);
    let context = rusthome_model::Context {
        id: format!("api-{}", s.hub.clock.now().timestamp_micros()),
        parent_id: None,
        user_id: Some(user.name.clone()),
    };
    let out = s
        .hub
        .call_service(rusthome_core::ServiceCall {
            domain,
            service,
            data,
            context,
        })
        .await?;
    Ok(Json(out))
}

#[derive(Deserialize)]
struct HistoryQuery {
    since: Option<DateTime<Utc>>,
}

async fn history(
    AxState(s): AxState<ApiState>,
    Path(raw): Path<String>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = parse_id(&raw)?;
    let rec = s
        .recorder
        .as_ref()
        .ok_or_else(|| ApiError::NotFound("recorder disabled".into()))?;
    let since = q
        .since
        .unwrap_or_else(|| s.hub.clock.now() - Duration::hours(24));
    let rows = rec
        .history(&id, since)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(json!(rows)))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use rusthome_core::SystemClock;
    use rusthome_model::{Area, Context};
    use serde_json::{Map, Value};
    use tower::ServiceExt;

    fn app(auth: AuthMode) -> (Router, Hub) {
        let hub = Hub::new(Arc::new(SystemClock));
        hub.states.set(
            EntityId::parse("light.salon").unwrap(),
            "on",
            Map::new(),
            Context::default(),
        );
        hub.registries.upsert_area(Area {
            id: "salon".into(),
            name: "Salon".into(),
            floor_id: None,
            icon: None,
            aliases: vec![],
        });
        (
            router(
                ApiState {
                    hub: hub.clone(),
                    recorder: None,
                    auth,
                },
                None,
            ),
            hub,
        )
    }

    async fn get_json(app: Router, uri: &str) -> (StatusCode, Value) {
        let res = app
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
    }

    #[tokio::test]
    async fn lists_states_and_registries() {
        let (app, _) = app(AuthMode::Dev);
        let (code, body) = get_json(app.clone(), "/api/v1/states").await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(body[0]["entity_id"], "light.salon");
        let (_, areas) = get_json(app.clone(), "/api/v1/registry/areas").await;
        assert_eq!(areas[0]["name"], "Salon");
        let (code, _) = get_json(app, "/api/v1/registry/nope").await;
        assert_eq!(code, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn single_state_and_errors() {
        let (app, _) = app(AuthMode::Dev);
        assert_eq!(
            get_json(app.clone(), "/api/v1/states/light.salon").await.1["state"],
            "on"
        );
        assert_eq!(
            get_json(app.clone(), "/api/v1/states/light.nope").await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            get_json(app, "/api/v1/states/Not%20An%20Id").await.0,
            StatusCode::BAD_REQUEST
        );
    }

    async fn post_json(app: Router, uri: &str, body: Value) -> (StatusCode, Value) {
        let req = Request::post(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    #[tokio::test]
    async fn calls_services_and_maps_errors() {
        use rusthome_core::{ServiceCall, ServiceError};
        let (app, hub) = app(AuthMode::Dev);
        hub.registries.upsert_entity(rusthome_model::EntityEntry {
            uid: "u".into(),
            entity_id: EntityId::parse("switch.prise").unwrap(),
            integration: "z2m".into(),
            unique_id: None,
            device_id: None,
            name: None,
            area_id: None,
            labels: vec![],
            icon: None,
            hidden: false,
            disabled: false,
            status: rusthome_model::EntityStatus::Active,
        });
        hub.services.register(
            "z2m",
            "switch",
            "turn_on",
            Arc::new(|c: ServiceCall| {
                Box::pin(async move {
                    if c.context.user_id.as_deref() == Some("hugo") {
                        Ok(serde_json::json!({"ok": true}))
                    } else {
                        Err(ServiceError::Failed("no user".into()))
                    }
                })
            }),
        );
        hub.services.register(
            "z2m",
            "switch",
            "turn_off",
            Arc::new(|_| {
                Box::pin(async { Err(ServiceError::Refused("MQTT is read-only".into())) })
            }),
        );
        let target = serde_json::json!({"entity_id": "switch.prise"});
        let (code, body) = post_json(
            app.clone(),
            "/api/v1/services/switch/turn_on",
            target.clone(),
        )
        .await;
        assert_eq!(
            (code, body["ok"].clone()),
            (StatusCode::OK, Value::Bool(true)),
            "caller is recorded in the context"
        );
        let (code, body) = post_json(
            app.clone(),
            "/api/v1/services/switch/turn_off",
            target.clone(),
        )
        .await;
        assert_eq!(code, StatusCode::CONFLICT);
        assert!(body["error"].as_str().unwrap().contains("read-only"));
        assert_eq!(
            post_json(app.clone(), "/api/v1/services/switch/nope", target)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            get_json(app, "/api/v1/services")
                .await
                .1
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn dev_user_is_admin_hugo() {
        let (app, _) = app(AuthMode::Dev);
        let (_, me) = get_json(app, "/api/v1/me").await;
        assert_eq!(me["name"], "hugo");
        assert_eq!(me["role"], "admin");
    }

    #[tokio::test]
    async fn without_auth_everything_but_health_is_refused() {
        let (app, _) = app(AuthMode::Disabled);
        assert_eq!(
            get_json(app.clone(), "/api/v1/states").await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            get_json(app.clone(), "/api/v1/me").await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(get_json(app, "/api/v1/health").await.0, StatusCode::OK);
    }
}
