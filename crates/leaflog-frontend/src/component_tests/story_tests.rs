// Component tests for the grow story's independent photo cards.
use super::support::*;
use leptos::prelude::*;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
async fn photo_card_updates_names_after_plants_load() {
    fresh_environment();
    let photo: leaflog_core::grows::Photo = serde_json::from_value(serde_json::json!({
        "id": "photo1", "grow_id": "g1", "plant_id": "plant-1", "plant_ids": ["plant-1"],
        "captured_at": "2026-11-20T08:15:00Z", "grow_day": 50, "tags": [],
        "original_name": "photo.jpg", "thumbnail_name": "photo.webp", "bytes_stored": 629,
        "created_at": "2026-11-20T08:15:00Z", "updated_at": "2026-11-20T08:15:00Z"
    }))
    .expect("test photo");
    let plants = RwSignal::new(Vec::<leaflog_core::grows::Plant>::new());
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <crate::story::PhotoCard photo plants=plants.into()
                    timezone="Europe/Berlin".to_string().into()
                    armed_photo=RwSignal::new(None) delete_photo=|_| {} />
            }
        })
    });
    assert!(mount.text().contains("Whole grow"), "{}", mount.text());
    plants.set(vec![
        serde_json::from_value(serde_json::json!({
            "id": "plant-1", "grow_id": "g1", "name": "Test plant A",
            "created_at": "2026-11-20T08:15:00Z", "updated_at": "2026-11-20T08:15:00Z"
        }))
        .expect("test plant"),
    ]);
    assert!(
        try_wait_for(&mut || mount.has("a[aria-label='Open the original photo']")).await,
        "photo links arrive asynchronously: {}",
        mount.text()
    );
    // The caption carries the plant name once; the thumbnail is the
    // link's silent preview (impeccable audit).
    assert!(
        try_wait_for(&mut || mount.text().contains("Test plant A")).await,
        "plant names arrive asynchronously: {}",
        mount.text()
    );
}

#[wasm_bindgen_test]
async fn story_renders_a_plant_linked_photo() {
    fresh_environment();
    let _stub = FetchStub::new(|method, url, _body| {
        if method == "GET" && url.contains("/api/grows/g1/story") {
            return (
                200,
                r#"{"timezone":"Europe/Berlin","days":[{"date":"2026-11-20","grow_day":50,"phase":"vegetative","actions":[],"photos":[{"id":"photo1","grow_id":"g1","plant_id":"plant-1","plant_ids":["plant-1"],"action_log_id":null,"captured_at":"2026-11-20T08:15:00Z","grow_day":50,"phase":"vegetative","tags":[],"original_name":"photo.jpg","thumbnail_name":"photo.webp","bytes_stored":629,"created_at":"2026-11-20T08:15:00Z","updated_at":"2026-11-20T08:15:00Z"}],"automatic":false,"has_observations":false,"light_hours":null,"light_partial":true,"periods":[{"sparse":false},{"sparse":false}]}],"before":null}"#
                    .to_string(),
            );
        }
        (404, String::new())
    });
    let plants = vec![
        serde_json::from_value(serde_json::json!({
            "id": "plant-1", "grow_id": "g1", "name": "Test plant A",
            "created_at": "2026-11-20T08:15:00Z", "updated_at": "2026-11-20T08:15:00Z"
        }))
        .expect("test plant"),
    ];
    let mount = Mounted::new(move || {
        with_i18n(move || {
            with_router(move || {
                view! {
                    <crate::story::Story
                        grow_id="g1".to_string().into()
                        plants=plants.into()
                        selected_plants=RwSignal::new(Vec::new())
                        timezone=RwSignal::new("UTC".to_string())
                        revision=0.into()
                        on_add=Callback::new(|_| {})
                        on_delete=Callback::new(|_| {})
                    />
                }
            })
        })
    });
    assert!(
        try_wait_for(&mut || {
            mount.has("a[aria-label='Open the original photo']")
                && mount.text().contains("Test plant A")
        })
        .await,
        "plant-linked photo renders: {}",
        mount.text()
    );
}
