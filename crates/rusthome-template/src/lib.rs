//! Home Assistant flavoured Jinja on top of minijinja (cf. spec §6.4).
//!
//! Covers every template of the real corpus (cf. inventaire/02 §4):
//! `states` (object and function), `is_state`, `state_attr`, `now()` (with
//! `strftime`/`isoformat`/`hour`/`minute`), `as_timestamp`, `iif`, `random`.
//! Date arithmetic `(now() - x).total_seconds()` is rewritten before
//! rendering into `seconds_since(x)`: minijinja cannot subtract objects, and
//! the user's YAML stays untouched.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use minijinja::value::{Enumerator, Object, ObjectRepr, Serde, Value};
use minijinja::{Environment, Error, ErrorKind, State as JState};
use rusthome_core::{Clock, StateMachine};
use rusthome_model::EntityId;

#[derive(Debug, thiserror::Error)]
#[error("template error: {0}")]
pub struct TemplateError(#[from] Error);

pub struct TemplateEngine {
    env: Environment<'static>,
}

impl TemplateEngine {
    #[must_use]
    pub fn new(states: Arc<StateMachine>, clock: Arc<dyn Clock>, tz: Tz) -> Self {
        let mut env = Environment::new();
        // Python methods used by HA templates: `dict.get`, `str.upper`, `items()`…
        env.set_unknown_method_callback(minijinja_contrib::pycompat::unknown_method_callback);
        let clock_for_since = Arc::clone(&clock);

        env.add_global(
            "states",
            Value::from_object(StatesObject { sm: states.clone() }),
        );

        let sm = states.clone();
        env.add_function("is_state", move |entity_id: String, value: Value| -> bool {
            let current = lookup(&sm, &entity_id).map(|s| s.state.clone());
            match (current, value.as_str()) {
                (Some(c), Some(v)) => c == v,
                // HA accepts a list of states
                (Some(c), None) => value
                    .try_iter()
                    .is_ok_and(|mut it| it.any(|v| v.as_str() == Some(c.as_str()))),
                (None, _) => false,
            }
        });

        let sm = states;
        env.add_function(
            "state_attr",
            move |entity_id: String, attr: String| -> Value {
                lookup(&sm, &entity_id)
                    .and_then(|s| s.attributes.get(&attr).map(|v| Value::from(Serde(v))))
                    .unwrap_or(Value::from(()))
            },
        );

        env.add_function("now", move || {
            Value::from_object(DateTimeObject(clock.now().with_timezone(&tz)))
        });

        env.add_function("as_timestamp", |v: Value| -> Result<f64, Error> {
            if let Some(dt) = v.downcast_object_ref::<DateTimeObject>() {
                return Ok(timestamp(&dt.0.with_timezone(&Utc)));
            }
            // `as_timestamp(x | default(0))` passes a number through.
            if let Some(n) = v.as_i64() {
                #[allow(clippy::cast_precision_loss)]
                return Ok(n as f64);
            }
            let raw = v.as_str().ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidOperation,
                    "as_timestamp expects a datetime",
                )
            })?;
            DateTime::parse_from_rfc3339(raw)
                .map(|d| timestamp(&d.with_timezone(&Utc)))
                .map_err(|e| {
                    Error::new(
                        ErrorKind::InvalidOperation,
                        format!("invalid datetime: {e}"),
                    )
                })
        });

        let clock2 = Arc::clone(&clock_for_since);
        env.add_function("seconds_since", move |v: Value| -> Result<f64, Error> {
            let then = if let Some(dt) = v.downcast_object_ref::<DateTimeObject>() {
                dt.0.with_timezone(&Utc)
            } else {
                let raw = v.as_str().ok_or_else(|| {
                    Error::new(ErrorKind::InvalidOperation, "expected a datetime")
                })?;
                DateTime::parse_from_rfc3339(raw)
                    .map_err(|e| {
                        Error::new(
                            ErrorKind::InvalidOperation,
                            format!("invalid datetime: {e}"),
                        )
                    })?
                    .with_timezone(&Utc)
            };
            #[allow(clippy::cast_precision_loss)]
            Ok((clock2.now() - then).num_milliseconds() as f64 / 1000.0)
        });

        env.add_filter("random", |v: Value| -> Result<Value, Error> {
            let items: Vec<Value> = v.try_iter()?.collect();
            if items.is_empty() {
                return Ok(Value::from(()));
            }
            Ok(items[fastrand::usize(..items.len())].clone())
        });

        env.add_function(
            "iif",
            |cond: Value, if_true: Option<Value>, if_false: Option<Value>| -> Value {
                if cond.is_true() {
                    if_true.unwrap_or(Value::from(true))
                } else {
                    if_false.unwrap_or(Value::from(false))
                }
            },
        );

        Self { env }
    }

    pub fn render(&self, source: &str, ctx: &serde_json::Value) -> Result<String, TemplateError> {
        Ok(self
            .env
            .render_str(&preprocess(source), Value::from(Serde(ctx)))?)
    }
}

/// `(now() - EXPR).total_seconds()` → `seconds_since(EXPR)`.
#[must_use]
pub fn preprocess(source: &str) -> std::borrow::Cow<'_, str> {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"\(\s*now\(\)\s*-\s*([A-Za-z_][\w.]*)\s*\)\s*\.total_seconds\(\)")
            .unwrap_or_else(|_| unreachable!())
    });
    RE.replace_all(source, "seconds_since($1)")
}

#[allow(clippy::cast_precision_loss)] // sub-second precision of a timestamp is irrelevant here
fn timestamp(dt: &DateTime<Utc>) -> f64 {
    dt.timestamp_millis() as f64 / 1000.0
}

fn lookup(sm: &StateMachine, entity_id: &str) -> Option<Arc<rusthome_model::State>> {
    EntityId::parse(entity_id).ok().and_then(|id| sm.get(&id))
}

/// `states` global: `states('light.x')` and `states.light.x.state`.
#[derive(Debug)]
struct StatesObject {
    sm: Arc<StateMachine>,
}

impl Object for StatesObject {
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let domain = key.as_str()?.to_owned();
        Some(Value::from_object(DomainObject {
            sm: self.sm.clone(),
            domain,
        }))
    }

    fn call(self: &Arc<Self>, _state: &mut JState<'_, '_>, args: &[Value]) -> Result<Value, Error> {
        let entity_id = args.first().and_then(Value::as_str).ok_or_else(|| {
            Error::new(ErrorKind::MissingArgument, "states() expects an entity id")
        })?;
        // HA returns "unknown" for missing entities.
        Ok(Value::from(
            lookup(&self.sm, entity_id).map_or_else(|| "unknown".to_owned(), |s| s.state.clone()),
        ))
    }
}

#[derive(Debug)]
struct DomainObject {
    sm: Arc<StateMachine>,
    domain: String,
}

impl Object for DomainObject {
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let state = lookup(&self.sm, &format!("{}.{}", self.domain, key.as_str()?))?;
        Some(Value::from(Serde(&*state)))
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        let prefix = format!("{}.", self.domain);
        let items: Vec<Value> = self
            .sm
            .all()
            .into_iter()
            .filter(|s| s.entity_id.as_str().starts_with(&prefix))
            .map(|s| Value::from(Serde(&*s)))
            .collect();
        Enumerator::Values(items)
    }

    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Iterable
    }
}

/// Minimal datetime object returned by `now()`.
#[derive(Debug)]
struct DateTimeObject(DateTime<Tz>);

impl Object for DateTimeObject {
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        use chrono::{Datelike, Timelike};
        let dt = &self.0;
        Some(match key.as_str()? {
            "year" => Value::from(dt.year()),
            "month" => Value::from(dt.month()),
            "day" => Value::from(dt.day()),
            "hour" => Value::from(dt.hour()),
            "minute" => Value::from(dt.minute()),
            "second" => Value::from(dt.second()),
            "weekday" => Value::from(dt.weekday().num_days_from_monday()),
            _ => return None,
        })
    }

    fn call_method(
        self: &Arc<Self>,
        _state: &mut JState<'_, '_>,
        method: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        match method {
            "strftime" => {
                let fmt = args.first().and_then(Value::as_str).ok_or_else(|| {
                    Error::new(ErrorKind::MissingArgument, "strftime expects a format")
                })?;
                Ok(Value::from(self.0.format(fmt).to_string()))
            }
            "isoformat" => Ok(Value::from(self.0.to_rfc3339())),
            _ => Err(Error::new(
                ErrorKind::UnknownMethod,
                format!("datetime has no method {method}"),
            )),
        }
    }

    fn render(self: &Arc<Self>, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.format("%Y-%m-%d %H:%M:%S%.6f%:z"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rusthome_core::{EventBus, ManualClock};
    use rusthome_model::Context;
    use serde_json::{Map, json};

    fn engine() -> TemplateEngine {
        let clock = Arc::new(ManualClock::new(
            Utc.with_ymd_and_hms(2026, 10, 17, 10, 5, 0).unwrap(),
        ));
        let sm = Arc::new(StateMachine::new(EventBus::new(8), clock.clone()));
        let mut attrs = Map::new();
        attrs.insert("temperature".into(), json!(14.5));
        sm.set(
            EntityId::parse("weather.bagneux").unwrap(),
            "sunny",
            attrs,
            Context::default(),
        );
        sm.set(
            EntityId::parse("input_boolean.presence").unwrap(),
            "on",
            Map::new(),
            Context::default(),
        );
        sm.set(
            EntityId::parse("automation.tireuse").unwrap(),
            "on",
            Map::new(),
            Context::default(),
        );
        TemplateEngine::new(sm, clock, chrono_tz::Europe::Paris)
    }

    fn r(src: &str) -> String {
        engine().render(src, &json!({})).unwrap()
    }

    #[test]
    fn states_function_and_object() {
        assert_eq!(r("{{ states('weather.bagneux') }}"), "sunny");
        assert_eq!(r("{{ states('sensor.missing') }}"), "unknown");
        // Legacy form used by 10 imported templates (cf. inventaire/02 §4)
        assert_eq!(r("{{ states.weather.bagneux.state }}"), "sunny");
        assert_eq!(
            r("{{ states.weather.bagneux.attributes.temperature }}"),
            "14.5"
        );
        assert_eq!(r("{{ states.input_boolean | list | length }}"), "1");
    }

    #[test]
    fn helpers() {
        // Booleans render Python-style (`True`), like Home Assistant.
        assert_eq!(r("{{ is_state('input_boolean.presence', 'on') }}"), "True");
        assert_eq!(
            r("{{ is_state('input_boolean.presence', ['off', 'on']) }}"),
            "True"
        );
        assert_eq!(
            r("{{ state_attr('weather.bagneux', 'temperature') | round }}"),
            "14.0"
        );
        assert_eq!(
            r("{{ state_attr('weather.bagneux', 'nope') is none }}"),
            "True"
        );
        assert_eq!(
            r("{{ iif(is_state('input_boolean.presence','on'), 'oui', 'non') }}"),
            "oui"
        );
    }

    #[test]
    fn now_is_local_time() {
        // 10:05 UTC = 12:05 in Paris (CEST)
        assert_eq!(r("{{ now().strftime('%H:%M') }}"), "12:05");
        assert_eq!(r("{{ now().hour }}"), "12");
        assert_eq!(r("{{ as_timestamp(now()) }}"), "1792231500.0");
    }

    #[test]
    fn real_corpus_date_arithmetic() {
        let e = engine();
        // 10:05:00 now; state changed 3 minutes earlier
        let ctx = json!({"trigger": {"from_state": {"last_changed": "2026-10-17T10:02:00+00:00"}}});
        assert_eq!(
            e.render(
                "{{ (now() - trigger.from_state.last_changed).total_seconds() > 90 }}",
                &ctx
            )
            .unwrap(),
            "True"
        );
        assert_eq!(
            e.render("{{ ((now() - trigger.from_state.last_changed).total_seconds() / 60) | round(1) }} min", &ctx).unwrap(),
            "3.0 min"
        );
        // Automation never triggered: `last_triggered` is null → default(0).
        assert_eq!(
            e.render("{{ as_timestamp(states.automation.tireuse.attributes.last_triggered | default(0)) }}", &json!({})).unwrap(),
            "0.0"
        );
    }

    #[test]
    fn badge_lookup_with_dict_get() {
        // Simplified from the real « Badge » automation (UIDs replaced).
        let src = "{% set u = ((trigger.tag_id | default('', true)) or (trigger.event.data.tag_id | default('', true))) | upper | trim %}{% set noms = {'AA-00-00-05': 'Hugo', 'AA-00-00-01': 'Léo'} %}{{ noms.get(u, 'Quelqu un') }}";
        let e = engine();
        assert_eq!(
            e.render(src, &json!({"trigger": {"tag_id": "aa-00-00-01 "}}))
                .unwrap(),
            "Léo"
        );
        assert_eq!(
            e.render(
                src,
                &json!({"trigger": {"event": {"data": {"tag_id": "ZZ"}}}})
            )
            .unwrap(),
            "Quelqu un"
        );
    }

    #[test]
    fn random_picks_an_element() {
        let out = engine()
            .render("{{ ['Radar', 'Snake'] | random }}", &json!({}))
            .unwrap();
        assert!(out == "Radar" || out == "Snake");
    }

    #[test]
    fn context_variables_and_errors() {
        let e = engine();
        assert_eq!(
            e.render(
                "{{ trigger.tag_id }}",
                &json!({"trigger": {"tag_id": "22-9C"}})
            )
            .unwrap(),
            "22-9C"
        );
        assert!(e.render("{{ now().nope() }}", &json!({})).is_err());
    }
}
