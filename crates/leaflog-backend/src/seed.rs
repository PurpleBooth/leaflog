//! Substrate-preset seeding (spec §5.3).
//!
//! The substrate profiles are seeded into an empty table on boot
//! (first boot or a wiped database) — it never overwrites user
//! data. Feeding schedules ship no bundled records: the library
//! starts empty by design (ADR 0004). The bundled data of the removed
//! catalogue features left with them: strains with the strain
//! catalogue (D075), equipment with the spaces/equipment removal
//! (D078). This module is all the bundled data that remains.

use crate::db::{Db, StoreError};
use crate::store;

/// A seeding failure (a storage error).
#[derive(Debug, thiserror::Error)]
pub enum SeedError {
    /// Writing the seed rows failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Seeds the substrate profiles (only into an empty table; part of
/// first boot, D079: previously a side effect of strain seeding).
///
/// # Errors
///
/// Returns a [`SeedError`] when the inserts fail.
pub async fn seed_feed_presets(db: &Db) -> Result<(), SeedError> {
    // Substrate profiles.
    let profiles = db.with(store::feeds::profile_count).await?;
    if profiles == 0 {
        let presets = leaflog_core::feed::substrate_presets();
        db.with(move |conn| -> Result<(), crate::db::StoreError> {
            let tx = conn.unchecked_transaction()?;
            for profile in &presets {
                store::feeds::upsert_profile(&tx, profile)?;
            }
            tx.commit()?;
            Ok(())
        })
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SeedError, seed_feed_presets};
    use crate::db::Db;

    #[tokio::test]
    async fn substrate_presets_seed_into_an_empty_table_only() -> Result<(), SeedError> {
        let db = Db::open_in_memory()?;
        seed_feed_presets(&db).await?;
        let profiles = profile_count(&db).await?;
        assert!(profiles >= 3, "soil, coco and hydro profiles seeded");
        // A second pass is a no-op (only an empty table is filled).
        seed_feed_presets(&db).await?;
        assert_eq!(profile_count(&db).await?, profiles, "no duplication");
        Ok(())
    }

    // Provenance note: the presets are generated from the §5.3
    // substrate science presets, not third-party records — their
    // `source` column names the derivation, asserted here once.
    #[tokio::test]
    async fn seeded_presets_carry_their_source() -> Result<(), SeedError> {
        let db = Db::open_in_memory()?;
        seed_feed_presets(&db).await?;
        let rows = db
            .with(
                |conn| -> Result<Vec<(String, String)>, crate::db::StoreError> {
                    let mut stmt =
                        conn.prepare("SELECT label, source FROM substrate_profile ORDER BY label")?;
                    let rows = stmt
                        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(rows)
                },
            )
            .await?;
        assert!(rows.len() >= 3, "profiles seeded");
        assert!(
            rows.iter().all(|(_, source)| source.contains("§5.3")),
            "the generated presets state where they come from"
        );
        Ok(())
    }

    async fn profile_count(db: &Db) -> Result<i64, crate::db::StoreError> {
        db.with(|conn| {
            conn.query_row("SELECT COUNT(*) FROM substrate_profile", [], |r| r.get(0))
                .map_err(crate::db::StoreError::from)
        })
        .await
    }
}
