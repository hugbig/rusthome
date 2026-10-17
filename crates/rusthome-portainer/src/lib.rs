//! Portainer integration: container monitoring (cf. docs/integrations.md §1).
//!
//! **GET only**: a Portainer API token carries all the rights of its user, so
//! the client deliberately has no way to start, stop or recreate anything.
//! Containers are identified by **name**, not id: a recreated container keeps
//! its entities (HA left ~160 orphans behind, cf. inventaire/05).

mod readings;

use std::collections::HashMap;
use std::time::Duration;

use rusthome_core::Hub;
use rusthome_model::{Context, Device, EntityEntry, EntityId, EntityStatus};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use url::Url;

pub use readings::{Reading, readings};

pub const INTEGRATION: &str = "portainer";

#[derive(Debug, Clone)]
pub struct PortainerConfig {
    pub url: Url,
    pub token: String,
    /// Portainer serves a self-signed certificate by default.
    pub accept_invalid_certs: bool,
    pub poll_every: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum PortainerError {
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Portainer answered {status} on {path}")]
    Status { status: u16, path: String },
}

/// Read-only Portainer client.
pub struct Client {
    http: reqwest::Client,
    base: Url,
    token: String,
}

impl Client {
    pub fn new(cfg: &PortainerConfig) -> Result<Self, PortainerError> {
        let http = reqwest::Client::builder()
            .tls_danger_accept_invalid_certs(cfg.accept_invalid_certs)
            .timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self {
            http,
            base: cfg.url.clone(),
            token: cfg.token.clone(),
        })
    }

    /// The only request method: GET.
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, PortainerError> {
        let url = self.base.join(path).map_err(|_| PortainerError::Status {
            status: 0,
            path: path.into(),
        })?;
        let res = self
            .http
            .get(url)
            .header("X-API-Key", &self.token)
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(PortainerError::Status {
                status: res.status().as_u16(),
                path: path.into(),
            });
        }
        Ok(res.json().await?)
    }

    /// All containers of all environments, with inspect + stats.
    pub async fn snapshot(&self) -> Result<Vec<ContainerData>, PortainerError> {
        let endpoints: Vec<Value> = self.get("api/endpoints").await?;
        let mut out = Vec::new();
        for ep in endpoints {
            let Some(ep_id) = ep["Id"].as_i64() else {
                continue;
            };
            let base = format!("api/endpoints/{ep_id}/docker/containers");
            let list: Vec<Value> = self.get(&format!("{base}/json?all=1")).await?;
            for summary in list {
                let Some(id) = summary["Id"].as_str() else {
                    continue;
                };
                let inspect: Value = self.get(&format!("{base}/{id}/json")).await?;
                // Stats only exist for running containers.
                let stats = if summary["State"] == "running" {
                    self.get(&format!("{base}/{id}/stats?stream=false"))
                        .await
                        .ok()
                } else {
                    None
                };
                out.push(ContainerData {
                    endpoint: ep_id,
                    summary,
                    inspect,
                    stats,
                });
            }
        }
        Ok(out)
    }
}

#[derive(Debug, Clone)]
pub struct ContainerData {
    pub endpoint: i64,
    pub summary: Value,
    pub inspect: Value,
    pub stats: Option<Value>,
}

impl ContainerData {
    /// `/frigate` → `frigate`.
    #[must_use]
    pub fn name(&self) -> String {
        self.summary["Names"][0]
            .as_str()
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_owned()
    }
}

/// Applies Portainer data to the hub. Keeps the entity ids resolved so far.
pub struct Sync {
    hub: Hub,
    resolved: HashMap<String, EntityId>, // unique key `{container}_{key}` → entity_id
}

impl Sync {
    #[must_use]
    pub fn new(hub: Hub) -> Self {
        Self {
            hub,
            resolved: HashMap::new(),
        }
    }

    /// Returns the number of states written.
    pub fn apply(&mut self, containers: &[ContainerData]) -> usize {
        let mut n = 0;
        let mut seen = Vec::new();
        for c in containers {
            let name = c.name();
            if name.is_empty() {
                continue;
            }
            let device_id = format!("portainer-{}-{name}", c.endpoint);
            self.hub.registries.upsert_device(Device {
                id: device_id.clone(),
                integration: INTEGRATION.into(),
                name: name.clone(),
                manufacturer: Some("Docker".into()),
                model: c.inspect["Config"]["Image"].as_str().map(str::to_owned),
                sw_version: None,
                area_id: None,
                labels: vec![],
                disabled: false,
            });
            for r in readings(c) {
                let Some(id) = self.entity_for(&name, &r, &device_id) else {
                    continue;
                };
                let mut attrs = Map::new();
                attrs.insert("friendly_name".into(), format!("{name} {}", r.label).into());
                if let Some(u) = r.unit {
                    attrs.insert("unit_of_measurement".into(), u.into());
                }
                if let Some(dc) = r.device_class {
                    attrs.insert("device_class".into(), dc.into());
                }
                self.hub
                    .states
                    .set(id.clone(), r.state, attrs, Context::default());
                seen.push(id);
                n += 1;
            }
        }
        // Containers that disappeared: their entities become unavailable (not deleted).
        let gone: Vec<EntityId> = self
            .resolved
            .values()
            .filter(|id| !seen.contains(id))
            .cloned()
            .collect();
        for id in gone {
            if let Some(s) = self.hub.states.get(&id)
                && !s.is_unavailable()
            {
                self.hub
                    .states
                    .set(id, "unavailable", s.attributes.clone(), Context::default());
            }
        }
        n
    }

    /// Takes over the HA entity (`unique_id` `…_{container}_{key}`) or creates one.
    fn entity_for(&mut self, container: &str, r: &Reading, device_id: &str) -> Option<EntityId> {
        let unique = format!("{container}_{}", r.key);
        if let Some(id) = self.resolved.get(&unique) {
            return Some(id.clone());
        }
        let reg = &self.hub.registries;
        let suffix = format!("_{unique}");
        let existing = reg.entities().into_iter().find(|e| {
            (e.integration == "ha:portainer" || e.integration == INTEGRATION)
                && e.unique_id
                    .as_deref()
                    .is_some_and(|u| u.ends_with(&suffix) || u == unique)
        });
        let entity_id = match &existing {
            Some(e) => e.entity_id.clone(),
            None => EntityId::parse(&format!("{}.{}_{}", r.domain, slug(container), r.key)).ok()?,
        };
        reg.upsert_entity(EntityEntry {
            uid: existing
                .as_ref()
                .map_or_else(|| format!("portainer-{unique}"), |e| e.uid.clone()),
            entity_id: entity_id.clone(),
            integration: INTEGRATION.into(),
            unique_id: Some(
                existing
                    .as_ref()
                    .and_then(|e| e.unique_id.clone())
                    .unwrap_or(unique.clone()),
            ),
            device_id: Some(device_id.to_owned()),
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
        self.resolved.insert(unique, entity_id.clone());
        Some(entity_id)
    }
}

fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_owned()
}

/// Polls forever; errors are logged and retried at the next tick.
pub async fn run(hub: Hub, cfg: PortainerConfig) {
    let client = match Client::new(&cfg) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "Portainer: client creation failed");
            return;
        }
    };
    let mut sync = Sync::new(hub);
    let mut tick = tokio::time::interval(cfg.poll_every);
    loop {
        tick.tick().await;
        match client.snapshot().await {
            Ok(containers) => {
                let n = sync.apply(&containers);
                tracing::debug!(
                    containers = containers.len(),
                    states = n,
                    "Portainer: polled"
                );
            }
            Err(e) => tracing::warn!(error = %e, "Portainer: poll failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use rusthome_core::SystemClock;
    use serde_json::json;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    pub(crate) fn frigate() -> ContainerData {
        ContainerData {
            endpoint: 2,
            summary: json!({"Id": "562c5fc3f366aa", "Names": ["/frigate"], "State": "running", "Image": "ghcr.io/blakeblackshear/frigate:stable"}),
            inspect: json!({
                "Config": {"Image": "ghcr.io/blakeblackshear/frigate:stable"},
                "RestartCount": 3,
                "State": {"Status": "running", "OOMKilled": false, "StartedAt": "2026-10-13T04:33:00.5Z", "Health": {"Status": "healthy"}}
            }),
            stats: Some(json!({
                "cpu_stats": {"cpu_usage": {"total_usage": 2_000_000_000u64}, "system_cpu_usage": 20_000_000_000u64, "online_cpus": 4},
                "precpu_stats": {"cpu_usage": {"total_usage": 1_000_000_000u64}, "system_cpu_usage": 10_000_000_000u64},
                "memory_stats": {"usage": 600_000_000u64, "limit": 4_000_000_000u64, "stats": {"inactive_file": 100_000_000u64}}
            })),
        }
    }

    fn ha_entry(entity_id: &str, unique_id: &str) -> EntityEntry {
        EntityEntry {
            uid: format!("ha-{entity_id}"),
            entity_id: EntityId::parse(entity_id).unwrap(),
            integration: "ha:portainer".into(),
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

    #[test]
    fn takes_over_ha_entities_by_name_and_key() {
        let hub = Hub::new(Arc::new(SystemClock));
        hub.registries.upsert_entity(ha_entry(
            "binary_sensor.frigate_statut",
            "01KY5EK1PW8TN4C05XGKN1ZEFM_frigate_status",
        ));
        hub.registries.upsert_entity(ha_entry(
            "sensor.frigate_sante",
            "01KY5EK1PW8TN4C05XGKN1ZEFM_frigate_container_health_state",
        ));
        // The stack entity of the same name must not be confused with the container's.
        hub.registries.upsert_entity(ha_entry(
            "binary_sensor.frigate_statut_2",
            "01KY5EK1PW8TN4C05XGKN1ZEFM_20_stack_status",
        ));
        let mut sync = Sync::new(hub.clone());
        assert!(sync.apply(&[frigate()]) >= 10);

        let status = hub
            .states
            .get(&EntityId::parse("binary_sensor.frigate_statut").unwrap())
            .unwrap();
        assert_eq!(status.state, "on");
        assert_eq!(
            hub.states
                .get(&EntityId::parse("sensor.frigate_sante").unwrap())
                .unwrap()
                .state,
            "healthy"
        );
        let owner = hub
            .registries
            .entity_by_entity_id(&EntityId::parse("binary_sensor.frigate_statut").unwrap())
            .unwrap();
        assert_eq!(owner.integration, INTEGRATION);
        assert_eq!(
            hub.registries
                .entity_by_entity_id(&EntityId::parse("binary_sensor.frigate_statut_2").unwrap())
                .unwrap()
                .integration,
            "ha:portainer",
            "stack entity untouched"
        );
        // Unknown to HA: generated id
        assert_eq!(
            hub.states
                .get(&EntityId::parse("sensor.frigate_cpu_usage_total").unwrap())
                .unwrap()
                .state,
            "40.0"
        );
    }

    #[test]
    fn vanished_container_becomes_unavailable() {
        let hub = Hub::new(Arc::new(SystemClock));
        let mut sync = Sync::new(hub.clone());
        sync.apply(&[frigate()]);
        sync.apply(&[]);
        let id = EntityId::parse("binary_sensor.frigate_status").unwrap();
        assert_eq!(hub.states.get(&id).unwrap().state, "unavailable");
    }

    #[tokio::test]
    async fn client_only_uses_get_with_api_key() {
        let server = MockServer::start().await;
        let ok = |body: Value| ResponseTemplate::new(200).set_body_json(body);
        Mock::given(method("GET"))
            .and(path("/api/endpoints"))
            .and(header("X-API-Key", "tok"))
            .respond_with(ok(json!([{"Id": 2, "Name": "local"}])))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/endpoints/2/docker/containers/json"))
            .and(query_param("all", "1"))
            .respond_with(ok(json!([frigate().summary])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(
                "/api/endpoints/2/docker/containers/562c5fc3f366aa/json",
            ))
            .respond_with(ok(frigate().inspect))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(
                "/api/endpoints/2/docker/containers/562c5fc3f366aa/stats",
            ))
            .respond_with(ok(frigate().stats.unwrap()))
            .mount(&server)
            .await;

        let client = Client::new(&PortainerConfig {
            url: Url::parse(&server.uri()).unwrap(),
            token: "tok".into(),
            accept_invalid_certs: false,
            poll_every: Duration::from_secs(30),
        })
        .unwrap();
        let data = client.snapshot().await.unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].name(), "frigate");
        assert!(data[0].stats.is_some());
        // Every request received by the fake Portainer was a GET.
        let received = server.received_requests().await.unwrap();
        assert!(
            received
                .iter()
                .all(|r| r.method == wiremock::http::Method::GET)
        );
    }

    #[tokio::test]
    async fn bad_token_is_reported() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let client = Client::new(&PortainerConfig {
            url: Url::parse(&server.uri()).unwrap(),
            token: "wrong".into(),
            accept_invalid_certs: false,
            poll_every: Duration::from_secs(30),
        })
        .unwrap();
        assert!(matches!(
            client.snapshot().await,
            Err(PortainerError::Status { status: 401, .. })
        ));
    }
}
