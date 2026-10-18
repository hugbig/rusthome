//! Automations API (M3): list, YAML read/write, enable, run, traces, HA import.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State as AxState};
use rusthome_automation::import::{HubResolver, Source, entity_id_for_alias, import, record};
use rusthome_automation::normalize::Normalizer;
use rusthome_automation::{Engine, Info};
use rusthome_ha_bridge::BridgeConfig;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ApiError, ApiState, Role, User};

/// Everything the automation endpoints need.
#[derive(Clone)]
pub struct AutomationsCtx {
    pub engine: Arc<Engine>,
    pub blueprints_dir: Option<PathBuf>,
    pub z2m_base: String,
    /// HA to import from (read-only REST).
    pub ha: Option<BridgeConfig>,
}

fn ctx(s: &ApiState) -> Result<&AutomationsCtx, ApiError> {
    s.automations
        .as_ref()
        .ok_or_else(|| ApiError::NotFound("automations are disabled".into()))
}

fn admin(user: &User) -> Result<(), ApiError> {
    if user.role == Role::Admin {
        Ok(())
    } else {
        Err(ApiError::Forbidden(
            "only admins can change automations".into(),
        ))
    }
}

pub(crate) async fn persist(s: &ApiState, id: &str) -> Result<(), ApiError> {
    let (Some(rec), Ok(c)) = (s.recorder.as_ref(), ctx(s)) else {
        return Ok(());
    };
    let (Some(doc), Some(info)) = (c.engine.document(id), c.engine.info(id)) else {
        return Ok(());
    };
    rec.save_automation(
        id,
        &record(&doc, &info.entity_id, info.enabled, &info.issues),
        s.hub.clock.now(),
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub(crate) async fn list(AxState(s): AxState<ApiState>) -> Result<Json<Vec<Info>>, ApiError> {
    Ok(Json(ctx(&s)?.engine.list()))
}

pub(crate) async fn get_one(
    AxState(s): AxState<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let c = ctx(&s)?;
    let (Some(info), Some(doc)) = (c.engine.info(&id), c.engine.document(&id)) else {
        return Err(ApiError::NotFound(format!("unknown automation {id}")));
    };
    let ordered = rusthome_automation::normalize::canonical_order(&doc);
    let yaml = serde_yaml::to_string(&ordered).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(json!({ "info": info, "yaml": yaml })))
}

#[derive(Deserialize)]
pub(crate) struct YamlBody {
    yaml: String,
}

/// Parses user YAML, normalizes it (legacy keys, device ids) and upserts.
/// Invalid documents are saved too (nothing is lost) but get disabled.
fn apply_yaml(s: &ApiState, yaml: &str, forced_id: Option<&str>) -> Result<Info, ApiError> {
    let c = ctx(s)?;
    let mut value: Value =
        serde_yaml::from_str(yaml).map_err(|e| ApiError::BadRequest(format!("YAML: {e}")))?;
    if !value.is_object() {
        return Err(ApiError::BadRequest(
            "an automation is a YAML mapping".into(),
        ));
    }
    let id = match forced_id {
        Some(id) => id.to_owned(),
        None => value["id"].as_str().map_or_else(
            || format!("{}", s.hub.clock.now().timestamp_millis()),
            str::to_owned,
        ),
    };
    value["id"] = id.clone().into();
    let resolver = HubResolver {
        hub: s.hub.clone(),
        blueprints_dir: c.blueprints_dir.clone(),
    };
    let mut n = Normalizer::new(&resolver, &c.z2m_base);
    let doc = n.automation(&value);
    let previous = c.engine.info(&id);
    let entity_id = if let Some(p) = &previous {
        p.entity_id.clone()
    } else {
        let alias = doc["alias"].as_str().unwrap_or("Sans nom").to_owned();
        entity_id_for_alias(&alias, |e| {
            rusthome_model::EntityId::parse(e)
                .is_ok_and(|e| s.hub.registries.entity_by_entity_id(&e).is_some())
        })
    };
    let keep_enabled = previous.is_some_and(|p| p.enabled);
    Ok(c.engine.upsert(doc, entity_id, keep_enabled, n.issues))
}

pub(crate) async fn put_one(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(id): Path<String>,
    Json(body): Json<YamlBody>,
) -> Result<Json<Info>, ApiError> {
    admin(&user)?;
    let info = apply_yaml(&s, &body.yaml, Some(&id))?;
    persist(&s, &info.id).await?;
    Ok(Json(info))
}

pub(crate) async fn create(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Json(body): Json<YamlBody>,
) -> Result<Json<Info>, ApiError> {
    admin(&user)?;
    let info = apply_yaml(&s, &body.yaml, None)?;
    persist(&s, &info.id).await?;
    Ok(Json(info))
}

pub(crate) async fn delete_one(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    admin(&user)?;
    let c = ctx(&s)?;
    if !c.engine.remove(&id) {
        return Err(ApiError::NotFound(format!("unknown automation {id}")));
    }
    if let Some(rec) = &s.recorder {
        rec.delete_automation(&id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    }
    Ok(Json(json!({ "deleted": id })))
}

async fn set_enabled(
    s: &ApiState,
    user: &User,
    id: &str,
    on: bool,
) -> Result<Json<Info>, ApiError> {
    admin(user)?;
    let info = ctx(s)?
        .engine
        .set_enabled(id, on)
        .map_err(ApiError::Conflict)?;
    persist(s, id).await?;
    Ok(Json(info))
}

pub(crate) async fn enable(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<Info>, ApiError> {
    set_enabled(&s, &user, &id, true).await
}

pub(crate) async fn disable(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<Info>, ApiError> {
    set_enabled(&s, &user, &id, false).await
}

/// « Exécuter » : runs the actions now (no triggers, no conditions).
pub(crate) async fn run_now(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    if user.role == Role::Display {
        return Err(ApiError::Forbidden(
            "display accounts cannot run automations".into(),
        ));
    }
    ctx(&s)?
        .engine
        .trigger_now(&id)
        .map_err(ApiError::Conflict)?;
    Ok(Json(json!({ "started": id })))
}

pub(crate) async fn traces(
    AxState(s): AxState<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!(ctx(&s)?.engine.traces(&id))))
}

#[derive(Deserialize)]
pub(crate) struct ImportQuery {
    #[serde(default)]
    overwrite: bool,
}

/// Imports every automation of HA (read-only REST), all **disabled**.
pub(crate) async fn import_ha(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Query(q): Query<ImportQuery>,
) -> Result<Json<Value>, ApiError> {
    admin(&user)?;
    let c = ctx(&s)?;
    let ha = c.ha.as_ref().ok_or_else(|| {
        ApiError::NotFound("Home Assistant is not configured (HA_URL/HA_TOKEN)".into())
    })?;
    let found = rusthome_ha_bridge::rest::fetch_automations(ha)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let sources: Vec<Source> = found
        .into_iter()
        .map(|a| Source {
            entity_id: a.entity_id,
            config: a.config,
        })
        .collect();
    let resolver = HubResolver {
        hub: s.hub.clone(),
        blueprints_dir: c.blueprints_dir.clone(),
    };
    let (report, records) = import(&c.engine, &resolver, &c.z2m_base, &sources, q.overwrite);
    if let Some(rec) = &s.recorder {
        for (id, r) in &records {
            rec.save_automation(id, r, s.hub.clock.now())
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
        }
    }
    Ok(Json(json!(report)))
}
