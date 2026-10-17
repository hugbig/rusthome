//! Core runtime: clock, event bus, state machine, registries and services
//! (cf. docs/architecture.md §4).

mod bus;
mod clock;
mod registries;
mod services;
mod states;

use std::sync::Arc;

pub use bus::{Event, EventBus, RegistryKind};
pub use clock::{Clock, ManualClock, SystemClock};
pub use registries::Registries;
pub use services::{
    ServiceCall, ServiceError, ServiceHandler, ServiceInfo, ServiceRegistry, target_entity_ids,
};
pub use states::StateMachine;

/// Everything an integration or the API needs to talk to the core.
#[derive(Clone)]
pub struct Hub {
    pub clock: Arc<dyn Clock>,
    pub bus: EventBus,
    pub states: Arc<StateMachine>,
    pub registries: Arc<Registries>,
    pub services: Arc<ServiceRegistry>,
}

impl Hub {
    /// Calls a service, routing each target entity to the integration that owns it.
    pub async fn call_service(&self, call: ServiceCall) -> Result<serde_json::Value, ServiceError> {
        let registries = Arc::clone(&self.registries);
        let owner = move |id: &rusthome_model::EntityId| {
            registries.entity_by_entity_id(id).map(|e| e.integration)
        };
        self.services.call(call, &owner).await
    }

    #[must_use]
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self::with_bus_capacity(clock, 1024)
    }

    /// Small capacities let tests exercise lagging subscribers.
    #[must_use]
    pub fn with_bus_capacity(clock: Arc<dyn Clock>, capacity: usize) -> Self {
        let bus = EventBus::new(capacity);
        Self {
            states: Arc::new(StateMachine::new(bus.clone(), clock.clone())),
            registries: Arc::new(Registries::new(bus.clone())),
            services: Arc::new(ServiceRegistry::default()),
            clock,
            bus,
        }
    }
}
