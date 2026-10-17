//! Shared MQTT client (cf. spec §5.2, integration `mqtt`).
//!
//! **Read-only mode** (default during development, AGENTS.md §2): the client
//! subscribes but [`Mqtt::publish`] refuses everything, so no frame can reach a
//! production device. Incoming messages are fanned out on an in-process
//! broadcast channel; consumers (Zigbee2MQTT…) filter by topic.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use bytes::Bytes;
use futures_util::future::BoxFuture;
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use tokio::sync::broadcast;
use url::Url;

#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub url: Url,
    pub username: Option<String>,
    pub password: Option<String>,
    pub read_only: bool,
    pub client_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub topic: String,
    pub payload: Bytes,
    pub retain: bool,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MqttError {
    #[error("MQTT is read-only: refused to publish on {0}")]
    ReadOnly(String),
    #[error("invalid MQTT URL: {0}")]
    Url(String),
    #[error("MQTT client: {0}")]
    Client(String),
}

/// Where integrations send commands. Implemented by [`Mqtt`]; faked in tests.
pub trait Publisher: Send + Sync {
    fn publish(&self, topic: String, payload: String) -> BoxFuture<'_, Result<(), MqttError>>;
}

impl Publisher for Mqtt {
    fn publish(&self, topic: String, payload: String) -> BoxFuture<'_, Result<(), MqttError>> {
        Box::pin(async move { Mqtt::publish(self, &topic, payload, false).await })
    }
}

/// Largest accepted MQTT packet (1 MiB).
pub const MAX_PACKET_SIZE: usize = 1024 * 1024;

/// Cheap to clone; all clones share the connection.
#[derive(Clone)]
pub struct Mqtt {
    client: AsyncClient,
    read_only: bool,
    tx: broadcast::Sender<Arc<Message>>,
    /// Re-sent after every reconnection (clean session).
    subscriptions: Arc<Mutex<Vec<String>>>,
}

impl Mqtt {
    /// Builds the client and its event loop; call [`run`] on the latter.
    pub fn new(cfg: &MqttConfig) -> Result<(Self, EventLoop), MqttError> {
        let opts = options(cfg)?;
        let (client, eventloop) = AsyncClient::new(opts, 256);
        let (tx, _) = broadcast::channel(4096);
        Ok((
            Self {
                client,
                read_only: cfg.read_only,
                tx,
                subscriptions: Arc::default(),
            },
            eventloop,
        ))
    }

    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Receives every message of every subscription; filter with [`topic_matches`].
    #[must_use]
    pub fn messages(&self) -> broadcast::Receiver<Arc<Message>> {
        self.tx.subscribe()
    }

    pub async fn subscribe(&self, filter: &str) -> Result<(), MqttError> {
        self.subscriptions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(filter.to_owned());
        self.client
            .subscribe(filter, QoS::AtLeastOnce)
            .await
            .map_err(|e| MqttError::Client(e.to_string()))
    }

    pub async fn publish(
        &self,
        topic: &str,
        payload: impl Into<Vec<u8>>,
        retain: bool,
    ) -> Result<(), MqttError> {
        if self.read_only {
            tracing::warn!(topic, "MQTT read-only: publish refused");
            return Err(MqttError::ReadOnly(topic.to_owned()));
        }
        self.client
            .publish(topic, QoS::AtLeastOnce, retain, payload.into())
            .await
            .map_err(|e| MqttError::Client(e.to_string()))
    }

    /// Injects a message as if received (tests and simulations).
    pub fn inject(&self, msg: Message) {
        let _ = self.tx.send(Arc::new(msg));
    }

    async fn resubscribe(&self) {
        let filters = self
            .subscriptions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        for f in filters {
            if let Err(e) = self.client.subscribe(&f, QoS::AtLeastOnce).await {
                tracing::warn!(filter = %f, error = %e, "MQTT resubscribe failed");
            }
        }
    }
}

fn options(cfg: &MqttConfig) -> Result<MqttOptions, MqttError> {
    let port = cfg.url.port().unwrap_or(1883);
    let host = cfg
        .url
        .host_str()
        .ok_or_else(|| MqttError::Url(cfg.url.to_string()))?;
    if cfg.url.scheme() != "mqtt" {
        return Err(MqttError::Url(format!(
            "unsupported scheme {}",
            cfg.url.scheme()
        )));
    }
    let mut opts = MqttOptions::new(cfg.client_id.clone(), host, port);
    opts.set_keep_alive(Duration::from_secs(30))
        .set_clean_session(true)
        // zigbee2mqtt/bridge/devices is ~28 KB for 17 devices: the 10 KB default drops
        // the connection in a loop (seen against the real broker).
        .set_max_packet_size(MAX_PACKET_SIZE, MAX_PACKET_SIZE);
    if let (Some(u), Some(p)) = (&cfg.username, &cfg.password) {
        opts.set_credentials(u.clone(), p.clone());
    }
    Ok(opts)
}

/// Drives the connection forever: reconnects with backoff, resubscribes,
/// forwards publications to [`Mqtt::messages`].
pub async fn run(mqtt: Mqtt, mut eventloop: EventLoop) {
    let mut backoff = Duration::from_secs(1);
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::ConnAck(_))) => {
                tracing::info!(read_only = mqtt.read_only, "MQTT connected");
                backoff = Duration::from_secs(1);
                mqtt.resubscribe().await;
            }
            Ok(Event::Incoming(Packet::Publish(p))) => {
                let _ = mqtt.tx.send(Arc::new(Message {
                    topic: p.topic.clone(),
                    payload: p.payload,
                    retain: p.retain,
                }));
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "MQTT connection error, retrying in {backoff:?}");
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(60));
            }
        }
    }
}

/// MQTT wildcard matching (`+` one level, `#` the rest).
#[must_use]
pub fn topic_matches(filter: &str, topic: &str) -> bool {
    let mut f = filter.split('/');
    let mut t = topic.split('/');
    loop {
        match (f.next(), t.next()) {
            (Some("#"), _) | (None, None) => return true,
            (Some(a), Some(b)) if a == "+" || a == b => {}
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(read_only: bool) -> MqttConfig {
        MqttConfig {
            url: Url::parse("mqtt://127.0.0.1:1").unwrap(),
            username: None,
            password: None,
            read_only,
            client_id: "test".into(),
        }
    }

    #[tokio::test]
    async fn read_only_refuses_every_publish() {
        let (mqtt, _eventloop) = Mqtt::new(&cfg(true)).unwrap();
        let err = mqtt
            .publish("zigbee2mqtt/Prise/set", r#"{"state":"ON"}"#, false)
            .await
            .unwrap_err();
        assert_eq!(err, MqttError::ReadOnly("zigbee2mqtt/Prise/set".into()));
    }

    #[tokio::test]
    async fn writable_client_queues_publish() {
        // Not connected: the request is only queued, which is enough to prove it is allowed.
        let (mqtt, _eventloop) = Mqtt::new(&cfg(false)).unwrap();
        assert!(mqtt.publish("t", "x", false).await.is_ok());
    }

    #[test]
    fn rejects_other_schemes() {
        let mut c = cfg(true);
        c.url = Url::parse("http://x").unwrap();
        assert!(Mqtt::new(&c).is_err());
    }

    #[test]
    fn accepts_large_bridge_payloads() {
        assert_eq!(
            options(&cfg(true)).unwrap().max_packet_size(),
            MAX_PACKET_SIZE
        );
    }

    #[test]
    fn wildcards() {
        assert!(topic_matches("zigbee2mqtt/#", "zigbee2mqtt/bridge/devices"));
        assert!(topic_matches("zigbee2mqtt/+", "zigbee2mqtt/Prise"));
        assert!(!topic_matches("zigbee2mqtt/+", "zigbee2mqtt/Prise/set"));
        assert!(topic_matches("a/+/c", "a/b/c"));
        assert!(!topic_matches("a/b", "a/b/c"));
        assert!(topic_matches("a/b", "a/b"));
    }

    #[tokio::test]
    async fn injected_messages_reach_subscribers() {
        let (mqtt, _eventloop) = Mqtt::new(&cfg(true)).unwrap();
        let mut rx = mqtt.messages();
        mqtt.inject(Message {
            topic: "a".into(),
            payload: Bytes::from_static(b"1"),
            retain: false,
        });
        assert_eq!(rx.recv().await.unwrap().topic, "a");
    }
}
