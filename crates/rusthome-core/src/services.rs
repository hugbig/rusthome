//! Service registry: `domain.service` → async handler (cf. spec §4.3).
//!
//! Integrations register what they can do (`switch.turn_on`, `mqtt.publish`…);
//! the API and, later, the automation engine call services by name.

use std::collections::BTreeMap;
use std::sync::{Arc, PoisonError, RwLock};

use futures_util::future::BoxFuture;
use rusthome_model::{Context, EntityId};
use serde::Serialize;
use serde_json::Value;

/// One call: target data as given by the caller (HA-compatible shape:
/// `{"entity_id": "switch.x", ...}`).
#[derive(Debug, Clone)]
pub struct ServiceCall {
    pub domain: String,
    pub service: String,
    pub data: Value,
    pub context: Context,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ServiceError {
    #[error("unknown service {0}")]
    NotFound(String),
    #[error("invalid data: {0}")]
    InvalidData(String),
    /// The integration refuses to act (read-only mode during development).
    #[error("refused: {0}")]
    Refused(String),
    #[error("{0}")]
    Failed(String),
}

pub type ServiceHandler =
    Arc<dyn Fn(ServiceCall) -> BoxFuture<'static, Result<Value, ServiceError>> + Send + Sync>;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ServiceInfo {
    pub domain: String,
    pub service: String,
    /// Integration that registered it.
    pub integration: String,
}

struct Entry {
    integration: String,
    handler: ServiceHandler,
}

/// Several integrations may provide the same service (`switch.turn_on` for
/// Zigbee2MQTT and Hue…): calls are routed to the integration that owns each
/// target entity.
#[derive(Default)]
pub struct ServiceRegistry {
    services: RwLock<BTreeMap<(String, String), Vec<Entry>>>,
}

impl ServiceRegistry {
    /// Registers (or replaces) `domain.service` for one integration.
    pub fn register(
        &self,
        integration: &str,
        domain: &str,
        service: &str,
        handler: ServiceHandler,
    ) {
        let mut map = self
            .services
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let entries = map
            .entry((domain.to_owned(), service.to_owned()))
            .or_default();
        entries.retain(|e| e.integration != integration);
        entries.push(Entry {
            integration: integration.to_owned(),
            handler,
        });
    }

    #[must_use]
    pub fn list(&self) -> Vec<ServiceInfo> {
        let map = self.services.read().unwrap_or_else(PoisonError::into_inner);
        map.iter()
            .flat_map(|((d, s), entries)| {
                entries.iter().map(|e| ServiceInfo {
                    domain: d.clone(),
                    service: s.clone(),
                    integration: e.integration.clone(),
                })
            })
            .collect()
    }

    /// Calls a service. `owner_of` gives the integration owning an entity
    /// (from the entity registry); targets are split per owner.
    pub async fn call(
        &self,
        call: ServiceCall,
        owner_of: &(dyn Fn(&EntityId) -> Option<String> + Sync),
    ) -> Result<Value, ServiceError> {
        let name = format!("{}.{}", call.domain, call.service);
        let handlers: Vec<(String, ServiceHandler)> = {
            let map = self.services.read().unwrap_or_else(PoisonError::into_inner);
            map.get(&(call.domain.clone(), call.service.clone()))
                .map(|v| {
                    v.iter()
                        .map(|e| (e.integration.clone(), Arc::clone(&e.handler)))
                        .collect()
                })
                .unwrap_or_default()
        };
        tracing::info!(service = %name, data = %call.data, "service call");

        // Services without entity targets (`mqtt.publish`…): one provider expected.
        if call.data.get("entity_id").is_none() {
            return match handlers.as_slice() {
                [(_, h)] => h(call).await,
                [] => Err(ServiceError::NotFound(name)),
                _ => Err(ServiceError::InvalidData(format!(
                    "{name} needs an entity_id"
                ))),
            };
        }

        let mut groups: BTreeMap<String, Vec<EntityId>> = BTreeMap::new();
        for id in target_entity_ids(&call.data)? {
            let owner = owner_of(&id)
                .ok_or_else(|| ServiceError::InvalidData(format!("unknown entity {id}")))?;
            groups.entry(owner).or_default().push(id);
        }
        let mut results = Vec::new();
        for (owner, ids) in groups {
            let Some((_, handler)) = handlers.iter().find(|(i, _)| *i == owner) else {
                return Err(if owner.starts_with("ha:") {
                    ServiceError::Refused(format!(
                        "{} is still driven by Home Assistant (read-only mirror)",
                        ids[0]
                    ))
                } else if handlers.is_empty() {
                    ServiceError::NotFound(name)
                } else {
                    ServiceError::Refused(format!("{owner} does not provide {name}"))
                });
            };
            let mut data = call.data.clone();
            data["entity_id"] =
                Value::from(ids.iter().map(ToString::to_string).collect::<Vec<_>>());
            results.push(
                handler(ServiceCall {
                    data,
                    ..call.clone()
                })
                .await?,
            );
        }
        Ok(if results.len() == 1 {
            results.remove(0)
        } else {
            Value::from(results)
        })
    }
}

impl std::fmt::Debug for ServiceRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceRegistry")
            .field("services", &self.list().len())
            .finish()
    }
}

/// Extracts target entity ids from `data.entity_id` (string or list).
pub fn target_entity_ids(data: &Value) -> Result<Vec<rusthome_model::EntityId>, ServiceError> {
    let raw: Vec<&str> = match data.get("entity_id") {
        Some(Value::String(s)) => vec![s.as_str()],
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .ok_or_else(|| ServiceError::InvalidData("entity_id must be strings".into()))
            })
            .collect::<Result<_, _>>()?,
        _ => return Err(ServiceError::InvalidData("entity_id is required".into())),
    };
    raw.into_iter()
        .map(|s| {
            rusthome_model::EntityId::parse(s).map_err(|e| ServiceError::InvalidData(e.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(domain: &str, service: &str, data: Value) -> ServiceCall {
        ServiceCall {
            domain: domain.into(),
            service: service.into(),
            data,
            context: Context::default(),
        }
    }

    fn echo(tag: &'static str) -> ServiceHandler {
        Arc::new(move |c: ServiceCall| {
            Box::pin(async move { Ok(json!({"by": tag, "data": c.data})) })
        })
    }

    fn owner(id: &EntityId) -> Option<String> {
        match id.as_str() {
            "switch.zigbee" | "switch.zigbee2" => Some("z2m".into()),
            "switch.hue" => Some("hue".into()),
            "switch.mirror" => Some("ha:mqtt".into()),
            _ => None,
        }
    }

    #[tokio::test]
    async fn routes_to_the_owning_integration() {
        let reg = ServiceRegistry::default();
        reg.register("z2m", "switch", "turn_on", echo("z2m"));
        reg.register("hue", "switch", "turn_on", echo("hue"));
        assert_eq!(reg.list().len(), 2);

        let out = reg
            .call(
                call(
                    "switch",
                    "turn_on",
                    json!({"entity_id": ["switch.zigbee", "switch.zigbee2"]}),
                ),
                &owner,
            )
            .await
            .unwrap();
        assert_eq!(out["by"], "z2m");
        assert_eq!(
            out["data"]["entity_id"],
            json!(["switch.zigbee", "switch.zigbee2"])
        );

        let mixed = reg
            .call(
                call(
                    "switch",
                    "turn_on",
                    json!({"entity_id": ["switch.zigbee", "switch.hue"]}),
                ),
                &owner,
            )
            .await
            .unwrap();
        assert_eq!(mixed.as_array().unwrap().len(), 2, "one call per owner");
    }

    #[tokio::test]
    async fn explains_refusals() {
        let reg = ServiceRegistry::default();
        reg.register("z2m", "switch", "turn_on", echo("z2m"));
        let mirrored = reg
            .call(
                call("switch", "turn_on", json!({"entity_id": "switch.mirror"})),
                &owner,
            )
            .await
            .unwrap_err();
        assert!(matches!(mirrored, ServiceError::Refused(m) if m.contains("Home Assistant")));
        assert!(matches!(
            reg.call(
                call("switch", "turn_on", json!({"entity_id": "switch.ghost"})),
                &owner
            )
            .await
            .unwrap_err(),
            ServiceError::InvalidData(_)
        ));
        assert_eq!(
            reg.call(
                call("switch", "nope", json!({"entity_id": "switch.zigbee"})),
                &owner
            )
            .await
            .unwrap_err(),
            ServiceError::NotFound("switch.nope".into())
        );
    }

    #[tokio::test]
    async fn services_without_targets() {
        let reg = ServiceRegistry::default();
        reg.register("mqtt", "mqtt", "publish", echo("mqtt"));
        let out = reg
            .call(call("mqtt", "publish", json!({"topic": "t"})), &owner)
            .await
            .unwrap();
        assert_eq!(out["by"], "mqtt");
    }

    #[test]
    fn extracts_targets() {
        let one = target_entity_ids(&json!({"entity_id": "switch.a"})).unwrap();
        assert_eq!(one[0].as_str(), "switch.a");
        assert_eq!(
            target_entity_ids(&json!({"entity_id": ["light.a", "light.b"]}))
                .unwrap()
                .len(),
            2
        );
        assert!(target_entity_ids(&json!({})).is_err());
        assert!(target_entity_ids(&json!({"entity_id": "Bad"})).is_err());
        assert!(target_entity_ids(&json!({"entity_id": [1]})).is_err());
    }
}
