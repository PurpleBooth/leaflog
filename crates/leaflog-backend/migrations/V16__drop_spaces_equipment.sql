-- Leaflog schema migration V16 (spaces + equipment-catalogue removal,
-- feature-strip D075).
--
-- Grow spaces and the equipment catalogue leave the product end to
-- end: the space table, its equipment link and the catalogue table are
-- dropped, and the grow's link to a space goes with them. HA entities
-- enter the app only as per-grow sensor bindings, which never depended
-- on equipment rows — `sensor_binding` and `sensor_snapshot` (V3) are
-- untouched, so live tiles, charts, the light auto-log and the WS
-- subscription keep running on bindings alone.
--
-- The drop is one-way; grows, plants and log entries are untouched
-- (the child rows of `grow_space` and `equipment_item` die with the
-- tables, the grow table itself only loses its nullable link column).
--
-- `ALTER TABLE ... DROP COLUMN` works here despite the column's
-- inline `REFERENCES grow_space(id)`: SQLite refuses a drop only when
-- the column is the *target* of a foreign key (or part of a key,
-- index, check, trigger or view) — a child-side reference drops fine
-- (V15 proved the same shape). Dropping the parent table first fires
-- the column's ON DELETE SET NULL on the way out, so no stale link
-- survives even between the statements. The link table is dropped
-- before its two parents so its cascades have nothing to fire.
--
-- Conventions as in V1: ULID TEXT ids, RFC 3339 UTC timestamps,
-- snake_case labels of leaflog-core enums validated in Rust.

DROP TABLE grow_space_equipment;
DROP TABLE grow_space;
DROP TABLE equipment_item;
ALTER TABLE grow DROP COLUMN grow_space_id;
