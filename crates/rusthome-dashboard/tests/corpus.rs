//! Import of the live dashboards of the real installation (redacted copy).

#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;

use rusthome_dashboard::lovelace::{NATIVE_CARDS, dashboard};
use rusthome_dashboard::refs::entity_refs;
use serde_json::Value;

fn corpus() -> Vec<Value> {
    serde_json::from_str(include_str!("fixtures/lovelace.json")).unwrap()
}

fn card_types(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(m) => {
            if let Some(t) = m.get("type").and_then(Value::as_str) {
                out.insert(t.to_owned());
            }
            for (k, c) in m {
                // original configs kept by unsupported cards are opaque
                if k != "original" {
                    card_types(c, out);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| card_types(x, out)),
        _ => {}
    }
}

#[test]
fn every_dashboard_imports_with_native_cards_only() {
    let allowed: BTreeSet<&str> = NATIVE_CARDS
        .iter()
        .copied()
        .chain([
            "sections",
            "masonry",
            "panel",
            "divider",
            "attribute",
            "toggle",
            "select-options",
        ])
        .collect();
    for d in corpus() {
        let (doc, _) = dashboard(&d["meta"], &d["config"]);
        let mut types = BTreeSet::new();
        card_types(&doc, &mut types);
        for t in &types {
            assert!(
                allowed.contains(t.as_str()),
                "{}: {t}",
                d["meta"]["url_path"]
            );
        }
    }
}

#[test]
fn normalizing_twice_changes_nothing() {
    for d in corpus() {
        let (doc, _) = dashboard(&d["meta"], &d["config"]);
        let (again, issues) = dashboard(&Value::Null, &doc);
        assert_eq!(again, doc, "{}", d["meta"]["url_path"]);
        // unsupported cards pass through silently: nothing new to report
        assert!(issues.is_empty(), "{issues:?}");
    }
}

#[test]
fn live_dashboards_have_no_unsupported_card() {
    for d in corpus() {
        let (doc, _) = dashboard(&d["meta"], &d["config"]);
        let left = rusthome_dashboard::refs::unsupported_cards(&doc);
        assert!(left.is_empty(), "{}: {left:?}", d["meta"]["url_path"]);
    }
}

#[test]
fn security_dashboard_is_fully_native() {
    let d = corpus()
        .into_iter()
        .find(|d| d["meta"]["url_path"] == "securite-alarme")
        .unwrap();
    let (doc, issues) = dashboard(&d["meta"], &d["config"]);
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(doc["title"], "Sécurité");
    assert_eq!(doc["views"][0]["type"], "sections");
    assert_eq!(doc["views"][0]["max_columns"], 3);
    let refs: Vec<String> = entity_refs(&doc).into_iter().map(|(_, e)| e).collect();
    assert!(refs.contains(&"alarm_control_panel.alarmo".to_owned()));
}

#[test]
fn nspanel_keeps_its_popups_and_kiosk_mode() {
    let d = corpus()
        .into_iter()
        .find(|d| d["meta"]["url_path"] == "dashboard-nspanel")
        .unwrap();
    let (doc, _) = dashboard(&d["meta"], &d["config"]);
    assert_eq!(doc["kiosk"], true);
    let text = doc.to_string();
    for hash in ["#media", "#meteo", "#spotify", "#nas"] {
        assert!(
            text.contains(&format!(r#""type":"popup","hash":"{hash}""#)),
            "{hash}"
        );
    }
}
