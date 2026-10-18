//! Zigbee2MQTT integration (cf. docs/architecture.md §5.2).
//!
//! Reads `zigbee2mqtt/bridge/devices` and device topics, maps exposes to
//! entities ([`mapping`]), and provides `switch`/`light`/`number`/`select`
//! services that publish on `zigbee2mqtt/<name>/set`. With a read-only MQTT
//! client every command is refused, so nothing reaches a real device.

pub mod bridge;
pub mod mapping;

use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

use mapping::{Kind, Z2mDevice, Z2mEntity, entities_for, state_of};
use rusthome_core::{Hub, ServiceCall, ServiceError, target_entity_ids};
use rusthome_model::{Context, Device, EntityEntry, EntityId, EntityStatus};
use rusthome_mqtt::{Mqtt, MqttError, Publisher, topic_matches};
use serde_json::{Map, Value, json};

pub const INTEGRATION: &str = "z2m";

#[derive(Debug, Clone)]
struct Binding {
    friendly_name: String,
    entity: Z2mEntity,
}

pub struct Z2m {
    hub: Hub,
    base: String,
    publisher: Arc<dyn Publisher>,
    bindings: RwLock<HashMap<EntityId, Binding>>,
    /// Network management (pairing, renaming, OTA, groups, bindings).
    bridge: bridge::Bridge,
}

/// Summary of a `bridge/devices` synchronisation.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub devices: usize,
    pub entities: usize,
    /// Entities that kept the `entity_id` they had in Home Assistant.
    pub taken_over: usize,
}

impl Z2m {
    #[must_use]
    pub fn new(hub: Hub, base_topic: &str, publisher: Arc<dyn Publisher>) -> Arc<Self> {
        Arc::new(Self {
            hub,
            base: base_topic.trim_end_matches('/').to_owned(),
            publisher,
            bindings: RwLock::default(),
            bridge: bridge::Bridge::default(),
        })
    }

    /// Creates or updates devices and entities from `bridge/devices`.
    pub fn handle_devices(&self, devices: &[Z2mDevice]) -> SyncReport {
        let reg = &self.hub.registries;
        let mut report = SyncReport::default();
        let mut bindings = HashMap::new();
        for d in devices {
            let ents = entities_for(d);
            if ents.is_empty() {
                continue;
            }
            let device_id = format!("z2m-{}", d.ieee_address);
            let mut ha_area = None;
            for e in ents {
                let existing = reg.entity_by_unique_id(&e.unique_id);
                if ha_area.is_none() {
                    ha_area = existing
                        .as_ref()
                        .and_then(|x| x.device_id.as_deref())
                        .and_then(|id| reg.device(id))
                        .and_then(|dev| dev.area_id);
                }
                let entity_id = match &existing {
                    Some(x) => {
                        if x.integration.starts_with("ha:") {
                            report.taken_over += 1;
                        }
                        x.entity_id.to_string()
                    }
                    None => self.free_entity_id(&e.default_entity_id),
                };
                let Some(entity_id) = entity_id_or_skip(&entity_id) else {
                    continue;
                };
                // User choices made in HA (name, area, hidden…) are kept.
                reg.upsert_entity(EntityEntry {
                    uid: existing
                        .as_ref()
                        .map_or_else(|| format!("z2m-{}", e.unique_id), |x| x.uid.clone()),
                    entity_id: entity_id.clone(),
                    integration: INTEGRATION.into(),
                    unique_id: Some(e.unique_id.clone()),
                    device_id: Some(device_id.clone()),
                    name: existing.as_ref().and_then(|x| x.name.clone()),
                    area_id: existing.as_ref().and_then(|x| x.area_id.clone()),
                    labels: existing
                        .as_ref()
                        .map(|x| x.labels.clone())
                        .unwrap_or_default(),
                    icon: existing.as_ref().and_then(|x| x.icon.clone()),
                    hidden: existing.as_ref().map_or(e.hidden, |x| x.hidden),
                    disabled: existing.as_ref().is_some_and(|x| x.disabled),
                    status: EntityStatus::Active,
                });
                bindings.insert(
                    entity_id,
                    Binding {
                        friendly_name: d.friendly_name.clone(),
                        entity: e,
                    },
                );
                report.entities += 1;
            }
            let def = d.definition.as_ref();
            reg.upsert_device(Device {
                id: device_id,
                integration: INTEGRATION.into(),
                // Exact name: it is part of the MQTT topics (`"Interrupteur sans fil "`
                // has a trailing space in the real installation).
                name: d.friendly_name.clone(),
                manufacturer: def
                    .and_then(|x| x.vendor.clone())
                    .or_else(|| d.manufacturer.clone()),
                model: def.and_then(|x| x.model.clone()),
                sw_version: None,
                area_id: ha_area,
                labels: vec![],
                disabled: d.disabled,
            });
            report.devices += 1;
        }
        *self
            .bindings
            .write()
            .unwrap_or_else(PoisonError::into_inner) = bindings;
        report
    }

    /// `domain.object` not used by another integration (suffix `_2`, `_3`…).
    fn free_entity_id(&self, wanted: &str) -> String {
        let taken = |id: &str| {
            EntityId::parse(id)
                .ok()
                .and_then(|i| self.hub.registries.entity_by_entity_id(&i))
                .is_some_and(|e| e.integration != INTEGRATION)
        };
        if !taken(wanted) {
            return wanted.to_owned();
        }
        // Bounded: a household never has 1000 homonyms.
        (2..1000)
            .map(|n| format!("{wanted}_{n}"))
            .find(|c| !taken(c))
            .unwrap_or_else(|| format!("{wanted}_z2m"))
    }

    /// Applies a device payload (`zigbee2mqtt/<name>`).
    pub fn handle_state(&self, friendly_name: &str, payload: &Value) -> usize {
        let Some(obj) = payload.as_object() else {
            return 0;
        };
        let bindings = self
            .bindings
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let mut n = 0;
        for (id, b) in bindings
            .iter()
            .filter(|(_, b)| b.friendly_name == friendly_name)
        {
            let Some(state) = obj
                .get(&b.entity.property)
                .and_then(|v| state_of(&b.entity, v))
            else {
                continue;
            };
            let mut attrs = Map::new();
            let name = self
                .hub
                .registries
                .entity_by_entity_id(id)
                .and_then(|e| e.name)
                .unwrap_or_else(|| b.entity.name.clone());
            attrs.insert("friendly_name".into(), name.into());
            if let Some(u) = &b.entity.unit {
                attrs.insert("unit_of_measurement".into(), u.clone().into());
            }
            if let Some(c) = b.entity.device_class {
                attrs.insert("device_class".into(), c.into());
            }
            if let Kind::Select { options } = &b.entity.kind {
                attrs.insert("options".into(), json!(options));
            }
            self.hub
                .states
                .set(id.clone(), state, attrs, Context::default());
            n += 1;
        }
        n
    }

    /// Registers the command services, routed here for entities owned by `z2m`.
    pub fn register_services(self: &Arc<Self>) {
        for (domain, service) in [
            ("switch", "turn_on"),
            ("switch", "turn_off"),
            ("switch", "toggle"),
            ("light", "turn_on"),
            ("light", "turn_off"),
            ("light", "toggle"),
            ("number", "set_value"),
            ("select", "select_option"),
        ] {
            let me = Arc::clone(self);
            self.hub.services.register(
                INTEGRATION,
                domain,
                service,
                Arc::new(move |call: ServiceCall| {
                    let me = Arc::clone(&me);
                    Box::pin(async move { me.command(&call).await })
                }),
            );
        }
    }

    async fn command(&self, call: &ServiceCall) -> Result<Value, ServiceError> {
        let mut sent = Vec::new();
        for id in target_entity_ids(&call.data)? {
            let binding = self
                .bindings
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&id)
                .cloned()
                .ok_or_else(|| {
                    ServiceError::InvalidData(format!("{id} is not a Zigbee2MQTT entity"))
                })?;
            let value = self.command_value(&id, &binding.entity, &call.service, &call.data)?;
            let topic = format!("{}/{}/set", self.base, binding.friendly_name);
            let payload = json!({ binding.entity.property.clone(): value }).to_string();
            self.publisher
                .publish(topic.clone(), payload.clone())
                .await
                .map_err(|e| match e {
                    MqttError::ReadOnly(_) => ServiceError::Refused(
                        "MQTT is read-only (RUSTHOME_MQTT_READ_ONLY=true): command not sent".into(),
                    ),
                    other => ServiceError::Failed(other.to_string()),
                })?;
            sent.push(json!({ "topic": topic, "payload": payload }));
        }
        Ok(Value::from(sent))
    }

    fn command_value(
        &self,
        id: &EntityId,
        e: &Z2mEntity,
        service: &str,
        data: &Value,
    ) -> Result<Value, ServiceError> {
        let current_on = || self.hub.states.get(id).is_some_and(|s| s.state == "on");
        match (&e.kind, service) {
            (
                Kind::Switch { on, off } | Kind::Light { on, off } | Kind::OptionSwitch { on, off },
                s,
            ) => Ok(match s {
                "turn_off" => off.clone(),
                "toggle" if current_on() => off.clone(),
                "turn_on" | "toggle" => on.clone(),
                other => {
                    return Err(ServiceError::InvalidData(format!(
                        "{other} not supported on {id}"
                    )));
                }
            }),
            (Kind::Number { min, max, .. }, "set_value") => {
                let v = data
                    .get("value")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| ServiceError::InvalidData("value is required".into()))?;
                if min.is_some_and(|m| v < m) || max.is_some_and(|m| v > m) {
                    return Err(ServiceError::InvalidData(format!(
                        "{v} out of range for {id}"
                    )));
                }
                Ok(json!(v))
            }
            (Kind::Select { options }, "select_option") => {
                let o = data
                    .get("option")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ServiceError::InvalidData("option is required".into()))?;
                if !options.iter().any(|x| x == o) {
                    return Err(ServiceError::InvalidData(format!(
                        "{o} is not an option of {id}"
                    )));
                }
                Ok(json!(o))
            }
            (_, other) => Err(ServiceError::InvalidData(format!(
                "{other} not supported on {id}"
            ))),
        }
    }

    /// State of the Zigbee network for the UI (`bridge/info`, devices,
    /// groups, recent events, last payload of each device).
    pub fn bridge_snapshot(&self) -> Value {
        self.bridge.snapshot()
    }

    /// Friendly name of a device from its IEEE address.
    pub fn friendly_name(&self, ieee: &str) -> Option<String> {
        self.bridge.friendly_name(ieee)
    }

    /// Sends a bridge request (`permit_join`, `device/rename`…) and waits for
    /// its answer. Refused before sending when MQTT is read-only.
    pub async fn bridge_request(
        &self,
        kind: &str,
        payload: Value,
    ) -> Result<Value, bridge::BridgeError> {
        let (sub, body, rx) = self.bridge.prepare(kind, payload)?;
        let topic = format!("{}/bridge/{sub}", self.base);
        if let Err(e) = self.publisher.publish(topic, body.clone()).await {
            self.bridge.cancel(&body);
            return Err(bridge::publish_error(e));
        }
        bridge::answer(rx, std::time::Duration::from_secs(30)).await
    }

    /// Subscribes and processes messages forever.
    pub async fn run(self: Arc<Self>, mqtt: Mqtt) {
        let mut rx = mqtt.messages();
        let devices_topic = format!("{}/bridge/devices", self.base);
        let device_filter = format!("{}/+", self.base);
        let bridge_prefix = format!("{}/bridge/", self.base);
        let bridge_topics: Vec<String> = ["info", "groups", "event", "response/#"]
            .iter()
            .map(|t| format!("{bridge_prefix}{t}"))
            .collect();
        for filter in [&devices_topic, &device_filter]
            .into_iter()
            .chain(&bridge_topics)
        {
            if let Err(e) = mqtt.subscribe(filter).await {
                tracing::error!(error = %e, filter, "Zigbee2MQTT: subscribe failed");
            }
        }
        loop {
            let msg = match rx.recv().await {
                Ok(m) => m,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(missed = n, "Zigbee2MQTT: lagging behind MQTT");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            };
            if msg.topic == devices_topic {
                if let Ok(raw) = serde_json::from_slice::<Value>(&msg.payload) {
                    self.bridge
                        .handle("devices", &raw, self.hub.clock.now().timestamp_millis());
                }
                match serde_json::from_slice::<Vec<Z2mDevice>>(&msg.payload) {
                    Ok(devices) => {
                        let report = self.handle_devices(&devices);
                        tracing::info!(?report, "Zigbee2MQTT: devices synchronised");
                    }
                    Err(e) => tracing::warn!(error = %e, "Zigbee2MQTT: unreadable bridge/devices"),
                }
            } else if let Some(sub) = msg.topic.strip_prefix(&bridge_prefix) {
                if let Ok(payload) = serde_json::from_slice::<Value>(&msg.payload) {
                    self.bridge
                        .handle(sub, &payload, self.hub.clock.now().timestamp_millis());
                }
            } else if topic_matches(&device_filter, &msg.topic)
                && !msg.topic.starts_with(&format!("{}/bridge", self.base))
            {
                let name = &msg.topic[self.base.len() + 1..];
                if let Ok(payload) = serde_json::from_slice::<Value>(&msg.payload) {
                    self.bridge.record_payload(name, &payload);
                    self.handle_state(name, &payload);
                }
            }
        }
    }
}

fn entity_id_or_skip(raw: &str) -> Option<EntityId> {
    match EntityId::parse(raw) {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::warn!(error = %e, "Zigbee2MQTT: entity skipped");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use futures_util::future::BoxFuture;
    use rusthome_core::SystemClock;

    #[derive(Default)]
    struct Recorder {
        read_only: bool,
        sent: Mutex<Vec<(String, String)>>,
    }

    impl Publisher for Recorder {
        fn publish(&self, topic: String, payload: String) -> BoxFuture<'_, Result<(), MqttError>> {
            Box::pin(async move {
                if self.read_only {
                    return Err(MqttError::ReadOnly(topic));
                }
                self.sent.lock().unwrap().push((topic, payload));
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn bridge_requests_are_refused_when_read_only() {
        let hub = Hub::new(Arc::new(SystemClock));
        let ro = Arc::new(Recorder {
            read_only: true,
            ..Recorder::default()
        });
        let z = Z2m::new(hub, "zigbee2mqtt", ro.clone());
        assert_eq!(
            z.bridge_request("permit_join", json!({ "time": 60 })).await,
            Err(bridge::BridgeError::ReadOnly)
        );
        assert!(ro.sent.lock().unwrap().is_empty(), "nothing sent");
    }

    #[tokio::test]
    async fn bridge_requests_go_to_the_bridge_topic() {
        let hub = Hub::new(Arc::new(SystemClock));
        let rec = Arc::new(Recorder::default());
        let z = Z2m::new(hub, "zigbee2mqtt", rec.clone());
        let z2 = Arc::clone(&z);
        let task = tokio::spawn(async move {
            z2.bridge_request("device/rename", json!({ "from": "a", "to": "b" }))
                .await
        });
        // answer the request as Zigbee2MQTT would
        let (topic, payload) = loop {
            if let Some(m) = rec.sent.lock().unwrap().first().cloned() {
                break m;
            }
            tokio::task::yield_now().await;
        };
        assert_eq!(topic, "zigbee2mqtt/bridge/request/device/rename");
        let tx = serde_json::from_str::<Value>(&payload).unwrap()["transaction"].clone();
        z.bridge.handle(
            "response/device/rename",
            &json!({ "status": "ok", "data": { "from": "a", "to": "b" }, "transaction": tx }),
            0,
        );
        assert_eq!(task.await.unwrap().unwrap()["to"], "b");
    }

    fn devices() -> Vec<Z2mDevice> {
        serde_json::from_str(include_str!("../tests/fixtures/bridge_devices.json")).unwrap()
    }

    fn hub_with_ha_mirror() -> Hub {
        let hub = Hub::new(Arc::new(SystemClock));
        // What the HA bridge imported for the plug's power sensor and its device.
        hub.registries.upsert_device(Device {
            id: "ha-dev-1".into(),
            integration: "ha:mqtt".into(),
            name: "Prise lampadaire".into(),
            manufacturer: None,
            model: None,
            sw_version: None,
            area_id: Some("salon".into()),
            labels: vec![],
            disabled: false,
        });
        hub.registries.upsert_entity(EntityEntry {
            uid: "ha-uid-1".into(),
            entity_id: EntityId::parse("sensor.0x54ef4410003a6d55_power").unwrap(),
            integration: "ha:mqtt".into(),
            unique_id: Some("0x54ef4410003a6d55_power_zigbee2mqtt".into()),
            device_id: Some("ha-dev-1".into()),
            name: Some("Puissance lampadaire".into()),
            area_id: None,
            labels: vec![],
            icon: None,
            hidden: false,
            disabled: false,
            status: EntityStatus::Active,
        });
        hub
    }

    #[test]
    fn takes_over_ha_entities_and_keeps_their_ids() {
        let hub = hub_with_ha_mirror();
        let z2m = Z2m::new(hub.clone(), "zigbee2mqtt", Arc::new(Recorder::default()));
        let report = z2m.handle_devices(&devices());
        assert_eq!(report.devices, 17);
        assert_eq!(report.taken_over, 1);

        let power = hub
            .registries
            .entity_by_entity_id(&EntityId::parse("sensor.0x54ef4410003a6d55_power").unwrap())
            .unwrap();
        assert_eq!(power.integration, "z2m");
        assert_eq!(power.uid, "ha-uid-1", "same registry entry");
        assert_eq!(
            power.name.as_deref(),
            Some("Puissance lampadaire"),
            "user name kept"
        );
        let device = hub.registries.device("z2m-0x54ef4410003a6d55").unwrap();
        assert_eq!(
            device.area_id.as_deref(),
            Some("salon"),
            "area inherited from the HA device"
        );
        // Unknown to HA: generated id
        assert!(
            hub.registries
                .entity_by_entity_id(&EntityId::parse("switch.prise_lampadaire").unwrap())
                .is_some()
        );
    }

    #[test]
    fn applies_real_payloads() {
        let hub = hub_with_ha_mirror();
        let z2m = Z2m::new(hub.clone(), "zigbee2mqtt", Arc::new(Recorder::default()));
        z2m.handle_devices(&devices());
        let samples: Map<String, Value> =
            serde_json::from_str(include_str!("../tests/fixtures/states.json")).unwrap();
        let payload = &samples["zigbee2mqtt/Radiateur salon"];
        // state, power, energy, device_temperature, linkquality (the other fields are null)
        assert_eq!(z2m.handle_state("Radiateur salon", payload), 5);
        let state = hub
            .states
            .get(&EntityId::parse("switch.radiateur_salon").unwrap())
            .unwrap();
        assert_eq!(state.state, "off");
        let energy = hub
            .states
            .get(&EntityId::parse("sensor.radiateur_salon_energy").unwrap())
            .unwrap();
        assert_eq!(energy.attributes["unit_of_measurement"], "kWh");
    }

    #[tokio::test]
    async fn commands_publish_on_set_topic() {
        let hub = hub_with_ha_mirror();
        let rec = Arc::new(Recorder::default());
        let z2m = Z2m::new(hub.clone(), "zigbee2mqtt", rec.clone());
        z2m.handle_devices(&devices());
        z2m.register_services();
        let call = |domain: &str, service: &str, data: Value| ServiceCall {
            domain: domain.into(),
            service: service.into(),
            data,
            context: Context::default(),
        };
        hub.call_service(call(
            "switch",
            "turn_on",
            json!({"entity_id": "switch.prise_lampadaire"}),
        ))
        .await
        .unwrap();
        hub.call_service(call(
            "number",
            "set_value",
            json!({"entity_id": "number.prise_lampadaire_overload_protection", "value": 2000}),
        ))
        .await
        .unwrap();
        let sent = rec.sent.lock().unwrap().clone();
        assert_eq!(
            sent[0],
            (
                "zigbee2mqtt/Prise lampadaire/set".into(),
                r#"{"state":"ON"}"#.into()
            )
        );
        assert_eq!(sent[1].1, r#"{"overload_protection":2000.0}"#);
        assert!(
            hub.call_service(call(
                "number",
                "set_value",
                json!({"entity_id": "number.prise_lampadaire_overload_protection", "value": 99999})
            ))
            .await
            .is_err(),
            "out of range"
        );
    }

    #[tokio::test]
    async fn read_only_mqtt_refuses_commands_clearly() {
        let hub = hub_with_ha_mirror();
        let z2m = Z2m::new(
            hub.clone(),
            "zigbee2mqtt",
            Arc::new(Recorder {
                read_only: true,
                ..Recorder::default()
            }),
        );
        z2m.handle_devices(&devices());
        z2m.register_services();
        let err = hub
            .call_service(ServiceCall {
                domain: "switch".into(),
                service: "turn_on".into(),
                data: json!({"entity_id": "switch.radiateur_salon"}),
                context: Context::default(),
            })
            .await
            .unwrap_err();
        assert!(matches!(err, ServiceError::Refused(m) if m.contains("read-only")));
    }
}
