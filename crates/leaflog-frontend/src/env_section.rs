//! The Environment section on the grow page (env-section rework
//! ticket 01): the live tiles as the top row, then the history charts
//! in two groups — **Air** and **Root zone** — each under an
//! uppercase micro-label header. One shared chart-window control in
//! the section header drives every chart. The window choice persists
//! per browser in localStorage. Ticket 02 groups the charts and joins
//! them in one crosshair-synced connect group; ticket 03 closes the
//! section with one quiet "not measured" card that chips every
//! sensor role the grow has not bound yet, each chip linking into the
//! existing binding flow. Ticket 05 folds the light story into the
//! Air charts: a night shading behind them (the bound light's
//! history, or the Light Cycle when no light is bound) replaces the
//! standalone Light card. Ticket 07 mounts the Root zone chart grid
//! lazily — the charts wait for the group's first scroll into view
//! (with an immediate-render fallback), while the group labels stay
//! structural so the section's outline never flickers.

use std::collections::BTreeMap;

use crate::api;
use crate::chart::{
    BandEditor, HistoryChart, WindowToggles, format_ec_us, format_kpa, format_pct, format_ph,
    format_ppm,
};
use crate::dashboard::EnvDashboard;
use crate::route_href;
use leaflog_core::chart::{ChartGroup, chart_group};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

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

/// One chart card the Environment section renders.
#[derive(Debug, Clone, Copy)]
struct SectionChart {
    /// The wire role: the history query's `role` and the
    /// [`chart_group`] input.
    role: &'static str,
    /// The card (and chip) title, one spelling per concept.
    title: &'static str,
    /// The card's value formatter.
    format_value: fn(f64) -> String,
    /// Whether a missing binding chips the role on the not-measured
    /// card. False for VPD (it derives from temperature + humidity,
    /// so one of those chips explains its absence) and for pH/EC
    /// (log-detail roles with their own fallback rules).
    chips_when_unbound: bool,
}

/// The section's cards in render order — the only roles a missing
/// binding can hide here. The Air/Root-zone split comes from the
/// core's [`chart_group`], the same table the backend gates its
/// Air-group light-timeline work on, so membership lives in exactly
/// one place. Reservoir roles carry no card here, and light has no
/// card either (ticket 05 folded it into the Air charts' night
/// shading): both chip nowhere — the edit page still takes the
/// binding, and the shading reads it.
const SECTION_CHARTS: &[SectionChart] = &[
    SectionChart {
        role: "temperature",
        title: "Temperature",
        format_value: leaflog_core::units::temp_both,
        chips_when_unbound: true,
    },
    SectionChart {
        role: "humidity",
        title: "Humidity",
        format_value: format_pct,
        chips_when_unbound: true,
    },
    SectionChart {
        role: "vpd",
        title: "VPD",
        format_value: format_kpa,
        chips_when_unbound: false,
    },
    SectionChart {
        role: "co2",
        title: "CO₂",
        format_value: format_ppm,
        chips_when_unbound: true,
    },
    SectionChart {
        role: "soil_moisture",
        title: "Soil moisture",
        format_value: format_pct,
        chips_when_unbound: true,
    },
    SectionChart {
        role: "ph",
        title: "pH",
        format_value: format_ph,
        chips_when_unbound: false,
    },
    SectionChart {
        role: "ec",
        title: "EC",
        format_value: format_ec_us,
        chips_when_unbound: false,
    },
];

/// The chart-card grid: two-up on wide screens (the paired roles
/// read side by side and the scroll shortens by half). Explicit
/// `grid-cols-1` at the base keeps every track `minmax(0, 1fr)`: an
/// implicit auto track would let a not-yet-resized chart SVG
/// stretch the column, and `resize()` — which re-reads the
/// container — would then confirm the stale width instead of
/// shrinking into it.
const CARD_GRID: &str = "grid grid-cols-1 gap-4 items-start lg:grid-cols-2";

/// Builds one chart card from the section table. The soil-moisture
/// card carries the Target Bands editor as its child; every other
/// card is childless — the optional-children prop takes the closure
/// only when present, so the two shapes branch here.
fn section_chart_card(
    chart: &'static SectionChart,
    grow_id: String,
    window: RwSignal<String>,
    markers: RwSignal<bool>,
) -> AnyView {
    let role = chart.role;
    let title = chart.title;
    let format_value = chart.format_value;
    if role == "soil_moisture" {
        // A fresh editor view per call: the children closure may run
        // more than once (Leptos re-invokes it on re-render).
        let children: ChildrenFn = std::sync::Arc::new(|| view! { <BandEditor /> }.into_any());
        view! {
            <HistoryChart grow_id=grow_id role=role title=title
                shared_window=window shared_markers=markers
                format_value=format_value children=children
            />
        }
        .into_any()
    } else {
        view! {
            <HistoryChart grow_id=grow_id role=role title=title
                shared_window=window shared_markers=markers
                format_value=format_value
            />
        }
        .into_any()
    }
}

/// Derives the unbound chart roles from a grow's binding map: a
/// role with no key is unbound, whatever else the map carries. Key
/// presence alone decides — a dead binding keeps its chart card
/// with the degraded banner, so it must not chip here.
#[must_use]
fn unbound_chart_roles(bindings: &BTreeMap<String, String>) -> Vec<(&'static str, &'static str)> {
    SECTION_CHARTS
        .iter()
        .filter(|chart| chart.chips_when_unbound && !bindings.contains_key(chart.role))
        .map(|chart| (chart.role, chart.title))
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

/// The section-header control: the shared 24h / 7d / 30d toggles,
/// a reserved empty slot for the future whole-grow window (it holds
/// the fourth button's width without drawing a disabled teaser, so
/// that window's arrival never shifts the row), and the shared
/// log-event marker toggle (ticket 04) — one switch hides or shows
/// the event ticks on every chart at once. The marker choice stays
/// per visit; only the window persists.
#[component]
fn HistoryWindowControl(window: RwSignal<String>, markers: RwSignal<bool>) -> impl IntoView {
    view! {
        <div class="flex items-center gap-3">
            <WindowToggles window aria_label="History window" />
            <span class="w-9 h-6" aria-hidden="true"></span>
            <label class="flex items-center gap-1 cursor-pointer text-sm text-base-content/70">
                <input class="checkbox checkbox-xs" type="checkbox"
                    prop:checked=move || markers.get()
                    on:change=move |ev| {
                        markers.set(event_target_checked(&ev));
                    }
                />
                "Markers"
            </label>
        </div>
    }
}

/// A group's micro-label header: the group's name — a glossary term —
/// in the section's quiet uppercase register, above its chart grid,
/// with an optional legend hint beside it (the Air group names its
/// night shading there).
#[component]
fn GroupLabel(text: &'static str, #[prop(default = "")] hint: &'static str) -> impl IntoView {
    view! {
        <div class="flex flex-wrap items-baseline gap-2 mt-4">
            <h4 class="m-0 text-xs font-semibold uppercase tracking-wider text-base-content/60">
                {text}
            </h4>
            {(!hint.is_empty()).then(|| {
                view! { <span class="text-xs text-base-content/50">{hint}</span> }
            })}
        </div>
    }
}

/// Whether this browser exposes `IntersectionObserver` — the gate
/// between the lazy and immediate mount paths. Probed reflectively
/// (the same seam the `ECharts` bridge uses) so availability is one
/// lookup, not a constructor round-trip.
fn intersection_observer_available() -> bool {
    web_sys::window()
        .and_then(|window| {
            js_sys::Reflect::get(window.as_ref(), &JsValue::from_str("IntersectionObserver")).ok()
        })
        .is_some_and(|candidate| candidate.dyn_into::<js_sys::Function>().is_ok())
}

/// The lazy group's initial render state (ticket 07): with an
/// observer the charts wait for the first intersection; without one
/// there is nothing to wait for, so they render immediately — the
/// fallback keeps the group readable on every browser.
const fn initial_visible(observer_available: bool) -> bool {
    !observer_available
}

/// How much vertical space the lazy block reserves before its
/// charts mount (ticket 07), so the section keeps its height while
/// the fetches are still pending and the mount does not jump the
/// page under the visitor. The values cover the stacked cards
/// (mobile) and the two-row grid (`lg`); anything taller simply
/// grows past the reservation when it mounts.
const fn reserved_block_height_class() -> &'static str {
    "min-h-[620px] lg:min-h-[480px]"
}

/// How long the lazy watch waits before it starts observing
/// (ticket 07). The Air cards above the block render only once
/// their first history fetch resolves, so the section grows by
/// hundreds of pixels after first paint — a watch armed at mount
/// sees the block "in view" on that short page and fires before any
/// scroll happened. Arming after this delay lets that growth pass;
/// a visitor who is already there when the watch arms still gets an
/// immediate intersection, and on a network so slow that the growth
/// outlasts the delay the block simply mounts a little early —
/// once, as always.
const LAZY_WATCH_ARM_DELAY_MS: i32 = 500;

/// A lazily mounted block of the section (ticket 07): the Root zone
/// chart grid renders only once its wrapper scrolls into view, so
/// the first paint skips the three history fetches a visitor may
/// never scroll to. The watch arms after
/// [`LAZY_WATCH_ARM_DELAY_MS`] and the wrapper reserves its height
/// meanwhile (see both constants for why). The block mounts once —
/// the observer disconnects on the first intersection and never
/// re-runs the work, whatever the scroll does afterwards. The
/// group's label is deliberately not part of the block: the
/// section's outline stays structural, so a visitor always sees
/// what is coming, and the label is the natural scroll target.
#[component]
fn LazyCharts(children: ChildrenFn) -> impl IntoView {
    let available = intersection_observer_available();
    let visible = RwSignal::new(initial_visible(available));
    // The watch arms after the section's own layout has settled;
    // until then there is nothing to observe honestly.
    let armed = RwSignal::new(false);
    // The one-shot arming timer; dropping it cancels it.
    let arming_slot = StoredValue::new_local(Option::<gloo_timers::callback::Timeout>::None);
    // The observer slot: filled once the watch arms, taken (and
    // disconnected) at the first intersection or at cleanup,
    // whichever comes first. The callback rides in the same slot so
    // its lifetime matches the observer's exactly.
    let observer_slot = StoredValue::new_local(
        Option::<(
            web_sys::IntersectionObserver,
            Closure<dyn FnMut(js_sys::Array)>,
        )>::None,
    );
    // The observed wrapper: its reserved height holds the block's
    // place in the layout while the charts are still unmounted, so
    // an intersection of the wrapper means the visitor scrolled to
    // where the charts belong — not to a transiently short page.
    let wrapper = NodeRef::<leptos::html::Div>::new();

    // Take down the whole watch: the arming timer first (it may
    // still be pending), then the observer pair.
    let drop_watch = move || {
        arming_slot.update_value(|slot| {
            if let Some(timer) = slot.take() {
                timer.cancel();
            }
        });
        observer_slot.update_value(|slot| {
            if let Some((observer, _callback)) = slot.take() {
                observer.disconnect();
            }
        });
    };

    // Arm the watch once, after the delay. A timer that cannot be
    // armed is not a reason to hide the group: arm now.
    if available {
        let timer = gloo_timers::callback::Timeout::new(
            u32::try_from(LAZY_WATCH_ARM_DELAY_MS).unwrap_or(0),
            move || armed.set(true),
        );
        arming_slot.set_value(Some(timer));
    } else {
        armed.set(true);
    }

    Effect::new(move |_| {
        if visible.get() {
            drop_watch();
            return;
        }
        if !armed.get() {
            return;
        }
        // Wait for the wrapper element to exist before observing it.
        let Some(wrapper) = wrapper.get() else { return };
        let entered = visible;
        let callback = Closure::<dyn FnMut(js_sys::Array)>::new(move |entries: js_sys::Array| {
            let intersected = entries.to_vec().iter().any(|entry| {
                entry
                    .dyn_ref::<web_sys::IntersectionObserverEntry>()
                    .is_some_and(web_sys::IntersectionObserverEntry::is_intersecting)
            });
            if intersected {
                entered.set(true);
            }
        });
        // The new wasm-bindgen closures hand JavaScript a function,
        // not themselves (theme.rs keeps a pair alive the same way).
        let handler = js_sys::Function::from(callback.as_js_value().clone());
        let Ok(observer) = web_sys::IntersectionObserver::new(&handler) else {
            // The availability probe lied (the constructor threw):
            // take the immediate path rather than hiding the group
            // behind a watch no callback will ever fire for.
            visible.set(true);
            return;
        };
        observer.observe(&wrapper);
        observer_slot.set_value(Some((observer, callback)));
    });
    on_cleanup(drop_watch);

    view! {
        <div node_ref=wrapper class=reserved_block_height_class()>
            {move || {
                let children = children.clone();
                visible.get().then(move || children())
            }}
        </div>
    }
}

/// The Environment section: the live tiles as its top row, then the
/// history charts in two groups — Air and Root zone, each under its
/// micro-label header — with one shared window control in the header
/// that every chart follows. Every group chart joins the connect
/// group, so a hover anywhere shows the crosshair on every chart at
/// the same timestamp. The Air charts carry the light story as a
/// night shading behind them (ticket 05) — observed light states
/// when a light entity is bound, the Light Cycle otherwise — so the
/// standalone Light card is gone and its legend rides the Air label.
#[component]
pub fn EnvironmentSection(grow_id: String) -> impl IntoView {
    // The lazy Root zone grid captures `grow_id` into its children
    // closure; the not-measured card keeps its own copy.
    let not_measured_grow_id = grow_id.clone();
    let window = RwSignal::new(load_history_window());
    // The shared marker toggle: on until the visitor turns it off,
    // per visit (only the window persists — the tick choice is a
    // glance, not a preference worth storage).
    let markers = RwSignal::new(true);
    // Persist every settled value, including the first visit's
    // default: the stored choice then never disagrees with what the
    // page shows.
    Effect::new(move |_| {
        let label = window.get();
        save_history_window(&label);
    });

    // The Air cards mount with the section. The Root zone grid is
    // the lazy child (ticket 07): built inside the children closure,
    // which Leptos may call more than once — RwSignals are Copy, the
    // grow id clones per call, so the closure stays `Fn`.
    let air_cards: Vec<_> = SECTION_CHARTS
        .iter()
        .filter(|chart| chart_group(chart.role) == ChartGroup::Air)
        .map(|chart| section_chart_card(chart, grow_id.clone(), window, markers))
        .collect();
    let root_zone_grid: ChildrenFn = {
        let grow_id = grow_id.clone();
        std::sync::Arc::new(move || {
            let cards: Vec<_> = SECTION_CHARTS
                .iter()
                .filter(|chart| chart_group(chart.role) == ChartGroup::RootZone)
                .map(|chart| section_chart_card(chart, grow_id.clone(), window, markers))
                .collect();
            view! { <div class=CARD_GRID>{cards}</div> }.into_any()
        })
    };

    view! {
        <div id="env-section">
            <div class="flex flex-wrap items-center justify-between gap-2">
                <h3>"Environment"</h3>
                <HistoryWindowControl window=window markers=markers />
            </div>
            <EnvDashboard grow_id=grow_id />
            // The Air group: above-ground concerns under their
            // micro-label header.
            <GroupLabel text="Air" hint="Shaded areas mark night periods." />
            <div class=CARD_GRID>{air_cards}</div>
            // The Root zone group: below-ground concerns. Its chart
            // grid mounts lazily (ticket 07) once the group scrolls
            // into view; the label above stays structural, so the
            // section's outline reads at first paint and the lazy
            // anchor sits right where the grid begins.
            <GroupLabel text="Root zone" />
            <LazyCharts children=root_zone_grid />
            <NotMeasuredCard grow_id=not_measured_grow_id />
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
    fn an_empty_binding_map_leaves_every_chip_role_unbound() {
        let unbound = unbound_chart_roles(&std::collections::BTreeMap::new());
        let roles: Vec<&str> = unbound.iter().map(|(role, _)| *role).collect();
        assert_eq!(roles, ["temperature", "humidity", "co2", "soil_moisture"]);
    }

    #[test]
    fn the_section_splits_cards_along_the_core_chart_groups() {
        // The backend gates its light-timeline work on the same
        // table, so the card layout and the light story stay in
        // lockstep by construction.
        let air: Vec<&str> = SECTION_CHARTS
            .iter()
            .filter(|chart| chart_group(chart.role) == ChartGroup::Air)
            .map(|chart| chart.role)
            .collect();
        assert_eq!(air, ["temperature", "humidity", "vpd", "co2"]);
        let root_zone: Vec<&str> = SECTION_CHARTS
            .iter()
            .filter(|chart| chart_group(chart.role) == ChartGroup::RootZone)
            .map(|chart| chart.role)
            .collect();
        assert_eq!(root_zone, ["soil_moisture", "ph", "ec"]);
    }

    #[test]
    fn every_section_role_maps_to_a_charted_group() {
        // A role that fell out of the core's table would silently
        // render nowhere — the section table must stay a subset of
        // the charted groups.
        for chart in SECTION_CHARTS {
            assert_ne!(
                chart_group(chart.role),
                ChartGroup::Untracked,
                "role {}",
                chart.role
            );
        }
    }

    #[test]
    fn only_bindable_roles_chip_the_not_measured_card() {
        // VPD derives from temperature + humidity; pH/EC read the
        // action log. Neither can be "not measured" by a missing
        // binding.
        let chipping: Vec<&str> = SECTION_CHARTS
            .iter()
            .filter(|chart| chart.chips_when_unbound)
            .map(|chart| chart.role)
            .collect();
        assert_eq!(
            chipping,
            ["temperature", "humidity", "co2", "soil_moisture"]
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
        assert_eq!(roles, ["humidity", "soil_moisture"]);
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
        assert_eq!(roles, ["temperature", "humidity", "co2", "soil_moisture"]);
    }

    #[test]
    fn the_chip_labels_spell_the_card_titles() {
        // One spelling per concept: a chip names the missing card
        // with the card's own title, CO₂ subscript included.
        let labels: Vec<&str> = unbound_chart_roles(&std::collections::BTreeMap::new())
            .iter()
            .map(|(_, label)| *label)
            .collect();
        assert_eq!(labels, ["Temperature", "Humidity", "CO₂", "Soil moisture"]);
    }

    #[test]
    fn without_an_observer_the_group_renders_immediately() {
        // The immediate-render fallback (ticket 07): with no
        // IntersectionObserver there is nothing to wait for — the
        // charts mount with the page instead of staying hidden
        // behind a sentinel no callback will ever fire for.
        assert!(initial_visible(false));
    }

    #[test]
    fn with_an_observer_the_group_waits_for_the_first_intersection() {
        // The lazy path: the charts wait below the fold until the
        // group scrolls into view, keeping the first paint cheap.
        assert!(!initial_visible(true));
    }
}
