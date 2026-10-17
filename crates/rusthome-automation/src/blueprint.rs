//! Blueprint expansion: an automation using `use_blueprint` becomes a plain
//! automation at import (spec §6.2: one usage, no blueprint engine needed).

use serde_json::{Map, Value};
use serde_yaml::Value as Yaml;

/// Expands `cfg.use_blueprint` with the blueprint YAML source.
/// `!input name` takes the automation input, else the blueprint default.
pub fn expand(cfg: &Value, blueprint_yaml: &str) -> Result<Value, String> {
    let bp: Yaml =
        serde_yaml::from_str(blueprint_yaml).map_err(|e| format!("invalid blueprint: {e}"))?;
    let inputs = cfg["use_blueprint"]["input"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let defaults: Map<String, Value> = match bp.get("blueprint").and_then(|b| b.get("input")) {
        Some(Yaml::Mapping(m)) => m
            .iter()
            .filter_map(|(k, v)| {
                Some((
                    k.as_str()?.to_owned(),
                    to_json(v.get("default")?, &Map::new(), &Map::new()).ok()?,
                ))
            })
            .collect(),
        _ => Map::new(),
    };
    let Yaml::Mapping(root) = bp else {
        return Err("blueprint is not a mapping".into());
    };
    let mut out = Map::new();
    for (k, v) in &root {
        let Some(key) = k.as_str() else { continue };
        if key != "blueprint" {
            out.insert(key.to_owned(), to_json(v, &inputs, &defaults)?);
        }
    }
    // The automation's own identity wins over the blueprint's.
    for key in ["id", "alias", "description", "category"] {
        if let Some(v) = cfg.get(key) {
            out.insert(key.to_owned(), v.clone());
        }
    }
    Ok(Value::Object(out))
}

fn to_json(
    v: &Yaml,
    inputs: &Map<String, Value>,
    defaults: &Map<String, Value>,
) -> Result<Value, String> {
    Ok(match v {
        Yaml::Tagged(t) if t.tag == "!input" => {
            let name = t.value.as_str().ok_or("!input needs a name")?;
            inputs
                .get(name)
                .or_else(|| defaults.get(name))
                .cloned()
                .ok_or_else(|| format!("missing blueprint input {name}"))?
        }
        Yaml::Tagged(t) => to_json(&t.value, inputs, defaults)?,
        Yaml::Mapping(m) => {
            let mut o = Map::new();
            for (k, v) in m {
                let key = match k {
                    Yaml::String(s) => s.clone(),
                    other => serde_yaml::to_string(other)
                        .map_err(|e| e.to_string())?
                        .trim()
                        .to_owned(),
                };
                o.insert(key, to_json(v, inputs, defaults)?);
            }
            Value::Object(o)
        }
        Yaml::Sequence(s) => Value::from(
            s.iter()
                .map(|x| to_json(x, inputs, defaults))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        other => serde_json::to_value(other).map_err(|e| e.to_string())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const AWTRIX: &str = include_str!("../tests/fixtures/blueprints/awtrix_random_effect.yaml");

    #[test]
    fn expands_the_real_awtrix_blueprint() {
        let cfg = json!({
            "id": "1700731472350", "alias": "Awtrix effect",
            "use_blueprint": {"path": "awtrix_random_effect.yaml", "input": {
                "effect_list": ["Fireworks", "Radar"], "change_interval": "/1", "awtrix_light_mqtt": "awtrix_ulanzi"}}
        });
        let out = expand(&cfg, AWTRIX).unwrap();
        assert_eq!(out["alias"], "Awtrix effect");
        assert_eq!(out["mode"], "queued");
        assert_eq!(out["trigger"][0]["minutes"], "/1");
        assert_eq!(out["variables"]["awtrix_light_mqtt"], "awtrix_ulanzi");
        assert_eq!(
            out["variables"]["effect_list"],
            json!(["Fireworks", "Radar"])
        );
        assert_eq!(out["variables"]["duration"], "10", "blueprint default used");
        assert_eq!(out["action"][0]["service"], "mqtt.publish");
    }

    #[test]
    fn missing_input_without_default_is_an_error() {
        let cfg = json!({"use_blueprint": {"input": {}}});
        assert!(
            expand(&cfg, AWTRIX)
                .unwrap_err()
                .contains("awtrix_light_mqtt")
        );
    }
}
