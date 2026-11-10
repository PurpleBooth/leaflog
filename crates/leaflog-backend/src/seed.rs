//! Bundled strain and equipment seed data (spec §4.1, §4.4).
//!
//! The curated records in `seeds/strains.json` carry a source URL, a
//! licence note and per-field evidence tags; per the seed-data
//! discipline, numbers are ranges wherever sources disagree.
//! `seeds/equipment.json` holds the bundled defaults of the equipment
//! catalogue (product facts only, no personal purchase details — the
//! repo is public), stamped `is_builtin` on first boot. Seeding runs
//! only when the corresponding table is empty (first boot or a wiped
//! database) — it never overwrites user data.

use std::collections::BTreeMap;

use leaflog_core::equipment::EquipmentInput;
use leaflog_core::strain::{EvidenceTag, StrainInput};
use serde::Deserialize;

use crate::db::{Db, StoreError};
use crate::store;

/// The bundled seed records.
const SEED: &str = include_str!("seeds/strains.json");

/// The bundled predefined equipment records.
const EQUIPMENT_SEED: &str = include_str!("seeds/equipment.json");

/// Provenance note stamped on bundled equipment catalogue entries.
const EQUIPMENT_SOURCE: &str = "bundled default (docs/DECISIONS.md D012)";

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
    // Substrate profiles + feed templates (separate check: they go into
    // their own tables and seed independently of strains).
    if let Err(error) = seed_feeds(db).await {
        eprintln!("feed/substrate seeding skipped: {error}");
    }

    let inserted = db
        .with(move |conn| -> Result<u32, SeedError> {
            // One transaction per pass: a partial seed must not persist,
            // or the count>0 guard above would make it permanent.
            let tx = conn.unchecked_transaction().map_err(StoreError::from)?;
            let mut inserted = 0_u32;
            for seed in &seeds {
                let mut strain = store::strain::from_input(&seed.input);
                strain.evidence = seed.evidence.clone();
                strain.is_builtin = true;
                strain.source_url = Some(seed.source_url.clone());
                strain.source_licence = Some(seed.source_licence.clone());
                store::strain::create(&tx, &strain)?;
                inserted += 1;
            }
            tx.commit().map_err(StoreError::from)?;
            Ok(inserted)
        })
        .await?;
    Ok(inserted)
}

/// Inserts the bundled predefined equipment when the table is empty;
/// returns how many rows were inserted.
///
/// # Errors
///
/// Returns a [`SeedError`] when the seed file cannot be parsed or the
/// inserts fail.
pub async fn seed_equipment_if_empty(db: &Db) -> Result<u32, SeedError> {
    let existing = db.with(store::equipment::count).await?;
    if existing > 0 {
        return Ok(0);
    }
    let seeds: Vec<EquipmentInput> = serde_json::from_str(EQUIPMENT_SEED)?;
    db.with(move |conn| -> Result<u32, SeedError> {
        // One transaction per pass: a partial seed must not persist, or
        // the count>0 guard above would make it permanent.
        let tx = conn.unchecked_transaction().map_err(StoreError::from)?;
        let mut inserted = 0_u32;
        for input in &seeds {
            store::equipment::create(&tx, input)?;
            inserted += 1;
        }
        // Mark the whole freshly-seeded set as bundled: the count>0
        // guard above means every row in the table is a seed. Rows
        // already present in upgraded databases stay unflagged
        // (docs/DECISIONS.md D020).
        tx.execute(
            "UPDATE equipment_item SET is_builtin = 1, source = ?1",
            rusqlite::params![EQUIPMENT_SOURCE],
        )
        .map_err(StoreError::from)?;
        tx.commit().map_err(StoreError::from)?;
        Ok(inserted)
    })
    .await
}

/// Seeds substrate profiles and generic feed templates (only into
/// empty tables).
async fn seed_feeds(db: &Db) -> Result<(), SeedError> {
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

    // Generic feed templates (one per substrate; simple EC ramps from
    // the substrate profile's own bands).
    let template_count: i64 = db
        .with(|conn| {
            conn.query_row("SELECT COUNT(*) FROM feed_template", [], |r| r.get(0))
                .map_err(crate::db::StoreError::from)
        })
        .await?;
    if template_count == 0 {
        let presets = leaflog_core::feed::substrate_presets();
        for (label, index) in [("soil", 0_usize), ("coco", 1), ("hydro", 2)] {
            let Some(profile) = presets.get(index).cloned() else {
                continue;
            };
            let rows = generic_template_rows(&profile);
            let template = store::feeds::FeedTemplate {
                id: crate::db::new_id(),
                name: format!("Generic {label} feed"),
                substrate: label.to_owned(),
                source: Some("derived from spec §5.3 substrate presets".to_owned()),
                is_builtin: true,
                rows,
                created_at: crate::db::now_rfc3339(),
                updated_at: crate::db::now_rfc3339(),
            };
            db.with(move |conn| -> Result<(), crate::db::StoreError> {
                let tx = conn.unchecked_transaction()?;
                tx.execute(
                    "INSERT INTO feed_template (id, name, substrate, source, is_builtin, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
                    rusqlite::params![
                        template.id,
                        template.name,
                        template.substrate,
                        template.source,
                        template.created_at,
                        template.updated_at,
                    ],
                )?;
                for row in &template.rows {
                    tx.execute(
                        "INSERT INTO feed_template_row (id, template_id, phase, day_offset, ec_target_us, ph_min, ph_max)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        rusqlite::params![
                            crate::db::new_id(),
                            template.id,
                            row.phase.label(),
                            i64::from(row.day_offset),
                            i64::from(row.ec_target_us),
                            row.ph_min,
                            row.ph_max,
                        ],
                    )?;
                }
                tx.commit()?;
                Ok(())
            })
            .await?;
        }
    }
    Ok(())
}

/// Builds a simple EC ramp template from a substrate profile's bands.
fn generic_template_rows(
    profile: &leaflog_core::feed::SubstrateProfile,
) -> Vec<leaflog_core::feed::FeedRow> {
    use leaflog_core::feed::{FeedPhase, FeedRow};
    let mid = |band: (u32, u32)| u32::midpoint(band.0, band.1);
    vec![
        FeedRow {
            phase: FeedPhase::Seedling,
            day_offset: 0,
            ec_target_us: mid(profile.ec_bands.seedling),
            ph_min: profile.ph_min,
            ph_max: profile.ph_max,
        },
        FeedRow {
            phase: FeedPhase::EarlyVeg,
            day_offset: 0,
            ec_target_us: mid(profile.ec_bands.early_veg),
            ph_min: profile.ph_min,
            ph_max: profile.ph_max,
        },
        FeedRow {
            phase: FeedPhase::LateVeg,
            day_offset: 14,
            ec_target_us: mid(profile.ec_bands.late_veg),
            ph_min: profile.ph_min,
            ph_max: profile.ph_max,
        },
        FeedRow {
            phase: FeedPhase::EarlyFlower,
            day_offset: 0,
            ec_target_us: mid(profile.ec_bands.early_flower),
            ph_min: profile.ph_min,
            ph_max: profile.ph_max,
        },
        FeedRow {
            phase: FeedPhase::MidFlower,
            day_offset: 21,
            ec_target_us: mid(profile.ec_bands.peak_flower),
            ph_min: profile.ph_min,
            ph_max: profile.ph_max,
        },
        FeedRow {
            phase: FeedPhase::LateFlower,
            day_offset: 42,
            ec_target_us: mid(profile.ec_bands.late_flower),
            ph_min: profile.ph_min,
            ph_max: profile.ph_max,
        },
        FeedRow {
            phase: FeedPhase::Ripen,
            day_offset: 0,
            ec_target_us: mid(profile.ec_bands.late_flower) / 2,
            ph_min: profile.ph_min,
            ph_max: profile.ph_max,
        },
        FeedRow {
            phase: FeedPhase::Flush,
            day_offset: 0,
            ec_target_us: 0,
            ph_min: profile.ph_min,
            ph_max: profile.ph_max,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{EQUIPMENT_SEED, SEED, SeedError, seed_equipment_if_empty, seed_strains_if_empty};
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
    async fn equipment_seed_file_is_valid_and_seeds_once() -> Result<(), SeedError> {
        let parsed: serde_json::Value = serde_json::from_str(EQUIPMENT_SEED)?;
        assert!(parsed.as_array().is_some_and(|a| !a.is_empty()));

        let db = Db::open_in_memory()?;
        let first = seed_equipment_if_empty(&db).await?;
        assert!(first >= 1, "first boot seeds the predefined equipment");
        let second = seed_equipment_if_empty(&db).await?;
        assert_eq!(second, 0, "second run must not duplicate seeds");
        Ok(())
    }

    #[tokio::test]
    async fn seeded_equipment_covers_only_catalogue_categories() -> Result<(), SeedError> {
        let db = Db::open_in_memory()?;
        seed_equipment_if_empty(&db).await?;
        let items = db
            .with(|conn| crate::store::equipment::list(conn, None))
            .await?;
        assert!(!items.is_empty(), "the trimmed seed set still ships");
        assert!(
            items.iter().all(
                |item| leaflog_core::equipment::EquipmentCategory::catalogue()
                    .contains(&item.category)
            ),
            "bundled presets ship for lights, tents and fans only (spec §4.1): {:?}",
            items.iter().map(|item| item.category).collect::<Vec<_>>()
        );
        Ok(())
    }

    #[tokio::test]
    async fn seeded_equipment_has_no_personal_purchase_details() -> Result<(), SeedError> {
        let db = Db::open_in_memory()?;
        seed_equipment_if_empty(&db).await?;
        let items = db
            .with(|conn| crate::store::equipment::list(conn, None))
            .await?;
        assert!(items.iter().all(|item| {
            item.purchase_date.is_none()
                && item.purchase_price.is_none()
                && item.currency.is_none()
                && item.real_watts.is_none()
        }));
        Ok(())
    }

    #[tokio::test]
    async fn seeded_equipment_is_marked_builtin_with_source() -> Result<(), SeedError> {
        let db = Db::open_in_memory()?;
        seed_equipment_if_empty(&db).await?;
        let items = db
            .with(|conn| crate::store::equipment::list(conn, None))
            .await?;
        assert_ne!(
            items.len(),
            0,
            "first boot must seed the equipment catalogue"
        );
        assert!(items.iter().all(|item| {
            item.is_builtin
                && item
                    .source
                    .as_deref()
                    .is_some_and(|source| !source.is_empty())
        }));
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
