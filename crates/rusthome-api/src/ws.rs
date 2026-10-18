//! Live WebSocket: a snapshot of all states, then every `state_changed`.

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use futures_util::SinkExt;
use rusthome_core::Event;
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;

use crate::ApiState;

pub(crate) async fn handler(ws: WebSocketUpgrade, State(state): State<ApiState>) -> Response {
    ws.on_upgrade(move |socket| serve(socket, state))
}

async fn serve(mut socket: WebSocket, state: ApiState) {
    // Subscribe before the snapshot so no change falls in between.
    let mut rx = state.hub.bus.subscribe();
    let snapshot = json!({
        "type": "snapshot",
        "states": state.hub.states.all().iter().map(|s| &**s).collect::<Vec<_>>(),
    });
    if socket
        .send(Message::text(snapshot.to_string()))
        .await
        .is_err()
    {
        return;
    }
    loop {
        tokio::select! {
            event = rx.recv() => match event {
                Ok(ev) => {
                    if let Event::StateChanged(change) = &*ev {
                        let msg = json!({ "type": "state_changed", "data": change });
                        if socket.send(Message::text(msg.to_string())).await.is_err() {
                            return;
                        }
                    }
                }
                Err(RecvError::Lagged(n)) => {
                    // The client must resync: tell it explicitly.
                    let msg = json!({ "type": "resync_required", "missed": n });
                    if socket.send(Message::text(msg.to_string())).await.is_err() {
                        return;
                    }
                }
                Err(RecvError::Closed) => return,
            },
            incoming = socket.recv() => match incoming {
                // Complete the closing handshake, otherwise browsers report an error.
                Some(Ok(Message::Close(_))) => {
                    // tungstenite queued the close reply: flush it before dropping.
                    let _ = socket.flush().await;
                    return;
                }
                Some(Err(_)) | None => return,
                Some(Ok(_)) => {} // M1: client messages are ignored
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures_util::StreamExt;
    use rusthome_core::{Hub, SystemClock};
    use rusthome_model::{Context, EntityId};
    use serde_json::{Map, Value};

    use crate::{ApiState, AuthMode, router};

    #[tokio::test]
    async fn sends_snapshot_then_changes() {
        let hub = Hub::new(Arc::new(SystemClock));
        hub.states.set(
            EntityId::parse("light.salon").unwrap(),
            "off",
            Map::new(),
            Context::default(),
        );
        let app = router(
            ApiState {
                hub: hub.clone(),
                recorder: None,
                auth: AuthMode::Dev,
                automations: None,
                dashboards: None,
                system: serde_json::Value::Null,
                display_dashboard: None,
                zigbee: None,
                frigate: None,
            },
            None,
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await });

        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/api/v1/ws"))
            .await
            .unwrap();
        let first: Value =
            serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(first["type"], "snapshot");
        assert_eq!(first["states"][0]["state"], "off");

        hub.states.set(
            EntityId::parse("light.salon").unwrap(),
            "on",
            Map::new(),
            Context::default(),
        );
        let next: Value =
            serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(next["type"], "state_changed");
        assert_eq!(next["data"]["new_state"]["state"], "on");

        // Client-initiated close gets a proper close frame back.
        ws.close(None).await.unwrap();
        let reply = ws.next().await.unwrap().unwrap();
        assert!(reply.is_close(), "expected close frame, got {reply:?}");
    }
}
