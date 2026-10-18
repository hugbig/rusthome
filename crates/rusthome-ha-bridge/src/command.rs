use serde_json::{Value, json};

/// The only messages the bridge can send to Home Assistant. Every variant is
/// a read. Adding a write here requires a validated migration phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    GetStates,
    SubscribeStateChanged,
    /// NFC badge scans, to feed `tag` triggers of RustHome automations.
    SubscribeTagScanned,
    FloorList,
    AreaList,
    LabelList,
    DeviceList,
    EntityList,
    /// Storage dashboards (metadata only).
    LovelaceDashboards,
    /// One dashboard's config (`url_path` given with [`Command::to_message_with`]).
    LovelaceConfig,
    /// Items of a to-do list (`entity_id` given with [`Command::to_message_with`]).
    TodoItems,
    /// Energy panel settings (which meters, which devices).
    EnergyPrefs,
    /// Long-term statistics (hourly/daily sums of energy meters…).
    Statistics,
    /// Weather forecast: a subscription whose first event is the forecast.
    WeatherForecast,
    /// Browses a media source (`media_source/browse_media`).
    MediaBrowse,
    /// Gives the (signed) URL of a playable media (`media_source/resolve_media`).
    MediaResolve,
}

impl Command {
    pub const ALL: [Command; 16] = [
        Command::GetStates,
        Command::SubscribeStateChanged,
        Command::SubscribeTagScanned,
        Command::FloorList,
        Command::AreaList,
        Command::LabelList,
        Command::DeviceList,
        Command::EntityList,
        Command::LovelaceDashboards,
        Command::LovelaceConfig,
        Command::TodoItems,
        Command::EnergyPrefs,
        Command::Statistics,
        Command::WeatherForecast,
        Command::MediaBrowse,
        Command::MediaResolve,
    ];

    /// Read commands whose name does not end in `/list`.
    pub const OTHER_READS: [&'static str; 8] = [
        "get_states",
        "subscribe_events",
        "lovelace/config",
        "energy/get_prefs",
        "recorder/statistics_during_period",
        "weather/subscribe_forecast",
        "media_source/browse_media",
        "media_source/resolve_media",
    ];

    #[must_use]
    pub fn message_type(self) -> &'static str {
        match self {
            Command::GetStates => "get_states",
            Command::SubscribeStateChanged | Command::SubscribeTagScanned => "subscribe_events",
            Command::FloorList => "config/floor_registry/list",
            Command::AreaList => "config/area_registry/list",
            Command::LabelList => "config/label_registry/list",
            Command::DeviceList => "config/device_registry/list",
            Command::EntityList => "config/entity_registry/list",
            Command::LovelaceDashboards => "lovelace/dashboards/list",
            Command::LovelaceConfig => "lovelace/config",
            Command::TodoItems => "todo/item/list",
            Command::EnergyPrefs => "energy/get_prefs",
            Command::Statistics => "recorder/statistics_during_period",
            Command::WeatherForecast => "weather/subscribe_forecast",
            Command::MediaBrowse => "media_source/browse_media",
            Command::MediaResolve => "media_source/resolve_media",
        }
    }

    #[must_use]
    pub fn to_message(self, id: u64) -> Value {
        let mut msg = json!({ "id": id, "type": self.message_type() });
        match self {
            Command::SubscribeStateChanged => msg["event_type"] = json!("state_changed"),
            Command::SubscribeTagScanned => msg["event_type"] = json!("tag_scanned"),
            _ => {}
        }
        msg
    }

    /// Same as [`Command::to_message`], plus extra read parameters
    /// (`url_path` of `lovelace/config`).
    #[must_use]
    pub fn to_message_with(self, id: u64, params: &serde_json::Map<String, Value>) -> Value {
        let mut msg = self.to_message(id);
        for (k, v) in params {
            if k != "id" && k != "type" {
                msg[k] = v.clone();
            }
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
                Command::OTHER_READS.contains(&t) || t.ends_with("/list"),
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
