//! Substrate and feed-preset seeding (spec §5.3).
//!
//! The substrate profiles and the generic feed templates are seeded
//! into empty tables on boot (first boot or a wiped database) — it
//! never overwrites user data. The bundled data of the removed
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

/// Seeds the substrate profiles and the generic feed templates (only
/// into empty tables; part of first boot, D079: previously a side
/// effect of strain seeding).
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
    use super::{SeedError, seed_feed_presets};
    use crate::db::Db;

    #[tokio::test]
    async fn feed_presets_seed_into_empty_tables_only() -> Result<(), SeedError> {
        let db = Db::open_in_memory()?;
        seed_feed_presets(&db).await?;
        let profiles: i64 = db
            .with(|conn| {
                conn.query_row("SELECT COUNT(*) FROM substrate_profile", [], |r| r.get(0))
                    .map_err(crate::db::StoreError::from)
            })
            .await?;
        assert!(profiles >= 3, "soil, coco and hydro profiles seeded");
        let templates: i64 = db
            .with(|conn| {
                conn.query_row("SELECT COUNT(*) FROM feed_template", [], |r| r.get(0))
                    .map_err(crate::db::StoreError::from)
            })
            .await?;
        assert!(templates >= 3, "one generic template per substrate seeded");
        // A second pass is a no-op (only empty tables are filled).
        seed_feed_presets(&db).await?;
        let templates_after: i64 = db
            .with(|conn| {
                conn.query_row("SELECT COUNT(*) FROM feed_template", [], |r| r.get(0))
                    .map_err(crate::db::StoreError::from)
            })
            .await?;
        assert_eq!(templates_after, templates, "no template duplication");
        Ok(())
    }

    // Provenance note: the presets are generated from the §5.3
    // substrate science presets, not third-party records — their
    // `source` column names the derivation, asserted here once.
    #[tokio::test]
    async fn seeded_templates_carry_their_source() -> Result<(), SeedError> {
        let db = Db::open_in_memory()?;
        seed_feed_presets(&db).await?;
        let rows: Vec<(Option<String>, i64)> = db
            .with(|conn| -> Result<Vec<(Option<String>, i64)>, crate::db::StoreError> {
                let mut stmt = conn.prepare(
                    "SELECT source, is_builtin FROM feed_template ORDER BY name",
                )?;
                let rows = stmt
                    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await?;
        assert!(!rows.is_empty(), "templates seeded");
        assert!(
            rows.iter()
                .all(|(source, builtin)| *builtin == 1
                    && source
                        .as_deref()
                        .is_some_and(|source| source.contains("§5.3"))),
            "the generated presets state where they come from"
        );
        Ok(())
    }
}
