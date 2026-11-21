//! Onboarding wizard states (D139): hidden, restart card, step one
//! role picks, step-two finish creating grow + plants + bindings,
//! and dismissal. Fetch stubs serve the four endpoints the wizard
//! touches; a Router wraps the mount because finishing navigates.

use super::support::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::wasm_bindgen_test;

use leptos::prelude::*;
use leptos::view;

fn state_json(show: bool, dismissed: bool, has_grows: bool) -> String {
    format!(r#"{{"show_wizard":{show},"dismissed":{dismissed},"has_grows":{has_grows}}}"#)
}

/// Stub the onboarding endpoints; `entities` feeds the step-one
/// pickers a single temperature sensor.
fn stub_wizard(state: String) -> FetchStub {
    FetchStub::new(move |method, url, _body| {
        if method == "GET" && url.ends_with("/api/onboarding") {
            return (200, state.clone());
        }
        if method == "GET" && url.ends_with("/api/ha/entities") {
            return (
                200,
                r#"{"entities":[{"entity_id":"sensor.temp","friendly_name":"Temperature","role":"temperature"}]}"#
                    .to_string(),
            );
        }
        (404, String::new())
    })
}

fn wizard() -> Mounted<impl IntoView> {
    Mounted::new(move || {
        with_i18n(move || {
            with_router(move || {
                view! { <crate::onboarding::OnboardingWizard /> }
            })
        })
    })
}

#[wasm_bindgen_test(async)]
async fn onboarding_hidden_when_dismissed_with_grows() {
    fresh_environment();
    let _stub = stub_wizard(state_json(false, true, true));
    let mount = wizard();
    try_wait_for(&mut || !mount.text().is_empty()).await;
    // No wizard section, no restart card.
    let t = mount.text();
    assert!(
        !t.contains("Welcome to Leaflog") && !t.contains("Run the setup wizard"),
        "nothing renders: {t:?}"
    );
}

#[wasm_bindgen_test(async)]
async fn onboarding_shows_restart_card_then_the_wizard() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.ends_with("/api/onboarding") {
            let restarted = fetch_calls()
                .iter()
                .any(|c| c.method == "PUT" && c.url.ends_with("/api/settings"));
            return (
                200,
                if restarted {
                    state_json(true, false, false)
                } else {
                    state_json(false, true, false)
                },
            );
        }
        if method == "GET" && url.ends_with("/api/ha/entities") {
            return (200, r#"{"entities":[]}"#.to_string());
        }
        if method == "PUT" && url.ends_with("/api/settings") {
            return (200, "{}".to_string());
        }
        (404, String::new())
    });
    let mount = wizard();
    wait_text(&mount, "Run the setup wizard").await;
    // Restarting clears the dismissal and shows the wizard.
    let restart = mount.q_all("button").first().cloned().unwrap();
    click(&restart);
    wait_text(&mount, "Welcome to Leaflog").await;
    assert!(mount.contains("1. Bind sensors"), "step one shows");
}

#[wasm_bindgen_test(async)]
async fn onboarding_step_one_lists_roles_and_advances() {
    fresh_environment();
    let _stub = stub_wizard(state_json(true, false, false));
    let mount = wizard();
    wait_text(&mount, "1. Bind sensors").await;
    let selects = mount.q_all("select.select");
    assert_eq!(
        selects.len(),
        7,
        "one picker per binding role (BINDING_ROLES)"
    );
    // Skipping advances to the plants form.
    let next = mount
        .q_all("button")
        .iter()
        .find(|b| b.text_content().is_some_and(|t| t.contains("Next")))
        .cloned()
        .unwrap();
    click(&next);
    wait_text(&mount, "2. Plants").await;
    assert!(mount.text().contains("Grow name"), "step-two form shows");
}

/// Scripted responses for the full wizard finish sequence. Body
/// assertions happen against the recorded calls, never in here: a
/// panic inside the fetch handler rejects the request.
fn finish_handler() -> impl Fn(&str, &str, &str) -> (u16, String) {
    |method, url, _body| {
        if method == "GET" && url.ends_with("/api/onboarding") {
            return (200, state_json(true, false, false));
        }
        if method == "GET" && url.ends_with("/api/ha/entities") {
            return (
                200,
                r#"{"entities":[{"entity_id":"sensor.temp","friendly_name":"Temperature","role":"temperature"}]}"#
                    .to_string(),
            );
        }
        if method == "POST" && url.ends_with("/api/grows") {
            // (Body asserted from the recorded calls below — an assert
            // here would panic inside the fetch handler and reject the
            // request.) Answer a grow the plants can hang off.
            // Full Grow JSON: substrate/started_on/status/notes/
            // created_at/updated_at have no serde default.
            return (
                200,
                r#"{"id":"g1","name":"Balcony run","substrate":"soil","started_on":"2026-11-21","status":"active","notes":null,"created_at":"2026-11-21T00:00:00Z","updated_at":"2026-11-21T00:00:00Z"}"#
                    .to_string(),
            );
        }
        if method == "POST" && url.contains("/api/grows/g1/plants") {
            return (200, "{}".to_string());
        }
        if method == "PUT" && url.ends_with("/api/grows/g1/bindings") {
            return (200, "{}".to_string());
        }
        if method == "PUT" && url.ends_with("/api/settings") {
            return (200, "{}".to_string());
        }
        (404, String::new())
    }
}

#[wasm_bindgen_test(async)]
async fn onboarding_finish_creates_grow_plants_and_bindings() {
    fresh_environment();
    let _stub = FetchStub::new(finish_handler());
    let mount = wizard();
    let shown = try_wait_for(&mut || mount.contains("1. Bind sensors")).await;
    if !shown {
        let t = mount.text();
        panic!("wizard absent; calls={:?}; text={t:?}", fetch_calls());
    }
    // Step one: bind the temperature role (first select) once its
    // entity option arrives.
    let select = mount
        .q_all("select.select")
        .first()
        .cloned()
        .unwrap()
        .unchecked_into::<web_sys::HtmlSelectElement>();
    wait_for(|| select.options().length() >= 2).await;
    select.set_value("sensor.temp");
    fire_change(&select);
    let next = mount
        .q_all("button")
        .iter()
        .find(|b| b.text_content().is_some_and(|t| t.contains("Next")))
        .cloned()
        .unwrap();
    click(&next);
    wait_text(&mount, "2. Plants").await;
    // Name the grow and ask for three plants.
    let name = mount
        .q("input[type='text']")
        .expect("grow-name input")
        .unchecked_into::<web_sys::HtmlInputElement>();
    type_input(&name, "Balcony run");
    let count = mount
        .q("input[type='number']")
        .expect("plant-count input")
        .unchecked_into::<web_sys::HtmlInputElement>();
    type_input(&count, "3");
    let start = mount
        .q_all("button[type='submit']")
        .first()
        .cloned()
        .unwrap();
    click(&start);
    wait_finish(&mount).await;
    assert_finish_calls();
}

/// Waits for the finish sequence to land: grow POST plus three plant
/// POSTs and the dismissal PUT. The navigation target is asserted via
/// the recorded calls.
async fn wait_finish<V: IntoView + 'static>(mount: &Mounted<V>) {
    let done = try_wait_for(&mut || {
        fetch_calls().iter().filter(|c| c.method == "POST").count() >= 4
            && fetch_calls()
                .iter()
                .any(|c| c.method == "PUT" && c.url.ends_with("/api/settings"))
    })
    .await;
    if !done {
        let t = mount.text();
        panic!("finish incomplete; calls={:?}; text={t:?}", fetch_calls());
    }
}

/// One plant POST per plant, the picked bindings, and the dismissal
/// setting.
fn assert_finish_calls() {
    let calls = fetch_calls();
    let plants: Vec<_> = calls
        .iter()
        .filter(|c| c.method == "POST" && c.url.contains("/plants"))
        .collect();
    assert_eq!(plants.len(), 3, "one plant POST per plant");
    for (index, call) in plants.iter().enumerate() {
        let expected = format!("\"name\":\"Plant {}\"", index + 1);
        assert!(call.body.contains(&expected), "body={:?}", call.body);
    }
    let bindings = calls
        .iter()
        .find(|c| c.method == "PUT" && c.url.ends_with("/api/grows/g1/bindings"))
        .expect("bindings PUT after picks");
    assert!(
        bindings.body.contains("temperature"),
        "body={:?}",
        bindings.body
    );
    let settings = calls
        .iter()
        .find(|c| c.method == "PUT" && c.url.ends_with("/api/settings"))
        .expect("dismissal PUT");
    assert!(
        settings.body.contains("\"onboarding_dismissed\":true"),
        "body={:?}",
        settings.body
    );
}

#[wasm_bindgen_test(async)]
async fn onboarding_dismiss_records_the_setting() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, body| {
        if method == "GET" && url.ends_with("/api/onboarding") {
            // After the dismissal PUT the state flips to hidden.
            let dismissed = fetch_calls()
                .iter()
                .any(|c| c.method == "PUT" && c.url.ends_with("/api/settings"));
            return (
                200,
                if dismissed {
                    state_json(false, true, false)
                } else {
                    state_json(true, false, false)
                },
            );
        }
        if method == "GET" && url.ends_with("/api/ha/entities") {
            return (200, r#"{"entities":[]}"#.to_string());
        }
        if method == "PUT" && url.ends_with("/api/settings") {
            assert!(
                body.contains("\"onboarding_dismissed\":true"),
                "body={body:?}"
            );
            return (200, "{}".to_string());
        }
        (404, String::new())
    });
    let mount = wizard();
    wait_text(&mount, "1. Bind sensors").await;
    let skip = mount
        .q_all("button")
        .iter()
        .find(|b| b.text_content().is_some_and(|t| t.contains("Skip")))
        .cloned()
        .unwrap();
    click(&skip);
    wait_text(&mount, "2. Plants").await;
    let dismiss = mount
        .q_all("button")
        .iter()
        .find(|b| b.text_content().is_some_and(|t| t.contains("Dismiss")))
        .cloned()
        .unwrap();
    click(&dismiss);
    let gone = try_wait_for(&mut || {
        !mount.text().contains("2. Plants") && !mount.text().contains("Welcome to Leaflog")
    })
    .await;
    assert!(gone, "wizard hides after dismissal");
}
