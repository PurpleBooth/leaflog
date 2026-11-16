-- Leaflog schema migration V15 (seed-bank removal, feature-strip
-- D075).
--
-- The seed bank leaves the product end to end: the batch log and its
-- germination tests are dropped, and the grow's link to a batch goes
-- with them. The drop is one-way; grows, plants and log entries are
-- untouched (the child rows of `seed_batch` die with the table, the
-- grow table itself only loses its nullable link column).
--
-- `ALTER TABLE ... DROP COLUMN` works here despite the column's
-- inline `REFERENCES seed_batch(id)`: SQLite refuses a drop only when
-- the column is the *target* of a foreign key (or part of a key,
-- index, check, trigger or view) — a child-side reference drops fine.
-- Dropping the parent table first fires the column's ON DELETE SET
-- NULL on the way out, so no stale link survives even between the
-- statements.
--
-- Conventions as in V1: ULID TEXT ids, RFC 3339 UTC timestamps,
-- snake_case labels of leaflog-core enums validated in Rust.

DROP TABLE germination_log;
DROP TABLE seed_batch;
ALTER TABLE grow DROP COLUMN seed_batch_id;
