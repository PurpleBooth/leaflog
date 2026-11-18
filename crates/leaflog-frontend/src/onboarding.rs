//! First-run onboarding wizard (spec §4.12, final two-step shape):
//! bind sensors → plants (the grow's strain label and photoperiod sit
//! inline, D075), ending with a first grow running. Every step can be
//! skipped, the wizard gates nothing (all screens stay reachable), and
//! dismissing it leaves the normal dashboard.

use std::collections::BTreeMap;

use leaflog_core::grows::Grow;
use leaflog_core::grows::GrowInput;
use leaflog_core::grows::GrowStatus;
use leaflog_core::grows::PhotoperiodType;
use leaflog_core::grows::Tolerance;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;
use serde_json::json;

use crate::ErrorIf;
use crate::api;
use crate::bindings::BINDING_ROLES;
use crate::grows::photoperiod_options;
use crate::i18n::use_i18n;
use crate::non_empty;

/// The `GET /api/onboarding` state.
// The endpoint reports a snapshot of booleans by design; the flags are
// the whole payload.
#[derive(Debug, Clone, Copy, Default, Deserialize, Eq, PartialEq)]
pub struct OnboardingState {
    /// Whether the wizard should show now.
    pub show_wizard: bool,
    /// Whether the user dismissed it before.
    pub dismissed: bool,
    /// Whether any grow exists.
    pub has_grows: bool,
}

/// The wizard, mounted on the home page.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn OnboardingWizard() -> impl IntoView {
    let state = RwSignal::<Option<OnboardingState>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let step = RwSignal::<u8>::new(1);
    // Captured here, not inside spawn_local: the router hook needs the
    // component's reactive context. Stored in a Copy wrapper so the
    // handler closures below stay Copy (view children may re-render).
    let navigate = StoredValue::new_local(leptos_router::hooks::use_navigate());

    // Step 1: role → entity picks, applied to the grow created in the
    // last step (a binding hangs off a grow, so the write lands once
    // the grow exists — the picks here mirror the grow page's pickers).
    let picks = RwSignal::<BTreeMap<String, String>>::new(BTreeMap::new());
    let entities = RwSignal::<Vec<serde_json::Value>>::new(Vec::new());
    let plant_prefix = RwSignal::<String>::new(String::from("Plant"));
    let plant_count = RwSignal::<u32>::new(1);
    let grow_name = RwSignal::<String>::new(String::new());
    let strain_label = RwSignal::<String>::new(String::new());
    let photoperiod = RwSignal::<PhotoperiodType>::new(PhotoperiodType::default());
    let started_on = RwSignal::<String>::new(default_start());

    let refresh_state = move || {
        spawn_local(async move {
            match api::get::<OnboardingState>("api/onboarding").await {
                Ok(fetched) => state.set(Some(fetched)),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    Effect::new(move |_| {
        refresh_state();
        spawn_local(async move {
            if let Ok(value) = api::get::<serde_json::Value>("api/ha/entities").await {
                entities.set(value["entities"].as_array().cloned().unwrap_or_default());
            }
        });
    });

    let set_dismissed = move |value: bool| {
        spawn_local(async move {
            let body = json!({ "onboarding_dismissed": value });
            match api::put::<serde_json::Value, serde_json::Value>("api/settings", &body).await {
                Ok(_) => refresh_state(),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    // Every step can be skipped: "skip" just advances; finishing with
    // everything skipped only dismisses the wizard.
    let skip = move |_| {
        step.update(|current| *current = (*current).clamp(1, 2) + 1);
    };

    // prevent_default first: Leptos has no `on:submit:prevent` modifier
    // (an unknown `:word` suffix becomes a custom event that never
    // fires), so without this the browser submits the form natively and
    // reloads the page, losing everything (docs/DECISIONS.md D061).
    let finish = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let navigate = navigate.get_value();
        spawn_local(async move {
            // Step 3: the grow is the wizard's whole point, so skipping
            // it entirely just dismisses.
            let mut grow_id = None;
            if plant_count.get() > 0 {
                let prefix = {
                    let name = plant_prefix.get();
                    if name.trim().is_empty() {
                        "Plant".to_owned()
                    } else {
                        name
                    }
                };
                let input = GrowInput {
                    name: {
                        let name = grow_name.get();
                        non_empty(name).unwrap_or_else(|| format!("{prefix} run"))
                    },
                    feed_template_id: None,
                    planned_veg_weeks: None,
                    // The strain label is the free text the grow
                    // carries (D075); photoperiod sits beside it.
                    strain_label: strain_label.get().trim().to_owned(),
                    photoperiod: photoperiod.get_untracked(),
                    heat_tolerance: Tolerance::default(),
                    cold_tolerance: Tolerance::default(),
                    light_tolerance: Tolerance::default(),
                    real_watts: None,
                    dimmable: true,
                    substrate: "soil".to_owned(),
                    started_on: started_on.get(),
                    status: GrowStatus::Active,
                    notes: None,
                };
                match api::post::<GrowInput, Grow>("api/grows", &input).await {
                    Ok(grow) => {
                        for index in 1..=plant_count.get() {
                            let plant = json!({ "name": format!("{prefix} {index}") });
                            if let Err(failure) = api::post::<serde_json::Value, serde_json::Value>(
                                &format!("api/grows/{}/plants", grow.id),
                                &plant,
                            )
                            .await
                            {
                                error.set(Some(failure.message));
                                return;
                            }
                        }
                        // Step 1's sensor picks become the new grow's
                        // bindings; empty picks write nothing.
                        let bound = picks.get();
                        if !bound.is_empty()
                            && let Err(failure) = api::put::<serde_json::Value, serde_json::Value>(
                                &format!("api/grows/{}/bindings", grow.id),
                                &json!({ "bindings": bound }),
                            )
                            .await
                        {
                            error.set(Some(failure.message));
                            return;
                        }
                        grow_id = Some(grow.id);
                    }
                    Err(failure) => {
                        error.set(Some(failure.message));
                        return;
                    }
                }
            }
            // Finishing dismisses the wizard whatever happened above;
            // a created grow lands on its dashboard.
            let _ = api::put::<serde_json::Value, serde_json::Value>(
                "api/settings",
                &json!({ "onboarding_dismissed": true }),
            )
            .await;
            if let Some(id) = grow_id {
                navigate(
                    &format!("/grows/{id}"),
                    leptos_router::NavigateOptions::default(),
                );
            } else {
                refresh_state();
            }
        });
    };

    // Hidden once dismissed or done; a "set up" card restarts it after
    // a dismissal while there is still nothing to show.
    let visible = move || state.get().is_some_and(|state| state.show_wizard);
    let restartable = move || {
        state
            .get()
            .is_some_and(|state| state.dismissed && !state.has_grows)
    };

    // The step indicator class as a reactive value: `step` lights up
    // every step reached so far (a bare inline closure trips the view!
    // parser, so the closure is built here and called in the markup).
    let step_class = move |reached: u8| {
        let step = step;
        move || {
            if step.get() >= reached {
                "step step-primary"
            } else {
                "step"
            }
        }
    };

    view! {
        <Show when=visible>
            <section class="card bg-base-100 shadow-sm px-5 py-4 my-4" aria-label="First-run setup">
                <h2>"Welcome to Leaflog"</h2>
                <p class="text-base-content/70">
                    "Two quick steps to a first grow — skip any of them, everything stays
                     reachable from the tabs."
                </p>
                <ul class="steps my-2">
                    <li class=step_class(1)>"Bind sensors"</li>
                    <li class=step_class(2)>"Plants"</li>
                </ul>
                <ErrorIf error />

                <Show when=move || step.get() == 1>
                    <div>
                        <h3>"1. Bind sensors"</h3>
                        <p class="text-base-content/70">
                            "Pick a Home Assistant entity per role — the picks become this
                             grow's bindings. Everything optional; you can bind (or rebind)
                             on the grow page any time."
                        </p>
                        <div class="grid gap-3 grid-cols-[repeat(auto-fill,minmax(14rem,1fr))]">
                            {BINDING_ROLES
                                .iter()
                                .map(|(role, label)| {
                                    let role = *role;
                                    let current = picks;
                                    let options = entities;
                                    view! {
                                        <label class="fieldset">
                                            <span class="fieldset-label">{*label}</span>
                                            <select class="select"
                                                prop:value=move || {
                                                    current.get().get(role).cloned().unwrap_or_default()
                                                }
                                                on:change=move |ev| {
                                                    let value = event_target_value(&ev);
                                                    picks.update(|map| {
                                                        if value.is_empty() {
                                                            map.remove(role);
                                                        } else {
                                                            map.insert(role.to_owned(), value);
                                                        }
                                                    });
                                                }
                                            >
                                                <option value="">"— unbound —"</option>
                                                {move || {
                                                    options
                                                        .get()
                                                        .iter()
                                                        .map(|entity| {
                                                            let id = entity["entity_id"]
                                                                .as_str()
                                                                .unwrap_or_default()
                                                                .to_owned();
                                                            let name = entity["attributes"]["friendly_name"]
                                                                .as_str()
                                                                .unwrap_or(&id)
                                                                .to_owned();
                                                            view! {
                                                                <option value=id>
                                                                    {format!("{name} ({id})")}
                                                                </option>
                                                            }
                                                        })
                                                        .collect::<Vec<_>>()
                                                }}
                                            </select>
                                        </label>
                                    }
                                })
                                .collect::<Vec<_>>()}
                        </div>
                        <div class="flex flex-wrap gap-2 mt-4">
                            <button class="btn btn-sm" on:click=skip>"Skip this step"</button>
                            <button class="btn btn-primary btn-sm" on:click=skip>"Next"</button>
                        </div>
                    </div>
                </Show>

                <Show when=move || step.get() == 2>
                    <form on:submit=finish>
                        <h3>"2. Plants"</h3>
                        <p class="text-base-content/70">
                            "Finishing creates the grow (today counts as day one — change it
                             if the seeds went in earlier) and opens its dashboard."
                        </p>
                        <div class="grid gap-3 grid-cols-[repeat(auto-fill,minmax(14rem,1fr))]">
                            <label class="fieldset"><span class="fieldset-label">"Grow name"</span>
                                <input class="input" type="text" placeholder="First run"
                                    prop:value=move || grow_name.get()
                                    on:input=move |ev| grow_name.set(event_target_value(&ev))
                                />
                            </label>
                            <label class="fieldset"><span class="fieldset-label">"Strain label"</span>
                                <input class="input" type="text" placeholder="Northern Lights #5"
                                    prop:value=move || strain_label.get()
                                    on:input=move |ev| strain_label.set(event_target_value(&ev))
                                />
                            </label>
                            <label class="fieldset"><span class="fieldset-label">"Photoperiod"</span>
                                <select class="select"
                                    prop:value=move || photoperiod.get().label().to_owned()
                                    on:change=move |ev| {
                                        photoperiod.set(
                                            PhotoperiodType::from_label(&event_target_value(&ev))
                                                .unwrap_or_default(),
                                        );
                                    }
                                >
                                    {photoperiod_options(use_i18n().get_locale())}
                                </select>
                            </label>
                            <label class="fieldset"><span class="fieldset-label">"Name prefix"</span>
                                <input class="input" type="text"
                                    prop:value=move || plant_prefix.get()
                                    on:input=move |ev| plant_prefix.set(event_target_value(&ev))
                                />
                            </label>
                            <label class="fieldset"><span class="fieldset-label">"Plants"</span>
                                <input class="input" type="number" min="1" max="99" step="1"
                                    prop:value=move || plant_count.get().to_string()
                                    on:input=move |ev| {
                                        plant_count.set(event_target_value(&ev).parse().unwrap_or(1).clamp(1, 99));
                                    }
                                />
                            </label>
                            <label class="fieldset"><span class="fieldset-label">"Germination (day one)"</span>
                                <input class="input" type="date"
                                    prop:value=move || started_on.get()
                                    on:input=move |ev| started_on.set(event_target_value(&ev))
                                />
                            </label>
                        </div>
                        <div class="flex flex-wrap gap-2 mt-4">
                            <button class="btn btn-sm" type="button" on:click=move |_| step.set(1)>"Back"</button>
                            <button class="btn btn-sm btn-error btn-outline" type="button"
                                on:click=move |_| set_dismissed(true)
                            >
                                "Dismiss — I'll explore on my own"
                            </button>
                            <button class="btn btn-primary btn-sm" type="submit">"Start the grow"</button>
                        </div>
                    </form>
                </Show>
            </section>
        </Show>

        // The empty-state "set up" card: dismissed but still nothing
        // grown — one tap restarts the wizard (spec §4.12).
        <Show when=restartable>
            <section class="card bg-base-100 shadow-sm px-5 py-4 my-4">
                <h3>"Set up your first grow"</h3>
                <p class="text-base-content/70">"The setup wizard is available whenever you want it."</p>
                <button class="btn btn-primary btn-sm" on:click=move |_| set_dismissed(false)>
                    "Run the setup wizard"
                </button>
            </section>
        </Show>
    }
}

/// Today as `YYYY-MM-DD`: germination defaults to today (editable).
fn default_start() -> String {
    // The browser knows the wall clock; the backend stores this date
    // verbatim as the grow's day one.
    let now = js_sys::Date::new_0();
    format!(
        "{:04}-{:02}-{:02}",
        now.get_full_year(),
        now.get_month() + 1,
        now.get_date()
    )
}
