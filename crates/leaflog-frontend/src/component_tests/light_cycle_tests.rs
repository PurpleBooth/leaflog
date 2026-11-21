//! `light_cycle.rs`: the `LightCycleEditor` form — stored override,
//! placeholders, save/reset round-trips and the undoable reset.

use super::support::*;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

const EMPTY_CYCLE: &str = r"{}";
const STORED_CYCLE: &str = r#"{"lights_on": "07:30", "hours_on": {"germination": null, "seedling": null, "vegetative": 20, "flower": 24}}"#;
const WITH_REALITY: &str = r#"{
    "lights_on": "06:00",
    "hours_on": {"germination": null, "seedling": null, "vegetative": null, "flower": null},
    "observed": {
        "entity_id": "light.tent",
        "lights_on": "06:05",
        "hours_on": 12
    }
}"#;

fn editor() -> Mounted<impl IntoView> {
    Mounted::new(move || {
        with_i18n(move || {
            view! {
                <crate::light_cycle::LightCycleEditor
                    grow_id=leptos::prelude::Signal::from("g1".to_string())
                    photoperiod=leptos::prelude::Signal::from(
                        leaflog_core::grows::PhotoperiodType::Photoperiod,
                    )
                    bindings_epoch=leptos::prelude::Signal::from(0u64)
                />
            }
        })
    })
}

fn lights_on_input<V: IntoView + 'static>(mount: &Mounted<V>) -> web_sys::HtmlInputElement {
    mount
        .q("input[type='time']")
        .expect("lights-on time input")
        .unchecked_into()
}

fn apply_button<V: IntoView + 'static>(mount: &Mounted<V>) -> web_sys::HtmlButtonElement {
    mount
        .q_all("button[type='submit']")
        .first()
        .expect("apply button")
        .clone()
        .unchecked_into()
}

#[wasm_bindgen_test(async)]
async fn light_cycle_editor_starts_blank_with_placeholders() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| match (method, url) {
        ("GET", u) if u.ends_with("/light-cycle") => (200, EMPTY_CYCLE.to_string()),
        _ => (404, String::new()),
    });
    let mount = editor();
    wait_for(|| {
        lights_on_input(&mount).value() == "" && lights_on_input(&mount).placeholder() == "06:00"
    })
    .await;
    // No stored override: no reset button, no reality caption.
    assert!(
        mount
            .q_all("button")
            .iter()
            .all(|b| { b.text_content().unwrap_or_default() != "Reset to default" }),
        "reset hidden without an override"
    );
    assert!(mount.q("[data-testid='light-reality']").is_none());
}

#[wasm_bindgen_test(async)]
async fn light_cycle_editor_loads_a_stored_override_with_reset() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| match (method, url) {
        ("GET", u) if u.ends_with("/light-cycle") => (200, STORED_CYCLE.to_string()),
        _ => (404, String::new()),
    });
    let mount = editor();
    wait_for(|| lights_on_input(&mount).value() == "07:30").await;
    let numbers: Vec<String> = mount
        .q_all("input[type='number']")
        .iter()
        .map(|el| {
            el.clone()
                .unchecked_into::<web_sys::HtmlInputElement>()
                .value()
        })
        .collect();
    assert_eq!(numbers, ["", "", "20", "24"], "stored hours per lit phase");
    assert!(
        mount
            .q_all("button")
            .iter()
            .any(|b| b.text_content().as_deref() == Some("Reset to default")),
        "reset visible with an override"
    );
}

#[wasm_bindgen_test(async)]
async fn light_cycle_editor_shows_the_observed_reality_caption() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| match (method, url) {
        ("GET", u) if u.ends_with("/light-cycle") => (200, WITH_REALITY.to_string()),
        _ => (404, String::new()),
    });
    let mount = editor();
    let seen = try_wait_for(&mut || mount.q("[data-testid='light-reality']").is_some()).await;
    if !seen {
        let t = mount.text();
        panic!("caption absent; calls={:?}; text={t:?}", fetch_calls());
    }
    let caption = mount
        .q("[data-testid='light-reality']")
        .expect("caption")
        .text_content()
        .unwrap_or_default();
    assert!(
        caption.contains("light.tent") && caption.contains("06:05"),
        "caption reports entity and learned time: {caption:?}"
    );
}

#[wasm_bindgen_test(async)]
async fn light_cycle_apply_puts_the_form_values() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, body| match (method, url) {
        ("GET", u) if u.ends_with("/light-cycle") => (200, EMPTY_CYCLE.to_string()),
        ("PUT", u) if u.ends_with("/light-cycle") => {
            // Echo back what was stored so the form keeps the values.
            (200, body.to_string())
        }
        _ => (404, String::new()),
    });
    let mount = editor();
    wait_for(|| lights_on_input(&mount).value() == "").await;
    let lights = lights_on_input(&mount);
    type_input(&lights, "07:45");
    let veg = &mount.q_all("input[type='number']")[2];
    type_input(veg, "20");
    click(&apply_button(&mount));
    wait_for(|| mount.text().contains("Saved.")).await;
    let put = fetch_calls()
        .into_iter()
        .find(|c| c.method == "PUT")
        .expect("PUT recorded");
    assert!(
        put.body.contains(r#""lights_on":"07:45""#) && put.body.contains(r#""vegetative":20"#),
        "form values reach the PUT body: {}",
        put.body
    );
}

#[wasm_bindgen_test(async)]
async fn light_cycle_reset_deletes_then_undo_reputs() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, body| match (method, url) {
        ("GET", u) if u.ends_with("/light-cycle") => (200, STORED_CYCLE.to_string()),
        ("DELETE", u) if u.ends_with("/light-cycle") => (204, String::new()),
        ("PUT", u) if u.ends_with("/light-cycle") => (200, body.to_string()),
        _ => (404, String::new()),
    });
    let mount = editor();
    wait_for(|| lights_on_input(&mount).value() == "07:30").await;
    let reset = mount
        .q_all("button")
        .into_iter()
        .find(|b| b.text_content().as_deref() == Some("Reset to default"))
        .expect("reset button");
    click(&reset);
    // Fields clear and the undo banner appears.
    wait_for(|| lights_on_input(&mount).value().is_empty() && mount.q("[role='status']").is_some())
        .await;
    assert!(
        mount.text().contains("Reset to default."),
        "reset status message shows"
    );
    // Undo re-PUTs the captured values.
    let undo = mount
        .q("[role='status'] button")
        .expect("undo button on the banner");
    click(&undo);
    wait_for(|| lights_on_input(&mount).value() == "07:30").await;
    let calls = fetch_calls();
    assert!(
        calls.iter().any(|c| c.method == "DELETE"),
        "reset issued DELETE"
    );
    let reputs: Vec<&Call> = calls.iter().filter(|c| c.method == "PUT").collect();
    assert!(
        reputs
            .iter()
            .any(|put| put.body.contains(r#""lights_on":"07:30""#)),
        "undo re-PUTs the pre-reset values"
    );
    assert!(mount.q("[role='status']").is_none(), "banner dismissed");
}

#[wasm_bindgen_test(async)]
async fn light_cycle_invalid_hours_show_a_validation_message() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| match (method, url) {
        ("GET", u) if u.ends_with("/light-cycle") => (200, EMPTY_CYCLE.to_string()),
        _ => (404, String::new()),
    });
    let mount = editor();
    wait_for(|| lights_on_input(&mount).value() == "").await;
    // A value no i64 can parse fails client-side validation with no
    // request. (Decimals never reach build_input — the browser's
    // step=1 constraint validation blocks the submit first — so an
    // overflowing digit string is the reachable bad input.)
    let veg = &mount.q_all("input[type='number']")[2];
    type_input(veg, "99999999999999999999");
    click(&apply_button(&mount));
    let seen = try_wait_for(&mut || mount.text().contains("whole numbers")).await;
    if !seen {
        let t = mount.text();
        panic!("no validation text; calls={:?}; text={t:?}", fetch_calls());
    }
    assert!(
        !fetch_calls().iter().any(|c| c.method == "PUT"),
        "invalid input never reaches the backend"
    );
}

#[wasm_bindgen_test(async)]
async fn light_cycle_fetch_failure_sets_the_status_line() {
    fresh_environment();
    let _stub = FetchStub::new(|method, _url, _body| match method {
        "GET" => (500, "nope".to_string()),
        _ => (404, String::new()),
    });
    let mount = editor();
    wait_for(|| mount.text().contains("500")).await;
    assert!(mount.text().contains("500"), "status line carries the code");
}
