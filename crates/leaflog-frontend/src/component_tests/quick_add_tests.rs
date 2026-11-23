//! Quick-add picker states (D139): closed by default, full taxonomy
//! when opened, pick semantics, and the recents row. Pure client
//! state — no fetch traffic to stub; localStorage is reset per test.

use super::support::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::wasm_bindgen_test;

use leptos::prelude::*;
use leptos::view;

/// The chip the picker is opened with (`None` = the full taxonomy).
type Chip = RwSignal<Option<leaflog_core::actions::ActionType>>;
/// The log entry being drafted.
type Draft = RwSignal<leaflog_core::grows::ActionInput>;
/// The phase picked from the "which phase" prompt.
type PhasePick = RwSignal<Option<leaflog_core::Phase>>;

/// The picker plus fresh chip/draft/phase-pick signals, the way the
/// grow page mounts it.
fn picker() -> (Mounted<impl IntoView>, Chip, Draft, PhasePick) {
    let chip = RwSignal::new(None);
    let draft = RwSignal::new(crate::grows::empty_action("UTC"));
    let pick_phase = RwSignal::new(None);
    let chip2 = chip;
    let draft2 = draft;
    let pick_phase2 = pick_phase;
    let pick_date = RwSignal::new(String::new());
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <crate::quick_add::QuickAddMenu chip=chip2 draft=draft2 pick_phase=pick_phase2 pick_date selected_plants=RwSignal::new(Vec::new()) timezone=RwSignal::new("UTC".to_owned()).into() />
            }
        })
    });
    (mount, chip, draft, pick_phase)
}

fn dialog(mount: &Mounted<impl IntoView + 'static>) -> web_sys::HtmlDialogElement {
    mount
        .q("#quick-add-dialog")
        .expect("dialog in the DOM")
        .unchecked_into::<web_sys::HtmlDialogElement>()
}

/// The picker button whose text is exactly `label`.
fn button_with_label(
    mount: &Mounted<impl IntoView + 'static>,
    label: &str,
) -> web_sys::HtmlElement {
    mount
        .q_all("button")
        .iter()
        .find(|b| b.text_content().is_some_and(|t| t.trim().ends_with(label)))
        .cloned()
        .unwrap_or_else(|| panic!("no {label} button"))
        .unchecked_into::<web_sys::HtmlElement>()
}

#[wasm_bindgen_test(async)]
async fn quick_add_starts_closed_without_data() {
    fresh_environment();
    let (mount, _chip, _draft, _pick_phase) = picker();
    let trigger = mount.q_all("button").first().cloned().unwrap();
    assert_eq!(
        trigger.get_attribute("aria-expanded").as_deref(),
        Some("false"),
        "closed by default"
    );
    assert!(!dialog(&mount).open(), "dialog not open without a click");
}

#[wasm_bindgen_test(async)]
async fn quick_add_opens_with_the_full_taxonomy() {
    fresh_environment();
    let (mount, _chip, _draft, _pick_phase) = picker();
    let trigger = mount.q_all("button").first().cloned().unwrap();
    click(&trigger);
    assert!(dialog(&mount).open(), "dialog opened by the trigger");
    assert!(
        try_wait_for(&mut || { trigger.get_attribute("aria-expanded").as_deref() == Some("true") })
            .await,
        "aria-expanded tracks the dialog"
    );
    for heading in [
        "Watering & feeding",
        "Plant care",
        "Environment",
        "Observations",
        "Milestones",
    ] {
        assert!(mount.text().contains(heading), "group {heading} present");
    }
    // Every pickable action renders (17 buttons with action labels
    // inside the dialog; D148 added Set phase).
    let dialog_el = dialog(&mount);
    let labels = [
        "Water",
        "Feed",
        "Trim",
        "Train",
        "Transplant",
        "Pest/IPM",
        "Lights",
        "Env change",
        "Light change",
        "Measure",
        "Photo",
        "Note",
        "Trichomes",
        "Flip 12/12",
        "Harvest",
        "Dry/cure",
        "Set phase",
    ];
    let text = dialog_el.text_content().unwrap_or_default();
    for label in labels {
        assert!(text.contains(label), "action {label} present");
    }
    assert!(
        !text.contains("Phase change"),
        "raw PhaseChange label stays out"
    );
}

#[wasm_bindgen_test(async)]
async fn quick_add_pick_sets_chip_and_draft_then_closes() {
    fresh_environment();
    let (mount, chip, draft, _pick_phase) = picker();
    let trigger = mount.q_all("button").first().cloned().unwrap();
    click(&trigger);
    let water = button_with_label(&mount, "Water");
    click(&water);
    assert_eq!(
        chip.get(),
        Some(leaflog_core::actions::ActionType::Water),
        "chip set by the pick"
    );
    assert_eq!(
        draft.get().action_type,
        leaflog_core::actions::ActionType::Water,
        "draft carries the picked kind"
    );
    assert!(!dialog(&mount).open(), "dialog closed after the pick");
}

#[wasm_bindgen_test(async)]
async fn quick_add_recents_resurface_and_cap_at_three() {
    fresh_environment();
    let (mount, _chip, _draft, _pick_phase) = picker();
    let trigger = mount.q_all("button").first().cloned().unwrap();
    for label in ["Water", "Feed", "Train", "Note"] {
        click(&trigger);
        let pick = button_with_label(&mount, label);
        click(&pick);
    }
    // Four picks, three remembered, most recent first.
    let stored = local_storage("leaflog.quick-add-recents");
    let keys: Vec<&str> = stored
        .trim_matches(['[', ']'])
        .split(',')
        .map(|k| k.trim_matches('"'))
        .collect();
    assert_eq!(keys.len(), 3, "recents cap at three");
    // Most recent first; the earliest pick (water) is evicted. The
    // stored keys are the actions' stable snake_case identifiers.
    assert_eq!(keys, ["note", "train", "feed"], "stored: {stored:?}");

    // Reopening resurfaces them under the Recent heading (the group
    // renders from the recents signal — wait out the microtask).
    click(&trigger);
    let shown = try_wait_for(&mut || {
        dialog(&mount)
            .text_content()
            .unwrap_or_default()
            .contains("Recent")
    })
    .await;
    assert!(shown, "recents group leads the dialog");
}
