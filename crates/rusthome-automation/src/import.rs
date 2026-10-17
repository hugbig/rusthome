//! Import from Home Assistant and persistence records.

use std::path::PathBuf;
use std::sync::Arc;

use rusthome_core::Hub;
use rusthome_model::EntityId;
use serde::Serialize;
use serde_json::{Value, json};

use crate::engine::Engine;
use crate::normalize::{Issue, Normalizer, Resolver};

/// Resolver over the hub registries, plus a directory of blueprint files
/// (HA's `/config/blueprints/automation/...`, copied by the user).
pub struct HubResolver {
    pub hub: Hub,
    pub blueprints_dir: Option<PathBuf>,
}

impl Resolver for HubResolver {
    fn entity_by_registry_id(&self, uid: &str) -> Option<String> {
        self.hub
            .registries
            .entity(uid)
            .map(|e| e.entity_id.to_string())
    }

    fn z2m_name(&self, ieee: &str) -> Option<String> {
        self.hub
            .registries
            .device(&format!("z2m-{ieee}"))
            .map(|d| d.name)
    }

    fn entity_by_unique_id(&self, unique_id: &str) -> Option<String> {
        self.hub
            .registries
            .entity_by_unique_id(unique_id)
            .map(|e| e.entity_id.to_string())
    }

    fn z2m_name_for_ha_device(&self, device_id: &str) -> Option<String> {
        let ha_name = self.hub.registries.device(device_id)?.name;
        self.hub
            .registries
            .devices()
            .into_iter()
            .find(|d| d.integration == "z2m" && d.name.trim() == ha_name.trim())
            .map(|d| d.name)
    }

    fn blueprint(&self, path: &str) -> Option<String> {
        let dir = self.blueprints_dir.as_ref()?;
        // Only a file name inside the directory: no path traversal.
        let name = std::path::Path::new(path).file_name()?;
        std::fs::read_to_string(dir.join(name)).ok()
    }
}

/// Persisted form of one automation.
#[must_use]
pub fn record(doc: &Value, entity_id: &EntityId, enabled: bool, issues: &[Issue]) -> Value {
    json!({ "doc": doc, "entity_id": entity_id, "enabled": enabled, "issues": issues })
}

/// Loads a persisted record into the engine.
pub fn load_record(engine: &Engine, rec: &Value) -> Option<String> {
    let doc = rec.get("doc")?.clone();
    let entity_id = EntityId::parse(rec["entity_id"].as_str()?).ok()?;
    let issues: Vec<Issue> = serde_json::from_value(rec["issues"].clone()).unwrap_or_default();
    let info = engine.upsert(doc, entity_id, rec["enabled"] == true, issues);
    Some(info.id)
}

#[derive(Debug, Default, Serialize)]
pub struct ImportReport {
    pub imported: usize,
    pub skipped_existing: usize,
    pub with_issues: usize,
    pub invalid: usize,
    pub ids: Vec<String>,
}

/// One HA automation to import.
pub struct Source {
    pub entity_id: String,
    pub config: Value,
}

/// Imports HA automations **disabled** (phase P2: the user enables them one
/// by one). Existing ids are kept unless `overwrite`.
pub fn import(
    engine: &Arc<Engine>,
    resolver: &dyn Resolver,
    z2m_base: &str,
    sources: &[Source],
    overwrite: bool,
) -> (ImportReport, Vec<(String, Value)>) {
    let mut report = ImportReport::default();
    let mut records = Vec::new();
    for s in sources {
        let mut n = Normalizer::new(resolver, z2m_base);
        let doc = n.automation(&s.config);
        let id = doc["id"]
            .as_str()
            .map_or_else(|| doc["id"].to_string(), str::to_owned);
        if !overwrite && engine.info(&id).is_some() {
            report.skipped_existing += 1;
            continue;
        }
        let Ok(entity_id) = EntityId::parse(&s.entity_id) else {
            continue;
        };
        let info = engine.upsert(doc.clone(), entity_id.clone(), false, n.issues.clone());
        if !n.issues.is_empty() {
            report.with_issues += 1;
        }
        if !info.valid {
            report.invalid += 1;
        }
        report.imported += 1;
        report.ids.push(id.clone());
        records.push((id, record(&doc, &entity_id, false, &n.issues)));
    }
    (report, records)
}

/// `automation.<slug(alias)>` for automations created in RustHome.
#[must_use]
pub fn entity_id_for_alias(alias: &str, taken: impl Fn(&str) -> bool) -> EntityId {
    let mut slug = String::new();
    for c in alias.trim().to_lowercase().chars() {
        let c = match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        };
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('_') {
            slug.push('_');
        }
    }
    let base = format!("automation.{}", slug.trim_matches('_'));
    let base = if base == "automation." {
        "automation.sans_nom".to_owned()
    } else {
        base
    };
    let candidate = (1..1000)
        .map(|n| {
            if n == 1 {
                base.clone()
            } else {
                format!("{base}_{n}")
            }
        })
        .find(|c| !taken(c))
        .unwrap_or(base);
    EntityId::parse(&candidate).unwrap_or_else(|_| {
        EntityId::parse("automation.sans_nom").unwrap_or_else(|_| unreachable!())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusthome_core::SystemClock;

    struct NoResolve;
    impl Resolver for NoResolve {
        fn entity_by_registry_id(&self, _: &str) -> Option<String> {
            None
        }
        fn z2m_name(&self, _: &str) -> Option<String> {
            None
        }
        fn entity_by_unique_id(&self, _: &str) -> Option<String> {
            None
        }
        fn z2m_name_for_ha_device(&self, _: &str) -> Option<String> {
            None
        }
        fn blueprint(&self, _: &str) -> Option<String> {
            None
        }
    }

    #[tokio::test]
    async fn imports_disabled_and_keeps_existing() {
        let hub = Hub::new(Arc::new(SystemClock));
        let engine = Engine::new(hub, chrono_tz::Europe::Paris);
        let src = vec![Source {
            entity_id: "automation.tireuse".into(),
            config: json!({"id": "t1", "alias": "Tireuse", "trigger": [{"platform": "state", "entity_id": "sensor.x"}], "action": []}),
        }];
        let (r, records) = import(&engine, &NoResolve, "zigbee2mqtt", &src, false);
        assert_eq!((r.imported, r.invalid), (1, 0));
        assert!(!engine.info("t1").unwrap().enabled, "imported disabled");
        assert_eq!(records[0].1["enabled"], false);
        let (again, _) = import(&engine, &NoResolve, "zigbee2mqtt", &src, false);
        assert_eq!(again.skipped_existing, 1, "user edits are not overwritten");
        // Round trip through the persisted record.
        let engine2 = Engine::new(Hub::new(Arc::new(SystemClock)), chrono_tz::Europe::Paris);
        assert_eq!(load_record(&engine2, &records[0].1).as_deref(), Some("t1"));
    }

    #[test]
    fn entity_ids_from_alias() {
        let taken = |s: &str| s == "automation.reset_tireuse";
        assert_eq!(
            entity_id_for_alias("Éteindre radiateur salon", |_| false).as_str(),
            "automation.eteindre_radiateur_salon"
        );
        assert_eq!(
            entity_id_for_alias("Reset tireuse", taken).as_str(),
            "automation.reset_tireuse_2"
        );
        assert_eq!(
            entity_id_for_alias("???", |_| false).as_str(),
            "automation.sans_nom"
        );
    }

    #[test]
    fn blueprint_paths_cannot_escape_the_directory() {
        let hub = Hub::new(Arc::new(SystemClock));
        let r = HubResolver {
            hub,
            blueprints_dir: Some(std::env::temp_dir()),
        };
        assert!(r.blueprint("../../etc/passwd").is_none());
    }
}
