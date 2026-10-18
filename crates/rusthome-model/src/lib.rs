//! Domain types shared by every `RustHome` crate.
//!
//! Identifiers keep the Home Assistant `domain.object_id` format so imported
//! automations, dashboards and ESP32 firmwares keep working unchanged
//! (cf. docs/architecture.md §4.1).

mod entity_id;
pub mod refs;
mod registry;
mod state;

pub use entity_id::{EntityId, EntityIdError};
pub use registry::{Area, Device, EntityEntry, EntityStatus, Floor, Label};
pub use state::{Context, State, StateChanged};
