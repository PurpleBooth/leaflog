-- Chart requests sort sensor snapshots by capture time; without this
-- index every chart view scans the table (audit D145 finding 11).
CREATE INDEX idx_snapshot_captured ON sensor_snapshot (captured_at);
