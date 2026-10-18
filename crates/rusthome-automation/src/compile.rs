//! Canonical automation document → typed, executable structure.
//!
//! Compiling is also the validation used by the YAML editor: every error is
//! reported with its path (`actions[2].repeat.while[0]`).

use std::time::Duration;

use rusthome_model::EntityId;
use serde_json::{Map, Value};

use crate::normalize::list;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Single,
    Restart,
    Queued(usize),
    Parallel(usize),
}

/// A literal duration or a template rendered at run time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dur {
    Fixed(Duration),
    Template(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimePart {
    Any,
    Every(u32),
    At(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Threshold {
    Value(f64),
    Entity(EntityId),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerKind {
    State {
        entity_ids: Vec<EntityId>,
        from: Option<Vec<String>>,
        to: Option<Vec<String>>,
        attribute: Option<String>,
        for_: Option<Dur>,
    },
    NumericState {
        entity_ids: Vec<EntityId>,
        attribute: Option<String>,
        above: Option<Threshold>,
        below: Option<Threshold>,
        for_: Option<Dur>,
    },
    Time {
        at: Vec<(u32, u32, u32)>,
    },
    TimePattern {
        hours: TimePart,
        minutes: TimePart,
        seconds: TimePart,
    },
    Event {
        event_type: String,
        event_data: Map<String, Value>,
    },
    Startup,
    Shutdown,
    Tag {
        tag_ids: Vec<String>,
    },
    Mqtt {
        topic: String,
        payload: Option<String>,
    },
    /// `RustHome` extension: press on an `event.*` entity (Hue buttons).
    EventEntity {
        entity_id: EntityId,
        event_types: Vec<String>,
    },
    Sun {
        event: String,
        offset: Option<Dur>,
    },
    Zone {
        entity_id: EntityId,
        zone: EntityId,
        event: String,
    },
    Template {
        value_template: String,
        for_: Option<Dur>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Trigger {
    pub id: Option<String>,
    pub kind: TriggerKind,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    State {
        entity_ids: Vec<EntityId>,
        states: Vec<String>,
        attribute: Option<String>,
        for_: Option<Dur>,
        match_any: bool,
    },
    NumericState {
        entity_ids: Vec<EntityId>,
        attribute: Option<String>,
        above: Option<Threshold>,
        below: Option<Threshold>,
    },
    Template(String),
    Trigger(Vec<String>),
    And(Vec<Condition>),
    Or(Vec<Condition>),
    Not(Vec<Condition>),
    Time {
        after: Option<(u32, u32, u32)>,
        before: Option<(u32, u32, u32)>,
        weekdays: Vec<String>,
    },
    Sun {
        after: Option<String>,
        before: Option<String>,
    },
    Zone {
        entity_id: EntityId,
        zone: EntityId,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub action: Action,
    pub continue_on_error: bool,
    pub enabled: bool,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Repeat {
    While(Vec<Condition>),
    Until(Vec<Condition>),
    Count(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// `domain.service` with templated target and data.
    Service {
        service: String,
        target: Value,
        data: Value,
    },
    Delay(Dur),
    Choose {
        options: Vec<(Vec<Condition>, Vec<Step>)>,
        default: Vec<Step>,
    },
    If {
        conditions: Vec<Condition>,
        then: Vec<Step>,
        otherwise: Vec<Step>,
    },
    Repeat {
        kind: Repeat,
        sequence: Vec<Step>,
    },
    Parallel(Vec<Vec<Step>>),
    Sequence(Vec<Step>),
    Condition(Condition),
    Variables(Map<String, Value>),
    WaitTemplate {
        template: String,
        timeout: Option<Dur>,
        continue_on_timeout: bool,
    },
    Event {
        event_type: String,
        data: Value,
    },
    Stop {
        reason: String,
        error: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Automation {
    pub id: String,
    pub alias: String,
    pub mode: Mode,
    pub variables: Map<String, Value>,
    pub triggers: Vec<Trigger>,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompileError {
    pub path: String,
    pub message: String,
}

type R<T> = Result<T, CompileError>;

fn err<T>(path: &str, message: impl Into<String>) -> R<T> {
    Err(CompileError {
        path: path.to_owned(),
        message: message.into(),
    })
}

/// Compiles a canonical document. All errors are collected.
pub fn compile(doc: &Value) -> Result<Automation, Vec<CompileError>> {
    let mut errors = Vec::new();
    let mut collect = |r: R<()>| {
        if let Err(e) = r {
            errors.push(e);
        }
    };
    let mode = match (
        doc["mode"].as_str().unwrap_or("single"),
        doc["max"].as_u64(),
    ) {
        ("single", _) => Mode::Single,
        ("restart", _) => Mode::Restart,
        #[allow(clippy::cast_possible_truncation)]
        ("queued", m) => Mode::Queued(m.unwrap_or(10) as usize),
        #[allow(clippy::cast_possible_truncation)]
        ("parallel", m) => Mode::Parallel(m.unwrap_or(10) as usize),
        (other, _) => {
            collect(err("mode", format!("unknown mode {other}")));
            Mode::Single
        }
    };
    let mut triggers = Vec::new();
    for (i, t) in list(doc.get("triggers")).iter().enumerate() {
        match trigger(t, &format!("triggers[{i}]")) {
            Ok(x) => triggers.push(x),
            Err(e) => collect(Err(e)),
        }
    }
    let conditions = match conditions(&list(doc.get("conditions")), "conditions") {
        Ok(c) => c,
        Err(e) => {
            collect(Err(e));
            Vec::new()
        }
    };
    let actions = match steps(&list(doc.get("actions")), "actions") {
        Ok(a) => a,
        Err(e) => {
            collect(Err(e));
            Vec::new()
        }
    };
    if triggers.is_empty() && errors.is_empty() {
        errors.push(CompileError {
            path: "triggers".into(),
            message: "at least one trigger is required".into(),
        });
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Automation {
        id: doc["id"]
            .as_str()
            .map_or_else(|| doc["id"].to_string(), str::to_owned),
        alias: doc["alias"].as_str().unwrap_or("Sans nom").to_owned(),
        mode,
        variables: doc["variables"].as_object().cloned().unwrap_or_default(),
        triggers,
        conditions,
        actions,
    })
}

fn entities(v: &Value, path: &str) -> R<Vec<EntityId>> {
    let raw = list(Some(v));
    if raw.is_empty() {
        return err(path, "entity_id is required");
    }
    raw.iter()
        .map(|x| {
            let s = x.as_str().ok_or_else(|| CompileError {
                path: path.into(),
                message: "entity_id must be a string".into(),
            })?;
            EntityId::parse(s).map_err(|e| CompileError {
                path: path.into(),
                message: e.to_string(),
            })
        })
        .collect()
}

fn strings(v: Option<&Value>) -> Option<Vec<String>> {
    let l = list(v);
    if l.is_empty() {
        return None;
    }
    Some(
        l.iter()
            .map(|x| match x {
                Value::String(s) => s.clone(),
                Value::Bool(true) => "on".into(),
                Value::Bool(false) => "off".into(),
                other => other.to_string(),
            })
            .collect(),
    )
}

/// `"00:05:00"`, `"05:00"`, `{hours, minutes, seconds, milliseconds}`, seconds, or a template.
pub fn duration(v: &Value, path: &str) -> R<Dur> {
    match v {
        Value::Number(n) => seconds(n.as_f64().unwrap_or(0.0))
            .map(Dur::Fixed)
            .ok_or_else(|| CompileError {
                path: path.into(),
                message: format!("invalid duration {n}"),
            }),
        Value::String(s) if s.contains("{{") || s.contains("{%") => Ok(Dur::Template(s.clone())),
        Value::String(s) => parse_hms(s).map(Dur::Fixed).ok_or_else(|| CompileError {
            path: path.into(),
            message: format!("invalid duration {s}"),
        }),
        Value::Object(o) => {
            if o.values()
                .any(|x| x.as_str().is_some_and(|s| s.contains("{{")))
            {
                return Ok(Dur::Template(v.to_string()));
            }
            let f = |k: &str| o.get(k).and_then(Value::as_f64).unwrap_or(0.0);
            let secs = f("days") * 86400.0
                + f("hours") * 3600.0
                + f("minutes") * 60.0
                + f("seconds")
                + f("milliseconds") / 1000.0;
            seconds(secs).map(Dur::Fixed).ok_or_else(|| CompileError {
                path: path.into(),
                message: format!("invalid duration ({secs} s)"),
            })
        }
        _ => err(path, "invalid duration"),
    }
}

/// `HH:MM[:SS]` as a duration.
pub fn parse_hms(s: &str) -> Option<Duration> {
    let parts: Vec<f64> = s
        .trim()
        .split(':')
        .map(str::parse::<f64>)
        .collect::<Result<_, _>>()
        .ok()?;
    let secs = match parts.as_slice() {
        [h, m] => h * 3600.0 + m * 60.0,
        [h, m, s] => h * 3600.0 + m * 60.0 + s,
        [s] => *s,
        _ => return None,
    };
    if secs < 0.0 {
        return None;
    }
    seconds(secs)
}

/// Longest accepted duration: 400 days (beyond, a typo or a template gone
/// wrong).
const MAX_DURATION_SECS: f64 = 400.0 * 86_400.0;

/// Seconds as a duration, negative values clamped to 0. `None` for NaN,
/// infinity or more than 400 days: `Duration::from_secs_f64` would panic on
/// a value coming from YAML or a template (code review n° 16).
#[must_use]
pub fn seconds(secs: f64) -> Option<Duration> {
    if !secs.is_finite() || secs > MAX_DURATION_SECS {
        return None;
    }
    Duration::try_from_secs_f64(secs.max(0.0)).ok()
}

fn time_of_day(s: &str) -> Option<(u32, u32, u32)> {
    let p: Vec<u32> = s
        .split(':')
        .map(|x| x.parse().ok())
        .collect::<Option<_>>()?;
    match p.as_slice() {
        [h, m] if *h < 24 && *m < 60 => Some((*h, *m, 0)),
        [h, m, s] if *h < 24 && *m < 60 && *s < 60 => Some((*h, *m, *s)),
        _ => None,
    }
}

fn time_part(v: Option<&Value>, path: &str) -> R<TimePart> {
    match v {
        None | Some(Value::Null) => Ok(TimePart::Any),
        Some(Value::Number(n)) => Ok(TimePart::At(
            n.as_u64().unwrap_or(0).try_into().unwrap_or(0),
        )),
        Some(Value::String(s)) if s == "*" => Ok(TimePart::Any),
        Some(Value::String(s)) => {
            if let Some(n) = s.strip_prefix('/') {
                n.parse().map(TimePart::Every).map_err(|_| CompileError {
                    path: path.into(),
                    message: format!("invalid pattern {s}"),
                })
            } else {
                s.parse().map(TimePart::At).map_err(|_| CompileError {
                    path: path.into(),
                    message: format!("invalid pattern {s}"),
                })
            }
        }
        _ => err(path, "invalid time pattern"),
    }
}

fn threshold(v: Option<&Value>, path: &str) -> R<Option<Threshold>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => Ok(n.as_f64().map(Threshold::Value)),
        Some(Value::String(s)) => match s.parse::<f64>() {
            Ok(x) => Ok(Some(Threshold::Value(x))),
            Err(_) => EntityId::parse(s)
                .map(|e| Some(Threshold::Entity(e)))
                .map_err(|_| CompileError {
                    path: path.into(),
                    message: format!("invalid threshold {s}"),
                }),
        },
        _ => err(path, "invalid threshold"),
    }
}

fn opt_dur(v: Option<&Value>, path: &str) -> R<Option<Dur>> {
    v.filter(|x| !x.is_null())
        .map(|x| duration(x, path))
        .transpose()
}

fn trigger(t: &Value, path: &str) -> R<Trigger> {
    let kind_name = t["trigger"].as_str().unwrap_or_default();
    let kind = match kind_name {
        "state" => TriggerKind::State {
            entity_ids: entities(&t["entity_id"], &format!("{path}.entity_id"))?,
            from: strings(t.get("from")),
            to: strings(t.get("to")),
            attribute: t["attribute"].as_str().map(str::to_owned),
            for_: opt_dur(t.get("for"), &format!("{path}.for"))?,
        },
        "numeric_state" => TriggerKind::NumericState {
            entity_ids: entities(&t["entity_id"], &format!("{path}.entity_id"))?,
            attribute: t["attribute"].as_str().map(str::to_owned),
            above: threshold(t.get("above"), &format!("{path}.above"))?,
            below: threshold(t.get("below"), &format!("{path}.below"))?,
            for_: opt_dur(t.get("for"), &format!("{path}.for"))?,
        },
        "time" => TriggerKind::Time {
            at: list(t.get("at"))
                .iter()
                .map(|a| {
                    a.as_str()
                        .and_then(time_of_day)
                        .ok_or_else(|| CompileError {
                            path: format!("{path}.at"),
                            message: format!("invalid time {a}"),
                        })
                })
                .collect::<R<_>>()?,
        },
        "time_pattern" => TriggerKind::TimePattern {
            hours: time_part(t.get("hours"), &format!("{path}.hours"))?,
            minutes: time_part(t.get("minutes"), &format!("{path}.minutes"))?,
            seconds: time_part(t.get("seconds"), &format!("{path}.seconds"))?,
        },
        "event" => TriggerKind::Event {
            event_type: t["event_type"].as_str().unwrap_or_default().to_owned(),
            event_data: t["event_data"].as_object().cloned().unwrap_or_default(),
        },
        "homeassistant" => match t["event"].as_str() {
            Some("start") => TriggerKind::Startup,
            Some("shutdown") => TriggerKind::Shutdown,
            other => return err(path, format!("unknown homeassistant event {other:?}")),
        },
        "tag" => TriggerKind::Tag {
            tag_ids: strings(t.get("tag_id")).unwrap_or_default(),
        },
        "mqtt" => TriggerKind::Mqtt {
            topic: t["topic"].as_str().unwrap_or_default().to_owned(),
            payload: t["payload"].as_str().map(str::to_owned),
        },
        "event_entity" => TriggerKind::EventEntity {
            entity_id: entities(&t["entity_id"], &format!("{path}.entity_id"))?.remove(0),
            event_types: strings(t.get("event_type")).unwrap_or_default(),
        },
        "sun" => TriggerKind::Sun {
            event: t["event"].as_str().unwrap_or("sunset").to_owned(),
            offset: opt_dur(t.get("offset"), &format!("{path}.offset"))?,
        },
        "zone" => TriggerKind::Zone {
            entity_id: entities(&t["entity_id"], &format!("{path}.entity_id"))?.remove(0),
            zone: entities(&t["zone"], &format!("{path}.zone"))?.remove(0),
            event: t["event"].as_str().unwrap_or("enter").to_owned(),
        },
        // Shorthand `trigger: zone.left` (newer HA syntax for zone triggers).
        "zone.left" | "zone.entered" => TriggerKind::Zone {
            entity_id: entities(
                &t["target"]["entity_id"],
                &format!("{path}.target.entity_id"),
            )?
            .remove(0),
            zone: entities(&t["options"]["zone"], &format!("{path}.options.zone"))?.remove(0),
            event: if kind_name == "zone.left" {
                "leave".into()
            } else {
                "enter".into()
            },
        },
        "template" => TriggerKind::Template {
            value_template: t["value_template"].as_str().unwrap_or_default().to_owned(),
            for_: opt_dur(t.get("for"), &format!("{path}.for"))?,
        },
        "device" => return err(path, "device trigger left untranslated (see import issues)"),
        other => return err(path, format!("unsupported trigger {other:?}")),
    };
    Ok(Trigger {
        id: t["id"].as_str().map(str::to_owned),
        kind,
        enabled: t["enabled"] != false,
    })
}

fn conditions(items: &[Value], path: &str) -> R<Vec<Condition>> {
    items
        .iter()
        .enumerate()
        .map(|(i, c)| condition(c, &format!("{path}[{i}]")))
        .collect()
}

fn condition(c: &Value, path: &str) -> R<Condition> {
    if let Some(s) = c.as_str() {
        return Ok(Condition::Template(s.to_owned()));
    }
    Ok(match c["condition"].as_str().unwrap_or_default() {
        "state" => Condition::State {
            entity_ids: entities(&c["entity_id"], &format!("{path}.entity_id"))?,
            states: strings(c.get("state")).ok_or_else(|| CompileError {
                path: format!("{path}.state"),
                message: "state is required".into(),
            })?,
            attribute: c["attribute"].as_str().map(str::to_owned),
            for_: opt_dur(c.get("for"), &format!("{path}.for"))?,
            match_any: c["match"] == "any",
        },
        "numeric_state" => Condition::NumericState {
            entity_ids: entities(&c["entity_id"], &format!("{path}.entity_id"))?,
            attribute: c["attribute"].as_str().map(str::to_owned),
            above: threshold(c.get("above"), &format!("{path}.above"))?,
            below: threshold(c.get("below"), &format!("{path}.below"))?,
        },
        "template" => {
            Condition::Template(c["value_template"].as_str().unwrap_or_default().to_owned())
        }
        "trigger" => Condition::Trigger(strings(c.get("id")).unwrap_or_default()),
        "and" => Condition::And(conditions(
            &list(c.get("conditions")),
            &format!("{path}.conditions"),
        )?),
        "or" => Condition::Or(conditions(
            &list(c.get("conditions")),
            &format!("{path}.conditions"),
        )?),
        "not" => Condition::Not(conditions(
            &list(c.get("conditions")),
            &format!("{path}.conditions"),
        )?),
        "time" => Condition::Time {
            after: c["after"].as_str().and_then(time_of_day),
            before: c["before"].as_str().and_then(time_of_day),
            weekdays: strings(c.get("weekday")).unwrap_or_default(),
        },
        "sun" => Condition::Sun {
            after: c["after"].as_str().map(str::to_owned),
            before: c["before"].as_str().map(str::to_owned),
        },
        "zone" => Condition::Zone {
            entity_id: entities(&c["entity_id"], &format!("{path}.entity_id"))?.remove(0),
            zone: entities(&c["zone"], &format!("{path}.zone"))?.remove(0),
        },
        "device" => {
            return err(
                path,
                "device condition left untranslated (see import issues)",
            );
        }
        other => return err(path, format!("unsupported condition {other:?}")),
    })
}

fn steps(items: &[Value], path: &str) -> R<Vec<Step>> {
    items
        .iter()
        .enumerate()
        .map(|(i, a)| step(a, &format!("{path}[{i}]")))
        .collect()
}

fn step(a: &Value, path: &str) -> R<Step> {
    let action = if let Some(service) = a["action"].as_str() {
        if !service.contains('.') {
            return err(path, format!("invalid service {service}"));
        }
        Action::Service {
            service: service.to_owned(),
            target: a.get("target").cloned().unwrap_or(Value::Null),
            data: a.get("data").cloned().unwrap_or(Value::Null),
        }
    } else if let Some(d) = a.get("delay") {
        Action::Delay(duration(d, &format!("{path}.delay"))?)
    } else if let Some(opts) = a.get("choose") {
        let mut options = Vec::new();
        for (i, o) in list(Some(opts)).iter().enumerate() {
            let p = format!("{path}.choose[{i}]");
            options.push((
                conditions(&list(o.get("conditions")), &format!("{p}.conditions"))?,
                steps(&list(o.get("sequence")), &format!("{p}.sequence"))?,
            ));
        }
        Action::Choose {
            options,
            default: steps(&list(a.get("default")), &format!("{path}.default"))?,
        }
    } else if let Some(cs) = a.get("if") {
        Action::If {
            conditions: conditions(&list(Some(cs)), &format!("{path}.if"))?,
            then: steps(&list(a.get("then")), &format!("{path}.then"))?,
            otherwise: steps(&list(a.get("else")), &format!("{path}.else"))?,
        }
    } else if let Some(r) = a.get("repeat") {
        let kind = if let Some(w) = r.get("while") {
            Repeat::While(conditions(&list(Some(w)), &format!("{path}.repeat.while"))?)
        } else if let Some(u) = r.get("until") {
            Repeat::Until(conditions(&list(Some(u)), &format!("{path}.repeat.until"))?)
        } else if let Some(c) = r.get("count") {
            Repeat::Count(c.as_str().map_or_else(|| c.to_string(), str::to_owned))
        } else {
            return err(
                &format!("{path}.repeat"),
                "repeat needs while, until or count",
            );
        };
        Action::Repeat {
            kind,
            sequence: steps(&list(r.get("sequence")), &format!("{path}.repeat.sequence"))?,
        }
    } else if let Some(branches) = a.get("parallel") {
        let mut out = Vec::new();
        for (i, b) in list(Some(branches)).iter().enumerate() {
            let p = format!("{path}.parallel[{i}]");
            out.push(match b.get("sequence") {
                Some(seq) => steps(&list(Some(seq)), &format!("{p}.sequence"))?,
                None => vec![step(b, &p)?],
            });
        }
        Action::Parallel(out)
    } else if let Some(seq) = a.get("sequence") {
        Action::Sequence(steps(&list(Some(seq)), &format!("{path}.sequence"))?)
    } else if a.get("condition").is_some() {
        Action::Condition(condition(a, path)?)
    } else if let Some(v) = a.get("variables") {
        Action::Variables(v.as_object().cloned().unwrap_or_default())
    } else if let Some(t) = a.get("wait_template") {
        Action::WaitTemplate {
            template: t.as_str().unwrap_or_default().to_owned(),
            timeout: opt_dur(a.get("timeout"), &format!("{path}.timeout"))?,
            continue_on_timeout: a["continue_on_timeout"] != false,
        }
    } else if let Some(e) = a.get("event") {
        Action::Event {
            event_type: e.as_str().unwrap_or_default().to_owned(),
            data: a.get("event_data").cloned().unwrap_or(Value::Null),
        }
    } else if let Some(s) = a.get("stop") {
        Action::Stop {
            reason: s.as_str().unwrap_or_default().to_owned(),
            error: a["error"] == true,
        }
    } else if a.get("device_id").is_some() {
        return err(path, "device action left untranslated (see import issues)");
    } else {
        return err(path, format!("unsupported action {a}"));
    };
    Ok(Step {
        action,
        continue_on_error: a["continue_on_error"] == true,
        enabled: a["enabled"] != false,
        alias: a["alias"].as_str().map(str::to_owned),
    })
}

#[cfg(test)]
mod duration_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn absurd_durations_are_errors_not_panics() {
        assert!(duration(&json!(1e20), "d").is_err());
        assert!(duration(&json!({"days": 1e15}), "d").is_err());
        assert_eq!(parse_hms("1e300"), None);
        assert_eq!(seconds(f64::INFINITY), None);
        assert_eq!(seconds(f64::NAN), None);
        assert_eq!(seconds(-5.0), Some(Duration::ZERO));
        assert!(matches!(
            duration(&json!({"hours": 1}), "d"),
            Ok(Dur::Fixed(d)) if d == Duration::from_secs(3600)
        ));
    }
}
