//! Environment, lamp and coach dashboard tiles (M7).

use crate::ErrorIf;
use crate::api;
use crate::i18n::{Locale, t, t_string, td_string, use_i18n};
use crate::labels;
use leaflog_core::Phase;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;

/// One binding's drift status.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BindingStatus {
    /// Binding role.
    pub role: String,
    /// Bound entity id.
    pub entity_id: String,
    /// Whether the entity currently reports data.
    pub available: bool,
}

/// The env snapshot from /api/grows/{id}/env.
#[derive(Debug, Clone, Deserialize)]
pub struct EnvSnapshot {
    /// Per-binding drift status (spec §4.1).
    pub bindings: Vec<BindingStatus>,
    /// Air temperature, °C.
    pub temperature_c: Option<f64>,
    /// Relative humidity, %.
    pub humidity_pct: Option<f64>,
    /// VPD, kPa.
    pub vpd_kpa: Option<f64>,
    /// VPD band for the phase.
    pub vpd_band: (f64, f64),
    /// RH window.
    pub rh_window: (f64, f64),
    /// Temperature thresholds; the live strip rechecks the streamed
    /// temperature against them.
    pub thresholds: Thresholds,
    /// Phase label (shown via coach; kept for future use)
    #[allow(dead_code)] // shown via coach; kept for future use
    pub phase: String,
    /// Whether temp is OK.
    pub temp_ok: Option<bool>,
    /// Whether VPD is OK.
    pub vpd_ok: Option<bool>,
}

/// Temperature thresholds.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Thresholds {
    /// Floor, °C (see `thresholds` above).
    pub floor_c: f64,
    /// Ceiling, °C.
    pub ceiling_c: f64,
}

/// Lamp advice from /api/grows/{id}/lamp.
#[derive(Debug, Clone, Deserialize)]
pub struct LampAdvice {
    /// Height range, inches.
    pub height: HeightRange,
    /// Dim recommendation.
    pub dim_pct: Option<u8>,
    /// Why (English sentence assembly; kept as fallback).
    pub why: String,
    /// One stable wire code per sentence of [`LampAdvice::why`]; the
    /// catalog keys these (`env.lamp_*`), unknown codes fall back to
    /// the English `why`.
    pub why_codes: Vec<String>,
}

/// Height range.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct HeightRange {
    /// Min, inches.
    pub min_in: f64,
    /// Max, inches.
    pub max_in: f64,
}

/// Coach response from /api/grows/{id}/coach.
#[derive(Debug, Clone, Deserialize)]
pub struct CoachResponse {
    /// Phase label.
    pub phase: String,
    /// Tips.
    pub tips: Vec<CoachTipData>,
    /// Equipment suggestions (will render as category chips).
    #[allow(dead_code)] // will render as category chips
    pub equipment_suggestions: Vec<String>,
    /// Light state the tips were computed for, so the frontend can
    /// localize the VPD lights-on/off wording.
    pub light_on: bool,
}

/// One coach tip.
#[derive(Debug, Clone, Deserialize)]
pub struct CoachTipData {
    /// Title (English; kept as fallback for unknown codes).
    pub title: String,
    /// Why (English; kept as fallback for unknown codes).
    pub why: String,
    /// Stable wire code keying the tip into the catalog (`env.coach_*`).
    pub code: String,
}

/// The lamp advice why-sentence, reassembled from the wire codes; an
/// unknown code falls back to the English assembly.
fn lamp_why(locale: Locale, advice: &LampAdvice) -> String {
    let mut out = String::new();
    for code in &advice.why_codes {
        let line = match code.as_str() {
            "dimmed_closer" => td_string!(locale, env.lamp_dimmed_closer).to_owned(),
            "canopy_hot_dim" => td_string!(locale, env.lamp_canopy_hot_dim).to_owned(),
            "canopy_hot_raise" => td_string!(locale, env.lamp_canopy_hot_raise).to_owned(),
            "stretch_raise" => td_string!(locale, env.lamp_stretch_raise).to_owned(),
            "stress_dim" => td_string!(locale, env.lamp_stress_dim).to_owned(),
            "stress_raise" => td_string!(locale, env.lamp_stress_raise).to_owned(),
            "light_sensitive" => td_string!(locale, env.lamp_light_sensitive).to_owned(),
            "light_tolerant" => td_string!(locale, env.lamp_light_tolerant).to_owned(),
            "default_starting_points" | "starting_points" => {
                td_string!(locale, env.lamp_starting_points).to_owned()
            }
            // Unknown wire code: keep the backend's English assembly.
            _ => return advice.why.clone(),
        };
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&line);
    }
    out
}

/// One coach tip localized from its wire code; the VPD tip rebuilds
/// its band numbers from the core table and takes its lights-on/off
/// wording from the response. Unknown codes keep the English title/why.
fn coach_tip_lines(
    locale: Locale,
    phase: Option<Phase>,
    tip: &CoachTipData,
    light_on: bool,
) -> (String, String) {
    match tip.code.as_str() {
        "vpd_band" => {
            let Some(phase) = phase else {
                return (tip.title.clone(), tip.why.clone());
            };
            let (min, max) = leaflog_core::env::vpd_band(phase);
            let state = if light_on {
                td_string!(locale, env.coach_vpd_when_on).to_owned()
            } else {
                td_string!(locale, env.coach_vpd_when_off).to_owned()
            };
            let title = format!(
                "{}{}",
                td_string!(
                    locale,
                    env.coach_vpd_title,
                    min = format!("{min:.1}"),
                    max = format!("{max:.1}")
                ),
                state
            );
            (title, td_string!(locale, env.coach_vpd_why).to_owned())
        }
        "coco_fertigate" => (
            td_string!(locale, env.coach_coco_fertigate).to_owned(),
            td_string!(locale, env.coach_coco_fertigate_why).to_owned(),
        ),
        "coco_runoff_ec" => (
            td_string!(locale, env.coach_coco_runoff_ec).to_owned(),
            td_string!(locale, env.coach_coco_runoff_ec_why).to_owned(),
        ),
        "hydro_reservoir" => (
            td_string!(locale, env.coach_hydro_reservoir).to_owned(),
            td_string!(locale, env.coach_hydro_reservoir_why).to_owned(),
        ),
        "soil_wet_dry" => (
            td_string!(locale, env.coach_soil_wet_dry).to_owned(),
            td_string!(locale, env.coach_soil_wet_dry_why).to_owned(),
        ),
        "flower_late_rh" => (
            td_string!(locale, env.coach_flower_late_rh).to_owned(),
            td_string!(locale, env.coach_flower_late_rh_why).to_owned(),
        ),
        "flower_stretch" => (
            td_string!(locale, env.coach_flower_stretch).to_owned(),
            td_string!(locale, env.coach_flower_stretch_why).to_owned(),
        ),
        "seedling_rh" => (
            td_string!(locale, env.coach_seedling_rh).to_owned(),
            td_string!(locale, env.coach_seedling_rh_why).to_owned(),
        ),
        "drying_conditions" => (
            td_string!(locale, env.coach_drying_conditions).to_owned(),
            td_string!(locale, env.coach_drying_conditions_why).to_owned(),
        ),
        "curing_jars" => (
            td_string!(locale, env.coach_curing_jars).to_owned(),
            td_string!(locale, env.coach_curing_jars_why).to_owned(),
        ),
        _ => (tip.title.clone(), tip.why.clone()),
    }
}

/// The combined env + lamp + coach dashboard for one grow.
#[component]
#[allow(clippy::too_many_lines)]
// declarative view code, not logic; see docs/DECISIONS.md
// Component prop; Leptos takes owned props.
#[allow(clippy::needless_pass_by_value)] // Leptos component prop contract; see docs/DECISIONS.md
pub fn EnvDashboard(grow_id: String) -> impl IntoView {
    let i18n = use_i18n();
    let env = RwSignal::<Option<EnvSnapshot>>::new(None);
    let lamp = RwSignal::<Option<LampAdvice>>::new(None);
    let coach = RwSignal::<Option<CoachResponse>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    // One EventSource per page: the env strip renders the streamed
    // values on top of the fetched snapshot (live values, ticket 02).
    let live = crate::live::subscribe_live();
    // The Effect below owns `grow_id`; the banner link targets the
    // edit page (route_href: path-absolute hrefs break under ingress).
    let edit_href = crate::route_href(&format!("/grows/{grow_id}/edit"));

    Effect::new(move |_| {
        let grow_id = grow_id.clone();
        spawn_local(async move {
            match api::get::<EnvSnapshot>(&format!("api/grows/{grow_id}/env")).await {
                Ok(data) => env.set(Some(data)),
                Err(failure) => error.set(Some(failure.message)),
            }
            match api::get::<LampAdvice>(&format!("api/grows/{grow_id}/lamp")).await {
                Ok(data) => lamp.set(Some(data)),
                Err(failure) => error.set(Some(failure.message)),
            }
            match api::get::<CoachResponse>(&format!("api/grows/{grow_id}/coach")).await {
                Ok(data) => coach.set(Some(data)),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    });

    view! {
        <div class="my-4 grid gap-3">
            <ErrorIf error />

            // Entity drift (spec §4.1): a dead binding shows a chip
            // and points at the grow edit page — never a silent "in
            // range". The banner tells the truth: it renders only once
            // the snapshot is loaded AND a binding is actually dead —
            // no flash while the fetch is in flight, and a healthy
            // grow stays quiet.
            {move || {
                // `?` on the Option signal: no snapshot, no banner —
                // nothing flashes while the fetch is in flight.
                let snapshot = env.get()?;
                let dead = snapshot
                    .bindings
                    .into_iter()
                    .filter(|binding| !binding.available)
                    .collect::<Vec<_>>();
                // The link href is cloned into the render closure so
                // this outer closure stays `FnMut` (Leptos may rerun it).
                let edit_href = edit_href.clone();
                (!dead.is_empty()).then(move || {
                    view! {
                        <p class="alert alert-warning py-2 px-4 text-sm" role="status">
                            {t!(i18n, env.no_data_prefix)}
                            {dead
                                .into_iter()
                                .map(|binding| {
                                    // The badge is where a bound entity id
                                    // renders as text, so it carries the
                                    // wrap-anywhere treatment (the long-id
                                    // regression, ticket 10) and the full
                                    // id on hover.
                                    let entity_id = binding.entity_id.clone();
                                    let role = labels::chart_title(
                                        i18n.get_locale(),
                                        &binding.role,
                                    );
                                    view! {
                                        <span class="badge badge-ghost entity-id"
                                            title=entity_id
                                        >
                                            {format!("{} ({role})", binding.role)}
                                        </span>
                                    }
                                })
                                .collect::<Vec<_>>()}
                            {t!(i18n, env.rebind_intro)}
                            <a class="link" href=edit_href>{t!(i18n, env.rebind_link)}</a>
                            "."
                        </p>
                    }
                })
            }}

            <div id="env-strip" class="grid gap-2 grid-cols-1 min-[360px]:grid-cols-2 lg:grid-cols-6">
                {move || {
                    let Some(e) = env.get() else { return Vec::<_>::new() };
                    // Streamed values override the snapshot's; without a
                    // live reading the backend values stand (live.rs).
                    // The tile colours read the same merged values, so
                    // they track the live stream too.
                    let v = crate::live::strip_values(&e, live.get().as_ref());
                    let temp_display = v.temperature_c.map_or_else(|| "—".to_owned(), leaflog_core::units::temp_both);
                    let rh_display = v.humidity_pct.map_or_else(|| "—".to_owned(), crate::chart::format_pct);
                    let vpd_display = v.vpd_kpa.map_or_else(|| "—".to_owned(), crate::chart::format_kpa);
                    let co2_display = v.co2_ppm.map_or_else(|| "—".to_owned(), crate::chart::format_ppm);
                    let soil_display = v.soil_moisture_pct.map_or_else(|| "—".to_owned(), crate::chart::format_pct);
                    // The colours come from the merged StripValues, not
                    // the page-load snapshot: a live push that crosses
                    // the threshold recolours the tile on the spot.
                    let temp_class = crate::live::tile_class(v.temp_ok);
                    let vpd_class = crate::live::tile_class(v.vpd_ok);
                    vec![
                        view! { <div class=temp_class>
                            <span class="text-xs uppercase tracking-wide text-base-content/60">{t!(i18n, env.temp)}</span>
                            <span class="text-lg font-semibold">{temp_display}</span>
                        </div> }.into_any(),
                        view! { <div class="rounded-box border border-base-300 bg-base-100 px-3 py-2 grid gap-0.5">
                            <span class="text-xs uppercase tracking-wide text-base-content/60">{t!(i18n, env.rh)}</span>
                            <span class="text-lg font-semibold">{rh_display}</span>
                        </div> }.into_any(),
                        view! { <div class=vpd_class>
                            <span class="text-xs uppercase tracking-wide text-base-content/60">{t!(i18n, env.vpd_tile)}</span>
                            <span class="text-lg font-semibold">{vpd_display}</span>
                            <span class="text-xs text-base-content/60">{t!(i18n, env.band_prefix)}{format!("{:.1}–{:.1}", e.vpd_band.0, e.vpd_band.1)}</span>
                        </div> }.into_any(),
                        view! { <div class="rounded-box border border-base-300 bg-base-100 px-3 py-2 grid gap-0.5">
                            <span class="text-xs uppercase tracking-wide text-base-content/60">{t!(i18n, env.rh_window)}</span>
                            <span class="text-lg font-semibold">{format!("{:.0}–{:.0} %", v.rh_window.0, v.rh_window.1)}</span>
                        </div> }.into_any(),
                        view! { <div class="rounded-box border border-base-300 bg-base-100 px-3 py-2 grid gap-0.5">
                            <span class="text-xs uppercase tracking-wide text-base-content/60">{t!(i18n, env.co2_tile)}</span>
                            <span class="text-lg font-semibold">{co2_display}</span>
                        </div> }.into_any(),
                        view! { <div class="rounded-box border border-base-300 bg-base-100 px-3 py-2 grid gap-0.5">
                            <span class="text-xs uppercase tracking-wide text-base-content/60">{t!(i18n, env.soil_tile)}</span>
                            <span class="text-lg font-semibold">{soil_display}</span>
                        </div> }.into_any(),
                    ]
                }}
            </div>

            // Lamp advice and phase coach pair on wide screens — two
            // cards with one short fact each stop wasting a full row
            // of whitespace each. Explicit base track, same reasoning
            // as the history grid in grows.rs.
            <div class="grid grid-cols-1 gap-4 items-start lg:grid-cols-2">
                {move || {
                    lamp.get().map(|advice| {
                        let locale = i18n.get_locale();
                        view! {
                            <div class="card bg-base-100 shadow-sm px-4 py-3">
                                <h4>{t!(i18n, env.lamp_height)}</h4>
                                <p>{t_string!(
                                    i18n,
                                    env.lamp_range,
                                    min = format!("{:.0}", advice.height.min_in),
                                    max = format!("{:.0}", advice.height.max_in),
                                    cm_min = format!("{:.0}", leaflog_core::units::inches_to_cm(advice.height.min_in)),
                                    cm_max = format!("{:.0}", leaflog_core::units::inches_to_cm(advice.height.max_in)),
                                )}</p>
                                {advice.dim_pct.map(|d| view! { <p class="text-base-content/70">{t_string!(i18n, env.dim_to, pct = d.to_string())}</p> })}
                                <p class="text-base-content/70 text-sm">{lamp_why(locale, &advice)}</p>
                            </div>
                        }
                    })
                }}

                {move || {
                    coach.get().map(|c| {
                        let locale = i18n.get_locale();
                        let phase = crate::phase_pick::phase_from_label(&c.phase);
                        let heading = phase.map_or_else(
                            || c.phase.clone(),
                            |known| labels::phase_label(locale, known),
                        );
                        view! {
                            <div class="card bg-base-100 shadow-sm px-4 py-3">
                                <h4>{t_string!(i18n, env.coach_heading, phase = heading)}</h4>
                                // Hairline dividers between tips: a loud
                                // colored border per tip is chrome shouting
                                // where quiet structure reads the list.
                                <ul class="list-none m-0 p-0 divide-y divide-base-300">
                                    {c.tips.iter().map(|tip| {
                                        let (title, why) =
                                            coach_tip_lines(locale, phase, tip, c.light_on);
                                        view! {
                                            <li class="py-2 first:pt-0 last:pb-0">
                                                <strong>{title}</strong>
                                                <p class="text-base-content/70 text-sm">{why}</p>
                                            </li>
                                        }
                                    }).collect::<Vec<_>>()}
                                </ul>
                            </div>
                        }
                    })
                }}
            </div>
        </div>
    }
}
