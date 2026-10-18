//! The client against a simulated ESPHome Builder (no real container).

// Test harness: panicking on unexpected input is the desired behaviour.
#![allow(clippy::unwrap_used)]

use futures_util::{SinkExt, StreamExt};
use rusthome_esphome::{Builder, BuilderError, Frame};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use url::Url;

/// Serves one WebSocket connection with the builder's behaviour for a few
/// commands; returns the address.
async fn mock() -> Url {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            tokio::spawn(async move {
                let ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
                let (mut tx, mut rx) = ws.split();
                let hello = json!({ "server_version": "1.14.9", "esphome_version": "2026.9.1" });
                tx.send(Message::text(hello.to_string())).await.unwrap();
                let mut log_stream: Option<String> = None;
                while let Some(Ok(Message::Text(t))) = rx.next().await {
                    let m: Value = serde_json::from_str(&t).unwrap();
                    let id = m["message_id"].as_str().unwrap().to_owned();
                    let reply = |v: Value| Message::text(v.to_string());
                    match m["command"].as_str().unwrap() {
                        "devices/list" => {
                            let r = json!({ "message_id": id, "result": {
                                "configured": [{ "name": "ir-box", "configuration": "ir-box.yaml" }],
                                "importable": [] } });
                            tx.send(reply(r)).await.unwrap();
                        }
                        "devices/get_config" => {
                            let r = json!({ "message_id": id, "error_code": "not_found",
                                            "details": "x.yaml" });
                            tx.send(reply(r)).await.unwrap();
                        }
                        "devices/logs" => {
                            log_stream = Some(id.clone());
                            for line in ["[I] boot", "[D] wifi"] {
                                let e =
                                    json!({ "message_id": id, "event": "output", "data": line });
                                tx.send(reply(e)).await.unwrap();
                            }
                        }
                        "devices/validate" => {
                            let e = json!({ "message_id": id, "event": "output", "data": "OK" });
                            tx.send(reply(e)).await.unwrap();
                            let e = json!({ "message_id": id, "event": "result",
                                            "data": { "success": true, "code": 0 } });
                            tx.send(reply(e)).await.unwrap();
                            tx.send(reply(json!({ "message_id": id, "result": null })))
                                .await
                                .unwrap();
                        }
                        "devices/stop_stream" => {
                            let ok = log_stream.as_deref() == m["args"]["stream_id"].as_str();
                            let r = json!({ "message_id": id, "result": { "cancelled": ok } });
                            tx.send(reply(r)).await.unwrap();
                        }
                        "crash" => return, // drops the connection
                        _ => {}
                    }
                }
            });
        }
    });
    Url::parse(&format!("http://{addr}")).unwrap()
}

#[tokio::test]
async fn calls_return_results_and_builder_errors() {
    let b = Builder::new(&mock().await).unwrap();
    let list = b.call("devices/list", json!({})).await.unwrap();
    assert_eq!(list["configured"][0]["configuration"], "ir-box.yaml");
    let err = b
        .call("devices/get_config", json!({ "configuration": "x.yaml" }))
        .await
        .unwrap_err();
    assert_eq!(err.code(), Some("not_found"));
}

#[tokio::test]
async fn streams_end_with_their_result_or_are_cancelled() {
    let b = Builder::new(&mock().await).unwrap();
    let mut s = b
        .stream(
            "devices/validate",
            json!({ "configuration": "ir-box.yaml" }),
        )
        .await
        .unwrap();
    let mut frames = Vec::new();
    while let Some(f) = s.next().await {
        frames.push(f);
    }
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[2], Frame::Result(Value::Null));
    drop(s);

    let mut logs = b
        .stream("devices/logs", json!({ "configuration": "ir-box.yaml" }))
        .await
        .unwrap();
    assert!(matches!(logs.next().await, Some(Frame::Event { data, .. }) if data == "[I] boot"));
    assert!(matches!(logs.next().await, Some(Frame::Event { .. })));
    logs.cancel().await;
    // the connection is still usable afterwards
    assert!(b.call("devices/list", json!({})).await.is_ok());
}

#[tokio::test]
async fn a_dropped_connection_fails_waiters_then_reconnects() {
    let b = Builder::new(&mock().await).unwrap();
    let err = b.call("crash", json!({})).await.unwrap_err();
    assert!(matches!(err, BuilderError::Disconnected), "{err}");
    assert!(b.call("devices/list", json!({})).await.is_ok());
}

#[tokio::test]
async fn an_absent_builder_is_reported() {
    let b = Builder::new(&Url::parse("http://127.0.0.1:9").unwrap()).unwrap();
    assert!(matches!(
        b.call("devices/list", json!({})).await,
        Err(BuilderError::Connect(_))
    ));
}
