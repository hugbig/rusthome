//! Read-only mirror of a Home Assistant instance over its WebSocket API
//! (migration phase P0, cf. docs/architecture.md §9).
//!
//! Read-only by construction: [`Command`] is a closed list of read commands.
//! There is deliberately no way to send `call_service` or any write
//! (AGENTS.md §2, rule 1).

mod client;
mod command;
pub mod rest;

use std::time::Duration;

use rusthome_core::Hub;
use rusthome_import::{ImportResult, import_registries};
use rusthome_model::{State, StateChanged};
use url::Url;

pub use client::{BridgeError, BridgeEvent, Connection, HaDashboard};
pub use command::Command;

#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Base URL of HA, e.g. `http://192.168.1.35:8123`.
    pub url: Url,
    pub token: String,
}

/// Reads every Lovelace dashboard of HA over a short-lived connection.
pub async fn fetch_dashboards(cfg: &BridgeConfig) -> Result<Vec<HaDashboard>, BridgeError> {
    let mut conn = Connection::connect(&cfg.url, &cfg.token).await?;
    conn.fetch_dashboards().await
}

/// Items of a HA to-do list over a short-lived connection.
pub async fn fetch_todo_items(
    cfg: &BridgeConfig,
    entity_id: &str,
) -> Result<Vec<serde_json::Value>, BridgeError> {
    let mut conn = Connection::connect(&cfg.url, &cfg.token).await?;
    conn.todo_items(entity_id).await
}

/// Energy panel settings of HA (short-lived connection, read-only).
pub async fn fetch_energy_prefs(cfg: &BridgeConfig) -> Result<serde_json::Value, BridgeError> {
    let mut conn = Connection::connect(&cfg.url, &cfg.token).await?;
    conn.energy_prefs().await
}

/// Long-term statistics of HA (short-lived connection, read-only).
pub async fn fetch_statistics(
    cfg: &BridgeConfig,
    ids: &[String],
    start: &str,
    end: &str,
    period: &str,
) -> Result<serde_json::Value, BridgeError> {
    let mut conn = Connection::connect(&cfg.url, &cfg.token).await?;
    conn.statistics(ids, start, end, period, &["change", "sum"])
        .await
}

/// Weather forecast of a HA weather entity (short-lived subscription).
pub async fn fetch_forecast(
    cfg: &BridgeConfig,
    entity_id: &str,
    kind: &str,
) -> Result<serde_json::Value, BridgeError> {
    let mut conn = Connection::connect(&cfg.url, &cfg.token).await?;
    tokio::time::timeout(
        Duration::from_secs(10),
        conn.weather_forecast(entity_id, kind),
    )
    .await
    .map_err(|_| BridgeError::Protocol("forecast: no answer within 10 s".into()))?
}

/// Browses a HA media source (short-lived connection, read-only).
pub async fn browse_media(
    cfg: &BridgeConfig,
    id: Option<&str>,
) -> Result<serde_json::Value, BridgeError> {
    let mut conn = Connection::connect(&cfg.url, &cfg.token).await?;
    conn.media_browse(id).await
}

/// Resolves a HA media to its signed URL (short-lived connection, read-only).
pub async fn resolve_media(cfg: &BridgeConfig, id: &str) -> Result<serde_json::Value, BridgeError> {
    let mut conn = Connection::connect(&cfg.url, &cfg.token).await?;
    conn.media_resolve(id).await
}

/// Summary of one synchronisation, logged and exposed to the UI.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct SyncReport {
    pub entities: usize,
    pub devices: usize,
    pub states: usize,
    pub skipped: usize,
    /// Entities left alone because a native integration (Zigbee2MQTT…) owns them.
    pub owned_natively: usize,
}

/// Connects, mirrors registries and states, then follows `state_changed`
/// forever, reconnecting with capped exponential backoff.
/// `ready` flips to `true` after the first snapshot: native integrations wait
/// for it to find the HA `unique_id`s they take over.
pub async fn run(hub: Hub, cfg: BridgeConfig, ready: tokio::sync::watch::Sender<bool>) {
    let mut backoff = Duration::from_secs(1);
    loop {
        let started = tokio::time::Instant::now();
        match run_once(&hub, &cfg, &ready).await {
            Ok(()) => tracing::warn!("HA bridge: connection closed, reconnecting"),
            Err(e) => tracing::warn!(error = %e, "HA bridge: error, reconnecting in {backoff:?}"),
        }
        // After a session that lasted, reconnect fast again: badges scanned
        // while waiting would be lost (code review n° 43).
        if started.elapsed() > Duration::from_secs(60) {
            backoff = Duration::from_secs(1);
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(60));
    }
}

async fn run_once(
    hub: &Hub,
    cfg: &BridgeConfig,
    ready: &tokio::sync::watch::Sender<bool>,
) -> Result<(), BridgeError> {
    let mut conn = Connection::connect(&cfg.url, &cfg.token).await?;
    let snapshot = conn.fetch_snapshot().await?;
    let imported = import_registries(&snapshot);
    let report = apply(hub, &imported, snapshot.states);
    tracing::info!(?report, "HA bridge: snapshot applied");
    ready.send_replace(true);

    conn.subscribe_state_changed().await?;
    conn.subscribe_tag_scanned().await?;
    while let Some(ev) = conn.next_event().await? {
        match ev {
            BridgeEvent::StateChanged(change) => apply_change(hub, &imported, change),
            // Badge scans are republished on RustHome's bus for `tag` triggers.
            BridgeEvent::TagScanned(data) => hub.bus.publish(rusthome_core::Event::Custom {
                event_type: "tag_scanned".into(),
                data,
            }),
        }
    }
    Ok(())
}

/// Pushes an import result and the initial states into the hub.
pub fn apply(hub: &Hub, imported: &ImportResult, states: Vec<State>) -> SyncReport {
    for f in &imported.floors {
        hub.registries.upsert_floor(f.clone());
    }
    for a in &imported.areas {
        hub.registries.upsert_area(a.clone());
    }
    for l in &imported.labels {
        hub.registries.upsert_label(l.clone());
    }
    for d in &imported.devices {
        hub.registries.upsert_device(d.clone());
    }
    let mut owned = 0;
    for e in &imported.entities {
        if natively_owned(hub, &e.entity_id) {
            owned += 1;
            continue;
        }
        hub.registries.upsert_entity(e.clone());
    }
    let mut applied = 0;
    for s in states {
        if is_skipped(imported, s.entity_id.as_str()) {
            continue;
        }
        // A natively owned entity keeps its own state; HA only seeds it until
        // the native integration reports (a Zigbee sensor may stay silent for
        // an hour after a restart).
        if natively_owned(hub, &s.entity_id) && hub.states.get(&s.entity_id).is_some() {
            continue;
        }
        hub.states.set_external(s);
        applied += 1;
    }
    SyncReport {
        entities: imported.entities.len(),
        devices: imported.devices.len(),
        states: applied,
        skipped: imported.skipped.len(),
        owned_natively: owned,
    }
}

fn apply_change(hub: &Hub, imported: &ImportResult, change: StateChanged) {
    if is_skipped(imported, change.entity_id.as_str()) || natively_owned(hub, &change.entity_id) {
        return;
    }
    match change.new_state {
        Some(s) => {
            hub.states.set_external(s);
        }
        None => {
            hub.states.remove(&change.entity_id);
        }
    }
}

/// An entity taken over by a native integration is no longer mirrored
/// (owner table of spec §9: one driver per entity).
fn natively_owned(hub: &Hub, entity_id: &rusthome_model::EntityId) -> bool {
    hub.registries
        .entity_by_entity_id(entity_id)
        .is_some_and(|e| !e.integration.starts_with("ha:"))
}

fn is_skipped(imported: &ImportResult, entity_id: &str) -> bool {
    imported.skipped.iter().any(|(id, _)| id == entity_id)
}

/// Convenience for the binary: shares the hub with the bridge task.
#[must_use]
pub fn spawn(
    hub: &Hub,
    cfg: BridgeConfig,
) -> (
    tokio::task::JoinHandle<()>,
    tokio::sync::watch::Receiver<bool>,
) {
    let (tx, rx) = tokio::sync::watch::channel(false);
    let hub = hub.clone();
    (tokio::spawn(async move { run(hub, cfg, tx).await }), rx)
}
