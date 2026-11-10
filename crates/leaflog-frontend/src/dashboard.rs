//! Environment, lamp and coach dashboard tiles (M7).

use crate::api;
use crate::equipment::ErrorIf;
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
        <div class="my-4 grid gap-3">
            <ErrorIf error />

            // Entity drift (spec §4.1): a dead binding shows a chip
            // and points at the entity-bindings editor below — never a
            // silent "in range".
            {move || {
                let dead = env
                    .get()
                    .map(|e| {
                        e.bindings
                            .into_iter()
                            .filter(|binding| !binding.available)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                (dead.is_empty()).then(move || {
                    view! {
                        <p class="alert alert-warning py-2 px-4 text-sm" role="status">
                            "No data: "
                            {dead
                                .into_iter()
                                .map(|binding| {
                                    view! {
                                        <span class="badge badge-ghost">
                                            {format!("{} ({})", binding.role, binding.entity_id)}
                                        </span>
                                    }
                                })
                                .collect::<Vec<_>>()}
                            " — rebind it in Entity bindings below."
                        </p>
                    }
                })
            }}

            <div class="grid gap-2 grid-cols-[repeat(auto-fill,minmax(9rem,1fr))]">
                {move || {
                    let Some(e) = env.get() else { return Vec::<_>::new() };
                    let temp_display = e.temperature_c.map_or_else(|| "—".to_owned(), leaflog_core::units::temp_both);
                    let rh_display = e.humidity_pct.map_or_else(|| "—".to_owned(), |h| format!("{h:.0} %"));
                    let vpd_display = e.vpd_kpa.map_or_else(|| "—".to_owned(), |v| format!("{v:.2} kPa"));
                    // Full class strings (Tailwind only compiles literals it
                    // can scan out of the sources; no format! fragments).
                    let temp_class = match e.temp_ok {
                        Some(true) => "rounded-box border border-success/50 bg-success/10 px-3 py-2 grid gap-0.5",
                        Some(false) => "rounded-box border border-warning/60 bg-warning/10 px-3 py-2 grid gap-0.5",
                        None => "rounded-box border border-base-300 bg-base-100 px-3 py-2 grid gap-0.5",
                    };
                    let vpd_class = match e.vpd_ok {
                        Some(true) => "rounded-box border border-success/50 bg-success/10 px-3 py-2 grid gap-0.5",
                        Some(false) => "rounded-box border border-warning/60 bg-warning/10 px-3 py-2 grid gap-0.5",
                        None => "rounded-box border border-base-300 bg-base-100 px-3 py-2 grid gap-0.5",
                    };
                    vec![
                        view! { <div class=temp_class>
                            <span class="text-xs uppercase tracking-wide text-base-content/60">"Temp"</span>
                            <span class="text-lg font-semibold">{temp_display}</span>
                        </div> }.into_any(),
                        view! { <div class="rounded-box border border-base-300 bg-base-100 px-3 py-2 grid gap-0.5">
                            <span class="text-xs uppercase tracking-wide text-base-content/60">"RH"</span>
                            <span class="text-lg font-semibold">{rh_display}</span>
                        </div> }.into_any(),
                        view! { <div class=vpd_class>
                            <span class="text-xs uppercase tracking-wide text-base-content/60">"VPD"</span>
                            <span class="text-lg font-semibold">{vpd_display}</span>
                            <span class="text-xs text-base-content/60">"band: "{format!("{:.1}–{:.1}", e.vpd_band.0, e.vpd_band.1)}</span>
                        </div> }.into_any(),
                        view! { <div class="rounded-box border border-base-300 bg-base-100 px-3 py-2 grid gap-0.5">
                            <span class="text-xs uppercase tracking-wide text-base-content/60">"RH window"</span>
                            <span class="text-lg font-semibold">{format!("{:.0}–{:.0} %", e.rh_window.0, e.rh_window.1)}</span>
                        </div> }.into_any(),
                    ]
                }}
            </div>

            {move || {
                lamp.get().map(|advice| {
                    view! {
                        <div class="card bg-base-100 shadow-sm px-4 py-3 my-3">
                            <h4>"Lamp height"</h4>
                            <p>{format!("{:.0}–{:.0} in / {:.0}–{:.0} cm from canopy", advice.height.min_in, advice.height.max_in, leaflog_core::units::inches_to_cm(advice.height.min_in), leaflog_core::units::inches_to_cm(advice.height.max_in))}</p>
                            {advice.dim_pct.map(|d| view! { <p class="text-base-content/70">"Dim to "{d}"%"</p> })}
                            <p class="text-base-content/70 text-sm">{advice.why}</p>
                        </div>
                    }
                })
            }}

            {move || {
                coach.get().map(|c| {
                    view! {
                        <div class="card bg-base-100 shadow-sm px-4 py-3 my-3">
                            <h4>{format!("Phase coach ({})", c.phase)}</h4>
                            <ul class="list-none m-0 p-0 grid gap-2">
                                {c.tips.iter().map(|tip| {
                                    view! {
                                        <li class="border-l-4 border-primary pl-3">
                                            <strong>{tip.title.clone()}</strong>
                                            <p class="text-base-content/70 text-sm">{tip.why.clone()}</p>
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
