//! Home Assistant automation configs → canonical `RustHome` documents.
//!
//! The stored document stays HA-shaped (the YAML editor round-trips it), in
//! the modern spelling: `triggers`/`conditions`/`actions`, `trigger:` and
//! `action:` keys. Legacy keys (`platform`, `service`, singular lists) are
//! rewritten, and `device` references are translated to entity ids and MQTT
//! topics: device ids are forbidden after import (cf. spec §6.1).

use serde_json::{Map, Value, json};

/// What the importer needs from the registries. Implemented over the hub.
pub trait Resolver {
    /// HA entity registry id (32 hex chars) → `entity_id`.
    fn entity_by_registry_id(&self, uid: &str) -> Option<String>;
    /// Zigbee IEEE address → Zigbee2MQTT friendly name.
    fn z2m_name(&self, ieee: &str) -> Option<String>;
    /// Integration `unique_id` → `entity_id` (Hue buttons…).
    fn entity_by_unique_id(&self, unique_id: &str) -> Option<String>;
    /// HA device id of a Zigbee2MQTT device → exact Zigbee2MQTT friendly name
    /// (older device triggers carry no `discovery_id`).
    fn z2m_name_for_ha_device(&self, device_id: &str) -> Option<String>;
    /// Blueprint YAML source by its HA path (`awtrix_random_effect.yaml`).
    fn blueprint(&self, path: &str) -> Option<String>;
}

/// One thing the importer could not translate. The automation is still
/// imported (disabled) so nothing is lost; the issue is shown to the user.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Issue {
    pub path: String,
    pub message: String,
}

pub struct Normalizer<'a> {
    resolver: &'a dyn Resolver,
    z2m_base: String,
    pub issues: Vec<Issue>,
}

impl<'a> Normalizer<'a> {
    #[must_use]
    pub fn new(resolver: &'a dyn Resolver, z2m_base: &str) -> Self {
        Self {
            resolver,
            z2m_base: z2m_base.to_owned(),
            issues: Vec::new(),
        }
    }

    fn issue(&mut self, path: &str, message: impl Into<String>) {
        self.issues.push(Issue {
            path: path.to_owned(),
            message: message.into(),
        });
    }

    /// Normalizes a whole automation config (blueprints expanded first).
    pub fn automation(&mut self, cfg: &Value) -> Value {
        if let Some(path) = cfg["use_blueprint"]["path"].as_str() {
            let expanded = self
                .resolver
                .blueprint(path)
                .ok_or_else(|| format!("blueprint {path} not available"))
                .and_then(|src| crate::blueprint::expand(cfg, &src));
            match expanded {
                Ok(plain) => return self.automation(&plain),
                Err(e) => self.issue("use_blueprint", e),
            }
        }
        let mut out = Map::new();
        for key in [
            "id",
            "alias",
            "description",
            "mode",
            "max",
            "max_exceeded",
            "variables",
            "trigger_variables",
            "category",
        ] {
            if let Some(v) = cfg.get(key) {
                out.insert(key.into(), v.clone());
            }
        }
        out.entry("mode").or_insert_with(|| json!("single"));
        let triggers = list(cfg.get("triggers").or_else(|| cfg.get("trigger")));
        let conditions = list(cfg.get("conditions").or_else(|| cfg.get("condition")));
        let actions = list(cfg.get("actions").or_else(|| cfg.get("action")));
        out.insert(
            "triggers".into(),
            Value::from(
                triggers
                    .iter()
                    .enumerate()
                    .map(|(i, t)| self.trigger(t, &format!("triggers[{i}]")))
                    .collect::<Vec<_>>(),
            ),
        );
        out.insert(
            "conditions".into(),
            Value::from(
                conditions
                    .iter()
                    .enumerate()
                    .map(|(i, c)| self.condition(c, &format!("conditions[{i}]")))
                    .collect::<Vec<_>>(),
            ),
        );
        out.insert("actions".into(), self.actions(&actions, "actions"));
        canonical_order(&Value::Object(out))
    }

    fn trigger(&mut self, t: &Value, path: &str) -> Value {
        let mut t = rename(t, "platform", "trigger");
        if t["trigger"] == "device" {
            t = self.device_trigger(&t, path);
        }
        // `trigger: zone.left` style shorthands are kept as-is (dead automations).
        t
    }

    fn device_trigger(&mut self, t: &Value, path: &str) -> Value {
        let keep_id = |mut v: Value| {
            if let Some(id) = t.get("id") {
                v["id"] = id.clone();
            }
            v
        };
        match (t["domain"].as_str(), t["type"].as_str()) {
            // Zigbee2MQTT button: `discovery_id` = "<ieee> action_<subtype>".
            (Some("mqtt"), Some("action")) => {
                let ieee = t["discovery_id"]
                    .as_str()
                    .and_then(|d| d.split_whitespace().next())
                    .unwrap_or_default();
                let subtype = t["subtype"].as_str().unwrap_or_default();
                let name = if ieee.is_empty() {
                    t["device_id"]
                        .as_str()
                        .and_then(|d| self.resolver.z2m_name_for_ha_device(d))
                } else {
                    self.resolver.z2m_name(ieee)
                };
                if let Some(name) = name {
                    keep_id(json!({
                        "trigger": "mqtt",
                        "topic": format!("{}/{name}/action", self.z2m_base),
                        "payload": subtype,
                    }))
                } else {
                    let who = if ieee.is_empty() {
                        t["device_id"].as_str().unwrap_or("?")
                    } else {
                        ieee
                    };
                    self.issue(
                        path,
                        format!("Zigbee device {who} not found (button {subtype})"),
                    );
                    t.clone()
                }
            }
            // Hue button: the device trigger carries the button resource id.
            (Some("hue"), Some(event_type)) => {
                let unique = t["unique_id"].as_str().unwrap_or_default();
                if let Some(entity_id) = self.resolver.entity_by_unique_id(unique) {
                    keep_id(json!({
                        "trigger": "event_entity",
                        "entity_id": entity_id,
                        "event_type": event_type,
                    }))
                } else {
                    self.issue(path, format!("Hue button {unique} not found"));
                    t.clone()
                }
            }
            // binary_sensor motion / no_motion → state on/off (+ for).
            (
                Some("binary_sensor"),
                Some(
                    kind @ ("motion" | "no_motion" | "opened" | "not_opened" | "turned_on"
                    | "turned_off"),
                ),
            ) => {
                let to = if matches!(kind, "motion" | "opened" | "turned_on") {
                    "on"
                } else {
                    "off"
                };
                match self.entity(t, path) {
                    Some(entity_id) => {
                        let mut v =
                            keep_id(json!({"trigger": "state", "entity_id": entity_id, "to": to}));
                        if let Some(f) = t.get("for") {
                            v["for"] = f.clone();
                        }
                        v
                    }
                    None => t.clone(),
                }
            }
            (domain, kind) => {
                self.issue(
                    path,
                    format!("unsupported device trigger {domain:?}/{kind:?}"),
                );
                t.clone()
            }
        }
    }

    fn condition(&mut self, c: &Value, path: &str) -> Value {
        // Shorthand template condition: a bare string.
        if let Some(s) = c.as_str() {
            return json!({"condition": "template", "value_template": s});
        }
        match c["condition"].as_str() {
            Some("device") => {
                let state = match c["type"].as_str() {
                    Some("is_on" | "is_motion" | "is_open") => "on",
                    Some("is_off" | "is_no_motion" | "is_not_open") => "off",
                    other => {
                        self.issue(path, format!("unsupported device condition {other:?}"));
                        return c.clone();
                    }
                };
                match self.entity(c, path) {
                    Some(entity_id) => {
                        json!({"condition": "state", "entity_id": entity_id, "state": state})
                    }
                    None => c.clone(),
                }
            }
            Some("and" | "or" | "not") => {
                let mut out = c.clone();
                let subs = list(c.get("conditions"));
                out["conditions"] = Value::from(
                    subs.iter()
                        .enumerate()
                        .map(|(i, s)| self.condition(s, &format!("{path}.conditions[{i}]")))
                        .collect::<Vec<_>>(),
                );
                out
            }
            _ => c.clone(),
        }
    }

    fn actions(&mut self, actions: &[Value], path: &str) -> Value {
        Value::from(
            actions
                .iter()
                .enumerate()
                .map(|(i, a)| self.action(a, &format!("{path}[{i}]")))
                .collect::<Vec<_>>(),
        )
    }

    fn action(&mut self, a: &Value, path: &str) -> Value {
        let mut a = rename(a, "service", "action");
        // Legacy `data_template` / `service_template`.
        if let Some(d) = a.as_object_mut().and_then(|o| o.remove("data_template")) {
            a["data"] = d;
        }
        // Device action → service call on the entity.
        if a.get("device_id").is_some()
            && a.get("domain").is_some()
            && a.get("type").is_some()
            && a.get("action").is_none()
        {
            let (domain, kind) = (
                a["domain"].as_str().unwrap_or_default().to_owned(),
                a["type"].as_str().unwrap_or_default().to_owned(),
            );
            if !matches!(kind.as_str(), "turn_on" | "turn_off" | "toggle") {
                self.issue(path, format!("unsupported device action {domain}/{kind}"));
                return a;
            }
            return match self.entity(&a, path) {
                Some(entity_id) => {
                    let mut v = json!({"action": format!("{domain}.{kind}"), "target": {"entity_id": entity_id}});
                    copy_flags(&a, &mut v);
                    v
                }
                None => a,
            };
        }
        // Nested blocks.
        if let Some(opts) = a.get("choose").map(|c| list(Some(c))) {
            let normalized: Vec<Value> = opts
                .iter()
                .enumerate()
                .map(|(i, o)| {
                    let p = format!("{path}.choose[{i}]");
                    let conds = list(o.get("conditions"));
                    json!({
                        "conditions": conds.iter().enumerate().map(|(j, c)| self.condition(c, &format!("{p}.conditions[{j}]"))).collect::<Vec<_>>(),
                        "sequence": self.actions(&list(o.get("sequence")), &format!("{p}.sequence")),
                    })
                })
                .collect();
            a["choose"] = Value::from(normalized);
            if let Some(d) = a.get("default").map(|d| list(Some(d))) {
                a["default"] = self.actions(&d, &format!("{path}.default"));
            }
        }
        if a.get("if").is_some() {
            let conds = list(a.get("if"));
            a["if"] = Value::from(
                conds
                    .iter()
                    .enumerate()
                    .map(|(j, c)| self.condition(c, &format!("{path}.if[{j}]")))
                    .collect::<Vec<_>>(),
            );
            for branch in ["then", "else"] {
                if let Some(b) = a.get(branch).map(|b| list(Some(b))) {
                    a[branch] = self.actions(&b, &format!("{path}.{branch}"));
                }
            }
        }
        if let Some(r) = a.get("repeat").cloned() {
            let mut r = r;
            if let Some(seq) = r.get("sequence").map(|s| list(Some(s))) {
                r["sequence"] = self.actions(&seq, &format!("{path}.repeat.sequence"));
            }
            for key in ["while", "until"] {
                if let Some(cs) = r.get(key).map(|c| list(Some(c))) {
                    r[key] = Value::from(
                        cs.iter()
                            .enumerate()
                            .map(|(j, c)| self.condition(c, &format!("{path}.repeat.{key}[{j}]")))
                            .collect::<Vec<_>>(),
                    );
                }
            }
            a["repeat"] = r;
        }
        if let Some(branches) = a.get("parallel").map(|p| list(Some(p))) {
            a["parallel"] = Value::from(
                branches
                    .iter()
                    .enumerate()
                    .map(|(i, b)| {
                        // A branch is a single action or `{sequence: [...]}`.
                        if let Some(seq) = b.get("sequence") {
                            json!({"sequence": self.actions(&list(Some(seq)), &format!("{path}.parallel[{i}].sequence"))})
                        } else {
                            self.action(b, &format!("{path}.parallel[{i}]"))
                        }
                    })
                    .collect::<Vec<_>>(),
            );
        }
        if let Some(seq) = a.get("sequence").map(|s| list(Some(s))) {
            a["sequence"] = self.actions(&seq, &format!("{path}.sequence"));
        }
        if a.get("condition").is_some() {
            return self.condition(&a, path);
        }
        a
    }

    /// Entity referenced by a device block: an `entity_id` that is either a
    /// real id or an HA registry id (32 hex chars).
    fn entity(&mut self, v: &Value, path: &str) -> Option<String> {
        let raw = v["entity_id"].as_str()?;
        if raw.contains('.') {
            return Some(raw.to_owned());
        }
        let found = self.resolver.entity_by_registry_id(raw);
        if found.is_none() {
            self.issue(path, format!("entity with registry id {raw} not found"));
        }
        found
    }
}

fn copy_flags(from: &Value, to: &mut Value) {
    for k in ["continue_on_error", "enabled", "alias"] {
        if let Some(v) = from.get(k) {
            to[k] = v.clone();
        }
    }
}

/// Readable key order for the YAML editor: document keys in HA's order and,
/// in every block, its type key first (`trigger:`, `condition:`, `action:`).
#[must_use]
pub fn canonical_order(v: &Value) -> Value {
    const TOP: [&str; 10] = [
        "id",
        "alias",
        "description",
        "mode",
        "max",
        "variables",
        "category",
        "triggers",
        "conditions",
        "actions",
    ];
    const FIRST: [&str; 13] = [
        "trigger",
        "condition",
        "action",
        "alias",
        "choose",
        "if",
        "repeat",
        "parallel",
        "sequence",
        "delay",
        "variables",
        "wait_template",
        "stop",
    ];
    match v {
        Value::Object(o) => {
            let priority: &[&str] = if o.contains_key("triggers") || o.contains_key("actions") {
                &TOP
            } else {
                &FIRST
            };
            let mut out = Map::new();
            for k in priority {
                if let Some(x) = o.get(*k) {
                    out.insert((*k).to_owned(), canonical_order(x));
                }
            }
            for (k, x) in o {
                if !out.contains_key(k) {
                    out.insert(k.clone(), canonical_order(x));
                }
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(canonical_order).collect()),
        other => other.clone(),
    }
}

/// `x`, `[x, y]` or nothing → list.
#[must_use]
pub fn list(v: Option<&Value>) -> Vec<Value> {
    match v {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(a)) => a.clone(),
        Some(other) => vec![other.clone()],
    }
}

fn rename(v: &Value, from: &str, to: &str) -> Value {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut()
        && !o.contains_key(to)
        && let Some(x) = o.remove(from)
    {
        o.insert(to.to_owned(), x);
    }
    v
}
