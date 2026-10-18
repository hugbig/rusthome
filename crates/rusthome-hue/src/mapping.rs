//! Hue CLIP v2 resources → `RustHome` entities. Pure and unit-tested.
//!
//! `unique_id` = Hue resource id, as in HA's Hue v2 integration (observed in
//! the real registry), so entities keep their HA `entity_id` on takeover.

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use crate::color;

/// Resources indexed by id, kept up to date by the event stream.
#[derive(Debug, Default, Clone)]
pub struct Resources {
    pub by_id: HashMap<String, Value>,
}

impl Resources {
    /// From the `GET /clip/v2/resource/<type>` lists.
    #[must_use]
    pub fn from_lists(lists: &HashMap<String, Vec<Value>>) -> Self {
        let mut by_id = HashMap::new();
        for items in lists.values() {
            for item in items {
                if let Some(id) = item["id"].as_str() {
                    by_id.insert(id.to_owned(), item.clone());
                }
            }
        }
        Self { by_id }
    }

    fn name_of(&self, rid: &str) -> Option<&str> {
        self.by_id.get(rid)?["metadata"]["name"].as_str()
    }

    /// Applies a partial update from the event stream. Returns the resource id
    /// when it is known (unknown resources need a full resync).
    pub fn merge(&mut self, update: &Value) -> Option<String> {
        let id = update["id"].as_str()?;
        let target = self.by_id.get_mut(id)?;
        deep_merge(target, update);
        Some(id.to_owned())
    }
}

fn deep_merge(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(t), Value::Object(p)) => {
            for (k, v) in p {
                deep_merge(t.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (t, p) => *t = p.clone(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HueEntity {
    /// Hue resource id = `unique_id`.
    pub resource_id: String,
    /// `light`, `grouped_light`, `scene`, `button`…
    pub rtype: String,
    pub domain: &'static str,
    pub name: String,
    /// Owning Hue device (or room/zone for groups and scenes).
    pub owner: Option<String>,
    pub state: String,
    pub attributes: Map<String, Value>,
}

/// All entities derived from the bridge resources.
#[must_use]
pub fn entities(res: &Resources) -> Vec<HueEntity> {
    let mut out: Vec<HueEntity> = res.by_id.values().filter_map(|r| entity(res, r)).collect();
    out.sort_by(|a, b| a.resource_id.cmp(&b.resource_id));
    out
}

/// Entity of one resource, if it maps to one.
#[must_use]
pub fn entity(res: &Resources, r: &Value) -> Option<HueEntity> {
    let id = r["id"].as_str()?.to_owned();
    let rtype = r["type"].as_str()?.to_owned();
    let owner = r["owner"]["rid"].as_str().map(str::to_owned);
    let owner_name = owner
        .as_deref()
        .and_then(|o| res.name_of(o))
        .unwrap_or("Hue");
    let mut attrs = Map::new();
    let (domain, name, state) = match rtype.as_str() {
        "light" => {
            light_attributes(r, &mut attrs);
            (
                "light",
                r["metadata"]["name"]
                    .as_str()
                    .unwrap_or(owner_name)
                    .to_owned(),
                on_off(r),
            )
        }
        "grouped_light" => {
            // The bridge-wide group ("all lights") has no room/zone owner.
            if r["owner"]["rtype"] != "room" && r["owner"]["rtype"] != "zone" {
                return None;
            }
            light_attributes(r, &mut attrs);
            ("light", owner_name.to_owned(), on_off(r))
        }
        "scene" => {
            let group = r["group"]["rid"].as_str();
            let group_name = group.and_then(|g| res.name_of(g)).unwrap_or("Hue");
            attrs.insert("group_name".into(), group_name.into());
            attrs.insert("status".into(), r["status"]["active"].clone());
            let state = r["status"]["last_recall"]
                .as_str()
                .unwrap_or("unknown")
                .to_owned();
            let name = format!(
                "{group_name} {}",
                r["metadata"]["name"].as_str().unwrap_or("scène")
            );
            attrs.insert("friendly_name".into(), name.clone().into());
            return Some(HueEntity {
                resource_id: id,
                rtype,
                domain: "scene",
                name,
                owner: group.map(str::to_owned),
                state,
                attributes: attrs,
            });
        }
        "button" => {
            let report = &r["button"]["button_report"];
            attrs.insert("event_type".into(), report["event"].clone());
            attrs.insert("event_types".into(), r["button"]["event_values"].clone());
            let control = r["metadata"]["control_id"].as_u64().unwrap_or(1);
            let state = report["updated"].as_str().unwrap_or("unknown").to_owned();
            ("event", format!("{owner_name} Bouton {control}"), state)
        }
        "entertainment_configuration" => {
            let name = r["metadata"]["name"]
                .as_str()
                .unwrap_or("Entertainment")
                .to_owned();
            let on = if r["status"] == "active" { "on" } else { "off" };
            attrs.insert("device_class".into(), "running".into());
            ("binary_sensor", format!("Hue Bridge {name}"), on.to_owned())
        }
        "device_power" => {
            attrs.insert("unit_of_measurement".into(), "%".into());
            attrs.insert("device_class".into(), "battery".into());
            let level = r["power_state"]["battery_level"].as_u64()?;
            (
                "sensor",
                format!("{owner_name} Batterie"),
                level.to_string(),
            )
        }
        _ => return None,
    };
    attrs.insert("friendly_name".into(), name.clone().into());
    Some(HueEntity {
        resource_id: id,
        rtype,
        domain,
        name,
        owner,
        state,
        attributes: attrs,
    })
}

fn on_off(r: &Value) -> String {
    match r["on"]["on"].as_bool() {
        Some(true) => "on",
        Some(false) => "off",
        None => "unknown",
    }
    .to_owned()
}

/// HA-compatible light attributes: brightness 0-255, color modes, color
/// (xy, hs, rgb) and color temperature (mireds and kelvin) with its range.
fn light_attributes(r: &Value, attrs: &mut Map<String, Value>) {
    let on = r["on"]["on"].as_bool() == Some(true);
    let has_color = r["color"].is_object();
    let has_ct = r["color_temperature"].is_object();
    let has_dim = r["dimming"].is_object();

    let mut modes = Vec::new();
    if has_ct {
        modes.push("color_temp");
    }
    if has_color {
        modes.push("xy");
    }
    if modes.is_empty() {
        modes.push(if has_dim { "brightness" } else { "onoff" });
    }
    attrs.insert("supported_color_modes".into(), json!(modes));

    if let Some(pct) = r["dimming"]["brightness"].as_f64() {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // 0..=255
        let b = on.then(|| (pct.clamp(0.0, 100.0) / 100.0 * 255.0).round() as u64);
        attrs.insert("brightness".into(), json!(b));
    }
    if has_ct {
        let schema = &r["color_temperature"]["mirek_schema"];
        let (min_m, max_m) = (
            schema["mirek_minimum"].as_u64().unwrap_or(153),
            schema["mirek_maximum"].as_u64().unwrap_or(500),
        );
        attrs.insert("min_mireds".into(), json!(min_m));
        attrs.insert("max_mireds".into(), json!(max_m));
        attrs.insert(
            "min_color_temp_kelvin".into(),
            json!(1_000_000 / max_m.max(1)),
        );
        attrs.insert(
            "max_color_temp_kelvin".into(),
            json!(1_000_000 / min_m.max(1)),
        );
    }
    if !on {
        attrs.insert("color_mode".into(), Value::Null);
        return;
    }
    // Hue reports a valid mirek only while the light is in white mode.
    let ct_mode = has_ct
        && r["color_temperature"]["mirek_valid"] == true
        && r["color_temperature"]["mirek"].is_u64();
    let rgb = if ct_mode {
        let m = r["color_temperature"]["mirek"]
            .as_u64()
            .unwrap_or(370)
            .max(1);
        attrs.insert("color_temp".into(), json!(m));
        attrs.insert("color_temp_kelvin".into(), json!(1_000_000 / m));
        #[allow(clippy::cast_precision_loss)] // mireds are small integers
        let k = 1_000_000.0 / m as f64;
        attrs.insert("color_mode".into(), json!("color_temp"));
        Some(color::kelvin_to_rgb(k))
    } else if let (Some(x), Some(y)) = (
        r["color"]["xy"]["x"].as_f64(),
        r["color"]["xy"]["y"].as_f64(),
    ) {
        attrs.insert("xy_color".into(), json!([x, y]));
        attrs.insert("color_mode".into(), json!("xy"));
        Some(color::xy_to_rgb(x, y))
    } else {
        attrs.insert(
            "color_mode".into(),
            json!(if has_dim { "brightness" } else { "onoff" }),
        );
        None
    };
    if let Some(rgb) = rgb {
        let (h, s) = color::rgb_to_hs(rgb);
        attrs.insert("rgb_color".into(), json!(rgb));
        attrs.insert("hs_color".into(), json!([h, s]));
    }
}

/// Body of `PUT /clip/v2/resource/<light|grouped_light>/<id>` for an
/// HA-style light service call.
pub fn light_command(service: &str, data: &Value, currently_on: bool) -> Result<Value, String> {
    let turn_on = match service {
        "turn_on" => true,
        "turn_off" => false,
        "toggle" => !currently_on,
        other => return Err(format!("unsupported service light.{other}")),
    };
    let mut body = json!({ "on": { "on": turn_on } });
    if let Some(t) = data["transition"].as_f64() {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let ms = (t.max(0.0) * 1000.0) as u64;
        body["dynamics"] = json!({ "duration": ms });
    }
    if !turn_on {
        return Ok(body);
    }
    let pct = data["brightness_pct"]
        .as_f64()
        .or_else(|| data["brightness"].as_f64().map(|b| b / 255.0 * 100.0));
    if let Some(p) = pct {
        body["dimming"] = json!({ "brightness": p.clamp(0.0, 100.0) });
    }
    let mirek = data["color_temp"].as_f64().or_else(|| {
        data["color_temp_kelvin"]
            .as_f64()
            .map(|k| 1_000_000.0 / k.max(1.0))
    });
    if let Some(m) = mirek {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let m = m.round().clamp(153.0, 500.0) as u64;
        body["color_temperature"] = json!({ "mirek": m });
    }
    if let Some(xy) = data["xy_color"].as_array().filter(|a| a.len() == 2) {
        body["color"] = json!({ "xy": { "x": xy[0], "y": xy[1] } });
    } else if let Some(rgb) = rgb_of(data) {
        let (x, y) = color::rgb_to_xy(rgb);
        body["color"] = json!({ "xy": { "x": x, "y": y } });
    }
    Ok(body)
}

/// `rgb_color: [r, g, b]` or `hs_color: [h, s]` of a service call.
fn rgb_of(data: &Value) -> Option<[u8; 3]> {
    if let Some(a) = data["rgb_color"].as_array().filter(|a| a.len() == 3) {
        let c = |i: usize| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped
            let v = a[i].as_f64().unwrap_or(0.0).clamp(0.0, 255.0) as u8;
            v
        };
        return Some([c(0), c(1), c(2)]);
    }
    let hs = data["hs_color"].as_array().filter(|a| a.len() == 2)?;
    Some(color::hs_to_rgb(hs[0].as_f64()?, hs[1].as_f64()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn real() -> Resources {
        let lists: HashMap<String, Vec<Value>> =
            serde_json::from_str(include_str!("../tests/fixtures/resources.json")).unwrap();
        Resources::from_lists(&lists)
    }

    fn by_id<'a>(es: &'a [HueEntity], id: &str) -> &'a HueEntity {
        es.iter().find(|e| e.resource_id == id).unwrap()
    }

    #[test]
    fn maps_the_real_bridge() {
        let es = entities(&real());
        let count = |d: &str| es.iter().filter(|e| e.domain == d).count();
        assert_eq!(count("scene"), 52);
        assert_eq!(count("light"), 9 + 6); // lights + room/zone groups (bridge-wide group skipped)
        assert_eq!(count("event"), 4);
        assert_eq!(count("binary_sensor"), 3);
        assert_eq!(count("sensor"), 1);

        // unique_ids observed in HA's registry
        let go = by_id(&es, "376d4fa0-4d3f-4cd0-8b5b-ab25fdc443bc");
        assert_eq!((go.domain, go.name.as_str()), ("light", "Hue go Bureau"));
        assert_eq!(go.state, "on");
        assert_eq!(go.attributes["brightness"], 204); // 79.84 % of 255, an integer as in HA
        assert_eq!(go.attributes["color_temp"], 284);
        assert_eq!(go.attributes["color_mode"], "color_temp");
        assert_eq!(
            go.attributes["supported_color_modes"],
            json!(["color_temp", "xy"])
        );
        assert_eq!(go.attributes["color_temp_kelvin"], 3521);
        assert_eq!(go.attributes["min_color_temp_kelvin"], 2000);
        assert!(
            go.attributes["rgb_color"].is_array(),
            "white shown as its color"
        );
        let scene = by_id(&es, "07cd7625-0bb7-4ed0-8df0-198027734719");
        assert_eq!(scene.name, "salon Printemps Floral");
        assert_eq!(scene.attributes["friendly_name"], "salon Printemps Floral");
    }

    #[test]
    fn color_mode_follows_the_bridge() {
        let mut res = real();
        let id = "376d4fa0-4d3f-4cd0-8b5b-ab25fdc443bc";
        res.merge(
            &json!({"id": id, "color": {"xy": {"x": 0.6915, "y": 0.3083}},
                          "color_temperature": {"mirek": null, "mirek_valid": false}}),
        );
        let e = entity(&res, &res.by_id[id].clone()).unwrap();
        assert_eq!(e.attributes["color_mode"], "xy");
        let rgb = e.attributes["rgb_color"].as_array().unwrap();
        assert_eq!(rgb[0], 255, "red light shows red");
        assert!(!e.attributes.contains_key("color_temp_kelvin"));
    }

    #[test]
    fn stream_updates_are_merged() {
        let mut res = real();
        let id = res.merge(&json!({"id": "376d4fa0-4d3f-4cd0-8b5b-ab25fdc443bc", "type": "light", "on": {"on": false}}));
        let r = res.by_id[&id.unwrap()].clone();
        let e = entity(&res, &r).unwrap();
        assert_eq!(e.state, "off");
        assert!(
            e.attributes["brightness"].is_null(),
            "no brightness when off, like HA"
        );
        assert_eq!(e.name, "Hue go Bureau", "untouched fields kept");
        assert!(
            res.merge(&json!({"id": "unknown", "on": {"on": true}}))
                .is_none()
        );
    }

    #[test]
    fn light_commands() {
        assert_eq!(
            light_command("turn_off", &json!({}), true).unwrap(),
            json!({"on": {"on": false}})
        );
        assert_eq!(
            light_command("toggle", &json!({}), true).unwrap(),
            json!({"on": {"on": false}})
        );
        let on = light_command(
            "turn_on",
            &json!({"brightness": 255, "color_temp_kelvin": 2700, "transition": 1.5}),
            false,
        )
        .unwrap();
        assert_eq!(on["dimming"]["brightness"], 100.0);
        assert_eq!(on["color_temperature"]["mirek"], 370);
        assert_eq!(on["dynamics"]["duration"], 1500);
        let red = light_command("turn_on", &json!({"hs_color": [0, 100]}), false).unwrap();
        assert!(red["color"]["xy"]["x"].as_f64().unwrap() > 0.6);
        let blue = light_command("turn_on", &json!({"rgb_color": [0, 0, 255]}), false).unwrap();
        assert!(blue["color"]["xy"]["y"].as_f64().unwrap() < 0.1);
        let pct = light_command("turn_on", &json!({"brightness_pct": 150}), false).unwrap();
        assert_eq!(pct["dimming"]["brightness"], 100.0, "clamped");
        assert!(light_command("blink", &json!({}), false).is_err());
    }
}
