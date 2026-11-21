//! Support plumbing for the browser-run component tests (D139):
//! mounting views into a per-test container, stubbing `window.fetch`
//! with scripted API responses, waiting for async renders, and small
//! DOM helpers. All helpers are crate-private to the test tree.
//!
//! The fetch stub lives in a small JS snippet installed once per page
//! (`window.__testApi`): it records `{method, url, body}` for every
//! call and delegates the response to a Rust closure set per test.
//! Request bodies of `web_sys::Request` objects (how gloo-net sends
//! JSON POSTs) are drained through `new Response(request.body)`, so
//! POST payloads are visible to the tests.

use js_sys::{Array, Object, Reflect};
use leptos::mount::mount_to;
use leptos::prelude::*;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::HtmlElement;

/// The scripted-fetch installation JS (installed once per page).
/// Records every `{method, url, body}` and dispatches responses
/// newest-first across per-test matchers (the newest install shadows
/// stale same-URL matchers from concurrent tests); see the inline
/// comment for the contract.
const TEST_API_JS: &str = r#"
(() => {
  if (window.__testApi) { return; }
  const api = {
    calls: [],
    handler: null,
    // Tests run concurrently in the browser (wasm-bindgen-test runs
    // up to 8 at once), so every install keeps its own matcher and
    // fetch dispatches newest-first: a test's own fresh install wins
    // over any stale same-URL matcher. A matcher returns (404, "") as
    // its "not mine" catch-all, which passes through to the next
    // matcher; each test therefore always answers its own URLs while
    // anything unmatched still 404s.
    matchers: [],
    installed: false,
    orig: null,
    stubFn: null,
    install() {
      if (this.installed) { window.fetch = this.stubFn; return; }
      this.orig = window.fetch.bind(window);
      const self = this;
      this.stubFn = (input, init) => (async () => {
        let method = 'GET';
        let url = '';
        let body = '';
        if (typeof input === 'string' || input instanceof URL) {
          url = String(input);
          method = (init && init.method) || 'GET';
          body = (init && init.body) ? String(init.body) : '';
        } else {
          url = input.url;
          method = input.method || 'GET';
          if (input.body) {
            try { body = await new Response(input.body).text(); } catch { /* drain failure: keep empty */ }
          }
        }
        self.calls.push({ method, url, body });
        let res = null;
        for (const matcher of [...self.matchers].reverse()) {
          const handled = matcher(method, url, body);
          // (404, "") is the catch-all: "not mine", try the next.
          if (handled && !(Number(handled.status) === 404 && String(handled.body) === '')) {
            res = handled;
            break;
          }
        }
        const answer = res || { status: 404, body: '' };
        const status = answer.status == null ? 200 : Number(answer.status);
        // Null-body statuses (204/304) reject a body in the Response
        // constructor — construct them body-less.
        if (status === 204 || status === 304) {
          return new Response(null, { status });
        }
        return new Response(answer.body == null ? '' : String(answer.body), { status });
      })();
      window.fetch = this.stubFn;
      this.installed = true;
    },
    uninstall() {
      const index = this.matchers.indexOf(this.handler);
      if (index !== -1) { this.matchers.splice(index, 1); }
    },
    reset() {
      this.calls.length = 0;
    },
  };
  window.__testApi = api;
})();
"#;

/// Guards a scripted-fetch installation. While alive, `window.fetch`
/// answers with the handler's responses and records every call;
/// dropped ⇒ real fetch is restored and the script cleared.
/// The wrapped user handler; kept alive so the JS function reference
/// in the matcher chain stays valid. `Option` so `Drop` can read it
/// via `take()` (the field exists for its lifetime, not its value).
type Handler = Closure<dyn Fn(String, String, String) -> JsValue>;

pub struct FetchStub {
    handler: Option<Handler>,
}

impl FetchStub {
    /// Installs the stub with `handler(method, url, body) ->
    /// (status, body)`.
    pub(crate) fn new(handler: impl Fn(&str, &str, &str) -> (u16, String) + 'static) -> Self {
        eval(TEST_API_JS);
        let handler: Handler = Closure::new(move |method: String, url: String, body: String| {
            let (status, response) = handler(&method, &url, &body);
            let payload = Object::new();
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("status"),
                &JsValue::from_f64(f64::from(status)),
            );
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("body"),
                &JsValue::from_str(&response),
            );
            payload.into()
        });
        let function: &js_sys::Function = handler.as_ref().unchecked_ref();
        let api = get_global("__testApi");
        let _ = Reflect::set(
            &api,
            &JsValue::from_str("handler"),
            &JsValue::from(function.clone()),
        );
        // Tests run concurrently in the browser, so each install adds
        // its own matcher to the chain; see TEST_API_JS for dispatch.
        let stored =
            js_sys::Reflect::get(&api, &JsValue::from_str("handler")).expect("handler stored");
        prop(&api, "matchers")
            .unchecked_into::<js_sys::Array>()
            .push(&stored);
        // Install after the handler is in place so no call can slip
        // through with a half-configured script.
        let _ = prop(&api, "install")
            .unchecked_into::<js_sys::Function>()
            .call0(&api);
        Self {
            handler: Some(handler),
        }
    }
}

impl std::fmt::Debug for FetchStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FetchStub").finish_non_exhaustive()
    }
}

impl Drop for FetchStub {
    fn drop(&mut self) {
        let api = get_global("__testApi");
        // Uninstall first: it splices `handler` out of the matcher
        // chain, so the handler reference must still be in place.
        let _ = prop(&api, "uninstall")
            .unchecked_into::<js_sys::Function>()
            .call0(&api);
        let _ = Reflect::set(&api, &JsValue::from_str("handler"), &JsValue::null());
        // Taking the closure (a real read of the field) drops it
        // with self, invalidating the JS ref as the last step.
        self.handler.take();
    }
}

/// One recorded fetch: method, absolute URL, and the drained body
/// (empty for GETs and body-less requests).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub method: String,
    pub url: String,
    pub body: String,
}

/// Every call recorded since the stub was installed.
pub fn fetch_calls() -> Vec<Call> {
    let api = get_global("__testApi");
    let recorded = prop(&api, "calls").unchecked_into::<Array>();
    recorded
        .iter()
        .map(|entry| Call {
            method: string_field(&entry, "method"),
            url: string_field(&entry, "url"),
            body: string_field(&entry, "body"),
        })
        .collect()
}

/// Removes every recorded call (handler stays installed).
pub fn clear_calls() {
    let api = get_global("__testApi");
    let _ = prop(&api, "reset")
        .unchecked_into::<js_sys::Function>()
        .call0(&api);
}

/// Installs a minimal `window.echarts` mock: `init` appends an
/// `<svg role="img" aria-label="mock chart">` to the container so the
/// component's SVG-label assertions can find something, and the chart
/// object accepts `setOption`/`dispose`/`resize`/`group` no-ops.
/// Clears web storage and the recorded calls between tests. Does not
/// restore fetch — use [`FetchStub`] scoping for that.
pub fn fresh_environment() {
    eval(
        // Pin the browser language to English at the source:
        // leptos_i18n's provider init re-derives the locale from
        // `navigator.languages` in an effect that flushes after
        // mount and would clobber any `set_locale(en)` back to the
        // host language (de-DE here). Overriding the navigator
        // accessors makes every provider derive `en` on its own.
        "for (const key of ['language', 'languages']) {\n\
             Object.defineProperty(navigator, key, {\n\
                 configurable: true,\n\
                 get: () => (key === 'language' ? 'en' : ['en']),\n\
             });\n\
         }\n\
         window.__errs = [];\n\
         window.addEventListener('error', e => window.__errs.push(String(e.message || e)));\n\
         window.addEventListener('unhandledrejection', e => window.__errs.push('rej:' + String(e.reason)));\n\
         window.localStorage.clear(); window.sessionStorage.clear();",
    );
    if !get_global("__testApi").is_undefined() {
        clear_calls();
    }
}

/// A mounted view inside a per-test container appended to
/// `<body>`. Dropping removes the container and unmounts the view
/// (reactive effects and timers cleaned up by Leptos' owner drop).
pub struct Mounted<V: IntoView> {
    container: HtmlElement,
    _handle: leptos::mount::UnmountHandle<<V as Render>::State>,
}

impl<V: IntoView + 'static> Mounted<V> {
    /// Mounts the view built by `build` into a fresh container div.
    /// The closure form matters: `build` runs inside the mount owner,
    /// so context providers (`I18nContextProvider`) and effects created
    /// while the view builds find the scope they need. Passing a
    /// pre-built view would run all of that outside any owner.
    pub(crate) fn new<F>(build: F) -> Self
    where
        F: FnOnce() -> V + 'static,
    {
        // Mounting arms Leptos effects, which spawn onto the global
        // executor; the app initializes it in `start_app`, tests do it
        // here (second call is a harmless no-op Err).
        let _ = any_spawner::Executor::init_wasm_bindgen();
        let document = document();
        let container: HtmlElement = document
            .create_element("div")
            .expect("container element")
            .unchecked_into();
        let body = document.body().expect("document body");
        body.append_with_node_1(&container)
            .expect("append container");
        let handle = mount_to(container.clone(), build);
        Self {
            container,
            _handle: handle,
        }
    }

    /// Full rendered text content.
    #[must_use = "the assertion must use the text"]
    pub(crate) fn text(&self) -> String {
        self.container.text_content().unwrap_or_default()
    }

    /// Text content contains `needle`.
    pub(crate) fn contains(&self, needle: &str) -> bool {
        self.text().contains(needle)
    }

    /// First element matching `selector` inside the container.
    pub(crate) fn q(&self, selector: &str) -> Option<web_sys::Element> {
        self.container.query_selector(selector).ok().flatten()
    }

    /// All elements matching `selector` inside the container.
    pub(crate) fn q_all(&self, selector: &str) -> Vec<web_sys::Element> {
        self.container.query_selector_all(selector).map_or_else(
            |_| Vec::new(),
            |list| {
                let length = list.length();
                (0..length)
                    .filter_map(|i| list.item(i))
                    .map(wasm_bindgen::JsCast::unchecked_into::<web_sys::Element>)
                    .collect()
            },
        )
    }

    /// True when `selector` matches at least one element.
    pub(crate) fn has(&self, selector: &str) -> bool {
        self.q(selector).is_some()
    }
}

impl<V: IntoView> std::fmt::Debug for Mounted<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mounted").finish_non_exhaustive()
    }
}

impl<V: IntoView> Drop for Mounted<V> {
    fn drop(&mut self) {
        let () = self.container.remove();
    }
}

/// Wraps a view in the i18n provider (browser locale resolves to the
/// English catalog in headless Chrome).
/// Wraps children in the i18n context (en locale). The children
/// closure matters: it must run INSIDE the provider so `use_i18n`
/// finds the context — a pre-built view would run the component
/// bodies before the provider provides anything.
pub fn with_i18n<N>(children: impl FnOnce() -> N + Send + 'static) -> impl IntoView
where
    N: IntoView + 'static,
    N::AsyncOutput: Send,
{
    view! {
        <crate::i18n::I18nContextProvider children=leptos::children::ToChildren::to_children(move || {
            // Pin the locale: headless Chrome inherits the host
            // language (de-DE here), while assertions key on en strings.
            crate::i18n::use_i18n().set_locale(crate::i18n::Locale::en);
            children()
        }) />
    }
}

/// Wraps children in the router (components using `<A>` or
/// navigation). Same laziness rule as [`with_i18n`].
pub fn with_router<N>(children: impl FnOnce() -> N + Send + 'static) -> impl IntoView
where
    N: IntoView + 'static,
{
    view! {
        <leptos_router::components::Router children=leptos::children::ToChildren::to_children(
            children,
        ) />
    }
}

/// Polls `pred` until true (25 ms interval, ~5 s cap).
pub async fn wait_for(mut pred: impl FnMut() -> bool) {
    assert!(
        try_wait_for(&mut pred).await,
        "condition not met within ~5s"
    );
}

/// [`wait_for`] without the panic: returns whether the predicate
/// was met, so a caller can panic with diagnostic context.
pub async fn try_wait_for(pred: &mut impl FnMut() -> bool) -> bool {
    for _ in 0..200 {
        if pred() {
            return true;
        }
        gloo_timers::future::TimeoutFuture::new(25).await;
    }
    false
}

/// Waits until the container's text contains `needle`.
pub async fn wait_text<V: IntoView + 'static>(mount: &Mounted<V>, needle: &'static str) {
    wait_for(|| mount.contains(needle)).await;
}

/// Dispatches a bubbling `click` on the element.
pub fn click(el: &web_sys::Element) {
    let init = web_sys::MouseEventInit::new();
    init.set_bubbles(true);
    init.set_cancelable(true);
    let event =
        web_sys::MouseEvent::new_with_mouse_event_init_dict("click", &init).expect("click event");
    let _ = el.dispatch_event(event.as_ref());
}

/// Dispatches a bubbling `input` event (Leptos binds follow these).
pub fn fire_input(el: &web_sys::Element) {
    let init = web_sys::EventInit::new();
    init.set_bubbles(true);
    let event = web_sys::Event::new_with_event_init_dict("input", &init).expect("input event");
    let _ = el.dispatch_event(&event);
}

/// Dispatches a bubbling `change` event (selects and file inputs).
pub fn fire_change(el: &web_sys::Element) {
    let init = web_sys::EventInit::new();
    init.set_bubbles(true);
    let event = web_sys::Event::new_with_event_init_dict("change", &init).expect("change event");
    let _ = el.dispatch_event(&event);
}

/// Types `value` into an `<input>` and fires the input event.
pub fn type_input(el: &web_sys::Element, value: &str) {
    let input: web_sys::HtmlInputElement = el.clone().unchecked_into();
    input.set_value(value);
    fire_input(el);
}

/// Returns the element's `class` attribute.
pub fn classes(el: &web_sys::Element) -> String {
    el.get_attribute("class").unwrap_or_default()
}

/// Reads a localStorage entry as a string (empty when absent).
pub fn local_storage(key: &str) -> String {
    let window = web_sys::window().expect("window");
    window
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.get_item(key).ok())
        .flatten()
        .unwrap_or_default()
}

/// Overwrites a localStorage entry.
/// Evaluates a JS snippet, ignoring its result.
pub fn eval(source: &str) {
    js_sys::eval(source).ok();
}

/// The shared test-script object (`window.__testApi`).
fn get_global(name: &str) -> JsValue {
    Reflect::get(&global_api(), &JsValue::from_str(name)).expect("global present")
}

/// Reads a property off a JS object, failing the test when absent.
fn prop(obj: &JsValue, name: &str) -> JsValue {
    Reflect::get(obj, &JsValue::from_str(name)).expect("property present")
}

/// The JS global object.
fn global_api() -> JsValue {
    js_sys::global().into()
}

/// The current document.
fn document() -> web_sys::Document {
    web_sys::window()
        .expect("window")
        .document()
        .expect("document")
}

/// Reads a string property off a JS object.
fn string_field(value: &JsValue, field: &str) -> String {
    Reflect::get(value, &JsValue::from_str(field))
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default()
}
