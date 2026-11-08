//! Leaflog frontend: a Leptos CSR single-page app.
//!
//! The router base is set at **runtime** from the ingress path the
//! backend injects into the served `index.html`
//! (`window.__LEAFLOG_BASE__`, plus a `<base href>`); all asset and API
//! URLs are relative, so deep links and refreshes work under an
//! arbitrary ingress prefix (verified by the M1 spike).

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

pub mod api;
mod equipment;
mod strains;

use equipment::EquipmentPage;
use js_sys::{Reflect, global};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::{A, Route, Router, Routes};
use leptos_router::path;
use strains::StrainsPage;
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

/// Root component: top navigation plus the router wired to the runtime
/// ingress base.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn App() -> impl IntoView {
    view! {
        <Router base=ingress_base()>
            <div class="shell">
                <header class="topbar">
                    <span class="brand" aria-hidden="true">"🌿"</span>
                    <h1>"Leaflog"</h1>
                    <nav class="mainnav" aria-label="Main">
                        <A href="/" exact=true>"Home"</A>
                        <A href="/equipment">"Equipment"</A>
                        <A href="/strains">"Strains"</A>
                    </nav>
                </header>
                <main id="main">
                    <Routes fallback=|| view! { <p>"Loading…"</p> }>
                        <Route path=path!("/") view=Home />
                        <Route path=path!("/spike") view=Spike />
                        <Route path=path!("/equipment") view=EquipmentPage />
                        <Route path=path!("/strains") view=StrainsPage />
                    </Routes>
                </main>
            </div>
        </Router>
    }
}

/// Home view: quick links and the backend health payload (fetched with
/// a *relative* URL, resolved through the injected `<base href>`).
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
        <section class="page">
            <div class="card">
                <h2>"Grow diary, at home in Home Assistant"</h2>
                <p>
                    "Track your grows: log actions and photos with sensor data, follow a
                     feeding schedule fitted to your plants, and keep every degree,
                     pH point and watt in one private place."
                </p>
                <p class="muted small">
                    "Milestone M2: the foundation — equipment inventory and the strain
                     database (bundled records carry sources and evidence tags). More
                     arrives with each milestone."
                </p>
                <ul class="links">
                    <li><A href="/equipment">"Equipment inventory"</A></li>
                    <li><A href="/strains">"Strain database"</A></li>
                </ul>
                <h3>"Backend health"</h3>
                <pre aria-live="polite">{move || health.get()}</pre>
            </div>
        </section>
    }
}

/// Deep-link target proving router navigation and refreshes work under
/// the runtime base (kept from the M1 spike).
#[component]
fn Spike() -> impl IntoView {
    view! {
        <section class="card">
            <h2>"Ingress spike"</h2>
            <p>"This deep link was served by the SPA under runtime base:"</p>
            <p><code>{ingress_base()}</code></p>
            <p><A href="/">"Back to home"</A></p>
        </section>
    }
}
