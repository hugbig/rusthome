//! Data of the Énergie panel and of weather forecasts. Both still come from
//! HA (read-only WebSocket) until RustHome keeps long-term statistics itself.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Path, Query, State as AxState};
use serde::Deserialize;
use serde_json::Value;

use crate::dashboards::{entity_of_domain, ha_of};
use crate::{ApiError, ApiState};

/// Forecasts change slowly: one HA request per entity and kind every 10 min.
const FORECAST_TTL: Duration = Duration::from_secs(600);
type Cache = Mutex<HashMap<String, (Instant, Value)>>;
static FORECASTS: LazyLock<Cache> = LazyLock::new(Cache::default);

#[derive(Deserialize)]
pub(crate) struct ForecastQuery {
    #[serde(rename = "type")]
    kind: Option<String>,
}

pub(crate) async fn forecast(
    AxState(s): AxState<ApiState>,
    Path(entity_id): Path<String>,
    Query(q): Query<ForecastQuery>,
) -> Result<Json<Value>, ApiError> {
    let id = entity_of_domain(&entity_id, "weather")?;
    let kind = q.kind.unwrap_or_else(|| "daily".into());
    if !["daily", "hourly", "twice_daily"].contains(&kind.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "unknown forecast type {kind}"
        )));
    }
    let key = format!("{}|{kind}", id.as_str());
    let cached = FORECASTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        .filter(|(at, _)| at.elapsed() < FORECAST_TTL)
        .map(|(_, v)| v.clone());
    if let Some(v) = cached {
        return Ok(Json(v));
    }
    let v = rusthome_ha_bridge::fetch_forecast(ha_of(&s)?, id.as_str(), &kind)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    FORECASTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, (Instant::now(), v.clone()));
    Ok(Json(v))
}

pub(crate) async fn energy_prefs(AxState(s): AxState<ApiState>) -> Result<Json<Value>, ApiError> {
    rusthome_ha_bridge::fetch_energy_prefs(ha_of(&s)?)
        .await
        .map(Json)
        .map_err(|e| ApiError::Internal(e.to_string()))
}

#[derive(Deserialize)]
pub(crate) struct StatsQuery {
    /// Comma-separated statistic ids (`sensor.linky_base`).
    ids: String,
    start: String,
    end: String,
    period: Option<String>,
}

pub(crate) async fn energy_statistics(
    AxState(s): AxState<ApiState>,
    Query(q): Query<StatsQuery>,
) -> Result<Json<Value>, ApiError> {
    let ids: Vec<String> = q
        .ids
        .split(',')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(str::to_owned)
        .collect();
    if ids.is_empty() || ids.len() > 50 {
        return Err(ApiError::BadRequest("1 to 50 statistic ids".into()));
    }
    let period = q.period.unwrap_or_else(|| "hour".into());
    if !["5minute", "hour", "day", "week", "month"].contains(&period.as_str()) {
        return Err(ApiError::BadRequest(format!("unknown period {period}")));
    }
    for t in [&q.start, &q.end] {
        chrono::DateTime::parse_from_rfc3339(t)
            .map_err(|_| ApiError::BadRequest(format!("not an RFC 3339 date: {t}")))?;
    }
    rusthome_ha_bridge::fetch_statistics(ha_of(&s)?, &ids, &q.start, &q.end, &period)
        .await
        .map(Json)
        .map_err(|e| ApiError::Internal(e.to_string()))
}
