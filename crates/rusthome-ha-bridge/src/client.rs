use futures_util::{SinkExt, StreamExt};
use rusthome_import::{HaArea, HaDevice, HaEntity, HaFloor, HaLabel, HaSnapshot};
use rusthome_model::{State, StateChanged};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use url::Url;

use crate::Command;

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("invalid Home Assistant URL: {0}")]
    Url(String),
    #[error("websocket: {0}")]
    Ws(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("authentication refused by Home Assistant")]
    AuthInvalid,
    #[error("unexpected message: {0}")]
    Protocol(String),
    #[error("command {command} failed: {message}")]
    CommandFailed {
        command: &'static str,
        message: String,
    },
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Events received from HA subscriptions.
// Short-lived values handled one by one: the size gap does not matter.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum BridgeEvent {
    StateChanged(StateChanged),
    /// `{"tag_id": …, "device_id": …}`
    TagScanned(Value),
}

/// One HA dashboard: metadata (`url_path`, `title`, `icon`…) and config.
#[derive(Debug, Clone)]
pub struct HaDashboard {
    pub meta: Value,
    pub config: Value,
}

/// An authenticated connection to the HA WebSocket API.
pub struct Connection {
    ws: Ws,
    next_id: u64,
    subscription: Option<u64>,
    tag_subscription: Option<u64>,
}

/// `http://host:8123` → `ws://host:8123/api/websocket` (https → wss).
pub(crate) fn websocket_url(base: &Url) -> Result<Url, BridgeError> {
    let mut url = base.clone();
    let scheme = match base.scheme() {
        "http" | "ws" => "ws",
        "https" | "wss" => "wss",
        other => return Err(BridgeError::Url(format!("unsupported scheme {other}"))),
    };
    url.set_scheme(scheme)
        .map_err(|()| BridgeError::Url(base.to_string()))?;
    url.set_path("/api/websocket");
    Ok(url)
}

impl Connection {
    pub async fn connect(base: &Url, token: &str) -> Result<Self, BridgeError> {
        let (ws, _) = tokio_tungstenite::connect_async(websocket_url(base)?.as_str()).await?;
        let mut conn = Self {
            ws,
            next_id: 1,
            subscription: None,
            tag_subscription: None,
        };
        conn.authenticate(token).await?;
        Ok(conn)
    }

    async fn authenticate(&mut self, token: &str) -> Result<(), BridgeError> {
        let first = self.recv().await?;
        if first["type"] != "auth_required" {
            return Err(BridgeError::Protocol(first.to_string()));
        }
        self.ws
            .send(Message::text(
                json!({ "type": "auth", "access_token": token }).to_string(),
            ))
            .await?;
        let reply = self.recv().await?;
        match reply["type"].as_str() {
            Some("auth_ok") => Ok(()),
            Some("auth_invalid") => Err(BridgeError::AuthInvalid),
            _ => Err(BridgeError::Protocol(reply.to_string())),
        }
    }

    async fn recv(&mut self) -> Result<Value, BridgeError> {
        loop {
            let msg = self
                .ws
                .next()
                .await
                .ok_or_else(|| BridgeError::Protocol("connection closed".into()))??;
            match msg {
                Message::Text(text) => return Ok(serde_json::from_str(text.as_str())?),
                Message::Close(_) => return Err(BridgeError::Protocol("connection closed".into())),
                _ => {} // ping/pong handled by tungstenite, binary unused by HA
            }
        }
    }

    async fn send(&mut self, cmd: Command) -> Result<u64, BridgeError> {
        self.send_with(cmd, &serde_json::Map::new()).await
    }

    async fn send_with(
        &mut self,
        cmd: Command,
        params: &serde_json::Map<String, Value>,
    ) -> Result<u64, BridgeError> {
        let id = self.next_id;
        self.next_id += 1;
        self.ws
            .send(Message::text(cmd.to_message_with(id, params).to_string()))
            .await?;
        Ok(id)
    }

    /// Sends a read command and waits for its result.
    async fn call<T: DeserializeOwned>(&mut self, cmd: Command) -> Result<T, BridgeError> {
        self.call_with(cmd, &serde_json::Map::new()).await
    }

    async fn call_with<T: DeserializeOwned>(
        &mut self,
        cmd: Command,
        params: &serde_json::Map<String, Value>,
    ) -> Result<T, BridgeError> {
        let id = self.send_with(cmd, params).await?;
        loop {
            let msg = self.recv().await?;
            if msg["id"] != id || msg["type"] != "result" {
                continue;
            }
            if msg["success"] != true {
                return Err(BridgeError::CommandFailed {
                    command: cmd.message_type(),
                    message: msg["error"]["message"]
                        .as_str()
                        .unwrap_or("unknown error")
                        .to_owned(),
                });
            }
            return Ok(serde_json::from_value(msg["result"].clone())?);
        }
    }

    pub async fn fetch_snapshot(&mut self) -> Result<HaSnapshot, BridgeError> {
        let raw_states: Vec<Value> = self.call(Command::GetStates).await?;
        Ok(HaSnapshot {
            floors: self.call::<Vec<HaFloor>>(Command::FloorList).await?,
            areas: self.call::<Vec<HaArea>>(Command::AreaList).await?,
            labels: self.call::<Vec<HaLabel>>(Command::LabelList).await?,
            devices: self.call::<Vec<HaDevice>>(Command::DeviceList).await?,
            entities: self.call::<Vec<HaEntity>>(Command::EntityList).await?,
            states: parse_states(raw_states),
        })
    }

    /// Every Lovelace dashboard: the default one (`url_path` `lovelace`)
    /// then the storage dashboards. Dashboards without a stored config
    /// (auto-generated strategies) are skipped.
    pub async fn fetch_dashboards(&mut self) -> Result<Vec<HaDashboard>, BridgeError> {
        let mut metas = vec![json!({ "url_path": "lovelace", "title": "Aperçu" })];
        for m in self.call::<Vec<Value>>(Command::LovelaceDashboards).await? {
            // the default dashboard may be listed too (YAML mode): keep one
            if m["url_path"] == "lovelace" {
                metas[0] = m;
            } else {
                metas.push(m);
            }
        }
        let mut out = Vec::new();
        for meta in metas {
            let mut params = serde_json::Map::new();
            if meta["url_path"] != "lovelace" {
                params.insert("url_path".into(), meta["url_path"].clone());
            }
            match self
                .call_with::<Value>(Command::LovelaceConfig, &params)
                .await
            {
                Ok(config) if config.get("views").is_some() => {
                    out.push(HaDashboard { meta, config });
                }
                Ok(_) => tracing::debug!(url_path = %meta["url_path"], "dashboard without views"),
                Err(e) => {
                    tracing::debug!(url_path = %meta["url_path"], error = %e, "dashboard skipped");
                }
            }
        }
        Ok(out)
    }

    /// Items of a to-do list (`todo/item/list`, read-only).
    pub async fn todo_items(&mut self, entity_id: &str) -> Result<Vec<Value>, BridgeError> {
        let mut params = serde_json::Map::new();
        params.insert("entity_id".into(), entity_id.into());
        let res: Value = self.call_with(Command::TodoItems, &params).await?;
        Ok(res["items"].as_array().cloned().unwrap_or_default())
    }

    /// Energy panel settings (`energy/get_prefs`, read-only).
    pub async fn energy_prefs(&mut self) -> Result<Value, BridgeError> {
        self.call_with(Command::EnergyPrefs, &serde_json::Map::new())
            .await
    }

    /// Long-term statistics: `period` is `5minute`, `hour`, `day`, `week`
    /// or `month`; `types` such as `change`, `sum`, `mean`.
    pub async fn statistics(
        &mut self,
        ids: &[String],
        start: &str,
        end: &str,
        period: &str,
        types: &[&str],
    ) -> Result<Value, BridgeError> {
        let mut p = serde_json::Map::new();
        p.insert("statistic_ids".into(), json!(ids));
        p.insert("start_time".into(), start.into());
        p.insert("end_time".into(), end.into());
        p.insert("period".into(), period.into());
        p.insert("types".into(), json!(types));
        self.call_with(Command::Statistics, &p).await
    }

    /// Weather forecast (`daily`, `hourly` or `twice_daily`): subscribes and
    /// returns the first event. The connection is meant to be dropped after.
    pub async fn weather_forecast(
        &mut self,
        entity_id: &str,
        kind: &str,
    ) -> Result<Value, BridgeError> {
        let mut p = serde_json::Map::new();
        p.insert("entity_id".into(), entity_id.into());
        p.insert("forecast_type".into(), kind.into());
        let id = self.send_with(Command::WeatherForecast, &p).await?;
        loop {
            let msg = self.recv().await?;
            if msg["id"] != id {
                continue;
            }
            if msg["type"] == "result" {
                if msg["success"] != true {
                    return Err(BridgeError::CommandFailed {
                        command: Command::WeatherForecast.message_type(),
                        message: msg["error"]["message"]
                            .as_str()
                            .unwrap_or("unknown error")
                            .to_owned(),
                    });
                }
                continue;
            }
            if msg["type"] == "event" {
                return Ok(msg["event"]["forecast"].clone());
            }
        }
    }

    /// One level of a media source tree (root when `id` is `None`).
    pub async fn media_browse(&mut self, id: Option<&str>) -> Result<Value, BridgeError> {
        let mut p = serde_json::Map::new();
        if let Some(id) = id {
            p.insert("media_content_id".into(), id.into());
        }
        self.call_with(Command::MediaBrowse, &p).await
    }

    /// `{url, mime_type}` of a playable media (the URL is signed by HA).
    pub async fn media_resolve(&mut self, id: &str) -> Result<Value, BridgeError> {
        let mut p = serde_json::Map::new();
        p.insert("media_content_id".into(), id.into());
        p.insert("expires".into(), 3600.into());
        self.call_with(Command::MediaResolve, &p).await
    }

    pub async fn subscribe_state_changed(&mut self) -> Result<&mut Self, BridgeError> {
        let id = self.send(Command::SubscribeStateChanged).await?;
        self.subscription = Some(id);
        Ok(self)
    }

    /// Also follows NFC badge scans (read-only subscription).
    pub async fn subscribe_tag_scanned(&mut self) -> Result<&mut Self, BridgeError> {
        let id = self.send(Command::SubscribeTagScanned).await?;
        self.tag_subscription = Some(id);
        Ok(self)
    }

    /// Next `state_changed` from the subscription; `None` when HA closes.
    /// Other events are skipped (use [`Self::next_event`] to get them).
    pub async fn next_state_changed(&mut self) -> Result<Option<StateChanged>, BridgeError> {
        loop {
            match self.next_event().await? {
                None => return Ok(None),
                Some(BridgeEvent::StateChanged(c)) => return Ok(Some(c)),
                Some(BridgeEvent::TagScanned(_)) => {}
            }
        }
    }

    /// Next event of any subscription; `None` when HA closes.
    pub async fn next_event(&mut self) -> Result<Option<BridgeEvent>, BridgeError> {
        if self.subscription.is_none() && self.tag_subscription.is_none() {
            return Err(BridgeError::Protocol("not subscribed".into()));
        }
        loop {
            let msg = match self.recv().await {
                Ok(m) => m,
                Err(BridgeError::Protocol(p)) if p == "connection closed" => return Ok(None),
                Err(e) => return Err(e),
            };
            if msg["type"] != "event" {
                continue;
            }
            if self.tag_subscription.is_some_and(|t| msg["id"] == t) {
                return Ok(Some(BridgeEvent::TagScanned(msg["event"]["data"].clone())));
            }
            if self.subscription.is_none_or(|s| msg["id"] != s) {
                continue;
            }
            match serde_json::from_value::<StateChanged>(msg["event"]["data"].clone()) {
                Ok(change) => return Ok(Some(BridgeEvent::StateChanged(change))),
                Err(e) => {
                    tracing::debug!(error = %e, "HA bridge: unparsable state_changed ignored");
                }
            }
        }
    }
}

/// Parses states one by one so a single odd entity cannot drop the snapshot.
fn parse_states(raw: Vec<Value>) -> Vec<State> {
    raw.into_iter()
        .filter_map(|v| match serde_json::from_value::<State>(v) {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::debug!(error = %e, "HA bridge: state ignored");
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_websocket_url() {
        let u = websocket_url(&Url::parse("http://192.168.1.35:8123").unwrap()).unwrap();
        assert_eq!(u.as_str(), "ws://192.168.1.35:8123/api/websocket");
        let s = websocket_url(&Url::parse("https://ha.example.org/").unwrap()).unwrap();
        assert_eq!(s.as_str(), "wss://ha.example.org/api/websocket");
        assert!(websocket_url(&Url::parse("ftp://x").unwrap()).is_err());
    }

    #[test]
    fn bad_states_are_skipped_not_fatal() {
        let states = parse_states(vec![
            json!({"entity_id": "light.a", "state": "on", "attributes": {}, "last_changed": "2026-10-17T10:00:00+00:00", "last_updated": "2026-10-17T10:00:00+00:00"}),
            json!({"entity_id": "garbage"}),
        ]);
        assert_eq!(states.len(), 1);
    }
}
