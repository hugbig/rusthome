use std::collections::BTreeMap;
use std::sync::{PoisonError, RwLock};

use rusthome_model::{Area, Device, EntityEntry, EntityId, Floor, Label};

use crate::{Event, EventBus, RegistryKind};

/// In-memory registries. Persistence is handled by `rusthome-recorder`,
/// which loads them at startup and listens to `RegistryUpdated`.
pub struct Registries {
    floors: RwLock<BTreeMap<String, Floor>>,
    areas: RwLock<BTreeMap<String, Area>>,
    labels: RwLock<BTreeMap<String, Label>>,
    devices: RwLock<BTreeMap<String, Device>>,
    /// Keyed by `uid` (stable across renames).
    entities: RwLock<BTreeMap<String, EntityEntry>>,
    bus: EventBus,
}

macro_rules! registry_methods {
    ($field:ident, $ty:ty, $kind:expr, $list:ident, $get:ident, $upsert:ident, $key:ident) => {
        #[must_use]
        pub fn $list(&self) -> Vec<$ty> {
            self.$field
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .values()
                .cloned()
                .collect()
        }

        #[must_use]
        pub fn $get(&self, id: &str) -> Option<$ty> {
            self.$field
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .get(id)
                .cloned()
        }

        /// Inserts or replaces; emits `RegistryUpdated` only on change.
        pub fn $upsert(&self, item: $ty) {
            let id = item.$key.clone();
            let changed = {
                let mut map = self.$field.write().unwrap_or_else(PoisonError::into_inner);
                map.insert(id.clone(), item.clone()).as_ref() != Some(&item)
            };
            if changed {
                self.bus.publish(Event::RegistryUpdated { kind: $kind, id });
            }
        }
    };
}

impl Registries {
    #[must_use]
    pub fn new(bus: EventBus) -> Self {
        Self {
            floors: RwLock::default(),
            areas: RwLock::default(),
            labels: RwLock::default(),
            devices: RwLock::default(),
            entities: RwLock::default(),
            bus,
        }
    }

    registry_methods!(
        floors,
        Floor,
        RegistryKind::Floor,
        floors,
        floor,
        upsert_floor,
        id
    );
    registry_methods!(
        areas,
        Area,
        RegistryKind::Area,
        areas,
        area,
        upsert_area,
        id
    );
    registry_methods!(
        labels,
        Label,
        RegistryKind::Label,
        labels,
        label,
        upsert_label,
        id
    );
    registry_methods!(
        devices,
        Device,
        RegistryKind::Device,
        devices,
        device,
        upsert_device,
        id
    );
    registry_methods!(
        entities,
        EntityEntry,
        RegistryKind::Entity,
        entities,
        entity,
        upsert_entity,
        uid
    );

    #[must_use]
    pub fn entity_by_entity_id(&self, entity_id: &EntityId) -> Option<EntityEntry> {
        self.entities
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .find(|e| &e.entity_id == entity_id)
            .cloned()
    }

    /// Entry carrying this integration `unique_id` (any integration): lets a
    /// native integration take over an entity mirrored from HA.
    #[must_use]
    pub fn entity_by_unique_id(&self, unique_id: &str) -> Option<EntityEntry> {
        self.entities
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .find(|e| e.unique_id.as_deref() == Some(unique_id))
            .cloned()
    }

    /// Effective area of an entity: its own, otherwise its device's.
    #[must_use]
    pub fn effective_area(&self, entry: &EntityEntry) -> Option<String> {
        entry.area_id.clone().or_else(|| {
            entry
                .device_id
                .as_deref()
                .and_then(|d| self.device(d))
                .and_then(|d| d.area_id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusthome_model::EntityStatus;

    fn entity(uid: &str, eid: &str, device: Option<&str>, area: Option<&str>) -> EntityEntry {
        EntityEntry {
            uid: uid.into(),
            entity_id: EntityId::parse(eid).unwrap(),
            integration: "test".into(),
            unique_id: None,
            device_id: device.map(Into::into),
            name: None,
            area_id: area.map(Into::into),
            labels: vec![],
            icon: None,
            hidden: false,
            disabled: false,
            status: EntityStatus::Active,
        }
    }

    #[test]
    fn upsert_emits_only_on_change() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();
        let reg = Registries::new(bus);
        let area = Area {
            id: "salon".into(),
            name: "Salon".into(),
            floor_id: None,
            icon: None,
            aliases: vec![],
        };
        reg.upsert_area(area.clone());
        assert!(rx.try_recv().is_ok());
        reg.upsert_area(area);
        assert!(rx.try_recv().is_err());
        assert_eq!(reg.areas().len(), 1);
    }

    #[test]
    fn effective_area_falls_back_to_device() {
        let reg = Registries::new(EventBus::new(16));
        reg.upsert_device(Device {
            id: "d1".into(),
            integration: "test".into(),
            name: "Prise".into(),
            manufacturer: None,
            model: None,
            sw_version: None,
            area_id: Some("salon".into()),
            labels: vec![],
            disabled: false,
        });
        let inherited = entity("u1", "switch.prise", Some("d1"), None);
        let own = entity("u2", "sensor.p", Some("d1"), Some("bureau"));
        assert_eq!(reg.effective_area(&inherited).as_deref(), Some("salon"));
        assert_eq!(reg.effective_area(&own).as_deref(), Some("bureau"));
    }

    #[test]
    fn finds_entity_by_entity_id() {
        let reg = Registries::new(EventBus::new(16));
        reg.upsert_entity(entity("u1", "light.salon", None, None));
        let found = reg.entity_by_entity_id(&EntityId::parse("light.salon").unwrap());
        assert_eq!(found.map(|e| e.uid).as_deref(), Some("u1"));
    }
}
