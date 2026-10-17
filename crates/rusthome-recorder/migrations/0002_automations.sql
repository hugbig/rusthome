-- Automations: canonical HA-shaped document + RustHome metadata (JSON).
CREATE TABLE automations (
    id         TEXT PRIMARY KEY NOT NULL,
    data       TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
