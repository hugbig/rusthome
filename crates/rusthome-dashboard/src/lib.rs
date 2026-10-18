//! Dashboards (M4, cf. docs/architecture.md §8.2).
//!
//! A dashboard is a Lovelace-shaped JSON document using native card types
//! only ([`lovelace::NATIVE_CARDS`]). [`lovelace::dashboard`] imports HA
//! dashboards and cleans user YAML; [`refs`] finds entity references for the
//! reference validator; [`Store`] keeps the documents in memory.

pub mod lovelace;
pub mod refs;

use std::collections::BTreeMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Something to fix in a dashboard: an unsupported card, a missing entity…
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    pub path: String,
    pub message: String,
}

/// `lovelace`, `securite-alarme`: lowercase letters, digits and dashes.
#[must_use]
pub fn valid_url_path(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 64
        && !p.starts_with('-')
        && p.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// What lists show about a dashboard.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Summary {
    pub url_path: String,
    pub title: String,
    pub icon: Option<String>,
    pub show_in_sidebar: bool,
    pub require_admin: bool,
    pub kiosk: bool,
    pub views: usize,
}

impl Summary {
    #[must_use]
    pub fn of(doc: &Value) -> Self {
        let url_path = doc["url_path"].as_str().unwrap_or_default().to_owned();
        Self {
            title: doc["title"].as_str().unwrap_or(&url_path).to_owned(),
            icon: doc["icon"].as_str().map(str::to_owned),
            show_in_sidebar: doc["show_in_sidebar"] != false,
            require_admin: doc["require_admin"] == true,
            kiosk: doc["kiosk"] == true,
            views: doc["views"].as_array().map_or(0, Vec::len),
            url_path,
        }
    }
}

/// In-memory dashboards, keyed by `url_path`.
#[derive(Default)]
pub struct Store {
    docs: RwLock<BTreeMap<String, Value>>,
    /// Dashboard opened at `/` (like HA's default dashboard).
    default: RwLock<Option<String>>,
}

impl Store {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list(&self) -> Vec<Summary> {
        self.read().values().map(Summary::of).collect()
    }

    pub fn get(&self, url_path: &str) -> Option<Value> {
        self.read().get(url_path).cloned()
    }

    pub fn contains(&self, url_path: &str) -> bool {
        self.read().contains_key(url_path)
    }

    /// Stores a document; its `url_path` field is the key.
    pub fn put(&self, doc: Value) -> Option<Summary> {
        let key = doc["url_path"].as_str()?.to_owned();
        let summary = Summary::of(&doc);
        self.write().insert(key, doc);
        Some(summary)
    }

    /// The default dashboard, if it still exists.
    pub fn default_dashboard(&self) -> Option<String> {
        let d = self
            .default
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()?;
        self.contains(&d).then_some(d)
    }

    pub fn set_default(&self, url_path: Option<String>) {
        *self
            .default
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = url_path;
    }

    pub fn remove(&self, url_path: &str) -> bool {
        self.write().remove(url_path).is_some()
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, BTreeMap<String, Value>> {
        self.docs
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, BTreeMap<String, Value>> {
        self.docs
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Image (poster, else fanart) of item `index` of an "upcoming media"
/// sensor: `data` is a list, often serialized as JSON, whose first element
/// holds the line templates. Only `http(s)` URLs are returned: the image
/// proxy serves nothing else than what the sensor lists.
#[must_use]
pub fn media_image_url(data: &Value, index: usize) -> Option<String> {
    let parsed;
    let list = match data {
        Value::String(s) => {
            parsed = serde_json::from_str::<Value>(s).ok()?;
            &parsed
        }
        other => other,
    };
    let item = list.as_array()?.get(index + 1)?;
    let url = item["poster"]
        .as_str()
        .or_else(|| item["fanart"].as_str())?;
    (url.starts_with("http://") || url.starts_with("https://")).then(|| url.to_owned())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn default_dashboard_must_exist() {
        let store = Store::new();
        store.put(json!({ "url_path": "apercu-3", "views": [] }));
        store.set_default(Some("apercu-3".into()));
        assert_eq!(store.default_dashboard().as_deref(), Some("apercu-3"));
        store.remove("apercu-3");
        assert_eq!(store.default_dashboard(), None);
        store.set_default(None);
        assert_eq!(store.default_dashboard(), None);
    }

    #[test]
    fn url_paths() {
        assert!(valid_url_path("securite-alarme"));
        assert!(valid_url_path("lovelace"));
        assert!(!valid_url_path("Sécurité"));
        assert!(!valid_url_path("a/b"));
        assert!(!valid_url_path(""));
    }

    #[test]
    fn media_images_come_only_from_the_sensor() {
        let data = json!([
            { "title_default": "$title" },
            { "title": "A", "poster": "http://192.0.2.21:8096/Items/1/Images/Backdrop" },
            { "title": "B", "fanart": "https://img.example/b.jpg" },
            { "title": "C", "poster": "file:///etc/passwd" }
        ]);
        assert_eq!(
            media_image_url(&data, 0).as_deref(),
            Some("http://192.0.2.21:8096/Items/1/Images/Backdrop")
        );
        assert_eq!(
            media_image_url(&Value::String(data.to_string()), 1).as_deref(),
            Some("https://img.example/b.jpg")
        );
        assert_eq!(media_image_url(&data, 2), None, "only http(s)");
        assert_eq!(media_image_url(&data, 9), None);
    }

    #[test]
    fn store_round_trip() {
        let s = Store::new();
        let sum = s
            .put(json!({ "url_path": "x", "title": "X", "views": [{}, {}] }))
            .unwrap();
        assert_eq!(sum.views, 2);
        assert!(sum.show_in_sidebar);
        assert_eq!(s.list().len(), 1);
        assert!(s.put(json!({ "title": "sans clé" })).is_none());
        assert!(s.remove("x"));
        assert!(s.get("x").is_none());
    }
}
