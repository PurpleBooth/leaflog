//! Leaflog frontend: a Leptos CSR single-page app.
//!
//! Milestone M1 spike goals: prove that the router base is set at
//! **runtime** from the ingress path the backend injects into the served
//! `index.html` (`window.__LEAFLOG_BASE__`, plus a `<base href>`), that
//! all asset URLs stay relative, and that deep links and refreshes work
//! under an arbitrary ingress prefix.

#![warn(clippy::nursery)]
#![deny(
    unused,
    nonstandard_style,
    future_incompatible,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    clippy::cargo,
    clippy::complexity,
    clippy::correctness,
    clippy::perf,
    clippy::style,
    clippy::suspicious,
    clippy::pedantic,
    non_fmt_panics
)]
#![allow(clippy::multiple_crate_versions)]

use js_sys::{Reflect, global};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::{A, Route, Router, Routes};
use leptos_router::path;
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

/// Reads `window.__LEAFLOG_BASE__`, injected at serve time by the
/// backend from the `X-Ingress-Path` header. Empty string when the app
/// is served at the root.
#[must_use]
pub fn ingress_base() -> String {
    Reflect::get(&global(), &JsValue::from_str("__LEAFLOG_BASE__"))
        .ok()
        .and_then(|value| value.as_string())
        .unwrap_or_default()
}

/// Module entry: mounts the app (called by `wasm-bindgen` on load).
#[wasm_bindgen(start)]
pub fn start_app() {
    mount_to_body(App);
}

/// Root component: wires the router with the runtime ingress base.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn App() -> impl IntoView {
    view! {
        <Router base=ingress_base()>
            <div class="shell">
                <header class="topbar">
                    <span class="brand" aria-hidden="true">"🌿"</span>
                    <h1>"Leaflog"</h1>
                </header>
                <main id="main">
                    <Routes fallback=|| view! { <p>"Loading…"</p> }>
                        <Route path=path!("/") view=Home />
                        <Route path=path!("/spike") view=Spike />
                    </Routes>
                </main>
            </div>
        </Router>
    }
}

/// Home view: reports the runtime base and the backend health payload
/// (fetched with a *relative* URL, resolved through the injected
/// `<base href>`).
#[component]
fn Home() -> impl IntoView {
    let health = RwSignal::new(String::from("…"));
    Effect::new(move |_| {
        spawn_local(async move {
            let fetched = match gloo_net::http::Request::get("api/health").send().await {
                Ok(resp) => match resp.text().await {
                    Ok(body) => body,
                    Err(err) => format!("read error: {err}"),
                },
                Err(err) => format!("network error: {err}"),
            };
            health.set(fetched);
        });
    });
    view! {
        <section class="card">
            <h2>"Grow diary, at home in Home Assistant"</h2>
            <p>
                "Runtime ingress base: " <code>{ingress_base()}</code>
            </p>
            <p>
                <A href="spike">"Try the deep-link spike page"</A>
            </p>
            <h3>"Backend health"</h3>
            <pre aria-live="polite">{move || health.get()}</pre>
        </section>
    }
}

/// Deep-link target proving router navigation and refreshes work under
/// the runtime base.
#[component]
fn Spike() -> impl IntoView {
    view! {
        <section class="card">
            <h2>"Ingress spike"</h2>
            <p>"This deep link was served by the SPA under runtime base:"</p>
            <p><code>{ingress_base()}</code></p>
            <p>
                "Refresh the page: it must reload through the ingress prefix \
                 without any absolute-path request."
            </p>
            <p><A href="/">"Back to home"</A></p>
        </section>
    }
}
