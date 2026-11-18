//! The Environment section on the grow page (env-section rework
//! ticket 01): the live tiles as the top row, the history charts
//! beneath them, and one shared chart-window control in the section
//! header. The window choice persists per browser in localStorage.

use crate::chart::{
    BandEditor, HistoryChart, WindowToggles, format_ec_us, format_kpa, format_on_off, format_pct,
    format_ph, format_ppm,
};
use crate::dashboard::EnvDashboard;
use leptos::prelude::*;

/// localStorage key holding the shared chart window as a wire label
/// (`day` / `week` / `month`).
const HISTORY_WINDOW_KEY: &str = "leaflog.history-window";

/// Maps a stored value to a shared-window wire label. Only the three
/// known labels survive; anything else (a missing key, a stale or
/// foreign value) falls back to `week`, the 7-day default — the
/// day/night rhythm stays visible without touching anything.
#[must_use]
fn normalize_history_window(stored: &str) -> &'static str {
    match stored {
        "day" => "day",
        "month" => "month",
        _ => "week",
    }
}

/// Reads the stored window; a missing key gives the 7-day default.
/// A locked-down storage area is ignored the same way.
fn load_history_window() -> String {
    let stored = web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(HISTORY_WINDOW_KEY).ok().flatten());
    normalize_history_window(stored.as_deref().unwrap_or_default()).to_owned()
}

/// Persists the window choice per browser; a locked-down storage
/// area is ignored (the page keeps working with the session's
/// choice).
fn save_history_window(label: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.set_item(HISTORY_WINDOW_KEY, label);
    }
}

/// The section-header control: the shared 24h / 7d / 30d toggles plus
/// a reserved empty slot for the future whole-grow window. The slot
/// holds the fourth button's width without drawing a disabled teaser,
/// so that window's arrival never shifts the row.
#[component]
fn HistoryWindowControl(window: RwSignal<String>) -> impl IntoView {
    view! {
        <div class="flex items-center">
            <WindowToggles window aria_label="History window" />
            <span class="w-9 h-6" aria-hidden="true"></span>
        </div>
    }
}

/// The Environment section: the live tiles as its top row, the
/// history charts beneath them, and one shared window control in the
/// header that every chart follows. Each card keeps its own toggle
/// until the grouping ticket retires it; while both exist, a shared
/// change wins on every card and a card's own toggle retargets that
/// card alone.
#[component]
pub fn EnvironmentSection(grow_id: String) -> impl IntoView {
    let window = RwSignal::new(load_history_window());
    // Persist every settled value, including the first visit's
    // default: the stored choice then never disagrees with what the
    // page shows.
    Effect::new(move |_| {
        let label = window.get();
        save_history_window(&label);
    });

    view! {
        <div>
            <div class="flex flex-wrap items-center justify-between gap-2">
                <h3>"Environment"</h3>
                <HistoryWindowControl window=window />
            </div>
            <EnvDashboard grow_id=grow_id.clone() />
            // Two-up on wide screens: the paired roles read side by
            // side and the scroll shortens by half. Explicit
            // `grid-cols-1` at the base keeps every track
            // `minmax(0, 1fr)`: an implicit auto track would let a
            // not-yet-resized chart SVG stretch the column, and
            // `resize()` — which re-reads the container — would then
            // confirm the stale width instead of shrinking into it.
            <div class="grid grid-cols-1 gap-4 items-start lg:grid-cols-2">
                <HistoryChart grow_id=grow_id.clone() shared_window=window
                    format_value=leaflog_core::units::temp_both
                />
                <HistoryChart grow_id=grow_id.clone() role="humidity" title="Humidity"
                    shared_window=window format_value=format_pct
                />
                <HistoryChart grow_id=grow_id.clone() role="vpd" title="VPD"
                    shared_window=window format_value=format_kpa
                />
                <HistoryChart grow_id=grow_id.clone() role="co2" title="CO₂"
                    shared_window=window format_value=format_ppm
                />
                <HistoryChart grow_id=grow_id.clone() role="light" title="Light"
                    shared_window=window format_value=format_on_off
                />
                <HistoryChart grow_id=grow_id.clone() role="soil_moisture" title="Soil moisture"
                    shared_window=window format_value=format_pct
                >
                    <BandEditor />
                </HistoryChart>
                <HistoryChart grow_id=grow_id.clone() role="ph" title="pH"
                    shared_window=window format_value=format_ph
                />
                <HistoryChart grow_id role="ec" title="EC"
                    shared_window=window format_value=format_ec_us
                />
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_storage_key_is_namespaced_to_leaflog() {
        assert_eq!(HISTORY_WINDOW_KEY, "leaflog.history-window");
    }

    #[test]
    fn normalize_keeps_the_three_wire_labels() {
        assert_eq!(normalize_history_window("day"), "day");
        assert_eq!(normalize_history_window("week"), "week");
        assert_eq!(normalize_history_window("month"), "month");
    }

    #[test]
    fn anything_else_falls_back_to_the_seven_day_default() {
        // A missing key arrives as the empty string; a stale or
        // foreign value must never break the page.
        assert_eq!(normalize_history_window(""), "week");
        assert_eq!(normalize_history_window("year"), "week");
        assert_eq!(normalize_history_window("Week"), "week");
    }
}
