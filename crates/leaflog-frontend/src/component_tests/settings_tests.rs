//! The settings screen (SPEC §6's language override, D154): the backend settings that had no
//! control — loaded once, saved as one body, defaults for unset keys.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::wasm_bindgen_test;

use super::support::*;

fn screen() -> Mounted<impl IntoView> {
    Mounted::new(move || {
        with_i18n(move || {
            view! { <crate::settings::SettingsPage /> }
        })
    })
}

fn stub(state: &'static str) -> FetchStub {
    FetchStub::new(move |method, url, _body| {
        if method == "GET" && url.ends_with("/api/settings") {
            return (200, state.to_owned());
        }
        (200, "{}".to_owned())
    })
}

fn put_body() -> serde_json::Value {
    fetch_calls()
        .into_iter()
        .rev()
        .find(|call| call.method == "PUT" && call.url.ends_with("/api/settings"))
        .map(|call| serde_json::from_str(&call.body).expect("JSON body"))
        .expect("a settings PUT")
}

#[wasm_bindgen_test(async)]
async fn settings_screen_shows_the_stored_values() {
    fresh_environment();
    let _stub =
        stub(r#"{"auto_log_ha_events":false,"language":"de","quiet_hours":"22:00 - 07:00"}"#);
    let mount = screen();
    let shown = try_wait_for(&mut || {
        mount
            .q("select[data-setting=language]")
            .and_then(|el| el.dyn_into::<web_sys::HtmlSelectElement>().ok())
            .is_some_and(|select| select.value() == "de")
    })
    .await;
    assert!(shown, "the stored language is selected");
    let auto_log = mount
        .q("input[data-setting=auto_log_ha_events]")
        .and_then(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok())
        .expect("auto-log checkbox");
    assert!(!auto_log.checked(), "stored false beats the default");
}

#[wasm_bindgen_test(async)]
async fn settings_screen_saves_one_body_and_leaves_other_keys_alone() {
    fresh_environment();
    let _stub = stub("{}");
    let mount = screen();
    let loaded = try_wait_for(&mut || {
        mount
            .q("select[data-setting=language]")
            .and_then(|el| el.dyn_into::<web_sys::HtmlSelectElement>().ok())
            .is_some_and(|select| select.value().is_empty())
    })
    .await;
    assert!(loaded);
    let select = mount
        .q("select[data-setting=language]")
        .and_then(|el| el.dyn_into::<web_sys::HtmlSelectElement>().ok())
        .expect("language select");
    select.set_value("de");
    fire_change(&select);
    let quiet = mount
        .q("input[data-setting=quiet_hours]")
        .and_then(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok())
        .expect("quiet hours input");
    type_input(&quiet, "21:30 - 06:30");
    let save = mount.q("button[type=submit]").expect("save button");
    click(&save);
    let saved = try_wait_for(&mut || {
        fetch_calls()
            .iter()
            .any(|call| call.method == "PUT" && call.url.ends_with("/api/settings"))
    })
    .await;
    assert!(saved, "the form PUTs api/settings");
    let body = put_body();
    assert_eq!(body["language"], "de");
    assert_eq!(body["quiet_hours"], "21:30 - 06:30");
    assert_eq!(
        body["auto_log_ha_events"], true,
        "the default is sent for an unset key"
    );
    assert_eq!(body["display_timezone"], serde_json::Value::Null);
    assert_eq!(
        body.get("onboarding_dismissed"),
        None,
        "not this screen's key"
    );
    let confirmed = try_wait_for(&mut || mount.contains("Saved")).await;
    assert!(confirmed, "the screen confirms the save");
}

#[wasm_bindgen_test(async)]
async fn settings_screen_offers_no_save_until_the_stored_values_loaded() {
    fresh_environment();
    let _stub = FetchStub::new(move |method, url, _body| {
        if method == "GET" && url.ends_with("/api/settings") {
            return (500, r#"{"error":"database is locked"}"#.to_owned());
        }
        (200, "{}".to_owned())
    });
    let mount = screen();
    let failed = try_wait_for(&mut || mount.text().contains("database is locked")).await;
    assert!(failed, "the load error is shown; page: {}", mount.text());
    assert!(
        mount.q("button[type=submit]").is_none(),
        "a failed load leaves nothing to save: the draft still holds the defaults"
    );
    assert!(
        !fetch_calls().iter().any(|call| call.method == "PUT"),
        "nothing was sent"
    );
}
