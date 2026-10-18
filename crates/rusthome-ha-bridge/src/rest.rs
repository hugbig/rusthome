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

/// Current image of a camera entity: `GET /api/camera_proxy/<entity_id>`.
/// Returns `(content type, bytes)`.
pub async fn camera_snapshot(
    cfg: &BridgeConfig,
    entity_id: &str,
) -> Result<(String, Vec<u8>), BridgeError> {
    let path = format!("/api/camera_proxy/{entity_id}");
    let url = format!("{}{path}", cfg.url.as_str().trim_end_matches('/'));
    let res = reqwest::Client::new()
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
    let content_type = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_owned();
    let bytes = res
        .bytes()
        .await
        .map_err(|e| BridgeError::Protocol(format!("GET {path}: {e}")))?;
    Ok((content_type, bytes.to_vec()))
}

/// Events of a calendar between two instants:
/// `GET /api/calendars/<entity_id>?start=…&end=…` (read-only).
pub async fn calendar_events(
    cfg: &BridgeConfig,
    entity_id: &str,
    start: &str,
    end: &str,
) -> Result<Vec<Value>, BridgeError> {
    let http = reqwest::Client::new();
    let path = format!("/api/calendars/{entity_id}");
    let mut url = cfg
        .url
        .join(&path)
        .map_err(|e| BridgeError::Url(e.to_string()))?;
    url.query_pairs_mut()
        .append_pair("start", start)
        .append_pair("end", end);
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

/// HA paths a media file may come from (signed media, thumbnails, Frigate
/// media of the HA integration, camera proxy). Nothing else of the HA API is
/// reachable through the media proxy.
pub const MEDIA_PREFIXES: [&str; 5] = [
    "/media/",
    "/api/media_source_thumbnail",
    "/api/frigate/",
    "/api/camera_proxy/",
    "/api/image_proxy/",
];

/// Is `path` (with its query) a HA media path, without traversal?
#[must_use]
pub fn is_media_path(path: &str) -> bool {
    let Ok(base) = url::Url::parse("http://ha.invalid") else {
        return false;
    };
    media_url(&base, path).is_some()
}

/// The URL of a HA media path, checked on its **normalized** form: the URL
/// parser resolves `..`, `%2e%2e`, `.%2E`… exactly as it will be sent, so a
/// raw-string check alone could be bypassed (code review 2026-10-18, n° 1).
fn media_url(base: &url::Url, path: &str) -> Option<url::Url> {
    let raw = path.split('?').next().unwrap_or_default();
    if !path.starts_with('/') || raw.contains("..") || raw.contains("//") || raw.contains('\\') {
        return None;
    }
    let url = base.join(path).ok()?;
    let normalized = url.path();
    let lower = normalized.to_ascii_lowercase();
    let clean = MEDIA_PREFIXES.iter().any(|m| normalized.starts_with(m))
        && url.host_str() == base.host_str()
        && url.port() == base.port()
        && !lower.contains("%2e")
        && !lower.contains("%2f")
        && !lower.contains("%5c");
    clean.then_some(url)
}

/// Opens a HA media file (GET), forwarding `Range` for video seeking.
pub async fn open_media(
    cfg: &BridgeConfig,
    path: &str,
    range: Option<&str>,
) -> Result<reqwest::Response, BridgeError> {
    let url = media_url(&cfg.url, path)
        .ok_or_else(|| BridgeError::Protocol(format!("not a media path: {path}")))?;
    let mut req = reqwest::Client::new().get(url).bearer_auth(&cfg.token);
    if let Some(r) = range {
        req = req.header(reqwest::header::RANGE, r);
    }
    let res = req
        .send()
        .await
        .map_err(|e| BridgeError::Protocol(format!("GET {path}: {e}")))?;
    if !res.status().is_success() {
        return Err(BridgeError::CommandFailed {
            command: "GET",
            message: format!("{path}: HTTP {}", res.status()),
        });
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn media_paths_are_checked_after_normalization() {
        assert!(is_media_path("/media/local/20261001-salon.mp4?authSig=x"));
        assert!(is_media_path("/media/local/ma%20video.mp4"));
        assert!(is_media_path("/api/camera_proxy/camera.chambre?token=t"));
        for bad in [
            "/media/%2e%2e/api/states",
            "/media/%2E%2e/api/config",
            "/media/.%2e/api/error_log",
            "/media/local/%2e%2e/%2e%2e/api/hassio/backups",
            "/media/../api/states",
            "/media//api/states",
            "/media/local%2f..%2fapi",
            "/api/states",
            "//evil.example/media/x",
            "http://evil.example/media/x",
            "/media/a\\..\\api",
        ] {
            assert!(!is_media_path(bad), "{bad}");
        }
    }

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

    #[tokio::test]
    async fn camera_snapshot_is_a_get() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/camera_proxy/camera.salon"))
            .and(header("authorization", "Bearer t"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "image/jpeg")
                    .set_body_bytes(vec![0xff, 0xd8]),
            )
            .mount(&server)
            .await;
        let cfg = BridgeConfig {
            url: url::Url::parse(&server.uri()).unwrap(),
            token: "t".into(),
        };
        let (ct, bytes) = camera_snapshot(&cfg, "camera.salon").await.unwrap();
        assert_eq!((ct.as_str(), bytes), ("image/jpeg", vec![0xff, 0xd8]));
        assert!(camera_snapshot(&cfg, "camera.absente").await.is_err());
    }

    #[tokio::test]
    async fn calendar_events_are_a_get_with_range() {
        use wiremock::matchers::query_param;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/calendars/calendar.perso"))
            .and(query_param("start", "2026-10-17T00:00:00Z"))
            .and(query_param("end", "2026-10-20T00:00:00Z"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    json!([{"summary": "Piscine", "start": {"date": "2026-10-18"}}]),
                ),
            )
            .mount(&server)
            .await;
        let cfg = BridgeConfig {
            url: url::Url::parse(&server.uri()).unwrap(),
            token: "t".into(),
        };
        let ev = calendar_events(
            &cfg,
            "calendar.perso",
            "2026-10-17T00:00:00Z",
            "2026-10-20T00:00:00Z",
        )
        .await
        .unwrap();
        assert_eq!(ev[0]["summary"], "Piscine");
    }

    #[test]
    fn media_paths_are_restricted() {
        assert!(is_media_path("/media/local/film.mp4?authSig=x"));
        assert!(is_media_path("/api/frigate/notifications/abc/snapshot.jpg"));
        assert!(!is_media_path("/api/states"));
        assert!(!is_media_path("/api/config/core"));
        assert!(!is_media_path("/media/../api/states"));
        assert!(!is_media_path("https://evil.example/x"));
    }
}
