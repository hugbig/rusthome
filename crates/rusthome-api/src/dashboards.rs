//! Dashboards API (M4): list, document + YAML, edit, HA import, camera
//! snapshots.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State as AxState};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use rusthome_dashboard::{Issue, Store, Summary, lovelace, refs, valid_url_path};
use rusthome_ha_bridge::BridgeConfig;
use rusthome_model::EntityId;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ApiError, ApiState, Role, User};

/// A JPEG/PNG image and its content type.
pub type Image = (String, Vec<u8>);
pub type ImageFuture<'a> = Pin<Box<dyn Future<Output = Result<Image, String>> + Send + 'a>>;

/// Where camera snapshots come from (Frigate, HA in read-only…). Wired by
/// the binary, which knows the integrations.
pub trait CameraSource: Send + Sync {
    fn snapshot<'a>(&'a self, entity_id: &'a EntityId) -> ImageFuture<'a>;
}

#[derive(Clone)]
pub struct DashboardsCtx {
    pub store: Arc<Store>,
    /// HA to import from (read-only WebSocket).
    pub ha: Option<BridgeConfig>,
    pub cameras: Option<Arc<dyn CameraSource>>,
}

fn ctx(s: &ApiState) -> Result<&DashboardsCtx, ApiError> {
    s.dashboards
        .as_ref()
        .ok_or_else(|| ApiError::NotFound("dashboards are disabled".into()))
}

fn visible(s: &ApiState, user: &User, sum: &Summary) -> bool {
    visible_for(user.role, s.display_dashboard.as_deref(), sum)
}

/// Admins see everything, residents what is not reserved to admins, the
/// wall tablet (`display`) its own dashboard only.
fn visible_for(role: Role, display_dashboard: Option<&str>, sum: &Summary) -> bool {
    match role {
        Role::Admin => true,
        Role::Resident => !sum.require_admin,
        Role::Display => display_dashboard == Some(sum.url_path.as_str()),
    }
}

/// Admins edit everything, residents the dashboards not reserved to admins,
/// display accounts nothing.
fn can_edit(user: &User, doc: Option<&Value>) -> Result<(), ApiError> {
    let locked = doc.is_some_and(|d| d["require_admin"] == true);
    match user.role {
        Role::Admin => Ok(()),
        Role::Resident if !locked => Ok(()),
        _ => Err(ApiError::Forbidden(
            "this account cannot edit this dashboard".into(),
        )),
    }
}

pub(crate) async fn persist(s: &ApiState, url_path: &str, doc: &Value) -> Result<(), ApiError> {
    if let Some(rec) = &s.recorder {
        rec.save_dashboard(url_path, doc, s.hub.clock.now())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    }
    Ok(())
}

/// Unsupported cards and references to unknown entities.
fn issues(s: &ApiState, doc: &Value) -> Vec<Issue> {
    let mut out = refs::unsupported_cards(doc);
    out.extend(refs::missing_entities(doc, |e| {
        EntityId::parse(e).is_ok_and(|id| {
            s.hub.states.get(&id).is_some() || s.hub.registries.entity_by_entity_id(&id).is_some()
        })
    }));
    out
}

pub(crate) async fn list(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
) -> Result<Json<Vec<Summary>>, ApiError> {
    let all = ctx(&s)?.store.list();
    Ok(Json(
        all.into_iter().filter(|d| visible(&s, &user, d)).collect(),
    ))
}

pub(crate) async fn get_one(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(url_path): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let doc = ctx(&s)?
        .store
        .get(&url_path)
        .ok_or_else(|| ApiError::NotFound(format!("unknown dashboard {url_path}")))?;
    let summary = Summary::of(&doc);
    if !visible(&s, &user, &summary) {
        return Err(ApiError::Forbidden("not visible to this account".into()));
    }
    let yaml = serde_yaml::to_string(&doc).map_err(|e| ApiError::Internal(e.to_string()))?;
    let issues = issues(&s, &doc);
    let editable = can_edit(&user, Some(&doc)).is_ok();
    Ok(Json(json!({
        "summary": summary, "doc": doc, "yaml": yaml, "issues": issues, "editable": editable
    })))
}

#[derive(Deserialize)]
pub(crate) struct YamlBody {
    yaml: String,
}

/// Parses user YAML (RustHome or Lovelace format) into a document.
fn parse(yaml: &str, url_path: Option<&str>) -> Result<Value, ApiError> {
    let raw: Value =
        serde_yaml::from_str(yaml).map_err(|e| ApiError::BadRequest(format!("YAML: {e}")))?;
    if !raw["views"].is_array() {
        return Err(ApiError::BadRequest(
            "a dashboard needs a `views` list".into(),
        ));
    }
    let mut meta = json!({});
    if let Some(p) = url_path {
        meta["url_path"] = p.into();
    }
    let (mut doc, _) = lovelace::dashboard(&meta, &raw);
    if let Some(p) = url_path {
        doc["url_path"] = p.into();
    }
    match doc["url_path"].as_str() {
        Some(p) if valid_url_path(p) => Ok(doc),
        Some(p) => Err(ApiError::BadRequest(format!(
            "invalid url_path {p:?}: lowercase letters, digits and dashes"
        ))),
        None => Err(ApiError::BadRequest("missing url_path".into())),
    }
}

pub(crate) async fn put_one(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(url_path): Path<String>,
    Json(body): Json<YamlBody>,
) -> Result<Json<Value>, ApiError> {
    let c = ctx(&s)?;
    let current = c
        .store
        .get(&url_path)
        .ok_or_else(|| ApiError::NotFound(format!("unknown dashboard {url_path}")))?;
    can_edit(&user, Some(&current))?;
    let doc = parse(&body.yaml, Some(&url_path))?;
    // only admins may lock or unlock a dashboard
    if user.role != Role::Admin && doc["require_admin"] == true {
        return Err(ApiError::Forbidden(
            "only admins can lock a dashboard".into(),
        ));
    }
    persist(&s, &url_path, &doc).await?;
    c.store.put(doc.clone());
    Ok(Json(
        json!({ "summary": Summary::of(&doc), "issues": issues(&s, &doc) }),
    ))
}

pub(crate) async fn create(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Json(body): Json<YamlBody>,
) -> Result<Json<Value>, ApiError> {
    let c = ctx(&s)?;
    can_edit(&user, None)?;
    let doc = parse(&body.yaml, None)?;
    let url_path = doc["url_path"].as_str().unwrap_or_default().to_owned();
    if c.store.contains(&url_path) {
        return Err(ApiError::Conflict(format!(
            "dashboard {url_path} already exists"
        )));
    }
    persist(&s, &url_path, &doc).await?;
    c.store.put(doc.clone());
    Ok(Json(
        json!({ "summary": Summary::of(&doc), "issues": issues(&s, &doc) }),
    ))
}

pub(crate) async fn delete_one(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(url_path): Path<String>,
) -> Result<Json<Value>, ApiError> {
    if user.role != Role::Admin {
        return Err(ApiError::Forbidden(
            "only admins can delete dashboards".into(),
        ));
    }
    let c = ctx(&s)?;
    if !c.store.remove(&url_path) {
        return Err(ApiError::NotFound(format!("unknown dashboard {url_path}")));
    }
    if let Some(rec) = &s.recorder {
        rec.delete_dashboard(&url_path)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    }
    Ok(Json(json!({ "deleted": url_path })))
}

#[derive(Deserialize)]
pub(crate) struct ImportQuery {
    #[serde(default)]
    overwrite: bool,
}

/// Imports every Lovelace dashboard of HA (read-only WebSocket).
pub(crate) async fn import_ha(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Query(q): Query<ImportQuery>,
) -> Result<Json<Value>, ApiError> {
    if user.role != Role::Admin {
        return Err(ApiError::Forbidden(
            "only admins can import dashboards".into(),
        ));
    }
    let c = ctx(&s)?;
    let ha = c.ha.as_ref().ok_or_else(|| {
        ApiError::NotFound("Home Assistant is not configured (HA_URL/HA_TOKEN)".into())
    })?;
    let found = rusthome_ha_bridge::fetch_dashboards(ha)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let (mut imported, mut existing, mut invalid, mut issues) = (vec![], vec![], vec![], 0);
    for d in found {
        let (doc, found_issues) = lovelace::dashboard(&d.meta, &d.config);
        let url_path = doc["url_path"].as_str().unwrap_or_default().to_owned();
        if !valid_url_path(&url_path) {
            invalid.push(url_path);
            continue;
        }
        if !q.overwrite && c.store.contains(&url_path) {
            existing.push(url_path);
            continue;
        }
        issues += found_issues.len();
        persist(&s, &url_path, &doc).await?;
        c.store.put(doc);
        imported.push(url_path);
    }
    Ok(Json(json!({
        "imported": imported, "skipped_existing": existing, "invalid": invalid, "issues": issues
    })))
}

/// Latest image of a camera entity.
pub(crate) async fn camera_snapshot(
    AxState(s): AxState<ApiState>,
    Path(entity_id): Path<String>,
) -> Result<Response, ApiError> {
    let id = EntityId::parse(&entity_id)
        .ok()
        .filter(|e| e.domain() == "camera")
        .ok_or_else(|| ApiError::BadRequest(format!("{entity_id} is not a camera")))?;
    let source = ctx(&s)?
        .cameras
        .as_ref()
        .ok_or_else(|| ApiError::NotFound("no camera source configured".into()))?;
    let (content_type, bytes) = source.snapshot(&id).await.map_err(ApiError::NotFound)?;
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        bytes,
    )
        .into_response())
}

pub(crate) fn ha_of(s: &ApiState) -> Result<&BridgeConfig, ApiError> {
    ctx(s)?.ha.as_ref().ok_or_else(|| {
        ApiError::NotFound("Home Assistant is not configured (HA_URL/HA_TOKEN)".into())
    })
}

pub(crate) fn entity_of_domain(raw: &str, domain: &str) -> Result<EntityId, ApiError> {
    EntityId::parse(raw)
        .ok()
        .filter(|e| e.domain() == domain)
        .ok_or_else(|| ApiError::BadRequest(format!("{raw} is not a {domain} entity")))
}

#[derive(Deserialize)]
pub(crate) struct DaysQuery {
    days: Option<u32>,
}

/// Upcoming events of a calendar (still read from HA).
pub(crate) async fn calendar_events(
    AxState(s): AxState<ApiState>,
    Path(entity_id): Path<String>,
    Query(q): Query<DaysQuery>,
) -> Result<Json<Value>, ApiError> {
    let id = entity_of_domain(&entity_id, "calendar")?;
    let now = s.hub.clock.now();
    let start = now - chrono::Duration::hours(1);
    let end = now + chrono::Duration::days(i64::from(q.days.unwrap_or(7).clamp(1, 60)));
    let events = rusthome_ha_bridge::rest::calendar_events(
        ha_of(&s)?,
        id.as_str(),
        &start.to_rfc3339(),
        &end.to_rfc3339(),
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(json!(events)))
}

/// Items of a to-do list (still read from HA).
pub(crate) async fn todo_items(
    AxState(s): AxState<ApiState>,
    Path(entity_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let id = entity_of_domain(&entity_id, "todo")?;
    let items = rusthome_ha_bridge::fetch_todo_items(ha_of(&s)?, id.as_str())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(json!(items)))
}

#[derive(Deserialize)]
pub(crate) struct ImageQuery {
    i: usize,
}

/// Poster of an "upcoming media" item (Emby…), fetched by the server so the
/// page stays HTTPS. Only URLs listed by the sensor itself are served.
pub(crate) async fn media_image(
    AxState(s): AxState<ApiState>,
    Path(entity_id): Path<String>,
    Query(q): Query<ImageQuery>,
) -> Result<Response, ApiError> {
    let id = entity_of_domain(&entity_id, "sensor")?;
    let state = s
        .hub
        .states
        .get(&id)
        .ok_or_else(|| ApiError::NotFound(format!("unknown entity {entity_id}")))?;
    let url = rusthome_dashboard::media_image_url(
        state.attributes.get("data").unwrap_or(&Value::Null),
        q.i,
    )
    .ok_or_else(|| ApiError::NotFound("no image for this item".into()))?;
    let res = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .get(&url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| ApiError::NotFound(format!("image unavailable: {e}")))?;
    let content_type = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .filter(|t| t.starts_with("image/"))
        .unwrap_or("image/jpeg")
        .to_owned();
    let bytes = res
        .bytes()
        .await
        .map_err(|e| ApiError::NotFound(format!("image unavailable: {e}")))?;
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "max-age=3600".to_owned()),
        ],
        bytes.to_vec(),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn each_role_sees_its_dashboards() {
        let panel = Summary::of(&json!({ "url_path": "dashboard-nspanel", "views": [] }));
        let locked =
            Summary::of(&json!({ "url_path": "admin", "require_admin": true, "views": [] }));
        assert!(visible_for(Role::Admin, None, &locked));
        assert!(!visible_for(Role::Resident, None, &locked));
        assert!(visible_for(Role::Resident, None, &panel));
        assert!(visible_for(
            Role::Display,
            Some("dashboard-nspanel"),
            &panel
        ));
        assert!(!visible_for(
            Role::Display,
            Some("dashboard-nspanel"),
            &locked
        ));
        assert!(
            !visible_for(Role::Display, None, &panel),
            "no dashboard configured: nothing"
        );
    }
}
