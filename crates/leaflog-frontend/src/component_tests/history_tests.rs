// Component tests for the history page (D139): the report band,
// accuracy cards and yield aggregates with and without data, plus
// the start-next-run action.
use super::support::*;
use leptos::prelude::*;
use wasm_bindgen_test::*;

const EMPTY_REPORT: &str =
    r#"{"accuracy":[],"yield_by_label":[],"yield_by_substrate":[],"yield_by_feed_schedule":[]}"#;

fn report_json() -> String {
    r#"{"accuracy":[{"grow_id":"g1","name":"Spring run","strain":"Northern Lights","boundaries":[{"boundary":"flip","predicted_day":42,"actual_day":45,"delta_days":3}]}],"yield_by_label":[{"label":"Northern Lights","total_dry_g":120,"total_wet_g":900,"total_trimmed_g":30,"grows":2}],"yield_by_substrate":[],"yield_by_feed_schedule":[]}"#.to_string()
}

fn page() -> Mounted<impl IntoView + 'static> {
    Mounted::new(move || {
        with_i18n(move || {
            with_router(move || {
                view! { <crate::history::HistoryPage /> }
            })
        })
    })
}

#[wasm_bindgen_test]
async fn history_page_empty_report() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.ends_with("/api/history") {
            return (200, EMPTY_REPORT.to_string());
        }
        (404, String::new())
    });
    let mount = page();
    wait_text(&mount, "history builds itself").await;
    let text = mount.text();
    assert!(text.contains("History"), "heading: {text:?}");
    // The deviation stat shows a dash until one boundary is logged.
    let deviation = mount
        .q("[data-testid='history-stat-deviation'] dd")
        .expect("deviation stat")
        .text_content();
    assert_eq!(
        deviation.as_deref(),
        Some("—"),
        "deviation dash: {deviation:?}"
    );
    // All three aggregates show their nothing-weighed fallback.
    assert_eq!(
        mount.text().matches("Nothing weighed yet").count(),
        3,
        "three aggregate fallbacks"
    );
}

#[wasm_bindgen_test]
async fn history_page_report_with_data() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.ends_with("/api/history") {
            return (200, report_json());
        }
        (404, String::new())
    });
    let mount = page();
    wait_text(&mount, "Spring run").await;
    let text = mount.text();
    assert!(text.contains("Northern Lights"), "strain badge: {text:?}");
    assert!(
        text.contains("day 45 (planned 42, +3)"),
        "boundary line: {text:?}"
    );
    let runs = mount
        .q("[data-testid='history-stat-runs'] dd")
        .expect("runs stat")
        .text_content();
    assert_eq!(runs.as_deref(), Some("1"), "run count");
    let deviation = mount
        .q("[data-testid='history-stat-deviation'] dd")
        .expect("deviation stat")
        .text_content();
    assert!(
        deviation.as_deref().is_some_and(|d| d.contains('3')),
        "mean deviation: {deviation:?}"
    );
    assert!(text.contains("120 g"), "dry yield: {text:?}");
    assert!(
        text.contains("Northern Lights") && mount.has("table"),
        "label aggregate table"
    );
    // The two empty aggregates still show their fallback.
    assert_eq!(
        mount.text().matches("Nothing weighed yet").count(),
        2,
        "substrate and schedule fallbacks"
    );
}

#[wasm_bindgen_test]
async fn history_page_next_run_posts() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.ends_with("/api/history") {
            return (200, report_json());
        }
        if method == "POST" && url.ends_with("/api/grows/g1/next-run") {
            return (200, "{}".to_string());
        }
        (404, String::new())
    });
    let mount = page();
    wait_text(&mount, "Spring run").await;
    let button = mount
        .q_all("button")
        .into_iter()
        .find(|b| b.text_content().as_deref() == Some("Start next run"))
        .expect("next-run button");
    let before = fetch_calls().len();
    click(&button);
    let posted = try_wait_for(&mut || {
        fetch_calls()
            .iter()
            .any(|call| call.method == "POST" && call.url.ends_with("/api/grows/g1/next-run"))
    })
    .await;
    assert!(posted, "next-run POST recorded");
    // The refresh re-issues the report GET afterwards.
    let _ = try_wait_for(&mut || fetch_calls().len() > before + 1).await;
}

#[wasm_bindgen_test]
async fn history_page_error_banner() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.ends_with("/api/history") {
            return (503, "down".to_string());
        }
        (404, String::new())
    });
    let mount = page();
    wait_text(&mount, "status 503").await;
    assert!(
        mount.q("button[aria-label='Retry']").is_some(),
        "retry button rendered"
    );
}

#[wasm_bindgen_test]
async fn plant_page_names_the_plant_as_its_heading() {
    fresh_environment();
    let _stub = FetchStub::new(move |method, url, _body| {
        if method == "GET" && url.contains("/api/plants/") && url.ends_with("/view") {
            return (
                200,
                r#"{"plant":{"id":"p1","grow_id":"g1","name":"NL-1","created_at":"2026-11-01T00:00:00Z","updated_at":"2026-11-01T00:00:00Z"},"actions":[{"id":"a1","grow_id":"g1","plant_ids":["p1"],"occurred_at":"2026-11-09T10:00:00Z","action_type":"phase_change","phase":"flower","grow_day":31,"details":{"target_phase":"flower"},"notes":"","appearance_tags":[],"created_at":"2026-11-09T10:00:00Z","updated_at":"2026-11-09T10:00:00Z"}],"photos":[],"timezone":"Europe/Berlin"}"#
                    .to_string(),
            );
        }
        (404, String::new())
    });
    let mount = Mounted::new(move || {
        with_i18n(move || {
            with_router(move || {
                view! {
                    <crate::history::PlantView plant_id=Signal::derive(|| "p1".to_string()) />
                }
            })
        })
    });
    let shown = try_wait_for(&mut || mount.contains("NL-1")).await;
    assert!(shown, "plant page text: {}", mount.text());
    let heading = mount.q("h2").and_then(|el| el.text_content());
    assert_eq!(
        heading.as_deref(),
        Some("NL-1"),
        "the plant is the page's h2"
    );
    // Entries read as labelled rows, never as the stored key.
    assert!(!mount.text().contains("phase_change"), "{}", mount.text());
    assert!(mount.text().contains("Phase change"), "{}", mount.text());
}
