//! Ingress base-path handling.
//!
//! Home Assistant serves the app under `/api/hassio_ingress/<token>/`
//! and marks every proxied request with an `X-Ingress-Path` header. The
//! header value is the public prefix under which the browser sees the
//! app. This module sanitizes that value and injects it into the SPA's
//! `index.html` at serve time (never at build time), as a `<base href>`
//! for the browser and a `window.__LEAFLOG_BASE__` global for the Leptos
//! router.

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
        r#"<base href="{}"><script>window.__LEAFLOG_BASE__={};</script>"#,
        escape_html(&base.href()),
        escape_js_string(base.as_str())
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
}
