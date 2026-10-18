//! Client of the ESPHome Device Builder's multiplexed WebSocket API.
//!
//! One connection, opened on first use and reopened after a drop. Every
//! command carries a `message_id`; replies and streamed events come back
//! with the same id, possibly out of order. A stream ends with a result (or
//! an error) for its id, except when cancelled with `devices/stop_stream`.
//! The client sends no `Origin` header: the builder only checks origins of
//! browser requests.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::{Mutex, mpsc};
use tokio_tungstenite::tungstenite::Message;
use url::Url;

/// Longest wait for a plain command (`devices/list` rescans the folder,
/// `editor/validate_yaml` may take up to 30 s).
const CALL_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, thiserror::Error)]
pub enum BuilderError {
    #[error("ESPHome Builder unreachable: {0}")]
    Connect(String),
    #[error("connection to ESPHome Builder lost")]
    Disconnected,
    #[error("ESPHome Builder did not answer in time")]
    Timeout,
    /// Error returned by the builder (`not_found`, `invalid_args`…).
    #[error("{code}: {details}")]
    Command { code: String, details: String },
    #[error("ESPHome Builder: {0}")]
    Http(#[from] reqwest::Error),
    #[error("invalid ESPHome Builder URL: {0}")]
    Url(String),
}

impl BuilderError {
    /// Builder error code (`not_found`, `already_exists`…), if any.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Command { code, .. } => Some(code),
            _ => None,
        }
    }
}

/// A reply or an event for one command.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Result(Value),
    Error { code: String, details: String },
    Event { event: String, data: Value },
}

/// Classifies an incoming frame: `(message_id, frame)`, or `None` for the
/// server info sent at connection time.
#[must_use]
pub fn classify(v: &Value) -> Option<(String, Frame)> {
    let id = v.get("message_id")?.as_str()?.to_owned();
    let frame = if let Some(code) = v.get("error_code") {
        Frame::Error {
            code: code.as_str().unwrap_or("error").to_owned(),
            details: v["details"].as_str().unwrap_or_default().to_owned(),
        }
    } else if let Some(event) = v.get("event") {
        Frame::Event {
            event: event.as_str().unwrap_or_default().to_owned(),
            data: v.get("data").cloned().unwrap_or(Value::Null),
        }
    } else {
        Frame::Result(v.get("result")?.clone())
    };
    Some((id, frame))
}

type Routes = Arc<StdMutex<HashMap<String, mpsc::UnboundedSender<Frame>>>>;

#[derive(Clone)]
struct Conn {
    out: mpsc::UnboundedSender<String>,
    routes: Routes,
    /// Cleared by the reader when the socket closes.
    alive: Arc<AtomicBool>,
}

impl Conn {
    fn alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
}

pub struct Builder {
    ws_url: String,
    http_base: Url,
    http: reqwest::Client,
    conn: Mutex<Option<Conn>>,
    next_id: AtomicU64,
}

impl Builder {
    /// `base` is the builder's HTTP address, e.g. `http://esphome:6052`.
    pub fn new(base: &Url) -> Result<Self, BuilderError> {
        let mut ws = base.clone();
        let scheme = match base.scheme() {
            "http" => "ws",
            "https" => "wss",
            other => return Err(BuilderError::Url(other.to_owned())),
        };
        ws.set_scheme(scheme)
            .map_err(|()| BuilderError::Url(base.to_string()))?;
        ws.set_path("/ws");
        Ok(Self {
            ws_url: ws.to_string(),
            http_base: base.clone(),
            http: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .build()?,
            conn: Mutex::new(None),
            next_id: AtomicU64::new(1),
        })
    }

    async fn connection(&self) -> Result<Conn, BuilderError> {
        let mut slot = self.conn.lock().await;
        if let Some(c) = slot.as_ref().filter(|c| c.alive()) {
            return Ok(c.clone());
        }
        let (ws, _) = tokio::time::timeout(
            Duration::from_secs(10),
            tokio_tungstenite::connect_async(self.ws_url.as_str()),
        )
        .await
        .map_err(|_| BuilderError::Connect("timeout".into()))?
        .map_err(|e| BuilderError::Connect(e.to_string()))?;
        let (mut sink, mut source) = ws.split();
        let (out, mut rx) = mpsc::unbounded_channel::<String>();
        let routes: Routes = Arc::default();
        let alive = Arc::new(AtomicBool::new(true));

        tokio::spawn(async move {
            while let Some(text) = rx.recv().await {
                if sink.send(Message::text(text)).await.is_err() {
                    break;
                }
            }
        });
        let reader_routes = Arc::clone(&routes);
        let reader_alive = Arc::clone(&alive);
        tokio::spawn(async move {
            // Always draining: tungstenite answers the server's pings here.
            while let Some(Ok(msg)) = source.next().await {
                let text = match msg {
                    Message::Text(t) => t.to_string(),
                    Message::Binary(b) => String::from_utf8_lossy(&b).into_owned(),
                    Message::Close(_) => break,
                    _ => continue,
                };
                let Ok(v) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                if let Some((id, frame)) = classify(&v) {
                    let routes = reader_routes
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if let Some(tx) = routes.get(&id) {
                        let _ = tx.send(frame);
                    }
                }
            }
            tracing::debug!("ESPHome Builder connection closed");
            // The next call reconnects; every waiter is woken (its channel
            // closes).
            reader_alive.store(false, Ordering::Release);
            reader_routes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clear();
        });
        let conn = Conn { out, routes, alive };
        *slot = Some(conn.clone());
        Ok(conn)
    }

    async fn open(
        &self,
        command: &str,
        args: Value,
    ) -> Result<(Conn, String, mpsc::UnboundedReceiver<Frame>), BuilderError> {
        let conn = self.connection().await?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed).to_string();
        let (tx, rx) = mpsc::unbounded_channel();
        conn.routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id.clone(), tx);
        let msg = json!({ "command": command, "message_id": id, "args": args });
        if conn.out.send(msg.to_string()).is_err() {
            return Err(BuilderError::Disconnected);
        }
        Ok((conn, id, rx))
    }

    /// Sends a command and waits for its result.
    pub async fn call(&self, command: &str, args: Value) -> Result<Value, BuilderError> {
        let (conn, id, mut rx) = self.open(command, args).await?;
        let wait = async {
            loop {
                match rx.recv().await {
                    Some(Frame::Result(v)) => return Ok(v),
                    Some(Frame::Error { code, details }) => {
                        return Err(BuilderError::Command { code, details });
                    }
                    Some(Frame::Event { .. }) => {}
                    None => return Err(BuilderError::Disconnected),
                }
            }
        };
        let out = tokio::time::timeout(CALL_TIMEOUT, wait)
            .await
            .unwrap_or(Err(BuilderError::Timeout));
        unroute(&conn, &id);
        out
    }

    /// Starts a streaming command (`devices/logs`, `firmware/follow_job`…).
    pub async fn stream(&self, command: &str, args: Value) -> Result<Stream<'_>, BuilderError> {
        let (conn, id, rx) = self.open(command, args).await?;
        Ok(Stream {
            builder: self,
            conn,
            id,
            rx,
            done: false,
        })
    }

    /// Downloads a build artifact (`firmware.factory.bin`…) through a
    /// single-use token.
    pub async fn download(
        &self,
        configuration: &str,
        file: &str,
    ) -> Result<reqwest::Response, BuilderError> {
        let t = self
            .call(
                "firmware/download_token",
                json!({ "configuration": configuration, "file": file }),
            )
            .await?;
        let token = t["token"].as_str().unwrap_or_default();
        let mut url = self
            .http_base
            .join("/api/firmware/download")
            .map_err(|e| BuilderError::Url(e.to_string()))?;
        url.query_pairs_mut().append_pair("token", token);
        Ok(self.http.get(url).send().await?.error_for_status()?)
    }
}

fn unroute(conn: &Conn, id: &str) {
    conn.routes
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(id);
}

/// Events of one streaming command, until its final result.
pub struct Stream<'a> {
    builder: &'a Builder,
    conn: Conn,
    id: String,
    rx: mpsc::UnboundedReceiver<Frame>,
    done: bool,
}

impl Stream<'_> {
    /// Next event; `None` once the stream is over. The final result (or
    /// error) is returned as the last frame.
    pub async fn next(&mut self) -> Option<Frame> {
        if self.done {
            return None;
        }
        let frame = self.rx.recv().await;
        match &frame {
            // Subscriptions acknowledge with `{subscribed: true}` and go on.
            Some(Frame::Result(v)) if v["subscribed"] == true => {}
            Some(Frame::Result(_) | Frame::Error { .. }) | None => self.done = true,
            Some(Frame::Event { .. }) => {}
        }
        frame
    }

    /// Stops a registered stream (logs, validation, job following). The
    /// builder sends nothing more for it.
    pub async fn cancel(mut self) {
        if !self.done {
            self.done = true;
            let _ = self
                .builder
                .call("devices/stop_stream", json!({ "stream_id": self.id }))
                .await;
        }
    }
}

impl Drop for Stream<'_> {
    fn drop(&mut self) {
        unroute(&self.conn, &self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_classified_by_their_keys() {
        assert_eq!(
            classify(&json!({ "message_id": "3", "result": [1] })),
            Some(("3".into(), Frame::Result(json!([1]))))
        );
        assert_eq!(
            classify(&json!({ "message_id": "3", "error_code": "not_found", "details": "x.yaml" })),
            Some((
                "3".into(),
                Frame::Error {
                    code: "not_found".into(),
                    details: "x.yaml".into()
                }
            ))
        );
        assert_eq!(
            classify(&json!({ "message_id": "4", "event": "output", "data": "INFO" })),
            Some((
                "4".into(),
                Frame::Event {
                    event: "output".into(),
                    data: json!("INFO")
                }
            ))
        );
        // server info (no id) and unknown shapes are ignored
        assert_eq!(classify(&json!({ "server_version": "1.14.9" })), None);
        assert_eq!(classify(&json!({ "message_id": "5" })), None);
    }

    #[test]
    fn ws_url_is_derived_from_the_http_address() {
        let b = Builder::new(&Url::parse("http://esphome:6052").unwrap()).unwrap();
        assert_eq!(b.ws_url, "ws://esphome:6052/ws");
        assert!(Builder::new(&Url::parse("ftp://x").unwrap()).is_err());
    }
}
