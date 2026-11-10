//! First-run onboarding wizard (spec §4.12): equipment → grow space →
//! strain → plants, ending with a first grow running. Every step can be
//! skipped, the wizard gates nothing (all screens stay reachable), and
//! dismissing it leaves the normal dashboard.

use leaflog_core::equipment::EquipmentItem;
use leaflog_core::grows::Grow;
use leaflog_core::grows::GrowInput;
use leaflog_core::grows::GrowStatus;
use leaflog_core::seedbank::SeedBatchView;
use leaflog_core::spaces::GrowSpace;
use leaflog_core::spaces::GrowSpaceInput;
use leaflog_core::strain::Strain;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;
use serde_json::json;

use crate::api;
use crate::equipment::ErrorIf;
use crate::non_empty;

/// The `GET /api/onboarding` state.
// The endpoint reports a snapshot of booleans by design; the flags are
// the whole payload.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, Deserialize, Eq, PartialEq)]
pub struct OnboardingState {
    /// Whether the wizard should show now.
    pub show_wizard: bool,
    /// Whether the user dismissed it before.
    pub dismissed: bool,
    /// Whether any grow space exists.
    pub has_spaces: bool,
    /// Whether any grow exists.
    pub has_grows: bool,
    /// Whether any catalogue entry exists.
    pub has_equipment: bool,
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

    let equipment = RwSignal::<Vec<EquipmentItem>>::new(Vec::new());
    let picked = RwSignal::<Vec<String>>::new(Vec::new());
    let space_name = RwSignal::<String>::new(String::new());
    let space_w = RwSignal::<String>::new(String::new());
    let space_d = RwSignal::<String>::new(String::new());
    let space_h = RwSignal::<String>::new(String::new());
    let strains = RwSignal::<Vec<Strain>>::new(Vec::new());
    let picked_strain = RwSignal::<String>::new(String::new());
    let batches = RwSignal::<Vec<SeedBatchView>>::new(Vec::new());
    let picked_batch = RwSignal::<String>::new(String::new());
    let plant_prefix = RwSignal::<String>::new(String::from("Plant"));
    let plant_count = RwSignal::<u32>::new(1);
    let grow_name = RwSignal::<String>::new(String::new());
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
            if let Ok(list) = api::get::<Vec<EquipmentItem>>("api/equipment").await {
                equipment.set(list);
            }
            if let Ok(list) = api::get::<Vec<Strain>>("api/strains").await {
                strains.set(list);
            }
            if let Ok(list) = api::get::<Vec<SeedBatchView>>("api/seed-batches").await {
                batches.set(list);
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
        step.update(|current| *current = (*current).clamp(1, 3) + 1);
    };

    // prevent_default first: Leptos has no `on:submit:prevent` modifier
    // (an unknown `:word` suffix becomes a custom event that never
    // fires), so without this the browser submits the form natively and
    // reloads the page, losing everything (docs/DECISIONS.md D061).
    let finish = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let navigate = navigate.get_value();
        spawn_local(async move {
            // Step 2: a space built from the picked equipment.
            let mut new_space_id = None;
            if !space_name.get().trim().is_empty() {
                let input = GrowSpaceInput {
                    name: space_name.get(),
                    width_cm: space_w.get().trim().parse().ok(),
                    depth_cm: space_d.get().trim().parse().ok(),
                    height_cm: space_h.get().trim().parse().ok(),
                    notes: None,
                };
                match api::post::<GrowSpaceInput, GrowSpace>("api/spaces", &input).await {
                    Ok(space) => {
                        let created = space.id;
                        new_space_id = Some(created.clone());
                        for id in picked.get() {
                            // 204 No Content: post_action, not post_empty
                            // (which would try to parse a missing body).
                            if let Err(failure) =
                                api::post_action(&format!("api/spaces/{created}/equipment/{id}"))
                                    .await
                            {
                                error.set(Some(failure.message));
                                return;
                            }
                        }
                    }
                    Err(failure) => {
                        error.set(Some(failure.message));
                        return;
                    }
                }
            }
            // Step 4: plants inside a grow in that space; the grow is
            // the wizard's whole point, so skipping it entirely just
            // dismisses.
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
                    strain_id: non_empty(picked_strain.get()),
                    grow_space_id: new_space_id,
                    seed_batch_id: non_empty(picked_batch.get()),
                    feed_template_id: None,
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
            .is_some_and(|state| state.dismissed && !state.has_spaces && !state.has_grows)
    };

    view! {
        <Show when=visible>
            <section class="card form" aria-label="First-run setup">
                <h2>"Welcome to Leaflog"</h2>
                <p class="muted">
                    "Four quick steps to a first grow — skip any of them, everything stays
                     reachable from the tabs."
                </p>
                <ol class="steps">
                    <li class:done=move || {step.get() > 1} class:active=move || {step.get() == 1}>"Equipment"</li>
                    <li class:done=move || {step.get() > 2} class:active=move || {step.get() == 2}>"Grow space"</li>
                    <li class:done=move || {step.get() > 3} class:active=move || {step.get() == 3}>"Strain"</li>
                    <li class:active=move || {step.get() == 4}>"Plants"</li>
                </ol>
                <ErrorIf error />

                <Show when=move || step.get() == 1>
                    <div>
                        <h3>"1. Equipment basics"</h3>
                        <p class="muted">
                            "Start from the bundled presets — anything you pick here goes straight
                             into your space. Add your own gear (and HA sensors) on the "
                            <a href=crate::route_href("/equipment")>"equipment page"</a>
                            " and come back; the wizard waits."
                        </p>
                        <ul class="chips">
                            {move || {
                                equipment
                                    .get()
                                    .into_iter()
                                    .map(|item| {
                                        let id = item.id.clone();
                                        let label = item.name;
                                        let id_for_check = id.clone();
                                        let is_picked =
                                            move || picked.get().contains(&id_for_check);
                                        view! {
                                            <li>
                                                <button
                                                    class:chip-active=is_picked
                                                    on:click=move |_| {
                                                        picked.update(|current| {
                                                            if let Some(position) =
                                                                current.iter().position(|entry| entry == &id)
                                                            {
                                                                current.remove(position);
                                                            } else {
                                                                current.push(id.clone());
                                                            }
                                                        });
                                                    }
                                                >
                                                    {label}
                                                </button>
                                            </li>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            }}
                        </ul>
                        <div class="form-actions">
                            <button on:click=skip>"Skip this step"</button>
                            <button class="primary" on:click=skip disabled=move || picked.get().is_empty()>
                                "Next"
                            </button>
                        </div>
                    </div>
                </Show>

                <Show when=move || step.get() == 2>
                    <div>
                        <h3>"2. Grow space"</h3>
                        <p class="muted">"Name the tent or room your picked equipment lives in."</p>
                        <div class="grid">
                            <label class="field">
                                <span>"Name"</span>
                                <input type="text" placeholder="60×60 tent"
                                    prop:value=move || space_name.get()
                                    on:input=move |ev| space_name.set(event_target_value(&ev))
                                />
                            </label>
                            <label class="field"><span>"Width (cm)"</span>
                                <input type="number" min="0"
                                    prop:value=move || space_w.get()
                                    on:input=move |ev| space_w.set(event_target_value(&ev))
                                />
                            </label>
                            <label class="field"><span>"Depth (cm)"</span>
                                <input type="number" min="0"
                                    prop:value=move || space_d.get()
                                    on:input=move |ev| space_d.set(event_target_value(&ev))
                                />
                            </label>
                            <label class="field"><span>"Height (cm)"</span>
                                <input type="number" min="0"
                                    prop:value=move || space_h.get()
                                    on:input=move |ev| space_h.set(event_target_value(&ev))
                                />
                            </label>
                        </div>
                        <div class="form-actions">
                            <button on:click=skip>"Skip this step"</button>
                            <button class="primary" on:click=move |_| step.set(3)>"Next"</button>
                        </div>
                    </div>
                </Show>

                <Show when=move || step.get() == 3>
                    <div>
                        <h3>"3. Strain"</h3>
                        <p class="muted">
                            "Pick a strain for the grow diary — bundled presets are marked and
                             read-only; clone one as your phenotype to tweak it."
                        </p>
                        <label class="field">
                            <span>"Strain"</span>
                            <select
                                prop:value=move || picked_strain.get()
                                on:change=move |ev| picked_strain.set(event_target_value(&ev))
                            >
                                <option value="">"— none —"</option>
                                {move || {
                                    strains
                                        .get()
                                        .into_iter()
                                        .map(|strain| {
                                            let id = strain.id.clone();
                                            let label = if strain.is_builtin {
                                                format!("{} (bundled)", strain.name)
                                            } else {
                                                strain.name
                                            };
                                            view! { <option value=id>{label}</option> }
                                        })
                                        .collect::<Vec<_>>()
                                }}
                            </select>
                        </label>
                        <div class="form-actions">
                            <button on:click=skip>"Skip this step"</button>
                            <button class="primary" on:click=move |_| step.set(4)>"Next"</button>
                        </div>
                    </div>
                </Show>

                <Show when=move || step.get() == 4>
                    <form on:submit=finish>
                        <h3>"4. Plants"</h3>
                        <p class="muted">
                            "Finishing creates a grow inside your space (today counts as day one —
                             change it if the seeds went in earlier) and opens its dashboard."
                        </p>
                        <div class="grid">
                            <label class="field"><span>"Grow name"</span>
                                <input type="text" placeholder="First run"
                                    prop:value=move || grow_name.get()
                                    on:input=move |ev| grow_name.set(event_target_value(&ev))
                                />
                            </label>
                            <label class="field"><span>"Name prefix"</span>
                                <input type="text"
                                    prop:value=move || plant_prefix.get()
                                    on:input=move |ev| plant_prefix.set(event_target_value(&ev))
                                />
                            </label>
                            <label class="field"><span>"Plants"</span>
                                <input type="number" min="1" max="99" step="1"
                                    prop:value=move || plant_count.get().to_string()
                                    on:input=move |ev| {
                                        plant_count.set(event_target_value(&ev).parse().unwrap_or(1).clamp(1, 99));
                                    }
                                />
                            </label>
                            <label class="field"><span>"Germination (day one)"</span>
                                <input type="date"
                                    prop:value=move || started_on.get()
                                    on:input=move |ev| started_on.set(event_target_value(&ev))
                                />
                            </label>
                            <label class="field"><span>"From seed batch"</span>
                                <select
                                    prop:value=move || picked_batch.get()
                                    on:change=move |ev| picked_batch.set(event_target_value(&ev))
                                >
                                    <option value="">"— none —"</option>
                                    {move || {
                                        batches
                                            .get()
                                            .into_iter()
                                            .map(|batch| {
                                                let id = batch.batch.id.clone();
                                                let label = format!(
                                                    "{} × {}",
                                                    batch.batch.quantity,
                                                    strain_label(&strains, batch.batch.strain_id.as_ref()),
                                                );
                                                view! { <option value=id>{label}</option> }
                                            })
                                            .collect::<Vec<_>>()
                                    }}
                                </select>
                            </label>
                        </div>
                        <div class="form-actions">
                            <button type="button" on:click=move |_| step.set(3)>"Back"</button>
                            <button type="button" class="danger"
                                on:click=move |_| set_dismissed(true)
                            >
                                "Dismiss — I'll explore on my own"
                            </button>
                            <button class="primary" type="submit">"Start the grow"</button>
                        </div>
                    </form>
                </Show>
            </section>
        </Show>

        // The empty-state "set up" card: dismissed but still nothing
        // grown — one tap restarts the wizard (spec §4.12).
        <Show when=restartable>
            <section class="card">
                <h3>"Set up your first grow"</h3>
                <p class="muted">"The setup wizard is available whenever you want it."</p>
                <button class="primary" on:click=move |_| set_dismissed(false)>
                    "Run the setup wizard"
                </button>
            </section>
        </Show>
    }
}

/// The strain name for a batch option (unlinked batches say so).
fn strain_label(strains: &RwSignal<Vec<Strain>>, strain_id: Option<&String>) -> String {
    strain_id.map_or_else(
        || "unlinked seeds".to_owned(),
        |id| {
            strains
                .get_untracked()
                .iter()
                .find(|strain| &strain.id == id)
                .map_or_else(|| "unknown strain".to_owned(), |strain| strain.name.clone())
        },
    )
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
