use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::EntityId;

/// Origin of a change: used for traces, logbook and permissions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct Context {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

/// State of one entity, with Home Assistant semantics (`unavailable`,
/// `unknown` are plain string states) so imported templates keep working.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct State {
    pub entity_id: EntityId,
    pub state: String,
    #[serde(default)]
    pub attributes: Map<String, Value>,
    pub last_changed: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    #[serde(default)]
    pub context: Context,
}

impl State {
    pub const UNAVAILABLE: &'static str = "unavailable";
    pub const UNKNOWN: &'static str = "unknown";

    #[must_use]
    pub fn is_unavailable(&self) -> bool {
        self.state == Self::UNAVAILABLE
    }

    /// True when `other` differs in state or attributes. Timestamps and
    /// context are ignored, as in Home Assistant.
    #[must_use]
    pub fn differs_from(&self, other: &State) -> bool {
        self.state != other.state || self.attributes != other.attributes
    }
}

/// Payload of a `state_changed` event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StateChanged {
    pub entity_id: EntityId,
    pub old_state: Option<State>,
    pub new_state: Option<State>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(state: &str, attrs: &Value) -> State {
        State {
            entity_id: EntityId::parse("sensor.x").unwrap(),
            state: state.into(),
            attributes: attrs.as_object().cloned().unwrap_or_default(),
            last_changed: Utc::now(),
            last_updated: Utc::now(),
            context: Context::default(),
        }
    }

    #[test]
    fn differs_ignores_timestamps() {
        let a = st("on", &serde_json::json!({"x": 1}));
        let mut b = a.clone();
        b.last_updated += chrono::Duration::seconds(5);
        assert!(!a.differs_from(&b));
        assert!(a.differs_from(&st("off", &serde_json::json!({"x": 1}))));
        assert!(a.differs_from(&st("on", &serde_json::json!({"x": 2}))));
    }

    #[test]
    fn deserializes_home_assistant_state_object() {
        let raw = serde_json::json!({
            "entity_id": "alarm_control_panel.alarmo",
            "state": "disarmed",
            "attributes": {"code_format": null},
            "last_changed": "2026-10-17T10:00:00.123456+00:00",
            "last_reported": "2026-10-17T10:00:00.123456+00:00",
            "last_updated": "2026-10-17T10:00:00.123456+00:00",
            "context": {"id": "01ABC", "parent_id": null, "user_id": null}
        });
        let s: State = serde_json::from_value(raw).unwrap();
        assert_eq!(s.state, "disarmed");
        assert!(!s.is_unavailable());
    }
}
