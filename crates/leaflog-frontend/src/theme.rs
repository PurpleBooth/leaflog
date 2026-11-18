//! Parent Home Assistant theme adoption (ticket 07, D100/D101).
//!
//! Inside HA's ingress iframe the app reads the parent frontend's
//! theme variables off the parent's root element, maps them onto the
//! daisyUI semantic variables (the mapping itself is
//! [`leaflog_core::theme::adopt`]), and re-applies them whenever the
//! parent theme changes. Every step is fallible: a missing parent, a
//! cross-origin parent, or an incomplete variable set silently leaves
//! the built-in theme in place — the fallback is mandatory, not a
//! user-visible state.

use js_sys::Reflect;
use leaflog_core::theme::{self, HaTheme};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

/// Every CSS property [`apply_adopted_theme`] may set on the app root,
/// so a later fallback can clear exactly these.
const APPLIED_PROPERTIES: &[&str] = &[
    "color-scheme",
    "--color-primary",
    "--color-primary-content",
    "--color-accent",
    "--color-accent-content",
    "--color-base-100",
    "--color-base-200",
    "--color-base-300",
    "--color-base-content",
    "--color-success",
    "--color-success-content",
    "--color-warning",
    "--color-warning-content",
    "--color-error",
    "--color-error-content",
    "--color-info",
    "--color-info-content",
];

/// Entry point (called once from `start_app`): applies once, then
/// watches the parent root for theme changes.
pub fn init_parent_theme_match() {
    apply_current();

    // The observer needs a readable parent; standalone stays built-in.
    let (Some(window), Some(parent)) = (web_sys::window(), parent_window()) else {
        return;
    };
    if parent == window {
        return;
    }
    let Some(root) = parent_root_element(&parent) else {
        return;
    };

    let callback = Closure::<dyn FnMut()>::new(apply_current);
    let handler = js_sys::Function::from(callback.as_js_value().clone());
    let Ok(observer) = web_sys::MutationObserver::new(&handler) else {
        return;
    };
    let init = web_sys::MutationObserverInit::new();
    init.set_attributes(true);
    init.set_attribute_filter(&JsValue::from_str("style"));
    if observer.observe_with_options(&root, &init).is_err() {
        return;
    }
    // One observer per page load; the pair must outlive this call.
    std::mem::forget((observer, callback));
}

/// Reads the parent theme and applies — or clears — the overrides.
fn apply_current() {
    let adopted = read_parent_theme().as_ref().and_then(theme::adopt);
    apply_adopted_theme(adopted.as_ref());
}

/// The parent `Window` unless the app is the top document.
fn parent_window() -> Option<web_sys::Window> {
    web_sys::window()?.parent().ok().flatten()
}

/// The parent document's root element, or `None` when the parent is
/// cross-origin (sandbox tightening or a foreign host page): the
/// `document` access throws there, which `Reflect::get` reports as
/// `Err` instead of a trap.
fn parent_root_element(parent: &web_sys::Window) -> Option<web_sys::Element> {
    let document = Reflect::get(parent.as_ref(), &JsValue::from_str("document")).ok()?;
    let document = document.dyn_into::<web_sys::Document>().ok()?;
    document.document_element()
}

/// Reads the HA theme variables off the parent root's computed style.
///
/// Like HA itself (`applyThemesOnElement` in
/// `src/common/dom/apply_themes_on_element.ts`), the live theme lives
/// as inline custom properties on the parent `<html>`, so both the
/// initial read and the style mutation watch target that element.
fn read_parent_theme() -> Option<HaTheme> {
    let parent = parent_window()?;
    let window = web_sys::window()?;
    if parent == window {
        return None;
    }
    let root = parent_root_element(&parent)?;
    let style = parent.get_computed_style(&root).ok()??;
    Some(HaTheme::from_values(|name| {
        let value = style.get_property_value(name).ok()?;
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    }))
}

/// Writes (or, with `None`, removes) the daisyUI overrides on this
/// document's root element: `body` sits outside the app mount div, so
/// the root is the only inline override point that covers everything.
fn apply_adopted_theme(adopted: Option<&theme::AdoptedTheme>) {
    let Some(window) = web_sys::window() else { return };
    let Some(document) = window.document() else { return };
    let Some(root) = document.document_element() else { return };
    let Ok(root) = root.dyn_into::<web_sys::HtmlHtmlElement>() else {
        return;
    };
    let style = root.style();
    for property in APPLIED_PROPERTIES {
        let _ = style.remove_property(property);
    }
    if let Some(adopted) = adopted {
        let _ = style.set_property("color-scheme", adopted.color_scheme());
        for (name, value) in adopted.entries() {
            let _ = style.set_property(name, value);
        }
    }
}
