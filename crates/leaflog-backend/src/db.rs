//! SQLite storage: a single-connection database guarded for blocking
//! access, plus schema migrations run by [refinery] (spec §3, D048).
//!
//! Migration files live in the crate's `migrations/` directory and are
//! discovered at compile time — adding a `V<n>__<name>.sql` file is the
//! whole workflow (`just migrate new <name>` creates it). Each file
//! applies in its own transaction; the applied history in
//! `refinery_schema_history` is checksum-verified against the embedded
//! set on every open, and a divergent, gapped, or newer-than-binary
//! history refuses to start.
//!
//! Databases from the earlier hand-rolled runner (D001, table
//! `schema_migrations`) are adopted once: the versions recorded there
//! are marked applied via refinery's fake-version target and the legacy
//! table is dropped — the SQL files themselves never changed, so the
//! embedded checksums describe exactly what those databases contain.
//!
//! Per `docs/DECISIONS.md` (D001) access goes through `rusqlite`, run
//! on Tokio's blocking pool via [`Db::with`]. The app is single-user,
//! so one connection is plenty; it is opened at `/data/leaflog.db`
//! inside Home Assistant (see [`crate::Config`]).
//!
//! [refinery]: https://crates.io/crates/refinery

use std::path::Path;
use std::sync::{Arc, Mutex};

use refinery::Target;
use rusqlite::Connection;

/// The embedded SQL migration set (refinery; spec §3). The macro
/// collects every `migrations/V<n>__<name>.sql` file at compile time;
/// `embedded::migrations::runner()` serves it.
// The refinery macro generates the inner module and function; they carry
// no doc comments of their own (see docs/DECISIONS.md D048).
#[allow(missing_docs)]
pub mod embedded {
    refinery::embed_migrations!("./migrations");
}

/// The applied-history table refinery maintains.
const HISTORY_TABLE: &str = "refinery_schema_history";

/// The pre-refinery runner's history table (D001), adopted and dropped
/// on first open with the new runner.
const LEGACY_HISTORY_TABLE: &str = "schema_migrations";

/// A storage-layer error.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// A SQLite error (syntax, constraint, busy, ...).
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// An I/O error while opening the database file.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// A migration failed, or the applied history does not match the
    /// embedded set (checksum mismatch, gap, or a version this binary
    /// does not know).
    #[error("migration error: {0}")]
    Migration(String),
    /// The connection mutex was poisoned by a panic in another task.
    #[error("database connection poisoned")]
    Poisoned,
    /// A blocking task panicked or was cancelled.
    #[error("database task join error: {0}")]
    Join(#[from] tokio::task::JoinError),
}

/// One row of the applied-migration history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMigration {
    /// Version number (the `V<n>` prefix of the file).
    pub version: i64,
    /// File-name part of the migration (`V2__grows_actions_photos` →
    /// `grows_actions_photos`).
    pub name: String,
    /// When the migration ran, RFC 3339; `None` for not-yet-applied
    /// entries reported as pending.
    pub applied_on: Option<String>,
}

/// What `migrate status` reports: applied rows, pending files, and —
/// for a database still carrying the pre-refinery history — the legacy
/// version awaiting adoption.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationStatus {
    /// Applied migrations, in version order.
    pub applied: Vec<AppliedMigration>,
    /// Embedded files not yet applied, in version order.
    pub pending: Vec<AppliedMigration>,
    /// Highest version of a legacy `schema_migrations` history that
    /// [`migrate_apply`] will adopt.
    pub legacy_version: Option<i64>,
}

/// The database handle, cheap to clone.
#[derive(Debug, Clone)]
pub struct Db(Arc<Mutex<Connection>>);

impl Db {
    /// Opens (creating parent directories and the file as needed) and
    /// migrates the database at `path`.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] when the file cannot be created, opened,
    /// or migrated — including a refusal to start when the applied
    /// history diverges from the embedded migration set.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let conn = open_connection(path)?;
        Self::init(conn)
    }

    /// Opens a private in-memory database (tests, tooling).
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] when migrations fail.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self, StoreError> {
        let mut conn = conn;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        migrate(&mut conn)?;
        Ok(Self(Arc::new(Mutex::new(conn))))
    }

    /// Runs a closure with the connection on Tokio's blocking pool. The
    /// mutex guard never crosses an `.await` — this method guarantees it
    /// by construction.
    ///
    /// # Errors
    ///
    /// Returns whatever error type the closure produces, with lock
    /// poisoning and join failures folded in via [`StoreError`].
    pub async fn with<T, E, F>(&self, f: F) -> Result<T, E>
    where
        F: FnOnce(&Connection) -> Result<T, E> + Send + 'static,
        E: From<StoreError> + Send + 'static,
        T: Send + 'static,
    {
        let db = Arc::clone(&self.0);
        let work = tokio::task::spawn_blocking(move || {
            let conn = lock(&db)?;
            f(&conn)
        });
        work.await.map_err(|e| E::from(StoreError::from(e)))?
    }

    /// The current schema version (highest applied migration).
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] when the query fails.
    pub fn version(conn: &Connection) -> Result<i64, StoreError> {
        Ok(conn.query_row(
            &format!("SELECT COALESCE(MAX(version), 0) FROM {HISTORY_TABLE}"),
            [],
            |row| row.get(0),
        )?)
    }
}

/// Opens (creating directories and file as needed) a connection without
/// migrating; shared by [`Db::open`] and the `migrate` subcommand.
fn open_connection(path: &Path) -> Result<Connection, StoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(Connection::open(path)?)
}

/// Locks the connection, mapping poisoning to [`StoreError::Poisoned`].
fn lock(db: &Mutex<Connection>) -> Result<std::sync::MutexGuard<'_, Connection>, StoreError> {
    db.lock().map_err(|poisoned| {
        drop(poisoned.into_inner());
        StoreError::Poisoned
    })
}

/// Applies all pending migrations (adopting a legacy history first if
/// one is present); returns the names of the migrations that ran.
///
/// Boot-time apply via [`Db::open`] is the normal path; this is the
/// `migrate apply` subcommand and test entry point.
///
/// # Errors
///
/// Returns a [`StoreError`] when a migration fails or the applied
/// history does not match the embedded set.
pub fn migrate_apply(path: &Path) -> Result<Vec<String>, StoreError> {
    let mut conn = open_connection(path)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    let before = status_of(&conn)?;
    migrate(&mut conn)?;
    Ok(before.pending.into_iter().map(|m| m.name).collect())
}

/// Reads the applied and pending migrations without changing anything
/// (beyond creating an empty database file when none exists).
///
/// # Errors
///
/// Returns a [`StoreError`] when the database cannot be opened or read.
pub fn migrate_status(path: &Path) -> Result<MigrationStatus, StoreError> {
    let conn = open_connection(path)?;
    status_of(&conn)
}

/// Adopts a legacy history when present, then applies pending
/// migrations. Refinery verifies the applied history against the
/// embedded set (names and checksums) on every run and errors on a
/// divergent, gapped, or unknown-version history.
fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    adopt_legacy_history(conn)?;
    embedded::migrations::runner()
        .run(conn)
        .map_err(|error| StoreError::Migration(error.to_string()))?;
    Ok(())
}

/// Adopts the D001 runner's `schema_migrations` table: the versions it
/// recorded are marked applied with the embedded names and checksums
/// (the SQL files never changed, so the checksums describe exactly what
/// those databases contain), and the legacy table is dropped. A no-op
/// for databases refinery already tracks; the table is also dropped
/// then, in case an adoption was interrupted between marking and
/// dropping.
fn adopt_legacy_history(conn: &mut Connection) -> Result<(), StoreError> {
    let runner = embedded::migrations::runner();
    let embedded_count =
        i64::try_from(runner.get_migrations().len()).expect("migration count fits in i64");

    let table = |name: &str| -> Result<bool, StoreError> {
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [name],
            |row| row.get(0),
        )?)
    };
    if table(HISTORY_TABLE)? {
        // Interrupted adoption: refinery knows the history, the legacy
        // table just never got dropped.
        if table(LEGACY_HISTORY_TABLE)? {
            conn.execute(&format!("DROP TABLE {LEGACY_HISTORY_TABLE}"), [])?;
        }
        return Ok(());
    }
    let Some(legacy_version) = legacy_history_version(conn)? else {
        return Ok(());
    };
    if legacy_version > embedded_count {
        return Err(StoreError::Migration(format!(
            "database history is at version {legacy_version}, but this binary only knows \
             {embedded_count} migrations; refusing to start against a newer database"
        )));
    }
    let target = i32::try_from(legacy_version)
        .expect("legacy version is bounded by the embedded count above");
    runner
        .set_target(Target::FakeVersion(target))
        .run(conn)
        .map_err(|error| StoreError::Migration(error.to_string()))?;
    conn.execute(&format!("DROP TABLE {LEGACY_HISTORY_TABLE}"), [])?;
    Ok(())
}

/// The highest version of a legacy `schema_migrations` table, if the
/// table exists at all.
fn legacy_history_version(conn: &Connection) -> Result<Option<i64>, StoreError> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [LEGACY_HISTORY_TABLE],
        |row| row.get(0),
    )?;
    if !exists {
        return Ok(None);
    }
    Ok(Some(conn.query_row(
        &format!("SELECT COALESCE(MAX(version), 0) FROM {LEGACY_HISTORY_TABLE}"),
        [],
        |row| row.get(0),
    )?))
}

/// Builds the [`MigrationStatus`] of an open connection: applied rows
/// from refinery's table (or the legacy version when adoption is still
/// pending), pending files from the embedded set.
fn status_of(conn: &Connection) -> Result<MigrationStatus, StoreError> {
    let runner = embedded::migrations::runner();
    let embedded = runner.get_migrations().clone();
    let mut applied: Vec<AppliedMigration> = Vec::new();
    let mut legacy_version = None;

    let refinery_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [HISTORY_TABLE],
        |row| row.get(0),
    )?;
    if refinery_exists {
        let mut statement = conn.prepare(&format!(
            "SELECT version, name, applied_on FROM {HISTORY_TABLE} ORDER BY version"
        ))?;
        let rows = statement.query_map([], |row| {
            Ok(AppliedMigration {
                version: row.get(0)?,
                name: row.get(1)?,
                applied_on: row.get(2)?,
            })
        })?;
        for row in rows {
            applied.push(row?);
        }
    } else {
        legacy_version = legacy_history_version(conn)?;
    }

    let applied_upto =
        legacy_version.unwrap_or_else(|| applied.last().map_or(0, |migration| migration.version));
    let pending = embedded
        .iter()
        .filter(|migration| i64::from(migration.version()) > applied_upto)
        .map(|migration| AppliedMigration {
            version: i64::from(migration.version()),
            name: migration.name().to_owned(),
            applied_on: None,
        })
        .collect();
    Ok(MigrationStatus {
        applied,
        pending,
        legacy_version,
    })
}

/// The current UTC instant as an RFC 3339 string.
///
/// # Panics
///
/// Cannot panic: formatting a UTC instant as RFC 3339 always succeeds.
#[must_use]
pub fn now_rfc3339() -> String {
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("RFC 3339 formatting of a UTC instant cannot fail")
}

/// A fresh ULID as the canonical entity id.
#[must_use]
pub fn new_id() -> String {
    ulid::Ulid::new().to_string()
}

#[cfg(test)]
mod tests {
    use super::{Db, StoreError, embedded};

    fn embedded_count() -> i64 {
        i64::try_from(embedded::migrations::runner().get_migrations().len())
            .expect("migration count fits in i64")
    }

    #[tokio::test]
    async fn in_memory_database_migrates_to_the_latest_version() -> Result<(), StoreError> {
        let db = Db::open_in_memory()?;
        db.with(|conn| -> Result<(), StoreError> {
            assert_eq!(Db::version(conn)?, embedded_count());
            let applied: i64 =
                conn.query_row("SELECT COUNT(*) FROM refinery_schema_history", [], |row| {
                    row.get(0)
                })?;
            assert_eq!(applied, embedded_count());
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn file_database_is_created_and_migrated() -> Result<(), StoreError> {
        let dir = std::env::temp_dir().join(format!("leaflog-db-test-{}", super::new_id()));
        let path = dir.join("nested").join("leaflog.db");
        let db = Db::open(&path)?;
        db.with(|conn| -> Result<(), StoreError> {
            assert_eq!(Db::version(conn)?, embedded_count());
            Ok(())
        })
        .await?;
        assert!(path.exists(), "database file must be created");
        std::fs::remove_dir_all(&dir).ok();
        Ok(())
    }

    #[tokio::test]
    async fn reopening_applies_nothing_new() -> Result<(), StoreError> {
        let dir = std::env::temp_dir().join(format!("leaflog-db-test-{}", super::new_id()));
        let path = dir.join("leaflog.db");
        drop(Db::open(&path)?);
        let db = Db::open(&path)?;
        db.with(|conn| -> Result<(), StoreError> {
            assert_eq!(Db::version(conn)?, embedded_count());
            let applied: i64 =
                conn.query_row("SELECT COUNT(*) FROM refinery_schema_history", [], |row| {
                    row.get(0)
                })?;
            assert_eq!(applied, embedded_count());
            Ok(())
        })
        .await?;
        std::fs::remove_dir_all(&dir).ok();
        Ok(())
    }
}
