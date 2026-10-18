//! HTTP API: REST + WebSocket (cf. spec §8.1).
//!
//! M1: read endpoints (states, registries, history), live WebSocket, and
//! authentication (`dev` mode or OIDC through Authelia).

mod auth;
pub mod automations;
pub mod dashboards;
pub mod esphome;
mod media;
pub mod oidc;
mod panels;
mod ws;
mod zigbee;

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
    pub automations: Option<automations::AutomationsCtx>,
    pub dashboards: Option<dashboards::DashboardsCtx>,
    /// What the binary reports about itself for Paramètres > Système
    /// (sources, read-only flags…). No secret goes in there.
    pub system: serde_json::Value,
    /// Dashboard imposed on `display` accounts (the wall tablet).
    pub display_dashboard: Option<String>,
    /// Zigbee2MQTT management (pairing, renaming, OTA, groups, bindings).
    pub zigbee: Option<std::sync::Arc<rusthome_z2m::Z2m>>,
    /// Frigate media (events, clips, recordings) for the media browser.
    pub frigate: Option<std::sync::Arc<rusthome_frigate::Frigate>>,
    /// ESPHome projects through the ESPHome Builder container.
    pub esphome: Option<std::sync::Arc<esphome::EsphomeCtx>>,
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
        .route("/logbook", get(logbook))
        .route("/system", get(system))
        .route("/ws", get(ws::handler))
        .route(
            "/automations",
            get(automations::list).post(automations::create),
        )
        .route(
            "/automations/import-ha",
            axum::routing::post(automations::import_ha),
        )
        .route(
            "/automations/{id}",
            get(automations::get_one)
                .put(automations::put_one)
                .delete(automations::delete_one),
        )
        .route(
            "/automations/{id}/enable",
            axum::routing::post(automations::enable),
        )
        .route(
            "/automations/{id}/disable",
            axum::routing::post(automations::disable),
        )
        .route(
            "/automations/{id}/run",
            axum::routing::post(automations::run_now),
        )
        .route("/automations/{id}/traces", get(automations::traces))
        .route(
            "/dashboards",
            get(dashboards::list).post(dashboards::create),
        )
        .route(
            "/settings/default-dashboard",
            axum::routing::put(dashboards::set_default),
        )
        .route(
            "/dashboards/import-ha",
            axum::routing::post(dashboards::import_ha),
        )
        .route(
            "/dashboards/{url_path}",
            get(dashboards::get_one)
                .put(dashboards::put_one)
                .delete(dashboards::delete_one),
        )
        .route(
            "/cameras/{entity_id}/snapshot",
            get(dashboards::camera_snapshot),
        )
        .route(
            "/calendars/{entity_id}/events",
            get(dashboards::calendar_events),
        )
        .route("/todo/{entity_id}/items", get(dashboards::todo_items))
        .route("/media/{entity_id}/image", get(dashboards::media_image))
        .route("/weather/{entity_id}/forecast", get(panels::forecast))
        .route("/energy/prefs", get(panels::energy_prefs))
        .route("/energy/statistics", get(panels::energy_statistics))
        .merge(media::routes())
        .merge(zigbee::routes())
        .merge(esphome::routes())
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

pub(crate) enum ApiError {
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

/// Refuses cross-site WebSocket upgrades (CSWSH): browsers always send
/// `Origin`, and a WebSocket handshake is not covered by CORS. The page must
/// come from the host it talks to; clients without `Origin` (scripts,
/// tests) are not browsers and carry no ambient credentials.
pub(crate) fn same_origin(headers: &axum::http::HeaderMap) -> bool {
    use axum::http::header::{HOST, ORIGIN};
    let Some(origin) = headers.get(ORIGIN) else {
        return true;
    };
    let host = headers.get(HOST).and_then(|h| h.to_str().ok());
    let origin_host = origin
        .to_str()
        .ok()
        .and_then(|o| url::Url::parse(o).ok())
        .and_then(|u| {
            let h = u.host_str()?.to_owned();
            Some(u.port().map_or(h.clone(), |p| format!("{h}:{p}")))
        });
    matches!((origin_host, host), (Some(o), Some(h)) if o.eq_ignore_ascii_case(h))
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

async fn system(AxState(s): AxState<ApiState>) -> Json<serde_json::Value> {
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "entities": s.hub.states.len(),
        "recorder": s.recorder.is_some(),
        "automations": s.automations.is_some(),
        "dashboards": s.dashboards.is_some(),
        "sources": s.system,
    }))
}

/// Settings key of the default dashboard.
pub const DEFAULT_DASHBOARD_KEY: &str = "default_dashboard";

/// The current user; `home` is the dashboard a `display` account is held to,
/// `default_dashboard` the one opened at `/`.
async fn me(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
) -> Json<serde_json::Value> {
    let home = (user.role == Role::Display)
        .then(|| s.display_dashboard.clone())
        .flatten();
    let default_dashboard = dashboards::default_for(&s, &user);
    Json(json!({
        "name": user.name, "role": user.role, "home": home, "default_dashboard": default_dashboard
    }))
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

#[derive(Deserialize)]
struct LogbookQuery {
    hours: Option<i64>,
    entity_id: Option<String>,
    limit: Option<u32>,
}

/// State changes of every entity (or one), newest first.
async fn logbook(
    AxState(s): AxState<ApiState>,
    Query(q): Query<LogbookQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rec = s
        .recorder
        .as_ref()
        .ok_or_else(|| ApiError::NotFound("recorder disabled".into()))?;
    let id = q.entity_id.as_deref().map(parse_id).transpose()?;
    let since = s.hub.clock.now() - Duration::hours(q.hours.unwrap_or(24).clamp(1, 24 * 30));
    let rows = rec
        .logbook(since, id.as_ref(), q.limit.unwrap_or(500).min(5000))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(json!(rows)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn websocket_upgrades_must_be_same_origin() {
        use axum::http::{HeaderMap, HeaderValue, header};
        let h = |origin: Option<&'static str>, host: &'static str| {
            let mut m = HeaderMap::new();
            m.insert(header::HOST, HeaderValue::from_static(host));
            if let Some(o) = origin {
                m.insert(header::ORIGIN, HeaderValue::from_static(o));
            }
            m
        };
        assert!(super::same_origin(&h(
            Some("https://maison.example.org"),
            "maison.example.org"
        )));
        assert!(super::same_origin(&h(
            Some("http://localhost:5173"),
            "localhost:5173"
        )));
        assert!(super::same_origin(&h(None, "127.0.0.1:8124")));
        assert!(!super::same_origin(&h(
            Some("https://evil.example.org"),
            "maison.example.org"
        )));
        assert!(!super::same_origin(&h(
            Some("http://localhost:5173"),
            "127.0.0.1:8124"
        )));
        assert!(!super::same_origin(&h(Some("null"), "maison.example.org")));
    }

    use std::sync::Arc;

    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use rusthome_core::SystemClock;
    use rusthome_model::{Area, Context};
    use serde_json::{Map, Value};
    use tower::ServiceExt;

    struct FakeCameras;

    impl dashboards::CameraSource for FakeCameras {
        fn snapshot<'a>(&'a self, entity_id: &'a EntityId) -> dashboards::ImageFuture<'a> {
            Box::pin(async move {
                if entity_id.as_str() == "camera.salon" {
                    Ok(("image/jpeg".to_owned(), vec![0xff, 0xd8]))
                } else {
                    Err(format!("no image for {entity_id}"))
                }
            })
        }
    }

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
                    automations: Some(automations::AutomationsCtx {
                        engine: rusthome_automation::Engine::new(
                            hub.clone(),
                            chrono_tz::Europe::Paris,
                        ),
                        blueprints_dir: None,
                        z2m_base: "zigbee2mqtt".into(),
                        ha: None,
                    }),
                    dashboards: Some(dashboards::DashboardsCtx {
                        store: Arc::new(rusthome_dashboard::Store::new()),
                        ha: None,
                        cameras: Some(Arc::new(FakeCameras)),
                    }),
                    system: serde_json::Value::Null,
                    display_dashboard: None,
                    zigbee: None,
                    frigate: None,
                    esphome: None,
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
        send_json(app, "POST", uri, body).await
    }

    async fn send_json(app: Router, method: &str, uri: &str, body: Value) -> (StatusCode, Value) {
        let req = Request::builder()
            .method(method)
            .uri(uri)
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
    async fn automations_yaml_round_trip_and_enable() {
        let (app, hub) = app(AuthMode::Dev);
        let yaml = "alias: Éteindre radiateur salon\ntrigger:\n  - platform: state\n    entity_id: light.salon\n    to: 'off'\naction:\n  - service: switch.turn_off\n    target:\n      entity_id: switch.radiateur\n";
        let (code, info) = post_json(
            app.clone(),
            "/api/v1/automations",
            serde_json::json!({ "yaml": yaml }),
        )
        .await;
        assert_eq!(code, StatusCode::OK, "{info}");
        assert_eq!(info["entity_id"], "automation.eteindre_radiateur_salon");
        assert_eq!(info["enabled"], false, "new automations start disabled");
        let id = info["id"].as_str().unwrap().to_owned();

        // Stored in the modern spelling.
        let (_, got) = get_json(app.clone(), &format!("/api/v1/automations/{id}")).await;
        let stored = got["yaml"].as_str().unwrap();
        assert!(
            stored.contains("triggers:") && stored.contains("action: switch.turn_off"),
            "{stored}"
        );

        let (code, enabled) = post_json(
            app.clone(),
            &format!("/api/v1/automations/{id}/enable"),
            serde_json::json!({}),
        )
        .await;
        assert_eq!(
            (code, enabled["enabled"].clone()),
            (StatusCode::OK, Value::Bool(true))
        );
        assert_eq!(
            hub.states
                .get(&EntityId::parse("automation.eteindre_radiateur_salon").unwrap())
                .unwrap()
                .state,
            "on"
        );
        assert_eq!(
            get_json(app, "/api/v1/automations")
                .await
                .1
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn invalid_automations_are_kept_but_not_enabled() {
        let (app, _) = app(AuthMode::Dev);
        let (code, info) = post_json(
            app.clone(),
            "/api/v1/automations",
            serde_json::json!({ "yaml": "id: x\nalias: Cassée\ntriggers: []\n" }),
        )
        .await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(info["valid"], false);
        assert_eq!(info["errors"][0]["path"], "triggers");
        assert_eq!(
            post_json(
                app.clone(),
                "/api/v1/automations/x/enable",
                serde_json::json!({})
            )
            .await
            .0,
            StatusCode::CONFLICT
        );
        assert_eq!(
            post_json(
                app.clone(),
                "/api/v1/automations",
                serde_json::json!({ "yaml": "a: [" })
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            post_json(app, "/api/v1/automations/import-ha", serde_json::json!({}))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn dashboards_accept_lovelace_yaml_and_flag_missing_entities() {
        let (app, _) = app(AuthMode::Dev);
        let yaml = "url_path: securite\ntitle: Sécurité\nviews:\n  - cards:\n      - type: custom:mushroom-entity-card\n        entity: light.salon\n      - type: tile\n        entity: light.disparue\n";
        let (code, created) = post_json(
            app.clone(),
            "/api/v1/dashboards",
            serde_json::json!({ "yaml": yaml }),
        )
        .await;
        assert_eq!(code, StatusCode::OK, "{created}");
        assert_eq!(created["summary"]["url_path"], "securite");
        let (_, got) = get_json(app.clone(), "/api/v1/dashboards/securite").await;
        assert_eq!(got["doc"]["views"][0]["cards"][0]["type"], "tile");
        assert_eq!(
            got["issues"][0]["message"],
            "entité absente : light.disparue"
        );
        assert_eq!(got["editable"], true);
        assert!(got["yaml"].as_str().unwrap().contains("light.salon"));

        let again = post_json(
            app.clone(),
            "/api/v1/dashboards",
            serde_json::json!({ "yaml": yaml }),
        )
        .await;
        assert_eq!(again.0, StatusCode::CONFLICT);
        let (code, put) = send_json(
            app.clone(),
            "PUT",
            "/api/v1/dashboards/securite",
            serde_json::json!({ "yaml": "url_path: ignorée\ntitle: Alarme\nviews: []\n" }),
        )
        .await;
        assert_eq!(code, StatusCode::OK, "{put}");
        assert_eq!(put["summary"]["title"], "Alarme");
        assert_eq!(
            put["summary"]["url_path"], "securite",
            "the path in the URL wins"
        );
        let (_, list) = get_json(app.clone(), "/api/v1/dashboards").await;
        assert_eq!(list.as_array().unwrap().len(), 1);
        let bad = post_json(
            app.clone(),
            "/api/v1/dashboards",
            serde_json::json!({ "yaml": "url_path: Pas Bon\nviews: []\n" }),
        )
        .await;
        assert_eq!(bad.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            send_json(
                app.clone(),
                "DELETE",
                "/api/v1/dashboards/securite",
                Value::Null
            )
            .await
            .0,
            StatusCode::OK
        );
        assert_eq!(
            get_json(app, "/api/v1/dashboards/securite").await.0,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn calendar_and_todo_check_domain_and_source() {
        let (app, _) = app(AuthMode::Dev);
        assert_eq!(
            get_json(app.clone(), "/api/v1/calendars/light.salon/events")
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
        // no HA configured in tests
        assert_eq!(
            get_json(
                app.clone(),
                "/api/v1/calendars/calendar.perso/events?days=3"
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            get_json(app, "/api/v1/todo/todo.courses/items").await.0,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn weather_and_energy_validate_before_asking_ha() {
        let (app, _) = app(AuthMode::Dev);
        let code = |u: &'static str| {
            let app = app.clone();
            async move { get_json(app, u).await.0 }
        };
        assert_eq!(
            code("/api/v1/weather/light.salon/forecast").await,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            code("/api/v1/weather/weather.x/forecast?type=weekly").await,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            code("/api/v1/energy/statistics?ids=a&start=hier&end=2026-10-17T00:00:00Z").await,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            code(
                "/api/v1/energy/statistics?ids=&start=2026-10-16T00:00:00Z&end=2026-10-17T00:00:00Z"
            )
            .await,
            StatusCode::BAD_REQUEST
        );
        // valid, but no HA configured in tests
        assert_eq!(code("/api/v1/energy/prefs").await, StatusCode::NOT_FOUND);
        assert_eq!(
            code("/api/v1/weather/weather.x/forecast").await,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn camera_snapshots_are_proxied() {
        let (app, _) = app(AuthMode::Dev);
        let res = app
            .clone()
            .oneshot(
                Request::get("/api/v1/cameras/camera.salon/snapshot")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()["content-type"], "image/jpeg");
        assert_eq!(
            get_json(app.clone(), "/api/v1/cameras/light.salon/snapshot")
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            get_json(app, "/api/v1/cameras/camera.garage/snapshot")
                .await
                .0,
            StatusCode::NOT_FOUND
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
