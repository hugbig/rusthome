//! Zigbee2MQTT `bridge/devices` → `RustHome` entities. Pure and unit-tested.
//!
//! Keys and `unique_id`s follow Zigbee2MQTT's Home Assistant discovery
//! (`{ieee}_{key}_zigbee2mqtt`) so entities already known to HA keep their
//! `entity_id` when RustHome takes them over (cf. docs/architecture.md §9).

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub struct Z2mDevice {
    pub ieee_address: String,
    pub friendly_name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub supported: bool,
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default)]
    pub definition: Option<Definition>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Definition {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub vendor: Option<String>,
    #[serde(default)]
    pub exposes: Vec<Expose>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Expose {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub property: Option<String>,
    #[serde(default)]
    pub access: u8,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub value_on: Option<Value>,
    #[serde(default)]
    pub value_off: Option<Value>,
    #[serde(default)]
    pub values: Vec<String>,
    #[serde(default)]
    pub value_min: Option<f64>,
    #[serde(default)]
    pub value_max: Option<f64>,
    #[serde(default)]
    pub value_step: Option<f64>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub features: Vec<Expose>,
}

/// Zigbee2MQTT access bits.
const ACCESS_SET: u8 = 0b010;

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// Main on/off of a plug or relay (`switch` expose).
    Switch {
        on: Value,
        off: Value,
    },
    Light {
        on: Value,
        off: Value,
    },
    /// Writable boolean option (`button_lock`, `auto_off`…), shown as a switch.
    OptionSwitch {
        on: Value,
        off: Value,
    },
    BinarySensor {
        on: Value,
        off: Value,
    },
    Sensor,
    Number {
        min: Option<f64>,
        max: Option<f64>,
        step: Option<f64>,
    },
    Select {
        options: Vec<String>,
    },
}

impl Kind {
    #[must_use]
    pub fn domain(&self) -> &'static str {
        match self {
            Kind::Switch { .. } | Kind::OptionSwitch { .. } => "switch",
            Kind::Light { .. } => "light",
            Kind::BinarySensor { .. } => "binary_sensor",
            Kind::Sensor => "sensor",
            Kind::Number { .. } => "number",
            Kind::Select { .. } => "select",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Z2mEntity {
    /// Discovery key: `switch`, `switch_auto_off`, `power`…
    pub key: String,
    /// Field of the device JSON payload.
    pub property: String,
    pub kind: Kind,
    pub unique_id: String,
    /// Display name, e.g. `Prise lampadaire Power`.
    pub name: String,
    /// `entity_id` used when HA never knew this entity.
    pub default_entity_id: String,
    pub unit: Option<String>,
    pub device_class: Option<&'static str>,
    pub config: bool,
    /// Hidden by default (link quality: noise, like in HA).
    pub hidden: bool,
}

/// Entities of one device. The coordinator, disabled and unsupported devices have none.
#[must_use]
pub fn entities_for(device: &Z2mDevice) -> Vec<Z2mEntity> {
    if device.kind == "Coordinator" || device.disabled {
        return Vec::new();
    }
    let Some(def) = &device.definition else {
        return Vec::new();
    };
    let slug = slugify(&device.friendly_name);
    let mut out = Vec::new();
    for e in &def.exposes {
        match e.kind.as_str() {
            "switch" | "light" => {
                let Some(state) = e.features.iter().find(|f| {
                    f.property
                        .as_deref()
                        .is_some_and(|p| p.starts_with("state"))
                }) else {
                    continue;
                };
                let (on, off) = on_off(state);
                let base = if e.kind == "switch" {
                    "switch"
                } else {
                    "light"
                };
                let key = match &e.endpoint {
                    Some(ep) => format!("{base}_{ep}"),
                    None => base.to_owned(),
                };
                let kind = if e.kind == "switch" {
                    Kind::Switch { on, off }
                } else {
                    Kind::Light { on, off }
                };
                let suffix = e
                    .endpoint
                    .as_ref()
                    .map(|ep| format!("_{ep}"))
                    .unwrap_or_default();
                out.push(Z2mEntity {
                    unique_id: unique_id(&device.ieee_address, &key),
                    default_entity_id: format!("{}.{slug}{suffix}", kind.domain()),
                    name: match &e.endpoint {
                        Some(ep) => format!("{} {ep}", device.friendly_name.trim()),
                        None => device.friendly_name.trim().to_owned(),
                    },
                    property: state.property.clone().unwrap_or_else(|| "state".into()),
                    key,
                    kind,
                    unit: None,
                    device_class: None,
                    config: false,
                    hidden: false,
                });
            }
            "binary" | "numeric" | "enum" | "text" => {
                if let Some(ent) = simple_entity(device, &slug, e) {
                    out.push(ent);
                }
            }
            // composite (siren `warning`), lists…: not mapped yet (M2 scope).
            _ => {}
        }
    }
    out
}

fn simple_entity(device: &Z2mDevice, slug: &str, e: &Expose) -> Option<Z2mEntity> {
    let property = e.property.clone()?;
    let writable = e.access & ACCESS_SET != 0;
    let (kind, key) = match (e.kind.as_str(), writable) {
        ("binary", true) => {
            let (on, off) = on_off(e);
            (Kind::OptionSwitch { on, off }, format!("switch_{property}"))
        }
        ("binary", false) => {
            let (on, off) = on_off(e);
            (Kind::BinarySensor { on, off }, property.clone())
        }
        ("numeric", true) => (
            Kind::Number {
                min: e.value_min,
                max: e.value_max,
                step: e.value_step,
            },
            property.clone(),
        ),
        ("enum", true) => (
            Kind::Select {
                options: e.values.clone(),
            },
            property.clone(),
        ),
        // read-only numeric/enum/text, and writable text (rare) as plain sensors
        _ => (Kind::Sensor, property.clone()),
    };
    let label = e.label.clone().unwrap_or_else(|| property.clone());
    Some(Z2mEntity {
        unique_id: unique_id(&device.ieee_address, &key),
        default_entity_id: format!("{}.{slug}_{property}", kind.domain()),
        name: format!("{} {label}", device.friendly_name.trim()),
        unit: e.unit.clone(),
        device_class: device_class(&property, &kind),
        config: e.category.as_deref() == Some("config"),
        hidden: property == "linkquality",
        property,
        key,
        kind,
    })
}

fn on_off(e: &Expose) -> (Value, Value) {
    (
        e.value_on.clone().unwrap_or_else(|| Value::from("ON")),
        e.value_off.clone().unwrap_or_else(|| Value::from("OFF")),
    )
}

#[must_use]
pub fn unique_id(ieee: &str, key: &str) -> String {
    format!("{ieee}_{key}_zigbee2mqtt")
}

/// Device class from the property name, as HA does for Zigbee2MQTT.
fn device_class(property: &str, kind: &Kind) -> Option<&'static str> {
    let binary = matches!(kind, Kind::BinarySensor { .. });
    Some(match property {
        "temperature" | "device_temperature" => "temperature",
        "humidity" => "humidity",
        "pressure" => "pressure",
        "power" => "power",
        "energy" => "energy",
        "voltage" => "voltage",
        "current" => "current",
        "illuminance" | "illuminance_lux" => "illuminance",
        "battery" | "battpercentage" => "battery",
        "battery_low" if binary => "battery",
        "contact" if binary => "door",
        "occupancy" | "presence" if binary => "occupancy",
        "tamper" if binary => "tamper",
        _ => return None,
    })
}

/// State string from a payload value. No special case for contact sensors:
/// their expose already says `value_on: false` (open = `contact: false`).
#[must_use]
pub fn state_of(entity: &Z2mEntity, value: &Value) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let as_on_off = |on: &Value, off: &Value| {
        if value == on {
            Some("on".to_owned())
        } else if value == off {
            Some("off".to_owned())
        } else {
            None
        }
    };
    match &entity.kind {
        Kind::Switch { on, off }
        | Kind::Light { on, off }
        | Kind::OptionSwitch { on, off }
        | Kind::BinarySensor { on, off } => as_on_off(on, off),
        Kind::Sensor | Kind::Number { .. } | Kind::Select { .. } => Some(match value {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        }),
    }
}

/// `Température extérieur` → `temperature_exterieur` (HA-style object id).
#[must_use]
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        let c = match c {
            'à' | 'â' | 'ä' | 'á' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            other => other,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn devices() -> Vec<Z2mDevice> {
        serde_json::from_str(include_str!("../tests/fixtures/bridge_devices.json")).unwrap()
    }

    fn device(name: &str) -> Z2mDevice {
        devices()
            .into_iter()
            .find(|d| d.friendly_name.trim() == name)
            .unwrap()
    }

    #[test]
    fn parses_every_real_device() {
        let all = devices();
        assert_eq!(all.len(), 18); // 17 devices + coordinator
        let total: usize = all.iter().map(|d| entities_for(d).len()).sum();
        assert!(total > 100, "{total} entities");
        assert_eq!(
            entities_for(all.iter().find(|d| d.kind == "Coordinator").unwrap()).len(),
            0
        );
    }

    #[test]
    fn plug_matches_home_assistant_discovery() {
        let d = device("Prise lampadaire");
        let ents = entities_for(&d);
        let by_key = |k: &str| ents.iter().find(|e| e.key == k).unwrap();
        // unique_ids observed in the real HA registry
        assert_eq!(
            by_key("switch").unique_id,
            "0x54ef4410003a6d55_switch_zigbee2mqtt"
        );
        assert_eq!(
            by_key("power").unique_id,
            "0x54ef4410003a6d55_power_zigbee2mqtt"
        );
        assert_eq!(
            by_key("switch_auto_off").unique_id,
            "0x54ef4410003a6d55_switch_auto_off_zigbee2mqtt"
        );
        assert_eq!(
            by_key("consumer_connected").unique_id,
            "0x54ef4410003a6d55_consumer_connected_zigbee2mqtt"
        );
        assert_eq!(
            by_key("overload_protection").unique_id,
            "0x54ef4410003a6d55_overload_protection_zigbee2mqtt"
        );
        assert_eq!(
            by_key("switch").default_entity_id,
            "switch.prise_lampadaire"
        );
        assert_eq!(
            by_key("power").default_entity_id,
            "sensor.prise_lampadaire_power"
        );
        assert_eq!(by_key("power").unit.as_deref(), Some("W"));
        assert_eq!(by_key("power").device_class, Some("power"));
        assert!(matches!(
            by_key("overload_protection").kind,
            Kind::Number { .. }
        ));
        assert!(by_key("linkquality").hidden);
    }

    #[test]
    fn states_from_payload() {
        let d = device("Prise lampadaire");
        let ents = entities_for(&d);
        let get = |k: &str| ents.iter().find(|e| e.key == k).unwrap().clone();
        assert_eq!(
            state_of(&get("switch"), &json!("ON")).as_deref(),
            Some("on")
        );
        assert_eq!(
            state_of(&get("switch"), &json!("OFF")).as_deref(),
            Some("off")
        );
        assert_eq!(
            state_of(&get("power"), &json!(812.4)).as_deref(),
            Some("812.4")
        );
        assert_eq!(state_of(&get("power"), &Value::Null), None);
        assert_eq!(
            state_of(&get("switch_power_outage_memory"), &json!(true)).as_deref(),
            Some("on")
        );
    }

    #[test]
    fn contact_open_is_on_like_home_assistant() {
        let d = device("porte_congelateur");
        let contact = entities_for(&d)
            .into_iter()
            .find(|e| e.property == "contact")
            .unwrap();
        assert_eq!(contact.device_class, Some("door"));
        assert_eq!(
            state_of(&contact, &json!(true)).as_deref(),
            Some("off"),
            "closed"
        );
        assert_eq!(
            state_of(&contact, &json!(false)).as_deref(),
            Some("on"),
            "open"
        );
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("Température extérieur"), "temperature_exterieur");
        assert_eq!(slugify("Interrupteur sans fil "), "interrupteur_sans_fil");
        assert_eq!(slugify("Prise meuble livng"), "prise_meuble_livng");
    }
}
