use std::sync::Arc;

use rusthome_model::StateChanged;
use serde::Serialize;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistryKind {
    Floor,
    Area,
    Label,
    Device,
    Entity,
}

// Events always travel inside an `Arc`, so the size gap between variants
// costs nothing per subscriber.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event_type", content = "data", rename_all = "snake_case")]
pub enum Event {
    StateChanged(StateChanged),
    RegistryUpdated { kind: RegistryKind, id: String },
}

/// Broadcast bus. Slow subscribers lag and miss events rather than blocking
/// the core; they must handle `RecvError::Lagged` (cf. spec §4.3).
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Arc<Event>>,
}

impl EventBus {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn publish(&self, event: Event) {
        // No subscriber is not an error.
        let _ = self.tx.send(Arc::new(event));
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Event>> {
        self.tx.subscribe()
    }
}
