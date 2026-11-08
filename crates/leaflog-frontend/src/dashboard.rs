//! Environment, lamp and coach dashboard tiles (M7).

use crate::api;
use crate::equipment::ErrorIf;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;

/// The env snapshot from /api/grows/{id}/env.
#[derive(Debug, Clone, Deserialize)]
pub struct EnvSnapshot {
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
    /// Temperature thresholds (deserialized; shown in tooltips).
    #[allow(dead_code)] // will display as tooltip when the UI adds them
    pub thresholds: Thresholds,
    /// Phase label (shown in the section heading).
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
    #[allow(dead_code)]
    pub floor_c: f64,
    /// Ceiling, °C.
    #[allow(dead_code)]
    pub ceiling_c: f64,
}

/// Lamp advice from /api/grows/{id}/lamp.
#[derive(Debug, Clone, Deserialize)]
pub struct LampAdvice {
    /// Height range, inches.
    pub height: HeightRange,
    /// Dim recommendation.
    pub dim_pct: Option<u8>,
    /// Why.
    pub why: String,
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
}

/// One coach tip.
#[derive(Debug, Clone, Deserialize)]
pub struct CoachTipData {
    /// Title.
    pub title: String,
    /// Why.
    pub why: String,
}

/// The combined env + lamp + coach dashboard for one grow.
#[component]
#[allow(clippy::too_many_lines)]
// declarative view code, not logic; see docs/DECISIONS.md
// Component prop; Leptos takes owned props.
#[allow(clippy::needless_pass_by_value)] // Leptos component prop contract; see docs/DECISIONS.md
pub fn EnvDashboard(grow_id: String) -> impl IntoView {
    let env = RwSignal::<Option<EnvSnapshot>>::new(None);
    let lamp = RwSignal::<Option<LampAdvice>>::new(None);
    let coach = RwSignal::<Option<CoachResponse>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);

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
        <div class="env-dashboard">
            <ErrorIf error />

            <div class="tiles">
                {move || {
                    let Some(e) = env.get() else { return Vec::<_>::new() };
                    let temp_display = e.temperature_c.map_or_else(|| "—".to_owned(), leaflog_core::units::temp_both);
                    let rh_display = e.humidity_pct.map_or_else(|| "—".to_owned(), |h| format!("{h:.0} %"));
                    let vpd_display = e.vpd_kpa.map_or_else(|| "—".to_owned(), |v| format!("{v:.2} kPa"));
                    let temp_class = e.temp_ok.map_or("", |ok| if ok { "tile-ok" } else { "tile-warn" });
                    let vpd_class = e.vpd_ok.map_or("", |ok| if ok { "tile-ok" } else { "tile-warn" });
                    vec![
                        view! { <div class=format!("tile {temp_class}")>
                            <span class="tile-label">"Temp"</span>
                            <span class="tile-value">{temp_display}</span>
                        </div> }.into_any(),
                        view! { <div class="tile">
                            <span class="tile-label">"RH"</span>
                            <span class="tile-value">{rh_display}</span>
                        </div> }.into_any(),
                        view! { <div class=format!("tile {vpd_class}")>
                            <span class="tile-label">"VPD"</span>
                            <span class="tile-value">{vpd_display}</span>
                            <span class="tile-sub">"band: "{format!("{:.1}–{:.1}", e.vpd_band.0, e.vpd_band.1)}</span>
                        </div> }.into_any(),
                        view! { <div class="tile">
                            <span class="tile-label">"RH window"</span>
                            <span class="tile-value">{format!("{:.0}–{:.0} %", e.rh_window.0, e.rh_window.1)}</span>
                        </div> }.into_any(),
                    ]
                }}
            </div>

            {move || {
                lamp.get().map(|advice| {
                    view! {
                        <div class="card slim">
                            <h4>"Lamp height"</h4>
                            <p>{format!("{:.0}–{:.0} in / {:.0}–{:.0} cm from canopy", advice.height.min_in, advice.height.max_in, leaflog_core::units::inches_to_cm(advice.height.min_in), leaflog_core::units::inches_to_cm(advice.height.max_in))}</p>
                            {advice.dim_pct.map(|d| view! { <p class="muted">"Dim to "{d}"%"</p> })}
                            <p class="muted small">{advice.why}</p>
                        </div>
                    }
                })
            }}

            {move || {
                coach.get().map(|c| {
                    view! {
                        <div class="card slim">
                            <h4>{format!("Phase coach ({})", c.phase)}</h4>
                            <ul class="coach-tips">
                                {c.tips.iter().map(|tip| {
                                    view! {
                                        <li>
                                            <strong>{tip.title.clone()}</strong>
                                            <p class="muted small">{tip.why.clone()}</p>
                                        </li>
                                    }
                                }).collect::<Vec<_>>()}
                            </ul>
                        </div>
                    }
                })
            }}
        </div>
    }
}
