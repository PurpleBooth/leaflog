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
// clippy 1.100 warns on the empty marker enums the Leptos TypedBuilder
// derive emits for every #[component]; the crate writes none itself.
#![allow(clippy::empty_enums)]

pub mod api;
mod bindings;
mod chart;
mod chart_option;
mod dashboard;
mod edit;
mod env_section;
mod feed;
mod gantt;
mod grows;
mod history;
mod labels;
mod light_cycle;
mod live;
mod onboarding;
mod phase_pick;
mod quick_add;
mod schedules;
mod theme;
mod ui;

// Browser-run component tests (D139): compiled only for the wasm
// target under `cargo test`, executed by wasm-bindgen-test-runner in
// headless Chrome via `just test-wasm`.
#[cfg(all(test, target_arch = "wasm32"))]
mod component_tests;

// The compile-time i18n module (D112): `build.rs` generates it from
// `locales/{en,de}.json` into OUT_DIR and the include declares
// `crate::i18n`. English is the default locale and the key source of
// truth; the generated keys struct fails the build when a key is
// missing from either catalog. No cookie: the locale chain (HA →
// browser → English) is applied explicitly, never persisted.
//
// The generated code is macro output: it trips the crate's
// pedantic/nursery denies (use_self, must_use_candidate, …) and omits
// item docs, so this module is the one narrow allow for codegen
// (logged in docs/DECISIONS.md).
#[allow(missing_docs, clippy::pedantic, clippy::nursery)]
mod generated_i18n {
    include!(concat!(env!("OUT_DIR"), "/i18n/mod.rs"));
}
pub use generated_i18n::i18n;

use edit::GrowEditRoute;
use grows::{GrowsPage, GrowsRoute, Landing};
use history::{HistoryPage, PlantViewPage};
use js_sys::{Reflect, global};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::{A, Route, Router, Routes};
use leptos_router::path;
use onboarding::OnboardingWizard;
use schedules::{ScheduleEditRoute, ScheduleNewRoute, SchedulesPage};
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

use i18n::{I18nContextProvider, Locale, t, t_string, use_i18n};
use leptos_i18n::td_string;

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

/// Builds an in-app `<A href>` under the runtime ingress base.
///
/// `leptos_router` 0.8 renders hrefs that already start with `/`
/// verbatim (its `use_resolved_path` prepends the router base only to
/// relative hrefs), and a path-absolute href ignores the document's
/// `<base href>` too — the leading slash replaces the base path. Under
/// ingress both effects send nav links to the bare origin path, where
/// Home Assistant (not this app) answers 404. Prepending the base keeps
/// the anchor inside the ingress prefix; the router strips the base
/// again when matching. No-op when the app is served at the root.
///
/// `path` must be a route path starting with `/` (before the base).
#[must_use]
pub fn route_href(path: &str) -> String {
    let base = ingress_base();
    if base.is_empty() {
        path.to_owned()
    } else {
        format!("{base}{path}")
    }
}

/// Module entry: mounts the app (called by `wasm-bindgen` on load).
#[wasm_bindgen(start)]
pub fn start_app() {
    theme::init_parent_theme_match();
    mount_to_body(App);
}

/// Empty strings become `None` (API nulls) — shared by the form pages.
pub(crate) fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

/// The shared inline error banner: renders while `error` holds a
/// message, clears from the tree when it is `None`. Retry reloads
/// the page in place — a bare `location.reload()`, no URL is built,
/// so the ingress prefix survives; dismiss clears the signal. Both
/// buttons are `min-h-11` targets. Lived in the equipment module
/// before the catalogue left (D078); every data page imports it
/// from the crate root now.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub(crate) fn ErrorIf(error: RwSignal<Option<String>>) -> impl IntoView {
    let i18n = use_i18n();
    move || {
        error.get().map(|message| {
            let retry_label = t_string!(i18n, errors.retry).into_owned();
            let dismiss_label = t_string!(i18n, errors.dismiss).into_owned();
            view! {
                <p
                    class="alert alert-error py-2 px-4 text-sm flex flex-wrap items-center gap-2"
                    role="alert"
                >
                    <span class="min-w-0 flex-1">{message}</span>
                    <span class="flex flex-wrap gap-2">
                        <button
                            class="btn btn-outline btn-sm min-h-11"
                            aria-label={retry_label}
                            on:click=move |_| {
                                if let Some(window) = leptos::web_sys::window() {
                                    let _ = window.location().reload();
                                }
                            }
                        >
                            {retry_label.clone()}
                        </button>
                        <button
                            class="btn btn-ghost btn-sm min-h-11"
                            aria-label={dismiss_label}
                            on:click=move |_| error.set(None)
                        >
                            {dismiss_label.clone()}
                        </button>
                    </span>
                </p>
            }
        })
    }
}

/// Maps a language tag from the locale chain to a shipped catalog:
/// `de…` → German, `en…` → English (an explicit English from Home
/// Assistant must beat a German browser locale, so `en*` maps too).
/// `None` for any other tag, which falls through the chain to the
/// browser locale or the English default.
#[must_use]
pub(crate) fn locale_from_tag(tag: &str) -> Option<Locale> {
    let lower = tag.to_ascii_lowercase();
    if lower.starts_with("de") {
        Some(Locale::de)
    } else if lower.starts_with("en") {
        Some(Locale::en)
    } else {
        None
    }
}

/// Maps one shipped [`Locale`] onto its decimal style (D123): German
/// writes comma decimals with dot grouping, English dot decimals with
/// comma grouping. The single mapping site — every formatter takes the
/// style from here instead of re-deciding per call site.
#[must_use]
pub(crate) const fn decimal_style(locale: Locale) -> leaflog_core::format::DecimalStyle {
    match locale {
        Locale::de => leaflog_core::format::DecimalStyle::Comma,
        Locale::en => leaflog_core::format::DecimalStyle::Dot,
    }
}

/// Applies the backend half of the locale chain (D112): the Home
/// Assistant user language (`GET api/locale`) wins over the browser
/// locale, which the i18n context has already resolved from the
/// navigator; English stays the default when neither maps to a
/// shipped catalog.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
fn LocaleChain() -> impl IntoView {
    let i18n = use_i18n();
    Effect::new(move |_| {
        spawn_local(async move {
            let reported = api::get::<api::LocaleReport>("api/locale")
                .await
                .ok()
                .and_then(|r| r.language);
            if let Some(locale) = reported.as_deref().and_then(locale_from_tag) {
                i18n.set_locale(locale);
            }
        });
    });
}

/// Root component: the i18n context (no cookie persistence) and the
/// locale-chain priming, then the router wired to the runtime ingress
/// base with the translated chrome.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn App() -> impl IntoView {
    view! {
        <I18nContextProvider enable_cookie=false>
            <LocaleChain />
            <Router base=ingress_base()>
                <AppShell />
            </Router>
        </I18nContextProvider>
    }
}

/// Header, navigation and route outlet; lives under the i18n provider
/// so the nav labels can translate.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
fn AppShell() -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div class="mx-auto max-w-6xl px-5">
            <header class="navbar min-h-0 w-full flex-wrap gap-x-2 px-1 py-3">
                <span class="text-2xl" aria-hidden="true">"🌿"</span>
                <h1 class="m-0 text-xl font-semibold tracking-wide">"Leaflog"</h1>
                <nav class="mainnav ml-auto flex flex-wrap items-center gap-4 text-sm"
                    aria-label={t_string!(i18n, nav.main).into_owned()}
                >
                    <A href=route_href("/") exact=true>{t!(i18n, nav.home)}</A>
                    <A href=route_href("/history")>{t!(i18n, nav.history)}</A>
                    <A href=route_href("/grows")>{t!(i18n, nav.grows)}</A>
                    <A href=route_href("/schedules")>{t!(i18n, nav.schedules)}</A>
                </nav>
            </header>
            <main id="main">
                {routes()}
            </main>
        </div>
    }
}

/// Home view: quick links and the backend health payload (fetched with
/// a *relative* URL, resolved through the injected `<base href>`). Now
/// the no-grows landing fallback too (ticket 11).
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn Home() -> impl IntoView {
    let i18n = use_i18n();
    // Status-first health card (D124): the flag drives the status
    // line; the raw payload — or the localised error line on a failed
    // fetch — stays in the collapsible details for honest diagnosis.
    let healthy = RwSignal::new(false);
    let details = RwSignal::new(String::from("…"));
    Effect::new(move |_| {
        spawn_local(async move {
            let (ok, payload) = match gloo_net::http::Request::get("api/health").send().await {
                Ok(resp) => match resp.text().await {
                    Ok(body) => (api::health_ok(&body), body),
                    Err(err) => (
                        false,
                        td_string!(
                            i18n.get_locale_untracked(),
                            page.health_read_error,
                            error = err.to_string()
                        ),
                    ),
                },
                Err(err) => (
                    false,
                    td_string!(
                        i18n.get_locale_untracked(),
                        page.health_network_error,
                        error = err.to_string()
                    ),
                ),
            };
            healthy.set(ok);
            details.set(payload);
        });
    });
    view! {
        <section class="pb-12">
            <OnboardingWizard />
            <div class={format!("{} my-4", ui::card())}>
                <h2>{t!(i18n, page.home_heading)}</h2>
                <p>
                    {t!(i18n, page.home_intro)}
                </p>
                <p class="text-base-content/70 text-sm">
                    {t!(i18n, page.home_privacy)}
                </p>
                <h3>{t!(i18n, page.health_heading)}</h3>
                <p class="flex items-center gap-2 text-sm" aria-live="polite">
                    <span
                        class=move || if healthy.get() {
                            "inline-block size-2.5 rounded-full bg-primary"
                        } else {
                            "inline-block size-2.5 rounded-full bg-error"
                        }
                        aria-hidden="true"
                    ></span>
                    {move || if healthy.get() {
                        t_string!(i18n, page.health_ok).into_owned()
                    } else {
                        t_string!(i18n, page.health_unreachable).into_owned()
                    }}
                </p>
                <details class="mt-2">
                    <summary class="cursor-pointer text-sm text-base-content/70">
                        {t!(i18n, page.health_details)}
                    </summary>
                    <pre class="mt-2 text-xs"><code>{move || details.get()}</code></pre>
                </details>
            </div>
        </section>
    }
}

/// The route table (D124): `#[cfg]` cannot sit on a view element, and
/// an interpolated `{expr}` child of `<Routes>` is forced through
/// `IntoRender` by the view macro, so a helper cannot inject a single
/// route tuple. The whole segment is therefore cfg-paired: debug
/// builds keep the M1 spike deep link; release builds drop the
/// segment, and `/spike` falls to the fallback.
#[cfg(debug_assertions)]
fn routes() -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <Routes fallback=move || view! { <p>{t!(i18n, page.loading)}</p> }>
            <Route path=path!("/") view=Landing />
            <Route path=path!("/spike") view=Spike />
            <Route path=path!("/history") view=HistoryPage />
            <Route path=path!("/plants/:id") view=PlantViewPage />
            <Route path=path!("/grows") view=GrowsPage />
            <Route path=path!("/grows/:id") view=GrowsRoute />
            <Route path=path!("/grows/:id/edit") view=GrowEditRoute />
            <Route path=path!("/schedules") view=SchedulesPage />
            <Route path=path!("/schedules/new") view=ScheduleNewRoute />
            <Route path=path!("/schedules/:id/edit") view=ScheduleEditRoute />
        </Routes>
    }
}

/// Release counterpart of [`routes`] (D124): no spike segment.
#[cfg(not(debug_assertions))]
fn routes() -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <Routes fallback=move || view! { <p>{t!(i18n, page.loading)}</p> }>
            <Route path=path!("/") view=Landing />
            <Route path=path!("/history") view=HistoryPage />
            <Route path=path!("/plants/:id") view=PlantViewPage />
            <Route path=path!("/grows") view=GrowsPage />
            <Route path=path!("/grows/:id") view=GrowsRoute />
            <Route path=path!("/grows/:id/edit") view=GrowEditRoute />
            <Route path=path!("/schedules") view=SchedulesPage />
            <Route path=path!("/schedules/new") view=ScheduleNewRoute />
            <Route path=path!("/schedules/:id/edit") view=ScheduleEditRoute />
        </Routes>
    }
}

/// Deep-link target proving router navigation and refreshes work under
/// the runtime base (kept from the M1 spike). Debug builds only (D124).
#[component]
#[cfg(debug_assertions)]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
fn Spike() -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <section class={format!("{} my-4", ui::card())}>
            <h2>{t!(i18n, page.spike_heading)}</h2>
            <p>{t!(i18n, page.spike_intro)}</p>
            <p><code>{ingress_base()}</code></p>
            <p><A href=route_href("/")>{t!(i18n, grows.back_home)}</A></p>
        </section>
    }
}

#[cfg(test)]
mod locale_chain_tests {
    use super::{Locale, locale_from_tag};

    #[test]
    fn german_tags_pick_the_german_catalog() {
        assert_eq!(locale_from_tag("de"), Some(Locale::de));
        assert_eq!(locale_from_tag("de-DE"), Some(Locale::de));
        assert_eq!(locale_from_tag("DE-at"), Some(Locale::de));
    }

    #[test]
    fn english_tags_pick_the_english_catalog() {
        // An explicit English from Home Assistant must beat a German
        // browser locale, so every en* tag maps.
        assert_eq!(locale_from_tag("en"), Some(Locale::en));
        assert_eq!(locale_from_tag("en-US"), Some(Locale::en));
    }

    #[test]
    fn unsupported_tags_fall_through_the_chain() {
        assert_eq!(locale_from_tag("fr-FR"), None);
        assert_eq!(locale_from_tag(""), None);
    }
}
