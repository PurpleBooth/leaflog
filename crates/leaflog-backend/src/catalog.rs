//! The backend message catalog (D112): a hand-rolled string catalog
//! keyed by locale and message.
//!
//! It covers the environment-alert notification templates and the four
//! standard error wrapper phrases (not found, bad request, conflict,
//! internal error). The catalog is an exhaustive `match` over
//! `(locale, message)` with `format!` interpolation — no runtime
//! fetches, no new dependencies. EN↔DE key parity is compile-time: a
//! message missing in one locale is a non-exhaustive-match build
//! error, the backend analogue of the frontend catalogs' parity panic.
//!
//! The locale is the Home Assistant config `language` (D112) with
//! English fallback for unsupported or absent tags. Two readers: the
//! alert path reads the bridge's cached language, and the API error
//! wrappers — whose response conversion has no state access — read
//! the process-global language published by the same config fetch
//! (one HA instance per process, so the language is app-wide; D113).
//! Deep interpolated detail text (store errors, alert kinds, grow
//! names) stays English by decision: the templates interpolate it
//! as-is.

use std::sync::{PoisonError, RwLock};

/// The locales the catalog speaks. `En` is the default and the
/// fallback anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    /// English — the default and the fallback.
    En,
    /// German (du-form, Home Assistant register).
    De,
}

impl Locale {
    /// Resolves a BCP-47 language tag (the HA config `language`
    /// field): the `de` primary subtag selects German, everything
    /// else — including `None` — falls back to English.
    #[must_use]
    pub fn from_language(language: Option<&str>) -> Self {
        let german = language
            .and_then(|tag| tag.split('-').next())
            .is_some_and(|primary| primary.eq_ignore_ascii_case("de"));
        if german { Self::De } else { Self::En }
    }
}

/// One catalog message. The English detail payloads (alert kinds,
/// grow names, store errors) ride inside the message and stay English
/// by decision (D112).
#[derive(Debug, Clone, Copy)]
pub enum Message<'a> {
    /// The standard 404 wrapper.
    NotFound,
    /// The standard 400 wrapper; the detail stays English.
    BadRequest {
        /// English detail text.
        detail: &'a str,
    },
    /// The standard 409 wrapper; the detail stays English.
    Conflict {
        /// English detail text.
        detail: &'a str,
    },
    /// The standard 500 wrapper; the detail stays English.
    Internal {
        /// English detail text.
        detail: &'a str,
    },
    /// The environment-alert notification title.
    EnvAlertTitle,
    /// The environment-alert body; the alert kind and the grow name
    /// stay English.
    EnvAlertBody {
        /// Alert kind ("temperature" or "vpd").
        kind: &'a str,
        /// Grow name.
        grow_name: &'a str,
    },
}

/// The published HA config language, set by the bridge's config fetch
/// ([`crate::ha`] `refresh_config`) and read by the API error
/// wrappers. One HA instance per process, so the language is
/// app-wide.
static PUBLISHED_LANGUAGE: RwLock<Option<String>> = RwLock::new(None);

/// Publishes the language from the latest successful config fetch.
/// Called by the bridge next to the language watch channel; failures
/// keep the previous value.
pub fn publish_language(language: Option<&str>) {
    let mut slot = PUBLISHED_LANGUAGE
        .write()
        .unwrap_or_else(PoisonError::into_inner);
    *slot = language.map(str::to_owned);
}

/// The locale for the surfaces without state access (the API error
/// wrappers): the published HA language with English fallback.
#[must_use]
pub fn current_locale() -> Locale {
    let snapshot = PUBLISHED_LANGUAGE
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    Locale::from_language(snapshot.as_deref())
}

/// Renders a message in the locale: the template map is the
/// exhaustive `(locale, message)` match, and `format!` interpolates
/// the English details.
#[must_use]
pub fn render(message: Message<'_>, locale: Locale) -> String {
    match (locale, message) {
        (Locale::En, Message::NotFound) => "not found".to_owned(),
        (Locale::En, Message::BadRequest { detail }) => format!("bad request: {detail}"),
        (Locale::En, Message::Conflict { detail }) => format!("conflict: {detail}"),
        (Locale::En, Message::Internal { detail }) => format!("internal error: {detail}"),
        (Locale::En, Message::EnvAlertTitle) => "Leaflog environment".to_owned(),
        (Locale::En, Message::EnvAlertBody { kind, grow_name }) => {
            format!("Leaflog: {kind} out of band in \"{grow_name}\" — check the panel.")
        }
        (Locale::De, Message::NotFound) => "Nicht gefunden".to_owned(),
        (Locale::De, Message::BadRequest { detail }) => format!("Ungültige Anfrage: {detail}"),
        (Locale::De, Message::Conflict { detail }) => format!("Konflikt: {detail}"),
        (Locale::De, Message::Internal { detail }) => format!("Interner Fehler: {detail}"),
        (Locale::De, Message::EnvAlertTitle) => "Leaflog-Umgebung".to_owned(),
        (Locale::De, Message::EnvAlertBody { kind, grow_name }) => {
            format!(
                "Leaflog: {kind} liegt außerhalb des Sollbereichs in „{grow_name}“ — schau im Panel nach."
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Locale, Message, render};

    #[test]
    fn locale_resolution_maps_the_de_prefix_and_falls_back_to_english() {
        assert_eq!(Locale::from_language(Some("de")), Locale::De);
        assert_eq!(Locale::from_language(Some("de-DE")), Locale::De);
        assert_eq!(Locale::from_language(Some("de-AT")), Locale::De);
        assert_eq!(Locale::from_language(Some("en")), Locale::En);
        assert_eq!(Locale::from_language(Some("en-US")), Locale::En);
        assert_eq!(Locale::from_language(Some("fr-FR")), Locale::En);
        assert_eq!(Locale::from_language(None), Locale::En);
        assert_eq!(Locale::from_language(Some("")), Locale::En);
    }

    #[test]
    fn wrappers_render_per_locale_with_english_details() {
        assert_eq!(render(Message::NotFound, Locale::En), "not found");
        assert_eq!(render(Message::NotFound, Locale::De), "Nicht gefunden");
        assert_eq!(
            render(Message::BadRequest { detail: "hour 30" }, Locale::De),
            "Ungültige Anfrage: hour 30"
        );
        assert_eq!(
            render(Message::BadRequest { detail: "hour 30" }, Locale::En),
            "bad request: hour 30"
        );
        assert_eq!(
            render(
                Message::Conflict {
                    detail: "bundled record"
                },
                Locale::De
            ),
            "Konflikt: bundled record"
        );
        assert_eq!(
            render(
                Message::Internal {
                    detail: "sqlite busy"
                },
                Locale::De
            ),
            "Interner Fehler: sqlite busy"
        );
    }

    #[test]
    fn alert_templates_render_du_form_and_interpolate_english_details() {
        assert_eq!(
            render(Message::EnvAlertTitle, Locale::En),
            "Leaflog environment"
        );
        assert_eq!(
            render(Message::EnvAlertTitle, Locale::De),
            "Leaflog-Umgebung"
        );
        assert_eq!(
            render(
                Message::EnvAlertBody {
                    kind: "temperature",
                    grow_name: "Hot run",
                },
                Locale::En
            ),
            "Leaflog: temperature out of band in \"Hot run\" — check the panel."
        );
        let german = render(
            Message::EnvAlertBody {
                kind: "vpd",
                grow_name: "Sommer",
            },
            Locale::De,
        );
        assert!(
            german.contains("außerhalb des Sollbereichs"),
            "du-form body, got {german:?}"
        );
        assert!(german.contains("vpd"), "kind stays English, got {german:?}");
        assert!(german.contains("Sommer"), "grow name interpolated");
    }
}
