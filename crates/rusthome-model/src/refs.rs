//! Rewriting entity references in documents (automations, dashboards) when
//! entities are renamed: exact values, lists, and ids inside templates
//! (`states('light.x')`), with word boundaries so `light.x` never touches
//! `light.x_2`.

use serde_json::Value;

/// Characters that may continue a word of an entity id.
fn word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Replaces every whole-word occurrence of `from` by `to` in `s`.
#[must_use]
pub fn replace_id(s: &str, from: &str, to: &str) -> (String, usize) {
    let mut out = String::with_capacity(s.len());
    let mut n = 0;
    let mut rest = s;
    while let Some(pos) = rest.find(from) {
        let head = &rest[..pos];
        let before = head.chars().next_back();
        // an id has a single dot: a dot before is only legit in `states.<id>`,
        // a dot after is attribute access (`states.light.x.state`)
        let before_ok = match before {
            None => true,
            Some('.') => head.ends_with("states."),
            Some(c) => !word_char(c),
        };
        let after_ok = rest[pos + from.len()..]
            .chars()
            .next()
            .is_none_or(|c| !word_char(c));
        out.push_str(&rest[..pos]);
        if before_ok && after_ok {
            out.push_str(to);
            n += 1;
        } else {
            out.push_str(from);
        }
        rest = &rest[pos + from.len()..];
    }
    out.push_str(rest);
    (out, n)
}

/// Applies the `(from, to)` renames to every string of `v` (keys untouched).
/// Returns the number of replacements.
pub fn rewrite_entity_ids(v: &mut Value, renames: &[(String, String)]) -> usize {
    match v {
        Value::String(s) => {
            let mut total = 0;
            for (from, to) in renames {
                if s.contains(from.as_str()) {
                    let (new, n) = replace_id(s, from, to);
                    if n > 0 {
                        *s = new;
                        total += n;
                    }
                }
            }
            total
        }
        Value::Array(a) => a.iter_mut().map(|x| rewrite_entity_ids(x, renames)).sum(),
        Value::Object(m) => m.values_mut().map(|x| rewrite_entity_ids(x, renames)).sum(),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn whole_ids_only() {
        let r = |s: &str| replace_id(s, "light.salon", "light.sejour");
        assert_eq!(r("light.salon"), ("light.sejour".into(), 1));
        assert_eq!(r("light.salon_2").1, 0);
        assert_eq!(r("xlight.salon").1, 0);
        assert_eq!(r("switch.light.salon").1, 0);
        assert_eq!(
            r("{{ states('light.salon') }} et {{ state_attr('light.salon', 'brightness') }}").0,
            "{{ states('light.sejour') }} et {{ state_attr('light.sejour', 'brightness') }}"
        );
        assert_eq!(r("{{ states.light.salon.state }}").1, 1, "attribute access");
    }

    #[test]
    fn rewrites_documents() {
        let mut doc = json!({
            "triggers": [{ "trigger": "state", "entity_id": ["light.salon", "light.salon_2"] }],
            "actions": [{ "action": "light.turn_on", "target": { "entity_id": "light.salon" } }],
            "light.salon": "keys are not rewritten"
        });
        let n = rewrite_entity_ids(&mut doc, &[("light.salon".into(), "light.sejour".into())]);
        assert_eq!(n, 2);
        assert_eq!(
            doc["triggers"][0]["entity_id"],
            json!(["light.sejour", "light.salon_2"])
        );
        assert_eq!(doc["actions"][0]["target"]["entity_id"], "light.sejour");
        assert!(doc.get("light.salon").is_some());
    }
}
