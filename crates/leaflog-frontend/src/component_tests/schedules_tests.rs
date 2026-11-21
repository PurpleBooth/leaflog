// Component tests for the schedules library and the schedule
// editor (D139): library states, editor load/save/refit, dose
// validation.
use leptos::prelude::*;
use wasm_bindgen::JsCast;

use super::support::*;
use wasm_bindgen_test::*;

fn summary_json(id: &str, name: &str, used_by: i64) -> String {
    format!(
        r#"{{"schedule":{{"id":"{id}","name":"{name}","source":"Biobizz B.V.","flower_weeks":4,"products":[],"created_at":"2026-11-01T10:00:00Z","updated_at":"2026-11-01T10:00:00Z"}},"product_count":3,"groups":["Rooting","Bloom"],"units":["ml_per_l"],"used_by":{used_by}}}"#
    )
}

fn schedule_json(id: &str, name: &str) -> String {
    format!(
        r#"{{"id":"{id}","name":"{name}","source":"Biobizz B.V.","flower_weeks":4,"products":[{{"id":"p1","name":"Bio Grow","unit":"ml_per_l","group":"Veg","note":null,"doses":{{"propagation":null,"veg":{{"min":2000,"max":4000}},"flower":[null,null,null,null],"water":null}}}}],"created_at":"2026-11-01T10:00:00Z","updated_at":"2026-11-01T10:00:00Z"}}"#
    )
}

fn library() -> Mounted<impl IntoView + 'static> {
    Mounted::new(|| {
        with_i18n(move || with_router(move || view! { <crate::schedules::SchedulesPage /> }))
    })
}

#[wasm_bindgen_test]
async fn schedules_page_empty_state() {
    fresh_environment();
    let _stub = FetchStub::new(|_method, url, _body| {
        if url.ends_with("/api/schedules") {
            return (200, "[]".to_string());
        }
        (404, String::new())
    });
    let mount = library();
    wait_text(&mount, "No feeding schedules yet").await;
    let text = mount.text();
    assert!(text.contains("New schedule"), "hero CTA: {text:?}");
}

#[wasm_bindgen_test]
async fn schedules_page_lists_rows_with_badges() {
    fresh_environment();
    let _stub = FetchStub::new(|_method, url, _body| {
        if url.ends_with("/api/schedules") {
            return (
                200,
                format!(
                    "[{},{}]",
                    summary_json("s1", "BioBizz", 2),
                    summary_json("s2", "House blend", 0)
                ),
            );
        }
        (404, String::new())
    });
    let mount = library();
    wait_text(&mount, "BioBizz").await;
    let text = mount.text();
    assert!(text.contains("House blend"), "rows: {text:?}");
    assert!(text.contains("ml/L"), "unit display mapping: {text:?}");
    assert!(text.contains("Unused"), "unused badge: {text:?}");
    assert!(text.contains('2'), "used-by count: {text:?}");
    // Every row has edit / duplicate / delete affordances.
    assert_eq!(
        mount.q_all("button.btn.btn-ghost.btn-sm").len(),
        4,
        "2 rows × (duplicate + delete)"
    );
}

#[wasm_bindgen_test]
async fn schedules_page_delete_requires_confirm() {
    fresh_environment();
    let _stub = FetchStub::new(move |method, url, _body| {
        if method == "GET" && url.ends_with("/api/schedules") {
            return (200, format!("[{}]", summary_json("s2", "House blend", 0)));
        }
        if method == "DELETE" && url.ends_with("/api/schedules/s2") {
            return (204, String::new());
        }
        (404, String::new())
    });
    let mount = library();
    let loaded = try_wait_for(&mut || mount.text().contains("House blend")).await;
    assert!(
        loaded,
        "list never loaded: calls={calls:?} text={text:?}",
        calls = fetch_calls(),
        text = mount.text()
    );
    let delete = mount
        .q_all("button.btn.btn-ghost.btn-sm.text-error")
        .into_iter()
        .next()
        .expect("delete button");
    click(&delete);
    // Armed: the confirm button appears; nothing deleted yet.
    let armed = try_wait_for(&mut || !mount.q_all("button.btn.btn-error.btn-sm").is_empty()).await;
    if armed {
        assert!(
            !fetch_calls().iter().any(|c| c.method == "DELETE"),
            "delete must wait for confirmation"
        );
    }
    let confirm = mount
        .q("button.btn.btn-error.btn-sm")
        .expect("confirm button");
    click(&confirm);
    wait_for(&mut || {
        fetch_calls()
            .iter()
            .any(|c| c.method == "DELETE" && c.url.ends_with("/api/schedules/s2"))
    })
    .await;
}

#[wasm_bindgen_test]
async fn schedules_page_duplicate_posts() {
    fresh_environment();
    let _stub = FetchStub::new(move |method, url, _body| {
        if method == "GET" && url.ends_with("/api/schedules") {
            return (200, format!("[{}]", summary_json("s1", "BioBizz", 0)));
        }
        if method == "POST" && url.ends_with("/api/schedules/s1/duplicate") {
            return (200, schedule_json("s9", "BioBizz (copy)"));
        }
        (404, String::new())
    });
    let mount = library();
    wait_text(&mount, "BioBizz").await;
    let duplicate = mount
        .q_all("button.btn.btn-ghost.btn-sm")
        .into_iter()
        .find(|b| b.text_content().is_some_and(|t| t.contains("Duplicate")))
        .expect("duplicate button");
    click(&duplicate);
    wait_for(&mut || {
        fetch_calls()
            .iter()
            .any(|c| c.method == "POST" && c.url.ends_with("/duplicate"))
    })
    .await;
}

fn editor(existing: Option<&str>) -> Mounted<impl IntoView + 'static> {
    let existing = existing.map(str::to_string);
    Mounted::new(move || {
        with_i18n(move || {
            with_router(move || {
                let existing = existing;
                view! { <crate::schedules::ScheduleEditorPage existing /> }
            })
        })
    })
}

#[wasm_bindgen_test]
async fn schedule_editor_new_defaults() {
    fresh_environment();
    let _stub = FetchStub::new(|_method, _url, _body| (404, String::new()));
    let mount = editor(None);
    wait_text(&mount, "New feeding schedule").await;
    // One empty row, PROPAGATION + VEG + 8 weeks + WATER.
    let inputs = mount.q_all("input.input");
    // name + source + row fields (name/group/note) + dose cells.
    assert!(inputs.len() >= 14, "grid inputs: {}", inputs.len());
    assert!(mount.text().contains("Doses: number or range"));
    // No delete button on a new schedule.
    assert!(
        !mount.text().contains("Delete schedule"),
        "no delete affordance when unsaved"
    );
}

#[wasm_bindgen_test]
async fn schedule_editor_existing_loads() {
    fresh_environment();
    let _stub = FetchStub::new(move |method, url, _body| {
        if method == "GET" && url.ends_with("/api/schedules/s1") {
            return (200, schedule_json("s1", "BioBizz"));
        }
        (404, String::new())
    });
    let mount = editor(Some("s1"));
    // Record any uncaught JS errors while we wait.
    let () = eval(
        r"window.__errs = []; window.addEventListener('error', e => window.__errs.push(String(e.message || e))); window.addEventListener('unhandledrejection', e => window.__errs.push('rej:' + String(e.reason)));
",
    );
    // The refit note only renders once the stored schedule has loaded.
    wait_for(|| mount.text().contains("Applies to growing plants")).await;
    let text = mount.text();
    assert!(
        text.contains("Delete schedule"),
        "stored schedule can be deleted"
    );
    let name = mount
        .q("input.input")
        .expect("name input")
        .unchecked_into::<web_sys::HtmlInputElement>();
    assert_eq!(name.value(), "BioBizz", "loaded name");
    // 4 weeks → WK 1..WK 4 columns; "Bio Grow" row present. Product
    // names live in input values, not text content.
    let grid = mount.text();
    assert!(grid.contains("WK 4"), "week columns: {grid:?}");
    let products = mount
        .q_all("table input.input")
        .into_iter()
        .filter_map(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok())
        .collect::<Vec<_>>();
    assert!(
        products.iter().any(|p| p.value() == "Bio Grow"),
        "product row name input: {:?}",
        products
            .iter()
            .map(web_sys::HtmlInputElement::value)
            .collect::<Vec<_>>()
    );
}

#[wasm_bindgen_test]
async fn schedule_editor_invalid_dose_blocks_save() {
    fresh_environment();
    let _stub = FetchStub::new(move |method, url, _body| {
        if method == "GET" && url.ends_with("/api/schedules/s1") {
            return (200, schedule_json("s1", "BioBizz"));
        }
        (404, String::new())
    });
    let mount = editor(Some("s1"));
    // The dose grid renders only once the stored schedule has loaded.
    wait_for(|| !mount.q_all("table input").is_empty()).await;
    // Type a non-dose into the first dose cell (PROPAGATION).
    let cells = mount
        .q_all("table input.input-sm.text-center")
        .into_iter()
        .collect::<Vec<_>>();
    let cell = cells
        .first()
        .expect("dose cell input")
        .clone()
        .unchecked_into::<web_sys::HtmlInputElement>();
    type_input(&cell, "x");
    fire_change(&cell);
    let save = mount
        .q_all("button.btn.btn-primary")
        .into_iter()
        .find(|b| {
            b.text_content()
                .is_some_and(|t| t.contains("Save schedule"))
        })
        .expect("save button");
    click(&save);
    wait_text(&mount, "is not a dose").await;
    assert!(
        !fetch_calls().iter().any(|c| c.method == "PUT"),
        "invalid dose must abort the save"
    );
}

#[wasm_bindgen_test]
async fn schedule_editor_save_puts_and_refits() {
    fresh_environment();
    let _stub = FetchStub::new(move |method, url, _body| {
        if method == "GET" && url.ends_with("/api/schedules/s1") {
            return (200, schedule_json("s1", "BioBizz"));
        }
        if method == "PUT" && url.ends_with("/api/schedules/s1") {
            return (200, schedule_json("s1", "BioBizz"));
        }
        if method == "GET" && url.ends_with("/api/schedules/s1/grows") {
            return (200, r#"["g1","g2"]"#.to_string());
        }
        if method == "POST" && url.contains("/feed/refit") {
            return (200, "null".to_string());
        }
        (404, String::new())
    });
    let mount = editor(Some("s1"));
    // The dose grid renders only once the stored schedule has loaded.
    wait_for(|| !mount.q_all("table input").is_empty()).await;
    let save = mount
        .q_all("button.btn.btn-primary")
        .into_iter()
        .find(|b| {
            b.text_content()
                .is_some_and(|t| t.contains("Save schedule"))
        })
        .expect("save button");
    click(&save);
    wait_for(&mut || {
        let calls = fetch_calls();
        calls.iter().any(|c| c.method == "PUT")
            && calls.iter().filter(|c| c.method == "POST").count() >= 2
    })
    .await;
    let calls = fetch_calls();
    assert!(
        calls
            .iter()
            .any(|c| c.method == "PUT" && c.url.ends_with("/api/schedules/s1")),
        "PUT recorded"
    );
    assert!(
        calls.iter().any(|c| c.body.contains("BioBizz")),
        "body carries the name"
    );
    assert!(
        calls
            .iter()
            .any(|c| c.url.ends_with("/api/grows/g1/feed/refit")),
        "refit g1"
    );
    assert!(
        calls
            .iter()
            .any(|c| c.url.ends_with("/api/grows/g2/feed/refit")),
        "refit g2"
    );
}
