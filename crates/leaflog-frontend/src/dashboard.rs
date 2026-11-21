//! Environment, lamp and coach dashboard tiles (M7).

use crate::ErrorIf;
use crate::api;
use crate::bindings::{ROLE_CO2, ROLE_HUMIDITY, ROLE_SOIL_MOISTURE, ROLE_TEMPERATURE};
use crate::i18n::{Locale, t, t_string, td_string, use_i18n};
use crate::labels;
use crate::live::{dead_entity, tile_class};
use crate::ui;
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
            // The VPD tip's why sentence is gone (D137): the title
            // carries the whole message, so no why line renders.
            (title, String::new())
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
            String::new(),
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
/// One environment-strip tile: the uppercase micro-label, the big
/// value, and an optional quiet band footnote. The caller decides the
/// tile recipe (plain [`ui::tile`] vs the live threshold colouring).
/// A dead binding renders a small warning mark beside the label
/// (spec §4.1's "no data, never in range", surfaced per tile since
/// D146): the mark is a link to the grow edit page — where the
/// binding flow lives — and its tooltip names the dead entity id,
/// so the data the tile is missing stays one hover away.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
fn EnvTile(
    /// Full class list for the tile div (recipe + colouring).
    class: String,
    /// The uppercase micro-label caption.
    label: String,
    /// The merged or snapshot value, already formatted.
    value: String,
    /// Optional corridor footnote under the value (D136).
    #[prop(optional)]
    band: Option<String>,
    /// The dead binding's entity id, when this tile's role is bound
    /// but its entity currently reports no data. `None` renders no
    /// mark — always passed (the strip computes it per role).
    dead_entity: Option<String>,
    /// Where the warning mark leads: the grow edit page, where the
    /// binding flow lives.
    edit_href: String,
) -> impl IntoView {
    let i18n = use_i18n();
    // The mark builds before the view: its accessible name needs the
    // localised hint with the entity id baked in, and the closure
    // clones what it captures so the outer render stays `FnMut`.
    let mark = dead_entity.map(|entity_id| {
        let edit_href = edit_href.clone();
        // The hint closes over its own clone of the id: two attribute
        // closures (title + aria-label) share one moved String.
        let hint_entity = entity_id.clone();
        view! {
            <a
                class="link link-warning text-xs font-bold ml-1"
                title={move || t_string!(i18n, env.dead_binding_hint, entity = hint_entity.clone())}
                aria-label={move || t_string!(i18n, env.dead_binding_hint, entity = entity_id.clone())}
                href=edit_href
            >
                "⚠"
            </a>
        }
    });
    view! {
        <div class=class>
            <span class="text-xs uppercase tracking-wide text-base-content/60">{label}</span>
            {mark}
            <span class="text-lg font-semibold">{value}</span>
            {band.map_or_else(
                || ().into_any(),
                |band| view! { <span class="text-xs text-base-content/60">{band}</span> }.into_any(),
            )}
        </div>
    }
}

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
    // The Effect below owns `grow_id`; the dead-binding tile marks
    // link to the edit page (route_href: path-absolute hrefs break
    // under ingress).
    let edit_href = crate::route_href(&format!("/grows/{grow_id}/edit"));

    Effect::new(move |_| {
        let grow_id = grow_id.clone();
        spawn_local(async move {
            match api::get::<EnvSnapshot>(&format!("api/grows/{grow_id}/env")).await {
                Ok(data) => env.set(Some(data)),
                Err(failure) => error.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
            }
            match api::get::<LampAdvice>(&format!("api/grows/{grow_id}/lamp")).await {
                Ok(data) => lamp.set(Some(data)),
                Err(failure) => error.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
            }
            match api::get::<CoachResponse>(&format!("api/grows/{grow_id}/coach")).await {
                Ok(data) => coach.set(Some(data)),
                Err(failure) => error.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
            }
        });
    });

    view! {
        <div class="my-4 grid gap-4">
            <ErrorIf error />

            <div id="env-strip" class="grid gap-2 grid-cols-1 min-[360px]:grid-cols-2 lg:grid-cols-5">
                {move || {
                    let Some(e) = env.get() else { return Vec::<_>::new() };
                    // Streamed values override the snapshot's; without a
                    // live reading the backend values stand (live.rs).
                    // The tile colours read the same merged values, so
                    // they track the live stream too.
                    let v = crate::live::strip_values(&e, live.get().as_ref());
                    let style = crate::decimal_style(i18n.get_locale());
                    let temp_display = v.temperature_c
                        .map_or_else(|| "—".to_owned(), |value| leaflog_core::units::temp_both(value, style));
                    let rh_display = v.humidity_pct
                        .map_or_else(|| "—".to_owned(), |value| crate::chart::format_pct(value, style));
                    let vpd_display = v.vpd_kpa
                        .map_or_else(|| "—".to_owned(), |value| crate::chart::format_kpa(value, style));
                    let co2_display = v.co2_ppm
                        .map_or_else(|| "—".to_owned(), |value| crate::chart::format_ppm(value, style));
                    let soil_display = v.soil_moisture_pct
                        .map_or_else(|| "—".to_owned(), |value| crate::chart::format_pct(value, style));
                    // The colours come from the merged StripValues, not
                    // the page-load snapshot: a live push that crosses
                    // the threshold recolours the tile on the spot.
                    let temp_class = tile_class(v.temp_ok);
                    let vpd_class = tile_class(v.vpd_ok);
                    // Entity drift, per tile (spec §4.1, D146): a role
                    // whose bound entity is dead marks its own tile
                    // instead of a page-wide banner. VPD derives from
                    // temperature + humidity, so either of those marks
                    // explains its missing value; unbound roles are
                    // the not-measured card's word, not dead data.
                    let dead_temp = dead_entity(&e.bindings, ROLE_TEMPERATURE);
                    let dead_humidity = dead_entity(&e.bindings, ROLE_HUMIDITY);
                    let dead_co2 = dead_entity(&e.bindings, ROLE_CO2);
                    let dead_soil = dead_entity(&e.bindings, ROLE_SOIL_MOISTURE);
                    // The phase's RH window rides under the LF reading
                    // the same way the VPD band rides under VPD (D136):
                    // one tile per quantity, the corridor as a footnote.
                    let rh_band = format!(
                        "{}{}–{} %",
                        t_string!(i18n, env.band_prefix),
                        leaflog_core::format::format_decimal(v.rh_window.0, 0, style),
                        leaflog_core::format::format_decimal(v.rh_window.1, 0, style)
                    );
                    let vpd_band = format!(
                        "{}{}–{}",
                        t_string!(i18n, env.band_prefix),
                        leaflog_core::format::format_decimal(e.vpd_band.0, 1, style),
                        leaflog_core::format::format_decimal(e.vpd_band.1, 1, style)
                    );
                    vec![
                        view! {
                            <EnvTile
                                class=temp_class.to_string()
                                label=t_string!(i18n, env.temp).into_owned()
                                value=temp_display
                                dead_entity=dead_temp
                                edit_href=edit_href.clone()
                            />
                        }
                            .into_any(),
                        view! {
                            <EnvTile
                                class=format!("{} grid gap-0.5", ui::tile())
                                label=t_string!(i18n, env.rh).into_owned()
                                value=rh_display
                                band=rh_band
                                dead_entity=dead_humidity
                                edit_href=edit_href.clone()
                            />
                        }
                            .into_any(),
                        view! {
                            <EnvTile
                                class=vpd_class.to_string()
                                label=t_string!(i18n, env.vpd_tile).into_owned()
                                value=vpd_display
                                band=vpd_band
                                dead_entity=None
                                edit_href=edit_href.clone()
                            />
                        }
                            .into_any(),
                        view! {
                            <EnvTile
                                class=format!("{} grid gap-0.5", ui::tile())
                                label=t_string!(i18n, env.co2_tile).into_owned()
                                value=co2_display
                                dead_entity=dead_co2
                                edit_href=edit_href.clone()
                            />
                        }
                            .into_any(),
                        view! {
                            <EnvTile
                                class=format!("{} grid gap-0.5", ui::tile())
                                label=t_string!(i18n, env.soil_tile).into_owned()
                                value=soil_display
                                dead_entity=dead_soil
                                edit_href=edit_href.clone()
                            />
                        }
                            .into_any(),
                    ]
                }}
            </div>

            // Lamp advice and phase coach pair on wide screens — two
            // cards with one short fact each stop wasting a full row
            // of whitespace each. Explicit base track, same reasoning
            // as the history grid in grows.rs.
            <div class=ui::CARD_GRID>
                {move || {
                    lamp.get().map(|advice| {
                        let locale = i18n.get_locale();
                        view! {
                            <div class=ui::card()>
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
                            <div class=ui::card()>
                                <h4>{t_string!(i18n, env.coach_heading, phase = heading)}</h4>
                                // Hairline dividers between tips: a loud
                                // colored border per tip is chrome shouting
                                // where quiet structure reads the list.
                                <ul class="list-none m-0 p-0 divide-y divide-base-300">
                                    {c.tips.iter().map(|tip| {
                                        let (title, why) =
                                            coach_tip_lines(locale, phase, tip, c.light_on);
                                        // Tips whose why sentence is gone
                                        // (D137) render the title only —
                                        // no empty paragraph gap.
                                        let why_line = (!why.is_empty()).then(|| {
                                            view! {
                                                <p class="text-base-content/70 text-sm">{why}</p>
                                            }
                                        });
                                        view! {
                                            <li class="py-2 first:pt-0 last:pb-0">
                                                <strong>{title}</strong>
                                                {why_line}
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
