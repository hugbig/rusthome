//! Read-only mirror of a Home Assistant instance over its WebSocket API
//! (migration phase P0, cf. docs/architecture.md §9).
//!
//! Read-only by construction: [`Command`] is a closed list of read commands.
//! There is deliberately no way to send `call_service` or any write
//! (AGENTS.md §2, rule 1).

mod client;
mod command;

use std::time::Duration;

use rusthome_core::Hub;
use rusthome_import::{ImportResult, import_registries};
use rusthome_model::{State, StateChanged};
use url::Url;

pub use client::{BridgeError, Connection};
pub use command::Command;

#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Base URL of HA, e.g. `http://192.168.1.35:8123`.
    pub url: Url,
    pub token: String,
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
        match run_once(&hub, &cfg, &ready).await {
            Ok(()) => tracing::warn!("HA bridge: connection closed, reconnecting"),
            Err(e) => tracing::warn!(error = %e, "HA bridge: error, reconnecting in {backoff:?}"),
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

    let events = conn.subscribe_state_changed().await?;
    while let Some(change) = events.next_state_changed().await? {
        apply_change(hub, &imported, change);
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
        if !is_skipped(imported, s.entity_id.as_str()) && !natively_owned(hub, &s.entity_id) {
            hub.states.set_external(s);
            applied += 1;
        }
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
