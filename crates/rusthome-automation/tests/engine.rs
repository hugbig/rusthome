//! Engine behaviour with virtual time (no real sleeps).

// Test harness: panicking on unexpected input is the desired behaviour.
#![allow(clippy::unwrap_used, clippy::too_many_lines)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{TimeZone, Utc};
use rusthome_automation::Engine;
use rusthome_automation::exec::Outcome;
use rusthome_core::{Event, Hub, ManualClock, ServiceCall, ServiceError};
use rusthome_model::{Context, EntityEntry, EntityId, EntityStatus};
use serde_json::{Map, Value, json};

struct World {
    hub: Hub,
    clock: Arc<ManualClock>,
    engine: Arc<Engine>,
    calls: Arc<Mutex<Vec<(String, Value)>>>,
}

fn id(s: &str) -> EntityId {
    EntityId::parse(s).unwrap()
}

fn world() -> World {
    let clock = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 10, 17, 10, 0, 0).unwrap(),
    ));
    let hub = Hub::new(clock.clone());
    let calls = Arc::new(Mutex::new(Vec::new()));
    // A fake integration owning a few entities and recording its calls.
    for e in ["switch.sirene", "light.salon", "switch.prise"] {
        hub.registries.upsert_entity(EntityEntry {
            uid: e.into(),
            entity_id: id(e),
            integration: "test".into(),
            unique_id: None,
            device_id: None,
            name: None,
            area_id: None,
            labels: vec![],
            icon: None,
            hidden: false,
            disabled: false,
            status: EntityStatus::Active,
        });
    }
    for (domain, service) in [
        ("switch", "turn_on"),
        ("switch", "turn_off"),
        ("light", "turn_on"),
        ("light", "turn_off"),
    ] {
        let calls = calls.clone();
        hub.services.register(
            "test",
            domain,
            service,
            Arc::new(move |c: ServiceCall| {
                let calls = calls.clone();
                Box::pin(async move {
                    calls
                        .lock()
                        .unwrap()
                        .push((format!("{}.{}", c.domain, c.service), c.data));
                    Ok::<_, ServiceError>(Value::Null)
                })
            }),
        );
    }
    let engine = Engine::new(hub.clone(), chrono_tz::Europe::Paris);
    World {
        hub,
        clock,
        engine,
        calls,
    }
}

fn set(w: &World, entity: &str, state: &str) {
    w.hub
        .states
        .set(id(entity), state, Map::new(), Context::default());
}

/// Lets spawned runs progress (virtual time).
async fn settle() {
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
}

async fn feed(w: &World, rx: &mut tokio::sync::broadcast::Receiver<Arc<Event>>) {
    while let Ok(ev) = rx.try_recv() {
        w.engine.handle_event(&ev);
    }
    settle().await;
}

fn calls(w: &World) -> Vec<String> {
    w.calls
        .lock()
        .unwrap()
        .iter()
        .map(|(s, _)| s.clone())
        .collect()
}

#[tokio::test(start_paused = true)]
async fn state_trigger_condition_and_service() {
    let w = world();
    let mut rx = w.hub.bus.subscribe();
    set(&w, "input_boolean.presence", "off");
    w.engine.upsert(
        json!({"id": "a1", "alias": "Lumière", "mode": "single",
               "triggers": [{"trigger": "state", "entity_id": "binary_sensor.porte", "to": "on"}],
               "conditions": [{"condition": "state", "entity_id": "input_boolean.presence", "state": "off"}],
               "actions": [{"action": "light.turn_on", "target": {"entity_id": "light.salon"}, "data": {"brightness": "{{ 100 + 55 }}"}}]}),
        id("automation.lumiere"),
        true,
        vec![],
    );
    feed(&w, &mut rx).await;
    set(&w, "binary_sensor.porte", "on");
    feed(&w, &mut rx).await;
    assert_eq!(calls(&w), ["light.turn_on"]);
    // Clone: borrowing would keep the lock for the rest of the test.
    let data = w.calls.lock().unwrap()[0].1.clone();
    assert_eq!(
        data["brightness"], 155,
        "template rendered to a native number"
    );
    assert_eq!(
        data["entity_id"],
        json!(["light.salon"]),
        "targets normalized to a list by the service router"
    );

    let trace = &w.engine.traces("a1")[0];
    assert_eq!(trace.outcome, Some(Outcome::Finished));
    assert_ne!(trace.steps, [] as [rusthome_automation::exec::TraceStep; 0]);
    let entity = w.hub.states.get(&id("automation.lumiere")).unwrap();
    assert_eq!(entity.state, "on");
    assert!(entity.attributes["last_triggered"].is_string());

    // Condition false: no call, trace says so.
    set(&w, "input_boolean.presence", "on");
    set(&w, "binary_sensor.porte", "off");
    set(&w, "binary_sensor.porte", "on");
    feed(&w, &mut rx).await;
    assert_eq!(calls(&w).len(), 1);
    assert_eq!(
        w.engine.traces("a1")[0].outcome,
        Some(Outcome::ConditionFailed)
    );
}

#[tokio::test(start_paused = true)]
async fn disabled_automation_never_fires() {
    let w = world();
    let mut rx = w.hub.bus.subscribe();
    let info = w.engine.upsert(
        json!({"id": "a2", "alias": "Importée", "triggers": [{"trigger": "state", "entity_id": "switch.prise"}],
               "actions": [{"action": "light.turn_on", "target": {"entity_id": "light.salon"}}]}),
        id("automation.importee"),
        false,
        vec![],
    );
    assert!(!info.enabled);
    set(&w, "switch.prise", "on");
    feed(&w, &mut rx).await;
    assert_eq!(calls(&w), [] as [std::string::String; 0]);
    assert_eq!(
        w.hub.states.get(&id("automation.importee")).unwrap().state,
        "off"
    );
}

#[tokio::test(start_paused = true)]
async fn invalid_automation_is_kept_but_cannot_be_enabled() {
    let w = world();
    let info = w.engine.upsert(
        json!({"id": "bad", "alias": "Cassée", "triggers": [], "actions": []}),
        id("automation.cassee"),
        true,
        vec![],
    );
    assert!(!info.valid && !info.enabled);
    assert!(w.engine.set_enabled("bad", true).is_err());
    assert!(w.engine.document("bad").is_some(), "nothing is lost");
}

#[tokio::test(start_paused = true)]
async fn modes_single_restart_queued() {
    for (mode, expected_runs) in [("single", 1), ("restart", 1), ("queued", 2)] {
        let w = world();
        let mut rx = w.hub.bus.subscribe();
        w.engine.upsert(
            json!({"id": "m", "alias": "Mode", "mode": mode,
                   "triggers": [{"trigger": "state", "entity_id": "sensor.x"}],
                   "actions": [{"delay": 10}, {"action": "switch.turn_on", "target": {"entity_id": "switch.prise"}}]}),
            id("automation.mode"),
            true,
            vec![],
        );
        set(&w, "sensor.x", "1");
        feed(&w, &mut rx).await;
        tokio::time::advance(Duration::from_secs(3)).await;
        set(&w, "sensor.x", "2"); // second trigger while the first run waits
        feed(&w, &mut rx).await;
        for _ in 0..30 {
            tokio::time::advance(Duration::from_secs(1)).await;
            settle().await;
        }
        assert_eq!(calls(&w).len(), expected_runs, "mode {mode}");
        if mode == "restart" {
            assert!(
                w.engine.traces("m").len() <= 1,
                "first run was cancelled, not traced as finished"
            );
        }
    }
}

#[tokio::test(start_paused = true)]
async fn state_for_fires_only_if_state_holds() {
    let w = world();
    let mut rx = w.hub.bus.subscribe();
    w.engine.upsert(
        json!({"id": "f", "alias": "Absence", "triggers": [{"trigger": "state", "entity_id": "person.hugo", "to": "not_home", "for": {"hours": 1}}],
               "actions": [{"action": "switch.turn_on", "target": {"entity_id": "switch.sirene"}}]}),
        id("automation.absence"),
        true,
        vec![],
    );
    set(&w, "person.hugo", "home");
    set(&w, "person.hugo", "not_home");
    feed(&w, &mut rx).await;
    tokio::time::advance(Duration::from_mins(30)).await;
    set(&w, "person.hugo", "home"); // back after 30 min: cancelled
    feed(&w, &mut rx).await;
    tokio::time::advance(Duration::from_secs(3600)).await;
    settle().await;
    assert_eq!(calls(&w), [] as [std::string::String; 0]);

    w.clock.advance(chrono::Duration::seconds(1)); // distinct last_changed
    set(&w, "person.hugo", "not_home");
    feed(&w, &mut rx).await;
    tokio::time::advance(Duration::from_secs(3601)).await;
    settle().await;
    assert_eq!(calls(&w), ["switch.turn_on"]);
}

#[tokio::test(start_paused = true)]
async fn time_pattern_fires_once_per_matching_second() {
    let w = world();
    w.engine.upsert(
        json!({"id": "t", "alias": "Horloge", "triggers": [{"trigger": "time_pattern", "minutes": "/5"}],
               "actions": [{"action": "switch.turn_on", "target": {"entity_id": "switch.prise"}}]}),
        id("automation.horloge"),
        true,
        vec![],
    );
    let at = |h, m, s| Utc.with_ymd_and_hms(2026, 10, 17, h, m, s).unwrap();
    for t in [
        at(10, 5, 0),  // 12:05:00 Paris
        at(10, 5, 0),  // same second: deduplicated
        at(10, 5, 1),  // second ≠ 0
        at(10, 6, 0),  // minute not multiple of 5
        at(10, 10, 0), // fires
    ] {
        w.engine.tick(t);
        settle().await; // ticks are seconds apart in reality
    }
    assert_eq!(calls(&w).len(), 2);
}

#[tokio::test(start_paused = true)]
async fn badge_tag_and_mqtt_button() {
    let w = world();
    w.engine.upsert(
        json!({"id": "b", "alias": "Badge", "triggers": [{"trigger": "tag", "tag_id": ["AA-00-00-01"]}],
               "actions": [{"action": "switch.turn_off", "target": {"entity_id": "switch.sirene"}}]}),
        id("automation.badge"),
        true,
        vec![],
    );
    w.engine.upsert(
        json!({"id": "z", "alias": "Bouton", "triggers": [{"trigger": "mqtt", "topic": "zigbee2mqtt/Interrupteur sans fil /action", "payload": "single_left"}],
               "actions": [{"action": "light.turn_on", "target": {"entity_id": "light.salon"}}]}),
        id("automation.bouton"),
        true,
        vec![],
    );
    w.engine.handle_event(&Event::Custom {
        event_type: "tag_scanned".into(),
        data: json!({"tag_id": "ZZ-99"}),
    });
    w.engine.handle_event(&Event::Custom {
        event_type: "tag_scanned".into(),
        data: json!({"tag_id": "AA-00-00-01"}),
    });
    w.engine
        .handle_mqtt("zigbee2mqtt/Interrupteur sans fil /action", "double_left");
    w.engine
        .handle_mqtt("zigbee2mqtt/Interrupteur sans fil /action", "single_left");
    settle().await;
    assert_eq!(calls(&w), ["switch.turn_off", "light.turn_on"]);
    assert_eq!(
        w.engine.mqtt_topics(),
        ["zigbee2mqtt/Interrupteur sans fil /action"]
    );
}

#[tokio::test(start_paused = true)]
async fn continue_on_error_and_error_stop() {
    let w = world();
    w.engine.upsert(
        json!({"id": "e", "alias": "Notif", "triggers": [{"trigger": "event", "event_type": "go"}],
               "actions": [
                   {"action": "notify.mobile_app_one_plus_12", "data": {"message": "x"}, "continue_on_error": true},
                   {"action": "switch.turn_on", "target": {"entity_id": "switch.prise"}},
                   {"action": "notify.absent"},
                   {"action": "light.turn_on", "target": {"entity_id": "light.salon"}}
               ]}),
        id("automation.notif"),
        true,
        vec![],
    );
    w.engine.handle_event(&Event::Custom {
        event_type: "go".into(),
        data: json!({}),
    });
    settle().await;
    // The missing notifier is skipped once, then stops the run the second time.
    assert_eq!(calls(&w), ["switch.turn_on"]);
    let t = &w.engine.traces("e")[0];
    assert_eq!(t.outcome, Some(Outcome::Error));
    assert!(t.error.as_ref().unwrap().contains("notify.absent"));
}

/// Simplified alarm chain (cf. inventaire/02 §9): sirens every 8 s while the
/// alarm is triggered, in parallel with clips; disarming ends the loop.
#[tokio::test(start_paused = true)]
async fn alarm_siren_loop_stops_on_disarm() {
    let w = world();
    let mut rx = w.hub.bus.subscribe();
    set(&w, "alarm_control_panel.alarmo", "armed_away");
    w.engine.upsert(
        json!({"id": "intrusion", "alias": "Alarmo — Intrusion", "mode": "parallel", "max": 10,
               "triggers": [{"trigger": "state", "entity_id": "alarm_control_panel.alarmo", "to": "triggered"}],
               "actions": [{"parallel": [
                   {"repeat": {"while": [{"condition": "state", "entity_id": "alarm_control_panel.alarmo", "state": "triggered"}],
                               "sequence": [{"action": "switch.turn_on", "target": {"entity_id": "switch.sirene"}, "continue_on_error": true},
                                            {"delay": 8}]}},
                   {"sequence": [{"action": "light.turn_on", "target": {"entity_id": "light.salon"}}]}
               ]}]}),
        id("automation.intrusion"),
        true,
        vec![],
    );
    feed(&w, &mut rx).await;
    set(&w, "alarm_control_panel.alarmo", "triggered");
    feed(&w, &mut rx).await;
    for _ in 0..20 {
        tokio::time::advance(Duration::from_secs(1)).await;
        settle().await;
    }
    let sirens = calls(&w).iter().filter(|c| *c == "switch.turn_on").count();
    assert_eq!(sirens, 3, "t=0, 8, 16 s");
    assert!(calls(&w).contains(&"light.turn_on".to_owned()));

    set(&w, "alarm_control_panel.alarmo", "disarmed");
    for _ in 0..30 {
        tokio::time::advance(Duration::from_secs(1)).await;
        settle().await;
    }
    assert_eq!(
        calls(&w).iter().filter(|c| *c == "switch.turn_on").count(),
        3,
        "loop ended"
    );
    assert_eq!(
        w.engine.traces("intrusion")[0].outcome,
        Some(Outcome::Finished)
    );
}

fn set_attrs(w: &World, entity: &str, state: &str, attrs: Value) {
    let Value::Object(m) = attrs else { panic!() };
    w.hub.states.set(id(entity), state, m, Context::default());
}

/// Code review n° 11 (alarm): GPS updates of `person.hugo` (attributes only)
/// must not reset « armement après 1 h d'absence ».
#[tokio::test(start_paused = true)]
async fn attribute_updates_keep_a_for_timer() {
    let w = world();
    let mut rx = w.hub.bus.subscribe();
    w.engine.upsert(
        json!({"id": "f", "alias": "Absence", "triggers": [{"trigger": "state", "entity_id": "person.hugo", "to": "not_home", "for": {"hours": 1}}],
               "actions": [{"action": "switch.turn_on", "target": {"entity_id": "switch.sirene"}}]}),
        id("automation.absence"),
        true,
        vec![],
    );
    set(&w, "person.hugo", "home");
    set(&w, "person.hugo", "not_home");
    feed(&w, &mut rx).await;
    for minute in 1..=50 {
        tokio::time::advance(Duration::from_mins(1)).await;
        set_attrs(
            &w,
            "person.hugo",
            "not_home",
            json!({"latitude": 48.0 + f64::from(minute) / 1000.0}),
        );
        feed(&w, &mut rx).await;
    }
    tokio::time::advance(Duration::from_mins(11)).await;
    settle().await;
    assert_eq!(
        calls(&w),
        ["switch.turn_on"],
        "armed after 1 h despite GPS updates"
    );
}

/// Code review n° 12: one `for` timer per entity of the trigger.
#[tokio::test(start_paused = true)]
async fn for_timers_are_per_entity() {
    let w = world();
    let mut rx = w.hub.bus.subscribe();
    w.engine.upsert(
        json!({"id": "p", "alias": "Portes", "mode": "parallel",
               "triggers": [{"trigger": "state", "entity_id": ["binary_sensor.porte_a", "binary_sensor.porte_b"], "to": "on", "for": {"minutes": 5}}],
               "actions": [{"action": "light.turn_on", "target": {"entity_id": "light.salon"}}]}),
        id("automation.portes"),
        true,
        vec![],
    );
    set(&w, "binary_sensor.porte_a", "off");
    set(&w, "binary_sensor.porte_b", "off");
    feed(&w, &mut rx).await;
    set(&w, "binary_sensor.porte_a", "on");
    feed(&w, &mut rx).await;
    tokio::time::advance(Duration::from_mins(2)).await;
    w.clock.advance(chrono::Duration::seconds(1));
    set(&w, "binary_sensor.porte_b", "on"); // must not restart porte_a's countdown
    feed(&w, &mut rx).await;
    tokio::time::advance(Duration::from_secs(181)).await;
    settle().await;
    assert_eq!(calls(&w), ["light.turn_on"], "porte_a after 5 min");
    tokio::time::advance(Duration::from_mins(2)).await;
    settle().await;
    assert_eq!(calls(&w).len(), 2, "then porte_b");
}

/// Code review n° 15 (alarm): disabling stops the siren loop and the queue.
#[tokio::test(start_paused = true)]
async fn disabling_stops_running_executions() {
    let w = world();
    let mut rx = w.hub.bus.subscribe();
    set(&w, "alarm_control_panel.alarmo", "armed_away");
    w.engine.upsert(
        json!({"id": "intrusion", "alias": "Intrusion", "mode": "queued",
               "triggers": [{"trigger": "state", "entity_id": "alarm_control_panel.alarmo", "to": "triggered"}],
               "actions": [{"repeat": {"while": [{"condition": "state", "entity_id": "alarm_control_panel.alarmo", "state": "triggered"}],
                                       "sequence": [{"action": "switch.turn_on", "target": {"entity_id": "switch.sirene"}}, {"delay": 8}]}}]}),
        id("automation.intrusion"),
        true,
        vec![],
    );
    feed(&w, &mut rx).await;
    set(&w, "alarm_control_panel.alarmo", "triggered");
    feed(&w, &mut rx).await;
    tokio::time::advance(Duration::from_secs(9)).await;
    settle().await;
    assert_eq!(calls(&w).len(), 2);
    w.engine.fire("intrusion", json!({"id": "queued"})); // queued behind the loop
    w.engine.set_enabled("intrusion", false).unwrap();
    for _ in 0..30 {
        tokio::time::advance(Duration::from_secs(1)).await;
        settle().await;
    }
    assert_eq!(calls(&w).len(), 2, "loop and queue stopped");
    w.engine.fire("intrusion", json!({"id": "late"}));
    settle().await;
    assert_eq!(calls(&w).len(), 2, "a disabled automation does not run");
}

/// Code review n° 13 and 14: entity thresholds fire on the crossing only,
/// and an unavailable threshold closes the range.
#[tokio::test(start_paused = true)]
async fn numeric_entity_thresholds() {
    let w = world();
    let mut rx = w.hub.bus.subscribe();
    set(&w, "input_number.seuil", "25");
    w.engine.upsert(
        json!({"id": "n", "alias": "Chaud",
               "triggers": [{"trigger": "numeric_state", "entity_id": "sensor.temp", "above": "input_number.seuil"}],
               "actions": [{"action": "switch.turn_on", "target": {"entity_id": "switch.prise"}}]}),
        id("automation.chaud"),
        true,
        vec![],
    );
    for v in ["20", "26", "26.5", "27"] {
        set(&w, "sensor.temp", v);
        feed(&w, &mut rx).await;
    }
    assert_eq!(calls(&w), ["switch.turn_on"], "once, on the crossing");
    set(&w, "sensor.temp", "20");
    set(&w, "input_number.seuil", "unavailable");
    feed(&w, &mut rx).await;
    set(&w, "sensor.temp", "30");
    feed(&w, &mut rx).await;
    assert_eq!(calls(&w).len(), 1, "no bound, no range");
}
