//! Bundled strain seed data (spec §4.4).
//!
//! The curated records in `seeds/strains.json` carry a source URL, a
//! licence note and per-field evidence tags; per the seed-data
//! discipline, numbers are ranges wherever sources disagree. Seeding
//! runs only when the strain table is empty (first boot or a wiped
//! database) — it never overwrites user data.

use std::collections::BTreeMap;

use leaflog_core::strain::{EvidenceTag, StrainInput};
use serde::Deserialize;

use crate::db::{Db, StoreError};
use crate::store;

/// The bundled seed records.
const SEED: &str = include_str!("seeds/strains.json");

/// One bundled record: an editable payload plus provenance.
#[derive(Debug, Deserialize)]
struct StrainSeed {
    /// The editable payload (same shape the API accepts).
    #[serde(flatten)]
    input: StrainInput,
    /// Per-field evidence tags for the numbers above.
    evidence: BTreeMap<String, EvidenceTag>,
    /// Where this record's facts came from.
    source_url: String,
    /// Licence terms for bundling these facts.
    source_licence: String,
}

/// A seeding failure (unreadable seed file or a storage error).
#[derive(Debug, thiserror::Error)]
pub enum SeedError {
    /// The embedded seed JSON does not match the expected shape.
    #[error("seed file invalid: {0}")]
    Invalid(#[from] serde_json::Error),
    /// Writing the seed rows failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Inserts the bundled strains when the table is empty; returns how
/// many rows were inserted.
///
/// # Errors
///
/// Returns a [`SeedError`] when the seed file cannot be parsed or the
/// inserts fail.
pub async fn seed_strains_if_empty(db: &Db) -> Result<u32, SeedError> {
    let existing = db.with(store::strain::count).await?;
    if existing > 0 {
        return Ok(0);
    }
    let seeds: Vec<StrainSeed> = serde_json::from_str(SEED)?;
    let inserted = db
        .with(move |conn| -> Result<u32, SeedError> {
            let mut inserted = 0_u32;
            for seed in &seeds {
                let mut strain = store::strain::from_input(&seed.input);
                strain.evidence = seed.evidence.clone();
                strain.source_url = Some(seed.source_url.clone());
                strain.source_licence = Some(seed.source_licence.clone());
                store::strain::create(conn, &strain)?;
                inserted += 1;
            }
            Ok(inserted)
        })
        .await?;
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::{SEED, SeedError, seed_strains_if_empty};
    use crate::db::Db;

    #[tokio::test]
    async fn seed_file_is_valid_and_seeds_once() -> Result<(), SeedError> {
        let parsed: serde_json::Value = serde_json::from_str(SEED)?;
        assert!(parsed.as_array().is_some_and(|a| a.len() >= 4));

        let db = Db::open_in_memory()?;
        let first = seed_strains_if_empty(&db).await?;
        assert!(first >= 4, "first boot seeds the bundled records");
        let second = seed_strains_if_empty(&db).await?;
        assert_eq!(second, 0, "second run must not duplicate seeds");
        Ok(())
    }

    #[tokio::test]
    async fn seeded_records_carry_provenance() -> Result<(), SeedError> {
        let db = Db::open_in_memory()?;
        seed_strains_if_empty(&db).await?;
        let strains = db.with(crate::store::strain::list).await?;
        assert!(strains.iter().all(|strain| {
            strain
                .source_url
                .as_deref()
                .is_some_and(|url| !url.is_empty())
                && strain
                    .source_licence
                    .as_deref()
                    .is_some_and(|licence| !licence.is_empty())
        }));
        Ok(())
    }
}
