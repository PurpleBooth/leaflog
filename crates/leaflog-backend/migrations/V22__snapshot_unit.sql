-- Record the stored unit (Celsius for normalized temperatures).
ALTER TABLE sensor_snapshot ADD COLUMN unit TEXT;
