//! Quick add picker: the sixteen logging actions behind one trigger,
//! opened as a native modal dialog grouped by grow work (D122) with
//! Lucide SVG icons (ticket 06, spec 95b2e66).

use crate::grows::empty_action;
use crate::i18n::{Locale, t, t_string, use_i18n};
use crate::labels;
use leaflog_core::actions::ActionType;
use leaflog_core::grows::ActionInput;
use leptos::prelude::*;
use leptos_i18n::td_string;
use leptos_icons::Icon;
use wasm_bindgen::JsCast;

/// One named section of the quick-add picker. The taxonomy splits the
/// sixteen actions by grow work — watering/feeding, plant care,
/// environment, observations, milestones — so the tent-first grower
/// taps a labelled 44px target instead of scanning a flat menu (D122).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupKey {
    /// Watering and nutrient feeding.
    Watering,
    /// Hands-on plant work: trim, train, transplant, pest care.
    PlantCare,
    /// The grow environment: lights, climate, lamp settings.
    Environment,
    /// Look-and-record checks: measurement, photo, note, trichomes.
    Observations,
    /// Events that move the diary: flip, harvest, dry/cure check.
    Milestones,
}

impl GroupKey {
    /// The localized group heading.
    fn label(self, locale: Locale) -> String {
        match self {
            Self::Watering => td_string!(locale, quick_add.group_watering).to_owned(),
            Self::PlantCare => td_string!(locale, quick_add.group_plant_care).to_owned(),
            Self::Environment => td_string!(locale, quick_add.group_environment).to_owned(),
            Self::Observations => td_string!(locale, quick_add.group_observations).to_owned(),
            Self::Milestones => td_string!(locale, quick_add.group_milestones).to_owned(),
        }
    }
}

/// The picker's action groups: every [`ActionType`] except
/// `PhaseChange` (which only the forecast/phase UIs create), each
/// kept in the chip order the pre-picker row had (spec §4.2).
fn action_groups() -> Vec<(GroupKey, Vec<ActionType>)> {
    vec![
        (
            GroupKey::Watering,
            vec![ActionType::Water, ActionType::Feed],
        ),
        (
            GroupKey::PlantCare,
            vec![
                ActionType::TrimDefoliate,
                ActionType::Train,
                ActionType::Transplant,
                ActionType::PestIpm,
            ],
        ),
        (
            GroupKey::Environment,
            vec![
                ActionType::Lights,
                ActionType::EnvChange,
                ActionType::LightChange,
            ],
        ),
        (
            GroupKey::Observations,
            vec![
                ActionType::Measurement,
                ActionType::Photo,
                ActionType::Note,
                ActionType::TrichomeCheck,
            ],
        ),
        (
            GroupKey::Milestones,
            vec![
                ActionType::Flip,
                ActionType::Harvest,
                ActionType::DryCureCheck,
            ],
        ),
    ]
}

/// The Lucide icon for a quick-add action. Actions without a fitting
/// glyph in the family render no icon rather than a wrong one
/// (ticket 06); `PhaseChange` never reaches the picker and maps to
/// `None` for the same reason.
fn icon_for(kind: ActionType) -> Option<icondata_core::Icon> {
    let icon = match kind {
        ActionType::Lights => icondata_lu::LuLightbulb,
        ActionType::Water => icondata_lu::LuDroplets,
        ActionType::Feed => icondata_lu::LuFlaskConical,
        ActionType::TrimDefoliate => icondata_lu::LuScissors,
        ActionType::Train => icondata_lu::LuWaypoints,
        ActionType::Transplant => icondata_lu::LuReplace,
        ActionType::Flip => icondata_lu::LuSunMoon,
        ActionType::PestIpm => icondata_lu::LuBug,
        ActionType::EnvChange => icondata_lu::LuFan,
        ActionType::LightChange => icondata_lu::LuSlidersVertical,
        ActionType::Measurement => icondata_lu::LuRuler,
        ActionType::Photo => icondata_lu::LuCamera,
        ActionType::Note => icondata_lu::LuPencil,
        ActionType::TrichomeCheck => icondata_lu::LuZoomIn,
        ActionType::Harvest => icondata_lu::LuWheat,
        ActionType::DryCureCheck => icondata_lu::LuHourglass,
        ActionType::PhaseChange => return None,
    };
    Some(icon)
}

/// localStorage key holding the most recent quick-add picks, most
/// recent first, as the actions' stable `snake_case` keys — the same
/// wire encoding the database and API use (D127).
const RECENTS_KEY: &str = "leaflog.quick-add-recents";

/// How many recent picks the dialog remembers (D127).
const RECENTS_MAX: usize = 3;

/// The updated recents list after picking `picked`: most recent
/// first, deduplicated (a re-pick moves the entry to the front),
/// capped at `max` entries.
#[must_use]
fn update_recents(existing: &[String], picked: &str, max: usize) -> Vec<String> {
    let mut updated = vec![picked.to_owned()];
    updated.extend(
        existing
            .iter()
            .filter(|key| key.as_str() != picked)
            .cloned(),
    );
    updated.truncate(max);
    updated
}

/// Parses a stored wire key back to the picker action it selects.
/// Unknown keys (older or tampered storage) and `PhaseChange` (never
/// offered by the picker) degrade to "not recent".
#[must_use]
fn parse_key(key: &str) -> Option<ActionType> {
    let kind = ActionType::all()
        .into_iter()
        .find(|kind| kind.label() == key)?;
    (kind != ActionType::PhaseChange).then_some(kind)
}

/// Reads the stored recent picks; a missing or locked-down storage
/// area (private mode) degrades to no recents.
fn load_recents() -> Vec<ActionType> {
    let stored = web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(RECENTS_KEY).ok().flatten());
    stored
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
        .map(|keys| keys.iter().filter_map(|key| parse_key(key)).collect())
        .unwrap_or_default()
}

/// Persists the recent picks per browser; a locked-down storage area
/// is ignored (the session keeps its recents in memory).
fn save_recents(keys: &[String]) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
        && let Ok(raw) = serde_json::to_string(keys)
    {
        let _ = storage.set_item(RECENTS_KEY, &raw);
    }
}

/// One labelled group of picker buttons: the recents row (D127) and
/// the taxonomy groups share this rendering, so a recent behaves and
/// looks exactly like its taxonomy chip.
fn action_group(
    locale: Locale,
    heading: String,
    actions: Vec<ActionType>,
    pick: impl Fn(ActionType) + Copy + 'static,
) -> impl IntoView {
    view! {
        <section class="mt-3">
            <h3 class="m-0 mb-2 text-xs font-semibold uppercase tracking-wider text-base-content/60">
                {heading}
            </h3>
            <div class="grid grid-cols-[repeat(auto-fill,minmax(7rem,1fr))] gap-2">
                {actions
                    .into_iter()
                    .map(|kind| {
                        view! {
                            <button
                                type="button"
                                class="btn min-h-11"
                                on:click=move |_| pick(kind)
                            >
                                {icon_for(kind).map(|icon| view! { <Icon icon=icon /> })}
                                {labels::action_label(locale, kind)}
                            </button>
                        }
                    })
                    .collect::<Vec<_>>()}
            </div>
        </section>
    }
}

/// The quick-add picker (ticket 06, D122): a trigger button opening a
/// native modal `<dialog>` — the browser owns the focus trap, the
/// Escape cancel and the focus restore to the trigger, while daisyUI's
/// `modal` chrome draws backdrop and box. Actions sit in labelled
/// groups of 44px buttons, one Lucide SVG each. Picking an action
/// opens the inline logging form card with the same draft semantics
/// the chip row had (D069). The last three picks resurface in a
/// "Recent" group at the top of the dialog (D127).
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub(crate) fn QuickAddMenu(
    chip: RwSignal<Option<ActionType>>,
    draft: RwSignal<ActionInput>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let i18n = use_i18n();
    let dialog = NodeRef::<leptos::html::Dialog>::new();
    let recents = RwSignal::new(Vec::<ActionType>::new());

    // Picking sets the entry's type on a fresh draft and opens the
    // form card — a plain reset left the draft as `note`, so every
    // quick add was logged as one (found by the e2e suite, D069).
    let pick = move |kind: ActionType| {
        let mut reset = empty_action();
        reset.action_type = kind;
        chip.set(Some(kind));
        draft.set(reset);
        // Remember the pick for the recents row (D127): stored most
        // recent first and capped, whatever path led to the pick.
        let keys = recents
            .get_untracked()
            .iter()
            .map(|recent| recent.label().to_owned())
            .collect::<Vec<_>>();
        let updated = update_recents(&keys, kind.label(), RECENTS_MAX);
        save_recents(&updated);
        recents.set(updated.iter().filter_map(|key| parse_key(key)).collect());
        // Natively close the dialog before reopening the form card;
        // the on:close handler keeps aria-expanded honest.
        if let Some(el) = dialog.get_untracked() {
            el.unchecked_into::<web_sys::HtmlDialogElement>().close();
        }
        open.set(false);
        // The form card just appeared below; hand focus to it so the
        // click that started the flow continues in the form.
        request_animation_frame(move || {
            if let Some(card) = document().get_element_by_id("quick-add-form")
                && let Some(el) = card.dyn_ref::<web_sys::HtmlElement>()
            {
                let _ = el.focus();
            }
        });
    };

    let on_trigger_click = move |_| {
        let Some(el) = dialog.get_untracked() else {
            return;
        };
        let el = el.unchecked_into::<web_sys::HtmlDialogElement>();
        if el.open() {
            return;
        }
        // Read the stored recents once per open (D127), not
        // reactively on every keystroke.
        recents.set(load_recents());
        // showModal puts the dialog in the top layer: the browser
        // then traps Tab, closes on Escape and restores focus to the
        // trigger on its own — no manual key handling needed.
        let _ = el.show_modal();
        open.set(true);
        // Focus the first action on the next frame, after the top
        // layer has rendered (focusing a hidden element is a no-op).
        request_animation_frame(move || {
            if let Some(first) = el.query_selector("button").ok().flatten()
                && let Some(btn) = first.dyn_ref::<web_sys::HtmlElement>()
            {
                let _ = btn.focus();
            }
        });
    };

    view! {
        <button
            type="button"
            class="btn btn-primary btn-sm"
            aria-haspopup="dialog"
            aria-expanded=move || if open.get() { "true" } else { "false" }
            aria-controls="quick-add-dialog"
            on:click=on_trigger_click
        >
            <Icon icon=icondata_lu::LuCirclePlus />
            {t!(i18n, quick_add.trigger)}
        </button>
        <dialog
            node_ref=dialog
            id="quick-add-dialog"
            class="modal"
            aria-modal="true"
            aria-labelledby="quick-add-dialog-title"
            // Escape (cancel) and the backdrop form close natively;
            // this event keeps aria-expanded honest on every path.
            on:close=move |_| open.set(false)
        >
            <div class="modal-box">
                <h2 id="quick-add-dialog-title" class="m-0 text-lg font-semibold">
                    {t!(i18n, quick_add.dialog_title)}
                </h2>
                {move || {
                    let locale = i18n.get_locale();
                    let groups = action_groups()
                        .into_iter()
                        .map(|(key, actions)| action_group(locale, key.label(locale), actions, pick))
                        .collect::<Vec<_>>();
                    // The recents group leads only when there is
                    // something remembered (D127).
                    let recent = recents.get();
                    let recents_group = (!recent.is_empty()).then(|| {
                        action_group(
                            locale,
                            td_string!(locale, quick_add.recents_heading).to_owned(),
                            recent,
                            pick,
                        )
                    });
                    view! {
                        <>
                            {recents_group}
                            {groups}
                        </>
                    }
                }}
            </div>
            // Clicking the backdrop submits this method=dialog form,
            // which closes the dialog natively (HTML spec behaviour).
            <form method="dialog" class="modal-backdrop">
                <button aria-label=move || t_string!(i18n, quick_add.close).into_owned()></button>
            </form>
        </dialog>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_recents_storage_key_is_namespaced_to_leaflog() {
        assert_eq!(RECENTS_KEY, "leaflog.quick-add-recents");
    }

    #[test]
    fn empty_history_gains_the_pick() {
        assert_eq!(update_recents(&[], "water", RECENTS_MAX), vec!["water"]);
    }

    #[test]
    fn re_picking_moves_the_entry_to_the_front() {
        let existing = vec!["water".to_owned(), "feed".to_owned(), "note".to_owned()];
        assert_eq!(
            update_recents(&existing, "feed", RECENTS_MAX),
            vec!["feed", "water", "note"]
        );
    }

    #[test]
    fn capping_evicts_the_oldest_entry() {
        let existing = vec!["water".to_owned(), "feed".to_owned(), "note".to_owned()];
        assert_eq!(
            update_recents(&existing, "trim_defoliate", 3),
            vec!["trim_defoliate", "water", "feed"]
        );
    }

    #[test]
    fn unrelated_entries_keep_their_order_after_the_pick() {
        let existing = vec!["water".to_owned(), "note".to_owned()];
        assert_eq!(
            update_recents(&existing, "photo", RECENTS_MAX),
            vec!["photo", "water", "note"]
        );
    }

    #[test]
    fn max_zero_remembers_nothing() {
        let existing = vec!["water".to_owned()];
        assert_eq!(update_recents(&existing, "feed", 0), Vec::<String>::new());
    }

    #[test]
    fn every_picker_action_has_a_distinct_icon() {
        let mut seen = Vec::new();
        let grouped = action_groups().into_iter().flat_map(|(_, actions)| actions);
        for kind in grouped {
            let icon = icon_for(kind)
                .unwrap_or_else(|| panic!("picker action without icon: {}", kind.label()));
            assert!(!seen.contains(&icon), "duplicate icon for {}", kind.label());
            seen.push(icon);
        }
        assert_eq!(seen.len(), 16);
    }

    #[test]
    fn unmapped_actions_render_no_icon_rather_than_a_wrong_one() {
        assert_eq!(icon_for(ActionType::PhaseChange), None);
    }

    #[test]
    fn action_groups_carry_every_action_exactly_once_without_phase_change() {
        let grouped: Vec<ActionType> = action_groups()
            .into_iter()
            .flat_map(|(_, actions)| actions)
            .collect();
        for kind in ActionType::all() {
            let count = grouped.iter().filter(|g| **g == kind).count();
            let expected = usize::from(kind != ActionType::PhaseChange);
            assert_eq!(
                count,
                expected,
                "action {} appears {} times",
                kind.label(),
                count
            );
        }
    }

    #[test]
    fn groups_preserve_the_chip_order_within_each_group() {
        let chip_order: Vec<ActionType> = ActionType::all()
            .into_iter()
            .filter(|kind| *kind != ActionType::PhaseChange)
            .collect();
        for (key, actions) in action_groups() {
            let expected: Vec<ActionType> = chip_order
                .iter()
                .copied()
                .filter(|kind| actions.contains(kind))
                .collect();
            assert_eq!(actions, expected, "group {key:?} scrambles chip order");
        }
    }

    #[test]
    fn the_picker_groups_sixteen_actions_in_total() {
        let total: usize = action_groups()
            .into_iter()
            .map(|(_, actions)| actions.len())
            .sum();
        assert_eq!(total, 16);
    }

    #[test]
    fn every_group_has_a_localized_label() {
        for (key, _) in action_groups() {
            assert!(!key.label(Locale::en).is_empty(), "no en label for {key:?}");
            assert!(!key.label(Locale::de).is_empty(), "no de label for {key:?}");
        }
    }
}
