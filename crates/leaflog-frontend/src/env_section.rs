//! The Environment section on the grow page (env-section rework
//! ticket 01): the live tiles as the top row, then the history charts
//! in two groups — **Air** and **Root zone** — each under an
//! uppercase micro-label header. One shared chart-window control in
//! the section header drives every chart. The window choice persists
//! per browser in localStorage. Ticket 02 groups the charts and joins
//! them in one crosshair-synced connect group; ticket 03 closes the
//! section with one quiet "not measured" card that chips every
//! sensor role the grow has not bound yet, each chip linking into the
//! existing binding flow.

use std::collections::BTreeMap;

use crate::api;
use crate::chart::{
    BandEditor, HistoryChart, WindowToggles, format_ec_us, format_kpa, format_on_off, format_pct,
    format_ph, format_ppm,
};
use crate::dashboard::EnvDashboard;
use crate::route_href;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;

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

/// The sensor roles whose chart cards this section renders, in card
/// order — the only roles a missing binding can hide here. VPD needs
/// no chip (it derives from temperature + humidity, so one of those
/// chips explains its absence), pH and EC are log-detail roles with
/// their own fallback rules, and the reservoir roles carry no chart
/// card in this section.
const CHART_SENSOR_ROLES: &[(&str, &str)] = &[
    ("temperature", "Temperature"),
    ("humidity", "Humidity"),
    ("co2", "CO₂"),
    ("light", "Light"),
    ("soil_moisture", "Soil moisture"),
];

/// Derives the unbound chart roles from a grow's binding map: a
/// role with no key is unbound, whatever else the map carries. Key
/// presence alone decides — a dead binding keeps its chart card
/// with the degraded banner, so it must not chip here.
#[must_use]
fn unbound_chart_roles(bindings: &BTreeMap<String, String>) -> Vec<(&'static str, &'static str)> {
    CHART_SENSOR_ROLES
        .iter()
        .filter(|(role, _)| !bindings.contains_key(*role))
        .copied()
        .collect()
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

/// A group's micro-label header: the group's name — a glossary term —
/// in the section's quiet uppercase register, above its chart grid.
#[component]
fn GroupLabel(text: &'static str) -> impl IntoView {
    view! {
        <h4 class="m-0 mt-4 text-xs font-semibold uppercase tracking-wider text-base-content/60">
            {text}
        </h4>
    }
}

/// The Environment section: the live tiles as its top row, then the
/// history charts in two groups — Air and Root zone, each under its
/// micro-label header — with the Light card between them, and one
/// shared window control in the header that every chart follows.
/// Every mounted chart joins the connect group, so a hover anywhere
/// shows the crosshair on every chart at the same timestamp.
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
            // The Air group: above-ground concerns under their
            // micro-label header. Two-up on wide screens: the paired
            // roles read side by side and the scroll shortens by half.
            // Explicit `grid-cols-1` at the base keeps every track
            // `minmax(0, 1fr)`: an implicit auto track would let a
            // not-yet-resized chart SVG stretch the column, and
            // `resize()` — which re-reads the container — would then
            // confirm the stale width instead of shrinking into it.
            <GroupLabel text="Air" />
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
            </div>
            // The Light card keeps its own slot between the groups —
            // the fixture hangs above the soil — until the
            // night-shading ticket folds it into the Air charts.
            <div class="mt-4">
                <HistoryChart grow_id=grow_id.clone() role="light" title="Light"
                    shared_window=window format_value=format_on_off joined_crosshair=false
                />
            </div>
            // The Root zone group: below-ground concerns.
            <GroupLabel text="Root zone" />
            <div class="grid grid-cols-1 gap-4 items-start lg:grid-cols-2">
                <HistoryChart grow_id=grow_id.clone() role="soil_moisture" title="Soil moisture"
                    shared_window=window format_value=format_pct
                >
                    <BandEditor />
                </HistoryChart>
                <HistoryChart grow_id=grow_id.clone() role="ph" title="pH"
                    shared_window=window format_value=format_ph
                />
                <HistoryChart grow_id=grow_id.clone() role="ec" title="EC"
                    shared_window=window format_value=format_ec_us
                />
            </div>
            <NotMeasuredCard grow_id />
        </div>
    }
}

/// The not-measured card (ticket 03): the section's quiet closing
/// word on unbound roles. One compact card — quieter than the chart
/// cards it follows — lists a chip for every sensor role the grow
/// has not bound, and each chip is the bind affordance: an in-app
/// link to the grow edit page, where the existing Entity bindings
/// flow lives. The card appears only once the bindings response
/// names at least one unbound role: in flight (or on a failed
/// fetch) it renders nothing, so it never flashes and never claims
/// a gap it cannot see. A dead binding is not unbound — its chart
/// card stays with the degraded banner — so only key absence chips
/// here.
#[component]
fn NotMeasuredCard(grow_id: String) -> impl IntoView {
    // `None` until the bindings response arrives: no card while the
    // fetch is in flight or after it failed.
    let unbound = RwSignal::<Option<Vec<(&'static str, &'static str)>>>::new(None);
    let edit_href = route_href(&format!("/grows/{grow_id}/edit"));

    Effect::new(move |_| {
        let id = grow_id.clone();
        spawn_local(async move {
            if let Ok(bindings) =
                api::get::<BTreeMap<String, String>>(&format!("api/grows/{id}/bindings")).await
            {
                unbound.set(Some(unbound_chart_roles(&bindings)));
            }
        });
    });

    view! {
        {move || {
            // `?` on the Option signal: no bindings response, no
            // card — nothing flashes while the fetch is in flight.
            let roles = unbound.get()?;
            // The moved href is a clone, so this outer closure stays
            // `FnMut` and Leptos may rerun it (same pattern as the
            // drift banner's edit link).
            let edit_href = edit_href.clone();
            (!roles.is_empty()).then(move || {
                view! {
                    <div class="card border border-base-300 bg-base-100 px-4 py-3 mt-4">
                        <div class="flex flex-wrap items-baseline gap-2">
                            <h4 class="m-0">"Not measured"</h4>
                            <span class="text-sm text-base-content/60">
                                "No sensor is bound for these readings — each chart
                                 appears once one is."
                            </span>
                        </div>
                        <ul class="list-none m-0 p-0 mt-2 flex flex-wrap gap-2">
                            {roles
                                .into_iter()
                                .map(|(_, label)| {
                                    // The chip itself is the bind affordance,
                                    // one tab stop per role, into the edit
                                    // page's Entity bindings flow.
                                    let href = edit_href.clone();
                                    view! {
                                        <li>
                                            <A attr:class="badge badge-ghost gap-1" href=href>
                                                {label}" "
                                                <span class="link">"Bind"</span>
                                            </A>
                                        </li>
                                    }
                                })
                                .collect::<Vec<_>>()}
                        </ul>
                    </div>
                }
            })
        }}
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

    #[test]
    fn an_empty_binding_map_leaves_every_chart_role_unbound() {
        let unbound = unbound_chart_roles(&std::collections::BTreeMap::new());
        let roles: Vec<&str> = unbound.iter().map(|(role, _)| *role).collect();
        assert_eq!(
            roles,
            ["temperature", "humidity", "co2", "light", "soil_moisture"]
        );
    }

    #[test]
    fn bound_roles_leave_the_chip_list() {
        let bindings = std::collections::BTreeMap::from([
            (
                "temperature".to_owned(),
                "sensor.grow_temperature".to_owned(),
            ),
            ("co2".to_owned(), "sensor.grow_co2".to_owned()),
        ]);
        let unbound = unbound_chart_roles(&bindings);
        let roles: Vec<&str> = unbound.iter().map(|(role, _)| *role).collect();
        assert_eq!(roles, ["humidity", "light", "soil_moisture"]);
    }

    #[test]
    fn foreign_keys_never_add_or_remove_chips() {
        // VPD derives from temperature + humidity, the pH/EC charts
        // read the action log, and the reservoir roles carry no
        // chart card here: none of those keys may touch the list.
        let bindings = std::collections::BTreeMap::from([
            ("vpd".to_owned(), "sensor.derived".to_owned()),
            ("ph".to_owned(), "sensor.ph".to_owned()),
            ("ec".to_owned(), "sensor.ec".to_owned()),
            ("reservoir_ph".to_owned(), "sensor.reservoir_ph".to_owned()),
            ("reservoir_ec".to_owned(), "sensor.reservoir_ec".to_owned()),
        ]);
        let unbound = unbound_chart_roles(&bindings);
        let roles: Vec<&str> = unbound.iter().map(|(role, _)| *role).collect();
        assert_eq!(
            roles,
            ["temperature", "humidity", "co2", "light", "soil_moisture"]
        );
    }

    #[test]
    fn the_chip_labels_spell_the_card_titles() {
        // One spelling per concept: a chip names the missing card
        // with the card's own title, CO₂ subscript included.
        let labels: Vec<&str> = unbound_chart_roles(&std::collections::BTreeMap::new())
            .iter()
            .map(|(_, label)| *label)
            .collect();
        assert_eq!(
            labels,
            ["Temperature", "Humidity", "CO₂", "Light", "Soil moisture"]
        );
    }
}
