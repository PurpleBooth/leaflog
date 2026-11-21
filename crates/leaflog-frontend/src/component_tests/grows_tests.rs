// Component tests for the grows surfaces (D139): the overview page's
// empty/list/error states, a timeline card with and without facts,
// and GrowForm's schedule select with and without schedules.
use super::support::*;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

use leaflog_core::actions::ActionDetails;
use leaflog_core::actions::ActionType;
use leaflog_core::grows::PhotoperiodType;
use leaflog_core::grows::{ActionEntry, GrowStatus};

fn grow_json(id: &str, name: &str) -> String {
    format!(
        r#"{{"id":"{id}","name":"{name}","substrate":"soil","started_on":"2026-04-01","status":"active","notes":null,"created_at":"2026-04-01T00:00:00Z","updated_at":"2026-04-01T00:00:00Z"}}"#
    )
}

fn sample_entry(id: &str, grow_day: u16) -> ActionEntry {
    ActionEntry {
        id: id.to_string(),
        grow_id: "g1".to_string(),
        plant_ids: Vec::new(),
        occurred_at: "2026-04-12T10:00:00Z".to_string(),
        action_type: ActionType::Water,
        phase: Some(leaflog_core::Phase::Vegetative),
        grow_day,
        details: ActionDetails {
            volume_ml: Some(500.0),
            input_ph: Some(6.3),
            input_ec_us: Some(600),
            ..ActionDetails::default()
        },
        notes: Some("Looking good".to_string()),
        appearance_tags: vec!["healthy".to_string()],
        created_at: "2026-04-12T10:00:00Z".to_string(),
        updated_at: "2026-04-12T10:00:00Z".to_string(),
    }
}

#[wasm_bindgen_test]
async fn grows_page_empty_state_without_data() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.ends_with("/api/grows") {
            return (200, "[]".to_string());
        }
        (404, String::new())
    });
    let mount = Mounted::new(move || {
        with_i18n(move || {
            with_router(move || {
                view! { <crate::grows::GrowsPage /> }
            })
        })
    });
    wait_text(&mount, "No grows yet").await;
    assert!(mount.has("p.py-6.text-center"), "empty state paragraph");
}

#[wasm_bindgen_test]
async fn grows_page_lists_grows_with_data() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.ends_with("/api/grows") {
            return (
                200,
                format!(
                    "[{},{}]",
                    grow_json("g1", "Balcony run"),
                    grow_json("g2", "Tent run")
                ),
            );
        }
        (404, String::new())
    });
    let mount = Mounted::new(move || {
        with_i18n(move || {
            with_router(move || {
                view! { <crate::grows::GrowsPage /> }
            })
        })
    });
    wait_text(&mount, "Balcony run").await;
    wait_text(&mount, "Tent run").await;
    let text = mount.text();
    assert!(text.contains("Active"), "status badges: {text:?}");
    assert!(
        text.contains("Started 2026-04-01"),
        "started line: {text:?}"
    );
    assert!(mount.has("ul > li.card"), "grow cards in a list");
    assert!(mount.text().contains("Soil"), "substrate label");
}

#[wasm_bindgen_test]
async fn grows_page_error_banner_and_retry() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.ends_with("/api/grows") {
            return (500, "boom".to_string());
        }
        (404, String::new())
    });
    let mount = Mounted::new(move || {
        with_i18n(move || {
            with_router(move || {
                view! { <crate::grows::GrowsPage /> }
            })
        })
    });
    wait_text(&mount, "status 500").await;
    // The retry button is present (clicking it reloads the page, so
    // the click path belongs to e2e, not a component test).
    assert!(
        mount.q("button[aria-label='Retry']").is_some(),
        "retry button rendered"
    );
    // Dismiss clears the banner without touching the list state.
    click(&mount.q("button[aria-label='Dismiss']").expect("dismiss"));
    let gone = try_wait_for(&mut || !mount.text().contains("status 500")).await;
    assert!(gone, "dismiss hides the banner");
}

#[wasm_bindgen_test]
async fn timeline_entry_renders_facts_and_notes() {
    fresh_environment();
    let deletions = RwSignal::new(Vec::<String>::new());
    let entry = sample_entry("e1", 12);
    let mount = Mounted::new(move || {
        view! {
            <ul>{crate::grows::timeline_entry(
                crate::i18n::Locale::en,
                entry.clone(),
                move |removed| deletions.update(|list| list.push(removed.id)),
            )}</ul>
        }
    });
    let text = mount.text();
    assert!(text.contains("Water"), "action label: {text:?}");
    // The day badge is deliberately lowercase ("day 12", e2e 17 pins
    // the phase-day split wording).
    assert!(text.contains("day 12"), "day badge: {text:?}");
    assert!(text.contains("Vegetative"), "phase badge: {text:?}");
    assert!(mount.has("dl.facts"), "facts list present");
    let facts = mount.q("dl.facts").expect("facts list").text_content();
    assert!(
        facts.as_deref().is_some_and(|f| f.contains("500")),
        "volume fact: {facts:?}"
    );
    assert!(text.contains("Looking good"), "notes: {text:?}");
    assert!(text.contains("Healthy"), "appearance tag: {text:?}");
    // The delete button replays the whole entry through the callback.
    let button = mount
        .q_all("button")
        .into_iter()
        .find(|b| b.text_content().as_deref() == Some("×"))
        .expect("delete button");
    click(&button);
    assert_eq!(
        deletions.get_untracked(),
        vec!["e1".to_string()],
        "delete callback carries the entry id"
    );
}

#[wasm_bindgen_test]
async fn timeline_entry_minimal_without_data() {
    fresh_environment();
    let deletions = RwSignal::new(Vec::<String>::new());
    let mut entry = sample_entry("e2", 3);
    entry.details = ActionDetails::default();
    entry.notes = None;
    entry.appearance_tags = Vec::new();
    let mount = Mounted::new(move || {
        view! {
            <ul>{crate::grows::timeline_entry(
                crate::i18n::Locale::en,
                entry.clone(),
                move |removed| deletions.update(|list| list.push(removed.id)),
            )}</ul>
        }
    });
    assert!(
        !mount.has("dl.facts"),
        "no facts list when details are empty"
    );
    let text = mount.text();
    assert!(!text.contains("Looking good"), "no notes paragraph");
    assert!(text.contains("day 3"), "day badge: {text:?}");
    // Only the local stamp remains besides header and label.
    assert!(text.contains("2026"), "stamp line: {text:?}");
}

fn form() -> (
    Mounted<impl IntoView + 'static>,
    RwSignal<leaflog_core::grows::GrowInput>,
) {
    let draft = RwSignal::new(crate::grows::empty_grow());
    let mount = Mounted::new(move || {
        let draft = draft;
        with_i18n(move || {
            view! { <crate::grows::GrowForm draft /> }
        })
    });
    (mount, draft)
}

#[wasm_bindgen_test]
async fn grow_form_without_schedules_offers_none() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.ends_with("/api/schedules") {
            return (200, "[]".to_string());
        }
        (404, String::new())
    });
    let (mount, draft) = form();
    wait_text(&mount, "Planned vegetative weeks").await;
    let text = mount.text();
    assert!(
        text.contains("No feeding schedule"),
        "none option: {text:?}"
    );
    assert!(
        text.contains("Advanced settings"),
        "advanced summary present"
    );
    // Two-way binding: typing feeds the draft.
    let name = mount
        .q("input[type='text']")
        .expect("name input")
        .unchecked_into::<web_sys::HtmlInputElement>();
    type_input(&name, "Spring grow");
    assert_eq!(
        draft.get_untracked().name,
        "Spring grow",
        "name binding writes the draft"
    );
    // The photoperiod select defaults to photoperiodic.
    assert_eq!(
        draft.get_untracked().photoperiod,
        PhotoperiodType::Photoperiod,
        "default photoperiod"
    );
}

#[wasm_bindgen_test]
async fn grow_form_lists_schedules_when_available() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.ends_with("/api/schedules") {
            return (
                200,
                r#"[{"schedule":{"id":"s1","name":"BioBizz","flower_weeks":2,"products":[]},"product_count":2,"groups":["Base"],"units":["ml/L"],"used_by":1}]"#
                    .to_string(),
            );
        }
        (404, String::new())
    });
    let (mount, draft) = form();
    wait_text(&mount, "Planned vegetative weeks").await;
    // The schedule select gains the stubbed option (identify it by
    // its none-option, not by length: photoperiod also has two).
    let select = mount
        .q_all("select.select")
        .into_iter()
        .map(wasm_bindgen::JsCast::unchecked_into::<web_sys::HtmlSelectElement>)
        .find(|s| {
            s.text_content()
                .unwrap_or_default()
                .contains("No feeding schedule")
        })
        .expect("schedule select with options");
    let shown = try_wait_for(&mut || {
        select
            .text_content()
            .unwrap_or_default()
            .contains("BioBizz")
    })
    .await;
    assert!(
        shown,
        "schedule option appears: {:?}",
        select.text_content()
    );
    select.set_value("s1");
    fire_change(&select);
    assert_eq!(
        draft.get_untracked().feed_schedule_id,
        Some("s1".to_string()),
        "picking a schedule writes the draft"
    );
}

// The status select keeps its two options in sync with the draft.
#[wasm_bindgen_test]
async fn grow_form_status_select_updates_draft() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.ends_with("/api/schedules") {
            return (200, "[]".to_string());
        }
        (404, String::new())
    });
    let (mount, draft) = form();
    wait_text(&mount, "Planned vegetative weeks").await;
    let select = mount
        .q_all("select.select")
        .into_iter()
        .map(wasm_bindgen::JsCast::unchecked_into::<web_sys::HtmlSelectElement>)
        .find(|s| s.text_content().unwrap_or_default().contains("Archived"))
        .expect("status select");
    select.set_value(GrowStatus::Archived.label());
    fire_change(&select);
    assert_eq!(
        draft.get_untracked().status,
        GrowStatus::Archived,
        "archived selection lands in the draft"
    );
}
