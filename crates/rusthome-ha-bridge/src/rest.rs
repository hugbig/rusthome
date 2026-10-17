//! Read-only REST calls to HA (GET only), for what the WebSocket API does
//! not expose: automation configurations.

use serde_json::Value;

use crate::{BridgeConfig, BridgeError};

/// One automation as configured in HA.
#[derive(Debug, Clone)]
pub struct HaAutomation {
    /// `automation.tireuse`
    pub entity_id: String,
    pub enabled: bool,
    pub config: Value,
}

async fn get(http: &reqwest::Client, cfg: &BridgeConfig, path: &str) -> Result<Value, BridgeError> {
    let url = format!("{}{path}", cfg.url.as_str().trim_end_matches('/'));
    let res = http
        .get(url)
        .bearer_auth(&cfg.token)
        .send()
        .await
        .map_err(|e| BridgeError::Protocol(format!("GET {path}: {e}")))?;
    if !res.status().is_success() {
        return Err(BridgeError::CommandFailed {
            command: "GET",
            message: format!("{path}: HTTP {}", res.status()),
        });
    }
    res.json()
        .await
        .map_err(|e| BridgeError::Protocol(format!("GET {path}: {e}")))
}

/// Every automation of HA: `GET /api/states`, then
/// `GET /api/config/automation/config/<id>` for each. Automations without
/// stored config (YAML-only or orphans) are skipped.
pub async fn fetch_automations(cfg: &BridgeConfig) -> Result<Vec<HaAutomation>, BridgeError> {
    let http = reqwest::Client::new();
    let states = get(&http, cfg, "/api/states").await?;
    let mut out = Vec::new();
    for s in states.as_array().into_iter().flatten() {
        let Some(entity_id) = s["entity_id"]
            .as_str()
            .filter(|e| e.starts_with("automation."))
        else {
            continue;
        };
        let Some(id) = s["attributes"]["id"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| s["attributes"]["id"].as_i64().map(|n| n.to_string()))
        else {
            continue;
        };
        match get(&http, cfg, &format!("/api/config/automation/config/{id}")).await {
            Ok(config) => out.push(HaAutomation {
                entity_id: entity_id.to_owned(),
                enabled: s["state"] == "on",
                config,
            }),
            Err(e) => {
                tracing::warn!(entity_id, error = %e, "HA automation config not readable, skipped");
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn reads_automations_with_get_only() {
        let server = MockServer::start().await;
        let states = json!([
            {"entity_id": "automation.tireuse", "state": "on", "attributes": {"id": "1690000000001"}},
            {"entity_id": "automation.orphan", "state": "unavailable", "attributes": {}},
            {"entity_id": "light.salon", "state": "on", "attributes": {}}
        ]);
        Mock::given(method("GET"))
            .and(path("/api/states"))
            .and(header("authorization", "Bearer t"))
            .respond_with(ResponseTemplate::new(200).set_body_json(states))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/config/automation/config/1690000000001"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"id": "1690000000001", "alias": "Tireuse"})),
            )
            .mount(&server)
            .await;

        let cfg = BridgeConfig {
            url: url::Url::parse(&server.uri()).unwrap(),
            token: "t".into(),
        };
        let autos = fetch_automations(&cfg).await.unwrap();
        assert_eq!(autos.len(), 1);
        assert_eq!(autos[0].entity_id, "automation.tireuse");
        assert!(autos[0].enabled);
        assert_eq!(autos[0].config["alias"], "Tireuse");
        assert!(
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .all(|r| r.method == wiremock::http::Method::GET)
        );
    }
}
