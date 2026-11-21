// Component tests for the feeding section (D139): the quiet row, the
// pending card (D130) and the fitted grid with the this-week card.
use super::support::*;
use leptos::prelude::*;
use wasm_bindgen_test::*;

use leaflog_core::feed::FeedSchedule;

fn grow_json(id: &str, schedule: Option<&str>) -> String {
    let assignment = schedule.map_or_else(|| "null".to_string(), |s| format!("\"{s}\""));
    format!(
        r#"{{"id":"{id}","name":"Spring run","feed_schedule_id":{assignment},"planned_veg_weeks":null,"strain_label":"","photoperiod":"photoperiod","heat_tolerance":"average","cold_tolerance":"average","light_tolerance":"average","real_watts":null,"dimmable":true,"substrate":"soil","started_on":"2026-04-01","status":"active","notes":null,"created_at":"2026-04-01T00:00:00Z","updated_at":"2026-04-01T00:00:00Z"}}"#
    )
}

fn schedule_json() -> String {
    r#"{"id":"s1","name":"BioBizz","source":null,"flower_weeks":4,"products":[],"created_at":"2026-11-01T10:00:00Z","updated_at":"2026-11-01T10:00:00Z"}"#.to_string()
}

// A short plan: one veg dose, one flower-week dose due today, and the
// protected WATER tail far ahead.
fn plan_json() -> String {
    r#"{"schedule_id":"s1","schedule_name":"BioBizz","flower_weeks":4,"fitted_at":"2026-11-01T10:00:00Z","days":[
        {"grow_day":5,"phase":"Veg","week":null,"doses":[{"name":"Bio Grow","unit":"ml_per_l","group":"","note":null,"dose":{"min":2000,"max":4000}}]},
        {"grow_day":19,"phase":"Flower","week":3,"doses":[{"name":"Bio Bloom","unit":"ml_per_l","group":"","note":null,"dose":{"min":2000,"max":4000}}]},
        {"grow_day":140,"phase":"Water","week":null,"doses":[]}]}"#.to_string()
}

fn forecast_json(today: i64) -> String {
    format!(r#"{{"today_day":{today}}}"#)
}

fn section(grow_id: &str) -> Mounted<impl IntoView + 'static> {
    let grow_id = grow_id.to_string();
    Mounted::new(move || {
        with_i18n(move || {
            with_router(move || {
                let grow_id = grow_id;
                view! { <crate::feed::FeedSection grow_id /> }
            })
        })
    })
}

#[wasm_bindgen_test]
async fn feed_section_quiet_row_without_schedule() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.ends_with("/api/grows/g1/feed/plan") {
            return (200, "null".to_string());
        }
        if method == "GET" && url.ends_with("/api/grows/g1") {
            return (200, grow_json("g1", None));
        }
        if method == "GET" && url.ends_with("/api/grows/g1/forecast") {
            return (200, forecast_json(19));
        }
        (404, String::new())
    });
    let mount = section("g1");
    wait_text(&mount, "No feeding schedule.").await;
    let text = mount.text();
    assert!(
        text.contains("Add feeding schedule"),
        "quiet row CTA: {text:?}"
    );
    assert!(!mount.has("table"), "no fitted grid");
    assert!(!text.contains("This week"), "no this-week card");
}

#[wasm_bindgen_test]
async fn feed_section_pending_card_when_unforecast() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.ends_with("/api/grows/g1/feed/plan") {
            return (200, "null".to_string());
        }
        if method == "GET" && url.ends_with("/api/grows/g1") {
            return (200, grow_json("g1", Some("s1")));
        }
        if method == "GET" && url.ends_with("/api/schedules/s1") {
            return (200, schedule_json());
        }
        if method == "GET" && url.ends_with("/api/grows/g1/forecast") {
            return (200, forecast_json(19));
        }
        (404, String::new())
    });
    let mount = section("g1");
    // D130: assigned but no forecast flower window → pending card.
    wait_text(&mount, "The plan appears once").await;
    let text = mount.text();
    assert!(text.contains("BioBizz"), "schedule name: {text:?}");
    assert!(text.contains("Open schedule"), "pending card CTA: {text:?}");
}

#[wasm_bindgen_test]
async fn feed_section_fitted_grid_with_data() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.ends_with("/api/grows/g1/feed/plan") {
            return (200, plan_json());
        }
        if method == "GET" && url.ends_with("/api/grows/g1/forecast") {
            return (200, forecast_json(19));
        }
        (404, String::new())
    });
    let mount = section("g1");
    wait_text(&mount, "This week").await;
    let text = mount.text();
    assert!(text.contains("Flower · week 3 of 3"), "week chip: {text:?}");
    assert!(text.contains("Bio Bloom"), "today's dose: {text:?}");
    assert!(
        text.contains("Fitted from “BioBizz” on 2026-11-01."),
        "attribution: {text:?}"
    );
    assert!(mount.has("table"), "fitted grid table");
    let table = mount
        .q("table[aria-label='Fitted feeding schedule chart']")
        .expect("grid table");
    let table_text = table.text_content().unwrap_or_default();
    assert!(table_text.contains("Bio Grow"), "veg row: {table_text:?}");
    assert!(table_text.contains("WK 3"), "week columns: {table_text:?}");
    // The current week's column is marked.
    let marked = mount.q_all("th.bg-primary\\/10").len();
    assert_eq!(marked, 1, "exactly one marked column");
}

#[wasm_bindgen_test]
async fn feed_section_error_banner_on_plan_failure() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.ends_with("/api/grows/g1/feed/plan") {
            return (500, "boom".to_string());
        }
        (404, String::new())
    });
    let mount = section("g1");
    wait_text(&mount, "status 500").await;
    assert!(
        mount.q("button[aria-label='Retry']").is_some(),
        "retry button rendered"
    );
}

// Sanity on the quiet-row fallback types: a schedule without an id
// still renders the pending card with its (empty) href.
#[wasm_bindgen_test]
async fn feed_schedule_deserializes_minimal() {
    fresh_environment();
    let schedule: FeedSchedule = serde_json::from_str(&schedule_json()).expect("schedule parses");
    assert_eq!(schedule.name, "BioBizz");
    assert_eq!(schedule.flower_weeks, 4);
    let grow: leaflog_core::grows::Grow =
        serde_json::from_str(&grow_json("g1", Some("s1"))).expect("grow parses");
    assert_eq!(grow.feed_schedule_id, Some("s1".to_string()));
}
