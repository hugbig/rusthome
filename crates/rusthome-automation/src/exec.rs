//! Execution of conditions and action sequences, with step-by-step traces.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Datelike, Timelike, Utc};
use chrono_tz::Tz;
use futures_util::future::{BoxFuture, join_all};
use rusthome_core::{Event, Hub, ServiceCall};
use rusthome_model::{Context, EntityId};
use rusthome_template::TemplateEngine;
use serde::Serialize;
use serde_json::{Map, Value, json};
use tokio::sync::Mutex;

use crate::compile::{Action, Condition, Dur, Repeat, Step, Threshold};

/// One recorded step of a run.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TraceStep {
    pub path: String,
    pub kind: String,
    pub at: DateTime<Utc>,
    pub result: StepResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepResult {
    Ok,
    /// Condition evaluated to false.
    False,
    /// Failed but `continue_on_error` let the run go on.
    ErrorContinued,
    Error,
    Skipped,
}

/// How a run ended.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Finished,
    /// Automation conditions were false: actions not run.
    ConditionFailed,
    /// A `condition` action or `stop` ended the run normally.
    Stopped,
    Error,
    Cancelled,
}

/// Shared state of one run.
pub struct Run {
    pub hub: Hub,
    pub tpl: Arc<TemplateEngine>,
    pub tz: Tz,
    pub context: Context,
    /// Template variables: `trigger`, `this`, automation variables…
    pub vars: Mutex<Map<String, Value>>,
    pub steps: Mutex<Vec<TraceStep>>,
}

/// Why a sequence stopped early.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Halt {
    Stopped(String),
    Error(String),
}

impl Run {
    async fn record(&self, path: &str, kind: &str, result: StepResult, detail: Option<String>) {
        self.steps.lock().await.push(TraceStep {
            path: path.to_owned(),
            kind: kind.to_owned(),
            at: self.hub.clock.now(),
            result,
            detail,
        });
    }

    async fn ctx(&self) -> Value {
        Value::Object(self.vars.lock().await.clone())
    }

    pub async fn render_str(&self, src: &str) -> Result<String, String> {
        let ctx = self.ctx().await;
        self.tpl.render(src, &ctx).map_err(|e| e.to_string())
    }

    /// Renders every templated string of a JSON value, keeping native types
    /// (`"{{ 5 }}"` → `5`), like HA.
    pub fn render_value<'a>(&'a self, v: &'a Value) -> BoxFuture<'a, Result<Value, String>> {
        Box::pin(async move {
            Ok(match v {
                Value::String(s) if is_template(s) => native(&self.render_str(s).await?),
                Value::Array(a) => {
                    let mut out = Vec::with_capacity(a.len());
                    for x in a {
                        out.push(self.render_value(x).await?);
                    }
                    Value::Array(out)
                }
                Value::Object(o) => {
                    let mut out = Map::new();
                    for (k, x) in o {
                        out.insert(k.clone(), self.render_value(x).await?);
                    }
                    Value::Object(out)
                }
                other => other.clone(),
            })
        })
    }

    async fn duration(&self, d: &Dur) -> Result<Duration, String> {
        match d {
            Dur::Fixed(x) => Ok(*x),
            Dur::Template(src) => {
                let rendered = self.render_str(src).await?;
                let v: Value =
                    serde_json::from_str(&rendered).unwrap_or(Value::String(rendered.clone()));
                match crate::compile::duration(&v, "").map_err(|e| e.message)? {
                    Dur::Fixed(x) => Ok(x),
                    Dur::Template(_) => Err(format!(
                        "duration template rendered to another template: {rendered}"
                    )),
                }
            }
        }
    }

    // ---------------- conditions ----------------

    /// All conditions true (AND), recording each evaluation.
    pub fn all<'a>(&'a self, conds: &'a [Condition], path: &'a str) -> BoxFuture<'a, bool> {
        Box::pin(async move {
            for (i, c) in conds.iter().enumerate() {
                let p = format!("{path}[{i}]");
                let ok = self.check(c, &p).await;
                if !ok {
                    return false;
                }
            }
            true
        })
    }

    #[allow(clippy::too_many_lines)] // one arm per condition type, kept together for readability
    fn check<'a>(&'a self, c: &'a Condition, path: &'a str) -> BoxFuture<'a, bool> {
        Box::pin(async move {
            let (ok, kind, detail) = match c {
                Condition::And(cs) => (
                    self.all(cs, &format!("{path}.conditions")).await,
                    "and",
                    None,
                ),
                Condition::Or(cs) => {
                    let mut any = false;
                    for (i, x) in cs.iter().enumerate() {
                        if self.check(x, &format!("{path}.conditions[{i}]")).await {
                            any = true;
                            break;
                        }
                    }
                    (any, "or", None)
                }
                Condition::Not(cs) => {
                    let mut any = false;
                    for (i, x) in cs.iter().enumerate() {
                        if self.check(x, &format!("{path}.conditions[{i}]")).await {
                            any = true;
                        }
                    }
                    (!any, "not", None)
                }
                Condition::Template(src) => match self.render_str(src).await {
                    Ok(out) => (truthy(&out), "template", Some(out)),
                    Err(e) => (false, "template", Some(e)),
                },
                Condition::Trigger(ids) => {
                    let current = self
                        .vars
                        .lock()
                        .await
                        .get("trigger")
                        .and_then(|t| t["id"].as_str().map(str::to_owned));
                    (
                        current.as_ref().is_some_and(|c| ids.contains(c)),
                        "trigger",
                        current,
                    )
                }
                Condition::State {
                    entity_ids,
                    states,
                    attribute,
                    for_,
                    match_any,
                } => {
                    let mut results = Vec::new();
                    for id in entity_ids {
                        results.push(
                            self.state_matches(id, states, attribute.as_deref(), for_.as_ref())
                                .await,
                        );
                    }
                    let ok = if *match_any {
                        results.iter().any(|x| *x)
                    } else {
                        results.iter().all(|x| *x)
                    };
                    (ok, "state", None)
                }
                Condition::NumericState {
                    entity_ids,
                    attribute,
                    above,
                    below,
                } => {
                    let ok = entity_ids.iter().all(|id| {
                        numeric_in_range(
                            &self.hub,
                            id,
                            attribute.as_deref(),
                            above.as_ref(),
                            below.as_ref(),
                        )
                    });
                    (ok, "numeric_state", None)
                }
                Condition::Time {
                    after,
                    before,
                    weekdays,
                } => (
                    time_matches(
                        self.hub.clock.now().with_timezone(&self.tz),
                        *after,
                        *before,
                        weekdays,
                    ),
                    "time",
                    None,
                ),
                Condition::Sun { .. } | Condition::Zone { .. } => (
                    false,
                    "unsupported",
                    Some(
                        "sun/zone conditions are not supported yet (no location configured)".into(),
                    ),
                ),
            };
            self.record(
                path,
                kind,
                if ok {
                    StepResult::Ok
                } else {
                    StepResult::False
                },
                detail,
            )
            .await;
            ok
        })
    }

    async fn state_matches(
        &self,
        id: &EntityId,
        states: &[String],
        attribute: Option<&str>,
        for_: Option<&Dur>,
    ) -> bool {
        let Some(s) = self.hub.states.get(id) else {
            return false;
        };
        let value = match attribute {
            Some(a) => s.attributes.get(a).map(value_as_state).unwrap_or_default(),
            None => s.state.clone(),
        };
        if !states.contains(&value) {
            return false;
        }
        match for_ {
            None => true,
            Some(d) => match self.duration(d).await {
                Ok(min) => {
                    self.hub.clock.now() - s.last_changed
                        >= chrono::Duration::from_std(min).unwrap_or_default()
                }
                Err(_) => false,
            },
        }
    }

    // ---------------- actions ----------------

    pub fn steps<'a>(
        &'a self,
        steps: &'a [Step],
        path: &'a str,
    ) -> BoxFuture<'a, Result<(), Halt>> {
        Box::pin(async move {
            for (i, s) in steps.iter().enumerate() {
                self.step(s, &format!("{path}[{i}]")).await?;
            }
            Ok(())
        })
    }

    fn step<'a>(&'a self, s: &'a Step, path: &'a str) -> BoxFuture<'a, Result<(), Halt>> {
        Box::pin(async move {
            if !s.enabled {
                self.record(
                    path,
                    kind_of(&s.action),
                    StepResult::Skipped,
                    Some("disabled".into()),
                )
                .await;
                return Ok(());
            }
            match self.action(&s.action, path).await {
                Err(Halt::Error(e)) if s.continue_on_error => {
                    self.record(
                        path,
                        kind_of(&s.action),
                        StepResult::ErrorContinued,
                        Some(e),
                    )
                    .await;
                    Ok(())
                }
                other => other,
            }
        })
    }

    #[allow(clippy::too_many_lines)] // one arm per action type, kept together for readability
    fn action<'a>(&'a self, a: &'a Action, path: &'a str) -> BoxFuture<'a, Result<(), Halt>> {
        Box::pin(async move {
            match a {
                Action::Service {
                    service,
                    target,
                    data,
                } => {
                    let (domain, name) = service.split_once('.').unwrap_or((service.as_str(), ""));
                    let mut payload = match self.render_value(data).await {
                        Ok(Value::Object(o)) => o,
                        Ok(Value::Null) => Map::new(),
                        Ok(other) => {
                            return self
                                .fail(
                                    path,
                                    "service",
                                    format!("data must be a mapping, got {other}"),
                                )
                                .await;
                        }
                        Err(e) => return self.fail(path, "service", e).await,
                    };
                    match self.render_value(target).await {
                        Ok(Value::Object(t)) => {
                            for (k, v) in t {
                                payload.insert(k, v);
                            }
                        }
                        Ok(_) => {}
                        Err(e) => return self.fail(path, "service", e).await,
                    }
                    let call = ServiceCall {
                        domain: domain.to_owned(),
                        service: name.to_owned(),
                        data: Value::Object(payload.clone()),
                        context: Context {
                            id: format!("{}-{path}", self.context.id),
                            parent_id: Some(self.context.id.clone()),
                            user_id: None,
                        },
                    };
                    match self.hub.call_service(call).await {
                        Ok(_) => {
                            self.record(
                                path,
                                "service",
                                StepResult::Ok,
                                Some(format!("{service} {}", Value::Object(payload))),
                            )
                            .await;
                            Ok(())
                        }
                        Err(e) => self.fail(path, "service", format!("{service}: {e}")).await,
                    }
                }
                Action::Delay(d) => match self.duration(d).await {
                    Ok(x) => {
                        self.record(
                            path,
                            "delay",
                            StepResult::Ok,
                            Some(format!("{}s", x.as_secs_f64())),
                        )
                        .await;
                        tokio::time::sleep(x).await;
                        Ok(())
                    }
                    Err(e) => self.fail(path, "delay", e).await,
                },
                Action::Choose { options, default } => {
                    for (i, (conds, seq)) in options.iter().enumerate() {
                        if self
                            .all(conds, &format!("{path}.choose[{i}].conditions"))
                            .await
                        {
                            self.record(
                                path,
                                "choose",
                                StepResult::Ok,
                                Some(format!("option {i}")),
                            )
                            .await;
                            return self
                                .steps(seq, &format!("{path}.choose[{i}].sequence"))
                                .await;
                        }
                    }
                    self.record(path, "choose", StepResult::Ok, Some("default".into()))
                        .await;
                    self.steps(default, &format!("{path}.default")).await
                }
                Action::If {
                    conditions,
                    then,
                    otherwise,
                } => {
                    if self.all(conditions, &format!("{path}.if")).await {
                        self.steps(then, &format!("{path}.then")).await
                    } else {
                        self.steps(otherwise, &format!("{path}.else")).await
                    }
                }
                Action::Repeat { kind, sequence } => self.repeat(kind, sequence, path).await,
                Action::Parallel(branches) => {
                    self.record(
                        path,
                        "parallel",
                        StepResult::Ok,
                        Some(format!("{} branches", branches.len())),
                    )
                    .await;
                    let futs = branches
                        .iter()
                        .enumerate()
                        .map(|(i, b)| self.steps_owned(b, format!("{path}.parallel[{i}]")));
                    for r in join_all(futs).await {
                        r?;
                    }
                    Ok(())
                }
                Action::Sequence(seq) => self.steps(seq, &format!("{path}.sequence")).await,
                Action::Condition(c) => {
                    if self.check(c, path).await {
                        Ok(())
                    } else {
                        Err(Halt::Stopped(format!("condition at {path} is false")))
                    }
                }
                Action::Variables(vars) => {
                    for (k, v) in vars {
                        match self.render_value(v).await {
                            Ok(x) => {
                                self.vars.lock().await.insert(k.clone(), x);
                            }
                            Err(e) => return self.fail(path, "variables", e).await,
                        }
                    }
                    self.record(path, "variables", StepResult::Ok, None).await;
                    Ok(())
                }
                Action::WaitTemplate {
                    template,
                    timeout,
                    continue_on_timeout,
                } => {
                    let limit = match timeout {
                        Some(d) => Some(self.duration(d).await.map_err(Halt::Error)?),
                        None => None,
                    };
                    let started = tokio::time::Instant::now();
                    loop {
                        if self.render_str(template).await.is_ok_and(|o| truthy(&o)) {
                            self.record(path, "wait_template", StepResult::Ok, None)
                                .await;
                            return Ok(());
                        }
                        if limit.is_some_and(|l| started.elapsed() >= l) {
                            self.record(
                                path,
                                "wait_template",
                                StepResult::False,
                                Some("timeout".into()),
                            )
                            .await;
                            return if *continue_on_timeout {
                                Ok(())
                            } else {
                                Err(Halt::Stopped("wait timeout".into()))
                            };
                        }
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
                Action::Event { event_type, data } => {
                    let data = self.render_value(data).await.map_err(Halt::Error)?;
                    self.hub.bus.publish(Event::Custom {
                        event_type: event_type.clone(),
                        data,
                    });
                    self.record(path, "event", StepResult::Ok, Some(event_type.clone()))
                        .await;
                    Ok(())
                }
                Action::Stop { reason, error } => {
                    self.record(
                        path,
                        "stop",
                        if *error {
                            StepResult::Error
                        } else {
                            StepResult::Ok
                        },
                        Some(reason.clone()),
                    )
                    .await;
                    Err(if *error {
                        Halt::Error(reason.clone())
                    } else {
                        Halt::Stopped(reason.clone())
                    })
                }
            }
        })
    }

    fn steps_owned(&self, steps: &[Step], path: String) -> BoxFuture<'_, Result<(), Halt>> {
        let steps = steps.to_vec();
        Box::pin(async move { self.steps(&steps, &path).await })
    }

    async fn repeat(&self, kind: &Repeat, sequence: &[Step], path: &str) -> Result<(), Halt> {
        // Safety net against runaway loops (the alarm sirens loop every 8 s).
        const MAX_ITERATIONS: u64 = 100_000;
        let count = match kind {
            Repeat::Count(src) => {
                let rendered = if is_template(src) {
                    self.render_str(src).await.map_err(Halt::Error)?
                } else {
                    src.clone()
                };
                Some(
                    rendered
                        .trim()
                        .parse::<u64>()
                        .map_err(|_| Halt::Error(format!("invalid repeat count {rendered}")))?,
                )
            }
            _ => None,
        };
        let mut i: u64 = 0;
        loop {
            i += 1;
            if i > MAX_ITERATIONS || count.is_some_and(|c| i > c) {
                break;
            }
            self.vars
                .lock()
                .await
                .insert("repeat".into(), json!({"index": i, "first": i == 1}));
            if let Repeat::While(conds) = kind
                && !self.all(conds, &format!("{path}.repeat.while")).await
            {
                break;
            }
            self.steps(sequence, &format!("{path}.repeat.sequence"))
                .await?;
            if let Repeat::Until(conds) = kind
                && self.all(conds, &format!("{path}.repeat.until")).await
            {
                break;
            }
        }
        self.record(
            path,
            "repeat",
            StepResult::Ok,
            Some(format!("{} iterations", i - 1)),
        )
        .await;
        Ok(())
    }

    async fn fail(&self, path: &str, kind: &str, message: String) -> Result<(), Halt> {
        self.record(path, kind, StepResult::Error, Some(message.clone()))
            .await;
        Err(Halt::Error(message))
    }
}

fn kind_of(a: &Action) -> &'static str {
    match a {
        Action::Service { .. } => "service",
        Action::Delay(_) => "delay",
        Action::Choose { .. } => "choose",
        Action::If { .. } => "if",
        Action::Repeat { .. } => "repeat",
        Action::Parallel(_) => "parallel",
        Action::Sequence(_) => "sequence",
        Action::Condition(_) => "condition",
        Action::Variables(_) => "variables",
        Action::WaitTemplate { .. } => "wait_template",
        Action::Event { .. } => "event",
        Action::Stop { .. } => "stop",
    }
}

#[must_use]
pub fn is_template(s: &str) -> bool {
    s.contains("{{") || s.contains("{%")
}

/// Rendered text → native JSON value when it looks like one (HA behaviour).
#[must_use]
pub fn native(s: &str) -> Value {
    let t = s.trim();
    match t {
        "True" => return Value::Bool(true),
        "False" => return Value::Bool(false),
        "None" => return Value::Null,
        _ => {}
    }
    if (t.parse::<f64>().is_ok() || t.starts_with('[') || t.starts_with('{'))
        && let Ok(v) = serde_json::from_str::<Value>(t)
    {
        return v;
    }
    Value::String(s.to_owned())
}

/// Template condition truthiness (HA: `true`, `on`, `yes`, non-zero).
#[must_use]
pub fn truthy(s: &str) -> bool {
    let t = s.trim().to_lowercase();
    match t.as_str() {
        "true" | "on" | "yes" | "enable" => true,
        "false" | "off" | "no" | "" | "none" | "disable" => false,
        other => other.parse::<f64>().is_ok_and(|n| n != 0.0),
    }
}

/// Attribute values compared as HA does: strings as-is, others as JSON text.
#[must_use]
pub fn value_as_state(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(true) => "on".into(),
        Value::Bool(false) => "off".into(),
        other => other.to_string(),
    }
}

fn threshold_value(hub: &Hub, t: &Threshold) -> Option<f64> {
    match t {
        Threshold::Value(v) => Some(*v),
        Threshold::Entity(id) => hub.states.get(id).and_then(|s| s.state.parse().ok()),
    }
}

/// Numeric state strictly within `(above, below)`.
#[must_use]
pub fn numeric_in_range(
    hub: &Hub,
    id: &EntityId,
    attribute: Option<&str>,
    above: Option<&Threshold>,
    below: Option<&Threshold>,
) -> bool {
    let Some(s) = hub.states.get(id) else {
        return false;
    };
    let raw = match attribute {
        Some(a) => s.attributes.get(a).map(value_as_state),
        None => Some(s.state.clone()),
    };
    let Some(v) = raw.and_then(|r| r.parse::<f64>().ok()) else {
        return false;
    };
    above
        .and_then(|t| threshold_value(hub, t))
        .is_none_or(|a| v > a)
        && below
            .and_then(|t| threshold_value(hub, t))
            .is_none_or(|b| v < b)
}

fn time_matches(
    now: DateTime<Tz>,
    after: Option<(u32, u32, u32)>,
    before: Option<(u32, u32, u32)>,
    weekdays: &[String],
) -> bool {
    let day = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
        [now.weekday().num_days_from_monday() as usize];
    if !weekdays.is_empty() && !weekdays.iter().any(|w| w == day) {
        return false;
    }
    let t = (now.hour(), now.minute(), now.second());
    match (after, before) {
        (Some(a), Some(b)) if a <= b => t >= a && t < b,
        // Window crossing midnight (22:00 → 06:00).
        (Some(a), Some(b)) => t >= a || t < b,
        (Some(a), None) => t >= a,
        (None, Some(b)) => t < b,
        (None, None) => true,
    }
}
