//! Automation engine (M3, cf. docs/architecture.md §6).
//!
//! Pipeline: HA config → [`normalize`] (canonical HA-shaped document, device
//! references translated) → [`compile`] (typed, validated) → engine.

pub mod blueprint;
pub mod compile;
pub mod engine;
pub mod exec;
pub mod import;
pub mod normalize;

pub use compile::{Automation, CompileError, compile};
pub use engine::{Engine, Info, Trace};
pub use normalize::{Issue, Normalizer, Resolver};

#[cfg(test)]
mod corpus_tests {
    use std::collections::HashMap;

    use serde_json::Value;

    use super::*;

    /// Registry extract of the real installation (see tests/fixtures).
    struct FixtureResolver {
        by_uid: HashMap<String, String>,
        by_unique: HashMap<String, String>,
        z2m: HashMap<String, String>,
        ha_devices: HashMap<String, String>,
    }

    impl FixtureResolver {
        fn load() -> Self {
            let reg: Value =
                serde_json::from_str(include_str!("../tests/fixtures/registry.json")).unwrap();
            let mut r = Self {
                by_uid: HashMap::new(),
                by_unique: HashMap::new(),
                z2m: HashMap::new(),
                ha_devices: HashMap::new(),
            };
            for e in reg["entities"].as_array().unwrap() {
                let id = e["entity_id"].as_str().unwrap().to_owned();
                r.by_uid
                    .insert(e["uid"].as_str().unwrap().to_owned(), id.clone());
                if let Some(u) = e["unique_id"].as_str() {
                    r.by_unique.insert(u.to_owned(), id);
                }
            }
            for d in reg["devices"].as_array().unwrap() {
                let (id, name) = (
                    d["id"].as_str().unwrap(),
                    d["name"].as_str().unwrap().to_owned(),
                );
                match id.strip_prefix("z2m-") {
                    Some(ieee) => {
                        r.z2m.insert(ieee.to_owned(), name);
                    }
                    None => {
                        r.ha_devices.insert(id.to_owned(), name);
                    }
                }
            }
            r
        }
    }

    impl Resolver for FixtureResolver {
        fn entity_by_registry_id(&self, uid: &str) -> Option<String> {
            self.by_uid.get(uid).cloned()
        }
        fn z2m_name(&self, ieee: &str) -> Option<String> {
            self.z2m.get(ieee).cloned()
        }
        fn entity_by_unique_id(&self, unique_id: &str) -> Option<String> {
            self.by_unique.get(unique_id).cloned()
        }
        fn blueprint(&self, path: &str) -> Option<String> {
            (path == "awtrix_random_effect.yaml").then(|| {
                include_str!("../tests/fixtures/blueprints/awtrix_random_effect.yaml").to_owned()
            })
        }
        fn z2m_name_for_ha_device(&self, device_id: &str) -> Option<String> {
            let ha_name = self.ha_devices.get(device_id)?;
            self.z2m
                .values()
                .find(|n| n.trim() == ha_name.trim())
                .cloned()
        }
    }

    fn corpus() -> Vec<Value> {
        serde_json::from_str(include_str!("../tests/fixtures/automations.json")).unwrap()
    }

    #[test]
    fn whole_real_corpus_imports() {
        let resolver = FixtureResolver::load();
        let mut ok = 0;
        let mut failures = Vec::new();
        let mut issues = Vec::new();
        for a in corpus() {
            let mut n = Normalizer::new(&resolver, "zigbee2mqtt");
            let doc = n.automation(&a["config"]);
            issues.extend(
                n.issues
                    .iter()
                    .map(|i| format!("{} {}: {}", a["entity_id"], i.path, i.message)),
            );
            match compile(&doc) {
                Ok(_) => ok += 1,
                Err(errs) => failures.push(format!("{}: {errs:?}", a["entity_id"])),
            }
            // No device id survives normalization when it was resolvable.
            if n.issues.is_empty() {
                assert!(
                    !doc.to_string().contains("\"device_id\""),
                    "{}",
                    a["entity_id"]
                );
            }
        }
        // The only known loss: one entity deleted from HA (c6456eb2…), used
        // 4 times by `presence_bureau` and `test_cube`; both stay importable
        // (disabled, with the issue shown) but do not compile until fixed.
        assert_eq!(issues.len(), 4, "{issues:#?}");
        assert!(
            issues
                .iter()
                .all(|i| i.contains("c6456eb2cb409c77637d4a74d6ef1aed")),
            "{issues:#?}"
        );
        assert_eq!(ok, 41, "{failures:#?}"); // incl. the expanded Awtrix blueprint
        assert_eq!(failures.len(), 2);
    }

    #[test]
    fn zigbee_button_becomes_mqtt_trigger() {
        let resolver = FixtureResolver::load();
        let mut n = Normalizer::new(&resolver, "zigbee2mqtt");
        let doc = n.automation(&serde_json::json!({
            "trigger": [{"platform": "device", "domain": "mqtt", "type": "action", "subtype": "double_left",
                          "device_id": "cc93", "discovery_id": "0x54ef4410002a0654 action_double_left"}],
            "action": [{"service": "light.toggle", "target": {"entity_id": "light.salon"}}]
        }));
        assert_eq!(doc["triggers"][0]["trigger"], "mqtt");
        assert_eq!(
            doc["triggers"][0]["topic"],
            "zigbee2mqtt/Interrupteur Living/action"
        );
        assert_eq!(doc["triggers"][0]["payload"], "double_left");
        assert_eq!(
            doc["actions"][0]["action"], "light.toggle",
            "legacy `service` renamed"
        );
        assert!(compile(&doc).is_ok());
    }

    #[test]
    fn button_without_discovery_id_keeps_exact_name() {
        let resolver = FixtureResolver::load();
        let mut n = Normalizer::new(&resolver, "zigbee2mqtt");
        let doc = n.automation(&serde_json::json!({
            "triggers": [{"trigger": "device", "domain": "mqtt", "type": "action", "subtype": "single_left",
                          "device_id": "135faf540e6b9f373faeb5671efd1597"}],
            "actions": []
        }));
        // Trailing space is part of the real Zigbee2MQTT name, hence of the topic.
        assert_eq!(
            doc["triggers"][0]["topic"],
            "zigbee2mqtt/Interrupteur sans fil /action"
        );
    }

    #[test]
    fn yaml_key_order_is_readable() {
        // Alphabetical input (as stored before `preserve_order`).
        let doc = serde_json::json!({
            "actions": [{"target": {"entity_id": "switch.x"}, "action": "switch.toggle"}],
            "alias": "A", "id": "1", "mode": "single",
            "triggers": [{"payload": "single_left", "topic": "t", "trigger": "mqtt"}]
        });
        let yaml = serde_yaml::to_string(&normalize::canonical_order(&doc)).unwrap();
        let order: Vec<&str> = yaml
            .lines()
            .filter(|l| !l.starts_with(' ') && !l.starts_with('-'))
            .map(|l| l.split(':').next().unwrap())
            .collect();
        assert_eq!(order, ["id", "alias", "mode", "triggers", "actions"]);
        assert!(yaml.contains("- trigger: mqtt\n  payload"), "{yaml}");
        assert!(yaml.contains("- action: switch.toggle\n  target"), "{yaml}");
    }

    #[test]
    fn compile_reports_paths() {
        let doc = serde_json::json!({
            "triggers": [{"trigger": "state", "entity_id": "Not An Id"}],
            "actions": [{"repeat": {"sequence": []}}]
        });
        let errs = compile(&doc).unwrap_err();
        let paths: Vec<_> = errs.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["triggers[0].entity_id", "actions[0].repeat"]);
    }

    #[test]
    fn durations() {
        use compile::{Dur, duration};
        use std::time::Duration;
        let d = |v: Value| duration(&v, "x").unwrap();
        assert_eq!(
            d(serde_json::json!("00:05:00")),
            Dur::Fixed(Duration::from_secs(300))
        );
        assert_eq!(
            d(serde_json::json!({"hours": 1, "minutes": 0, "seconds": 0})),
            Dur::Fixed(Duration::from_secs(3600))
        );
        assert_eq!(d(serde_json::json!(8)), Dur::Fixed(Duration::from_secs(8)));
        assert!(matches!(d(serde_json::json!("{{ 5 }}")), Dur::Template(_)));
    }
}
