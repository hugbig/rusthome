-- Server-side settings (default dashboard…), one JSON-free text value per key.
CREATE TABLE settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
