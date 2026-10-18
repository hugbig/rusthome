//! Lovelace (HA) → RustHome dashboard document.
//!
//! The document keeps the Lovelace shape (views, sections, cards, `tap_action`)
//! but only uses native card types ([`NATIVE_CARDS`]). Mushroom, Bubble,
//! tempometer, mini-graph… are mapped to their native equivalent; anything
//! else becomes an `unsupported` card that keeps the original config, so an
//! import never loses data. Native cards go through unchanged, which makes
//! the normalizer idempotent: it also cleans YAML typed by the user.

use serde_json::{Map, Value, json};

use crate::Issue;

/// Card types the frontend knows how to render.
pub const NATIVE_CARDS: [&str; 22] = [
    "grid",
    "vertical-stack",
    "horizontal-stack",
    "popup",
    "heading",
    "tile",
    "button",
    "chips",
    "entities",
    "alarm-panel",
    "gauge",
    "camera",
    "logbook",
    "thermostat",
    "media-control",
    "graph",
    "weather",
    "media-list",
    "calendar",
    "todo-list",
    "map",
    "unsupported",
];

const VIEW_TYPES: [&str; 3] = ["sections", "masonry", "panel"];

struct Ctx {
    issues: Vec<Issue>,
}

impl Ctx {
    fn issue(&mut self, path: &str, message: impl Into<String>) {
        self.issues.push(Issue {
            path: path.to_owned(),
            message: message.into(),
        });
    }
}

/// Copies the listed keys of `src` that are set (non-null).
fn pick(src: &Value, keys: &[&str]) -> Map<String, Value> {
    let mut out = Map::new();
    for k in keys {
        if let Some(v) = src.get(*k).filter(|v| !v.is_null()) {
            out.insert((*k).to_owned(), v.clone());
        }
    }
    out
}

fn with_type(t: &str, mut m: Map<String, Value>) -> Value {
    let mut out = Map::new();
    out.insert("type".into(), t.into());
    out.append(&mut m);
    Value::Object(out)
}

fn str_of<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// `call-service` (old) → `perform-action`; other actions kept as is.
fn action(a: &Value) -> Value {
    let Some(kind) = str_of(a, "action") else {
        return json!({ "action": "more-info" });
    };
    match kind {
        "call-service" | "perform-action" => {
            let mut m = Map::new();
            m.insert("action".into(), "perform-action".into());
            if let Some(s) = str_of(a, "perform_action").or_else(|| str_of(a, "service")) {
                m.insert("perform_action".into(), s.into());
            }
            for (from, to) in [
                ("target", "target"),
                ("data", "data"),
                ("service_data", "data"),
            ] {
                if let Some(v) = a.get(from).filter(|v| !v.is_null()) {
                    m.insert(to.into(), v.clone());
                }
            }
            Value::Object(m)
        }
        _ => Value::Object(pick(
            a,
            &["action", "navigation_path", "url_path", "entity"],
        )),
    }
}

fn copy_action(src: &Value, out: &mut Map<String, Value>) {
    for k in ["tap_action", "hold_action"] {
        if let Some(a) = src.get(k).filter(|a| a.is_object()) {
            out.insert(k.into(), action(a));
        }
    }
}

/// `["a.b", {entity: "c.d"}]` → `["a.b", "c.d"]`.
fn entity_list(v: Option<&Value>) -> Vec<Value> {
    v.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|e| {
            e.as_str()
                .or_else(|| str_of(e, "entity"))
                .map(|s| Value::from(s.to_owned()))
        })
        .collect()
}

/// Graph cards come from several sources: fixed key order keeps the
/// normalizer idempotent.
fn graph(m: &Map<String, Value>) -> Value {
    with_type(
        "graph",
        pick(
            &Value::Object(m.clone()),
            &[
                "name",
                "icon",
                "entities",
                "hours_to_show",
                "chart",
                "aggregate",
                "period",
                "statistics",
            ],
        ),
    )
}

fn cards(list: Option<&Value>, path: &str, cx: &mut Ctx) -> Vec<Value> {
    let mut out = Vec::new();
    for (i, c) in list
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        out.push(card(c, &format!("{path}.cards[{i}]"), cx));
    }
    out
}

fn unsupported(c: &Value, t: &str, path: &str, cx: &mut Ctx) -> Value {
    cx.issue(path, format!("carte non supportée : {t}"));
    json!({ "type": "unsupported", "original": c })
}

/// Bubble pop-up: a vertical-stack whose first card is the pop-up header.
fn bubble_popup(c: &Value) -> Option<&Value> {
    if c["type"] != "vertical-stack" {
        return None;
    }
    let first = c["cards"].get(0)?;
    (first["type"] == "custom:bubble-card" && first["card_type"] == "pop-up").then_some(first)
}

/// Layout keys every card may carry: size in a section grid (`grid_options`,
/// 12 columns) and display conditions (`visibility`).
const LAYOUT_KEYS: [&str; 2] = ["grid_options", "visibility"];

fn card(c: &Value, path: &str, cx: &mut Ctx) -> Value {
    let mut out = card_body(c, path, cx);
    if let Value::Object(m) = &mut out
        && m.get("type").and_then(Value::as_str) != Some("unsupported")
    {
        for k in LAYOUT_KEYS {
            if let Some(v) = c.get(k).filter(|v| !v.is_null()) {
                m.insert(k.to_owned(), v.clone());
            }
        }
    }
    out
}

#[allow(clippy::too_many_lines)] // one arm per card type reads best flat
fn card_body(c: &Value, path: &str, cx: &mut Ctx) -> Value {
    let Some(t) = str_of(c, "type") else {
        cx.issue(path, "carte sans type");
        return json!({ "type": "unsupported", "original": c });
    };
    if let Some(head) = bubble_popup(c) {
        let mut m = Map::new();
        m.insert(
            "hash".into(),
            str_of(head, "hash").unwrap_or("#popup").into(),
        );
        if let Some(n) = str_of(head, "name") {
            m.insert("title".into(), n.into());
        }
        let rest: Vec<Value> = c["cards"]
            .as_array()
            .map(|a| a[1..].to_vec())
            .unwrap_or_default();
        m.insert(
            "cards".into(),
            cards(Some(&Value::Array(rest)), path, cx).into(),
        );
        return with_type("popup", m);
    }
    match t {
        "grid" => {
            let mut m = pick(c, &["title", "columns", "square"]);
            m.insert("cards".into(), cards(c.get("cards"), path, cx).into());
            with_type("grid", m)
        }
        "vertical-stack" | "horizontal-stack" => {
            let mut m = pick(c, &["title"]);
            m.insert("cards".into(), cards(c.get("cards"), path, cx).into());
            with_type(t, m)
        }
        "popup" => {
            let mut m = pick(c, &["hash", "title"]);
            m.insert("cards".into(), cards(c.get("cards"), path, cx).into());
            with_type("popup", m)
        }
        "heading" => with_type("heading", pick(c, &["heading", "icon", "heading_style"])),
        "custom:mushroom-title-card" => {
            let mut m = Map::new();
            if let Some(h) = str_of(c, "title") {
                m.insert("heading".into(), h.into());
            }
            with_type("heading", m)
        }
        "custom:bubble-card" => match str_of(c, "card_type") {
            Some("separator") => {
                let mut m = pick(c, &["icon"]);
                m.insert("heading".into(), str_of(c, "name").unwrap_or("").into());
                with_type("heading", m)
            }
            Some("pop-up") => {
                let mut m = pick(c, &["hash"]);
                m.insert("cards".into(), json!([]));
                with_type("popup", m)
            }
            Some("media-player") if str_of(c, "entity").is_some() => {
                with_type("media-control", pick(c, &["entity", "name"]))
            }
            Some(_) if str_of(c, "entity").is_some() => {
                let mut m = pick(c, &["entity", "name", "icon"]);
                copy_action(c, &mut m);
                with_type("tile", m)
            }
            // a Bubble button without entity is a plain (navigation) button
            Some("button") => {
                let mut m = pick(c, &["name", "icon"]);
                copy_action(c, &mut m);
                with_type("button", m)
            }
            other => unsupported(
                c,
                &format!("bubble-card {}", other.unwrap_or("?")),
                path,
                cx,
            ),
        },
        "tile" => {
            let mut m = pick(
                c,
                &[
                    "entity",
                    "name",
                    "icon",
                    "color",
                    "vertical",
                    "features",
                    "state_content",
                ],
            );
            copy_action(c, &mut m);
            with_type("tile", m)
        }
        "entity" => with_type("tile", pick(c, &["entity", "name", "icon"])),
        "custom:mushroom-entity-card"
        | "custom:mushroom-light-card"
        | "custom:mushroom-fan-card" => {
            let mut m = pick(c, &["entity", "name", "icon"]);
            if c["layout"] == "vertical" {
                m.insert("vertical".into(), true.into());
            }
            copy_action(c, &mut m);
            with_type("tile", m)
        }
        "button" => {
            let mut m = pick(c, &["entity", "name", "icon"]);
            copy_action(c, &mut m);
            with_type("button", m)
        }
        "custom:mushroom-template-card" => {
            let mut m = pick(c, &["icon", "entity"]);
            for k in ["primary", "secondary"] {
                if let Some(s) = str_of(c, k) {
                    if s.contains("{{") || s.contains("{%") {
                        cx.issue(path, format!("modèle Jinja ignoré dans « {k} »"));
                    } else if k == "primary" {
                        m.insert("name".into(), s.into());
                    }
                }
            }
            copy_action(c, &mut m);
            with_type("button", m)
        }
        "chips" => with_type("chips", pick(c, &["chips"])),
        "custom:mushroom-chips-card" => {
            let mut chips = Vec::new();
            for (i, chip) in c["chips"].as_array().into_iter().flatten().enumerate() {
                if let Some(e) = str_of(chip, "entity") {
                    let mut m = Map::new();
                    m.insert("entity".into(), e.into());
                    copy_action(chip, &mut m);
                    chips.push(Value::Object(m));
                } else {
                    cx.issue(
                        &format!("{path}.chips[{i}]"),
                        format!(
                            "puce non supportée : {}",
                            str_of(chip, "type").unwrap_or("?")
                        ),
                    );
                }
            }
            with_type(
                "chips",
                Map::from_iter([("chips".to_owned(), chips.into())]),
            )
        }
        "entities" => {
            let mut m = pick(c, &["title", "icon"]);
            let rows: Vec<Value> = c["entities"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
                .map(|(i, r)| row(r, &format!("{path}.entities[{i}]"), cx))
                .collect();
            m.insert("entities".into(), rows.into());
            with_type("entities", m)
        }
        "alarm-panel" => with_type("alarm-panel", pick(c, &["entity", "name", "states"])),
        "gauge" => with_type(
            "gauge",
            pick(
                c,
                &[
                    "entity", "name", "unit", "min", "max", "severity", "needle", "decimals",
                ],
            ),
        ),
        "custom:tempometer-gauge-card" => {
            let mut m = pick(c, &["entity", "min", "max", "severity", "decimals"]);
            if let Some(n) = str_of(c, "title").or_else(|| str_of(c, "name")) {
                m.insert("name".into(), n.into());
            }
            with_type("gauge", m)
        }
        "camera" => with_type("camera", pick(c, &["entity", "name"])),
        "picture-entity" | "picture-glance" => {
            let Some(e) = str_of(c, "camera_image").or_else(|| str_of(c, "entity")) else {
                return unsupported(c, t, path, cx);
            };
            let mut m = Map::new();
            m.insert("entity".into(), e.into());
            if let Some(n) = str_of(c, "name").or_else(|| str_of(c, "title")) {
                m.insert("name".into(), n.into());
            }
            with_type("camera", m)
        }
        "logbook" => {
            let mut m = pick(c, &["title", "hours_to_show"]);
            let mut ents = entity_list(c.get("entities"));
            match &c["target"]["entity_id"] {
                Value::String(s) => ents.push(s.clone().into()),
                Value::Array(a) => ents.extend(a.iter().cloned()),
                _ => {}
            }
            m.insert("entities".into(), ents.into());
            with_type("logbook", m)
        }
        "thermostat" => with_type("thermostat", pick(c, &["entity", "name"])),
        "media-control" => with_type(
            "media-control",
            pick(c, &["entity", "name", "volume", "source"]),
        ),
        "custom:mushroom-media-player-card" => {
            let mut m = pick(c, &["entity", "name"]);
            if c["show_volume_level"] == true {
                m.insert("volume".into(), true.into());
            }
            with_type("media-control", m)
        }
        "graph" => {
            let mut m = pick(
                c,
                &[
                    "name",
                    "icon",
                    "hours_to_show",
                    "chart",
                    "aggregate",
                    "statistics",
                    "period",
                ],
            );
            m.insert("entities".into(), entity_list(c.get("entities")).into());
            graph(&m)
        }
        "custom:mini-graph-card" => {
            let mut m = pick(c, &["name", "icon", "hours_to_show"]);
            m.insert("entities".into(), entity_list(c.get("entities")).into());
            if c["show"]["graph"] == "bar" {
                m.insert("chart".into(), "bar".into());
            }
            if let Some(f) = str_of(c, "aggregate_func") {
                m.insert("aggregate".into(), f.into());
            }
            if let Some(g) = str_of(c, "group_by") {
                m.insert("period".into(), if g == "date" { "day" } else { g }.into());
            }
            graph(&m)
        }
        "history-graph" | "sensor" => {
            let mut m = pick(c, &["name", "icon", "hours_to_show"]);
            let ents = if t == "sensor" {
                str_of(c, "entity")
                    .map(|e| vec![Value::from(e)])
                    .unwrap_or_default()
            } else {
                entity_list(c.get("entities"))
            };
            m.insert("entities".into(), ents.into());
            graph(&m)
        }
        "statistics-graph" => {
            let mut m = pick(c, &["title", "period"]);
            if let Some(title) = m.remove("title") {
                m.insert("name".into(), title);
            }
            m.insert("entities".into(), entity_list(c.get("entities")).into());
            if let Some(d) = c["days_to_show"].as_u64() {
                m.insert("hours_to_show".into(), (d * 24).into());
            }
            if c["chart_type"] == "bar" {
                m.insert("chart".into(), "bar".into());
            }
            m.insert("statistics".into(), true.into());
            graph(&m)
        }
        "weather" => with_type(
            "weather",
            pick(c, &["entity", "name", "show_forecast", "extra"]),
        ),
        "weather-forecast" => with_type("weather", pick(c, &["entity", "name", "show_forecast"])),
        "custom:meteofrance-weather-card" => {
            let mut m = pick(c, &["entity", "name"]);
            let mut extra = Map::new();
            for (from, to) in [
                ("rainChanceEntity", "rain_chance"),
                ("rainForecastEntity", "next_rain"),
                ("uvEntity", "uv"),
                ("alertEntity", "alert"),
                ("cloudCoverEntity", "cloud_cover"),
                ("freezeChanceEntity", "freeze_chance"),
                ("snowChanceEntity", "snow_chance"),
            ] {
                if let Some(e) = str_of(c, from) {
                    extra.insert(to.into(), e.into());
                }
            }
            if !extra.is_empty() {
                m.insert("extra".into(), extra.into());
            }
            m.insert("show_forecast".into(), true.into());
            with_type("weather", m)
        }
        "media-list" | "custom:upcoming-media-card" => {
            with_type("media-list", pick(c, &["entity", "title", "max"]))
        }
        "calendar" => {
            let mut m = pick(c, &["title", "days"]);
            if !m.contains_key("days") {
                m.insert("days".into(), 7.into());
            }
            m.insert("entities".into(), entity_list(c.get("entities")).into());
            with_type("calendar", m)
        }
        "custom:atomic-calendar-revive" => {
            let mut m = Map::new();
            if let Some(n) = str_of(c, "name") {
                m.insert("title".into(), n.into());
            }
            m.insert(
                "days".into(),
                c["maxDaysToShow"].as_u64().unwrap_or(7).into(),
            );
            m.insert("entities".into(), entity_list(c.get("entities")).into());
            with_type("calendar", m)
        }
        "todo-list" => with_type("todo-list", pick(c, &["entity", "title"])),
        "map" => {
            let mut m = pick(c, &["title", "hours_to_show"]);
            m.insert("entities".into(), entity_list(c.get("entities")).into());
            with_type("map", m)
        }
        // several cameras: one camera card each
        "custom:advanced-camera-card" | "custom:frigate-card" => {
            let cams: Vec<Value> = c["cameras"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|cam| {
                    let e = str_of(cam, "camera_entity")?;
                    let mut m = Map::new();
                    m.insert("entity".into(), e.into());
                    if let Some(t) = str_of(cam, "title") {
                        m.insert("name".into(), t.into());
                    }
                    Some(with_type("camera", m))
                })
                .collect();
            if cams.is_empty() {
                return unsupported(c, t, path, cx);
            }
            let cols = cams.len().min(2);
            with_type(
                "grid",
                Map::from_iter([
                    ("columns".to_owned(), cols.into()),
                    ("cards".to_owned(), cams.into()),
                ]),
            )
        }
        // the Spotify player of the account, found at display time
        "custom:spotify-card" => json!({ "type": "media-control", "source": "spotify" }),
        "unsupported" => with_type("unsupported", pick(c, &["original"])),
        other => unsupported(c, other, path, cx),
    }
}

/// One row of an `entities` card.
fn row(r: &Value, path: &str, cx: &mut Ctx) -> Value {
    if let Some(e) = r.as_str() {
        return json!({ "entity": e });
    }
    match str_of(r, "type") {
        None => {
            let mut m = pick(r, &["entity", "name", "icon", "slider", "toggle"]);
            copy_action(r, &mut m);
            Value::Object(m)
        }
        Some("custom:slider-entity-row") => {
            let mut m = pick(r, &["entity", "name", "icon", "toggle"]);
            m.insert("slider".into(), true.into());
            Value::Object(m)
        }
        Some("divider") => json!({ "type": "divider" }),
        // already normalized: kept as is (idempotence, code review n° 27)
        Some("unsupported") => with_type("unsupported", pick(r, &["original"])),
        Some("attribute") => with_type(
            "attribute",
            pick(r, &["entity", "attribute", "name", "icon", "suffix"]),
        ),
        Some(other) => {
            cx.issue(path, format!("ligne non supportée : {other}"));
            json!({ "type": "unsupported", "original": r })
        }
    }
}

fn view(v: &Value, i: usize, cx: &mut Ctx) -> Value {
    let path = format!("views[{i}]");
    let mut m = pick(v, &["title", "path", "icon", "max_columns", "subview"]);
    let raw_type = str_of(v, "type").unwrap_or("masonry");
    let kind = match raw_type {
        t if VIEW_TYPES.contains(&t) => t,
        "custom:grid-layout" | "custom:vertical-layout" => "masonry",
        other => {
            cx.issue(
                &path,
                format!("type de vue inconnu « {other} », affichée en masonry"),
            );
            "masonry"
        }
    };
    if raw_type == "custom:grid-layout" {
        m.insert("max_columns".into(), 1.into());
    }
    let mut out = Map::new();
    out.insert("type".into(), kind.into());
    out.append(&mut m);
    if kind == "sections" {
        let sections: Vec<Value> = v["sections"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(j, s)| {
                let sp = format!("{path}.sections[{j}]");
                let mut sm = pick(s, &["title", "column_span"]);
                sm.insert("cards".into(), cards(s.get("cards"), &sp, cx).into());
                Value::Object(sm)
            })
            .collect();
        out.insert("sections".into(), sections.into());
    } else {
        out.insert("cards".into(), cards(v.get("cards"), &path, cx).into());
    }
    if let Some(b) = v["badges"].as_array().filter(|b| !b.is_empty()) {
        let badges: Vec<Value> = b
            .iter()
            .filter_map(|x| {
                x.as_str()
                    .map(|e| json!({ "entity": e }))
                    .or_else(|| str_of(x, "entity").map(|e| json!({ "entity": e })))
            })
            .collect();
        out.insert("badges".into(), badges.into());
    }
    Value::Object(out)
}

/// Normalizes a whole dashboard. `meta` holds the HA dashboard metadata
/// (`url_path`, `title`, `icon`, `show_in_sidebar`, `require_admin`) and
/// `config` the Lovelace config; a RustHome document can be passed as both.
#[must_use]
pub fn dashboard(meta: &Value, config: &Value) -> (Value, Vec<Issue>) {
    let mut cx = Ctx { issues: Vec::new() };
    let mut out = Map::new();
    let pref = |k: &str| {
        config
            .get(k)
            .filter(|v| !v.is_null())
            .or_else(|| meta.get(k).filter(|v| !v.is_null()))
            .cloned()
    };
    for k in [
        "url_path",
        "title",
        "icon",
        "show_in_sidebar",
        "require_admin",
        // RustHome: tabs show their icon only (the title stays as tooltip)
        "tabs_icon_only",
    ] {
        if let Some(v) = pref(k) {
            out.insert(k.into(), v);
        }
    }
    let kiosk = config["kiosk"] == true || config.get("kiosk_mode").is_some();
    if kiosk {
        out.insert("kiosk".into(), true.into());
    }
    let views: Vec<Value> = config["views"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, v)| view(v, i, &mut cx))
        .collect();
    out.insert("views".into(), views.into());
    (Value::Object(out), cx.issues)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(c: &Value) -> (Value, Vec<Issue>) {
        let (d, issues) = dashboard(&json!({}), &json!({ "views": [{ "cards": [c] }] }));
        (d["views"][0]["cards"][0].clone(), issues)
    }

    #[test]
    fn unsupported_rows_are_not_wrapped_twice() {
        let raw =
            json!({ "type": "entities", "entities": [{ "type": "custom:inconnue", "x": 1 }] });
        let (once, _) = one(&raw);
        let (twice, issues) = one(&once);
        assert_eq!(once, twice);
        assert_eq!(issues.len(), 0, "no spurious issue on the second pass");
    }

    #[test]
    fn mushroom_cards_become_tiles() {
        let (c, issues) = one(&json!({
            "type": "custom:mushroom-light-card", "entity": "light.a",
            "layout": "vertical", "icon_type": "entity-picture"
        }));
        assert_eq!(
            c,
            json!({ "type": "tile", "entity": "light.a", "vertical": true })
        );
        assert_eq!(issues, [] as [Issue; 0]);
    }

    #[test]
    fn tempometer_becomes_a_gauge() {
        let (c, _) = one(&json!({
            "type": "custom:tempometer-gauge-card", "entity": "sensor.p",
            "min": 950, "max": 1050, "title": "Baromètre", "decimals": 0,
            "severity": { "green": 1015, "yellow": 1000, "red": 950 }
        }));
        assert_eq!(c["type"], "gauge");
        assert_eq!(c["name"], "Baromètre");
        assert_eq!(c["severity"]["green"], 1015);
    }

    #[test]
    fn bubble_popup_stack_becomes_a_popup() {
        let (c, issues) = one(&json!({
            "type": "vertical-stack",
            "cards": [
                { "type": "custom:bubble-card", "card_type": "pop-up", "hash": "#media", "bg_color": "#000" },
                { "type": "media-control", "entity": "media_player.tele" }
            ]
        }));
        assert_eq!(
            c,
            json!({ "type": "popup", "hash": "#media",
                    "cards": [{ "type": "media-control", "entity": "media_player.tele" }] })
        );
        assert_eq!(issues, [] as [Issue; 0]);
    }

    #[test]
    fn unknown_cards_are_kept_as_unsupported() {
        let raw = json!({ "type": "custom:inconnue", "limit": 10 });
        let (c, issues) = one(&raw);
        assert_eq!(c, json!({ "type": "unsupported", "original": raw }));
        assert_eq!(issues[0].path, "views[0].cards[0]");
        assert!(issues[0].message.contains("custom:inconnue"));
    }

    #[test]
    fn legacy_call_service_action_is_rewritten() {
        let (c, _) = one(&json!({
            "type": "tile", "entity": "light.a",
            "tap_action": { "action": "call-service", "service": "light.toggle", "service_data": { "x": 1 } }
        }));
        assert_eq!(
            c["tap_action"],
            json!({ "action": "perform-action", "perform_action": "light.toggle", "data": { "x": 1 } })
        );
    }

    #[test]
    fn entities_rows_and_logbook_target() {
        let (c, _) = one(&json!({
            "type": "entities",
            "entities": ["light.a", { "type": "custom:slider-entity-row", "entity": "light.b", "toggle": true }, { "type": "divider" }]
        }));
        assert_eq!(
            c["entities"],
            json!([{ "entity": "light.a" }, { "entity": "light.b", "toggle": true, "slider": true }, { "type": "divider" }])
        );
        let (l, _) = one(
            &json!({ "type": "logbook", "hours_to_show": 24, "target": { "entity_id": ["a.b", "c.d"] } }),
        );
        assert_eq!(
            l,
            json!({ "type": "logbook", "hours_to_show": 24, "entities": ["a.b", "c.d"] })
        );
    }

    #[test]
    fn p2_cards_are_mapped() {
        let (c, issues) = one(&json!({
            "type": "custom:upcoming-media-card", "entity": "sensor.emby_latest_films",
            "title": "Films", "max": 3, "image_style": "fanart"
        }));
        assert_eq!(
            c,
            json!({ "type": "media-list", "entity": "sensor.emby_latest_films", "title": "Films", "max": 3 })
        );
        assert_eq!(issues, [] as [Issue; 0]);
        let (cal, _) = one(&json!({
            "type": "custom:atomic-calendar-revive", "name": "Agenda", "maxDaysToShow": 3,
            "entities": [{ "entity": "calendar.perso" }]
        }));
        assert_eq!(
            cal,
            json!({ "type": "calendar", "title": "Agenda", "days": 3, "entities": ["calendar.perso"] })
        );
        let (cams, _) = one(&json!({
            "type": "custom:advanced-camera-card",
            "cameras": [{ "camera_entity": "camera.a", "title": "A" }, { "camera_entity": "camera.b" }]
        }));
        assert_eq!(cams["type"], "grid");
        assert_eq!(
            cams["cards"][0],
            json!({ "type": "camera", "entity": "camera.a", "name": "A" })
        );
        let (sp, _) = one(&json!({ "type": "custom:spotify-card", "limit": 10 }));
        assert_eq!(sp, json!({ "type": "media-control", "source": "spotify" }));
        let (todo, _) = one(&json!({
            "type": "todo-list", "entity": "todo.courses", "grid_options": { "columns": "full" }
        }));
        assert_eq!(
            todo,
            json!({ "type": "todo-list", "entity": "todo.courses", "grid_options": { "columns": "full" } }),
            "the size in the section grid is kept"
        );
        let (map, _) = one(
            &json!({ "type": "map", "entities": [{ "entity": "device_tracker.p" }], "theme_mode": "auto" }),
        );
        assert_eq!(
            map,
            json!({ "type": "map", "entities": ["device_tracker.p"] })
        );
        let (bubble, _) = one(&json!({ "type": "custom:bubble-card", "card_type": "button" }));
        assert_eq!(bubble, json!({ "type": "button" }));
    }

    #[test]
    fn kiosk_and_grid_layout_view() {
        let (d, _) = dashboard(
            &json!({ "url_path": "dashboard-nspanel", "title": "NsPanel" }),
            &json!({ "kiosk_mode": { "mobile_settings": { "hide_header": true } },
                     "views": [{ "title": "Accueil", "type": "custom:grid-layout", "cards": [] }] }),
        );
        assert_eq!(d["kiosk"], true);
        assert_eq!(d["url_path"], "dashboard-nspanel");
        assert_eq!(d["views"][0]["type"], "masonry");
        assert_eq!(d["views"][0]["max_columns"], 1);
    }

    #[test]
    fn tabs_icon_only_is_kept() {
        let (d, _) = dashboard(
            &json!({ "url_path": "apercu-3" }),
            &json!({ "tabs_icon_only": true, "views": [] }),
        );
        assert_eq!(d["tabs_icon_only"], true);
    }
}
