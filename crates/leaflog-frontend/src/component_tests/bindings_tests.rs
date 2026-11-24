//! `bindings.rs` components: the per-role entity pickers.

use super::support::*;
use leptos::prelude::*;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

const ENTITIES: &str = r#"{"entities":[
    {"entity_id":"sensor.tent_temp","attributes":{"friendly_name":"Tent Temperature"}},
    {"entity_id":"sensor.tent_hum","attributes":{"friendly_name":"Tent Humidity"}}
]}"#;

fn role_selects<V: leptos::IntoView + 'static>(
    mount: &Mounted<V>,
) -> Vec<web_sys::HtmlSelectElement> {
    mount
        .q_all("select.select")
        .into_iter()
        .map(wasm_bindgen::JsCast::unchecked_into)
        .collect()
}

#[wasm_bindgen_test(async)]
async fn entity_bindings_render_the_seven_role_pickers() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| match (method, url) {
        ("GET", u) if u.ends_with("/bindings") => (200, r#"{"bindings":{}}"#.to_string()),
        ("GET", u) if u.ends_with("/ha/entities") => (200, ENTITIES.to_string()),
        _ => (404, String::new()),
    });
    let epoch = RwSignal::new(0u64);
    let mount = Mounted::new(move || {
        with_i18n(
            move || view! { <crate::bindings::EntityBindings grow_id="g1".to_string() bindings_epoch=epoch /> },
        )
    });
    // 7 roles, each picker starts unbound once the fetch lands.
    wait_for(|| role_selects(&mount).len() == 7).await;
    for select in role_selects(&mount) {
        assert_eq!(select.value(), "", "all pickers start unbound");
    }
    // textContent concatenates without spaces, so match the words
    // individually.
    let text = mount.text();
    assert!(
        text.contains("Entity"),
        "heading renders; got: {:?}",
        text.get(..200).unwrap_or(&text)
    );
    assert!(mount.contains("bindings"), "heading renders");
}

#[wasm_bindgen_test(async)]
async fn entity_bindings_preselect_and_offer_friendly_names() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| match (method, url) {
        ("GET", u) if u.ends_with("/bindings") => (
            200,
            r#"{"bindings":{"temperature":"sensor.tent_temp"}}"#.to_string(),
        ),
        ("GET", u) if u.ends_with("/ha/entities") => (200, ENTITIES.to_string()),
        _ => (404, String::new()),
    });
    let epoch = RwSignal::new(0u64);
    let mount = Mounted::new(move || {
        with_i18n(
            move || view! { <crate::bindings::EntityBindings grow_id="g1".to_string() bindings_epoch=epoch /> },
        )
    });
    wait_for(|| {
        role_selects(&mount)
            .first()
            .is_some_and(|s| s.value() == "sensor.tent_temp")
    })
    .await;
    let first = &role_selects(&mount)[0];
    let options = first.options();
    assert_eq!(options.length(), 3, "unbound + two entities");
    assert!(
        options
            .item(1)
            .map(|o| o.text_content().unwrap_or_default())
            .is_some_and(|t| t.contains("Tent Temperature (sensor.tent_temp)")),
        "option shows friendly name plus id"
    );
}

#[wasm_bindgen_test(async)]
async fn entity_bindings_change_puts_the_whole_map_and_bumps_the_epoch() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| match (method, url) {
        ("GET", u) if u.ends_with("/bindings") => (
            200,
            r#"{"bindings":{"temperature":"sensor.tent_temp"}}"#.to_string(),
        ),
        ("GET", u) if u.ends_with("/ha/entities") => (200, ENTITIES.to_string()),
        ("PUT", u) if u.ends_with("/bindings") => (200, "{}".to_string()),
        _ => (404, String::new()),
    });
    let epoch = RwSignal::new(0u64);
    let mount = Mounted::new(move || {
        with_i18n(
            move || view! { <crate::bindings::EntityBindings grow_id="g1".to_string() bindings_epoch=epoch /> },
        )
    });
    wait_for(|| {
        role_selects(&mount)
            .first()
            .is_some_and(|s| s.value() == "sensor.tent_temp")
    })
    .await;
    // Bind humidity via the second picker.
    let second = &role_selects(&mount)[1];
    second.set_value("sensor.tent_hum");
    fire_change(second);
    wait_for(|| epoch.get_untracked() == 1).await;
    let calls = fetch_calls();
    let put = calls
        .iter()
        .find(|c| c.method == "PUT" && c.url.ends_with("/bindings"))
        .expect("PUT recorded");
    assert!(
        put.body.contains(r#""temperature":"sensor.tent_temp""#),
        "existing binding preserved: {}",
        put.body
    );
    assert!(
        put.body.contains(r#""humidity":"sensor.tent_hum""#),
        "new binding added: {}",
        put.body
    );
    // No error alert after a successful save.
    assert!(mount.q("[role='alert']").is_none());
}

#[wasm_bindgen_test(async)]
async fn entity_bindings_unbound_choice_removes_the_role() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| match (method, url) {
        ("GET", u) if u.ends_with("/bindings") => (
            200,
            r#"{"bindings":{"temperature":"sensor.tent_temp"}}"#.to_string(),
        ),
        ("GET", u) if u.ends_with("/ha/entities") => (200, ENTITIES.to_string()),
        ("PUT", u) if u.ends_with("/bindings") => (200, "{}".to_string()),
        _ => (404, String::new()),
    });
    let epoch = RwSignal::new(0u64);
    let mount = Mounted::new(move || {
        with_i18n(
            move || view! { <crate::bindings::EntityBindings grow_id="g1".to_string() bindings_epoch=epoch /> },
        )
    });
    wait_for(|| {
        role_selects(&mount)
            .first()
            .is_some_and(|s| s.value() == "sensor.tent_temp")
    })
    .await;
    let first = &role_selects(&mount)[0];
    first.set_value("");
    fire_change(first);
    wait_for(|| epoch.get_untracked() == 1).await;
    let calls = fetch_calls();
    let put = calls
        .iter()
        .find(|c| c.method == "PUT")
        .expect("PUT recorded");
    assert!(
        !put.body.contains("temperature"),
        "unbound choice drops the role: {}",
        put.body
    );
}

#[wasm_bindgen_test(async)]
async fn entity_bindings_fetch_failure_shows_the_error_banner() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| match (method, url) {
        ("GET", u) if u.ends_with("/bindings") => (500, "boom".to_string()),
        ("GET", u) if u.ends_with("/ha/entities") => (200, ENTITIES.to_string()),
        _ => (404, String::new()),
    });
    let epoch = RwSignal::new(0u64);
    let mount = Mounted::new(move || {
        with_i18n(
            move || view! { <crate::bindings::EntityBindings grow_id="g1".to_string() bindings_epoch=epoch /> },
        )
    });
    wait_for(|| mount.q("[role='alert']").is_some()).await;
    // A non-JSON error body maps to the status-only message (the raw
    // backend text never enters the DOM, D121).
    assert!(
        mount.text().contains("500"),
        "failure text reaches the alert"
    );
}

#[wasm_bindgen_test(async)]
async fn entity_bindings_show_the_stance_toggle_only_with_a_light_binding() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| match (method, url) {
        ("GET", u) if u.ends_with("/bindings") => (
            200,
            r#"{"bindings":{"temperature":"sensor.tent_temp"},"light_unavailable_as_off":false}"#
                .to_string(),
        ),
        ("GET", u) if u.ends_with("/ha/entities") => (200, ENTITIES.to_string()),
        _ => (404, String::new()),
    });
    let epoch = RwSignal::new(0u64);
    let mount = Mounted::new(move || {
        with_i18n(
            move || view! { <crate::bindings::EntityBindings grow_id="g1".to_string() bindings_epoch=epoch /> },
        )
    });
    wait_for(|| {
        role_selects(&mount)
            .first()
            .is_some_and(|s| s.value() == "sensor.tent_temp")
    })
    .await;
    assert!(
        mount
            .q("input[data-stance='light_unavailable_as_off']")
            .is_none(),
        "no light bound: the stance toggle stays hidden"
    );

    // Re-render with a bound light: the toggle appears, reflecting the
    // grow's stored stance.
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| match (method, url) {
        ("GET", u) if u.ends_with("/bindings") => (
            200,
            r#"{"bindings":{"light":"light.tent_led"},"light_unavailable_as_off":true}"#
                .to_string(),
        ),
        ("GET", u) if u.ends_with("/ha/entities") => (200, ENTITIES.to_string()),
        _ => (404, String::new()),
    });
    let epoch = RwSignal::new(0u64);
    let mount = Mounted::new(move || {
        with_i18n(
            move || view! { <crate::bindings::EntityBindings grow_id="g1".to_string() bindings_epoch=epoch /> },
        )
    });
    wait_for(|| {
        mount
            .q("input[data-stance='light_unavailable_as_off']")
            .is_some()
    })
    .await;
    let toggle = mount
        .q("input[data-stance='light_unavailable_as_off']")
        .expect("toggle rendered");
    let toggle: web_sys::HtmlInputElement = wasm_bindgen::JsCast::unchecked_into(toggle);
    assert!(toggle.checked(), "stored stance is on");
    let text = mount.text();
    assert!(
        text.contains("Unavailable counts as off"),
        "label renders in English: {:?}",
        text.get(..200).unwrap_or(&text)
    );
}

#[wasm_bindgen_test(async)]
async fn entity_bindings_toggle_the_stance_alongside_the_map() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| match (method, url) {
        ("GET", u) if u.ends_with("/bindings") => (
            200,
            r#"{"bindings":{"light":"light.tent_led"},"light_unavailable_as_off":false}"#
                .to_string(),
        ),
        ("GET", u) if u.ends_with("/ha/entities") => (200, ENTITIES.to_string()),
        ("PUT", u) if u.ends_with("/bindings") => (
            200,
            r#"{"bindings":{"light":"light.tent_led"},"light_unavailable_as_off":true}"#
                .to_string(),
        ),
        _ => (404, String::new()),
    });
    let epoch = RwSignal::new(0u64);
    let mount = Mounted::new(move || {
        with_i18n(
            move || view! { <crate::bindings::EntityBindings grow_id="g1".to_string() bindings_epoch=epoch /> },
        )
    });
    wait_for(|| {
        mount
            .q("input[data-stance='light_unavailable_as_off']")
            .is_some()
    })
    .await;
    let toggle = mount
        .q("input[data-stance='light_unavailable_as_off']")
        .expect("toggle rendered");
    let toggle: web_sys::HtmlInputElement = wasm_bindgen::JsCast::unchecked_into(toggle);
    assert!(!toggle.checked());
    toggle.set_checked(true);
    fire_change(toggle.as_ref());
    wait_for(|| epoch.get_untracked() == 1).await;
    let calls = fetch_calls();
    let put = calls
        .iter()
        .find(|c| c.method == "PUT" && c.url.ends_with("/bindings"))
        .expect("PUT recorded");
    assert!(
        put.body.contains(r#""light_unavailable_as_off":true"#),
        "stance travels with the save: {}",
        put.body
    );
    assert!(
        put.body.contains(r#""light":"light.tent_led""#),
        "binding preserved: {}",
        put.body
    );
    assert!(toggle.checked(), "optimistic local state");
}
