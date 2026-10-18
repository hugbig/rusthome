//! Frigate integration (cf. docs/integrations.md §1).
//!
//! Cameras and tracked objects come from `GET /api/config` (MQTT keeps
//! retained topics of removed cameras); live state comes from MQTT; the
//! initial object counts come from in-progress events, because Frigate only
//! publishes counts on change (`binary_sensor.chambre_person_occupancy` feeds
//! the alarm: it must be right at startup). Feature switches publish on
//! `frigate/<cam>/<feature>/set`, refused while MQTT is read-only.
//!
//! Only camera names and tracked objects are kept from the config: it also
//! holds the MQTT password and RTSP URLs with credentials.

pub mod media;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use rusthome_core::{Hub, ServiceCall, ServiceError, target_entity_ids};
use rusthome_model::{Context, Device, EntityEntry, EntityId, EntityStatus};
use rusthome_mqtt::{Mqtt, MqttError, Publisher};
use serde_json::{Map, Value, json};
use url::Url;

pub const INTEGRATION: &str = "frigate";

/// Switchable camera features (same set as HA's Frigate integration).
pub const FEATURES: [&str; 4] = ["detect", "recordings", "snapshots", "motion"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Camera {
    pub name: String,
    pub objects: Vec<String>,
}

/// Cameras and tracked objects; nothing else is read from the config.
#[must_use]
fn str_list(v: &Value) -> Option<Vec<String>> {
    v.as_array().map(|a| {
        a.iter()
            .filter_map(|x| x.as_str().map(str::to_owned))
            .collect()
    })
}

pub fn cameras_from_config(config: &Value) -> Vec<Camera> {
    let default_objects =
        str_list(&config["objects"]["track"]).unwrap_or_else(|| vec!["person".into()]);
    let mut cams: Vec<Camera> = config["cameras"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, c)| c["enabled"] != false)
        .map(|(name, c)| Camera {
            name: name.clone(),
            objects: str_list(&c["objects"]["track"]).unwrap_or_else(|| default_objects.clone()),
        })
        .collect();
    cams.sort_by(|a, b| a.name.cmp(&b.name));
    cams
}

/// One entity: HA `unique_id` is `{entry}:{kind}:{suffix}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    pub kind: &'static str,
    pub suffix: String,
    pub domain: &'static str,
    pub default_entity_id: String,
    pub name: String,
}

#[must_use]
pub fn specs(cam: &Camera) -> Vec<Spec> {
    let c = &cam.name;
    let spec = |kind, suffix: String, domain, object_id: String, name: String| Spec {
        kind,
        default_entity_id: format!("{domain}.{object_id}"),
        suffix,
        domain,
        name,
    };
    let mut out = vec![
        spec("camera", c.clone(), "camera", c.clone(), c.clone()),
        spec(
            "motion_sensor",
            c.clone(),
            "binary_sensor",
            format!("{c}_motion"),
            format!("{c} Motion"),
        ),
    ];
    for obj in cam.objects.iter().map(String::as_str).chain(["all"]) {
        out.push(spec(
            "occupancy_sensor",
            format!("{c}_{obj}"),
            "binary_sensor",
            format!("{c}_{obj}_occupancy"),
            format!("{c} {obj} occupancy"),
        ));
        out.push(spec(
            "sensor_object_count",
            format!("{c}_{obj}"),
            "sensor",
            format!("{c}_{obj}_count"),
            format!("{c} {obj} count"),
        ));
    }
    for f in FEATURES {
        out.push(spec(
            "switch",
            format!("{c}_{f}"),
            "switch",
            format!("{c}_{f}"),
            format!("{c} {f}"),
        ));
    }
    out
}

/// What an MQTT message changes, as `(kind, suffix, state)` triples.
#[must_use]
pub fn interpret(
    prefix: &str,
    cams: &[Camera],
    topic: &str,
    payload: &str,
) -> Vec<(&'static str, String, String)> {
    let Some(rest) = topic.strip_prefix(prefix).and_then(|r| r.strip_prefix('/')) else {
        return Vec::new();
    };
    let parts: Vec<&str> = rest.split('/').collect();
    let Some(cam) = parts
        .first()
        .and_then(|c| cams.iter().find(|x| x.name == *c))
    else {
        return Vec::new();
    };
    let c = &cam.name;
    let on_off = |p: &str| match p {
        "ON" => Some("on".to_owned()),
        "OFF" => Some("off".to_owned()),
        _ => None,
    };
    match parts.as_slice() {
        [_, "motion"] => on_off(payload)
            .map(|s| vec![("motion_sensor", c.clone(), s)])
            .unwrap_or_default(),
        [_, feature, "state"] if FEATURES.contains(feature) => on_off(payload)
            .map(|s| vec![("switch", format!("{c}_{feature}"), s)])
            .unwrap_or_default(),
        [_, obj] if *obj == "all" || cam.objects.iter().any(|o| o == obj) => {
            match payload.trim().parse::<u64>() {
                Ok(n) => vec![
                    ("sensor_object_count", format!("{c}_{obj}"), n.to_string()),
                    (
                        "occupancy_sensor",
                        format!("{c}_{obj}"),
                        if n > 0 { "on" } else { "off" }.to_owned(),
                    ),
                ],
                Err(_) => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

/// Initial counts from in-progress events (`(camera, label)` → count).
#[must_use]
pub fn counts_from_events(events: &[Value]) -> HashMap<(String, String), u64> {
    let mut m = HashMap::new();
    for e in events {
        if let (Some(c), Some(l)) = (e["camera"].as_str(), e["label"].as_str()) {
            *m.entry((c.to_owned(), l.to_owned())).or_insert(0) += 1;
            *m.entry((c.to_owned(), "all".to_owned())).or_insert(0) += 1;
        }
    }
    m
}

pub struct Frigate {
    hub: Hub,
    prefix: String,
    api: Url,
    http: reqwest::Client,
    /// For media files: connect timeout only (clips stream for a while).
    media_http: reqwest::Client,
    publisher: Arc<dyn Publisher>,
    cameras: Mutex<Vec<Camera>>,
    /// `{kind}:{suffix}` → entity id
    ids: Mutex<HashMap<String, EntityId>>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub cameras: usize,
    pub entities: usize,
    pub taken_over: usize,
}

impl Frigate {
    pub fn new(
        hub: Hub,
        api: Url,
        prefix: &str,
        publisher: Arc<dyn Publisher>,
    ) -> Result<Arc<Self>, reqwest::Error> {
        Ok(Arc::new(Self {
            hub,
            prefix: prefix.trim_end_matches('/').to_owned(),
            api,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()?,
            media_http: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .build()?,
            publisher,
            cameras: Mutex::default(),
            ids: Mutex::default(),
        }))
    }

    async fn get(&self, path: &str) -> Result<Value, reqwest::Error> {
        let url = format!("{}{path}", self.api.as_str().trim_end_matches('/'));
        self.http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }

    /// Frigate camera name behind a `camera.*` entity, if Frigate owns it.
    pub fn camera_for(&self, entity_id: &EntityId) -> Option<String> {
        self.ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find_map(|(key, id)| {
                (id == entity_id)
                    .then(|| key.strip_prefix("camera:").map(str::to_owned))
                    .flatten()
            })
    }

    /// Latest JPEG of a camera (`GET /api/<camera>/latest.jpg`, read-only).
    pub async fn snapshot(&self, camera: &str) -> Result<Vec<u8>, reqwest::Error> {
        let url = format!(
            "{}/api/{camera}/latest.jpg",
            self.api.as_str().trim_end_matches('/')
        );
        let res = self.http.get(url).send().await?.error_for_status()?;
        Ok(res.bytes().await?.to_vec())
    }

    /// Registers cameras and entities; initial states from in-progress events.
    pub fn load(&self, cams: Vec<Camera>, in_progress: &[Value]) -> SyncReport {
        let reg = &self.hub.registries;
        let counts = counts_from_events(in_progress);
        let mut report = SyncReport {
            cameras: cams.len(),
            ..SyncReport::default()
        };
        let mut ids = HashMap::new();
        for cam in &cams {
            let device_id = format!("frigate-{}", cam.name);
            let mut ha_area = None;
            for s in specs(cam) {
                let tail = format!(":{}:{}", s.kind, s.suffix);
                let existing = reg.entities().into_iter().find(|e| {
                    (e.integration == "ha:frigate" || e.integration == INTEGRATION)
                        && e.unique_id.as_deref().is_some_and(|u| u.ends_with(&tail))
                });
                if existing
                    .as_ref()
                    .is_some_and(|e| e.integration.starts_with("ha:"))
                {
                    report.taken_over += 1;
                }
                if ha_area.is_none() {
                    ha_area = existing
                        .as_ref()
                        .and_then(|e| e.device_id.as_deref())
                        .and_then(|d| reg.device(d))
                        .and_then(|d| d.area_id);
                }
                let entity_id = match &existing {
                    Some(e) => e.entity_id.clone(),
                    None => match EntityId::parse(&s.default_entity_id) {
                        Ok(id) => id,
                        Err(_) => continue,
                    },
                };
                reg.upsert_entity(EntityEntry {
                    uid: existing.as_ref().map_or_else(
                        || format!("frigate-{}:{}", s.kind, s.suffix),
                        |e| e.uid.clone(),
                    ),
                    entity_id: entity_id.clone(),
                    integration: INTEGRATION.into(),
                    unique_id: existing
                        .as_ref()
                        .and_then(|e| e.unique_id.clone())
                        .or_else(|| Some(format!("{}:{}", s.kind, s.suffix))),
                    device_id: Some(device_id.clone()),
                    name: existing.as_ref().and_then(|e| e.name.clone()),
                    area_id: existing.as_ref().and_then(|e| e.area_id.clone()),
                    labels: existing
                        .as_ref()
                        .map(|e| e.labels.clone())
                        .unwrap_or_default(),
                    icon: None,
                    hidden: existing.as_ref().is_some_and(|e| e.hidden),
                    disabled: existing.as_ref().is_some_and(|e| e.disabled),
                    status: EntityStatus::Active,
                });
                // Initial state: counts from events; motion off; camera idle.
                let initial = match s.kind {
                    "sensor_object_count" | "occupancy_sensor" => {
                        let obj = s
                            .suffix
                            .strip_prefix(&format!("{}_", cam.name))
                            .unwrap_or("all");
                        let n = counts
                            .get(&(cam.name.clone(), obj.to_owned()))
                            .copied()
                            .unwrap_or(0);
                        Some(if s.kind == "occupancy_sensor" {
                            if n > 0 { "on" } else { "off" }.to_owned()
                        } else {
                            n.to_string()
                        })
                    }
                    "motion_sensor" => Some("off".to_owned()),
                    "camera" => Some("idle".to_owned()),
                    _ => None, // switches: retained MQTT state arrives right after subscribing
                };
                if let Some(state) = initial {
                    self.set(&entity_id, &s, state);
                }
                ids.insert(format!("{}:{}", s.kind, s.suffix), entity_id);
                report.entities += 1;
            }
            reg.upsert_device(Device {
                id: device_id,
                integration: INTEGRATION.into(),
                name: cam.name.clone(),
                manufacturer: Some("Frigate".into()),
                model: Some("Camera".into()),
                sw_version: None,
                area_id: ha_area,
                labels: vec![],
                disabled: false,
            });
        }
        *self.ids.lock().unwrap_or_else(PoisonError::into_inner) = ids;
        *self.cameras.lock().unwrap_or_else(PoisonError::into_inner) = cams;
        report
    }

    fn set(&self, id: &EntityId, spec: &Spec, state: String) {
        let mut attrs = Map::new();
        let name = self
            .hub
            .registries
            .entity_by_entity_id(id)
            .and_then(|e| e.name)
            .unwrap_or_else(|| spec.name.clone());
        attrs.insert("friendly_name".into(), name.into());
        match spec.kind {
            "occupancy_sensor" => {
                attrs.insert("device_class".into(), "occupancy".into());
            }
            "motion_sensor" => {
                attrs.insert("device_class".into(), "motion".into());
            }
            _ => {}
        }
        self.hub
            .states
            .set(id.clone(), state, attrs, Context::default());
    }

    /// Applies one MQTT message. Returns the number of states written.
    pub fn handle_message(&self, topic: &str, payload: &str) -> usize {
        if topic == format!("{}/available", self.prefix) {
            if payload.trim() == "offline" {
                let ids: Vec<EntityId> = self
                    .ids
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .values()
                    .cloned()
                    .collect();
                for id in &ids {
                    if let Some(s) = self.hub.states.get(id) {
                        self.hub.states.set(
                            id.clone(),
                            "unavailable",
                            s.attributes.clone(),
                            Context::default(),
                        );
                    }
                }
                return ids.len();
            }
            return 0;
        }
        let cams = self
            .cameras
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let mut n = 0;
        for (kind, suffix, state) in interpret(&self.prefix, &cams, topic, payload) {
            let Some(cam) = cams
                .iter()
                .find(|c| suffix == c.name || suffix.starts_with(&format!("{}_", c.name)))
            else {
                continue;
            };
            let Some(spec) = specs(cam)
                .into_iter()
                .find(|s| s.kind == kind && s.suffix == suffix)
            else {
                continue;
            };
            let id = self
                .ids
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&format!("{kind}:{suffix}"))
                .cloned();
            if let Some(id) = id {
                self.set(&id, &spec, state);
                n += 1;
            }
        }
        n
    }

    pub fn register_services(self: &Arc<Self>) {
        for service in ["turn_on", "turn_off", "toggle"] {
            let me = Arc::clone(self);
            self.hub.services.register(
                INTEGRATION,
                "switch",
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
        for target in target_entity_ids(&call.data)? {
            let key = self
                .ids
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .find(|(_, id)| **id == target)
                .map(|(k, _)| k.clone())
                .ok_or_else(|| {
                    ServiceError::InvalidData(format!("{target} is not a Frigate entity"))
                })?;
            let suffix = key.strip_prefix("switch:").ok_or_else(|| {
                ServiceError::InvalidData(format!("{target} is not a Frigate switch"))
            })?;
            let (cam, feature) = FEATURES
                .iter()
                .find_map(|f| {
                    suffix
                        .strip_suffix(&format!("_{f}"))
                        .map(|c| (c.to_owned(), *f))
                })
                .ok_or_else(|| ServiceError::InvalidData(format!("unknown feature in {suffix}")))?;
            let on = match call.service.as_str() {
                "turn_on" => true,
                "turn_off" => false,
                _ => self.hub.states.get(&target).is_none_or(|s| s.state != "on"),
            };
            let topic = format!("{}/{cam}/{feature}/set", self.prefix);
            let payload = if on { "ON" } else { "OFF" };
            self.publisher
                .publish(topic.clone(), payload.into())
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

    /// Frigate announces it is back (`<prefix>/available` = `online`).
    #[must_use]
    pub fn back_online(&self, topic: &str, payload: &str) -> bool {
        topic == format!("{}/available", self.prefix) && payload.trim() == "online"
    }

    /// Loads cameras over HTTP (retrying), then follows MQTT.
    pub async fn run(self: Arc<Self>, mqtt: Mqtt) {
        let mut rx = mqtt.messages();
        self.sync().await;
        if let Err(e) = mqtt.subscribe(&format!("{}/#", self.prefix)).await {
            tracing::error!(error = %e, "Frigate: subscribe failed");
        }
        loop {
            match rx.recv().await {
                Ok(m) if m.topic.starts_with(&self.prefix) => {
                    // Snapshots are binary JPEGs: only text payloads matter here.
                    if let Ok(text) = std::str::from_utf8(&m.payload) {
                        self.handle_message(&m.topic, text);
                        // Back after a restart: the entities left
                        // `unavailable` are reloaded (occupancy feeds the
                        // alarm, code review n° 20).
                        if self.back_online(&m.topic, text) {
                            let me = Arc::clone(&self);
                            tokio::spawn(async move { me.sync().await });
                        }
                    }
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(missed = n, "Frigate: lagging");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    }

    /// Loads cameras and events in progress over HTTP, retrying until done.
    async fn sync(&self) {
        let mut backoff = Duration::from_secs(2);
        loop {
            let loaded = async {
                let cfg = self.get("/api/config").await?;
                let events = self.get("/api/events?in_progress=1&limit=100").await?;
                Ok::<_, reqwest::Error>((
                    cameras_from_config(&cfg),
                    events.as_array().cloned().unwrap_or_default(),
                ))
            }
            .await;
            match loaded {
                Ok((cams, events)) => {
                    let report = self.load(cams, &events);
                    tracing::info!(?report, "Frigate: cameras synchronised");
                    break;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Frigate API unreachable, retrying in {backoff:?}");
                }
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(60));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use super::*;
    use futures_util::future::BoxFuture;
    use rusthome_core::SystemClock;

    #[derive(Default)]
    struct Rec {
        read_only: bool,
        sent: StdMutex<Vec<(String, String)>>,
    }

    impl Publisher for Rec {
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

    /// Shape of Frigate's `/api/config`, with fake secrets that must never be kept.
    fn config() -> Value {
        json!({
            "mqtt": {"host": "192.168.1.35", "password": "secret"},
            "objects": {"track": ["person"]},
            "cameras": {
                "chambre": {"enabled": true, "ffmpeg": {"inputs": [{"path": "rtsp://user:pass@cam/stream"}]}, "objects": {"track": ["person"]}},
                "garage": {"enabled": false}
            }
        })
    }

    fn ha_entry(entity_id: &str, unique_id: &str) -> EntityEntry {
        EntityEntry {
            uid: format!("ha-{entity_id}"),
            entity_id: EntityId::parse(entity_id).unwrap(),
            integration: "ha:frigate".into(),
            unique_id: Some(unique_id.into()),
            device_id: None,
            name: None,
            area_id: None,
            labels: vec![],
            icon: None,
            hidden: false,
            disabled: false,
            status: EntityStatus::Active,
        }
    }

    fn setup(read_only: bool) -> (Hub, Arc<Frigate>, Arc<Rec>) {
        let hub = Hub::new(Arc::new(SystemClock));
        hub.registries.upsert_entity(ha_entry(
            "binary_sensor.chambre_person_occupancy",
            "01M0D8QDQ8Q1SJHZGEX5T61PZJ:occupancy_sensor:chambre_person",
        ));
        hub.registries.upsert_entity(ha_entry(
            "switch.chambre_recordings",
            "01M0D8QDQ8Q1SJHZGEX5T61PZJ:switch:chambre_recordings",
        ));
        let rec = Arc::new(Rec {
            read_only,
            ..Rec::default()
        });
        let f = Frigate::new(
            hub.clone(),
            Url::parse("http://127.0.0.1:5000").unwrap(),
            "frigate",
            rec.clone(),
        )
        .unwrap();
        (hub, f, rec)
    }

    fn state(hub: &Hub, id: &str) -> String {
        hub.states
            .get(&EntityId::parse(id).unwrap())
            .unwrap()
            .state
            .clone()
    }

    #[test]
    fn frigate_coming_back_is_detected() {
        let (_, f, _) = setup(true);
        assert!(f.back_online("frigate/available", "online"));
        assert!(f.back_online("frigate/available", " online\n"));
        assert!(!f.back_online("frigate/available", "offline"));
        assert!(!f.back_online("frigate/chambre/person", "online"));
    }

    #[test]
    fn only_enabled_cameras_and_no_secret_kept() {
        let cams = cameras_from_config(&config());
        assert_eq!(
            cams,
            vec![Camera {
                name: "chambre".into(),
                objects: vec!["person".into()]
            }]
        );
        assert!(!format!("{cams:?}").contains("secret") && !format!("{cams:?}").contains("pass"));
    }

    #[test]
    fn person_already_present_at_startup_is_seen() {
        let (hub, f, _) = setup(true);
        let in_progress = vec![json!({"camera": "chambre", "label": "person"})];
        let report = f.load(cameras_from_config(&config()), &in_progress);
        assert_eq!(report.taken_over, 2);
        assert_eq!(state(&hub, "binary_sensor.chambre_person_occupancy"), "on");
        assert_eq!(state(&hub, "sensor.chambre_person_count"), "1");
        assert_eq!(state(&hub, "binary_sensor.chambre_all_occupancy"), "on");
    }

    #[test]
    fn mqtt_messages_update_states() {
        let (hub, f, _) = setup(true);
        f.load(cameras_from_config(&config()), &[]);
        assert_eq!(state(&hub, "binary_sensor.chambre_person_occupancy"), "off");
        f.handle_message("frigate/chambre/person", "2");
        assert_eq!(state(&hub, "binary_sensor.chambre_person_occupancy"), "on");
        assert_eq!(state(&hub, "sensor.chambre_person_count"), "2");
        f.handle_message("frigate/chambre/motion", "ON");
        assert_eq!(state(&hub, "binary_sensor.chambre_motion"), "on");
        f.handle_message("frigate/chambre/recordings/state", "ON");
        assert_eq!(state(&hub, "switch.chambre_recordings"), "on");
        // Retained topics of a camera removed from the config are ignored.
        assert_eq!(
            f.handle_message("frigate/sonoff_pt2/recordings/state", "OFF"),
            0
        );
        // Frigate down: everything becomes unavailable.
        f.handle_message("frigate/available", "offline");
        assert_eq!(
            state(&hub, "binary_sensor.chambre_person_occupancy"),
            "unavailable"
        );
    }

    #[tokio::test]
    async fn switch_commands() {
        let (hub, f, rec) = setup(false);
        f.load(cameras_from_config(&config()), &[]);
        f.register_services();
        let call = |svc: &str| ServiceCall {
            domain: "switch".into(),
            service: svc.into(),
            data: json!({"entity_id": "switch.chambre_recordings"}),
            context: Context::default(),
        };
        hub.call_service(call("turn_on")).await.unwrap();
        assert_eq!(
            rec.sent.lock().unwrap()[0],
            ("frigate/chambre/recordings/set".into(), "ON".into())
        );

        let (hub_ro, f_ro, _) = setup(true);
        f_ro.load(cameras_from_config(&config()), &[]);
        f_ro.register_services();
        let err = hub_ro.call_service(call("turn_off")).await.unwrap_err();
        assert!(matches!(err, ServiceError::Refused(m) if m.contains("read-only")));
    }
}
