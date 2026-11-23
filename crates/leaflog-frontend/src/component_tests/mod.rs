//! Browser-run component tests (D139): the frontend crate's own
//! component suite, executed by wasm-bindgen-test-runner in headless
//! Chrome (`just test-wasm`). Gated to the wasm target so native
//! `cargo test` never compiles them; the `just ci` gate runs both.

// The wait helpers take `FnMut` closures that capture mounted views,
// whose render state is not `Send`. These tests run single-threaded
// in one browser tab, so `Send` is meaningless here.
#![allow(clippy::future_not_send)]

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

mod bindings_tests;
mod feed_tests;
mod gantt_tests;
mod grows_tests;
mod history_tests;
mod light_cycle_tests;
mod onboarding_tests;
mod quick_add_tests;
mod schedules_tests;
mod settings_tests;
mod shared_components_tests;
mod story_tests;
mod support;
mod ui_tests;
