//! Startup configuration from environment variables (`.env` in dev).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use chrono_tz::Tz;
use rusthome_api::oidc::RoleMap;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Env {
    Development,
    Production,
}

/// Authentication settings; the binary turns them into an `AuthMode`
/// (OIDC needs an async discovery at startup).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthConfig {
    Dev,
    Disabled,
    // Boxed: OIDC settings are much larger than the other variants.
    Oidc(Box<OidcConfig>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcConfig {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    /// Public URL of RustHome (behind SWAG), e.g. `https://maison.example.org`.
    pub public_url: Url,
    pub roles: RoleMap,
}

impl OidcConfig {
    #[must_use]
    pub fn redirect_url(&self) -> String {
        format!(
            "{}/api/v1/auth/callback",
            self.public_url.as_str().trim_end_matches('/')
        )
    }
}

/// Spec §8.3: admin account is admin, Hugo and Sokhna are residents.
pub const DEFAULT_ROLES: &str = "admin=admin,hugo=resident,sokhna=resident";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MqttSettings {
    pub url: Url,
    pub username: Option<String>,
    pub password: Option<String>,
    /// Safe default: `true` (subscribe only, never publish).
    pub read_only: bool,
    pub z2m_base_topic: String,
    /// Frigate API (`http://192.168.1.25:5000`); Frigate shares the MQTT client.
    pub frigate_url: Option<Url>,
    pub frigate_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortainerSettings {
    pub url: Url,
    pub token: String,
    /// Portainer's default certificate is self-signed.
    pub accept_invalid_certs: bool,
}

/// ESPHome Builder (spec §5.3 bis).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EsphomeSettings {
    pub url: Url,
    /// Read-only projects (the alarm nodes by default).
    pub locked: Vec<String>,
    /// Projects RustHome may flash over the air (the bench by default).
    pub flashable: Vec<String>,
    /// `project@ip`: the only address over-the-air operations reach.
    pub addresses: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HueSettings {
    pub url: Url,
    pub key: String,
    /// Safe default: `true` (no light is switched during development).
    pub read_only: bool,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub env: Env,
    pub auth: AuthConfig,
    pub bind: SocketAddr,
    pub database_url: String,
    pub static_dir: Option<PathBuf>,
    pub tz: Tz,
    pub retention_days: i64,
    pub ha: Option<(Url, String)>,
    pub fixtures: Option<PathBuf>,
    /// HA blueprints copied here are expanded when automations are imported.
    pub blueprints_dir: PathBuf,
    /// Dashboard imposed on `display` accounts (`RUSTHOME_DISPLAY_DASHBOARD`).
    pub display_dashboard: Option<String>,
    pub mqtt: Option<MqttSettings>,
    pub portainer: Option<PortainerSettings>,
    pub hue: Option<HueSettings>,
    pub esphome: Option<EsphomeSettings>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("{0} has an invalid value: {1}")]
    Invalid(&'static str, String),
    #[error("RUSTHOME_AUTH=dev is forbidden when RUSTHOME_ENV=production")]
    DevAuthInProduction,
    #[error("HA_URL and HA_TOKEN must be set together")]
    HaPartial,
    #[error("RUSTHOME_AUTH=oidc requires {0}")]
    OidcMissing(&'static str),
}

impl Config {
    /// Summary for Paramètres > Système. Only hosts and flags: never a
    /// token, password or URL user info.
    pub fn system_info(&self) -> serde_json::Value {
        let host = |u: &Url| {
            u.host_str().map(|h| {
                u.port()
                    .map_or_else(|| h.to_owned(), |p| format!("{h}:{p}"))
            })
        };
        serde_json::json!({
            "env": if self.env == Env::Production { "production" } else { "development" },
            "auth": match &self.auth {
                AuthConfig::Dev => "dev",
                AuthConfig::Disabled => "disabled",
                AuthConfig::Oidc(_) => "oidc",
            },
            "timezone": self.tz.name(),
            "retention_days": self.retention_days,
            "ha_mirror": self.ha.as_ref().and_then(|(u, _)| host(u)),
            "mqtt": self.mqtt.as_ref().map(|m| serde_json::json!({
                "broker": host(&m.url),
                "read_only": m.read_only,
                "z2m_base_topic": m.z2m_base_topic,
                "frigate": m.frigate_url.as_ref().and_then(host),
            })),
            "hue": self.hue.as_ref().map(|h| serde_json::json!({
                "bridge": host(&h.url),
                "read_only": h.read_only,
            })),
            "portainer": self.portainer.as_ref().and_then(|p| host(&p.url)),
            "esphome": self.esphome.as_ref().map(|e| serde_json::json!({
                "builder": host(&e.url),
                "locked": e.locked,
                "flashable": e.flashable,
                "addresses": e.addresses,
            })),
        })
    }

    /// Pure: takes the variables as a map so it is unit-testable.
    pub fn from_vars(vars: &HashMap<String, String>) -> Result<Self, ConfigError> {
        let get = |k: &str| vars.get(k).map(String::as_str).filter(|v| !v.is_empty());

        let env = match get("RUSTHOME_ENV").unwrap_or("development") {
            "development" | "dev" => Env::Development,
            "production" | "prod" => Env::Production,
            other => return Err(ConfigError::Invalid("RUSTHOME_ENV", other.into())),
        };
        let auth_kind = get("RUSTHOME_AUTH").unwrap_or(if env == Env::Development {
            "dev"
        } else {
            "none"
        });
        let auth = match auth_kind {
            "dev" => AuthConfig::Dev,
            "none" => AuthConfig::Disabled,
            "oidc" => {
                let need =
                    |k: &'static str| get(k).map(str::to_owned).ok_or(ConfigError::OidcMissing(k));
                let public = need("RUSTHOME_PUBLIC_URL")?;
                AuthConfig::Oidc(Box::new(OidcConfig {
                    issuer: need("RUSTHOME_OIDC_ISSUER")?,
                    client_id: need("RUSTHOME_OIDC_CLIENT_ID")?,
                    client_secret: need("RUSTHOME_OIDC_CLIENT_SECRET")?,
                    public_url: Url::parse(&public)
                        .map_err(|_| ConfigError::Invalid("RUSTHOME_PUBLIC_URL", public))?,
                    roles: RoleMap::parse(get("RUSTHOME_ROLES").unwrap_or(DEFAULT_ROLES))
                        .map_err(|e| ConfigError::Invalid("RUSTHOME_ROLES", e))?,
                }))
            }
            other => return Err(ConfigError::Invalid("RUSTHOME_AUTH", other.into())),
        };
        if env == Env::Production && auth == AuthConfig::Dev {
            return Err(ConfigError::DevAuthInProduction);
        }
        let bind = get("RUSTHOME_BIND")
            .unwrap_or("127.0.0.1:8124")
            .parse()
            .map_err(|_| {
                ConfigError::Invalid(
                    "RUSTHOME_BIND",
                    get("RUSTHOME_BIND").unwrap_or_default().into(),
                )
            })?;
        let tz = get("RUSTHOME_TZ")
            .unwrap_or("Europe/Paris")
            .parse()
            .map_err(|_| {
                ConfigError::Invalid("RUSTHOME_TZ", get("RUSTHOME_TZ").unwrap_or_default().into())
            })?;
        let retention_days = retention_days(get("RUSTHOME_RETENTION_DAYS"))?;
        let ha = match (get("HA_URL"), get("HA_TOKEN")) {
            (Some(u), Some(t)) => Some((
                Url::parse(u).map_err(|_| ConfigError::Invalid("HA_URL", u.into()))?,
                t.to_owned(),
            )),
            (None, None) => None,
            _ => return Err(ConfigError::HaPartial),
        };
        let mqtt = mqtt_settings(vars)?;
        let portainer = portainer_settings(vars)?;
        let hue = hue_settings(vars)?;
        let esphome = esphome_settings(vars)?;
        Ok(Self {
            mqtt,
            portainer,
            hue,
            esphome,
            env,
            auth,
            bind,
            database_url: get("RUSTHOME_DB")
                .unwrap_or("sqlite://var/rusthome.db")
                .to_owned(),
            static_dir: get("RUSTHOME_STATIC_DIR").map(PathBuf::from),
            tz,
            retention_days,
            ha,
            fixtures: get("RUSTHOME_FIXTURES").map(PathBuf::from),
            display_dashboard: match get("RUSTHOME_DISPLAY_DASHBOARD") {
                Some(p) if rusthome_dashboard::valid_url_path(p) => Some(p.to_owned()),
                Some(p) => {
                    return Err(ConfigError::Invalid(
                        "RUSTHOME_DISPLAY_DASHBOARD",
                        p.to_owned(),
                    ));
                }
                None => None,
            },
            blueprints_dir: PathBuf::from(
                get("RUSTHOME_BLUEPRINTS_DIR").unwrap_or("var/blueprints"),
            ),
        })
    }
}

/// History retention in days, at least 1: 0 or less would make the hourly
/// purge delete everything (code review n° 46).
fn retention_days(raw: Option<&str>) -> Result<i64, ConfigError> {
    raw.unwrap_or("10")
        .parse()
        .ok()
        .filter(|d: &i64| *d >= 1)
        .ok_or_else(|| {
            ConfigError::Invalid("RUSTHOME_RETENTION_DAYS", raw.unwrap_or_default().into())
        })
}

/// ESPHome Builder settings; `None` without `RUSTHOME_ESPHOME_URL`. The
/// lists default to the alarm nodes (locked) and the bench (flashable); an
/// empty value means an empty list, on Hugo's decision only.
fn esphome_settings(
    vars: &HashMap<String, String>,
) -> Result<Option<EsphomeSettings>, ConfigError> {
    let Some(u) = vars
        .get("RUSTHOME_ESPHOME_URL")
        .map(String::as_str)
        .filter(|v| !v.is_empty())
    else {
        return Ok(None);
    };
    let list = |k: &str, default: &[&str]| -> Vec<String> {
        vars.get(k).map_or_else(
            || default.iter().map(|s| (*s).to_owned()).collect(),
            |v| {
                v.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect()
            },
        )
    };
    Ok(Some(EsphomeSettings {
        url: Url::parse(u).map_err(|_| ConfigError::Invalid("RUSTHOME_ESPHOME_URL", u.into()))?,
        locked: list(
            "RUSTHOME_ESPHOME_LOCKED",
            rusthome_esphome::policy::DEFAULT_LOCKED,
        ),
        flashable: list(
            "RUSTHOME_ESPHOME_FLASHABLE",
            rusthome_esphome::policy::DEFAULT_FLASHABLE,
        ),
        addresses: list("RUSTHOME_ESPHOME_ADDRESSES", &[]),
    }))
}

/// Hue settings; `None` unless both URL and key are set.
fn hue_settings(vars: &HashMap<String, String>) -> Result<Option<HueSettings>, ConfigError> {
    let get = |k: &str| vars.get(k).map(String::as_str).filter(|v| !v.is_empty());
    Ok(match (get("RUSTHOME_HUE_URL"), get("RUSTHOME_HUE_KEY")) {
        (None, None) => None,
        (Some(u), Some(k)) => Some(HueSettings {
            url: Url::parse(u).map_err(|_| ConfigError::Invalid("RUSTHOME_HUE_URL", u.into()))?,
            key: k.to_owned(),
            read_only: match get("RUSTHOME_HUE_READ_ONLY").unwrap_or("true") {
                "true" | "1" | "yes" => true,
                "false" | "0" | "no" => false,
                other => return Err(ConfigError::Invalid("RUSTHOME_HUE_READ_ONLY", other.into())),
            },
        }),
        _ => {
            return Err(ConfigError::Invalid(
                "RUSTHOME_HUE_URL/KEY",
                "both must be set".into(),
            ));
        }
    })
}

/// Portainer settings; `None` unless both URL and token are set.
fn portainer_settings(
    vars: &HashMap<String, String>,
) -> Result<Option<PortainerSettings>, ConfigError> {
    let get = |k: &str| vars.get(k).map(String::as_str).filter(|v| !v.is_empty());
    Ok(
        match (
            get("RUSTHOME_PORTAINER_URL"),
            get("RUSTHOME_PORTAINER_TOKEN"),
        ) {
            (None, None) => None,
            (Some(u), Some(t)) => Some(PortainerSettings {
                url: Url::parse(u)
                    .map_err(|_| ConfigError::Invalid("RUSTHOME_PORTAINER_URL", u.into()))?,
                token: t.to_owned(),
                accept_invalid_certs: matches!(
                    get("RUSTHOME_PORTAINER_TLS_INSECURE"),
                    Some("true" | "1" | "yes")
                ),
            }),
            _ => {
                return Err(ConfigError::Invalid(
                    "RUSTHOME_PORTAINER_URL/TOKEN",
                    "both must be set".into(),
                ));
            }
        },
    )
}

/// MQTT settings; `None` when `RUSTHOME_MQTT_URL` is not set.
fn mqtt_settings(vars: &HashMap<String, String>) -> Result<Option<MqttSettings>, ConfigError> {
    let get = |k: &str| vars.get(k).map(String::as_str).filter(|v| !v.is_empty());
    Ok(match get("RUSTHOME_MQTT_URL") {
        None => None,
        Some(raw) => Some(MqttSettings {
            url: Url::parse(raw)
                .map_err(|_| ConfigError::Invalid("RUSTHOME_MQTT_URL", raw.into()))?,
            username: get("RUSTHOME_MQTT_USERNAME").map(str::to_owned),
            password: get("RUSTHOME_MQTT_PASSWORD").map(str::to_owned),
            read_only: match get("RUSTHOME_MQTT_READ_ONLY").unwrap_or("true") {
                "true" | "1" | "yes" => true,
                "false" | "0" | "no" => false,
                other => {
                    return Err(ConfigError::Invalid(
                        "RUSTHOME_MQTT_READ_ONLY",
                        other.into(),
                    ));
                }
            },
            frigate_url: match get("RUSTHOME_FRIGATE_URL") {
                Some(u) => Some(
                    Url::parse(u)
                        .map_err(|_| ConfigError::Invalid("RUSTHOME_FRIGATE_URL", u.into()))?,
                ),
                None => None,
            },
            frigate_prefix: get("RUSTHOME_FRIGATE_TOPIC_PREFIX")
                .unwrap_or("frigate")
                .to_owned(),
            z2m_base_topic: get("RUSTHOME_Z2M_BASE_TOPIC")
                .unwrap_or("zigbee2mqtt")
                .to_owned(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn defaults_are_safe_for_dev() {
        let c = Config::from_vars(&vars(&[])).unwrap();
        assert_eq!(c.env, Env::Development);
        assert_eq!(c.auth, AuthConfig::Dev);
        assert_eq!(c.bind.to_string(), "127.0.0.1:8124");
        assert!(c.ha.is_none());
    }

    #[test]
    fn retention_is_at_least_one_day() {
        assert_eq!(retention_days(None), Ok(10));
        assert_eq!(retention_days(Some("30")), Ok(30));
        for bad in ["0", "-3", "dix"] {
            assert!(retention_days(Some(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn esphome_lists_default_to_the_alarm_and_the_bench() {
        let c =
            Config::from_vars(&vars(&[("RUSTHOME_ESPHOME_URL", "http://esphome:6052")])).unwrap();
        let e = c.esphome.unwrap();
        assert_eq!(e.locked, ["lecteur-alarme", "tag-reader"]);
        assert_eq!(e.flashable, ["test-nfc"]);
        let c = Config::from_vars(&vars(&[
            ("RUSTHOME_ESPHOME_URL", "http://esphome:6052"),
            ("RUSTHOME_ESPHOME_FLASHABLE", "test-nfc, ir-box"),
            ("RUSTHOME_ESPHOME_LOCKED", ""),
        ]))
        .unwrap();
        let e = c.esphome.unwrap();
        assert_eq!(e.locked.len(), 0);
        assert_eq!(e.flashable, ["test-nfc", "ir-box"]);
        assert!(Config::from_vars(&vars(&[])).unwrap().esphome.is_none());
    }

    #[test]
    fn display_dashboard_must_be_a_dashboard_path() {
        let c = Config::from_vars(&vars(&[(
            "RUSTHOME_DISPLAY_DASHBOARD",
            "dashboard-nspanel",
        )]))
        .unwrap();
        assert_eq!(c.display_dashboard.as_deref(), Some("dashboard-nspanel"));
        assert!(Config::from_vars(&vars(&[("RUSTHOME_DISPLAY_DASHBOARD", "Mon Panel")])).is_err());
        assert!(
            Config::from_vars(&vars(&[]))
                .unwrap()
                .display_dashboard
                .is_none()
        );
    }

    #[test]
    fn system_info_has_no_secret() {
        let c = Config::from_vars(&vars(&[
            ("HA_URL", "http://192.168.1.35:8123"),
            ("HA_TOKEN", "very-secret-token"),
            ("RUSTHOME_MQTT_URL", "mqtt://user:pw@192.168.1.35:1883"),
            ("RUSTHOME_MQTT_PASSWORD", "mqtt-secret"),
        ]))
        .unwrap();
        let info = c.system_info();
        assert_eq!(info["ha_mirror"], "192.168.1.35:8123");
        assert_eq!(info["mqtt"]["broker"], "192.168.1.35:1883");
        assert_eq!(info["mqtt"]["read_only"], true);
        let text = info.to_string();
        for secret in ["very-secret-token", "mqtt-secret", "user:pw", "pw@"] {
            assert!(!text.contains(secret), "{secret} leaked");
        }
    }

    #[test]
    fn production_defaults_to_no_auth_and_refuses_dev_auth() {
        let c = Config::from_vars(&vars(&[("RUSTHOME_ENV", "production")])).unwrap();
        assert_eq!(c.auth, AuthConfig::Disabled);
        let err = Config::from_vars(&vars(&[
            ("RUSTHOME_ENV", "production"),
            ("RUSTHOME_AUTH", "dev"),
        ]))
        .unwrap_err();
        assert_eq!(err, ConfigError::DevAuthInProduction);
    }

    #[test]
    fn ha_url_and_token_go_together() {
        assert_eq!(
            Config::from_vars(&vars(&[("HA_URL", "http://192.168.1.35:8123")])).unwrap_err(),
            ConfigError::HaPartial
        );
        let c = Config::from_vars(&vars(&[
            ("HA_URL", "http://192.168.1.35:8123"),
            ("HA_TOKEN", "t"),
        ]))
        .unwrap();
        assert_eq!(c.ha.unwrap().0.host_str(), Some("192.168.1.35"));
    }

    #[test]
    fn invalid_values_are_reported() {
        assert!(matches!(
            Config::from_vars(&vars(&[("RUSTHOME_BIND", "nope")])),
            Err(ConfigError::Invalid("RUSTHOME_BIND", _))
        ));
        assert!(matches!(
            Config::from_vars(&vars(&[("RUSTHOME_TZ", "Mars/Olympus")])),
            Err(ConfigError::Invalid("RUSTHOME_TZ", _))
        ));
    }

    #[test]
    fn oidc_requires_its_settings_and_builds_the_redirect() {
        let base = [
            ("RUSTHOME_ENV", "production"),
            ("RUSTHOME_AUTH", "oidc"),
            ("RUSTHOME_OIDC_ISSUER", "https://auth.example.org"),
            ("RUSTHOME_OIDC_CLIENT_ID", "rusthome"),
            ("RUSTHOME_OIDC_CLIENT_SECRET", "s3cret"),
            ("RUSTHOME_PUBLIC_URL", "https://maison.example.org/"),
        ];
        let c = Config::from_vars(&vars(&base)).unwrap();
        let AuthConfig::Oidc(o) = c.auth else {
            panic!("expected OIDC")
        };
        assert_eq!(
            o.redirect_url(),
            "https://maison.example.org/api/v1/auth/callback"
        );
        assert_eq!(o.roles, RoleMap::parse(DEFAULT_ROLES).unwrap());
        let missing = Config::from_vars(&vars(&base[..5])).unwrap_err();
        assert_eq!(missing, ConfigError::OidcMissing("RUSTHOME_PUBLIC_URL"));
        let mut bad_roles = base.to_vec();
        bad_roles.push(("RUSTHOME_ROLES", "hugo=root"));
        assert!(matches!(
            Config::from_vars(&vars(&bad_roles)),
            Err(ConfigError::Invalid("RUSTHOME_ROLES", _))
        ));
    }

    #[test]
    fn mqtt_is_read_only_unless_explicitly_disabled() {
        let c =
            Config::from_vars(&vars(&[("RUSTHOME_MQTT_URL", "mqtt://192.168.1.35:1883")])).unwrap();
        let m = c.mqtt.unwrap();
        assert!(m.read_only);
        assert_eq!(m.z2m_base_topic, "zigbee2mqtt");
        let w = Config::from_vars(&vars(&[
            ("RUSTHOME_MQTT_URL", "mqtt://x:1883"),
            ("RUSTHOME_MQTT_READ_ONLY", "false"),
        ]))
        .unwrap();
        assert!(!w.mqtt.unwrap().read_only);
        assert!(
            Config::from_vars(&vars(&[
                ("RUSTHOME_MQTT_URL", "mqtt://x"),
                ("RUSTHOME_MQTT_READ_ONLY", "peut-etre")
            ]))
            .is_err()
        );
        assert!(Config::from_vars(&vars(&[])).unwrap().mqtt.is_none());
    }

    #[test]
    fn portainer_needs_url_and_token() {
        assert!(Config::from_vars(&vars(&[])).unwrap().portainer.is_none());
        assert!(
            Config::from_vars(&vars(&[(
                "RUSTHOME_PORTAINER_URL",
                "https://192.168.1.25:9443"
            )]))
            .is_err()
        );
        let c = Config::from_vars(&vars(&[
            ("RUSTHOME_PORTAINER_URL", "https://192.168.1.25:9443"),
            ("RUSTHOME_PORTAINER_TOKEN", "ptr_x"),
            ("RUSTHOME_PORTAINER_TLS_INSECURE", "true"),
        ]))
        .unwrap();
        assert!(c.portainer.unwrap().accept_invalid_certs);
    }

    #[test]
    fn hue_is_read_only_by_default() {
        let c = Config::from_vars(&vars(&[
            ("RUSTHOME_HUE_URL", "https://192.168.1.247"),
            ("RUSTHOME_HUE_KEY", "k"),
        ]))
        .unwrap();
        assert!(c.hue.unwrap().read_only);
        assert!(Config::from_vars(&vars(&[("RUSTHOME_HUE_KEY", "k")])).is_err());
    }
}
