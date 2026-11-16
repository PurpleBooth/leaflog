//! The phase picker's pure logic (D073): what recording a pick means,
//! and the spelled-out phase labels for the interface.

use leaflog_core::Phase;
use leaflog_core::actions::{ActionDetails, ActionType};
use leaflog_core::grows::ActionInput;
use leaflog_core::strain::PhotoperiodType;

/// The interface label for a phase: spelled out, "Flowering" not the
/// terse code label.
#[must_use]
pub const fn phase_display(phase: Phase) -> &'static str {
    match phase {
        Phase::Germination => "Germination",
        Phase::Seedling => "Seedling",
        Phase::Vegetative => "Vegetative",
        Phase::Flower => "Flowering",
        Phase::Drying => "Drying",
        Phase::Curing => "Curing",
    }
}

/// The phase for a code label ("vegetative"), as the API serializes
/// it.
#[must_use]
pub fn phase_from_label(label: &str) -> Option<Phase> {
    Phase::all()
        .into_iter()
        .find(|phase| phase.label() == label)
}

/// All six phases in picker order with their interface labels.
#[must_use]
pub fn picker_phases() -> Vec<(Phase, &'static str)> {
    Phase::all()
        .into_iter()
        .map(|phase| (phase, phase_display(phase)))
        .collect()
}

/// The event a pick records (D073): Flowering on a photoperiod is the
/// flip; Drying is the harvest; every other boundary — and an
/// autoflower's Flowering — is the generic Phase-change action with
/// the target phase.
#[must_use]
pub const fn event_for_pick(
    phase: Phase,
    photoperiod: PhotoperiodType,
) -> (ActionType, Option<Phase>) {
    match (phase, photoperiod) {
        (Phase::Flower, PhotoperiodType::Photoperiod) => (ActionType::Flip, None),
        (Phase::Drying, _) => (ActionType::Harvest, None),
        (other, _) => (ActionType::PhaseChange, Some(other)),
    }
}

/// Composes the log entry for a pick. `occurred_at` is a full RFC 3339
/// instant — today by default, backdatable.
#[must_use]
pub fn pick_input(phase: Phase, photoperiod: PhotoperiodType, occurred_at: String) -> ActionInput {
    let (action_type, target_phase) = event_for_pick(phase, photoperiod);
    ActionInput {
        plant_ids: Vec::new(),
        occurred_at,
        action_type,
        details: ActionDetails {
            target_phase,
            ..ActionDetails::default()
        },
        notes: None,
        appearance_tags: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{event_for_pick, phase_display, phase_from_label, pick_input, picker_phases};
    use leaflog_core::Phase;
    use leaflog_core::actions::ActionType;
    use leaflog_core::strain::PhotoperiodType;

    #[test]
    fn labels_are_spelled_out() {
        assert_eq!(phase_display(Phase::Flower), "Flowering");
        assert_eq!(phase_display(Phase::Vegetative), "Vegetative");
        assert_eq!(phase_display(Phase::Germination), "Germination");
        assert_eq!(picker_phases().len(), 6);
    }

    #[test]
    fn code_labels_round_trip() {
        assert_eq!(phase_from_label("vegetative"), Some(Phase::Vegetative));
        assert_eq!(phase_from_label("flower"), Some(Phase::Flower));
        assert_eq!(phase_from_label("nonsense"), None);
    }

    #[test]
    fn photoperiod_flowering_is_a_flip() {
        let (action, target) = event_for_pick(Phase::Flower, PhotoperiodType::Photoperiod);
        assert_eq!(action, ActionType::Flip);
        assert_eq!(target, None);
    }

    #[test]
    fn autoflower_flowering_is_phase_change() {
        let (action, target) = event_for_pick(Phase::Flower, PhotoperiodType::Auto);
        assert_eq!(action, ActionType::PhaseChange);
        assert_eq!(target, Some(Phase::Flower));
    }

    #[test]
    fn drying_is_a_harvest_and_others_are_phase_changes() {
        let (action, target) = event_for_pick(Phase::Drying, PhotoperiodType::Photoperiod);
        assert_eq!(action, ActionType::Harvest);
        assert_eq!(target, None);
        let (action, target) = event_for_pick(Phase::Curing, PhotoperiodType::Auto);
        assert_eq!(action, ActionType::PhaseChange);
        assert_eq!(target, Some(Phase::Curing));
    }

    #[test]
    fn pick_input_carries_the_target_and_date() {
        let input = pick_input(
            Phase::Seedling,
            PhotoperiodType::Photoperiod,
            "2026-11-16T12:00:00Z".to_owned(),
        );
        assert_eq!(input.action_type, ActionType::PhaseChange);
        assert_eq!(input.details.target_phase, Some(Phase::Seedling));
        assert_eq!(input.occurred_at, "2026-11-16T12:00:00Z");
    }
}
