//! End-to-end test of the bridge against a scripted fake Home Assistant
//! (loopback socket, no real HA involved).

// Test harness: panicking on unexpected input is the desired behaviour.
#![allow(clippy::unwrap_used, clippy::needless_pass_by_value)]

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use rusthome_core::{Hub, SystemClock};
use rusthome_ha_bridge::{Command, Connection, apply};
use rusthome_import::import_registries;
use rusthome_model::EntityId;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

fn state(entity_id: &str, value: &str, attrs: Value) -> Value {
    json!({"entity_id": entity_id, "state": value, "attributes": attrs,
           "last_changed": "2026-10-17T10:00:00+00:00", "last_updated": "2026-10-17T10:00:00+00:00",
           "context": {"id": "c1"}})
}

/// Fake HA: answers read commands, asserts nothing else is ever sent.
async fn fake_ha(listener: TcpListener) -> Vec<String> {
    let (tcp, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
    let allowed: Vec<&str> = Command::ALL.iter().map(|c| c.message_type()).collect();
    let mut seen = Vec::new();

    ws.send(Message::text(json!({"type": "auth_required"}).to_string()))
        .await
        .unwrap();
    let auth: Value =
        serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(auth["type"], "auth");
    assert_eq!(auth["access_token"], "secret-token");
    ws.send(Message::text(json!({"type": "auth_ok"}).to_string()))
        .await
        .unwrap();

    while let Some(Ok(msg)) = ws.next().await {
        let Message::Text(text) = msg else { continue };
        let req: Value = serde_json::from_str(text.as_str()).unwrap();
        let t = req["type"].as_str().unwrap().to_owned();
        assert!(
            allowed.contains(&t.as_str()),
            "bridge sent a non read-only command: {t}"
        );
        seen.push(t.clone());
        let id = req["id"].clone();
        let result = match t.as_str() {
            "get_states" => json!([
                state("light.salon", "off", json!({})),
                state(
                    "media_player.emby_ghost",
                    "unavailable",
                    json!({"restored": true})
                ),
            ]),
            "config/floor_registry/list" => json!([{"floor_id": "rdc", "name": "RDC", "level": 0}]),
            "config/area_registry/list" => {
                json!([{"area_id": "salon", "name": "Salon", "floor_id": "rdc"}])
            }
            "config/label_registry/list" => json!([]),
            "config/device_registry/list" => {
                json!([{"id": "dev1", "name": "Hue salon", "area_id": "salon"}])
            }
            "config/entity_registry/list" => json!([
                {"id": "u1", "entity_id": "light.salon", "platform": "hue", "device_id": "dev1"},
                {"id": "u2", "entity_id": "media_player.emby_ghost", "platform": "emby"}
            ]),
            "subscribe_events" => {
                ws.send(Message::text(
                    json!({"id": id, "type": "result", "success": true, "result": null})
                        .to_string(),
                ))
                .await
                .unwrap();
                let ev = json!({"id": id, "type": "event", "event": {"event_type": "state_changed", "data": {
                    "entity_id": "light.salon",
                    "old_state": state("light.salon", "off", json!({})),
                    "new_state": state("light.salon", "on", json!({"brightness": 200}))}}});
                ws.send(Message::text(ev.to_string())).await.unwrap();
                ws.close(None).await.ok();
                break;
            }
            other => panic!("unexpected {other}"),
        };
        ws.send(Message::text(
            json!({"id": id, "type": "result", "success": true, "result": result}).to_string(),
        ))
        .await
        .unwrap();
    }
    seen
}

#[tokio::test]
async fn mirrors_registries_states_and_changes_read_only() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = url::Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let server = tokio::spawn(fake_ha(listener));

    let hub = Hub::new(Arc::new(SystemClock));
    let mut conn = Connection::connect(&url, "secret-token").await.unwrap();
    let snapshot = conn.fetch_snapshot().await.unwrap();
    let imported = import_registries(&snapshot);
    let report = apply(&hub, &imported, snapshot.states);

    assert_eq!(
        (
            report.entities,
            report.devices,
            report.states,
            report.skipped
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(hub.registries.areas()[0].floor_id.as_deref(), Some("rdc"));
    let salon = EntityId::parse("light.salon").unwrap();
    assert_eq!(hub.states.get(&salon).unwrap().state, "off");
    assert!(
        hub.states
            .get(&EntityId::parse("media_player.emby_ghost").unwrap())
            .is_none()
    );

    let events = conn.subscribe_state_changed().await.unwrap();
    let change = events.next_state_changed().await.unwrap().unwrap();
    hub.states.set_external(change.new_state.unwrap());
    assert_eq!(hub.states.get(&salon).unwrap().state, "on");
    assert!(
        events.next_state_changed().await.unwrap().is_none(),
        "closed stream ends cleanly"
    );

    let seen = server.await.unwrap();
    assert!(seen.contains(&"subscribe_events".to_owned()));
}

#[tokio::test]
async fn refused_token_is_reported() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = url::Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
        ws.send(Message::text(json!({"type": "auth_required"}).to_string()))
            .await
            .unwrap();
        let _ = ws.next().await;
        ws.send(Message::text(
            json!({"type": "auth_invalid", "message": "bad"}).to_string(),
        ))
        .await
        .unwrap();
    });
    let err = Connection::connect(&url, "wrong").await.err().unwrap();
    assert!(matches!(err, rusthome_ha_bridge::BridgeError::AuthInvalid));
}

#[test]
fn natively_owned_entities_are_not_overwritten() {
    use rusthome_import::{HaEntity, HaSnapshot};
    use rusthome_model::{Context, EntityEntry, EntityStatus, State};

    let hub = Hub::new(Arc::new(SystemClock));
    let salon = EntityId::parse("switch.prise").unwrap();
    // Zigbee2MQTT already owns the entity…
    hub.registries.upsert_entity(EntityEntry {
        uid: "z2m-1".into(),
        entity_id: salon.clone(),
        integration: "z2m".into(),
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
    hub.states.set(
        salon.clone(),
        "on",
        serde_json::Map::new(),
        Context::default(),
    );
    // …and HA reports an older value for it.
    let snapshot = HaSnapshot {
        entities: vec![HaEntity {
            id: "ha-1".into(),
            entity_id: "switch.prise".into(),
            platform: "mqtt".into(),
            ..HaEntity::default()
        }],
        ..HaSnapshot::default()
    };
    let stale: State = serde_json::from_value(state("switch.prise", "off", json!({}))).unwrap();
    let report = apply(&hub, &import_registries(&snapshot), vec![stale]);
    assert_eq!((report.owned_natively, report.states), (1, 0));
    assert_eq!(hub.states.get(&salon).unwrap().state, "on");
    assert_eq!(
        hub.registries
            .entity_by_entity_id(&salon)
            .unwrap()
            .integration,
        "z2m"
    );
}
