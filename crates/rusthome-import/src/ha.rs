//! Lenient mirrors of Home Assistant registry payloads. Unknown fields are
//! ignored and most fields are optional: HA adds fields between releases.

use rusthome_model::State;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HaFloor {
    pub floor_id: String,
    pub name: String,
    #[serde(default)]
    pub level: Option<i32>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HaArea {
    pub area_id: String,
    pub name: String,
    #[serde(default)]
    pub floor_id: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HaLabel {
    pub label_id: String,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HaDevice {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub name_by_user: Option<String>,
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub sw_version: Option<String>,
    #[serde(default)]
    pub area_id: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub disabled_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HaEntity {
    /// Registry id (UUID-like): referenced by device-based automations.
    pub id: String,
    pub entity_id: String,
    pub platform: String,
    #[serde(default)]
    pub unique_id: Option<String>,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub area_id: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub original_name: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub hidden_by: Option<String>,
    #[serde(default)]
    pub disabled_by: Option<String>,
}

/// Everything read from HA in one pass (also the format of dev fixtures).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HaSnapshot {
    pub floors: Vec<HaFloor>,
    pub areas: Vec<HaArea>,
    pub labels: Vec<HaLabel>,
    pub devices: Vec<HaDevice>,
    pub entities: Vec<HaEntity>,
    pub states: Vec<State>,
}
