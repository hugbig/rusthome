//! Philips Hue integration (CLIP v2, local API).
//!
//! Reads every resource, follows the bridge event stream (SSE) for live
//! updates, takes over HA's Hue entities by `unique_id` (= resource id), and
//! provides `light.*` / `scene.turn_on`. **Read-only by default**
//! (`RUSTHOME_HUE_READ_ONLY=true`): writes are refused before any request.

pub mod mapping;
mod sse;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use futures_util::StreamExt;
use mapping::{HueEntity, Resources, entities, entity, light_command};
use rusthome_core::{Hub, ServiceCall, ServiceError, target_entity_ids};
use rusthome_model::{Context, Device, EntityEntry, EntityId, EntityStatus};
use serde_json::{Value, json};
use url::Url;

pub use sse::SseParser;

pub const INTEGRATION: &str = "hue";

/// Resource types read from the bridge.
pub const RESOURCE_TYPES: [&str; 9] = [
    "device",
    "light",
    "room",
    "zone",
    "grouped_light",
    "scene",
    "button",
    "entertainment_configuration",
    "device_power",
];

#[derive(Debug, Clone)]
pub struct HueConfig {
    /// `https://192.168.1.247`
    pub url: Url,
    pub key: String,
    pub read_only: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum HueError {
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("bridge answered {0}")]
    Status(u16),
    #[error("Hue is read-only (RUSTHOME_HUE_READ_ONLY=true): command not sent")]
    ReadOnly,
}

pub struct Client {
    http: reqwest::Client,
    stream_http: reqwest::Client,
    cfg: HueConfig,
}

impl Client {
    pub fn new(cfg: HueConfig) -> Result<Self, HueError> {
        // The bridge certificate is signed by Signify's private CA.
        let base = || reqwest::Client::builder().tls_danger_accept_invalid_certs(true);
        Ok(Self {
            http: base().timeout(Duration::from_secs(15)).build()?,
            stream_http: base().build()?, // no timeout: long-lived stream
            cfg,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.cfg.url.as_str().trim_end_matches('/'))
    }

    pub async fn resources(&self) -> Result<Resources, HueError> {
        let mut lists = HashMap::new();
        for t in RESOURCE_TYPES {
            let res = self
                .http
                .get(self.url(&format!("/clip/v2/resource/{t}")))
                .header("hue-application-key", &self.cfg.key)
                .send()
                .await?;
            if !res.status().is_success() {
                return Err(HueError::Status(res.status().as_u16()));
            }
            let body: Value = res.json().await?;
            lists.insert(
                t.to_owned(),
                body["data"].as_array().cloned().unwrap_or_default(),
            );
        }
        Ok(Resources::from_lists(&lists))
    }

    /// The only write. Refused up front in read-only mode.
    pub async fn put(&self, rtype: &str, id: &str, body: &Value) -> Result<(), HueError> {
        if self.cfg.read_only {
            tracing::warn!(rtype, id, "Hue read-only: command refused");
            return Err(HueError::ReadOnly);
        }
        let res = self
            .http
            .put(self.url(&format!("/clip/v2/resource/{rtype}/{id}")))
            .header("hue-application-key", &self.cfg.key)
            .json(body)
            .send()
            .await?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(HueError::Status(res.status().as_u16()))
        }
    }
}

/// Live link between the bridge and the hub.
pub struct Hue {
    hub: Hub,
    client: Client,
    resources: Mutex<Resources>,
    /// resource id → entity id
    ids: Mutex<HashMap<String, EntityId>>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub entities: usize,
    pub taken_over: usize,
}

impl Hue {
    pub fn new(hub: Hub, cfg: HueConfig) -> Result<Arc<Self>, HueError> {
        Ok(Arc::new(Self {
            hub,
            client: Client::new(cfg)?,
            resources: Mutex::default(),
            ids: Mutex::default(),
        }))
    }

    /// Registers devices and entities, writes every state.
    pub fn load(&self, res: Resources) -> SyncReport {
        let reg = &self.hub.registries;
        let mut report = SyncReport::default();
        let mut ids = HashMap::new();
        let mut ha_areas: HashMap<String, String> = HashMap::new();
        for e in entities(&res) {
            let existing = reg.entity_by_unique_id(&e.resource_id);
            if existing
                .as_ref()
                .is_some_and(|x| x.integration.starts_with("ha:"))
            {
                report.taken_over += 1;
            }
            let entity_id = match &existing {
                Some(x) => x.entity_id.clone(),
                None => match EntityId::parse(&format!("{}.{}", e.domain, slug(&e.name))) {
                    Ok(id) => id,
                    Err(_) => continue,
                },
            };
            let device_id = e.owner.as_ref().map(|o| format!("hue-{o}"));
            if let (Some(owner), Some(area)) = (
                &e.owner,
                existing
                    .as_ref()
                    .and_then(|x| x.device_id.as_deref())
                    .and_then(|d| reg.device(d))
                    .and_then(|d| d.area_id),
            ) {
                ha_areas.entry(owner.clone()).or_insert(area);
            }
            reg.upsert_entity(EntityEntry {
                uid: existing
                    .as_ref()
                    .map_or_else(|| format!("hue-{}", e.resource_id), |x| x.uid.clone()),
                entity_id: entity_id.clone(),
                integration: INTEGRATION.into(),
                unique_id: Some(e.resource_id.clone()),
                device_id,
                name: existing.as_ref().and_then(|x| x.name.clone()),
                area_id: existing.as_ref().and_then(|x| x.area_id.clone()),
                labels: existing
                    .as_ref()
                    .map(|x| x.labels.clone())
                    .unwrap_or_default(),
                icon: existing.as_ref().and_then(|x| x.icon.clone()),
                hidden: existing.as_ref().is_some_and(|x| x.hidden),
                disabled: existing.as_ref().is_some_and(|x| x.disabled),
                status: EntityStatus::Active,
            });
            self.write_state(&entity_id, &e);
            ids.insert(e.resource_id.clone(), entity_id);
            report.entities += 1;
        }
        for r in res
            .by_id
            .values()
            .filter(|r| matches!(r["type"].as_str(), Some("device" | "room" | "zone")))
        {
            let Some(id) = r["id"].as_str() else { continue };
            reg.upsert_device(Device {
                id: format!("hue-{id}"),
                integration: INTEGRATION.into(),
                name: r["metadata"]["name"]
                    .as_str()
                    .unwrap_or("Hue")
                    .trim()
                    .to_owned(),
                manufacturer: r["product_data"]["manufacturer_name"]
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| Some("Signify".into())),
                model: r["product_data"]["product_name"]
                    .as_str()
                    .map(str::to_owned),
                sw_version: r["product_data"]["software_version"]
                    .as_str()
                    .map(str::to_owned),
                area_id: ha_areas.get(id).cloned(),
                labels: vec![],
                disabled: false,
            });
        }
        *self.ids.lock().unwrap_or_else(PoisonError::into_inner) = ids;
        *self
            .resources
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = res;
        report
    }

    fn write_state(&self, id: &EntityId, e: &HueEntity) {
        let mut attrs = e.attributes.clone();
        if let Some(name) = self
            .hub
            .registries
            .entity_by_entity_id(id)
            .and_then(|x| x.name)
        {
            attrs.insert("friendly_name".into(), name.into());
        }
        self.hub
            .states
            .set(id.clone(), e.state.clone(), attrs, Context::default());
    }

    /// Applies one event-stream message (`[{type, data: [...]}]`). Returns
    /// `false` when a full resync is needed (unknown resource added).
    pub fn handle_event(&self, payload: &Value) -> bool {
        let Some(events) = payload.as_array() else {
            return true;
        };
        for ev in events {
            if ev["type"] != "update" {
                return false; // add/delete: structure changed
            }
            for upd in ev["data"].as_array().into_iter().flatten() {
                let merged = {
                    let mut res = self
                        .resources
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    res.merge(upd)
                        .and_then(|id| res.by_id.get(&id).map(|r| (entity(&res, r), id)))
                };
                let Some((Some(e), id)) = merged else {
                    continue;
                };
                let target = self
                    .ids
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .get(&id)
                    .cloned();
                if let Some(target) = target {
                    self.write_state(&target, &e);
                }
            }
        }
        true
    }

    pub fn register_services(self: &Arc<Self>) {
        for (domain, service) in [
            ("light", "turn_on"),
            ("light", "turn_off"),
            ("light", "toggle"),
            ("scene", "turn_on"),
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
        for target in target_entity_ids(&call.data)? {
            let (rid, rtype) = {
                let ids = self.ids.lock().unwrap_or_else(PoisonError::into_inner);
                let rid = ids
                    .iter()
                    .find(|(_, e)| **e == target)
                    .map(|(r, _)| r.clone());
                let res = self
                    .resources
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                let rtype = rid
                    .as_ref()
                    .and_then(|r| res.by_id.get(r))
                    .and_then(|v| v["type"].as_str())
                    .map(str::to_owned);
                (rid, rtype)
            };
            let (Some(rid), Some(rtype)) = (rid, rtype) else {
                return Err(ServiceError::InvalidData(format!(
                    "{target} is not a Hue entity"
                )));
            };
            let body = if call.domain == "scene" {
                json!({ "recall": { "action": "active" } })
            } else {
                let on = self
                    .hub
                    .states
                    .get(&target)
                    .is_some_and(|s| s.state == "on");
                light_command(&call.service, &call.data, on).map_err(ServiceError::InvalidData)?
            };
            self.client
                .put(&rtype, &rid, &body)
                .await
                .map_err(|e| match e {
                    HueError::ReadOnly => ServiceError::Refused(e.to_string()),
                    other => ServiceError::Failed(other.to_string()),
                })?;
            sent.push(json!({ "resource": rid, "body": body }));
        }
        Ok(Value::from(sent))
    }

    /// Loads everything, then follows the event stream; resyncs on errors.
    pub async fn run(self: Arc<Self>) {
        let mut backoff = Duration::from_secs(2);
        loop {
            match self.client.resources().await {
                Ok(res) => {
                    let report = self.load(res);
                    tracing::info!(
                        ?report,
                        read_only = self.client.cfg.read_only,
                        "Hue: bridge synchronised"
                    );
                    backoff = Duration::from_secs(2);
                    if let Err(e) = self.follow_stream().await {
                        tracing::warn!(error = %e, "Hue: event stream ended");
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Hue: bridge unreachable, retrying in {backoff:?}");
                }
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(60));
        }
    }

    async fn follow_stream(&self) -> Result<(), HueError> {
        let res = self
            .client
            .stream_http
            .get(self.client.url("/eventstream/clip/v2"))
            .header("hue-application-key", &self.client.cfg.key)
            .header("accept", "text/event-stream")
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(HueError::Status(res.status().as_u16()));
        }
        let mut parser = SseParser::default();
        let mut body = res.bytes_stream();
        while let Some(chunk) = body.next().await {
            for data in parser.push(&chunk?) {
                match serde_json::from_str::<Value>(&data) {
                    Ok(v) if !self.handle_event(&v) => return Ok(()), // resync
                    Ok(_) => {}
                    Err(e) => tracing::debug!(error = %e, "Hue: unreadable event"),
                }
            }
        }
        Ok(())
    }
}

fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().to_lowercase().chars() {
        let c = match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusthome_core::SystemClock;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn real() -> Resources {
        let lists: HashMap<String, Vec<Value>> =
            serde_json::from_str(include_str!("../tests/fixtures/resources.json")).unwrap();
        Resources::from_lists(&lists)
    }

    fn hue(url: &str, read_only: bool) -> (Hub, Arc<Hue>) {
        let hub = Hub::new(Arc::new(SystemClock));
        // HA's mirror of the "Hue go Bureau" light, renamed by the user.
        hub.registries.upsert_entity(EntityEntry {
            uid: "ha-go".into(),
            entity_id: EntityId::parse("light.hue_go_bureau").unwrap(),
            integration: "ha:hue".into(),
            unique_id: Some("376d4fa0-4d3f-4cd0-8b5b-ab25fdc443bc".into()),
            device_id: None,
            name: Some("Lampe du bureau".into()),
            area_id: Some("bureau".into()),
            labels: vec![],
            icon: None,
            hidden: false,
            disabled: false,
            status: EntityStatus::Active,
        });
        let h = Hue::new(
            hub.clone(),
            HueConfig {
                url: Url::parse(url).unwrap(),
                key: "k".into(),
                read_only,
            },
        )
        .unwrap();
        (hub, h)
    }

    fn call(domain: &str, service: &str, data: Value) -> ServiceCall {
        ServiceCall {
            domain: domain.into(),
            service: service.into(),
            data,
            context: Context::default(),
        }
    }

    #[test]
    fn takes_over_and_follows_events() {
        let (hub, h) = hue("https://127.0.0.1", true);
        let report = h.load(real());
        assert_eq!(report.taken_over, 1);
        assert!(report.entities > 60);
        let go = EntityId::parse("light.hue_go_bureau").unwrap();
        let entry = hub.registries.entity_by_entity_id(&go).unwrap();
        assert_eq!(
            (entry.integration.as_str(), entry.area_id.as_deref()),
            ("hue", Some("bureau"))
        );
        assert_eq!(
            hub.states.get(&go).unwrap().attributes["friendly_name"],
            "Lampe du bureau"
        );

        let ev = json!([{"type": "update", "data": [{"id": "376d4fa0-4d3f-4cd0-8b5b-ab25fdc443bc", "type": "light", "on": {"on": false}}]}]);
        assert!(h.handle_event(&ev));
        assert_eq!(hub.states.get(&go).unwrap().state, "off");
        assert!(
            !h.handle_event(&json!([{"type": "add", "data": []}])),
            "structure change asks for a resync"
        );
    }

    #[tokio::test]
    async fn read_only_refuses_before_any_request() {
        let server = MockServer::start().await;
        let (hub, h) = hue(&server.uri(), true);
        h.load(real());
        h.register_services();
        let err = hub
            .call_service(call(
                "light",
                "turn_on",
                json!({"entity_id": "light.hue_go_bureau"}),
            ))
            .await
            .unwrap_err();
        assert!(matches!(err, ServiceError::Refused(m) if m.contains("read-only")));
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "nothing sent to the bridge"
        );
    }

    #[tokio::test]
    async fn writable_sends_clip_v2_put() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path(
                "/clip/v2/resource/light/376d4fa0-4d3f-4cd0-8b5b-ab25fdc443bc",
            ))
            .and(header("hue-application-key", "k"))
            .and(body_json(
                json!({"on": {"on": true}, "dimming": {"brightness": 50.0}}),
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"data": [], "errors": []})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path(
                "/clip/v2/resource/scene/07cd7625-0bb7-4ed0-8df0-198027734719",
            ))
            .and(body_json(json!({"recall": {"action": "active"}})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
            .expect(1)
            .mount(&server)
            .await;
        let (hub, h) = hue(&server.uri(), false);
        h.load(real());
        h.register_services();
        hub.call_service(call(
            "light",
            "turn_on",
            json!({"entity_id": "light.hue_go_bureau", "brightness_pct": 50}),
        ))
        .await
        .unwrap();
        hub.call_service(call(
            "scene",
            "turn_on",
            json!({"entity_id": "scene.salon_printemps_floral"}),
        ))
        .await
        .unwrap();
    }
}
