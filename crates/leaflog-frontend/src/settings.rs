//! The settings screen (SPEC §6's language override and the other
//! backend settings that had no control, D154). Loaded once, edited
//! in place, saved as one body. Unset keys fall back to the backend
//! defaults shown in the hints.

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde_json::{Value, json};

use crate::api;
use crate::i18n::{t, t_string, use_i18n};
use crate::non_empty;
use crate::ui;

/// One mebibyte, for the photo-size warning (stored in bytes).
const MIB: f64 = 1024.0 * 1024.0;

/// What the save button and the status line show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SaveStatus {
    Idle,
    Saving,
    Saved,
    Failed,
}

/// The form state. Unset numbers and empty strings mean "follow the
/// backend default"; the booleans carry that default.
#[derive(Debug, Clone, PartialEq)]
struct Draft {
    language: String,
    timezone: String,
    retention_months: Option<f64>,
    photo_warn_mib: Option<f64>,
    auto_log: bool,
    notify: bool,
    quiet_hours: String,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            language: String::new(),
            timezone: String::new(),
            retention_months: None,
            photo_warn_mib: None,
            auto_log: true,
            notify: false,
            quiet_hours: String::new(),
        }
    }
}

impl Draft {
    /// `GET /api/settings` → the form state.
    fn from_settings(settings: &Value) -> Self {
        Self {
            language: settings["language"].as_str().unwrap_or_default().to_owned(),
            timezone: settings["display_timezone"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            retention_months: settings["snapshot_retention_months"].as_f64(),
            photo_warn_mib: settings["photo_warn_bytes"].as_f64().map(|b| b / MIB),
            auto_log: settings["auto_log_ha_events"].as_bool().unwrap_or(true),
            notify: settings["env_alert_notifications"]
                .as_bool()
                .unwrap_or(false),
            quiet_hours: settings["quiet_hours"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        }
    }

    /// The `PUT /api/settings` body: `null` resets a key, the
    /// defaults fill unset ones.
    fn to_body(&self) -> Value {
        json!({
            "language": non_empty(self.language.clone()),
            "display_timezone": non_empty(self.timezone.clone()),
            "snapshot_retention_months": self
                .retention_months
                .filter(|months| months.is_finite())
                .map(|months| whole(months.max(1.0))),
            "photo_warn_bytes": self
                .photo_warn_mib
                .filter(|mib| mib.is_finite())
                .map(|mib| whole(mib.max(1.0) * MIB)),
            "auto_log_ha_events": self.auto_log,
            "env_alert_notifications": self.notify,
            "quiet_hours": non_empty(self.quiet_hours.clone()),
        })
    }
}

/// A rounded, non-negative count; the backend stores whole numbers.
fn whole(value: f64) -> u64 {
    #[allow(clippy::cast_possible_truncation)]
    // the cast saturates and the checked conversion falls back to 1
    u64::try_from(value.round() as i64).unwrap_or(1)
}

/// `/settings`: one card per group, one save for the whole page.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn SettingsPage() -> impl IntoView {
    let i18n = use_i18n();
    let draft = RwSignal::new(Draft::default());
    let status = RwSignal::new(SaveStatus::Idle);
    let error = RwSignal::new(None::<String>);
    // Nothing can be saved before the stored values are in the draft:
    // an early save, or one after a failed load, would write the
    // defaults over them. The error banner is the only content then;
    // reloading the page retries.
    let loaded = RwSignal::new(false);

    Effect::new(move |_| {
        spawn_local(async move {
            match api::get::<Value>("api/settings").await {
                Ok(settings) => {
                    draft.set(Draft::from_settings(&settings));
                    loaded.set(true);
                }
                Err(failure) => {
                    error.set(Some(api::failure_text(
                        &failure,
                        i18n.get_locale_untracked(),
                    )));
                }
            }
        });
    });

    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        status.set(SaveStatus::Saving);
        let body = draft.with_untracked(Draft::to_body);
        spawn_local(async move {
            match api::put::<Value, Value>("api/settings", &body).await {
                Ok(_) => {
                    error.set(None);
                    status.set(SaveStatus::Saved);
                }
                Err(failure) => {
                    error.set(Some(api::failure_text(
                        &failure,
                        i18n.get_locale_untracked(),
                    )));
                    status.set(SaveStatus::Failed);
                }
            }
        });
    };

    view! {
        <ui::PageHeader>
            <h2>{t!(i18n, settings.heading)}</h2>
        </ui::PageHeader>
        <crate::ErrorIf error />
        <Show when=move || loaded.get()>
        <form class="grid grid-cols-1 gap-4 lg:grid-cols-2" on:submit=save>
            <GeneralCard draft />
            <AutomationCard draft />
            <div class="flex flex-wrap items-center gap-3 lg:col-span-2">
                <button class="btn btn-primary min-h-11" type="submit"
                    disabled=move || status.get() == SaveStatus::Saving
                >
                    {t!(i18n, settings.save)}
                </button>
                <span class="text-sm text-base-content/70" aria-live="polite">
                    {move || match status.get() {
                        SaveStatus::Saving => t_string!(i18n, settings.saving).into_owned(),
                        SaveStatus::Saved => t_string!(i18n, settings.saved).into_owned(),
                        SaveStatus::Failed => t_string!(i18n, settings.save_failed).into_owned(),
                        SaveStatus::Idle => String::new(),
                    }}
                </span>
            </div>
        </form>
        </Show>
    }
}

/// Language, time zone and the two size limits.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
fn GeneralCard(draft: RwSignal<Draft>) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <section class=ui::card() aria-labelledby="settings-general">
            <h3 id="settings-general">{t!(i18n, settings.general)}</h3>
            <ui::Field label=Signal::derive(move || t_string!(i18n, settings.language).into_owned())>
                <select class="select" data-setting="language"
                    prop:value=move || draft.get().language
                    on:change=move |ev| draft.update(|d| d.language = event_target_value(&ev))
                >
                    <option value="">{t!(i18n, settings.language_system)}</option>
                    <option value="en">"English"</option>
                    <option value="de">"Deutsch"</option>
                </select>
                <p class="text-xs text-base-content/70">{t!(i18n, settings.language_hint)}</p>
            </ui::Field>
            <ui::Field label=Signal::derive(move || t_string!(i18n, settings.timezone).into_owned())>
                <input class="input" type="text" data-setting="display_timezone"
                    placeholder="Europe/Berlin"
                    prop:value=move || draft.get().timezone
                    on:input=move |ev| draft.update(|d| d.timezone = event_target_value(&ev))
                />
                <p class="text-xs text-base-content/70">{t!(i18n, settings.timezone_hint)}</p>
            </ui::Field>
            <ui::NumberField label=Signal::derive(move || t_string!(i18n, settings.retention).into_owned())
                step="1" draft
                read={|d: &Draft| d.retention_months}
                write={|d: &mut Draft, value| d.retention_months = value}
                minimum="1"
                placeholder="24"
            />
            <p class="-mt-2 text-xs text-base-content/70">{t!(i18n, settings.retention_hint)}</p>
            <ui::NumberField label=Signal::derive(move || t_string!(i18n, settings.photo_warn).into_owned())
                step="1" draft
                read={|d: &Draft| d.photo_warn_mib}
                write={|d: &mut Draft, value| d.photo_warn_mib = value}
                minimum="1"
                placeholder="2048"
            />
            <p class="-mt-2 text-xs text-base-content/70">{t!(i18n, settings.photo_warn_hint)}</p>
        </section>
    }
}

/// The two Home Assistant switches and the quiet window.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
fn AutomationCard(draft: RwSignal<Draft>) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <section class=ui::card() aria-labelledby="settings-automation">
            <h3 id="settings-automation">{t!(i18n, settings.automation)}</h3>
            <label class="fieldset">
                <span class="fieldset-label">{t!(i18n, settings.auto_log)}</span>
                <input class="checkbox" type="checkbox" data-setting="auto_log_ha_events"
                    prop:checked=move || draft.get().auto_log
                    on:change=move |ev| draft.update(|d| d.auto_log = event_target_checked(&ev))
                />
                <p class="text-xs text-base-content/70">{t!(i18n, settings.auto_log_hint)}</p>
            </label>
            <label class="fieldset">
                <span class="fieldset-label">{t!(i18n, settings.notify)}</span>
                <input class="checkbox" type="checkbox" data-setting="env_alert_notifications"
                    prop:checked=move || draft.get().notify
                    on:change=move |ev| draft.update(|d| d.notify = event_target_checked(&ev))
                />
                <p class="text-xs text-base-content/70">{t!(i18n, settings.notify_hint)}</p>
            </label>
            <ui::Field label=Signal::derive(move || t_string!(i18n, settings.quiet_hours).into_owned())>
                <input class="input" type="text" data-setting="quiet_hours"
                    placeholder="22:00 - 07:00"
                    prop:value=move || draft.get().quiet_hours
                    on:input=move |ev| draft.update(|d| d.quiet_hours = event_target_value(&ev))
                />
                <p class="text-xs text-base-content/70">{t!(i18n, settings.quiet_hours_hint)}</p>
            </ui::Field>
        </section>
    }
}
