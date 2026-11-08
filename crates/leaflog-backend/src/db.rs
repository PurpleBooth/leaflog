//! SQLite storage: a single-connection database guarded for blocking
//! access, plus the versioned-migration runner.
//!
//! Per `docs/DECISIONS.md` (D001) access goes through `rusqlite`, run
//! on Tokio's blocking pool via [`Db::with`]. The app is single-user,
//! so one connection is plenty; it is opened at `/data/leaflog.db`
//! inside Home Assistant (see [`crate::Config`]).

use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

/// Embedded schema migrations, applied in order. Array index + 1 is the
/// schema version. Files live in the crate's `migrations/` directory.
static MIGRATIONS: &[&str] = &[
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0001_init.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0002_grows_actions_photos.sql"
    )),
];

/// A storage-layer error.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// A SQLite error (syntax, constraint, busy, ...).
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// An I/O error while opening the database file.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// The connection mutex was poisoned by a panic in another task.
    #[error("database connection poisoned")]
    Poisoned,
    /// A blocking task panicked or was cancelled.
    #[error("database task join error: {0}")]
    Join(#[from] tokio::task::JoinError),
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
    /// or migrated.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
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
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        migrate(&conn)?;
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
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?)
    }
}

/// Locks the connection, mapping poisoning to [`StoreError::Poisoned`].
fn lock(db: &Mutex<Connection>) -> Result<std::sync::MutexGuard<'_, Connection>, StoreError> {
    db.lock().map_err(|poisoned| {
        drop(poisoned.into_inner());
        StoreError::Poisoned
    })
}

/// Applies every pending migration in its own transaction.
fn migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
             version    INTEGER PRIMARY KEY,
             applied_at TEXT NOT NULL
         );",
    )?;
    let current = Db::version(conn).map_err(|e| match e {
        StoreError::Sqlite(sqlite) => sqlite,
        other => unreachable!("version() only fails with SQLite errors, got {other:?}"),
    })?;
    for (index, sql) in MIGRATIONS.iter().enumerate() {
        let version = i64::try_from(index + 1).expect("migration index always fits in i64");
        if version <= current {
            continue;
        }
        let applied_at = now_rfc3339();
        conn.execute_batch(&format!(
            "BEGIN IMMEDIATE;
             {sql}
             INSERT INTO schema_migrations (version, applied_at)
                 VALUES ({version}, '{applied_at}');
             COMMIT;"
        ))?;
    }
    Ok(())
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
    use super::{Db, StoreError};

    #[tokio::test]
    async fn in_memory_database_migrates_to_version_1() -> Result<(), StoreError> {
        let db = Db::open_in_memory()?;
        db.with(|conn| -> Result<(), StoreError> {
            assert_eq!(Db::version(conn)?, 2);
            let applied = conn.query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| {
                r.get::<_, i64>(0)
            })?;
            assert_eq!(applied, 2);
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
            assert_eq!(Db::version(conn)?, 2);
            Ok(())
        })
        .await?;
        assert!(path.exists(), "database file must be created");
        std::fs::remove_dir_all(&dir).ok();
        Ok(())
    }

    #[tokio::test]
    async fn reopening_applies_nothing_new() -> Result<(), StoreError> {
        let db = Db::open_in_memory()?;
        db.with(|conn| -> Result<(), StoreError> { Ok(super::migrate(conn)?) })
            .await?;
        db.with(|conn| -> Result<(), StoreError> {
            assert_eq!(Db::version(conn)?, 2);
            Ok(())
        })
        .await
    }
}
