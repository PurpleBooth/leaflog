//! History charts (spec §3): a series over the shaded recommended-band
//! corridor, fed by the grow history endpoint, with rolling chart
//! windows (24 h / 7 d / 30 d) owned by the Environment section's one
//! shared control. Every card — the continuous-role lines (temperature,
//! humidity, VPD, CO₂, light, soil moisture) and the pH/EC pair
//! scatters — renders through `ECharts` (SVG renderer) via the
//! option-builder seam, and every mounted chart joins one connect
//! group so a hover anywhere shows the crosshair everywhere. The
//! card-visibility rule: a role with
//! no binding (or, for the pH/EC log-detail roles, no entries) renders
//! no card at all. A dead binding is `bound: true`, so its card stays
//! and the env dashboard's No-data banner names it. A bound role whose
//! history fetch fails keeps its card too, in a degraded presentation
//! naming the failure — only unbound and dataless roles hide.

use crate::api;
use crate::api::{GrowHistory, SoilBand};
use crate::chart_option;
use js_sys::{Function, Reflect};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

/// Axis formatter: relative humidity percent.
#[must_use]
pub fn format_pct(value: f64) -> String {
    format!("{value:.0} %")
}

/// The soil-moisture band editor: two percent inputs with save and
/// reset-to-default, stored under the `soil_moisture_band` settings
/// key. The chart card reads the same override for its corridor.
#[component]
pub fn BandEditor() -> impl IntoView {
    let min = RwSignal::<String>::new(String::new());
    let max = RwSignal::<String>::new(String::new());
    let stored = RwSignal::<bool>::new(false);
    let status = RwSignal::<Option<String>>::new(None);

    Effect::new(move |_| {
        spawn_local(async move {
            match api::get::<serde_json::Value>("api/settings").await {
                Ok(settings) => {
                    let parsed = settings
                        .get("soil_moisture_band")
                        .and_then(|value| serde_json::from_value::<SoilBand>(value.clone()).ok());
                    stored.set(parsed.is_some());
                    if let Some(band) = parsed {
                        min.set(format!("{}", band.min));
                        max.set(format!("{}", band.max));
                    }
                }
                Err(failure) => status.set(Some(failure.message)),
            }
        });
    });

    let save = move |reset: bool| {
        let payload = if reset {
            serde_json::json!({ "soil_moisture_band": null })
        } else {
            match (min.get().parse::<f64>().ok(), max.get().parse::<f64>().ok()) {
                (Some(min_value), Some(max_value))
                    if (0.0..=100.0).contains(&min_value)
                        && min_value <= max_value
                        && max_value <= 100.0 =>
                {
                    serde_json::json!({
                        "soil_moisture_band": { "min": min_value, "max": max_value }
                    })
                }
                _ => {
                    status.set(Some("Enter 0–100 with min ≤ max.".to_owned()));
                    return;
                }
            }
        };
        spawn_local(async move {
            match api::put::<serde_json::Value, serde_json::Value>("api/settings", &payload).await {
                Ok(_) => {
                    stored.set(!reset);
                    status.set(Some(if reset {
                        "Reset to default.".to_owned()
                    } else {
                        "Saved.".to_owned()
                    }));
                }
                Err(failure) => status.set(Some(failure.message)),
            }
        });
    };

    view! {
        <div class="flex flex-wrap items-end gap-2 my-1">
            <label class="fieldset">
                <span class="fieldset-label">"Band min %"</span>
                <input class="input input-sm w-24" type="number" min="0" max="100"
                    prop:value=move || min.get()
                    on:input=move |ev| min.set(event_target_value(&ev))
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Band max %"</span>
                <input class="input input-sm w-24" type="number" min="0" max="100"
                    prop:value=move || max.get()
                    on:input=move |ev| max.set(event_target_value(&ev))
                />
            </label>
            <button class="btn btn-sm self-end" on:click=move |_| save(false)>
                "Apply band"
            </button>
            <Show when=move || stored.get() fallback=|| ()>
                <button class="btn btn-sm btn-ghost self-end" on:click=move |_| save(true)>
                    "Reset to default"
                </button>
            </Show>
            {move || {
                status.get().map(|message| {
                    view! { <span class="text-base-content/60 text-sm">{message}</span> }
                })
            }}
        </div>
    }
}

/// Axis formatter: VPD kilopascals.
#[must_use]
pub fn format_kpa(value: f64) -> String {
    format!("{value:.2} kPa")
}

/// Axis formatter: CO₂ parts per million.
#[must_use]
pub fn format_ppm(value: f64) -> String {
    format!("{value:.0} ppm")
}

/// Axis formatter: a binary entity's on/off state.
#[must_use]
pub fn format_on_off(value: f64) -> String {
    if value >= 0.5 {
        "on".to_owned()
    } else {
        "off".to_owned()
    }
}

/// Axis formatter: pH values (dimensionless).
#[must_use]
pub fn format_ph(value: f64) -> String {
    format!("{value:.1}")
}

/// Axis formatter: EC in µS/cm (canonical, spec §5.3).
#[must_use]
pub fn format_ec_us(value: f64) -> String {
    format!("{value:.0} µS/cm")
}

/// One windowed chart for one sensor role on the grow overview.
/// `format_value` renders axis and tooltip text (per the HA unit
/// system). Every role draws through the `ECharts` option-builder
/// seam: lines for the continuous roles, the staircase for the light,
/// and the pair scatter (input plus runoff) for pH/EC. The
/// card-visibility rule: while the first response is in flight, or
/// when it says the role is unbound (no sensor, or no pH/EC entries),
/// the card renders nothing at all — the overview shows only what the
/// grow has. A bound role whose history fetch fails keeps its card,
/// with a degraded banner naming the failure: a dead binding is the
/// only state the page may call "no data", so hiding a bound card
/// would lie about the grow. The chart keeps its last data until a
/// retry succeeds. The pH/EC log-detail roles have no binding row to
/// fall back on, so a failed first fetch for them keeps the
/// in-flight hiding. `children` render inside the card under the
/// chart (the soil card carries its band editor).
///
/// `shared_window` is the Environment section's window signal — the
/// section control owns the window for every card (the per-card
/// toggles are gone; one instrument panel, one window).
///
/// `shared_markers` is the section's log-event marker toggle — the
/// same rule: one switch in the section header moves every card.
///
/// `joined_crosshair` opts a card out of the section's `ECharts`
/// connect group. The connected hover is broadcast in pixels, so a
/// chart at a different pixel scale would translate the same pixels
/// to a different timestamp. The light card sits in the full-width
/// slot between the Air and Root zone grids (the later shading
/// ticket retires it), so it stays out of the group.
#[component]
pub fn HistoryChart(
    grow_id: String,
    #[prop(default = "temperature")] role: &'static str,
    #[prop(default = "Temperature")] title: &'static str,
    format_value: fn(f64) -> String,
    shared_window: RwSignal<String>,
    shared_markers: RwSignal<bool>,
    #[prop(default = true)] joined_crosshair: bool,
    #[prop(optional)] children: Option<ChildrenFn>,
) -> impl IntoView {
    let data = RwSignal::<Option<GrowHistory>>::new(None);
    // The last fetch failure, if any: the degraded banner's text. A
    // successful response clears it (the chart recovered).
    let fetch_failed = RwSignal::<Option<String>>::new(None);
    // The bound bit for a failed fetch, resolved against the bindings
    // endpoint (the response normally carries it). Only a role with a
    // binding row flips true, so the pH/EC log-detail roles stay
    // hidden on failure exactly as in flight.
    let fallback_bound = RwSignal::new(false);
    // The visibility bit: the card mounts once `bound` flips true and
    // unmounts if it ever flips false, without re-mounting on every
    // in-card refetch (a window switch keeps the card in place).
    let bound =
        Memo::new(move |_| data.get().is_some_and(|chart| chart.bound) || fallback_bound.get());

    Effect::new(move |_| {
        let id = grow_id.clone();
        let window_label = shared_window.get();
        spawn_local(async move {
            match api::get::<GrowHistory>(&format!(
                "api/grows/{id}/history?role={role}&window={window_label}"
            ))
            .await
            {
                Ok(value) => {
                    fetch_failed.set(None);
                    fallback_bound.set(false);
                    data.set(Some(value));
                }
                Err(failure) => {
                    fetch_failed.set(Some(failure.message));
                    // The response that would say `bound` never
                    // arrived; the bindings endpoint is the
                    // independent source. A sensor role with a binding
                    // keeps its card (degraded); an unbound one stays
                    // hidden.
                    if let Ok(bindings) = api::get::<std::collections::BTreeMap<String, String>>(
                        &format!("api/grows/{id}/bindings"),
                    )
                    .await
                    {
                        fallback_bound.set(bindings.contains_key(role));
                    }
                }
            }
        });
    });

    view! {
        // The card-visibility gate is a reactive block, not `Show`: the
        // card carries the children slot (the soil band editor), and a
        // `Show` children closure cannot call a children builder while
        // staying `Fn`. Returning `None` unmounts the card entirely.
        {move || {
            bound.get().then(|| {
                // The card builds per visibility activation; the
                // footer (the soil band editor) re-mounts with it
                // and its own signals re-run.
                let footer = children.clone().map(|build| build());
                view! {
                    <div class="card bg-base-100 shadow-sm px-4 py-3">
                        <div class="flex flex-wrap items-baseline gap-2">
                            <h5 class="m-0 text-base font-semibold">{title}</h5>
                            <Show
                                when=move || data.get().is_some_and(|chart| chart.degraded)
                                fallback=|| ()
                            >
                                <span class="badge badge-warning badge-sm">"snapshot only"</span>
                            </Show>
                        </div>
                        {move || {
                            fetch_failed.get().map(|message| {
                                view! {
                                    <p class="alert alert-warning py-2 px-4 text-sm" role="alert">
                                        "History unavailable — " {message}
                                    </p>
                                }
                            })
                        }}
                        // An empty window renders an honest hint, not a
                        // frame with degenerate axes: the card says the
                        // quiet is data, not breakage.
                        {move || {
                            data.get()
                                .filter(chart_option::window_is_empty)
                                .map(|chart| {
                                    let hint = if chart.role == "ph" || chart.role == "ec" {
                                        "No feed entries in this window."
                                    } else {
                                        "No readings in this window."
                                    };
                                    view! {
                                        <p class="my-2 text-sm text-base-content/60">{hint}</p>
                                    }
                                })
                        }}
                        <EChartsHistory
                            data=data
                            format_value=format_value
                            joined_crosshair=joined_crosshair
                            markers=shared_markers
                        />
                        {footer}
                    </div>
                }
            })
        }}
    }
}

/// The Chart window toggle group: three rolling windows ending now,
/// labelled 24h / 7d / 30d (the old "Day" collided with the Light
/// Cycle's day). The API labels stay `day` / `week` / `month`. The
/// Environment section's header control carries it — one window for
/// the whole instrument panel, no per-card toggles.
#[component]
pub(crate) fn WindowToggles(
    window: RwSignal<String>,
    #[prop(default = "Chart window")] aria_label: &'static str,
) -> impl IntoView {
    view! {
        <div class="flex gap-1" role="group" aria-label=aria_label>
            {["day", "week", "month"]
                .into_iter()
                .map(|label| {
                    let caption = match label {
                        "day" => "24h",
                        "week" => "7d",
                        _ => "30d",
                    };
                    view! {
                        <button
                            class=move || if window.get() == label {
                                "btn btn-xs btn-primary"
                            } else {
                                "btn btn-xs"
                            }
                            on:click=move |_| window.set(label.to_owned())
                        >
                            {caption}
                        </button>
                    }
                })
                .collect::<Vec<_>>()}
        </div>
    }
}

/// The `ECharts`-drawn chart (ticket 02): a stable container div plus a
/// mount effect that feeds each loaded history response through the
/// pure option builder and drives `window.echarts` (SVG renderer) over
/// it. The axis and tooltip formatters are wasm closures handed to
/// `ECharts` through `Closure::into_js_value`, so the JS side owns their
/// lifetime and the GC reclaims them when the chart is disposed. No
/// application JavaScript exists — `js_sys` carries every call.
#[component]
fn EChartsHistory(
    data: RwSignal<Option<GrowHistory>>,
    format_value: fn(f64) -> String,
    joined_crosshair: bool,
    markers: RwSignal<bool>,
) -> impl IntoView {
    let container = NodeRef::<leptos::html::Div>::new();
    let instance = StoredValue::new_local(None::<JsValue>);

    Effect::new(move |_| {
        let Some(mut chart) = data.get() else { return };
        // The toggle hides the ticks by stripping the marker set
        // before the builder sees it (a hidden set renders no series).
        // Reading the signal here re-runs the effect on every flip.
        if !markers.get() {
            chart.markers.clear();
        }
        let Some(el) = container.get() else { return };
        let style = chart_option::role_style(&chart.role);
        let option = chart_option::history_option(&chart, &theme_look(), &style);
        // serde_wasm_bindgen's default maps `serde_json::Value::Object`
        // through `serialize_map`, which produces a JS `Map` — and
        // `setOption` reads plain objects only, so a Map option renders
        // an empty chart. Maps become objects here; the tuple points
        // stay `[epoch_ms, value]` arrays.
        let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
        let Ok(option_js) = option.serialize(&serializer) else {
            return;
        };
        // Fresh closures per option build: setOption replaces the
        // previous references, and `into_js_value` lets the JS GC
        // reclaim each one once nothing points at it any more.
        let axis_labels = axis_label_closure(format_value).into_js_value();
        let tooltip_html = tooltip_closure(format_value).into_js_value();
        attach_formatters(&option_js, &axis_labels, &tooltip_html);
        if let Some(existing) = instance.get_value() {
            call_chart_method(
                &existing,
                "setOption",
                &[&option_js, &JsValue::from_bool(true)],
            );
        } else {
            let Some(echarts) = echarts_global() else {
                return;
            };
            let mounted = init_svg_chart(&echarts, el.as_ref());
            instance.set_value(Some(mounted.clone()));
            call_chart_method(
                &mounted,
                "setOption",
                &[&option_js, &JsValue::from_bool(true)],
            );
            label_svg_root(el.as_ref(), &format!("{} history", chart.role));
            // The mounted chart joins the section's connect group so
            // a hover anywhere moves the crosshair on every chart at
            // the same timestamp. Full-width cards opt out: their
            // pixel scale differs, so the broadcast would misalign.
            if joined_crosshair {
                join_connect_group(&echarts, &mounted);
            }
        }
    });

    let resize = window_event_listener(leptos::ev::resize, move |_| {
        if let Some(existing) = instance.get_value() {
            call_chart_method(&existing, "resize", &[]);
        }
    });
    on_cleanup(move || {
        if let Some(existing) = instance.get_value() {
            call_chart_method(&existing, "dispose", &[]);
        }
        instance.set_value(None);
        resize.remove();
    });

    view! { <div node_ref=container class="h-36 lg:h-44 w-full" /> }
}

/// The connect-group id the section's history charts share: hovering
/// any chart moves the axis pointer and shows the role-scoped tooltip
/// on every chart in the group at the same timestamp. The shared
/// y-label gutter the option builder emits keeps the pixel broadcast
/// aligned across the plots.
const CONNECT_GROUP: &str = "leaflog-env-history";

/// Joins a mounted chart instance to the section's connect group: the
/// instance's `group` property names it, and `echarts.connect`
/// registers the group so hover actions broadcast. Both are
/// idempotent, so the per-mount repetition is safe; a disposed
/// instance leaves the group with its registration.
fn join_connect_group(echarts: &js_sys::Object, instance: &JsValue) {
    let group = JsValue::from_str(CONNECT_GROUP);
    let _ = Reflect::set(instance, &JsValue::from_str("group"), &group);
    let Some(connect) = Reflect::get(echarts, &JsValue::from_str("connect"))
        .ok()
        .and_then(|value| value.dyn_into::<Function>().ok())
    else {
        return;
    };
    let _ = connect.call1(echarts, &group);
}

/// Reads `window.echarts`; `None` when the bundle script did not load
/// (the build pipeline wires it, so this is a degraded fallback).
fn echarts_global() -> Option<js_sys::Object> {
    Reflect::get(&js_sys::global(), &JsValue::from_str("echarts"))
        .ok()
        .and_then(|value| value.dyn_into::<js_sys::Object>().ok())
}

/// Calls `echarts.init` with the SVG renderer forced (the only
/// supported renderer; the canvas painter is excluded at bundle level).
fn init_svg_chart(echarts: &js_sys::Object, el: &JsValue) -> JsValue {
    let Some(init) = Reflect::get(echarts, &JsValue::from_str("init"))
        .ok()
        .and_then(|value| value.dyn_into::<Function>().ok())
    else {
        return JsValue::UNDEFINED;
    };
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &options,
        &JsValue::from_str("renderer"),
        &JsValue::from_str("svg"),
    );
    init.call3(&JsValue::UNDEFINED, el, &JsValue::UNDEFINED, &options)
        .unwrap_or(JsValue::UNDEFINED)
}

/// Calls a method on the chart instance with the given arguments,
/// ignoring failures (a disposed instance must not panic the app).
/// `setOption` passes `notMerge: true`: the marker toggle changes the
/// series list, and a merged update would keep the stale series — a
/// full replace is always correct because every rebuild carries the
/// complete option (nothing stateful like dataZoom lives on these
/// charts).
fn call_chart_method(instance: &JsValue, name: &str, arguments: &[&JsValue]) {
    let Ok(method) = Reflect::get(instance, &JsValue::from_str(name)) else {
        return;
    };
    let Some(function) = method.dyn_ref::<Function>() else {
        return;
    };
    let args = js_sys::Array::from_iter(arguments.iter().map(|value| (*value).clone()));
    let _ = function.apply(instance, &args);
}

/// Attaches the wasm formatter closures: the value-axis label renderer
/// and the hover tooltip. The builder emits one value-axis object and
/// pre-creates the `axisLabel` and `tooltip` targets, so a missing path
/// would mean the builder seam broke; the skip just leaves `ECharts`'s
/// default formatting.
fn attach_formatters(option: &JsValue, axis_labels: &JsValue, tooltip_html: &JsValue) {
    let Ok(y_axis) = Reflect::get(option, &JsValue::from_str("yAxis")) else {
        return;
    };
    set_child_property(&y_axis, "axisLabel", "formatter", axis_labels);
    set_child_property(option, "tooltip", "formatter", tooltip_html);
}

/// Sets `target[child_key].[property] = value`, skipping silently when
/// the builder's structure is missing (`ECharts` then keeps defaults).
fn set_child_property(target: &JsValue, child_key: &str, property: &str, value: &JsValue) {
    if let Ok(child) = Reflect::get(target, &JsValue::from_str(child_key)) {
        let _ = js_sys::Reflect::set(&child, &JsValue::from_str(property), value);
    }
}

/// Marks the SVG root `ECharts` rendered as the chart's accessible
/// image, preserving the aria-label contract the hand-rolled SVG held.
fn label_svg_root(container: &JsValue, label: &str) {
    let Ok(element) = container.clone().dyn_into::<web_sys::Element>() else {
        return;
    };
    if let Ok(Some(svg)) = element.query_selector("svg") {
        let _ = svg.set_attribute("role", "img");
        let _ = svg.set_attribute("aria-label", label);
    }
}

/// The value-axis label closure: raw tick value in, dual-unit string
/// out (`format_value` is the HA-unit-system formatter the card gets
/// as a prop).
fn axis_label_closure(
    format_value: fn(f64) -> String,
) -> Closure<dyn Fn(JsValue, JsValue) -> JsValue> {
    Closure::new(move |value: JsValue, _index: JsValue| {
        JsValue::from_str(&format_value(value.as_f64().unwrap_or(0.0)))
    })
}

/// The hover tooltip closure: one line of `value — time (UTC)` per
/// series entry, joined with line breaks. With the axis trigger
/// `ECharts` passes an array of series entries; the series data are
/// `[epoch ms, value]` pairs. Named entries (the pair scatter's input
/// and runoff) get a `name:` prefix; unnamed single-series charts keep
/// the plain `value — time` line they always showed.
fn tooltip_closure(
    format_value: fn(f64) -> String,
) -> Closure<dyn Fn(JsValue, JsValue, JsValue) -> JsValue> {
    Closure::new(
        move |params: JsValue, _ticket: JsValue, _callback: JsValue| {
            let mut lines: Vec<String> = Vec::new();
            if let Ok(entries) = params.dyn_into::<js_sys::Array>() {
                entries.for_each(&mut |entry: JsValue, _index, _array| {
                    if let Some(line) = tooltip_line(&entry, format_value) {
                        lines.push(line);
                    }
                });
            }
            JsValue::from_str(&lines.join("<br/>"))
        },
    )
}

/// One tooltip line for one series entry: `name: value — time` when
/// the series carries a name, `value — time` otherwise. A log-event
/// tick (the marker series) names its event instead — its y position
/// is placement geometry, not a reading. Entries without a usable
/// `[epoch ms, value]` pair drop out.
fn tooltip_line(entry: &JsValue, format_value: fn(f64) -> String) -> Option<String> {
    let value = Reflect::get(entry, &JsValue::from_str("value")).ok()?;
    let pair = value.dyn_into::<js_sys::Array>().ok()?;
    let at = pair.get(0).as_f64().unwrap_or(0.0);
    let reading = pair.get(1).as_f64().unwrap_or(0.0);
    let series_name = Reflect::get(entry, &JsValue::from_str("seriesName"))
        .ok()
        .and_then(|name| name.as_string())
        .filter(|name| !name.is_empty());
    if series_name.as_deref() == Some(chart_option::MARKER_SERIES_NAME) {
        let event = Reflect::get(entry, &JsValue::from_str("name"))
            .ok()
            .and_then(|name| name.as_string())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "Event".to_owned());
        return Some(format!("{event} — {}", utc_minutes(at)));
    }
    let body = format!("{} — {}", format_value(reading), utc_minutes(at));
    Some(match series_name {
        Some(name) => format!("{name}: {body}"),
        None => body,
    })
}

/// Formats an epoch-ms tooltip timestamp: `2026-11-16 14:03 UTC`.
#[must_use]
fn utc_minutes(epoch_ms: f64) -> String {
    let iso = js_sys::Date::new(&JsValue::from_f64(epoch_ms))
        .to_iso_string()
        .as_string()
        .unwrap_or_default();
    let date = iso.get(..16).unwrap_or_default().replace('T', " ");
    format!("{date} UTC")
}

/// Reads the active theme's colours off hidden probe elements so the
/// chart matches whichever daisyUI theme the browser picked. The
/// fallbacks are the light `leaflog` palette.
#[must_use]
fn theme_look() -> chart_option::ChartLook {
    chart_option::ChartLook {
        accent: probe_color("text-primary").unwrap_or_else(|| "#2e7d4f".to_owned()),
        compare: probe_color("text-secondary").unwrap_or_else(|| "#4c5f4e".to_owned()),
        text: probe_color("text-base-content").unwrap_or_else(|| "#1d2b1f".to_owned()),
        muted: probe_color("text-base-content/50").unwrap_or_else(|| "#1d2b1f80".to_owned()),
    }
}

/// Resolves one Tailwind colour class to its computed CSS colour by
/// mounting a throwaway probe span; `None` outside a browser context
/// or while the stylesheet has not applied yet (the unstyled initial
/// black is never a leaflog theme colour, so it means "styles not
/// ready" — the chart then draws with the light-palette fallbacks
/// instead of baking black chrome into the option).
fn probe_color(class: &str) -> Option<String> {
    let window = web_sys::window()?;
    let document = window.document()?;
    let probe = document.create_element("span").ok()?;
    probe.set_class_name(class);
    let parent = document.body()?;
    parent.append_child(&probe).ok()?;
    let colour = window
        .get_computed_style(&probe)
        .ok()
        .flatten()
        .and_then(|style| style.get_property_value("color").ok())
        .filter(|colour| !colour.is_empty() && colour != "rgb(0, 0, 0)");
    let _ = parent.remove_child(&probe);
    colour
}
