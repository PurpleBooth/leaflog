-- Leaflog schema migration 0003 (M4: sensor bindings, snapshots).

CREATE TABLE sensor_binding (
    grow_id    TEXT NOT NULL REFERENCES grow(id) ON DELETE CASCADE,
    role       TEXT NOT NULL,   -- temperature | humidity | co2 | soil_moisture |
                                -- light | reservoir_ph | reservoir_ec
    entity_id  TEXT NOT NULL,   -- Home Assistant entity_id
    updated_at TEXT NOT NULL,
    PRIMARY KEY (grow_id, role)
);

CREATE TABLE sensor_snapshot (
    id            TEXT PRIMARY KEY,
    action_log_id TEXT NOT NULL REFERENCES action_log(id) ON DELETE CASCADE,
    role          TEXT NOT NULL,
    entity_id     TEXT NOT NULL,
    state         TEXT NOT NULL,   -- raw state string at capture time
    captured_at   TEXT NOT NULL
);

CREATE INDEX idx_snapshot_action ON sensor_snapshot(action_log_id);
