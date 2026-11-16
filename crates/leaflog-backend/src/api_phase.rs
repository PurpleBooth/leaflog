//! Phase resolution shared by the API (D073).
//!
//! Builds the forecast actuals from the action log — the canonical
//! flip/harvest events first, the latest Phase-change pin per
//! boundary filling the rest — and derives the phase a grow is in
//! today from the re-anchored forecast.

use leaflog_core::Phase;
use leaflog_core::actions::ActionType;
use leaflog_core::forecast::{self, Actuals, PhaseWindow};

use crate::db::StoreError;
use crate::store;

/// The forecast actuals for a grow (D073). Canonical events win their
/// boundary: a logged flip or harvest always pins it. Phase-change
/// pins fill the remaining boundaries; the latest pin per boundary
/// wins (the store resolves that).
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub(crate) fn forecast_actuals(
    conn: &rusqlite::Connection,
    grow_id: &str,
) -> Result<Actuals, StoreError> {
    let flip = store::actions::first_event_day(conn, grow_id, ActionType::Flip, u16::MAX)?;
    let harvest = store::actions::first_event_day(conn, grow_id, ActionType::Harvest, u16::MAX)?;
    let trichome =
        store::actions::first_event_day(conn, grow_id, ActionType::TrichomeCheck, u16::MAX)?;
    let pin_day = |phase: Phase| -> Result<Option<i64>, StoreError> {
        Ok(store::actions::latest_phase_change_day(conn, grow_id, phase.label())?.map(i64::from))
    };
    let flip_day_i = flip.map(i64::from);
    let drying_pin = pin_day(Phase::Drying)?;

    Ok(Actuals {
        flip_day: flip_day_i,
        // A Harvest ends flower; a Phase-change into drying fills in
        // when no Harvest was logged.
        harvest_day: harvest.map(i64::from).or(drying_pin),
        trichome_check_day: trichome.map(i64::from),
        germinated: false,
        germination_start: pin_day(Phase::Germination)?,
        seedling_start: pin_day(Phase::Seedling)?,
        vegetative_start: pin_day(Phase::Vegetative)?,
        // The flower boundary: the latest Flowering pin, or the flip
        // when no pick exists (an auto grower may log the Flip chip).
        flower_start: pin_day(Phase::Flower)?.or(flip_day_i),
        curing_start: pin_day(Phase::Curing)?,
    })
}

/// The phase the grow is in on `today`, from the re-anchored forecast.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub(crate) fn current_phase(
    conn: &rusqlite::Connection,
    grow: &leaflog_core::grows::Grow,
    today: u16,
) -> Result<Phase, StoreError> {
    let actuals = forecast_actuals(conn, &grow.id)?;
    let input = forecast::input_for(grow.photoperiod, grow.planned_veg_weeks, actuals);
    let windows = forecast::forecast(&input);
    Ok(phase_at(&windows, i64::from(today)))
}

/// The phase whose window contains `day`: the last window starting at
/// or before it wins, Germination as the fallback.
#[must_use]
pub(crate) fn phase_at(windows: &[PhaseWindow], day: i64) -> Phase {
    windows
        .iter()
        .rev()
        .find(|window| window.likely_start <= day)
        .map_or(Phase::Germination, |window| window.phase)
}

#[cfg(test)]
mod tests {
    use super::phase_at;
    use leaflog_core::Phase;
    use leaflog_core::forecast::{Actuals, ForecastInput, PhaseDefaults, forecast};
    use leaflog_core::grows::PhotoperiodType;

    #[test]
    fn phase_at_walks_the_windows() {
        let input = ForecastInput {
            photoperiod: PhotoperiodType::Photoperiod,
            planned_veg_weeks: Some(4),
            auto_total_days: None,
            strain_flower_days: None,
            defaults: PhaseDefaults::default(),
            actuals: Actuals::default(),
        };
        let windows = forecast(&input);
        // Germ 3–10 (likely 6): the boundary day belongs to the next
        // phase.
        assert_eq!(phase_at(&windows, 0), Phase::Germination);
        assert_eq!(phase_at(&windows, 5), Phase::Germination);
        assert_eq!(phase_at(&windows, 6), Phase::Seedling);
        assert_eq!(phase_at(&windows, 22), Phase::Seedling);
        assert_eq!(phase_at(&windows, 23), Phase::Vegetative);
        assert_eq!(phase_at(&windows, 51), Phase::Flower);
        assert_eq!(phase_at(&windows, 500), Phase::Curing);
    }
}
