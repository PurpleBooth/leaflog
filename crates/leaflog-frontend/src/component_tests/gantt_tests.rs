//! `gantt.rs`: the phase-window timeline, fetched per grow.

use super::support::*;
use leptos::prelude::*;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

/// One full window set (all six phases); JSON wire shape.
const FORECAST: &str = r#"{
    "today_day": 10,
    "phase": "vegetative",
    "phase_day": 4,
    "windows": [
        {"phase": "germination", "earliest_start": 0, "likely_start": 0, "latest_start": 0, "earliest_end": 4, "likely_end": 5, "latest_end": 6, "actual_start": true},
        {"phase": "seedling", "earliest_start": 4, "likely_start": 5, "latest_start": 6, "earliest_end": 12, "likely_end": 14, "latest_end": 16, "actual_start": false},
        {"phase": "vegetative", "earliest_start": 12, "likely_start": 14, "latest_start": 16, "earliest_end": 30, "likely_end": 40, "latest_end": 50, "actual_start": false},
        {"phase": "flower", "earliest_start": 30, "likely_start": 40, "latest_start": 50, "earliest_end": 95, "likely_end": 105, "latest_end": 115, "actual_start": false},
        {"phase": "drying", "earliest_start": 95, "likely_start": 105, "latest_start": 115, "earliest_end": 112, "likely_end": 122, "latest_end": 132, "actual_start": false},
        {"phase": "curing", "earliest_start": 112, "likely_start": 122, "latest_start": 132, "earliest_end": 132, "likely_end": 142, "latest_end": 152, "actual_start": false}
    ]
}"#;

/// Open-ended: an unpinned photoperiod veg projects no flower window.
const OPEN_ENDED: &str = r#"{
    "today_day": 40,
    "phase": "vegetative",
    "phase_day": 40,
    "windows": [
        {"phase": "germination", "earliest_start": 0, "likely_start": 0, "latest_start": 0, "earliest_end": 4, "likely_end": 5, "latest_end": 6, "actual_start": true},
        {"phase": "seedling", "earliest_start": 4, "likely_start": 5, "latest_start": 6, "earliest_end": 12, "likely_end": 14, "latest_end": 16, "actual_start": false},
        {"phase": "vegetative", "earliest_start": 12, "likely_start": 14, "latest_start": 16, "earliest_end": 30, "likely_end": 40, "latest_end": 50, "actual_start": false}
    ]
}"#;

fn stub(forecast: &'static str) -> FetchStub {
    FetchStub::new(move |method, url, _body| {
        assert_eq!(method, "GET");
        assert!(url.ends_with("/forecast"), "unexpected url {url}");
        (200, forecast.to_string())
    })
}

#[wasm_bindgen_test(async)]
async fn gantt_shows_loading_without_data() {
    fresh_environment();
    // An empty window set settles immediately: no rows, the today
    // marker, and the bare caption count (open-ended shape).
    let _stub = stub(r#"{"today_day":0,"phase":"germination","phase_day":0,"windows":[]}"#);
    let mount = Mounted::new(move || {
        with_i18n(move || view! { <crate::gantt::GanttTimeline grow_id="g1".to_string() /> })
    });
    let settled = try_wait_for(&mut || !mount.contains("Loading forecast")).await;
    if !settled {
        let t = mount.text();
        panic!("still loading; calls={:?}; text={t:?}", fetch_calls());
    }
    let t = mount.text();
    assert!(t.contains("today"), "today marker renders: {t:?}");
    assert!(t.ends_with('0'), "bare caption count renders: {t:?}");
    assert!(
        mount.q_all(".gantt-row").is_empty(),
        "no rows without windows"
    );
}

#[wasm_bindgen_test(async)]
async fn gantt_renders_one_row_per_window() {
    fresh_environment();
    let _stub = stub(FORECAST);
    let mount = Mounted::new(move || {
        with_i18n(move || view! { <crate::gantt::GanttTimeline grow_id="g1".to_string() /> })
    });
    // Six phase rows land once the forecast resolves.
    wait_for(|| mount.q_all(".gantt-row").len() >= 6).await;
    let t = mount.text();
    assert!(
        t.contains("Germination"),
        "first phase row labelled; text={t:?}"
    );
    assert!(mount.contains("Curing"), "last phase row labelled");
}

#[wasm_bindgen_test(async)]
async fn gantt_open_ended_forecast_hides_the_total() {
    fresh_environment();
    let _stub = stub(OPEN_ENDED);
    let mount = Mounted::new(move || {
        with_i18n(move || view! { <crate::gantt::GanttTimeline grow_id="g1".to_string() /> })
    });
    wait_for(|| mount.q_all(".gantt-row").len() >= 3).await;
    // Open-ended: caption shows the bare count, no " / total" suffix.
    let text = mount.text();
    assert!(
        !text.contains(" / "),
        "no projected total on open-ended plans: {text:?}"
    );
}

#[wasm_bindgen_test(async)]
async fn gantt_fetch_failure_shows_the_error_banner() {
    fresh_environment();
    let _stub = FetchStub::new(|_method, _url, _body| (503, "unavailable".to_string()));
    let mount = Mounted::new(move || {
        with_i18n(move || view! { <crate::gantt::GanttTimeline grow_id="g1".to_string() /> })
    });
    wait_for(|| mount.q("[role='alert']").is_some()).await;
    assert!(mount.text().contains("503"), "status reaches the alert");
}
