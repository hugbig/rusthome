//! The automation engine: triggers, modes, runs and traces.
//!
//! Safety: automations imported from HA are **disabled** until the user
//! enables them one by one (phase P2). Even enabled, they act through the
//! service registry, so read-only integrations still refuse every command.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex as StdMutex, PoisonError};

use chrono::{DateTime, Timelike, Utc};
use chrono_tz::Tz;
use rusthome_core::{Event, Hub};
use rusthome_model::{Context, EntityEntry, EntityId, EntityStatus, StateChanged};
use rusthome_template::TemplateEngine;
use serde::Serialize;
use serde_json::{Map, Value, json};
use tokio::sync::Mutex;
use tokio::sync::broadcast::error::RecvError;

use crate::compile::{Automation, CompileError, Mode, TimePart, TriggerKind, compile};
use crate::exec::{Halt, Outcome, Run, TraceStep, numeric_in_range, value_as_state};
use crate::normalize::Issue;

pub const INTEGRATION: &str = "automation";
/// Traces kept per automation.
pub const TRACES_KEPT: usize = 20;

/// A finished (or running) run, for the trace viewer.
#[derive(Debug, Clone, Serialize)]
pub struct Trace {
    pub run_id: u64,
    pub automation_id: String,
    pub trigger: Value,
    pub started: DateTime<Utc>,
    pub finished: Option<DateTime<Utc>>,
    pub outcome: Option<Outcome>,
    pub error: Option<String>,
    pub steps: Vec<TraceStep>,
}

/// Public view of one automation.
#[derive(Debug, Clone, Serialize)]
pub struct Info {
    pub id: String,
    pub alias: String,
    pub entity_id: EntityId,
    pub enabled: bool,
    pub valid: bool,
    pub errors: Vec<CompileError>,
    pub issues: Vec<Issue>,
    pub last_triggered: Option<DateTime<Utc>>,
    pub running: usize,
}

struct Entry {
    doc: Value,
    compiled: Option<Arc<Automation>>,
    errors: Vec<CompileError>,
    issues: Vec<Issue>,
    entity_id: EntityId,
    enabled: bool,
    last_triggered: Option<DateTime<Utc>>,
    running: HashMap<u64, tokio::task::AbortHandle>,
    queue: VecDeque<Value>,
    /// `for` timers: trigger index → task waiting for the state to hold.
    pending_for: HashMap<usize, tokio::task::AbortHandle>,
}

pub struct Engine {
    hub: Hub,
    tpl: Arc<TemplateEngine>,
    tz: Tz,
    entries: StdMutex<BTreeMap<String, Entry>>,
    traces: StdMutex<HashMap<String, VecDeque<Trace>>>,
    next_run: std::sync::atomic::AtomicU64,
    /// Last second processed by the time ticker (dedup).
    last_tick: StdMutex<Option<DateTime<Tz>>>,
}

impl Engine {
    #[must_use]
    pub fn new(hub: Hub, tz: Tz) -> Arc<Self> {
        let tpl = Arc::new(TemplateEngine::new(
            hub.states.clone(),
            hub.clock.clone(),
            tz,
        ));
        Arc::new(Self {
            hub,
            tpl,
            tz,
            entries: StdMutex::default(),
            traces: StdMutex::default(),
            next_run: std::sync::atomic::AtomicU64::new(1),
            last_tick: StdMutex::default(),
        })
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Entry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Adds or replaces an automation (already normalized). Invalid documents
    /// are kept (so nothing is lost) but cannot be enabled.
    pub fn upsert(
        &self,
        doc: Value,
        entity_id: EntityId,
        enabled: bool,
        issues: Vec<Issue>,
    ) -> Info {
        let (compiled, errors) = match compile(&doc) {
            Ok(a) => (Some(Arc::new(a)), Vec::new()),
            Err(e) => (None, e),
        };
        let id = doc["id"]
            .as_str()
            .map_or_else(|| doc["id"].to_string(), str::to_owned);
        let enabled = enabled && compiled.is_some();
        {
            let mut entries = self.entries();
            let previous = entries.remove(&id);
            if let Some(p) = &previous {
                p.running
                    .values()
                    .chain(p.pending_for.values())
                    .for_each(tokio::task::AbortHandle::abort);
            }
            entries.insert(
                id.clone(),
                Entry {
                    doc,
                    compiled,
                    errors,
                    issues,
                    entity_id: entity_id.clone(),
                    enabled,
                    last_triggered: previous.and_then(|p| p.last_triggered),
                    running: HashMap::new(),
                    queue: VecDeque::new(),
                    pending_for: HashMap::new(),
                },
            );
        }
        self.hub.registries.upsert_entity(EntityEntry {
            uid: format!("automation-{id}"),
            entity_id,
            integration: INTEGRATION.into(),
            unique_id: Some(id.clone()),
            device_id: None,
            name: None,
            area_id: None,
            labels: vec![],
            icon: None,
            hidden: false,
            disabled: false,
            status: EntityStatus::Active,
        });
        self.publish_state(&id);
        self.info(&id)
            .unwrap_or_else(|| unreachable!("just inserted"))
    }

    pub fn remove(&self, id: &str) -> bool {
        let Some(e) = self.entries().remove(id) else {
            return false;
        };
        e.running
            .values()
            .chain(e.pending_for.values())
            .for_each(tokio::task::AbortHandle::abort);
        self.hub.states.remove(&e.entity_id);
        true
    }

    /// Enables or disables. Refused for invalid documents.
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<Info, String> {
        {
            let mut entries = self.entries();
            let e = entries
                .get_mut(id)
                .ok_or_else(|| format!("unknown automation {id}"))?;
            if enabled && e.compiled.is_none() {
                return Err("this automation has errors: fix it before enabling".into());
            }
            e.enabled = enabled;
            if !enabled {
                e.pending_for.drain().for_each(|(_, h)| h.abort());
            }
        }
        self.publish_state(id);
        self.info(id)
            .ok_or_else(|| format!("unknown automation {id}"))
    }

    #[must_use]
    pub fn info(&self, id: &str) -> Option<Info> {
        self.entries().get(id).map(|e| info_of(id, e))
    }

    #[must_use]
    pub fn list(&self) -> Vec<Info> {
        self.entries()
            .iter()
            .map(|(id, e)| info_of(id, e))
            .collect()
    }

    #[must_use]
    pub fn document(&self, id: &str) -> Option<Value> {
        self.entries().get(id).map(|e| e.doc.clone())
    }

    #[must_use]
    pub fn traces(&self, id: &str) -> Vec<Trace> {
        self.traces
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
            .map(|t| t.iter().rev().cloned().collect())
            .unwrap_or_default()
    }

    fn publish_state(&self, id: &str) {
        let Some((entity_id, attrs, on)) = self.entries().get(id).map(|e| {
            let mut a = Map::new();
            a.insert("friendly_name".into(), e.doc["alias"].clone());
            a.insert("id".into(), id.into());
            a.insert("mode".into(), e.doc["mode"].clone());
            a.insert("current".into(), e.running.len().into());
            a.insert(
                "last_triggered".into(),
                json!(e.last_triggered.map(|t| t.to_rfc3339())),
            );
            (e.entity_id.clone(), a, e.enabled)
        }) else {
            return;
        };
        self.hub.states.set(
            entity_id,
            if on { "on" } else { "off" },
            attrs,
            Context::default(),
        );
    }

    // ---------------- triggers ----------------

    /// Follows the bus, the clock and startup; runs forever.
    pub async fn run(self: Arc<Self>) {
        self.fire_matching(
            |k| matches!(k, TriggerKind::Startup),
            &json!({"platform": "homeassistant", "event": "start"}),
        );
        let mut rx = self.hub.bus.subscribe();
        let mut tick = tokio::time::interval(std::time::Duration::from_millis(250));
        loop {
            tokio::select! {
                ev = rx.recv() => match ev {
                    Ok(ev) => self.handle_event(&ev),
                    Err(RecvError::Lagged(n)) => tracing::warn!(missed = n, "automation engine lagging behind the bus"),
                    Err(RecvError::Closed) => return,
                },
                _ = tick.tick() => self.tick(self.hub.clock.now()),
            }
        }
    }

    pub fn handle_event(self: &Arc<Self>, ev: &Event) {
        match ev {
            Event::StateChanged(change) => self.on_state_changed(change),
            Event::Custom { event_type, data } => {
                let trig =
                    json!({"platform": "event", "event": {"event_type": event_type, "data": data}});
                self.fire_matching(
                    |k| match k {
                        TriggerKind::Event {
                            event_type: t,
                            event_data,
                        } => {
                            t == event_type
                                && event_data.iter().all(|(key, v)| data.get(key) == Some(v))
                        }
                        _ => false,
                    },
                    &trig,
                );
                if event_type == "tag_scanned" {
                    let tag = data["tag_id"].as_str().unwrap_or_default().to_owned();
                    let trig = json!({"platform": "tag", "tag_id": tag, "event": {"event_type": event_type, "data": data}});
                    self.fire_matching(
                        |k| matches!(k, TriggerKind::Tag { tag_ids } if tag_ids.contains(&tag)),
                        &trig,
                    );
                }
            }
            Event::RegistryUpdated { .. } => {}
        }
    }

    /// MQTT message (wired by the binary to the shared MQTT client).
    pub fn handle_mqtt(self: &Arc<Self>, topic: &str, payload: &str) {
        let trig = json!({"platform": "mqtt", "topic": topic, "payload": payload});
        self.fire_matching(
            |k| matches!(k, TriggerKind::Mqtt { topic: t, payload: p } if t == topic && p.as_deref().is_none_or(|p| p == payload)),
            &trig,
        );
    }

    /// Topics the MQTT triggers need.
    #[must_use]
    pub fn mqtt_topics(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .entries()
            .values()
            .filter_map(|e| e.compiled.clone())
            .flat_map(|a| {
                a.triggers
                    .iter()
                    .filter_map(|t| {
                        if let TriggerKind::Mqtt { topic, .. } = &t.kind {
                            Some(topic.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        out.sort();
        out.dedup();
        out
    }

    fn on_state_changed(self: &Arc<Self>, change: &StateChanged) {
        let candidates: Vec<(String, Arc<Automation>)> = self
            .entries()
            .iter()
            .filter(|(_, e)| e.enabled)
            .filter_map(|(id, e)| e.compiled.clone().map(|c| (id.clone(), c)))
            .collect();
        for (id, auto) in candidates {
            for (idx, t) in auto.triggers.iter().enumerate().filter(|(_, t)| t.enabled) {
                let fires = match &t.kind {
                    TriggerKind::State {
                        entity_ids,
                        from,
                        to,
                        attribute,
                        ..
                    } => {
                        entity_ids.contains(&change.entity_id)
                            && state_trigger_matches(
                                change,
                                from.as_deref(),
                                to.as_deref(),
                                attribute.as_deref(),
                            )
                    }
                    TriggerKind::NumericState {
                        entity_ids,
                        attribute,
                        above,
                        below,
                        ..
                    } => {
                        entity_ids.contains(&change.entity_id)
                            && {
                                // Fires on crossing into the range only.
                                let now_in = numeric_in_range(
                                    &self.hub,
                                    &change.entity_id,
                                    attribute.as_deref(),
                                    above.as_ref(),
                                    below.as_ref(),
                                );
                                let was_in = change.old_state.as_ref().is_some_and(|old| {
                                let raw = match attribute.as_deref() {
                                    Some(a) => old.attributes.get(a).map(value_as_state),
                                    None => Some(old.state.clone()),
                                };
                                raw.and_then(|r| r.parse::<f64>().ok()).is_some_and(|v| {
                                    above.as_ref().is_none_or(|a| matches!(a, crate::compile::Threshold::Value(x) if v > *x))
                                        && below.as_ref().is_none_or(|b| matches!(b, crate::compile::Threshold::Value(x) if v < *x))
                                })
                            });
                                now_in && !was_in
                            }
                    }
                    TriggerKind::EventEntity {
                        entity_id,
                        event_types,
                    } => {
                        *entity_id == change.entity_id
                            && change.new_state.as_ref().is_some_and(|s| {
                                !s.is_unavailable()
                                    && change.old_state.as_ref().is_none_or(|o| o.state != s.state)
                                    && (event_types.is_empty()
                                        || s.attributes
                                            .get("event_type")
                                            .and_then(Value::as_str)
                                            .is_some_and(|e| event_types.iter().any(|x| x == e)))
                            })
                    }
                    _ => false,
                };
                if !fires {
                    // A change that no longer matches cancels a running `for` timer.
                    let watches = match &t.kind {
                        TriggerKind::State { entity_ids, .. }
                        | TriggerKind::NumericState { entity_ids, .. } => {
                            entity_ids.contains(&change.entity_id)
                        }
                        _ => false,
                    };
                    if watches
                        && trigger_for(&t.kind).is_some()
                        && let Some(h) = self
                            .entries()
                            .get_mut(&id)
                            .and_then(|e| e.pending_for.remove(&idx))
                    {
                        h.abort();
                    }
                    continue;
                }
                let trig = json!({
                    "platform": trigger_platform(&t.kind), "id": t.id.clone().unwrap_or_else(|| idx.to_string()), "idx": idx,
                    "entity_id": change.entity_id, "from_state": change.old_state, "to_state": change.new_state,
                });
                match trigger_for(&t.kind) {
                    Some(d) => self.schedule_for(&id, idx, &d, change.entity_id.clone(), trig),
                    None => self.fire(&id, trig),
                }
            }
        }
    }

    /// `for:` → fire only if the new state still holds after the duration.
    fn schedule_for(
        self: &Arc<Self>,
        id: &str,
        idx: usize,
        d: &crate::compile::Dur,
        entity: EntityId,
        trig: Value,
    ) {
        let crate::compile::Dur::Fixed(wait) = *d else {
            tracing::warn!(
                automation = id,
                "templated `for` on triggers is not supported yet"
            );
            return;
        };
        // At expiry the state must still be the one that triggered (same value
        // and same `last_changed`): a return home cancels « armement après 1 h ».
        let expected_changed = trig["to_state"]["last_changed"].clone();
        let expected_state = trig["to_state"]["state"].clone();
        let me = Arc::clone(self);
        let aid = id.to_owned();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(wait).await;
            let still = me.hub.states.get(&entity).is_some_and(|s| {
                json!(s.last_changed) == expected_changed && json!(s.state) == expected_state
            });
            if still {
                me.fire(&aid, trig);
            }
        })
        .abort_handle();
        if let Some(e) = self.entries().get_mut(id) {
            // A newer change restarts the countdown.
            if let Some(old) = e.pending_for.insert(idx, handle) {
                old.abort();
            }
        }
    }

    /// Time and time-pattern triggers, once per second.
    pub fn tick(self: &Arc<Self>, now_utc: DateTime<Utc>) {
        let now = now_utc
            .with_timezone(&self.tz)
            .with_nanosecond(0)
            .unwrap_or_else(|| now_utc.with_timezone(&self.tz));
        {
            let mut last = self
                .last_tick
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if last.as_ref() == Some(&now) {
                return;
            }
            *last = Some(now);
        }
        let hms = (now.hour(), now.minute(), now.second());
        self.fire_matching(
            |k| match k {
                TriggerKind::Time { at } => at.contains(&hms),
                TriggerKind::TimePattern {
                    hours,
                    minutes,
                    seconds,
                } => pattern_matches(hours, minutes, seconds, hms),
                _ => false,
            },
            &json!({"platform": "time", "now": now.to_rfc3339()}),
        );
    }

    fn fire_matching(self: &Arc<Self>, pred: impl Fn(&TriggerKind) -> bool, base: &Value) {
        let hits: Vec<(String, usize, Option<String>)> = self
            .entries()
            .iter()
            .filter(|(_, e)| e.enabled)
            .filter_map(|(id, e)| e.compiled.as_ref().map(|c| (id, c)))
            .flat_map(|(id, c)| {
                c.triggers
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.enabled && pred(&t.kind))
                    .map(|(i, t)| (id.clone(), i, t.id.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();
        for (id, idx, tid) in hits {
            let mut trig = base.clone();
            trig["idx"] = idx.into();
            trig["id"] = tid.unwrap_or_else(|| idx.to_string()).into();
            self.fire(&id, trig);
        }
    }

    // ---------------- runs ----------------

    /// Starts a run according to the automation mode.
    pub fn fire(self: &Arc<Self>, id: &str, trigger: Value) {
        let mut entries = self.entries();
        let Some(e) = entries.get_mut(id) else { return };
        let Some(auto) = e.compiled.clone() else {
            return;
        };
        let busy = e.running.len();
        let start = match auto.mode {
            Mode::Single if busy > 0 => {
                tracing::info!(
                    automation = id,
                    "already running (mode single): trigger ignored"
                );
                false
            }
            Mode::Restart => {
                e.running.drain().for_each(|(_, h)| h.abort());
                true
            }
            Mode::Queued(max) if busy > 0 => {
                if e.queue.len() < max {
                    e.queue.push_back(trigger.clone());
                } else {
                    tracing::warn!(automation = id, "queue full: trigger dropped");
                }
                false
            }
            Mode::Parallel(max) if busy >= max => {
                tracing::warn!(
                    automation = id,
                    "max parallel runs reached: trigger dropped"
                );
                false
            }
            _ => true,
        };
        if !start {
            return;
        }
        e.last_triggered = Some(self.hub.clock.now());
        let run_id = self
            .next_run
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let me = Arc::clone(self);
        let aid = id.to_owned();
        let (entity_id, doc_vars) = (e.entity_id.clone(), auto.variables.clone());
        let handle = tokio::spawn(async move {
            me.execute(run_id, &aid, &auto, entity_id, doc_vars, trigger)
                .await;
        })
        .abort_handle();
        e.running.insert(run_id, handle);
        drop(entries);
        self.publish_state(id);
    }

    async fn execute(
        self: Arc<Self>,
        run_id: u64,
        id: &str,
        auto: &Automation,
        entity_id: EntityId,
        doc_vars: Map<String, Value>,
        trigger: Value,
    ) {
        let started = self.hub.clock.now();
        let mut vars = Map::new();
        vars.insert("trigger".into(), trigger.clone());
        vars.insert(
            "this".into(),
            self.hub
                .states
                .get(&entity_id)
                .map_or(Value::Null, |s| json!(*s)),
        );
        let run = Run {
            hub: self.hub.clone(),
            tpl: Arc::clone(&self.tpl),
            tz: self.tz,
            context: Context {
                id: format!("automation-{id}-{run_id}"),
                parent_id: None,
                user_id: None,
            },
            vars: Mutex::new(vars),
            steps: Mutex::new(Vec::new()),
        };
        // Automation variables, rendered once with `trigger` available.
        let mut error = None;
        for (k, v) in &doc_vars {
            match run.render_value(v).await {
                Ok(x) => {
                    run.vars.lock().await.insert(k.clone(), x);
                }
                Err(e) => error = Some(format!("variable {k}: {e}")),
            }
        }
        let outcome = if let Some(e) = &error {
            tracing::warn!(automation = id, error = %e, "automation variables failed");
            Outcome::Error
        } else if !run.all(&auto.conditions, "conditions").await {
            Outcome::ConditionFailed
        } else {
            match run.steps(&auto.actions, "actions").await {
                Ok(()) => Outcome::Finished,
                Err(Halt::Stopped(_)) => Outcome::Stopped,
                Err(Halt::Error(e)) => {
                    error = Some(e);
                    Outcome::Error
                }
            }
        };
        let trace = Trace {
            run_id,
            automation_id: id.to_owned(),
            trigger,
            started,
            finished: Some(self.hub.clock.now()),
            outcome: Some(outcome),
            error,
            steps: run.steps.lock().await.clone(),
        };
        self.finish(id, run_id, trace);
    }

    fn finish(self: &Arc<Self>, id: &str, run_id: u64, trace: Trace) {
        {
            let mut traces = self.traces.lock().unwrap_or_else(PoisonError::into_inner);
            let list = traces.entry(id.to_owned()).or_default();
            list.push_back(trace);
            while list.len() > TRACES_KEPT {
                list.pop_front();
            }
        }
        let next = {
            let mut entries = self.entries();
            let Some(e) = entries.get_mut(id) else { return };
            e.running.remove(&run_id);
            e.queue.pop_front()
        };
        self.publish_state(id);
        if let Some(trigger) = next {
            self.fire(id, trigger);
        }
    }

    /// Runs the actions now, skipping triggers and conditions (« Exécuter »).
    pub fn trigger_now(self: &Arc<Self>, id: &str) -> Result<(), String> {
        if self.entries().get(id).is_none_or(|e| e.compiled.is_none()) {
            return Err(format!("{id} is unknown or has errors"));
        }
        self.fire(id, json!({"platform": null, "id": "manual"}));
        Ok(())
    }
}

fn info_of(id: &str, e: &Entry) -> Info {
    Info {
        id: id.to_owned(),
        alias: e.doc["alias"].as_str().unwrap_or("Sans nom").to_owned(),
        entity_id: e.entity_id.clone(),
        enabled: e.enabled,
        valid: e.compiled.is_some(),
        errors: e.errors.clone(),
        issues: e.issues.clone(),
        last_triggered: e.last_triggered,
        running: e.running.len(),
    }
}

fn trigger_platform(k: &TriggerKind) -> &'static str {
    match k {
        TriggerKind::State { .. } | TriggerKind::EventEntity { .. } => "state",
        TriggerKind::NumericState { .. } => "numeric_state",
        _ => "other",
    }
}

fn trigger_for(k: &TriggerKind) -> Option<crate::compile::Dur> {
    match k {
        TriggerKind::State { for_, .. } | TriggerKind::NumericState { for_, .. } => for_.clone(),
        _ => None,
    }
}

/// HA state-trigger semantics. With `to`/`from`, the *state* must change
/// (attribute-only updates do not fire); without them any change fires.
#[must_use]
pub fn state_trigger_matches(
    c: &StateChanged,
    from: Option<&[String]>,
    to: Option<&[String]>,
    attribute: Option<&str>,
) -> bool {
    let Some(new) = &c.new_state else {
        return false;
    };
    let pick = |s: &rusthome_model::State| match attribute {
        Some(a) => s.attributes.get(a).map(value_as_state).unwrap_or_default(),
        None => s.state.clone(),
    };
    let new_v = pick(new);
    let old_v = c.old_state.as_ref().map(pick);
    if (attribute.is_some() || to.is_some() || from.is_some())
        && old_v.as_deref() == Some(new_v.as_str())
    {
        return false;
    }
    to.is_none_or(|t| t.contains(&new_v))
        && from.is_none_or(|f| old_v.as_ref().is_some_and(|o| f.contains(o)))
}

/// `minutes: /5` fires at second 0 of every 5th minute: smaller unspecified
/// units count as 0 once a larger unit is given (HA semantics).
#[must_use]
pub fn pattern_matches(
    hours: &TimePart,
    minutes: &TimePart,
    seconds: &TimePart,
    (h, m, s): (u32, u32, u32),
) -> bool {
    let ok = |p: &TimePart, v: u32| match p {
        TimePart::Any => true,
        TimePart::Every(n) => *n > 0 && v.is_multiple_of(*n),
        TimePart::At(x) => v == *x,
    };
    let minutes = if matches!(minutes, TimePart::Any) && !matches!(hours, TimePart::Any) {
        &TimePart::At(0)
    } else {
        minutes
    };
    let seconds = if matches!(seconds, TimePart::Any)
        && !(matches!(hours, TimePart::Any) && matches!(minutes, TimePart::Any))
    {
        &TimePart::At(0)
    } else {
        seconds
    };
    ok(hours, h) && ok(minutes, m) && ok(seconds, s)
}
