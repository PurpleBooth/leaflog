//! Grows UI (spec §4.2, §4.3): grow list/creation and the grow detail
//! page with quick-add action chips, the timeline, plant management and
//! the photo gallery with uploads.

use leaflog_core::actions::{ActionDetails, ActionType};
use leaflog_core::grows::{
    ActionEntry, ActionInput, Grow, GrowInput, GrowStatus, Photo, Plant, PlantInput,
};
use leaflog_core::strain::Strain;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::web_sys::FormData;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;

use crate::api;
use crate::dashboard::EnvDashboard;
use crate::equipment::ErrorIf;
use crate::feed::FeedPanel;
use crate::gantt::GanttTimeline;
use crate::live::LivePanel;

/// The grows overview page.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn GrowsPage() -> impl IntoView {
    let grows = RwSignal::<Vec<Grow>>::new(Vec::new());
    let strains = RwSignal::<Vec<Strain>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);
    let form_open = RwSignal::<bool>::new(false);
    let draft = RwSignal::<GrowInput>::new(empty_grow());

    let refresh = move || {
        spawn_local(async move {
            let grow_list = api::get::<Vec<Grow>>("api/grows").await;
            let strain_list = api::get::<Vec<Strain>>("api/strains").await;
            match (grow_list, strain_list) {
                (Ok(grow_list), Ok(strain_list)) => {
                    grows.set(grow_list);
                    strains.set(strain_list);
                }
                (Err(failure), _) | (_, Err(failure)) => error.set(Some(failure.message)),
            }
        });
    };
    Effect::new(move |_| {
        refresh();
    });

    let save = move |_ev: leptos::ev::SubmitEvent| {
        spawn_local(async move {
            match api::post::<GrowInput, Grow>("api/grows", &draft.get()).await {
                Ok(_) => {
                    form_open.set(false);
                    error.set(None);
                    refresh();
                }
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    view! {
        <section class="page">
            <div class="page-head">
                <h2>"Grows"</h2>
                <button class="primary" on:click=move |_| {
                    draft.set(empty_grow());
                    form_open.set(true);
                }>"Start a grow"</button>
            </div>

            <ErrorIf error />

            <Show when=move || form_open.get()>
                <form class="card form" on:submit:prevent=save>
                    <h3>"New grow"</h3>
                    <div class="grid">
                        <label class="field">
                            <span>"Name *"</span>
                            <input type="text" required placeholder="Closet run"
                                prop:value=move || draft.get().name
                                on:input=move |ev| draft.update(|d| d.name = event_target_value(&ev))
                            />
                        </label>
                        <label class="field">
                            <span>"Strain"</span>
                            <select
                                prop:value=move || draft.get().strain_id.unwrap_or_default()
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    draft.update(|d| {
                                        d.strain_id = (!value.is_empty()).then_some(value);
                                    });
                                }
                            >
                                <option value="">"— pick a strain —"</option>
                                {move || {
                                    strains
                                        .get()
                                        .into_iter()
                                        .map(|strain| {
                                            view! {
                                                <option value=strain.id>
                                                    {strain.name}
                                                </option>
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                }}
                            </select>
                        </label>
                        <label class="field">
                            <span>"Substrate *"</span>
                            <select
                                prop:value=move || draft.get().substrate
                                on:change=move |ev| draft.update(|d| d.substrate = event_target_value(&ev))
                            >
                                {leaflog_core::grows::SUBSTRATES
                                    .iter()
                                    .map(|substrate| view! { <option value=*substrate>{*substrate}</option> })
                                    .collect::<Vec<_>>()}
                            </select>
                        </label>
                        <label class="field">
                            <span>"Started (seed/clone) *"</span>
                            <input type="date" required
                                prop:value=move || draft.get().started_on
                                on:input=move |ev| draft.update(|d| d.started_on = event_target_value(&ev))
                            />
                        </label>
                    </div>
                    <div class="form-actions">
                        <button class="primary" type="submit">"Save"</button>
                        <button type="button" on:click=move |_| form_open.set(false)>"Cancel"</button>
                    </div>
                </form>
            </Show>

            <ul class="cards">
                {move || {
                    grows
                        .get()
                        .into_iter()
                        .map(|grow| {
                            view! {
                                <li class="card">
                                    <div class="card-title">
                                        <A href=format!("/grows/{}", grow.id)>
                                            <strong>{grow.name.clone()}</strong>
                                        </A>
                                        <span class="badge">{grow.status.label()}</span>
                                        <span class="muted">{grow.substrate.clone()}</span>
                                    </div>
                                    <p class="muted small">"Started "{grow.started_on}</p>
                                </li>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </ul>
            <Show when=move || grows.get().is_empty() && error.get().is_none()>
                <p class="empty">"No grows yet — start your first one."</p>
            </Show>
            <p><A href="/">"Back to home"</A></p>
        </section>
    }
}

/// The grow detail page: chips, timeline, plants and photos.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn GrowDetail() -> impl IntoView {
    let params = use_params_map();
    let grow_id = move || params.get().get("id").unwrap_or_default();

    let grow = RwSignal::<Option<Grow>>::new(None);
    let plants = RwSignal::<Vec<Plant>>::new(Vec::new());
    let actions = RwSignal::<Vec<ActionEntry>>::new(Vec::new());
    let photos = RwSignal::<Vec<Photo>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);

    let chip = RwSignal::<Option<ActionType>>::new(None);
    let draft = RwSignal::<ActionInput>::new(empty_action());
    let upload_note = RwSignal::<Option<String>>::new(None);

    let refresh = move || {
        let id = grow_id();
        spawn_local(async move {
            let base = format!("api/grows/{id}");
            match api::get::<Grow>(&base).await {
                Ok(value) => grow.set(Some(value)),
                Err(failure) => error.set(Some(failure.message)),
            }
            match api::get::<Vec<Plant>>(&format!("{base}/plants")).await {
                Ok(value) => plants.set(value),
                Err(failure) => error.set(Some(failure.message)),
            }
            match api::get::<Vec<ActionEntry>>(&format!("{base}/actions?limit=200")).await {
                Ok(value) => actions.set(value),
                Err(failure) => error.set(Some(failure.message)),
            }
            match api::get::<Vec<Photo>>(&format!("{base}/photos")).await {
                Ok(value) => photos.set(value),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };
    Effect::new(move |_| {
        params.track();
        refresh();
    });

    let save_action = move |_ev: leptos::ev::SubmitEvent| {
        let id = grow_id();
        spawn_local(async move {
            let input = draft.get();
            match api::post::<ActionInput, ActionEntry>(&format!("api/grows/{id}/actions"), &input)
                .await
            {
                Ok(_) => {
                    chip.set(None);
                    error.set(None);
                    refresh();
                }
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    let add_plant = move |_ev: leptos::ev::MouseEvent| {
        let id = grow_id();
        spawn_local(async move {
            let name = format!("Plant {}", plants.get().len() + 1);
            match api::post::<PlantInput, Plant>(
                &format!("api/grows/{id}/plants"),
                &PlantInput { name },
            )
            .await
            {
                Ok(_) => refresh(),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    let upload = move |ev: leptos::ev::Event| {
        let id = grow_id();
        let input = event_target::<leptos::web_sys::HtmlInputElement>(&ev);
        let Some(files) = input.files() else {
            return;
        };
        let form = FormData::new().expect("FormData constructs");
        for index in 0..files.length() {
            if let Some(file) = files.get(index) {
                form.append_with_blob("file", &file)
                    .expect("appending a blob cannot fail");
            }
        }
        spawn_local(async move {
            let url = format!("api/grows/{id}/photos");
            let result = api::post_form(&url, &form).await;
            match result {
                Ok(()) => {
                    upload_note.set(Some("Photos uploaded".to_owned()));
                    refresh();
                }
                Err(failure) => upload_note.set(Some(failure.message)),
            }
        });
    };

    view! {
        <section class="page">
            {move || {
                grow.get().map_or_else(
                    || ().into_any(),
                    |grow| {
                        view! {
                            <div>
                                <div class="page-head">
                                    <h2>{grow.name.clone()}</h2>
                                    <span class="badge">{grow.substrate.clone()}</span>
                                </div>
                                <p class="muted">"Started "{grow.started_on}</p>
                            </div>
                        }
                        .into_any()
                    },
                )
            }}

            <ErrorIf error />

            <h3>"Forecast"</h3>
            <GanttTimeline grow_id=grow_id() />

            <EnvDashboard grow_id=grow_id() />

            <h3>"Feed schedule"</h3>
            <FeedPanel grow_id=grow_id() />

            <h3>"Quick add"</h3>
            <nav class="chips" aria-label="Quick add action">
                {ActionType::all()
                    .into_iter()
                    .map(|kind| {
                        let current = chip;
                        view! {
                            <button
                                class:chip-active=move || current.get() == Some(kind)
                                on:click=move |_| {
                                    let selected = current.get() == Some(kind);
                                    current.set((!selected).then_some(kind));
                                    draft.set(empty_action());
                                }
                            >
                                {kind.display()}
                            </button>
                        }
                    })
                    .collect::<Vec<_>>()}
            </nav>

            <Show when=move || chip.get().is_some()>
                <form class="card form" on:submit:prevent=save_action>
                    <ActionForm draft kind=chip />
                    <div class="form-actions">
                        <button class="primary" type="submit">"Log it"</button>
                        <button type="button" on:click=move |_| chip.set(None)>"Cancel"</button>
                    </div>
                </form>
            </Show>

            <div class="split">
                <div>
                    <h3>"Timeline"</h3>
                    <ul class="timeline">
                        {move || {
                            actions
                                .get()
                                .into_iter()
                                .map(|entry| {
                                    view! {
                                        <li class="card">
                                            <div class="card-title">
                                                <strong>{entry.action_type.display()}</strong>
                                                <span class="badge">{"day "}{entry.grow_day}</span>
                                                {entry.phase.map(|phase| view! {
                                                    <span class="badge">{phase.label()}</span>
                                                })}
                                            </div>
                                            <dl class="facts">
                        {entry.details.volume_ml.map(|volume| view! {
                            <div><dt>"Volume"</dt><dd>{leaflog_core::units::small_volume_both(volume)}</dd></div>
                        })}
                        {entry.details.input_ph.map(|ph| view! {
                            <div><dt>"pH in"</dt><dd>{format!("{ph:.1}")}</dd></div>
                        })}
                        {entry.details.runoff_ph.map(|ph| view! {
                            <div><dt>"pH out"</dt><dd>{format!("{ph:.1}")}</dd></div>
                        })}
                        {entry.details.input_ec_us.map(|ec| view! {
                            <div><dt>"EC in"</dt><dd>{format!("{ec} µS/cm")}</dd></div>
                        })}
                        {entry.details.runoff_ec_us.map(|ec| view! {
                            <div><dt>"EC out"</dt><dd>{format!("{ec} µS/cm")}</dd></div>
                        })}
                        {entry.details.plant_height_cm.map(|height| view! {
                            <div><dt>"Height"</dt><dd>{leaflog_core::units::length_both(height)}</dd></div>
                        })}
                                            </dl>
                                            {entry.notes.clone().map(|notes| view! {
                                                <p class="notes">{notes}</p>
                                            })}
                                            {(!entry.appearance_tags.is_empty()).then(|| {
                                                view! {
                                                    <p class="tags">
                                                        {entry
                                                            .appearance_tags
                                                            .iter()
                                                            .map(|tag| {
                                                                view! { <span class="tag">{tag.clone()}</span> }
                                                            })
                                                            .collect::<Vec<_>>()}
                                                    </p>
                                                }
                                            })}
                                            <p class="muted small">{entry.occurred_at.clone()}</p>
                                        </li>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </ul>
                    <Show when=move || actions.get().is_empty()>
                        <p class="empty">"Nothing logged yet."</p>
                    </Show>
                </div>

                <div>
                    <LivePanel grow_id=grow_id() />
                    <h3>"Plants"</h3>
                    <ul class="cards">
                        {move || {
                            plants
                                .get()
                                .into_iter()
                                .map(|plant| {
                                    view! {
                                        <li class="card slim">
                                            <strong>{plant.name}</strong>
                                        </li>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </ul>
                    <button on:click=add_plant>"Add plant"</button>

                    <h3>"Photos"</h3>
                    <label class="field">
                        <span>"Upload photos (JPEG/PNG/WebP; EXIF/GPS stripped on store)"</span>
                        <input type="file" accept="image/jpeg,image/png,image/webp" multiple
                            on:change=upload
                        />
                    </label>
                    {move || upload_note.get().map(|note| view! { <p class="muted small">{note}</p> })}
                    <div class="gallery">
                        {move || {
                            photos
                                .get()
                                .into_iter()
                                .map(|photo| {
                                    let url = format!("api/photos/{}/thumbnail", photo.id);
                                    let full = format!("api/photos/{}/original", photo.id);
                                    view! {
                                        <a href=full target="_blank" rel="noreferrer">
                                            <img src=url alt=format!("day {}", photo.grow_day) />
                                            <span class="muted small">{"day "}{photo.grow_day}</span>
                                        </a>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </div>
                </div>
            </div>

            <p><A href="/grows">"All grows"</A></p>
        </section>
    }
}

/// The chip form: shared fields plus the ones relevant to the chosen
/// action type.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
fn ActionForm(draft: RwSignal<ActionInput>, kind: RwSignal<Option<ActionType>>) -> impl IntoView {
    view! {
        <div class="grid">
            <label class="field">
                <span>"When (UTC) *"</span>
                <input type="datetime-local" required
                    prop:value=move || draft.get().occurred_at
                    on:input=move |ev| draft.update(|d| d.occurred_at = event_target_value(&ev))
                />
            </label>

            {move || {
                let d = draft;
                let kind = kind.get()?;
                Some(match kind {
                    ActionType::Water | ActionType::Feed => view! {
                        {num_field("Volume (ml)", "1", d, |a| a.details.volume_ml, |a, v| a.details.volume_ml = v)}
                        {num_field("pH in", "0.1", d, |a| a.details.input_ph, |a, v| a.details.input_ph = v)}
                        {num_field("pH out", "0.1", d, |a| a.details.runoff_ph, |a, v| a.details.runoff_ph = v)}
                        {num_field("EC in (µS/cm)", "10", d, |a| a.details.input_ec_us.map(f64::from), |a, v| a.details.input_ec_us = clamp_int(v))}
                        {num_field("EC out (µS/cm)", "10", d, |a| a.details.runoff_ec_us.map(f64::from), |a, v| a.details.runoff_ec_us = clamp_int(v))}
                    }
                        .into_any(),
                    ActionType::Measurement => view! {
                        {num_field("Plant height (cm)", "1", d, |a| a.details.plant_height_cm, |a, v| a.details.plant_height_cm = v)}
                    }
                        .into_any(),
                    ActionType::LightChange => view! {
                        {num_field("Height above canopy (cm)", "1", d, |a| a.details.height_cm, |a, v| a.details.height_cm = v)}
                        {num_field("Dimmer (%)", "5", d, |a| a.details.dim_pct.map(f64::from), |a, v| a.details.dim_pct = clamp_int(v))}
                    }
                        .into_any(),
                    ActionType::TrichomeCheck => view! {
                        {num_field("Cloudy (%)", "5", d, |a| a.details.cloudy_pct.map(f64::from), |a, v| a.details.cloudy_pct = clamp_int(v))}
                        {num_field("Amber (%)", "5", d, |a| a.details.amber_pct.map(f64::from), |a, v| a.details.amber_pct = clamp_int(v))}
                    }
                        .into_any(),
                    ActionType::Harvest => view! {
                        {num_field("Wet weight (g)", "1", d, |a| a.details.weight_g, |a, v| a.details.weight_g = v)}
                    }
                        .into_any(),
                    _ => ().into_any(),
                })
            }}

            <label class="field wide">
                <span>"Notes"</span>
                <textarea rows="2"
                    prop:value=move || draft.get().notes.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.notes = non_empty(event_target_value(&ev)))
                />
            </label>
        </div>
        <fieldset class="tags">
            <legend class="muted small">"Appearance"</legend>
            {leaflog_core::actions::APPEARANCE_TAGS
                .iter()
                .map(|tag| {
                    view! {
                        <label class="field checkbox">
                            <input type="checkbox"
                                on:change=move |ev| {
                                    let tag = (*tag).to_owned();
                                    let checked = event_target_checked(&ev);
                                    draft.update(|d| {
                                        d.appearance_tags.retain(|existing| existing != &tag);
                                        if checked {
                                            d.appearance_tags.push(tag);
                                        }
                                    });
                                }
                            />
                            <span class="small">{*tag}</span>
                        </label>
                    }
                })
                .collect::<Vec<_>>()}
        </fieldset>
    }
}

/// Converts an optional form float into an integer field after
/// rounding and clamping.
// f64 -> i64 is the one unavoidable primitive cast; values come from
// parsed number inputs and are clamped before the cast.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // see docs/DECISIONS.md
fn clamp_int<T: TryFrom<i64>>(value: Option<f64>) -> Option<T> {
    value.and_then(|value| T::try_from(value.round() as i64).ok())
}

/// One labelled numeric input bound to a spot in the draft's details.
fn num_field(
    label: &'static str,
    step: &'static str,
    draft: RwSignal<ActionInput>,
    read: impl Fn(&ActionInput) -> Option<f64> + Clone + Send + 'static,
    write: impl Fn(&mut ActionInput, Option<f64>) + Clone + Send + 'static,
) -> impl IntoView {
    view! {
        <label class="field">
            <span>{label}</span>
            <input type="number" step=step
                prop:value=move || {
                    read(&draft.get()).map(|value| value.to_string()).unwrap_or_default()
                }
                on:input=move |ev| {
                    let parsed = event_target_value(&ev).parse::<f64>().ok();
                    draft.update(|current| write(current, parsed));
                }
            />
        </label>
    }
}

/// Empty strings become `None`.
fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

/// A blank grow draft.
fn empty_grow() -> GrowInput {
    GrowInput {
        name: String::new(),
        strain_id: None,
        substrate: "soil".to_owned(),
        started_on: String::new(),
        status: GrowStatus::default(),
        notes: None,
    }
}

/// A blank action draft, timestamped now (local time, close enough for
/// a pre-filled form the user can adjust).
fn empty_action() -> ActionInput {
    ActionInput {
        plant_ids: Vec::new(),
        occurred_at: js_sys::Date::new_0()
            .to_iso_string()
            .as_string()
            .unwrap_or_default(),
        action_type: ActionType::Note,
        details: ActionDetails::default(),
        notes: None,
        appearance_tags: Vec::new(),
    }
}
