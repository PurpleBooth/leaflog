//! Ingress base-path handling.
//!
//! Home Assistant serves the app under `/api/hassio_ingress/<token>/`
//! and marks every proxied request with an `X-Ingress-Path` header. The
//! header value is the public prefix under which the browser sees the
//! app. This module sanitizes that value and injects it into the SPA's
//! `index.html` at serve time (never at build time), as a `<base href>`
//! for the browser and a `window.__LEAFLOG_BASE__` global for the Leptos
//! router. It also builds the Content-Security-Policy that allows
//! exactly the injected script and the template's own inline scripts
//! (each by hash) and nothing else inline.

/// The sanitized ingress base path of a request.
///
/// The stored string is either empty (app served at the root) or an
/// absolute path without a trailing slash, such as
/// `/api/hassio_ingress/abc123`. Only ASCII alphanumerics, `-`, `_`,
/// `.`, and `/` are accepted; anything else fails safe to the root so a
/// hostile header can never inject markup or traversal.
#[allow(clippy::module_name_repetitions)] // domain name wins over the lint; see docs/DECISIONS.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngressPath(String);

impl IngressPath {
    /// Sanitizes a raw `X-Ingress-Path` header value (`None` or empty
    /// means the app is served at the root).
    #[must_use]
    pub fn new(raw: Option<&str>) -> Self {
        let Some(raw) = raw.map(str::trim) else {
            return Self(String::new());
        };
        let trimmed = trimmed_path(raw);
        if !is_safe_path(&trimmed) {
            return Self(String::new());
        }
        Self(trimmed)
    }

    /// The base path without a trailing slash; empty when at the root.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The value for a `<base href>` attribute: the base path with a
    /// trailing slash (`/` when at the root).
    #[must_use]
    pub fn href(&self) -> String {
        if self.0.is_empty() {
            "/".to_owned()
        } else {
            format!("{}/", self.0)
        }
    }
}

/// Strips trailing slashes and collapses leading whitespace-safe input.
fn trimmed_path(raw: &str) -> String {
    let mut out = raw.trim().trim_end_matches('/').to_owned();
    if out == "/" {
        out.clear();
    }
    out
}

/// Accepts only `/`-rooted paths of `[A-Za-z0-9._-]` segments.
fn is_safe_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.is_empty()
        && path[1..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
        && path.split('/').all(|seg| seg != ".." && seg != ".")
}

/// Renders the served `index.html` by injecting the runtime base.
///
/// Any `<base>` tag baked in at build time is stripped first (the build
/// uses a relative public URL, so this is belt and braces), then a
/// `<base href>` and a `window.__LEAFLOG_BASE__` global are injected
/// right after `<head>`.
#[must_use]
pub fn render_index(template: &str, base: &IngressPath) -> String {
    let stripped = strip_base_tags(template);
    let head = format!(
        r#"<base href="{}">{}"#,
        escape_html(&base.href()),
        base_script_tag(base)
    );
    stripped.find("<head>").map_or_else(
        || format!("{head}{stripped}"),
        |at| {
            format!(
                "{}{head}{}",
                &stripped[..at + "<head>".len()],
                &stripped[at + "<head>".len()..]
            )
        },
    )
}

/// The exact inline script source [`render_index`] injects. One source
/// of truth: [`content_security_policy`] hashes *this* string, so the
/// header and the HTML can never drift apart.
fn base_script_source(base: &IngressPath) -> String {
    format!(
        "window.__LEAFLOG_BASE__={};",
        escape_js_string(base.as_str())
    )
}

/// The `<script>` element carrying [`base_script_source`].
fn base_script_tag(base: &IngressPath) -> String {
    format!("<script>{}</script>", base_script_source(base))
}

/// Content-Security-Policy header value for the served SPA shell.
///
/// The app lives on Home Assistant's origin, where Leaflog XSS would be
/// HA XSS (docs/THREAT-MODEL.md §2), so the shell bounds what any
/// injected markup can do. Every directive allows exactly what the
/// Trunk-built app and the ingress injection load:
///
/// - `script-src 'self'` — the Trunk bundle (JS and wasm fetches are
///   same-origin); `'wasm-unsafe-eval'` — compiling the Leptos wasm
///   module, which Chromium gates on script-src; `'sha256-…'` hashes —
///   one for the inline script [`render_index`] injects, plus one for
///   every inline `<script>` in `template`. Trunk (0.21.x) emits its
///   wasm-bindgen bootstrap *inline* into `index.html`, and its text
///   names per-build hashed files, so no baked-in hash can cover it —
///   the hashes are computed from the actual template at serve time,
///   which stays correct on every rebuild. There is deliberately
///   **no** `'unsafe-inline'`: the hashes pin our scripts and only
///   ours.
/// - `style-src 'self' 'unsafe-inline'` — the linked stylesheet plus
///   the inline styles Leptos emits (element `style` attributes and
///   dynamic `<style>` elements).
/// - `img-src`/`connect-src`/`form-action`/`base-uri` `'self'` —
///   photos, API polling, and the injected `<base href>` are all
///   same-origin relative URLs (AGENTS.md ingress rules).
/// - `object-src 'none'`, `default-src 'self'`.
///
/// No `frame-ancestors` on purpose: HA's ingress panel frames the app
/// from the HA page, and remote-UI proxy setups must not be bricked by
/// guesswork; see docs/DECISIONS.md D040.
#[must_use]
pub fn content_security_policy(base: &IngressPath, template: &str) -> String {
    use base64::Engine as _;
    use sha2::Digest as _;

    let engine = base64::engine::general_purpose::STANDARD;
    let mut hashes = vec![format!(
        "'sha256-{}'",
        engine.encode(sha2::Sha256::digest(base_script_source(base)))
    )];
    hashes.extend(
        inline_script_sources(template)
            .into_iter()
            .map(|source| format!("'sha256-{}'", engine.encode(sha2::Sha256::digest(source)))),
    );
    format!(
        "default-src 'self'; \
         script-src 'self' 'wasm-unsafe-eval' {}; \
         style-src 'self' 'unsafe-inline'; \
         img-src 'self'; \
         connect-src 'self'; \
         object-src 'none'; \
         base-uri 'self'; \
         form-action 'self'",
        hashes.join(" ")
    )
}

/// The exact contents of every inline `<script>` element in `html`
/// (one with no `src` attribute): the bytes between the tags, with no
/// whitespace normalization — browsers hash inline scripts byte for
/// byte (verified in Chromium 152: a padded script whose *trimmed*
/// hash is listed stays blocked; MDN documents the same). Hashing the
/// raw content is therefore the only rule that boots the app.
///
/// The template is our own Trunk output (or the test fixture shaped
/// like it), never user input, so a simple scanner is honest here; a
/// script body ends at the first `</script`, which is also where the
/// HTML parser ends it. Anything the scanner might miss simply stays
/// blocked by the CSP — the failure mode is closed.
fn inline_script_sources(html: &str) -> Vec<&str> {
    let mut sources = Vec::new();
    let mut rest = html;
    while let Some(tag_at) = rest.find("<script") {
        let after_open = &rest[tag_at + "<script".len()..];
        let Some(open_end) = after_open.find('>') else {
            break;
        };
        let attributes = &after_open[..open_end];
        let body = &after_open[open_end + 1..];
        let Some(close_at) = body.find("</script") else {
            break;
        };
        if !has_src_attribute(attributes) {
            sources.push(&body[..close_at]);
        }
        let after_close = &body[close_at..];
        let Some(tag_end) = after_close.find('>') else {
            break;
        };
        rest = &after_close[tag_end + 1..];
    }
    sources
}

/// True when a `<script …>` attribute list carries a `src` attribute
/// (making the element external, covered by `'self'`, not a hash).
fn has_src_attribute(attributes: &str) -> bool {
    attributes.split_ascii_whitespace().any(|token| {
        let name = token.split('=').next().unwrap_or_default();
        name.eq_ignore_ascii_case("src")
    })
}

/// Removes every `<base ...>` tag (build-time leftovers).
fn strip_base_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<base") {
        out.push_str(&rest[..start]);
        match rest[start..].find('>') {
            Some(end) => rest = &rest[start + end + 1..],
            None => {
                // Unterminated tag: drop the remainder rather than emit it.
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Escapes the five XML special characters for attribute/element text.
fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// Produces a double-quoted JavaScript string literal.
fn escape_js_string(value: &str) -> String {
    use std::fmt::Write as _;

    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::{IngressPath, escape_html, escape_js_string, is_safe_path};

    #[test]
    fn sanitizes_paths() {
        assert_eq!(IngressPath::new(None).as_str(), "");
        assert_eq!(IngressPath::new(Some("")).as_str(), "");
        assert_eq!(IngressPath::new(Some("/")).as_str(), "");
        assert_eq!(
            IngressPath::new(Some("/api/hassio_ingress/abc/")).as_str(),
            "/api/hassio_ingress/abc"
        );
    }

    #[test]
    fn rejects_unsafe_paths() {
        assert_eq!(IngressPath::new(Some("api/ingress")).as_str(), "");
        assert_eq!(IngressPath::new(Some("/a/../b")).as_str(), "");
        assert_eq!(IngressPath::new(Some("/a\"><script>")).as_str(), "");
        assert_eq!(IngressPath::new(Some("/a b")).as_str(), "");
    }

    #[test]
    fn safe_path_rules() {
        assert!(is_safe_path("/api/hassio_ingress/Xyz-123_.4"));
        assert!(!is_safe_path("/api//../etc"));
        assert!(!is_safe_path("relative"));
    }

    #[test]
    fn href_has_trailing_slash() {
        assert_eq!(IngressPath::new(None).href(), "/");
        assert_eq!(
            IngressPath::new(Some("/api/hassio_ingress/x")).href(),
            "/api/hassio_ingress/x/"
        );
    }

    #[test]
    fn injects_into_head_and_strips_baked_base() {
        let template = r#"<!DOCTYPE html><html><head><title>t</title><base href="./"></head><body></body></html>"#;
        let base = IngressPath::new(Some("/api/hassio_ingress/tok"));
        let out = super::render_index(template, &base);
        assert!(out.contains(r#"<base href="/api/hassio_ingress/tok/">"#));
        assert!(out.contains(r#"window.__LEAFLOG_BASE__="/api/hassio_ingress/tok";"#));
        assert!(
            !out.contains(r#"<base href="./">"#),
            "baked base must be stripped"
        );
        assert!(!out.contains(r#"href="./">"#));
        assert_eq!(out.matches("<base").count(), 1);
    }

    #[test]
    fn root_base_injects_empty_string_global() {
        let template = "<html><head></head><body></body></html>";
        let out = super::render_index(template, &IngressPath::new(None));
        assert!(out.contains(r#"<base href="/">"#));
        assert!(out.contains("window.__LEAFLOG_BASE__=\"\";"));
    }

    #[test]
    fn escaping_helpers() {
        assert_eq!(escape_html(r#"<a&"b">"#), "&lt;a&amp;&quot;b&quot;&gt;");
        assert_eq!(escape_js_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }

    #[test]
    fn csp_hash_matches_the_script_rendered_into_the_page() {
        use base64::Engine as _;
        use sha2::Digest as _;

        for raw in [None, Some("/api/hassio_ingress/tok")] {
            let base = IngressPath::new(raw);
            let html = super::render_index("<html><head></head><body></body></html>", &base);
            let start = html.find("<script>").expect("script tag injected") + "<script>".len();
            let end = start + html[start..].find("</script>").expect("script tag closed");
            let source = &html[start..end];
            let hash = base64::engine::general_purpose::STANDARD
                .encode(sha2::Sha256::digest(source.as_bytes()));
            assert!(
                super::content_security_policy(&base, "").contains(&format!("'sha256-{hash}'")),
                "CSP must allow exactly the injected script {source:?}"
            );
        }
    }

    /// The inline wasm-bindgen bootstrap Trunk 0.21.x emits into
    /// `index.html`: an inline `<script type="module">` (no `src`)
    /// whose text names per-build hashed files.
    const TRUNK_BOOTSTRAP_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <title>fixture</title>
    <link rel="stylesheet" href="./styles-56560df6175930ac.css" />
    <script type="module">
import init, * as bindings from './leaflog-frontend-83eff7e5ef5e945c.js';
const wasm = await init({ module_or_path: './leaflog-frontend-83eff7e5ef5e945c_bg.wasm' });


window.wasmBindings = bindings;


dispatchEvent(new CustomEvent("TrunkApplicationStarted", {detail: {wasm}}));

</script>
  <link rel="modulepreload" href="./leaflog-frontend-83eff7e5ef5e945c.js"></head>
  <body><div id="app"></div></body>
</html>"#;

    #[test]
    fn csp_allows_the_trunk_bootstrap_by_hash() {
        use base64::Engine as _;
        use sha2::Digest as _;

        // One hash per inline script in the template: the base script
        // (root base) plus the bootstrap. Browsers hash the EXACT bytes
        // between the tags (Chromium 152 verified: a padded script
        // whose trimmed hash is listed stays blocked), so the policy
        // must carry the raw-content hash — not a trimmed one.
        let bootstrap = super::inline_script_sources(TRUNK_BOOTSTRAP_TEMPLATE);
        assert_eq!(bootstrap.len(), 1, "only the bootstrap is inline");
        let policy =
            super::content_security_policy(&IngressPath::new(None), TRUNK_BOOTSTRAP_TEMPLATE);
        let engine = base64::engine::general_purpose::STANDARD;
        let raw = engine.encode(sha2::Sha256::digest(
            TRUNK_BOOTSTRAP_TEMPLATE
                .split_once("<script type=\"module\">")
                .and_then(|(_, after)| after.split_once("</script>"))
                .map_or("", |(body, _)| body)
                .as_bytes(),
        ));
        let trimmed = engine.encode(sha2::Sha256::digest(bootstrap[0].trim().as_bytes()));
        assert_ne!(trimmed, raw, "fixture must exercise the exact-bytes rule");
        assert!(
            policy.contains(&format!("'sha256-{raw}'")),
            "CSP must allow the Trunk bootstrap by its exact-content hash: {policy}"
        );
        assert!(!policy.contains(&format!("'sha256-{trimmed}'")));
        // The external references stay covered by 'self', not a hash.
        assert!(policy.contains("script-src 'self' 'wasm-unsafe-eval' 'sha256-"));
        assert!(!policy.contains("unsafe-inline' 'sha256"));
        // And the base-script hash is still there alongside it.
        assert!(policy.contains("'sha256-AEesw44Mnq5qKta2TJ7d7OT2cUsXlnb0xxvKbIPhr+8='"));
    }

    #[test]
    fn inline_script_extraction_rules() {
        // External scripts (src) are excluded; inline contents are
        // returned byte-exact (no trimming); several scripts are all
        // collected.
        let html = "<script src=\"./a.js\"></script>\
                    <script>  one();  </script>\
                    <script type=\"module\">\ntwo();\n</script>";
        assert_eq!(
            super::inline_script_sources(html),
            vec!["  one();  ", "\ntwo();\n"],
            "inline contents byte-exact, src scripts skipped"
        );
        // Unterminated markup yields nothing rather than panicking.
        assert_eq!(
            super::inline_script_sources("<script>never closed"),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn csp_for_the_root_base_is_pinned() {
        // The first hash is the SHA-256 of `window.__LEAFLOG_BASE__="";`
        // — the script render_index injects for a root base; with no
        // inline scripts in the template it is the only one. Pinning
        // the whole policy keeps every directive a deliberate choice.
        assert_eq!(
            super::content_security_policy(&IngressPath::new(None), ""),
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval' \
             'sha256-AEesw44Mnq5qKta2TJ7d7OT2cUsXlnb0xxvKbIPhr+8='; \
             style-src 'self' 'unsafe-inline'; img-src 'self'; \
             connect-src 'self'; object-src 'none'; base-uri 'self'; \
             form-action 'self'"
        );
    }

    #[test]
    fn csp_hash_follows_the_ingress_base() {
        // SHA-256 of `window.__LEAFLOG_BASE__="/api/hassio_ingress/tok";`.
        assert!(
            super::content_security_policy(&IngressPath::new(Some("/api/hassio_ingress/tok")), "")
                .contains("'sha256-GVL7MyGmf7Vwea0rhsVonqMNYfP8zoFFd2Zlp05r2i4='")
        );
    }
}
