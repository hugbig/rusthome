//! ESPHome for RustHome (spec §5.3 bis): the projects (YAML, compilation,
//! flashing, logs) are handled by the official ESPHome Device Builder
//! container, driven through its multiplexed WebSocket API (`/ws`).
//!
//! RustHome never sends `subscribe_events`: that command turns on the
//! builder's sweeps (ping and Native API probes of the devices). Device
//! lists and states are read on demand instead.

pub mod builder;
pub mod policy;

pub use builder::{Builder, BuilderError, Frame, Stream};
pub use policy::Policy;
