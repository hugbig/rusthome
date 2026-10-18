//! Zigbee network management through the Zigbee2MQTT bridge API
//! (`<base>/bridge/request/<kind>` → `<base>/bridge/response/<kind>`):
//! pairing, renaming, OTA, removal, groups and bindings. State of the bridge
//! (`bridge/info`, `bridge/groups`, `bridge/event`) is kept for the UI.
//! With a read-only MQTT client every request is refused before sending.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError, RwLock};
use std::time::Duration;

use rusthome_mqtt::MqttError;
use serde_json::{Value, json};
use tokio::sync::oneshot;

/// Requests the UI may send; anything else is refused.
pub const REQUESTS: [&str; 13] = [
    "permit_join",
    "device/rename",
    "device/remove",
    "device/ota_update/check",
    "device/ota_update/update",
    "device/bind",
    "device/unbind",
    "group/add",
    "group/remove",
    "group/rename",
    "group/members/add",
    "group/members/remove",
    "device/options",
];

/// Requests answered only once done (OTA can take 30 minutes): fire and forget.
const NO_WAIT: [&str; 1] = ["device/ota_update/update"];

const RECENT_EVENTS: usize = 50;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BridgeError {
    #[error("unknown request {0}")]
    Unknown(String),
    #[error("MQTT is read-only (RUSTHOME_MQTT_READ_ONLY=true): request not sent")]
    ReadOnly,
    #[error("Zigbee2MQTT: {0}")]
    Refused(String),
    #[error("no answer from Zigbee2MQTT")]
    Timeout,
    #[error("MQTT: {0}")]
    Mqtt(String),
}

#[derive(Default)]
struct State {
    info: Value,
    devices: Value,
    groups: Value,
    events: VecDeque<Value>,
    payloads: HashMap<String, Value>,
}

/// Bridge state and pending requests.
#[derive(Default)]
pub struct Bridge {
    state: RwLock<State>,
    pending: Mutex<HashMap<String, oneshot::Sender<Value>>>,
    next: AtomicU64,
}

impl Bridge {
    /// Applies one `bridge/<sub>` message. Returns true when it was handled.
    pub fn handle(&self, sub: &str, payload: &Value, now_ms: i64) -> bool {
        if let Some(kind) = sub.strip_prefix("response/") {
            let _ = kind;
            if let Some(tx) = payload["transaction"].as_str()
                && let Some(waiter) = self
                    .pending
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(tx)
            {
                let _ = waiter.send(payload.clone());
            }
            return true;
        }
        let mut st = self.state.write().unwrap_or_else(PoisonError::into_inner);
        match sub {
            "info" => st.info = payload.clone(),
            "devices" => st.devices = payload.clone(),
            "groups" => st.groups = payload.clone(),
            "event" => {
                let mut e = payload.clone();
                e["received"] = now_ms.into();
                st.events.push_front(e);
                st.events.truncate(RECENT_EVENTS);
            }
            _ => return false,
        }
        true
    }

    /// Keeps the last state payload of a device (LQI, battery, OTA…).
    pub fn record_payload(&self, friendly_name: &str, payload: &Value) {
        let mut st = self.state.write().unwrap_or_else(PoisonError::into_inner);
        let slot = st
            .payloads
            .entry(friendly_name.to_owned())
            .or_insert_with(|| json!({}));
        if let (Some(dst), Some(src)) = (slot.as_object_mut(), payload.as_object()) {
            for (k, v) in src {
                dst.insert(k.clone(), v.clone());
            }
        }
    }

    /// Everything the Zigbee page shows.
    pub fn snapshot(&self) -> Value {
        let st = self.state.read().unwrap_or_else(PoisonError::into_inner);
        json!({
            "info": st.info,
            "devices": st.devices,
            "groups": st.groups,
            "events": st.events,
            "payloads": st.payloads,
        })
    }

    /// Friendly name of a device by IEEE address.
    pub fn friendly_name(&self, ieee: &str) -> Option<String> {
        let st = self.state.read().unwrap_or_else(PoisonError::into_inner);
        st.devices.as_array()?.iter().find_map(|d| {
            (d["ieee_address"] == ieee)
                .then(|| d["friendly_name"].as_str().map(str::to_owned))
                .flatten()
        })
    }

    /// Prepares a request: checks its kind, adds a transaction id, registers
    /// the waiter. Returns the topic suffix, the payload and the receiver.
    pub fn prepare(
        &self,
        kind: &str,
        mut payload: Value,
    ) -> Result<(String, String, Option<oneshot::Receiver<Value>>), BridgeError> {
        if !REQUESTS.contains(&kind) {
            return Err(BridgeError::Unknown(kind.to_owned()));
        }
        let tx = format!("rusthome-{}", self.next.fetch_add(1, Ordering::Relaxed));
        if let Some(m) = payload.as_object_mut() {
            m.insert("transaction".into(), tx.clone().into());
        }
        let rx = if NO_WAIT.contains(&kind) {
            None
        } else {
            let (s, r) = oneshot::channel();
            self.pending
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(tx.clone(), s);
            Some(r)
        };
        Ok((format!("request/{kind}"), payload.to_string(), rx))
    }

    /// Forgets a request that could not be sent.
    pub fn cancel(&self, payload: &str) {
        if let Ok(v) = serde_json::from_str::<Value>(payload)
            && let Some(tx) = v["transaction"].as_str()
        {
            self.pending
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(tx);
        }
    }
}

/// Maps a publish error.
pub(crate) fn publish_error(e: MqttError) -> BridgeError {
    match e {
        MqttError::ReadOnly(_) => BridgeError::ReadOnly,
        other => BridgeError::Mqtt(other.to_string()),
    }
}

/// Waits for the bridge answer (`status: ok|error`).
pub(crate) async fn answer(
    rx: Option<oneshot::Receiver<Value>>,
    timeout: Duration,
) -> Result<Value, BridgeError> {
    let Some(rx) = rx else {
        return Ok(json!({ "status": "ok", "data": { "queued": true } }));
    };
    let resp = tokio::time::timeout(timeout, rx)
        .await
        .map_err(|_| BridgeError::Timeout)?
        .map_err(|_| BridgeError::Timeout)?;
    if resp["status"] == "ok" {
        Ok(resp["data"].clone())
    } else {
        Err(BridgeError::Refused(
            resp["error"].as_str().unwrap_or("unknown error").to_owned(),
        ))
    }
}

/// New entity ids after renaming a device from `old` to `new` (HA rule: the
/// slug of the old name, when it prefixes an object id, becomes the new slug).
/// `taken` says whether an id is already used; clashes get `_2`, `_3`…
pub fn plan_renames(
    old_name: &str,
    new_name: &str,
    entity_ids: &[String],
    taken: impl Fn(&str) -> bool,
) -> Vec<(String, String)> {
    let old = crate::mapping::slugify(old_name);
    let new = crate::mapping::slugify(new_name);
    if old.is_empty() || new.is_empty() || old == new {
        return Vec::new();
    }
    let mut planned: Vec<(String, String)> = Vec::new();
    for id in entity_ids {
        let Some((domain, object)) = id.split_once('.') else {
            continue;
        };
        let Some(rest) = object.strip_prefix(&old) else {
            continue;
        };
        if !(rest.is_empty() || rest.starts_with('_')) {
            continue; // `salon2_x` is not `salon` + suffix
        }
        let base = format!("{domain}.{new}{rest}");
        let mut candidate = base.clone();
        let mut n = 2;
        while taken(&candidate) || planned.iter().any(|(_, t)| *t == candidate) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        planned.push((id.clone(), candidate));
    }
    planned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_entity_renames_like_ha() {
        let ids = vec![
            "sensor.temperature_salon_temperature".to_owned(),
            "sensor.temperature_salon_battery".to_owned(),
            "sensor.temperature_salon2_battery".to_owned(),
            "sensor.autre".to_owned(),
        ];
        let plan = plan_renames("Température Salon", "Séjour", &ids, |id| {
            id == "sensor.sejour_battery"
        });
        assert_eq!(
            plan,
            vec![
                (
                    "sensor.temperature_salon_temperature".to_owned(),
                    "sensor.sejour_temperature".to_owned()
                ),
                (
                    "sensor.temperature_salon_battery".to_owned(),
                    "sensor.sejour_battery_2".to_owned()
                ),
            ]
        );
        assert_eq!(plan_renames("A", "a", &ids, |_| false).len(), 0);
    }

    #[test]
    fn requests_and_answers() {
        let b = Bridge::default();
        assert_eq!(
            b.prepare("device/interview_all", json!({})).unwrap_err(),
            BridgeError::Unknown("device/interview_all".into())
        );
        let (topic, payload, rx) = b.prepare("permit_join", json!({ "time": 120 })).unwrap();
        assert_eq!(topic, "request/permit_join");
        let sent: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(sent["time"], 120);
        let tx = sent["transaction"].as_str().unwrap().to_owned();
        assert!(b.handle(
            "response/permit_join",
            &json!({ "status": "ok", "data": { "time": 120 }, "transaction": tx }),
            0
        ));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let data = rt.block_on(answer(rx, Duration::from_secs(1))).unwrap();
        assert_eq!(data["time"], 120);
        // OTA updates are not awaited
        let (_, _, rx) = b
            .prepare("device/ota_update/update", json!({ "id": "x" }))
            .unwrap();
        assert!(rx.is_none());
    }

    #[test]
    fn refused_and_timed_out_requests() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let b = Bridge::default();
        let (_, payload, rx) = b.prepare("device/remove", json!({ "id": "x" })).unwrap();
        let tx = serde_json::from_str::<Value>(&payload).unwrap()["transaction"].clone();
        b.handle(
            "response/device/remove",
            &json!({ "status": "error", "error": "Device 'x' does not exist", "transaction": tx }),
            0,
        );
        assert_eq!(
            rt.block_on(answer(rx, Duration::from_secs(1))).unwrap_err(),
            BridgeError::Refused("Device 'x' does not exist".into())
        );
        let (_, _, rx) = b
            .prepare("group/add", json!({ "friendly_name": "g" }))
            .unwrap();
        assert_eq!(
            rt.block_on(answer(rx, Duration::from_millis(10)))
                .unwrap_err(),
            BridgeError::Timeout
        );
    }

    #[test]
    fn keeps_bridge_state_and_recent_events() {
        let b = Bridge::default();
        b.handle(
            "info",
            &json!({ "permit_join": false, "version": "2.6.0" }),
            0,
        );
        b.handle(
            "devices",
            &json!([{ "ieee_address": "0x01", "friendly_name": "Salon" }]),
            0,
        );
        for i in 0..60 {
            b.handle(
                "event",
                &json!({ "type": "device_joined", "data": { "i": i } }),
                i,
            );
        }
        b.record_payload("Salon", &json!({ "linkquality": 120 }));
        b.record_payload("Salon", &json!({ "battery": 90 }));
        let s = b.snapshot();
        assert_eq!(s["info"]["version"], "2.6.0");
        assert_eq!(s["events"].as_array().unwrap().len(), 50);
        assert_eq!(s["events"][0]["data"]["i"], 59, "newest first");
        assert_eq!(
            s["payloads"]["Salon"],
            json!({ "linkquality": 120, "battery": 90 })
        );
        assert_eq!(b.friendly_name("0x01").as_deref(), Some("Salon"));
        assert!(!b.handle("state", &json!("online"), 0));
    }
}
