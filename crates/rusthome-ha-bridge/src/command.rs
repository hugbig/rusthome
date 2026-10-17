use serde_json::{Value, json};

/// The only messages the bridge can send to Home Assistant. Every variant is
/// a read. Adding a write here requires a validated migration phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    GetStates,
    SubscribeStateChanged,
    FloorList,
    AreaList,
    LabelList,
    DeviceList,
    EntityList,
}

impl Command {
    pub const ALL: [Command; 7] = [
        Command::GetStates,
        Command::SubscribeStateChanged,
        Command::FloorList,
        Command::AreaList,
        Command::LabelList,
        Command::DeviceList,
        Command::EntityList,
    ];

    #[must_use]
    pub fn message_type(self) -> &'static str {
        match self {
            Command::GetStates => "get_states",
            Command::SubscribeStateChanged => "subscribe_events",
            Command::FloorList => "config/floor_registry/list",
            Command::AreaList => "config/area_registry/list",
            Command::LabelList => "config/label_registry/list",
            Command::DeviceList => "config/device_registry/list",
            Command::EntityList => "config/entity_registry/list",
        }
    }

    #[must_use]
    pub fn to_message(self, id: u64) -> Value {
        let mut msg = json!({ "id": id, "type": self.message_type() });
        if self == Command::SubscribeStateChanged {
            msg["event_type"] = json!("state_changed");
        }
        msg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guard rail: no command may look like a write.
    #[test]
    fn every_command_is_a_read() {
        for cmd in Command::ALL {
            let t = cmd.message_type();
            assert!(
                t == "get_states" || t == "subscribe_events" || t.ends_with("/list"),
                "{t} is not an allowed read command"
            );
            assert!(
                !t.contains("call_service")
                    && !t.contains("update")
                    && !t.contains("remove")
                    && !t.contains("create")
            );
        }
    }

    #[test]
    fn subscribe_only_targets_state_changed() {
        let msg = Command::SubscribeStateChanged.to_message(3);
        assert_eq!(msg["event_type"], "state_changed");
        assert_eq!(msg["id"], 3);
    }
}
