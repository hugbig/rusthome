//! Zigbee network management (M5): pairing, renaming (with entity id
//! propagation after a preview), OTA, removal, groups and bindings, through
//! the Zigbee2MQTT bridge API. Admins only. With MQTT read-only every request
//! is refused before anything is sent.

use axum::Json;
use axum::extract::{Path, Query, State as AxState};
use rusthome_model::EntityId;
use rusthome_model::refs::rewrite_entity_ids;
use rusthome_z2m::Z2m;
use rusthome_z2m::bridge::{BridgeError, plan_renames};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ApiError, ApiState, Role, User};

fn z2m<'a>(s: &'a ApiState, user: &User) -> Result<&'a Z2m, ApiError> {
    if user.role != Role::Admin {
        return Err(ApiError::Forbidden(
            "only admins manage the Zigbee network".into(),
        ));
    }
    s.zigbee
        .as_deref()
        .ok_or_else(|| ApiError::NotFound("Zigbee2MQTT is not configured".into()))
}

impl From<BridgeError> for ApiError {
    fn from(e: BridgeError) -> Self {
        match e {
            BridgeError::Unknown(_) => ApiError::BadRequest(e.to_string()),
            BridgeError::ReadOnly | BridgeError::Refused(_) => ApiError::Conflict(e.to_string()),
            BridgeError::Timeout | BridgeError::Mqtt(_) => ApiError::Internal(e.to_string()),
        }
    }
}

fn name_of(z: &Z2m, ieee: &str) -> Result<String, ApiError> {
    z.friendly_name(ieee)
        .ok_or_else(|| ApiError::NotFound(format!("unknown Zigbee device {ieee}")))
}

pub(crate) async fn overview(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(z2m(&s, &user)?.bridge_snapshot()))
}

#[derive(Deserialize)]
pub(crate) struct PermitJoin {
    /// Seconds (0 closes pairing; Zigbee2MQTT allows up to 254).
    seconds: u16,
    /// Pair through one router only (IEEE address), else the whole network.
    via: Option<String>,
}

pub(crate) async fn permit_join(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Json(b): Json<PermitJoin>,
) -> Result<Json<Value>, ApiError> {
    let z = z2m(&s, &user)?;
    let mut payload = json!({ "time": b.seconds.min(254) });
    if let Some(v) = b.via {
        payload["device"] = name_of(z, &v)?.into();
    }
    Ok(Json(z.bridge_request("permit_join", payload).await?))
}

/// Entity ids of a Zigbee device (`unique_id` = `<ieee>_<key>_zigbee2mqtt`).
fn device_entities(s: &ApiState, ieee: &str) -> Vec<String> {
    let prefix = format!("{ieee}_");
    s.hub
        .registries
        .entities()
        .into_iter()
        .filter(|e| {
            e.unique_id
                .as_deref()
                .is_some_and(|u| u.starts_with(&prefix))
        })
        .map(|e| e.entity_id.as_str().to_owned())
        .collect()
}

#[derive(Deserialize)]
pub(crate) struct RenameQuery {
    name: String,
}

/// What renaming would change: entity ids, automations and dashboards.
fn rename_plan(s: &ApiState, z: &Z2m, ieee: &str, new_name: &str) -> Result<Value, ApiError> {
    let old = name_of(z, ieee)?;
    let renames = plan_renames(&old, new_name, &device_entities(s, ieee), |id| {
        EntityId::parse(id).is_ok_and(|e| s.hub.registries.entity_by_entity_id(&e).is_some())
    });
    let mut automations = Vec::new();
    if let Some(a) = &s.automations {
        for info in a.engine.list() {
            if let Some(mut doc) = a.engine.document(&info.id)
                && rewrite_entity_ids(&mut doc, &renames) > 0
            {
                automations.push(json!({ "id": info.id, "alias": info.alias }));
            }
        }
    }
    let mut dashboards = Vec::new();
    if let Some(d) = &s.dashboards {
        for sum in d.store.list() {
            if let Some(mut doc) = d.store.get(&sum.url_path)
                && rewrite_entity_ids(&mut doc, &renames) > 0
            {
                dashboards.push(json!({ "url_path": sum.url_path, "title": sum.title }));
            }
        }
    }
    Ok(json!({
        "from": old,
        "to": new_name,
        "entities": renames.iter().map(|(f, t)| json!({ "from": f, "to": t })).collect::<Vec<_>>(),
        "automations": automations,
        "dashboards": dashboards,
    }))
}

pub(crate) async fn rename_preview(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(ieee): Path<String>,
    Query(q): Query<RenameQuery>,
) -> Result<Json<Value>, ApiError> {
    let z = z2m(&s, &user)?;
    Ok(Json(rename_plan(&s, z, &ieee, q.name.trim())?))
}

#[derive(Deserialize)]
pub(crate) struct Rename {
    name: String,
    /// Also rename the entity ids (and their references) as previewed.
    #[serde(default)]
    entities: bool,
}

pub(crate) async fn rename(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(ieee): Path<String>,
    Json(b): Json<Rename>,
) -> Result<Json<Value>, ApiError> {
    let z = z2m(&s, &user)?;
    let name = b.name.trim();
    if name.is_empty() || name.contains('/') || name.contains('#') || name.contains('+') {
        return Err(ApiError::BadRequest(
            "a Zigbee name cannot be empty nor contain / # +".into(),
        ));
    }
    let plan = rename_plan(&s, z, &ieee, name)?;
    // the device first: if Zigbee2MQTT refuses, nothing else changes
    z.bridge_request(
        "device/rename",
        json!({ "from": plan["from"], "to": name, "homeassistant_rename": false }),
    )
    .await?;
    let mut moved = 0;
    if b.entities {
        let renames: Vec<(String, String)> = plan["entities"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| Some((e["from"].as_str()?.to_owned(), e["to"].as_str()?.to_owned())))
            .collect();
        moved = apply_entity_renames(&s, &renames).await?;
    }
    Ok(Json(
        json!({ "renamed": name, "entities_renamed": moved, "plan": plan }),
    ))
}

/// Renames entity ids everywhere: registry, current state, history,
/// automations and dashboards. Returns the number of entities renamed.
pub(crate) async fn apply_entity_renames(
    s: &ApiState,
    renames: &[(String, String)],
) -> Result<usize, ApiError> {
    let mut done = 0;
    for (from, to) in renames {
        let (Ok(old), Ok(new)) = (EntityId::parse(from), EntityId::parse(to)) else {
            continue;
        };
        if let Some(mut entry) = s.hub.registries.entity_by_entity_id(&old) {
            entry.entity_id = new.clone();
            s.hub.registries.upsert_entity(entry);
        }
        if let Some(st) = s.hub.states.remove(&old) {
            s.hub.states.set(
                new.clone(),
                st.state.clone(),
                st.attributes.clone(),
                rusthome_model::Context::default(),
            );
        }
        if let Some(rec) = &s.recorder {
            rec.rename_history(&old, &new)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
        }
        done += 1;
    }
    if let Some(a) = &s.automations {
        for info in a.engine.list() {
            if let Some(mut doc) = a.engine.document(&info.id)
                && rewrite_entity_ids(&mut doc, renames) > 0
            {
                a.engine.upsert(
                    doc,
                    info.entity_id.clone(),
                    info.enabled,
                    info.issues.clone(),
                );
                crate::automations::persist(s, &info.id).await?;
            }
        }
    }
    if let Some(d) = &s.dashboards {
        for sum in d.store.list() {
            if let Some(mut doc) = d.store.get(&sum.url_path)
                && rewrite_entity_ids(&mut doc, renames) > 0
            {
                crate::dashboards::persist(s, &sum.url_path, &doc).await?;
                d.store.put(doc);
            }
        }
    }
    Ok(done)
}

#[derive(Deserialize)]
pub(crate) struct Ota {
    /// `check` or `update`.
    action: String,
}

pub(crate) async fn ota(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(ieee): Path<String>,
    Json(b): Json<Ota>,
) -> Result<Json<Value>, ApiError> {
    let z = z2m(&s, &user)?;
    let kind = match b.action.as_str() {
        "check" => "device/ota_update/check",
        "update" => "device/ota_update/update",
        other => return Err(ApiError::BadRequest(format!("unknown OTA action {other}"))),
    };
    let id = name_of(z, &ieee)?;
    Ok(Json(z.bridge_request(kind, json!({ "id": id })).await?))
}

#[derive(Deserialize)]
pub(crate) struct Remove {
    #[serde(default)]
    force: bool,
}

pub(crate) async fn remove(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(ieee): Path<String>,
    Json(b): Json<Remove>,
) -> Result<Json<Value>, ApiError> {
    let z = z2m(&s, &user)?;
    let id = name_of(z, &ieee)?;
    Ok(Json(
        z.bridge_request("device/remove", json!({ "id": id, "force": b.force }))
            .await?,
    ))
}

#[derive(Deserialize)]
pub(crate) struct Bind {
    /// Source device (IEEE address).
    from: String,
    /// Target: device IEEE address, group name, or `default_bind_group`.
    to: String,
    #[serde(default)]
    clusters: Vec<String>,
    #[serde(default)]
    unbind: bool,
}

pub(crate) async fn bind(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Json(b): Json<Bind>,
) -> Result<Json<Value>, ApiError> {
    let z = z2m(&s, &user)?;
    let from = name_of(z, &b.from)?;
    let to = z.friendly_name(&b.to).unwrap_or(b.to.clone());
    let mut payload = json!({ "from": from, "to": to });
    if !b.clusters.is_empty() {
        payload["clusters"] = json!(b.clusters);
    }
    let kind = if b.unbind {
        "device/unbind"
    } else {
        "device/bind"
    };
    Ok(Json(z.bridge_request(kind, payload).await?))
}

#[derive(Deserialize)]
pub(crate) struct GroupBody {
    name: String,
}

pub(crate) async fn group_add(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Json(b): Json<GroupBody>,
) -> Result<Json<Value>, ApiError> {
    let z = z2m(&s, &user)?;
    Ok(Json(
        z.bridge_request("group/add", json!({ "friendly_name": b.name.trim() }))
            .await?,
    ))
}

pub(crate) async fn group_remove(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(group): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let z = z2m(&s, &user)?;
    Ok(Json(
        z.bridge_request("group/remove", json!({ "id": group }))
            .await?,
    ))
}

#[derive(Deserialize)]
pub(crate) struct Member {
    /// Device IEEE address.
    device: String,
    endpoint: Option<u8>,
    #[serde(default)]
    remove: bool,
}

pub(crate) async fn group_member(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(group): Path<String>,
    Json(b): Json<Member>,
) -> Result<Json<Value>, ApiError> {
    let z = z2m(&s, &user)?;
    let mut device = name_of(z, &b.device)?;
    if let Some(ep) = b.endpoint {
        device = format!("{device}/{ep}");
    }
    let kind = if b.remove {
        "group/members/remove"
    } else {
        "group/members/add"
    };
    Ok(Json(
        z.bridge_request(kind, json!({ "group": group, "device": device }))
            .await?,
    ))
}
/// Routes of Zigbee network management.
pub(crate) fn routes() -> axum::Router<ApiState> {
    axum::Router::new()
        .route("/zigbee", axum::routing::get(overview))
        .route("/zigbee/permit_join", axum::routing::post(permit_join))
        .route("/zigbee/bind", axum::routing::post(bind))
        .route(
            "/zigbee/devices/{ieee}/rename-preview",
            axum::routing::get(rename_preview),
        )
        .route("/zigbee/devices/{ieee}/rename", axum::routing::post(rename))
        .route("/zigbee/devices/{ieee}/ota", axum::routing::post(ota))
        .route("/zigbee/devices/{ieee}/remove", axum::routing::post(remove))
        .route("/zigbee/groups", axum::routing::post(group_add))
        .route(
            "/zigbee/groups/{group}",
            axum::routing::delete(group_remove),
        )
        .route(
            "/zigbee/groups/{group}/members",
            axum::routing::post(group_member),
        )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rusthome_core::{Hub, SystemClock};
    use rusthome_model::{Context, EntityEntry, EntityStatus};
    use serde_json::{Map, json};

    use super::*;
    use crate::{AuthMode, automations, dashboards};

    fn state() -> ApiState {
        let hub = Hub::new(Arc::new(SystemClock));
        ApiState {
            hub: hub.clone(),
            recorder: None,
            auth: AuthMode::Dev,
            automations: Some(automations::AutomationsCtx {
                engine: rusthome_automation::Engine::new(hub.clone(), chrono_tz::Europe::Paris),
                blueprints_dir: None,
                z2m_base: "zigbee2mqtt".into(),
                ha: None,
            }),
            dashboards: Some(dashboards::DashboardsCtx {
                store: Arc::new(rusthome_dashboard::Store::new()),
                ha: None,
                cameras: None,
            }),
            system: Value::Null,
            display_dashboard: None,
            zigbee: None,
            frigate: None,
            esphome: None,
        }
    }

    #[tokio::test]
    async fn renaming_entities_updates_every_reference() {
        let s = state();
        let old = EntityId::parse("sensor.salon_temperature").unwrap();
        s.hub.registries.upsert_entity(EntityEntry {
            uid: "z2m-1".into(),
            entity_id: old.clone(),
            integration: "z2m".into(),
            unique_id: Some("0x01_temperature_zigbee2mqtt".into()),
            device_id: None,
            name: None,
            area_id: None,
            labels: vec![],
            icon: None,
            hidden: false,
            disabled: false,
            status: EntityStatus::Active,
        });
        s.hub
            .states
            .set(old.clone(), "21.4", Map::new(), Context::default());
        let engine = &s.automations.as_ref().unwrap().engine;
        engine.upsert(
            json!({ "id": "a1", "alias": "Chauffage",
                    "triggers": [{ "trigger": "numeric_state", "entity_id": "sensor.salon_temperature", "below": 18 }],
                    "actions": [{ "action": "climate.turn_on", "target": { "entity_id": "climate.salon" } }] }),
            EntityId::parse("automation.chauffage").unwrap(),
            false,
            vec![],
        );
        let store = &s.dashboards.as_ref().unwrap().store;
        store.put(json!({ "url_path": "maison", "views": [{ "cards": [
            { "type": "tile", "entity": "sensor.salon_temperature" }
        ] }] }));

        let moved = apply_entity_renames(
            &s,
            &[(
                "sensor.salon_temperature".into(),
                "sensor.sejour_temperature".into(),
            )],
        )
        .await;
        assert_eq!(moved.ok(), Some(1));
        let new = EntityId::parse("sensor.sejour_temperature").unwrap();
        let entry = s.hub.registries.entity_by_entity_id(&new).unwrap();
        assert_eq!(entry.uid, "z2m-1", "same registry entry");
        assert!(s.hub.registries.entity_by_entity_id(&old).is_none());
        assert_eq!(s.hub.states.get(&new).unwrap().state, "21.4");
        assert!(s.hub.states.get(&old).is_none());
        assert_eq!(
            engine.document("a1").unwrap()["triggers"][0]["entity_id"],
            "sensor.sejour_temperature"
        );
        assert_eq!(
            store.get("maison").unwrap()["views"][0]["cards"][0]["entity"],
            "sensor.sejour_temperature"
        );
    }
}
