//! Strain database page (spec §4.4): seeded catalogue, user records,
//! phenotype clones, create/edit form.

use std::collections::BTreeMap;

use leaflog_core::strain::{
    EvidenceTag, HeightClass, PhotoperiodType, SeedSex, Strain, StrainInput, Tolerance,
};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;

use crate::api;
use crate::route_href;

/// The strains page.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn StrainsPage() -> impl IntoView {
    let strains = RwSignal::<Vec<Strain>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);
    let form_open = RwSignal::<bool>::new(false);
    let editing_id = RwSignal::<Option<String>>::new(None);
    let draft = RwSignal::<StrainInput>::new(empty_strain());

    let refresh = move || {
        spawn_local(async move {
            match api::get::<Vec<Strain>>("api/strains").await {
                Ok(list) => strains.set(list),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };
    Effect::new(move |_| {
        refresh();
    });

    let start_create = move |_| {
        draft.set(empty_strain());
        editing_id.set(None);
        form_open.set(true);
    };

    let start_edit = move |strain: &Strain| {
        draft.set(StrainInput::from_strain(strain));
        editing_id.set(Some(strain.id.clone()));
        form_open.set(true);
    };

    // prevent_default first — see docs/DECISIONS.md D061.
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        spawn_local(async move {
            let input = draft.get();
            let result = match editing_id.get() {
                Some(id) => api::put::<StrainInput, Strain>(&format!("api/strains/{id}"), &input)
                    .await
                    .map(|_| ()),
                None => api::post::<StrainInput, Strain>("api/strains", &input)
                    .await
                    .map(|_| ()),
            };
            match result {
                Ok(()) => {
                    form_open.set(false);
                    error.set(None);
                    refresh();
                }
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    let clone = move |strain: &Strain| {
        let id = strain.id.clone();
        spawn_local(async move {
            match api::post_empty::<Strain>(&format!("api/strains/{id}/clone")).await {
                Ok(_) => refresh(),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    let remove = move |strain: &Strain| {
        let id = strain.id.clone();
        spawn_local(async move {
            match api::delete(&format!("api/strains/{id}")).await {
                Ok(()) => refresh(),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    view! {
        <section class="pb-12">
            <div class="flex flex-wrap items-center justify-between gap-4">
                <h2>"Strains"</h2>
                <button class="btn btn-primary btn-sm" on:click=start_create>"Add strain"</button>
            </div>

            <ErrorIf error />

            <Show when=move || form_open.get()>
                <form class="card bg-base-100 shadow-sm px-5 py-4 my-4" on:submit=save>
                    <h3>{move || if editing_id.get().is_some() { "Edit strain" } else { "New strain" }}</h3>
                    <StrainForm draft />
                    <div class="flex flex-wrap gap-2 mt-4">
                        <button class="btn btn-primary btn-sm" type="submit">"Save"</button>
                        <button class="btn btn-sm" type="button" on:click=move |_| form_open.set(false)>"Cancel"</button>
                    </div>
                </form>
            </Show>

            <ul class="grid gap-4 lg:grid-cols-2">
                {move || {
                    strains
                        .get()
                        .into_iter()
                        .map(|strain| {
                            let for_edit = strain.clone();
                            let for_clone = strain.clone();
                            let for_delete = strain.clone();
                            let bundled = strain.is_builtin;
                            view! {
                                <li class="card bg-base-100 shadow-sm px-5 py-4">
                                    <div class="flex flex-wrap items-baseline gap-2">
                                        <strong>{strain.name}</strong>
                                        <span class="badge badge-ghost">{strain.photoperiod.label()}</span>
                                        // Bundled records are read-only at the API; the badge
                                        // says so and edit/delete stay hidden (§4.4).
                                        {bundled
                                            .then(|| view! { <span class="badge badge-outline">"bundled"</span> })}
                                        {strain.breeder.map(|b| view! { <span class="text-base-content/70">{b}</span> })}
                                    </div>
                                    <dl class="facts">
                    {strain.flower_days_min.zip(strain.flower_days_max).map(|(min, max)| {
                        let evidence = evidence_label(&strain.evidence, "flower_days_min");
                        view! {
                            <div>
                                <dt>"Flowering" <span class="badge badge-xs badge-outline" title=evidence>{evidence}</span></dt>
                                <dd>{format!("{min}-{max} days")}</dd>
                            </div>
                        }
                    })}
                    {strain
                        .auto_total_days_min
                        .zip(strain.auto_total_days_max)
                        .map(|(min, max)| view! { <div><dt>"Seed to harvest"</dt><dd>{format!("{min}-{max} days")}</dd></div> })}
                    {strain.indica_pct.map(|pct| view! {
                        <div><dt>"Genetics"</dt><dd>{format!("{pct}% indica / {}% sativa", 100 - u32::from(pct))}</dd></div>
                    })}
                    {strain.height_class.map(|height| view! {
                        <div><dt>"Height"</dt><dd>{height.label()}</dd></div>
                    })}
                    {strain.stretch_factor.map(|stretch| view! {
                        <div><dt>"Stretch"</dt><dd>{format!("x{stretch:.1}")}</dd></div>
                    })}
                                        <div>
                                            <dt>"Heat / cold / light"</dt>
                                            <dd>
                                                {format!(
                                                    "{} / {} / {}",
                                                    strain.heat_tolerance.label(),
                                                    strain.cold_tolerance.label(),
                                                    strain.light_tolerance.label(),
                                                )}
                                                <span class="badge badge-xs badge-outline" title="inferred">"inferred"</span>
                                            </dd>
                                        </div>
                                    </dl>
                    {(!strain.landrace_tags.is_empty()).then(|| {
                        view! {
                            <p class="flex flex-wrap gap-1.5 mt-2">
                                {strain
                                    .landrace_tags
                                    .iter()
                                    .map(|tag| view! { <span class="badge badge-sm badge-ghost">{tag.clone()}</span> })
                                    .collect::<Vec<_>>()}
                            </p>
                        }
                    })}
                    {strain.notes.map(|notes| view! { <p class="mt-2 text-base-content/70">{notes}</p> })}
                    {strain.source_url.map(|url| view! {
                        <p class="text-base-content/70 text-sm">
                            "Source: " <a href=url target="_blank" rel="noreferrer">{source_host(&url)}</a>
                        </p>
                    })}
                                    <div class="flex flex-wrap gap-2 mt-3">
                                        {(!bundled)
                                            .then(|| view! { <button class="btn btn-sm" on:click=move |_| start_edit(&for_edit)>"Edit"</button> })}
                                        <button class="btn btn-sm" on:click=move |_| clone(&for_clone)>"Clone as my phenotype"</button>
                                        {(!bundled)
                                            .then(|| view! { <button class="btn btn-error btn-outline btn-sm" on:click=move |_| remove(&for_delete)>"Delete"</button> })}
                                    </div>
                                </li>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </ul>

            <Show when=move || strains.get().is_empty() && error.get().is_none()>
                <p class="py-6 text-center text-base-content/60">"No strains yet — add one, or clone a bundled record as your phenotype."</p>
            </Show>

            <p><A href=route_href("/")>"Back to home"</A></p>
        </section>
    }
}

/// The create/edit form for a strain.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
fn StrainForm(draft: RwSignal<StrainInput>) -> impl IntoView {
    view! {
        <div class="grid gap-3 grid-cols-[repeat(auto-fill,minmax(14rem,1fr))]">
            <label class="fieldset">
                <span class="fieldset-label">"Name *"</span>
                <input class="input" type="text" required
                    prop:value=move || draft.get().name
                    on:input=move |ev| draft.update(|d| d.name = event_target_value(&ev))
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Breeder"</span>
                <input class="input" type="text"
                    prop:value=move || draft.get().breeder.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.breeder = non_empty(event_target_value(&ev)))
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Type *"</span>
                <select class="select"
                    prop:value=move || draft.get().photoperiod.label().to_owned()
                    on:change=move |ev| {
                        draft.update(|d| {
                            d.photoperiod = if event_target_value(&ev) == "auto" {
                                PhotoperiodType::Auto
                            } else {
                                PhotoperiodType::Photoperiod
                            }
                        });
                    }
                >
                    <option value="photoperiod">"Photoperiod"</option>
                    <option value="auto">"Autoflower"</option>
                </select>
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Seeds *"</span>
                <select class="select"
                    prop:value=move || draft.get().sex.label().to_owned()
                    on:change=move |ev| {
                        draft.update(|d| {
                            d.sex = if event_target_value(&ev) == "regular" {
                                SeedSex::Regular
                            } else {
                                SeedSex::Feminized
                            }
                        });
                    }
                >
                    <option value="feminized">"Feminized"</option>
                    <option value="regular">"Regular"</option>
                </select>
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Indica %"</span>
                <input class="input" type="number" min="0" max="100" step="1"
                    prop:value=move || draft.get().indica_pct.map(|p| p.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.indica_pct = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Flower days (min)"</span>
                <input class="input" type="number" min="0" step="1"
                    prop:value=move || draft.get().flower_days_min.map(|v| v.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.flower_days_min = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Flower days (max)"</span>
                <input class="input" type="number" min="0" step="1"
                    prop:value=move || draft.get().flower_days_max.map(|v| v.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.flower_days_max = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Auto: seed-to-harvest days (min)"</span>
                <input class="input" type="number" min="0" step="1"
                    prop:value=move || draft.get().auto_total_days_min.map(|v| v.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.auto_total_days_min = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Auto: seed-to-harvest days (max)"</span>
                <input class="input" type="number" min="0" step="1"
                    prop:value=move || draft.get().auto_total_days_max.map(|v| v.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.auto_total_days_max = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Stretch factor (1.5 = +50%)"</span>
                <input class="input" type="number" min="1" step="0.1"
                    prop:value=move || draft.get().stretch_factor.map(|v| v.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.stretch_factor = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Veg weeks (min)"</span>
                <input class="input" type="number" min="0" step="1"
                    prop:value=move || draft.get().veg_weeks_min.map(|v| v.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.veg_weeks_min = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Veg weeks (max)"</span>
                <input class="input" type="number" min="0" step="1"
                    prop:value=move || draft.get().veg_weeks_max.map(|v| v.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.veg_weeks_max = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Height class"</span>
                <select class="select"
                    prop:value=move || draft.get().height_class.map(|h| h.label().to_owned()).unwrap_or_default()
                    on:change=move |ev| {
                        draft.update(|d| {
                            d.height_class = match event_target_value(&ev).as_str() {
                                "short" => Some(HeightClass::Short),
                                "medium" => Some(HeightClass::Medium),
                                "tall" => Some(HeightClass::Tall),
                                _ => None,
                            }
                        });
                    }
                >
                    <option value="">"Unknown"</option>
                    <option value="short">"Short"</option>
                    <option value="medium">"Medium"</option>
                    <option value="tall">"Tall"</option>
                </select>
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"User rating (0-5)"</span>
                <input class="input" type="number" min="0" max="5" step="1"
                    prop:value=move || draft.get().user_rating.map(|v| v.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.user_rating = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Heat tolerance"</span>
                <select class="select"
                    prop:value=move || draft.get().heat_tolerance.label().to_owned()
                    on:change=move |ev| {
                        draft.update(|d| d.heat_tolerance = tolerance_from_label(&event_target_value(&ev)));
                    }
                >
                    <option value="sensitive">"Sensitive"</option>
                    <option value="average">"Average"</option>
                    <option value="tolerant">"Tolerant"</option>
                </select>
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Cold tolerance"</span>
                <select class="select"
                    prop:value=move || draft.get().cold_tolerance.label().to_owned()
                    on:change=move |ev| {
                        draft.update(|d| d.cold_tolerance = tolerance_from_label(&event_target_value(&ev)));
                    }
                >
                    <option value="sensitive">"Sensitive"</option>
                    <option value="average">"Average"</option>
                    <option value="tolerant">"Tolerant"</option>
                </select>
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Light tolerance"</span>
                <select class="select"
                    prop:value=move || draft.get().light_tolerance.label().to_owned()
                    on:change=move |ev| {
                        draft.update(|d| d.light_tolerance = tolerance_from_label(&event_target_value(&ev)));
                    }
                >
                    <option value="sensitive">"Sensitive"</option>
                    <option value="average">"Average"</option>
                    <option value="tolerant">"Tolerant"</option>
                </select>
            </label>
            <label class="fieldset col-span-full">
                <span class="fieldset-label">"Lineage tags (comma separated)"</span>
                <input class="input" type="text" placeholder="haze, afghan"
                    prop:value=move || draft.get().landrace_tags.join(", ")
                    on:input=move |ev| {
                        draft.update(|d| {
                            d.landrace_tags = event_target_value(&ev)
                                .split(',')
                                .map(str::trim)
                                .filter(|tag| !tag.is_empty())
                                .map(str::to_owned)
                                .collect();
                        });
                    }
                />
            </label>
            <label class="fieldset col-span-full">
                <span class="fieldset-label">"Terpene notes"</span>
                <input class="input" type="text"
                    prop:value=move || draft.get().terpene_notes.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.terpene_notes = non_empty(event_target_value(&ev)))
                />
            </label>
            <label class="fieldset col-span-full">
                <span class="fieldset-label">"Effect notes"</span>
                <input class="input" type="text"
                    prop:value=move || draft.get().effect_notes.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.effect_notes = non_empty(event_target_value(&ev)))
                />
            </label>
            <label class="fieldset col-span-full">
                <span class="fieldset-label">"Yield notes"</span>
                <input class="input" type="text"
                    prop:value=move || draft.get().yield_notes.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.yield_notes = non_empty(event_target_value(&ev)))
                />
            </label>
            <label class="fieldset col-span-full">
                <span class="fieldset-label">"Notes"</span>
                <textarea class="textarea" rows="2"
                    prop:value=move || draft.get().notes.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.notes = non_empty(event_target_value(&ev)))
                />
            </label>
        </div>
    }
}

/// Parses a tolerance label from a select value.
fn tolerance_from_label(label: &str) -> Tolerance {
    match label {
        "sensitive" => Tolerance::Sensitive,
        "tolerant" => Tolerance::Tolerant,
        _ => Tolerance::Average,
    }
}

/// Shows the error banner when set.
#[component]
fn ErrorIf(error: RwSignal<Option<String>>) -> impl IntoView {
    move || {
        error.get().map(|message| {
            view! { <p class="alert alert-error py-2 px-4 text-sm" role="alert">{message}</p> }
        })
    }
}

/// Empty strings become `None` (API nulls).
fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

/// A blank strain draft.
const fn empty_strain() -> StrainInput {
    StrainInput {
        name: String::new(),
        breeder: None,
        photoperiod: PhotoperiodType::Photoperiod,
        sex: SeedSex::Feminized,
        indica_pct: None,
        landrace_tags: Vec::new(),
        flower_days_min: None,
        flower_days_max: None,
        stretch_factor: None,
        veg_weeks_min: None,
        veg_weeks_max: None,
        height_class: None,
        auto_total_days_min: None,
        auto_total_days_max: None,
        yield_notes: None,
        terpene_notes: None,
        effect_notes: None,
        heat_tolerance: Tolerance::Average,
        cold_tolerance: Tolerance::Average,
        light_tolerance: Tolerance::Average,
        user_rating: None,
        notes: None,
    }
}

/// Short evidence label for a field, "?" when untagged.
fn evidence_label(evidence: &BTreeMap<String, EvidenceTag>, field: &str) -> &'static str {
    match evidence.get(field) {
        Some(EvidenceTag::Breeder) => "breeder",
        Some(EvidenceTag::GrowerReported) => "grower-reported",
        Some(EvidenceTag::Inferred) => "inferred",
        Some(EvidenceTag::Unverified) | None => "unverified",
    }
}

/// Extracts a short host from a source URL for display.
fn source_host(url: &str) -> String {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(url)
        .to_owned()
}
