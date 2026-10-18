//! What RustHome may do with an ESPHome project.
//!
//! - **Locked** projects (the alarm nodes, AGENTS.md §2 rule 3) are
//!   read-only: no edit, no deletion, no compilation for them, no flash,
//!   no logs.
//! - **Flashable** projects are the only ones RustHome installs over the
//!   air. During development that is the test bench (`test-nfc`); Hugo
//!   opens the others when he decides to (spec §5.3 bis).
//! - Over-the-air operations (installation, logs) only reach the **address
//!   fixed in the configuration** for the project: never an address typed in
//!   the browser, never a `.local` name or a `use_address` from the YAML,
//!   which could point at an alarm node (code review 2026-10-18, n° 2).

/// Alarm nodes: never driven by RustHome until the alarm migration phase.
pub const DEFAULT_LOCKED: &[&str] = &["lecteur-alarme", "tag-reader"];
/// Test bench: the only node flashed over the air during development.
pub const DEFAULT_FLASHABLE: &[&str] = &["test-nfc"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    locked: Vec<String>,
    flashable: Vec<String>,
    /// Project → IPv4 address of its board.
    addresses: Vec<(String, std::net::Ipv4Addr)>,
}

impl Default for Policy {
    fn default() -> Self {
        Self::new(
            DEFAULT_LOCKED.iter().map(|s| (*s).to_owned()),
            DEFAULT_FLASHABLE.iter().map(|s| (*s).to_owned()),
        )
    }
}

impl Policy {
    /// Names are compared without the `.yaml` extension.
    pub fn new(
        locked: impl IntoIterator<Item = String>,
        flashable: impl IntoIterator<Item = String>,
    ) -> Self {
        let norm = |v: String| stem(v.trim()).to_owned();
        Self {
            locked: locked
                .into_iter()
                .map(norm)
                .filter(|s| !s.is_empty())
                .collect(),
            flashable: flashable
                .into_iter()
                .map(norm)
                .filter(|s| !s.is_empty())
                .collect(),
            addresses: Vec::new(),
        }
    }

    /// Adds the boards' addresses (`project@ip`). An address for a locked
    /// project is refused: RustHome must never be able to reach it.
    pub fn with_addresses(
        mut self,
        entries: impl IntoIterator<Item = String>,
    ) -> Result<Self, String> {
        for e in entries {
            let (name, ip) = e
                .split_once('@')
                .ok_or_else(|| format!("{e}: expected project@ip"))?;
            let ip: std::net::Ipv4Addr = ip
                .trim()
                .parse()
                .map_err(|_| format!("{e}: invalid IPv4 address"))?;
            let name = stem(name.trim()).to_owned();
            if self.is_locked(&name, None) {
                return Err(format!("{name} is locked: no address may be given for it"));
            }
            self.addresses.push((name, ip));
        }
        Ok(self)
    }

    /// The configured address of a project's board, if any (never for a
    /// locked project).
    #[must_use]
    pub fn address(&self, configuration: &str, name: Option<&str>) -> Option<std::net::Ipv4Addr> {
        if self.is_locked(configuration, name) {
            return None;
        }
        let file = stem(configuration);
        self.addresses
            .iter()
            .find(|(p, _)| p == file || name.is_some_and(|n| n == p))
            .map(|(_, ip)| *ip)
    }

    /// Locked by its file name or by its `esphome.name`.
    #[must_use]
    pub fn is_locked(&self, configuration: &str, name: Option<&str>) -> bool {
        let file = stem(configuration);
        self.locked
            .iter()
            .any(|l| l == file || name.is_some_and(|n| n == l))
    }

    /// May be installed over the air (and never when locked).
    #[must_use]
    pub fn can_flash(&self, configuration: &str, name: Option<&str>) -> bool {
        if self.is_locked(configuration, name) {
            return false;
        }
        let file = stem(configuration);
        self.flashable
            .iter()
            .any(|f| f == file || name.is_some_and(|n| n == f))
    }
}

fn stem(configuration: &str) -> &str {
    configuration
        .strip_suffix(".yaml")
        .or_else(|| configuration.strip_suffix(".yml"))
        .unwrap_or(configuration)
}

/// A project file name the API accepts: a plain `*.yaml` name at the top of
/// the builder's folder, never `secrets.yaml` (whose values stay hidden).
#[must_use]
pub fn valid_configuration(configuration: &str) -> bool {
    let Some(base) = configuration
        .strip_suffix(".yaml")
        .or_else(|| configuration.strip_suffix(".yml"))
    else {
        return false;
    };
    !base.is_empty()
        && base.len() <= 64
        && base != "secrets"
        && base
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && base
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

/// `esphome.name` of a project's YAML, `${substitutions}` resolved. Custom
/// tags (`!secret`, `!lambda`…) are kept as tagged values and ignored.
#[must_use]
pub fn yaml_device_name(content: &str) -> Option<String> {
    let doc: serde_yaml::Value = serde_yaml::from_str(content).ok()?;
    let raw = doc.get("esphome")?.get("name")?.as_str()?;
    let subs = doc.get("substitutions");
    let mut out = String::new();
    let mut rest = raw;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find('}')?;
        let key = &after[..end];
        out.push_str(subs?.get(key)?.as_str()?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(out.trim().to_owned()).filter(|n| !n.is_empty())
}

/// A secret key name (`wifi_password`…).
#[must_use]
pub fn valid_secret_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 64 && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alarm_nodes_are_locked_by_default() {
        let p = Policy::default();
        assert!(p.is_locked("lecteur-alarme.yaml", None));
        assert!(p.is_locked("autre.yaml", Some("tag-reader")));
        assert!(!p.is_locked("ir-box.yaml", Some("ir-box")));
        // locked wins over flashable
        let p = Policy::new(["x".into()], ["x.yaml".into()]);
        assert!(!p.can_flash("x.yaml", None));
    }

    #[test]
    fn only_the_bench_is_flashable_by_default() {
        let p = Policy::default();
        assert!(p.can_flash("test-nfc.yaml", None));
        assert!(!p.can_flash("ir-box.yaml", Some("ir-box")));
        let p = Policy::new(Vec::new(), [" ir-box ".into(), String::new()]);
        assert!(p.can_flash("ir-box.yml", None));
    }

    #[test]
    fn addresses_are_fixed_per_project_and_never_for_the_alarm() {
        let p = Policy::default()
            .with_addresses([
                "test-nfc@192.168.1.80".into(),
                "ir-box.yaml@10.0.0.7".into(),
            ])
            .unwrap();
        assert_eq!(
            p.address("test-nfc.yaml", None),
            Some([192, 168, 1, 80].into())
        );
        assert_eq!(
            p.address("x.yaml", Some("ir-box")),
            Some([10, 0, 0, 7].into())
        );
        assert_eq!(p.address("eink.yaml", None), None);
        assert!(
            Policy::default()
                .with_addresses(["tag-reader@10.0.0.9".into()])
                .is_err()
        );
        assert!(
            Policy::default()
                .with_addresses(["test-nfc@nope".into()])
                .is_err()
        );
        assert!(
            Policy::default()
                .with_addresses(["test-nfc".into()])
                .is_err()
        );
    }

    #[test]
    fn device_names_are_read_from_the_yaml() {
        let y = "substitutions:\n  device_name: lecteur-alarme\nesphome:\n  name: ${device_name}\napi:\n  encryption:\n    key: !secret api_key\n";
        assert_eq!(yaml_device_name(y).as_deref(), Some("lecteur-alarme"));
        assert_eq!(
            yaml_device_name("esphome:\n  name: pre-${x}\nsubstitutions:\n  x: fix\n").as_deref(),
            Some("pre-fix")
        );
        assert_eq!(yaml_device_name("esphome:\n  name: ${missing}\n"), None);
        assert_eq!(yaml_device_name("wifi: {}"), None);
        assert_eq!(yaml_device_name(": not yaml ["), None);
    }

    #[test]
    fn configuration_names() {
        for ok in ["ir-box.yaml", "esphome-web-d39c98.yaml", "a_b.yml"] {
            assert!(valid_configuration(ok), "{ok}");
        }
        for bad in [
            "secrets.yaml",
            "../x.yaml",
            "a/b.yaml",
            ".hidden.yaml",
            "x.json",
            ".yaml",
            "-x.yaml",
        ] {
            assert!(!valid_configuration(bad), "{bad}");
        }
        assert!(valid_secret_key("wifi_password"));
        assert!(!valid_secret_key("a-b"));
        assert!(!valid_secret_key(""));
    }
}
