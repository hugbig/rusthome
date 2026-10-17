//! Importers from Home Assistant formats into the `RustHome` model.
//!
//! M1 covers the registries (floors, areas, labels, devices, entities) as
//! returned by the HA WebSocket API (`config/*_registry/list`), with the dead
//! weight filtered out (cf. inventaire/05-mort.md §7).

mod ha;
mod registries;

pub use ha::{HaArea, HaDevice, HaEntity, HaFloor, HaLabel, HaSnapshot};
pub use registries::{ImportResult, SkipReason, import_registries};
