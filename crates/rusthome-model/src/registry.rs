use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::EntityId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Floor {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub level: Option<i32>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Area {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub floor_id: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Label {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Device {
    pub id: String,
    /// Integration that owns the device (`mqtt`, `esphome`, `ha_bridge`…).
    pub integration: String,
    pub name: String,
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
    pub disabled: bool,
}

/// Lifecycle of a registry entry (cf. inventaire/05: ~300 orphans in HA).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum EntityStatus {
    #[default]
    Active,
    /// The integration no longer provides this entity: candidate for purge.
    Orphan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EntityEntry {
    /// Stable internal id, survives renames of `entity_id`.
    pub uid: String,
    pub entity_id: EntityId,
    pub integration: String,
    /// Identifier of the entity inside its integration.
    #[serde(default)]
    pub unique_id: Option<String>,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    /// Own area; when `None` the device area applies.
    #[serde(default)]
    pub area_id: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub status: EntityStatus,
}
