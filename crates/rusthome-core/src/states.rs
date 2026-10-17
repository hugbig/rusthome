use std::sync::Arc;

use dashmap::DashMap;
use rusthome_model::{Context, EntityId, State, StateChanged};
use serde_json::{Map, Value};

use crate::{Clock, Event, EventBus};

pub struct StateMachine {
    states: DashMap<EntityId, Arc<State>>,
    bus: EventBus,
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for StateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateMachine")
            .field("entities", &self.states.len())
            .finish_non_exhaustive()
    }
}

impl StateMachine {
    #[must_use]
    pub fn new(bus: EventBus, clock: Arc<dyn Clock>) -> Self {
        Self {
            states: DashMap::new(),
            bus,
            clock,
        }
    }

    #[must_use]
    pub fn get(&self, entity_id: &EntityId) -> Option<Arc<State>> {
        self.states.get(entity_id).map(|s| Arc::clone(&s))
    }

    /// All states, sorted by `entity_id` for stable output.
    #[must_use]
    pub fn all(&self) -> Vec<Arc<State>> {
        let mut v: Vec<_> = self.states.iter().map(|e| Arc::clone(e.value())).collect();
        v.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
        v
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.states.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Sets a state computed locally. `last_changed` only moves when the
    /// state string changes; nothing is emitted when nothing changed.
    pub fn set(
        &self,
        entity_id: EntityId,
        state: impl Into<String>,
        attributes: Map<String, Value>,
        context: Context,
    ) -> Option<Arc<State>> {
        let now = self.clock.now();
        let state = state.into();
        let old = self.get(&entity_id);
        let last_changed = match &old {
            Some(o) if o.state == state => o.last_changed,
            _ => now,
        };
        let new = State {
            entity_id,
            state,
            attributes,
            last_changed,
            last_updated: now,
            context,
        };
        self.store(old, new)
    }

    /// Stores a state received as-is from a source that owns its timestamps
    /// (e.g. the Home Assistant mirror).
    pub fn set_external(&self, new: State) -> Option<Arc<State>> {
        let old = self.get(&new.entity_id);
        self.store(old, new)
    }

    pub fn remove(&self, entity_id: &EntityId) -> Option<Arc<State>> {
        let (_, old) = self.states.remove(entity_id)?;
        self.bus.publish(Event::StateChanged(StateChanged {
            entity_id: entity_id.clone(),
            old_state: Some((*old).clone()),
            new_state: None,
        }));
        Some(old)
    }

    fn store(&self, old: Option<Arc<State>>, new: State) -> Option<Arc<State>> {
        if old.as_ref().is_some_and(|o| !o.differs_from(&new)) {
            return None;
        }
        let new = Arc::new(new);
        self.states.insert(new.entity_id.clone(), Arc::clone(&new));
        self.bus.publish(Event::StateChanged(StateChanged {
            entity_id: new.entity_id.clone(),
            old_state: old.map(|o| (*o).clone()),
            new_state: Some((*new).clone()),
        }));
        Some(new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManualClock;
    use chrono::{Duration, TimeZone, Utc};

    fn setup() -> (
        StateMachine,
        Arc<ManualClock>,
        tokio::sync::broadcast::Receiver<Arc<Event>>,
    ) {
        let clock = Arc::new(ManualClock::new(
            Utc.with_ymd_and_hms(2026, 10, 17, 12, 0, 0).unwrap(),
        ));
        let bus = EventBus::new(16);
        let rx = bus.subscribe();
        (StateMachine::new(bus, clock.clone()), clock, rx)
    }

    fn id(s: &str) -> EntityId {
        EntityId::parse(s).unwrap()
    }

    #[test]
    fn set_emits_state_changed_once() {
        let (sm, _, mut rx) = setup();
        assert!(
            sm.set(id("light.salon"), "on", Map::new(), Context::default())
                .is_some()
        );
        assert!(
            matches!(&*rx.try_recv().unwrap(), Event::StateChanged(c) if c.old_state.is_none())
        );
        // Same state and attributes: no event
        assert!(
            sm.set(id("light.salon"), "on", Map::new(), Context::default())
                .is_none()
        );
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn last_changed_moves_only_when_state_changes() {
        let (sm, clock, _rx) = setup();
        let first = sm
            .set(id("sensor.t"), "20", Map::new(), Context::default())
            .unwrap();
        clock.advance(Duration::minutes(1));
        let mut attrs = Map::new();
        attrs.insert("unit".into(), "°C".into());
        let attr_change = sm
            .set(id("sensor.t"), "20", attrs, Context::default())
            .unwrap();
        assert_eq!(attr_change.last_changed, first.last_changed);
        assert!(attr_change.last_updated > first.last_updated);
        clock.advance(Duration::minutes(1));
        let state_change = sm
            .set(id("sensor.t"), "21", Map::new(), Context::default())
            .unwrap();
        assert!(state_change.last_changed > first.last_changed);
    }

    #[test]
    fn remove_emits_event_with_no_new_state() {
        let (sm, _, mut rx) = setup();
        sm.set(id("switch.x"), "off", Map::new(), Context::default());
        let _ = rx.try_recv();
        assert!(sm.remove(&id("switch.x")).is_some());
        assert!(
            matches!(&*rx.try_recv().unwrap(), Event::StateChanged(c) if c.new_state.is_none())
        );
        assert!(sm.is_empty());
    }

    #[test]
    fn all_is_sorted() {
        let (sm, _, _rx) = setup();
        for e in ["sensor.b", "light.a", "sensor.a"] {
            sm.set(id(e), "1", Map::new(), Context::default());
        }
        let ids: Vec<_> = sm.all().iter().map(|s| s.entity_id.to_string()).collect();
        assert_eq!(ids, ["light.a", "sensor.a", "sensor.b"]);
    }
}
