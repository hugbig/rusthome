//! Reference validator: every entity a dashboard points to.

use serde_json::Value;

use crate::Issue;

/// `(path, entity_id)` for every entity referenced by the document. The
/// original config kept in `unsupported` cards is ignored (not rendered).
#[must_use]
pub fn entity_refs(doc: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    walk(doc, "", &mut out);
    out
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

fn walk(node: &Value, path: &str, out: &mut Vec<(String, String)>) {
    match node {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("unsupported") {
                return;
            }
            for (k, child) in map {
                let p = join(path, k);
                match (k.as_str(), child) {
                    ("entity", Value::String(e)) => out.push((path.to_owned(), e.clone())),
                    ("entities", Value::Array(a)) => {
                        for (i, x) in a.iter().enumerate() {
                            match x {
                                Value::String(e) => out.push((format!("{p}[{i}]"), e.clone())),
                                other => walk(other, &format!("{p}[{i}]"), out),
                            }
                        }
                    }
                    ("extra", Value::Object(x)) => {
                        for (name, e) in x {
                            if let Value::String(e) = e {
                                out.push((join(&p, name), e.clone()));
                            }
                        }
                    }
                    _ => walk(child, &p, out),
                }
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                walk(x, &format!("{path}[{i}]"), out);
            }
        }
        _ => {}
    }
}

/// One issue per reference to an entity that `exists` does not know.
pub fn missing_entities(doc: &Value, exists: impl Fn(&str) -> bool) -> Vec<Issue> {
    entity_refs(doc)
        .into_iter()
        .filter(|(_, e)| !exists(e))
        .map(|(path, e)| Issue {
            path,
            message: format!("entité absente : {e}"),
        })
        .collect()
}

/// One issue per `unsupported` card, with the original card type.
#[must_use]
pub fn unsupported_cards(doc: &Value) -> Vec<Issue> {
    let mut out = Vec::new();
    find_unsupported(doc, "", &mut out);
    out
}

fn find_unsupported(node: &Value, path: &str, out: &mut Vec<Issue>) {
    match node {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("unsupported") {
                let t = map
                    .get("original")
                    .and_then(|o| o.get("type"))
                    .and_then(Value::as_str)
                    .unwrap_or("?");
                out.push(Issue {
                    path: path.to_owned(),
                    message: format!("carte non supportée : {t}"),
                });
                return;
            }
            for (k, child) in map {
                find_unsupported(child, &join(path, k), out);
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                find_unsupported(x, &format!("{path}[{i}]"), out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn finds_every_reference_but_unsupported_ones() {
        let doc = json!({ "views": [{ "cards": [
            { "type": "tile", "entity": "light.a" },
            { "type": "logbook", "entities": ["b.b"] },
            { "type": "entities", "entities": [{ "entity": "c.c" }, { "type": "divider" }] },
            { "type": "weather", "entity": "weather.x", "extra": { "uv": "sensor.uv" } },
            { "type": "unsupported", "original": { "entity": "z.z" } }
        ] }] });
        let refs: Vec<String> = entity_refs(&doc).into_iter().map(|(_, e)| e).collect();
        assert_eq!(refs, ["light.a", "b.b", "c.c", "weather.x", "sensor.uv"]);
        let unsupported = unsupported_cards(&doc);
        assert_eq!(unsupported.len(), 1);
        assert_eq!(unsupported[0].path, "views[0].cards[4]");
        assert_eq!(unsupported[0].message, "carte non supportée : ?");
        let missing = missing_entities(&doc, |e| e != "c.c");
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].path, "views[0].cards[2].entities[0]");
    }
}
