//! Read-only probe of the production broker (dev tool).
//! Reads `.env`, subscribes to a topic filter (2nd argument, default
//! `zigbee2mqtt/#`), never publishes, and writes what it saw to the path given
//! as first argument (JSON lines).
//! Only `bridge/devices` and device state topics are kept (no `bridge/info`,
//! which carries the Z2M configuration).

use std::io::Write as _;
use std::time::Duration;

use rusthome_mqtt::{Mqtt, MqttConfig, run, topic_matches};

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt().with_env_filter("info").init();
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "probe.jsonl".into());
    let cfg = MqttConfig {
        url: url::Url::parse(&var("RUSTHOME_MQTT_URL").unwrap_or_default())
            .unwrap_or_else(|e| panic!("RUSTHOME_MQTT_URL: {e}")),
        username: var("RUSTHOME_MQTT_USERNAME"),
        password: var("RUSTHOME_MQTT_PASSWORD"),
        read_only: true, // always, whatever .env says
        client_id: "rusthome-probe".into(),
    };
    let (mqtt, eventloop) = Mqtt::new(&cfg).unwrap_or_else(|e| panic!("{e}"));
    let mut rx = mqtt.messages();
    tokio::spawn(run(mqtt.clone(), eventloop));
    let filter = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "zigbee2mqtt/#".into());
    mqtt.subscribe(&filter)
        .await
        .unwrap_or_else(|e| panic!("{e}"));

    let mut file = std::fs::File::create(&out).unwrap_or_else(|e| panic!("{out}: {e}"));
    let deadline = tokio::time::sleep(Duration::from_secs(20));
    tokio::pin!(deadline);
    let mut n = 0;
    loop {
        tokio::select! {
            () = &mut deadline => break,
            msg = rx.recv() => {
                let Ok(m) = msg else { continue };
                let keep = m.topic == "zigbee2mqtt/bridge/devices"
                    || (!topic_matches("zigbee2mqtt/bridge/#", &m.topic) && !m.topic.ends_with("/availability"))
                    || !m.topic.starts_with("zigbee2mqtt/");
                if !keep { continue }
                let payload: serde_json::Value = serde_json::from_slice(&m.payload)
                    .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&m.payload).chars().take(200).collect()));
                writeln!(file, "{}", serde_json::json!({"topic": m.topic, "retain": m.retain, "payload": payload})).ok();
                n += 1;
            }
        }
    }
    eprintln!("{n} messages written to {out}");
}
