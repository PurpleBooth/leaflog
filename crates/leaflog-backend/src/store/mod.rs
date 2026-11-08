//! Persistence for domain objects: SQL row ⇄ `leaflog-core` type mapping
//! on top of [`crate::db`].

pub mod actions;
pub mod equipment;
pub mod grows;
pub mod photos;
pub mod settings;
pub mod strain;

use crate::db::StoreError;

/// The row was not found (maps to HTTP 404).
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("not found")]
pub struct NotFound;

/// A store error with the common "missing row" case teased out.
pub type StoreResult<T> = Result<T, StoreOrNotFound>;

/// Either a [`StoreError`] or a [`NotFound`].
#[derive(Debug, thiserror::Error)]
pub enum StoreOrNotFound {
    /// A storage failure.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The addressed row does not exist.
    #[error("not found")]
    NotFound(#[from] NotFound),
}

impl From<rusqlite::Error> for StoreOrNotFound {
    fn from(error: rusqlite::Error) -> Self {
        StoreError::Sqlite(error).into()
    }
}
