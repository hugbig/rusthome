use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A validated `domain.object_id` identifier, e.g. `light.salon`.
///
/// Same rules as Home Assistant: lowercase ASCII letters, digits and `_`,
/// exactly one dot, no leading/trailing underscore in either part.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(try_from = "String", into = "String")]
pub struct EntityId(String);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EntityIdError {
    #[error("entity id `{0}` must have the form `domain.object_id`")]
    Format(String),
    #[error("entity id `{0}` contains invalid characters")]
    Charset(String),
}

impl EntityId {
    pub fn parse(raw: &str) -> Result<Self, EntityIdError> {
        let (domain, object) = raw
            .split_once('.')
            .ok_or_else(|| EntityIdError::Format(raw.to_owned()))?;
        if domain.is_empty() || object.is_empty() || object.contains('.') {
            return Err(EntityIdError::Format(raw.to_owned()));
        }
        if !is_valid_part(domain) || !is_valid_part(object) {
            return Err(EntityIdError::Charset(raw.to_owned()));
        }
        Ok(Self(raw.to_owned()))
    }

    #[must_use]
    pub fn domain(&self) -> &str {
        self.0.split_once('.').map_or("", |(d, _)| d)
    }

    #[must_use]
    pub fn object_id(&self) -> &str {
        self.0.split_once('.').map_or("", |(_, o)| o)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_valid_part(part: &str) -> bool {
    !part.starts_with('_')
        && !part.ends_with('_')
        && part
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

impl FromStr for EntityId {
    type Err = EntityIdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<String> for EntityId {
    type Error = EntityIdError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<EntityId> for String {
    fn from(value: EntityId) -> Self {
        value.0
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_ids() {
        let id = EntityId::parse("alarm_control_panel.alarmo").unwrap();
        assert_eq!(id.domain(), "alarm_control_panel");
        assert_eq!(id.object_id(), "alarmo");
        // Z2M-style ids with hex addresses are valid (cf. inventaire/05)
        assert!(EntityId::parse("sensor.0xa4c13860c082643c_battpercentage").is_ok());
    }

    #[test]
    fn rejects_malformed_ids() {
        for raw in [
            "light",
            ".salon",
            "light.",
            "light.salon.extra",
            "Light.salon",
            "light.sa lon",
            "light._x",
            "light.x_",
        ] {
            assert!(EntityId::parse(raw).is_err(), "{raw} should be rejected");
        }
    }

    #[test]
    fn serde_round_trip_validates() {
        let id: EntityId = serde_json::from_str("\"switch.radiateur_salon\"").unwrap();
        assert_eq!(
            serde_json::to_string(&id).unwrap(),
            "\"switch.radiateur_salon\""
        );
        assert!(serde_json::from_str::<EntityId>("\"not an id\"").is_err());
    }
}
