use std::collections::{HashMap, HashSet};

use rusthome_model::{Area, Device, EntityEntry, EntityId, EntityStatus, Floor, Label, State};
use serde::Serialize;

use crate::HaSnapshot;

/// Why an HA entity is not imported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// HA keeps it only as a `restored` ghost: the integration no longer provides it.
    Ghost,
    /// Portainer entity of a recreated container (hash-prefixed id).
    PortainerOrphan,
    /// `entity_id` does not respect the `domain.object_id` rules.
    InvalidEntityId,
}

#[derive(Debug, Default)]
pub struct ImportResult {
    pub floors: Vec<Floor>,
    pub areas: Vec<Area>,
    pub labels: Vec<Label>,
    pub devices: Vec<Device>,
    pub entities: Vec<EntityEntry>,
    pub skipped: Vec<(String, SkipReason)>,
}

/// Integration name given to everything mirrored from HA, e.g. `ha:mqtt`.
fn ha_integration(platform: &str) -> String {
    format!("ha:{platform}")
}

/// Pure conversion: no I/O, deterministic, fully unit-tested.
#[must_use]
pub fn import_registries(snap: &HaSnapshot) -> ImportResult {
    let ghosts: HashSet<&str> = snap
        .states
        .iter()
        .filter(|s| is_ghost(s))
        .map(|s| s.entity_id.as_str())
        .collect();

    let mut out = ImportResult {
        floors: snap
            .floors
            .iter()
            .map(|f| Floor {
                id: f.floor_id.clone(),
                name: f.name.clone(),
                level: f.level,
                icon: f.icon.clone(),
            })
            .collect(),
        areas: snap
            .areas
            .iter()
            .map(|a| Area {
                id: a.area_id.clone(),
                name: a.name.clone(),
                floor_id: a.floor_id.clone(),
                icon: a.icon.clone(),
                aliases: a.aliases.clone(),
            })
            .collect(),
        labels: snap
            .labels
            .iter()
            .map(|l| Label {
                id: l.label_id.clone(),
                name: l.name.clone(),
                color: l.color.clone(),
                icon: l.icon.clone(),
            })
            .collect(),
        ..ImportResult::default()
    };

    // A device belongs to the integration of its first kept entity.
    let mut device_platform: HashMap<&str, &str> = HashMap::new();

    for e in &snap.entities {
        let Ok(entity_id) = EntityId::parse(&e.entity_id) else {
            out.skipped
                .push((e.entity_id.clone(), SkipReason::InvalidEntityId));
            continue;
        };
        if ghosts.contains(e.entity_id.as_str()) {
            out.skipped.push((e.entity_id.clone(), SkipReason::Ghost));
            continue;
        }
        if e.platform == "portainer" && is_container_hash_prefixed(entity_id.object_id()) {
            out.skipped
                .push((e.entity_id.clone(), SkipReason::PortainerOrphan));
            continue;
        }
        if let Some(d) = e.device_id.as_deref() {
            device_platform.entry(d).or_insert(e.platform.as_str());
        }
        out.entities.push(EntityEntry {
            uid: e.id.clone(),
            entity_id,
            integration: ha_integration(&e.platform),
            unique_id: e.unique_id.clone(),
            device_id: e.device_id.clone(),
            // Only a name chosen by the user: the integration default
            // (`original_name`) lacks the device prefix HA adds to friendly_name.
            name: e.name.clone(),
            area_id: e.area_id.clone(),
            labels: e.labels.clone(),
            icon: e.icon.clone(),
            hidden: e.hidden_by.is_some(),
            disabled: e.disabled_by.is_some(),
            status: EntityStatus::Active,
        });
    }

    // Devices without any kept entity are dead weight (removed containers, …).
    out.devices = snap
        .devices
        .iter()
        .filter_map(|d| {
            let platform = device_platform.get(d.id.as_str())?;
            Some(Device {
                id: d.id.clone(),
                integration: ha_integration(platform),
                name: d
                    .name_by_user
                    .clone()
                    .or_else(|| d.name.clone())
                    .unwrap_or_else(|| d.id.clone()),
                manufacturer: d.manufacturer.clone(),
                model: d.model.clone(),
                sw_version: d.sw_version.clone(),
                area_id: d.area_id.clone(),
                labels: d.labels.clone(),
                disabled: d.disabled_by.is_some(),
            })
        })
        .collect();

    out
}

fn is_ghost(state: &State) -> bool {
    state
        .attributes
        .get("restored")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
}

/// Portainer names entities of recreated containers with the container id
/// (12+ hex chars) as prefix, e.g. `sensor.562c5fc3f366_frigate_…`.
fn is_container_hash_prefixed(object_id: &str) -> bool {
    let prefix = object_id.split('_').next().unwrap_or_default();
    prefix.len() >= 12 && prefix.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HaArea, HaDevice, HaEntity, HaFloor};
    use chrono::Utc;
    use rusthome_model::Context;

    fn ent(id: &str, entity_id: &str, platform: &str, device: Option<&str>) -> HaEntity {
        HaEntity {
            id: id.into(),
            entity_id: entity_id.into(),
            platform: platform.into(),
            device_id: device.map(Into::into),
            ..HaEntity::default()
        }
    }

    fn state(entity_id: &str, attrs: &serde_json::Value) -> State {
        State {
            entity_id: EntityId::parse(entity_id).unwrap(),
            state: "unavailable".into(),
            attributes: attrs.as_object().cloned().unwrap_or_default(),
            last_changed: Utc::now(),
            last_updated: Utc::now(),
            context: Context::default(),
        }
    }

    #[test]
    fn skips_ghosts_portainer_orphans_and_invalid_ids() {
        let snap = HaSnapshot {
            entities: vec![
                ent("1", "media_player.emby_old", "emby", None),
                ent(
                    "2",
                    "sensor.562c5fc3f366_frigate_cpu",
                    "portainer",
                    Some("dc"),
                ),
                ent("3", "sensor.frigate_cpu", "portainer", Some("dc")),
                ent("4", "Bad.Id", "x", None),
                ent("5", "light.salon", "hue", Some("dh")),
            ],
            states: vec![state(
                "media_player.emby_old",
                &serde_json::json!({"restored": true}),
            )],
            ..HaSnapshot::default()
        };
        let r = import_registries(&snap);
        let kept: Vec<_> = r.entities.iter().map(|e| e.entity_id.to_string()).collect();
        assert_eq!(kept, ["sensor.frigate_cpu", "light.salon"]);
        assert_eq!(
            r.skipped,
            vec![
                ("media_player.emby_old".into(), SkipReason::Ghost),
                (
                    "sensor.562c5fc3f366_frigate_cpu".into(),
                    SkipReason::PortainerOrphan
                ),
                ("Bad.Id".into(), SkipReason::InvalidEntityId),
            ]
        );
    }

    #[test]
    fn hash_prefix_needs_twelve_hex_chars() {
        assert!(is_container_hash_prefixed("562c5fc3f366_frigate"));
        assert!(!is_container_hash_prefixed("frigate_cpu"));
        assert!(!is_container_hash_prefixed("deadbeef_x")); // too short
    }

    #[test]
    fn devices_without_kept_entities_are_dropped() {
        let snap = HaSnapshot {
            devices: vec![
                HaDevice {
                    id: "dh".into(),
                    name: Some("Hue Salon".into()),
                    ..HaDevice::default()
                },
                HaDevice {
                    id: "dead".into(),
                    name: Some("Old container".into()),
                    ..HaDevice::default()
                },
            ],
            entities: vec![ent("5", "light.salon", "hue", Some("dh"))],
            ..HaSnapshot::default()
        };
        let r = import_registries(&snap);
        assert_eq!(r.devices.len(), 1);
        assert_eq!(r.devices[0].integration, "ha:hue");
    }

    #[test]
    fn user_names_win_and_flags_are_mapped() {
        let mut e = ent("9", "switch.prise", "mqtt", Some("dz"));
        e.original_name = Some("Prise".into());
        e.name = Some("Prise lampadaire".into());
        e.disabled_by = Some("user".into());
        e.hidden_by = Some("integration".into());
        let snap = HaSnapshot {
            devices: vec![HaDevice {
                id: "dz".into(),
                name: Some("0x00158d".into()),
                name_by_user: Some("Prise lampadaire".into()),
                ..HaDevice::default()
            }],
            entities: vec![e],
            ..HaSnapshot::default()
        };
        let r = import_registries(&snap);
        let entity = &r.entities[0];
        assert_eq!(entity.name.as_deref(), Some("Prise lampadaire"));
        let mut default_only = ent("10", "sensor.x", "hue", None);
        default_only.original_name = Some("Printemps Floral".into());
        let r2 = import_registries(&HaSnapshot {
            entities: vec![default_only],
            ..HaSnapshot::default()
        });
        assert_eq!(
            r2.entities[0].name, None,
            "integration default name is not a user name"
        );
        assert!(entity.disabled && entity.hidden);
        assert_eq!(entity.uid, "9"); // HA registry id kept: resolves device-based automations later
        assert_eq!(r.devices[0].name, "Prise lampadaire");
    }

    /// Floors and areas from the real inventory (same field names as HA).
    #[test]
    fn imports_inventory_floors_and_areas() {
        let raw: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/floors_areas.json")).unwrap();
        let floors: Vec<HaFloor> = serde_json::from_value(raw["floors"].clone()).unwrap();
        let areas: Vec<HaArea> = serde_json::from_value(raw["areas"].clone()).unwrap();
        let r = import_registries(&HaSnapshot {
            floors,
            areas,
            ..HaSnapshot::default()
        });
        assert_eq!(r.floors.len(), 2);
        assert_eq!(r.areas.len(), 10);
        let floor_ids: HashSet<_> = r.floors.iter().map(|f| f.id.as_str()).collect();
        assert!(
            r.areas
                .iter()
                .all(|a| a.floor_id.as_deref().is_none_or(|f| floor_ids.contains(f)))
        );
    }
}
