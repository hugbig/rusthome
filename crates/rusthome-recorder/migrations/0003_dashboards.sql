-- Dashboards: RustHome document (Lovelace-shaped JSON), keyed by url_path.
CREATE TABLE dashboards (
    url_path   TEXT PRIMARY KEY NOT NULL,
    data       TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
