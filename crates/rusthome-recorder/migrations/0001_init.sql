-- Registries: one JSON document per row, typed by rusthome-model.
CREATE TABLE floors   (id TEXT PRIMARY KEY NOT NULL, data TEXT NOT NULL);
CREATE TABLE areas    (id TEXT PRIMARY KEY NOT NULL, data TEXT NOT NULL);
CREATE TABLE labels   (id TEXT PRIMARY KEY NOT NULL, data TEXT NOT NULL);
CREATE TABLE devices  (id TEXT PRIMARY KEY NOT NULL, data TEXT NOT NULL);
CREATE TABLE entities (id TEXT PRIMARY KEY NOT NULL, data TEXT NOT NULL);

-- State history (retention applied by Recorder::purge).
CREATE TABLE states (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_id    TEXT NOT NULL,
    state        TEXT NOT NULL,
    attributes   TEXT NOT NULL,
    last_changed TEXT NOT NULL,
    last_updated TEXT NOT NULL
);
CREATE INDEX states_entity_time ON states (entity_id, last_updated);
CREATE INDEX states_time ON states (last_updated);
