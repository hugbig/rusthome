//! SQLite persistence: registries and state history (cf. spec §4.4).

use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use rusthome_core::{Event, Hub, Registries, RegistryKind};
use rusthome_model::{EntityId, State};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use tokio::sync::broadcast::error::RecvError;

#[derive(Debug, thiserror::Error)]
pub enum RecorderError {
    #[error("database: {0}")]
    Db(#[from] sqlx::Error),
    #[error("migration: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// One line of the logbook.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LogbookEntry {
    pub entity_id: String,
    pub state: String,
    pub previous: String,
    pub when: DateTime<Utc>,
}

#[derive(Clone)]
pub struct Recorder {
    pool: SqlitePool,
    /// Domains never written to history (cf. inventaire/04: Portainer = 32 % of entities).
    excluded_integrations: Arc<Vec<String>>,
}

impl Recorder {
    /// Opens (or creates) the database and applies migrations.
    /// `url` is e.g. `sqlite://var/rusthome.db` or `sqlite::memory:`.
    pub async fn open(url: &str) -> Result<Self, RecorderError> {
        let opts = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        // In-memory databases are per-connection: keep a single one.
        let max = if url.contains(":memory:") { 1 } else { 4 };
        let pool = SqlitePoolOptions::new()
            .max_connections(max)
            .connect_with(opts)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self {
            pool,
            excluded_integrations: Arc::new(vec!["ha:portainer".into()]),
        })
    }

    // ---------- registries ----------

    async fn put<T: Serialize>(
        &self,
        table: Table,
        id: &str,
        item: &T,
    ) -> Result<(), RecorderError> {
        sqlx::query(table.upsert_sql())
            .bind(id)
            .bind(serde_json::to_string(item)?)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn all<T: DeserializeOwned>(&self, table: Table) -> Result<Vec<T>, RecorderError> {
        let rows = sqlx::query(table.select_sql())
            .fetch_all(&self.pool)
            .await?;
        rows.iter()
            .map(|r| Ok(serde_json::from_str(r.get::<&str, _>("data"))?))
            .collect()
    }

    /// Persists one registry item, read back from the in-memory registries.
    pub async fn save_registry_item(
        &self,
        reg: &Registries,
        kind: RegistryKind,
        id: &str,
    ) -> Result<(), RecorderError> {
        match kind {
            RegistryKind::Floor => match reg.floor(id) {
                Some(x) => self.put(Table::Floors, id, &x).await,
                None => Ok(()),
            },
            RegistryKind::Area => match reg.area(id) {
                Some(x) => self.put(Table::Areas, id, &x).await,
                None => Ok(()),
            },
            RegistryKind::Label => match reg.label(id) {
                Some(x) => self.put(Table::Labels, id, &x).await,
                None => Ok(()),
            },
            RegistryKind::Device => match reg.device(id) {
                Some(x) => self.put(Table::Devices, id, &x).await,
                None => Ok(()),
            },
            RegistryKind::Entity => match reg.entity(id) {
                Some(x) => self.put(Table::Entities, id, &x).await,
                None => Ok(()),
            },
        }
    }

    /// Persists every registry item (after a lag on the bus).
    pub async fn save_all_registries(&self, reg: &Registries) -> Result<usize, RecorderError> {
        let mut tx = self.pool.begin().await?;
        let mut n = 0;
        macro_rules! save {
            ($items:expr, $table:expr, $key:ident) => {
                for item in $items {
                    sqlx::query($table.upsert_sql())
                        .bind(&item.$key)
                        .bind(serde_json::to_string(&item)?)
                        .execute(&mut *tx)
                        .await?;
                    n += 1;
                }
            };
        }
        save!(reg.floors(), Table::Floors, id);
        save!(reg.areas(), Table::Areas, id);
        save!(reg.labels(), Table::Labels, id);
        save!(reg.devices(), Table::Devices, id);
        save!(reg.entities(), Table::Entities, uid);
        tx.commit().await?;
        Ok(n)
    }

    /// Loads persisted registries into memory (startup).
    pub async fn load_registries(&self, reg: &Registries) -> Result<usize, RecorderError> {
        let floors: Vec<rusthome_model::Floor> = self.all(Table::Floors).await?;
        let areas: Vec<rusthome_model::Area> = self.all(Table::Areas).await?;
        let labels: Vec<rusthome_model::Label> = self.all(Table::Labels).await?;
        let devices: Vec<rusthome_model::Device> = self.all(Table::Devices).await?;
        let entities: Vec<rusthome_model::EntityEntry> = self.all(Table::Entities).await?;
        let n = floors.len() + areas.len() + labels.len() + devices.len() + entities.len();
        for x in floors {
            reg.upsert_floor(x);
        }
        for x in areas {
            reg.upsert_area(x);
        }
        for x in labels {
            reg.upsert_label(x);
        }
        for x in devices {
            reg.upsert_device(x);
        }
        for x in entities {
            reg.upsert_entity(x);
        }
        Ok(n)
    }

    // ---------- automations ----------

    /// Stores one automation record (document, entity id, enabled, issues).
    pub async fn save_automation(
        &self,
        id: &str,
        record: &serde_json::Value,
        now: DateTime<Utc>,
    ) -> Result<(), RecorderError> {
        sqlx::query(
            "INSERT INTO automations (id, data, updated_at) VALUES (?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET data = excluded.data, updated_at = excluded.updated_at",
        )
        .bind(id)
        .bind(serde_json::to_string(record)?)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_automation(&self, id: &str) -> Result<bool, RecorderError> {
        let res = sqlx::query("DELETE FROM automations WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn load_automations(&self) -> Result<Vec<serde_json::Value>, RecorderError> {
        let rows = sqlx::query("SELECT data FROM automations ORDER BY id")
            .fetch_all(&self.pool)
            .await?;
        rows.iter()
            .map(|r| Ok(serde_json::from_str(r.get::<&str, _>("data"))?))
            .collect()
    }

    // ---------- settings ----------

    pub async fn setting(&self, key: &str) -> Result<Option<String>, RecorderError> {
        let row = sqlx::query("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get::<String, _>("value")))
    }

    /// Sets a setting, or removes it with `None`.
    pub async fn set_setting(&self, key: &str, value: Option<&str>) -> Result<(), RecorderError> {
        match value {
            Some(v) => {
                sqlx::query(
                    "INSERT INTO settings (key, value) VALUES (?, ?)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                )
                .bind(key)
                .bind(v)
                .execute(&self.pool)
                .await?;
            }
            None => {
                sqlx::query("DELETE FROM settings WHERE key = ?")
                    .bind(key)
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(())
    }

    // ---------- dashboards ----------

    pub async fn save_dashboard(
        &self,
        url_path: &str,
        doc: &serde_json::Value,
        now: DateTime<Utc>,
    ) -> Result<(), RecorderError> {
        sqlx::query(
            "INSERT INTO dashboards (url_path, data, updated_at) VALUES (?, ?, ?)
             ON CONFLICT(url_path) DO UPDATE SET data = excluded.data, updated_at = excluded.updated_at",
        )
        .bind(url_path)
        .bind(serde_json::to_string(doc)?)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_dashboard(&self, url_path: &str) -> Result<bool, RecorderError> {
        let res = sqlx::query("DELETE FROM dashboards WHERE url_path = ?")
            .bind(url_path)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn load_dashboards(&self) -> Result<Vec<serde_json::Value>, RecorderError> {
        let rows = sqlx::query("SELECT data FROM dashboards ORDER BY url_path")
            .fetch_all(&self.pool)
            .await?;
        rows.iter()
            .map(|r| Ok(serde_json::from_str(r.get::<&str, _>("data"))?))
            .collect()
    }

    // ---------- history ----------

    pub async fn record_state(&self, s: &State) -> Result<(), RecorderError> {
        sqlx::query("INSERT INTO states (entity_id, state, attributes, last_changed, last_updated) VALUES (?, ?, ?, ?, ?)")
            .bind(s.entity_id.as_str())
            .bind(&s.state)
            .bind(serde_json::to_string(&s.attributes)?)
            .bind(s.last_changed.to_rfc3339())
            .bind(s.last_updated.to_rfc3339())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// History of one entity since `since`, oldest first.
    pub async fn history(
        &self,
        entity_id: &EntityId,
        since: DateTime<Utc>,
    ) -> Result<Vec<State>, RecorderError> {
        let rows = sqlx::query(
            "SELECT state, attributes, last_changed, last_updated FROM states
             WHERE entity_id = ? AND last_updated >= ? ORDER BY last_updated, id",
        )
        .bind(entity_id.as_str())
        .bind(since.to_rfc3339())
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| {
                Ok(State {
                    entity_id: entity_id.clone(),
                    state: r.get("state"),
                    attributes: serde_json::from_str(r.get::<&str, _>("attributes"))?,
                    last_changed: parse_ts(r.get("last_changed")),
                    last_updated: parse_ts(r.get("last_updated")),
                    context: rusthome_model::Context::default(),
                })
            })
            .collect()
    }

    /// Logbook: state changes since `since`, newest first. Numeric sensors
    /// (with a unit) are left out, as in HA, and so are the first rows of
    /// each entity in the window (no previous state to compare with).
    pub async fn logbook(
        &self,
        since: DateTime<Utc>,
        entity_id: Option<&EntityId>,
        limit: u32,
    ) -> Result<Vec<LogbookEntry>, RecorderError> {
        let rows = sqlx::query(
            "SELECT entity_id, state, prev, last_changed FROM (
                 SELECT entity_id, state, last_changed, last_updated, id,
                        json_extract(attributes, '$.unit_of_measurement') AS unit,
                        LAG(state) OVER (PARTITION BY entity_id ORDER BY last_updated, id) AS prev
                 FROM states
                 WHERE last_updated >= ? AND (? IS NULL OR entity_id = ?)
             )
             WHERE prev IS NOT NULL AND prev != state AND unit IS NULL
             ORDER BY last_updated DESC, id DESC
             LIMIT ?",
        )
        .bind(since.to_rfc3339())
        .bind(entity_id.map(EntityId::as_str))
        .bind(entity_id.map(EntityId::as_str))
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| LogbookEntry {
                entity_id: r.get("entity_id"),
                state: r.get("state"),
                previous: r.get("prev"),
                when: parse_ts(r.get("last_changed")),
            })
            .collect())
    }

    /// Moves the history of a renamed entity to its new id. Returns the
    /// number of rows moved.
    pub async fn rename_history(
        &self,
        from: &EntityId,
        to: &EntityId,
    ) -> Result<u64, RecorderError> {
        let res = sqlx::query("UPDATE states SET entity_id = ? WHERE entity_id = ?")
            .bind(to.as_str())
            .bind(from.as_str())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    /// Deletes history older than `keep`. Returns the number of rows removed.
    pub async fn purge(&self, now: DateTime<Utc>, keep: Duration) -> Result<u64, RecorderError> {
        let res = sqlx::query("DELETE FROM states WHERE last_updated < ?")
            .bind((now - keep).to_rfc3339())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    fn is_excluded(&self, hub: &Hub, entity_id: &EntityId) -> bool {
        hub.registries
            .entity_by_entity_id(entity_id)
            .is_some_and(|e| self.excluded_integrations.contains(&e.integration))
    }

    /// Follows the bus: persists registry updates and state changes.
    pub async fn run(self, hub: Hub) {
        let mut rx = hub.bus.subscribe();
        loop {
            let event = match rx.recv().await {
                Ok(e) => e,
                Err(RecvError::Lagged(n)) => {
                    // Registry updates may be among the missed events (e.g. the
                    // first HA snapshot, ~3700 events): resave everything.
                    // Missed state changes are only lost from history.
                    tracing::warn!(missed = n, "recorder lagged: resaving all registries");
                    if let Err(e) = self.save_all_registries(&hub.registries).await {
                        tracing::error!(error = %e, "registry resave failed");
                    }
                    continue;
                }
                Err(RecvError::Closed) => return,
            };
            let res = match &*event {
                Event::RegistryUpdated { kind, id } => {
                    self.save_registry_item(&hub.registries, *kind, id).await
                }
                Event::StateChanged(c) => match &c.new_state {
                    Some(s) if !self.is_excluded(&hub, &s.entity_id) => self.record_state(s).await,
                    _ => Ok(()),
                },
                Event::Custom { .. } => Ok(()),
            };
            if let Err(e) = res {
                tracing::error!(error = %e, "recorder write failed");
            }
        }
    }
}

/// Registry tables. SQL is spelled out per table: sqlx only accepts static
/// query strings (no interpolation, hence no injection).
#[derive(Clone, Copy)]
enum Table {
    Floors,
    Areas,
    Labels,
    Devices,
    Entities,
}

impl Table {
    fn upsert_sql(self) -> &'static str {
        match self {
            Table::Floors => {
                "INSERT INTO floors (id, data) VALUES (?, ?) ON CONFLICT(id) DO UPDATE SET data = excluded.data"
            }
            Table::Areas => {
                "INSERT INTO areas (id, data) VALUES (?, ?) ON CONFLICT(id) DO UPDATE SET data = excluded.data"
            }
            Table::Labels => {
                "INSERT INTO labels (id, data) VALUES (?, ?) ON CONFLICT(id) DO UPDATE SET data = excluded.data"
            }
            Table::Devices => {
                "INSERT INTO devices (id, data) VALUES (?, ?) ON CONFLICT(id) DO UPDATE SET data = excluded.data"
            }
            Table::Entities => {
                "INSERT INTO entities (id, data) VALUES (?, ?) ON CONFLICT(id) DO UPDATE SET data = excluded.data"
            }
        }
    }

    fn select_sql(self) -> &'static str {
        match self {
            Table::Floors => "SELECT data FROM floors ORDER BY id",
            Table::Areas => "SELECT data FROM areas ORDER BY id",
            Table::Labels => "SELECT data FROM labels ORDER BY id",
            Table::Devices => "SELECT data FROM devices ORDER BY id",
            Table::Entities => "SELECT data FROM entities ORDER BY id",
        }
    }
}

/// Timestamps are written by us in RFC 3339; a corrupt value falls back to epoch.
fn parse_ts(raw: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(raw).map_or(DateTime::UNIX_EPOCH, |d| d.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rusthome_core::{EventBus, ManualClock};
    use rusthome_model::{Area, Context, EntityEntry, EntityStatus};
    use serde_json::Map;

    async fn mem() -> Recorder {
        Recorder::open("sqlite::memory:").await.unwrap()
    }

    fn st(entity: &str, value: &str, at: DateTime<Utc>) -> State {
        State {
            entity_id: EntityId::parse(entity).unwrap(),
            state: value.into(),
            attributes: Map::new(),
            last_changed: at,
            last_updated: at,
            context: Context::default(),
        }
    }

    #[tokio::test]
    async fn lagging_recorder_still_persists_every_registry_item() {
        let rec = mem().await;
        let clock = Arc::new(ManualClock::new(
            Utc.with_ymd_and_hms(2026, 10, 17, 12, 0, 0).unwrap(),
        ));
        let hub = Hub::with_bus_capacity(clock, 4);
        let task = tokio::spawn(rec.clone().run(hub.clone()));
        tokio::task::yield_now().await;
        // A burst far larger than the bus, like the first HA snapshot.
        for i in 0..50 {
            hub.registries.upsert_area(Area {
                id: format!("a{i:02}"),
                name: format!("Pièce {i}"),
                floor_id: None,
                icon: None,
                aliases: vec![],
            });
        }
        let fresh = Registries::new(EventBus::new(8));
        for _ in 0..100 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            if rec.load_registries(&fresh).await.unwrap() == 50 {
                break;
            }
        }
        task.abort();
        assert_eq!(fresh.areas().len(), 50);
    }

    #[tokio::test]
    async fn settings_round_trip() {
        let rec = mem().await;
        assert_eq!(rec.setting("default_dashboard").await.unwrap(), None);
        rec.set_setting("default_dashboard", Some("apercu-2"))
            .await
            .unwrap();
        rec.set_setting("default_dashboard", Some("apercu-3"))
            .await
            .unwrap();
        assert_eq!(
            rec.setting("default_dashboard").await.unwrap().as_deref(),
            Some("apercu-3")
        );
        rec.set_setting("default_dashboard", None).await.unwrap();
        assert_eq!(rec.setting("default_dashboard").await.unwrap(), None);
    }

    #[tokio::test]
    async fn dashboards_round_trip() {
        let rec = mem().await;
        let now = Utc.with_ymd_and_hms(2026, 10, 17, 12, 0, 0).unwrap();
        for title in ["A", "B"] {
            rec.save_dashboard(
                "securite",
                &serde_json::json!({"url_path": "securite", "title": title}),
                now,
            )
            .await
            .unwrap();
        }
        let all = rec.load_dashboards().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0]["title"], "B");
        assert!(rec.delete_dashboard("securite").await.unwrap());
        assert!(!rec.delete_dashboard("securite").await.unwrap());
    }

    #[tokio::test]
    async fn automations_round_trip() {
        let rec = mem().await;
        let now = Utc.with_ymd_and_hms(2026, 10, 17, 12, 0, 0).unwrap();
        rec.save_automation(
            "a1",
            &serde_json::json!({"doc": {"id": "a1"}, "enabled": false}),
            now,
        )
        .await
        .unwrap();
        rec.save_automation(
            "a1",
            &serde_json::json!({"doc": {"id": "a1"}, "enabled": true}),
            now,
        )
        .await
        .unwrap();
        let all = rec.load_automations().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0]["enabled"], true);
        assert!(rec.delete_automation("a1").await.unwrap());
        assert_eq!(rec.load_automations().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn registries_round_trip() {
        let rec = mem().await;
        let reg = Registries::new(EventBus::new(8));
        reg.upsert_area(Area {
            id: "salon".into(),
            name: "Salon".into(),
            floor_id: Some("rdc".into()),
            icon: None,
            aliases: vec![],
        });
        rec.save_registry_item(&reg, RegistryKind::Area, "salon")
            .await
            .unwrap();

        let fresh = Registries::new(EventBus::new(8));
        assert_eq!(rec.load_registries(&fresh).await.unwrap(), 1);
        assert_eq!(
            fresh.area("salon").unwrap().floor_id.as_deref(),
            Some("rdc")
        );
    }

    #[tokio::test]
    async fn logbook_keeps_real_changes_only() {
        let rec = mem().await;
        let t0 = Utc.with_ymd_and_hms(2026, 10, 17, 8, 0, 0).unwrap();
        let m = |n| t0 + Duration::minutes(n);
        for (e, v, at) in [
            ("light.salon", "off", m(0)),
            ("light.salon", "off", m(1)), // attribute update only
            ("light.salon", "on", m(2)),
            ("binary_sensor.porte", "off", m(0)),
            ("binary_sensor.porte", "on", m(3)),
        ] {
            rec.record_state(&st(e, v, at)).await.unwrap();
        }
        let mut temp = st("sensor.temp", "20", m(0));
        temp.attributes
            .insert("unit_of_measurement".into(), "°C".into());
        rec.record_state(&temp).await.unwrap();
        temp.state = "21".into();
        temp.last_updated = m(4);
        rec.record_state(&temp).await.unwrap();

        let all = rec.logbook(t0, None, 50).await.unwrap();
        let seen: Vec<(&str, &str, &str)> = all
            .iter()
            .map(|e| (e.entity_id.as_str(), e.previous.as_str(), e.state.as_str()))
            .collect();
        assert_eq!(
            seen,
            [
                ("binary_sensor.porte", "off", "on"),
                ("light.salon", "off", "on")
            ],
            "newest first, no numeric sensor, no repeat"
        );
        let salon = EntityId::parse("light.salon").unwrap();
        assert_eq!(rec.logbook(t0, Some(&salon), 50).await.unwrap().len(), 1);
        assert_eq!(rec.logbook(t0, None, 1).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn history_follows_a_renamed_entity() {
        let rec = mem().await;
        let t0 = Utc.with_ymd_and_hms(2026, 10, 18, 8, 0, 0).unwrap();
        rec.record_state(&st("sensor.salon_t", "20", t0))
            .await
            .unwrap();
        rec.record_state(&st("sensor.salon_t", "21", t0 + Duration::minutes(5)))
            .await
            .unwrap();
        let (old, new) = (
            EntityId::parse("sensor.salon_t").unwrap(),
            EntityId::parse("sensor.sejour_t").unwrap(),
        );
        assert_eq!(rec.rename_history(&old, &new).await.unwrap(), 2);
        assert_eq!(rec.history(&new, t0).await.unwrap().len(), 2);
        assert_eq!(rec.history(&old, t0).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn history_and_purge() {
        let rec = mem().await;
        let t0 = Utc.with_ymd_and_hms(2026, 10, 1, 0, 0, 0).unwrap();
        rec.record_state(&st("sensor.t", "19", t0)).await.unwrap();
        rec.record_state(&st("sensor.t", "21", t0 + Duration::days(9)))
            .await
            .unwrap();
        rec.record_state(&st("sensor.other", "1", t0 + Duration::days(9)))
            .await
            .unwrap();

        let id = EntityId::parse("sensor.t").unwrap();
        let all = rec.history(&id, t0).await.unwrap();
        assert_eq!(
            all.iter().map(|s| s.state.as_str()).collect::<Vec<_>>(),
            ["19", "21"]
        );

        let removed = rec
            .purge(t0 + Duration::days(10), Duration::days(5))
            .await
            .unwrap();
        assert_eq!(removed, 1);
        assert_eq!(rec.history(&id, t0).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn run_persists_bus_events_and_skips_portainer() {
        let rec = mem().await;
        let clock = Arc::new(ManualClock::new(
            Utc.with_ymd_and_hms(2026, 10, 17, 12, 0, 0).unwrap(),
        ));
        let hub = Hub::new(clock.clone());
        let task = tokio::spawn(rec.clone().run(hub.clone()));
        tokio::task::yield_now().await;

        hub.registries.upsert_entity(EntityEntry {
            uid: "p1".into(),
            entity_id: EntityId::parse("sensor.frigate_cpu").unwrap(),
            integration: "ha:portainer".into(),
            unique_id: None,
            device_id: None,
            name: None,
            area_id: None,
            labels: vec![],
            icon: None,
            hidden: false,
            disabled: false,
            status: EntityStatus::Active,
        });
        hub.states.set(
            EntityId::parse("sensor.frigate_cpu").unwrap(),
            "12",
            Map::new(),
            Context::default(),
        );
        hub.states.set(
            EntityId::parse("light.salon").unwrap(),
            "on",
            Map::new(),
            Context::default(),
        );

        // Let the recorder drain the bus.
        for _ in 0..50 {
            tokio::task::yield_now().await;
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            if !rec
                .history(
                    &EntityId::parse("light.salon").unwrap(),
                    DateTime::UNIX_EPOCH,
                )
                .await
                .unwrap()
                .is_empty()
            {
                break;
            }
        }
        task.abort();
        assert_eq!(
            rec.history(
                &EntityId::parse("light.salon").unwrap(),
                DateTime::UNIX_EPOCH
            )
            .await
            .unwrap()
            .len(),
            1
        );
        assert_eq!(
            rec.history(
                &EntityId::parse("sensor.frigate_cpu").unwrap(),
                DateTime::UNIX_EPOCH
            )
            .await
            .unwrap()
            .len(),
            0
        );
        let fresh = Registries::new(EventBus::new(8));
        assert_eq!(
            rec.load_registries(&fresh).await.unwrap(),
            1,
            "entity registry persisted"
        );
    }
}
