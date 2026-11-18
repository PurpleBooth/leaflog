//! Grows UI (spec §4.2, §4.3): grow list/creation and the grow detail
//! page with quick-add action chips, the timeline, plant management and
//! the photo gallery with uploads.

use crate::phase_pick::{self, phase_display, phase_from_label, picker_phases};
use crate::quick_add::QuickAddMenu;
use futures::future::Either;
use gloo_timers::future::TimeoutFuture;
use leaflog_core::Phase;
use leaflog_core::actions::{ActionDetails, ActionType};
use leaflog_core::grows::{
    ActionEntry, ActionInput, Grow, GrowInput, GrowStatus, Photo, PhotoperiodType, Plant,
    PlantInput, Tolerance,
};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::web_sys::FormData;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use serde::de::DeserializeOwned;
use std::future::Future;
use wasm_bindgen::JsCast;

use crate::ErrorIf;
use crate::Home;
use crate::api::{self, ApiFailure};
use crate::env_section::EnvironmentSection;
use crate::feed::FeedPanel;
use crate::gantt::GanttTimeline;
use crate::i18n::{Locale, t, t_string, td_string, use_i18n};
use crate::labels;
use crate::route_href;

/// The grows overview page.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn GrowsPage() -> impl IntoView {
    let i18n = use_i18n();
    let grows = RwSignal::<Vec<Grow>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);
    let loading = RwSignal::<bool>::new(true);
    let form_open = RwSignal::<bool>::new(false);
    let draft = RwSignal::<GrowInput>::new(empty_grow());

    // One fetch: the grows list is the page's only resource, so the
    // error banner tri-states exactly that request.
    let refresh = move || {
        loading.set(true);
        spawn_local(async move {
            let path = "api/grows";
            let timeout_message = td_string!(
                i18n.get_locale_untracked(),
                errors.request_timeout,
                path = path
            );
            match fetch_list::<Vec<Grow>>(path, timeout_message).await {
                Ok(list) => {
                    grows.set(list);
                    error.set(None);
                }
                Err(failure) => error.set(Some(failure.message)),
            }
            loading.set(false);
        });
    };
    Effect::new(move |_| {
        refresh();
    });

    // prevent_default first — see docs/DECISIONS.md D061.
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
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
        <section class="pb-12">
            <div class="flex flex-wrap items-center justify-between gap-4">
                <h2>{t!(i18n, page.grows_heading)}</h2>
                <button class="btn btn-primary btn-sm" on:click=move |_| {
                    draft.set(empty_grow());
                    form_open.set(true);
                }>{t!(i18n, grows.start_grow)}</button>
            </div>

            <ErrorIf error />

            <Show when=move || error.get().is_some()>
                // A retry keeps the old banner up while its requests
                // run, so the button stays visible through the stall.
                // Gate the action on both states: a second click
                // mid-stall would run an overlapping refresh whose
                // stale task re-raises the banner a healed run just
                // cleared.
                <button class="btn btn-sm mt-2" on:click=move |_| {
                    if error.get().is_some() && !loading.get() {
                        refresh();
                    }
                }>
                    {t!(i18n, grows.retry)}
                </button>
            </Show>

            <Show when=move || form_open.get()>
                <form class="card bg-base-100 shadow-sm px-5 py-4 my-4" on:submit=save>
                    <h3>{t!(i18n, grows.new_grow)}</h3>
                    <GrowForm draft />
                    <div class="flex flex-wrap gap-2 mt-4">
                        <button class="btn btn-primary btn-sm" type="submit">{t!(i18n, grows.save)}</button>
                        <button class="btn btn-sm" type="button" on:click=move |_| form_open.set(false)>{t!(i18n, grows.cancel)}</button>
                    </div>
                </form>
            </Show>

            <ul class="grid gap-4 lg:grid-cols-2">
                {move || {
                    grows
                        .get()
                        .into_iter()
                        .map(|grow| {
                            view! {
                                <li class="card bg-base-100 shadow-sm px-5 py-4">
                                    <div class="flex flex-wrap items-baseline gap-2">
                                        <A href=route_href(&format!("/grows/{}", grow.id))>
                                            <strong>{grow.name.clone()}</strong>
                                        </A>
                                        <span class="badge badge-ghost">{labels::status_label(
                                            i18n.get_locale(),
                                            grow.status,
                                        )}</span>
                                        <span class="text-base-content/70">{grow.substrate.clone()}</span>
                                    </div>
                                    <p class="text-base-content/70 text-sm">{t!(i18n, grows.started, date = grow.started_on)}</p>
                                </li>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </ul>
            <Show when=move || loading.get()>
                <p class="py-6 text-center text-base-content/70">{t!(i18n, grows.loading)}</p>
            </Show>
            <Show
                when=move || {
                    !loading.get()
                        && grows.get().is_empty()
                        && error.get().is_none()
                }
            >
                <p class="py-6 text-center text-base-content/60">{t!(i18n, grows.empty)}</p>
            </Show>
            <p><A href=route_href("/")>{t!(i18n, grows.back_home)}</A></p>
        </section>
    }
}

/// The `/grows/:id` route: reads the path id and renders the overview.
#[component]
pub fn GrowsRoute() -> impl IntoView {
    let params = use_params_map();
    let grow_id = Signal::derive(move || params.get().get("id").unwrap_or_default());
    view! { <GrowDetail grow_id /> }
}

/// The app landing page (D073 follow-up): the most recently updated
/// active grow's overview, falling back to the most recent grow
/// overall, and to the existing home (onboarding) with no grows at
/// all.
#[component]
pub fn Landing() -> impl IntoView {
    let picked = RwSignal::<Option<String>>::new(None);
    Effect::new(move |_| {
        spawn_local(async move {
            match api::get::<Vec<Grow>>("api/grows").await {
                Ok(grows) => picked.set(Some(pick_landing_grow(&grows))),
                // A failed fetch is the home page's problem to show.
                Err(_) => picked.set(None),
            }
        });
    });
    view! {
        <Show
            when=move || {
                picked.get().as_deref().is_some_and(|id| !id.is_empty())
            }
            fallback=|| view! { <Home /> }
        >
            {move || {
                let id = picked.get().unwrap_or_default();
                view! { <GrowDetail grow_id=Signal::derive(move || id.clone()) /> }
            }}
        </Show>
    }
}

/// Picks the grow the app lands on: the most recently updated grow
/// whose status is still active; with none active, the most recently
/// updated grow overall. Empty when there are no grows (the landing
/// then shows the onboarding home).
#[must_use]
fn pick_landing_grow(grows: &[Grow]) -> String {
    let newest = |grow: &Grow| grow.updated_at.clone();
    let active = grows
        .iter()
        .filter(|grow| grow.status == GrowStatus::Active)
        .max_by(|a, b| newest(a).cmp(&newest(b)));
    let overall = grows.iter().max_by(|a, b| newest(a).cmp(&newest(b)));
    active
        .or(overall)
        .map_or_else(String::new, |grow| grow.id.clone())
}

/// The grow overview page: logging loop, environment (live tiles and
/// history charts under one shared window control, env-section
/// ticket 01), forecast, feed schedule, timeline, plants and photos.
/// Configuration lives on the edit page (grow-page rework ticket 01),
/// reached through the header's Edit control.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn GrowDetail(grow_id: Signal<String>) -> impl IntoView {
    let i18n = use_i18n();
    let grow = RwSignal::<Option<Grow>>::new(None);
    let plants = RwSignal::<Vec<Plant>>::new(Vec::new());
    let actions = RwSignal::<Vec<ActionEntry>>::new(Vec::new());
    let photos = RwSignal::<Vec<Photo>>::new(Vec::new());
    let forecast = RwSignal::<Option<api::Forecast>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);

    let chip = RwSignal::<Option<ActionType>>::new(None);
    let draft = RwSignal::<ActionInput>::new(empty_action());
    let upload_note = RwSignal::<Option<String>>::new(None);
    // Set once a quick-add entry saves; cleared the moment the next
    // entry is picked, so the note always names the latest save.
    let just_logged = RwSignal::new(false);
    Effect::new(move |_| {
        if chip.get().is_some() {
            just_logged.set(false);
        }
    });

    // The phase picker (D073): the grow's photoperiod toggle decides
    // the Flowering mapping — a photoperiod grow records the flip, an
    // autoflower the generic Phase-change. Unset defaults to
    // photoperiod semantics.
    let pick_phase = RwSignal::<Option<Phase>>::new(None);
    let pick_date = RwSignal::<String>::new(String::new());
    let grow_photoperiod = Signal::derive(move || {
        grow.get()
            .as_ref()
            .map(|current| current.photoperiod)
            .unwrap_or_default()
    });

    let refresh = move || {
        let id = grow_id.get();
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
            // The phase-day badge rides the same refresh, so a new
            // phase pick re-anchors it without a reload.
            match api::get::<api::Forecast>(&format!("{base}/forecast")).await {
                Ok(value) => forecast.set(Some(value)),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };
    Effect::new(move |_| {
        grow_id.track();
        refresh();
    });

    // prevent_default first — see docs/DECISIONS.md D061.
    let save_action = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let id = grow_id.get();
        spawn_local(async move {
            let input = draft.get();
            match api::post::<ActionInput, ActionEntry>(&format!("api/grows/{id}/actions"), &input)
                .await
            {
                Ok(_) => {
                    chip.set(None);
                    error.set(None);
                    just_logged.set(true);
                    refresh();
                }
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    // Recording a phase pick writes the transition event (D073); the
    // forecast, bands, coach, lamp advice and day-count target follow
    // on the next refresh.
    let save_pick = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(phase) = pick_phase.get() else {
            return;
        };
        let id = grow_id.get();
        let photoperiod = grow_photoperiod.get();
        let when = pick_date.get();
        let day = if when.is_empty() {
            datetime_local_now()
        } else {
            when
        };
        let date = day.get(..10).unwrap_or_default().to_owned();
        let input = phase_pick::pick_input(phase, photoperiod, format!("{date}T12:00:00"));
        spawn_local(async move {
            match api::post::<ActionInput, ActionEntry>(&format!("api/grows/{id}/actions"), &input)
                .await
            {
                Ok(_) => {
                    pick_phase.set(None);
                    pick_date.set(String::new());
                    error.set(None);
                    refresh();
                }
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    let add_plant = move |_ev: leptos::ev::MouseEvent| {
        let id = grow_id.get();
        spawn_local(async move {
            let name = format!("Plant {}", plants.get().len() + 1);
            match api::post::<PlantInput, Plant>(
                &format!("api/grows/{id}/plants"),
                &PlantInput {
                    name,
                    wet_g: None,
                    dry_g: None,
                    trimmed_g: None,
                },
            )
            .await
            {
                Ok(_) => refresh(),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    let upload = move |ev: leptos::ev::Event| {
        let id = grow_id.get();
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
        <section class="pb-12">
            {move || {
                grow.get().map_or_else(
                    || ().into_any(),
                    |current| {
                        let strain_label = current.strain_label.clone();
                        let archived = current.status == GrowStatus::Archived;
                        view! {
                            <div>
                                // One left-aligned quiet cluster: name ·
                                // glyph + medium · strain label, with the
                                // status badge joining it. The actions sit
                                // on the same wrap row, pushed right:
                                // quick add is the page's primary loop, edit
                                // the rare one. An unlabelled grow leaves the
                                // label slot empty — no placeholder.
                                <div class="flex flex-wrap items-center gap-x-3 gap-y-2">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <h2>{current.name.clone()}</h2>
                                        <span class="badge badge-ghost">
                                            {substrate_glyph(&current.substrate).map(|glyph| view! {
                                                <span aria-hidden="true">{glyph}</span>
                                            })}
                                            {current.substrate.clone()}
                                        </span>
                                        {(!strain_label.is_empty()).then(|| {
                                            let label = strain_label.clone();
                                            view! {
                                                <span class="badge badge-ghost">
                                                    {label}
                                                </span>
                                            }
                                        })}
                                        {move || {
                                            forecast.get().and_then(|data| {
                                                phase_from_label(&data.phase).map(|phase| view! {
                                                    <span id="phase-day" class="badge badge-ghost">
                                                        {labels::phase_label(i18n.get_locale(), phase)}
                                                        {" · "}
                                                        {t_string!(i18n.get_locale(), grow.day, day = data.phase_day.to_string())}
                                                    </span>
                                                })
                                            })
                                        }}
                                        {archived
                                            .then(|| {
                                                let status = labels::status_label(
                                                    i18n.get_locale(),
                                                    current.status,
                                                );
                                                view! {
                                                    <span class="badge badge-ghost">
                                                        {status}
                                                    </span>
                                                }
                                            })}
                                    </div>
                                    // The logging menu lives in the header
                                    // (the loop's first tap); the chosen
                                    // form opens right below it.
                                    <div class="flex items-center gap-2 ml-auto">
                                        <QuickAddMenu chip draft />
                                        <A href=route_href(&format!("/grows/{}/edit", current.id))
                                            attr:class="btn btn-sm"
                                        >
                                            {t!(i18n, grow.edit)}
                                        </A>
                                    </div>
                                </div>
                                <p class="m-0 text-sm text-base-content/60">
                                    {t!(i18n, grows.started, date = current.started_on)}
                                </p>
                                {current.notes.map(|notes| view! {
                                    <p class="text-base-content/70">{notes}</p>
                                })}
                            </div>
                        }
                        .into_any()
                    },
                )
            }}

            <ErrorIf error />

            // The chosen entry form opens directly under the header —
            // the loop is pick, fill, log, without a scroll (PRODUCT
            // principle 2). A quiet status line confirms the save; the
            // entry itself lands in the timeline at the foot.
            <Show when=move || chip.get().is_some()>
                <form
                    id="quick-add-form"
                    tabindex="-1"
                    class="card bg-base-100 shadow-sm px-5 py-4 mt-4"
                    on:submit=save_action
                >
                    <ActionForm draft kind=chip />
                    <div class="flex flex-wrap gap-2 mt-4">
                        <button class="btn btn-primary btn-sm" type="submit">{t!(i18n, grow.quick_add_log)}</button>
                        <button class="btn btn-sm" type="button" on:click=move |_| chip.set(None)>{t!(i18n, grows.cancel)}</button>
                    </div>
                </form>
            </Show>
            {move || {
                just_logged.get().then(|| {
                    view! {
                        <p class="mt-2 mb-0 text-sm text-base-content/60" role="status">
                            {t!(i18n, grow.logged_note)}
                        </p>
                    }
                })
            }}

            <EnvironmentSection grow_id=grow_id.get() />

            <h3>{t!(i18n, grow.forecast)}</h3>
            <form id="phase-picker" class="flex flex-wrap gap-2 items-end my-2" on:submit=save_pick>
                <label class="fieldset">
                    <span class="fieldset-label">{t!(i18n, grow.set_phase)}</span>
                    <select class="select" required
                        prop:value=move || pick_phase
                            .get()
                            .map_or(String::new(), |phase| phase.label().to_owned())
                        on:change=move |ev| {
                            let value = event_target_value(&ev);
                            pick_phase.set(phase_from_label(&value));
                        }
                    >
                        <option value="">{t!(i18n, grow.set_phase_placeholder)}</option>
                        {picker_phases()
                            .into_iter()
                            .map(|(phase, _)| {
                                let name = labels::phase_label(i18n.get_locale(), phase);
                                view! { <option value=phase.label()>{name}</option> }
                            })
                            .collect::<Vec<_>>()}
                    </select>
                </label>
                <label class="fieldset">
                    <span class="fieldset-label">{t!(i18n, grow.when)}</span>
                    <input class="input" type="date"
                        prop:value=move || pick_date.get()
                        on:input=move |ev| pick_date.set(event_target_value(&ev))
                    />
                </label>
                <button class="btn btn-sm btn-primary self-end" type="submit"
                    disabled=move || pick_phase.get().is_none()
                >
                    {t!(i18n, grow.record_phase)}
                </button>
            </form>
            <GanttTimeline grow_id=grow_id.get() />

            <h3>{t!(i18n, grow.feed_schedule)}</h3>
            <FeedPanel grow_id=grow_id.get() />

            <div class="grid gap-6 items-start lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
                <div>
                    <h3>{t!(i18n, grow.timeline)}</h3>
                    <ul class="list-none m-0 p-0 grid gap-2">
                        {move || {
                            let locale = i18n.get_locale();
                            actions
                                .get()
                                .into_iter()
                                .map(|entry| {
                                    view! {
                                        <li class="card bg-base-100 shadow-sm px-4 py-3">
                                            <div class="flex flex-wrap items-baseline gap-2">
                                                <strong>{labels::action_label(locale, entry.action_type)}</strong>
                                                <span class="badge badge-ghost">{t_string!(locale, grow.day, day = entry.grow_day.to_string())}</span>
                                                {entry.phase.map(|phase| view! {
                                                    <span class="badge badge-ghost">{labels::phase_label(locale, phase)}</span>
                                                })}
                                            </div>
                                            <dl class="facts">
                        {entry.details.volume_ml.map(|volume| view! {
                            <div><dt>{t!(i18n, grow.volume)}</dt><dd>{leaflog_core::units::small_volume_both(volume)}</dd></div>
                        })}
                        {entry.details.input_ph.map(|ph| view! {
                            <div><dt>{t!(i18n, grow.ph_in)}</dt><dd>{format!("{ph:.1}")}</dd></div>
                        })}
                        {entry.details.runoff_ph.map(|ph| view! {
                            <div><dt>{t!(i18n, grow.ph_out)}</dt><dd>{format!("{ph:.1}")}</dd></div>
                        })}
                        {entry.details.input_ec_us.map(|ec| view! {
                            <div><dt>{t!(i18n, grow.ec_in)}</dt><dd>{format!("{ec} µS/cm")}</dd></div>
                        })}
                        {entry.details.runoff_ec_us.map(|ec| view! {
                            <div><dt>{t!(i18n, grow.ec_out)}</dt><dd>{format!("{ec} µS/cm")}</dd></div>
                        })}
                        {entry.details.plant_height_cm.map(|height| view! {
                            <div><dt>{t!(i18n, grow.height)}</dt><dd>{leaflog_core::units::length_both(height)}</dd></div>
                        })}
                                            </dl>
                                            {entry.notes.clone().map(|notes| view! {
                                                <p class="mt-2 text-base-content/70">{notes}</p>
                                            })}
                                            let tags = entry.appearance_tags.clone();
                                            {(!tags.is_empty()).then(move || {
                                                view! {
                                                        <p class="flex flex-wrap gap-1.5 mt-2">
                                                            {tags
                                                                .iter()
                                                                .map(|tag| {
                                                                    let label = labels::appearance_label(locale, tag);
                                                                    view! { <span class="badge badge-sm badge-ghost">{label}</span> }
                                                                })
                                                                .collect::<Vec<_>>()}
                                                        </p>
                                                }
                                            })}
                                            <p class="text-base-content/70 text-sm">
                                                {local_stamp(&entry.occurred_at)}
                                            </p>
                                        </li>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </ul>
                    <Show when=move || actions.get().is_empty()>
                        <p class="py-6 text-center text-base-content/60">{t!(i18n, grow.nothing_logged)}</p>
                    </Show>
                </div>

                <div>
                    <h3>{t!(i18n, grow.plants)}</h3>
                    <ul class="grid gap-4 lg:grid-cols-2">
                        {move || {
                            plants
                                .get()
                                .into_iter()
                                .map(|plant| {
                                    view! {
                                        <li class="card bg-base-100 shadow-sm px-4 py-3 my-3">
                                            <A href=route_href(&format!("/plants/{}", plant.id))>
                                                {plant.name.clone()}
                                            </A>
                                        </li>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </ul>
                    <button class="btn btn-sm" on:click=add_plant>{t!(i18n, grow.add_plant)}</button>

                    <h3>{t!(i18n, grow.photos)}</h3>
                    <label class="fieldset">
                        <span class="fieldset-label">{t!(i18n, grow.upload_label)}</span>
                        <input type="file" class="file-input file-input-sm" accept="image/jpeg,image/png,image/webp" multiple
                            on:change=upload
                        />
                    </label>
                    {move || upload_note.get().map(|note| view! { <p class="text-base-content/70 text-sm">{note}</p> })}
                    <div class="grid gap-2 mt-3 grid-cols-[repeat(auto-fill,minmax(7rem,1fr))]">
                        {move || {
                            let locale = i18n.get_locale();
                            photos
                                .get()
                                .into_iter()
                                .map(|photo| {
                                    let url = format!("api/photos/{}/thumbnail", photo.id);
                                    let full = format!("api/photos/{}/original", photo.id);
                                    let alt = t_string!(locale, grow.day, day = photo.grow_day.to_string());
                                    view! {
                                        <a href=full target="_blank" rel="noreferrer"
                                            class="grid gap-1 text-center no-underline"
                                        >
                                            <img class="w-full h-auto rounded-lg border border-base-300"
                                                src=url alt=alt />
                                            <span class="text-base-content/70 text-sm">{t_string!(locale, grow.day, day = photo.grow_day.to_string())}</span>
                                        </a>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </div>
                </div>
            </div>

            <p><A href=route_href("/grows")>{t!(i18n, grow.all_grows)}</A></p>
        </section>
    }
}

/// The create/edit form fields for one grow (spec §4.2): identity,
/// strain label, photoperiod, substrate, start date, status and
/// notes. Used by the grows page (create) and the grow edit page
/// (edit, grow-page rework ticket 01).
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn GrowForm(draft: RwSignal<GrowInput>) -> impl IntoView {
    view! {
        <div class="grid gap-3 grid-cols-[repeat(auto-fill,minmax(14rem,1fr))]">
            <label class="fieldset">
                <span class="fieldset-label">"Name *"</span>
                <input class="input" type="text" required placeholder="Closet run"
                    prop:value=move || draft.get().name
                    on:input=move |ev| draft.update(|d| d.name = event_target_value(&ev))
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Strain label"</span>
                <input class="input" type="text" placeholder="Northern Lights #5"
                    prop:value=move || draft.get().strain_label
                    on:input=move |ev| {
                        draft.update(|d| d.strain_label = event_target_value(&ev));
                    }
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Photoperiod"</span>
                <select class="select"
                    prop:value=move || draft.get().photoperiod.label().to_owned()
                    on:change=move |ev| {
                        let value = event_target_value(&ev);
                        draft.update(|d| {
                            d.photoperiod = PhotoperiodType::from_label(&value)
                                .unwrap_or_default();
                        });
                    }
                >
                    {photoperiod_options()}
                </select>
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Substrate *"</span>
                <select class="select"
                    prop:value=move || draft.get().substrate
                    on:change=move |ev| draft.update(|d| d.substrate = event_target_value(&ev))
                >
                    {leaflog_core::grows::SUBSTRATES
                        .iter()
                        .map(|substrate| view! { <option value=*substrate>{*substrate}</option> })
                        .collect::<Vec<_>>()}
                </select>
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Started (seed/clone) *"</span>
                <input class="input" type="date" required
                    prop:value=move || draft.get().started_on
                    on:input=move |ev| draft.update(|d| d.started_on = event_target_value(&ev))
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Planned vegetative weeks"</span>
                <input class="input" type="number" min="1" max="52"
                    placeholder="—"
                    prop:value=move || draft
                        .get()
                        .planned_veg_weeks
                        .map_or(String::new(), |weeks| weeks.to_string())
                    on:input=move |ev| {
                        let text = event_target_value(&ev);
                        draft.update(|d| {
                            d.planned_veg_weeks =
                                text.parse::<u8>().ok().filter(|weeks| *weeks > 0);
                        });
                    }
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Status"</span>
                <select class="select"
                    prop:value=move || draft.get().status.label().to_owned()
                    on:change=move |ev| {
                        let value = event_target_value(&ev);
                        draft.update(|d| {
                            d.status = if value == GrowStatus::Archived.label() {
                                GrowStatus::Archived
                            } else {
                                GrowStatus::Active
                            };
                        });
                    }
                >
                    <option value=GrowStatus::Active.label()>{GrowStatus::Active.label()}</option>
                    <option value=GrowStatus::Archived.label()>{GrowStatus::Archived.label()}</option>
                </select>
            </label>
            <label class="fieldset col-span-full">
                <span class="fieldset-label">"Notes"</span>
                <textarea class="textarea" rows="2"
                    prop:value=move || draft.get().notes.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.notes = non_empty(event_target_value(&ev)))
                />
            </label>
            // The tuning knobs (D075) live collapsed under Advanced
            // settings so the daily flow stays simple; every default
            // is sensible, so expanding this is always optional.
            <details class="col-span-full">
                <summary class="cursor-pointer select-none text-sm font-medium">
                    "Advanced settings"
                </summary>
                <div class="grid gap-3 mt-2 grid-cols-[repeat(auto-fill,minmax(14rem,1fr))]">
                    <label class="fieldset">
                        <span class="fieldset-label">"Heat tolerance"</span>
                        <select class="select"
                            prop:value=move || draft.get().heat_tolerance.label().to_owned()
                            on:change=move |ev| {
                                let value = event_target_value(&ev);
                                draft.update(|d| {
                                    d.heat_tolerance = Tolerance::from_label(&value)
                                        .unwrap_or_default();
                                });
                            }
                        >
                            {tolerance_options()}
                        </select>
                    </label>
                    <label class="fieldset">
                        <span class="fieldset-label">"Cold tolerance"</span>
                        <select class="select"
                            prop:value=move || draft.get().cold_tolerance.label().to_owned()
                            on:change=move |ev| {
                                let value = event_target_value(&ev);
                                draft.update(|d| {
                                    d.cold_tolerance = Tolerance::from_label(&value)
                                        .unwrap_or_default();
                                });
                            }
                        >
                            {tolerance_options()}
                        </select>
                    </label>
                    <label class="fieldset">
                        <span class="fieldset-label">"Light tolerance"</span>
                        <select class="select"
                            prop:value=move || draft.get().light_tolerance.label().to_owned()
                            on:change=move |ev| {
                                let value = event_target_value(&ev);
                                draft.update(|d| {
                                    d.light_tolerance = Tolerance::from_label(&value)
                                        .unwrap_or_default();
                                });
                            }
                        >
                            {tolerance_options()}
                        </select>
                    </label>
                    <label class="fieldset">
                        <span class="fieldset-label">"Real wall watts"</span>
                        <input class="input" type="number" min="0" step="0.1"
                            placeholder="—"
                            prop:value=move || draft
                                .get()
                                .real_watts
                                .map_or(String::new(), |watts| watts.to_string())
                            on:input=move |ev| {
                                let text = event_target_value(&ev);
                                draft.update(|d| {
                                    d.real_watts = text
                                        .parse::<f64>()
                                        .ok()
                                        .filter(|watts| *watts >= 0.0 && watts.is_finite());
                                });
                            }
                        />
                    </label>
                    <label class="fieldset">
                        <span class="fieldset-label">"Dimmable"</span>
                        <input class="checkbox" type="checkbox"
                            prop:checked=move || draft.get().dimmable
                            on:change=move |ev| {
                                draft.update(|d| d.dimmable = event_target_checked(&ev));
                            }
                        />
                    </label>
                </div>
            </details>
        </div>
    }
}

/// The chip form: shared fields plus the ones relevant to the chosen
/// action type.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
fn ActionForm(draft: RwSignal<ActionInput>, kind: RwSignal<Option<ActionType>>) -> impl IntoView {
    view! {
        <div class="grid gap-3 grid-cols-[repeat(auto-fill,minmax(14rem,1fr))]">
            <label class="fieldset">
                <span class="fieldset-label">"When (UTC) *"</span>
                <input class="input" type="datetime-local" required
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

            <label class="fieldset col-span-full">
                <span class="fieldset-label">"Notes"</span>
                <textarea class="textarea" rows="2"
                    prop:value=move || draft.get().notes.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.notes = non_empty(event_target_value(&ev)))
                />
            </label>
        </div>
        <fieldset class="fieldset mt-3">
            <legend class="fieldset-legend">"Appearance"</legend>
            {leaflog_core::appearance::Category::all()
                .into_iter()
                .map(|category| {
                    let traits = leaflog_core::appearance::TRAITS
                        .iter()
                        .filter(|entry| entry.category == category);
                    view! {
                        <div class="mt-2">
                            <p class="text-base-content/60 text-xs uppercase tracking-wide">
                                {category.label()}
                            </p>
                            <div class="flex flex-wrap gap-x-4 gap-y-1 mt-1">
                                {traits
                                    .into_iter()
                                    .map(|entry| {
                                        let entry = *entry;
                                        view! {
                                            <label
                                                class="flex items-center gap-2 cursor-pointer"
                                                title=entry.meaning
                                            >
                                                <input
                                                    type="checkbox"
                                                    class="checkbox checkbox-sm"
                                                    on:change=move |ev| {
                                                        let tag = entry.tag.to_owned();
                                                        let checked =
                                                            event_target_checked(&ev);
                                                        draft.update(|d| {
                                                            d.appearance_tags
                                                                .retain(|existing| {
                                                                    existing != &tag
                                                                });
                                                            if checked {
                                                                d.appearance_tags.push(tag);
                                                            }
                                                        });
                                                    }
                                                />
                                                <span class="text-sm">{entry.label}</span>
                                            </label>
                                        }
                                    })
                                    .collect::<Vec<_>>()}
                            </div>
                        </div>
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
        <label class="fieldset">
            <span class="fieldset-label">{label}</span>
            <input class="input" type="number" step=step
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

/// How long one grows-page list request may run before it is
/// abandoned and tried again (a stalled request must never pin the
/// page).
const REQUEST_TIMEOUT_MS: u32 = 10_000;

/// Fetches a JSON list for the grows page. A stalled request is
/// retried once by itself, so the page heals without a hard refresh;
/// a request that keeps timing out gives up. The timeout semantics
/// live in [`retry_once_after_timeout`], which is generic and
/// unit-tested natively — gloo's timer and request types are
/// wasm-only, so this wrapper only wires them up.
// WASM is single-threaded; the gloo timer/request types are not `Send`
// and `spawn_local` does not require it (same allowance as api.rs).
#[allow(clippy::future_not_send)]
async fn fetch_list<T: DeserializeOwned>(path: &str, timeout_message: String) -> Result<T, ApiFailure> {
    retry_once_after_timeout(
        || api::get::<T>(path),
        || TimeoutFuture::new(REQUEST_TIMEOUT_MS),
        || ApiFailure {
            status: 0,
            message: timeout_message.clone(),
        },
    )
    .await
}

/// The grows-page list recovery (ticket 02) as pure control flow: at
/// most two attempts, each raced against a fresh deadline. A stalled
/// attempt — [`race_timeout`] yielding `None` — is retried once by
/// itself; any other outcome is final, and a second stall gives up
/// with `give_up()`. Generic over the attempt and deadline factories
/// so the timeout/retry semantics are unit-testable natively (gloo-net
/// and gloo-timers themselves are not).
// WASM is single-threaded; the gloo timer/request types are not `Send`
// and `spawn_local` does not require it (same allowance as api.rs).
#[allow(clippy::future_not_send)]
async fn retry_once_after_timeout<T, E, Attempt, Deadline>(
    mut attempt: impl FnMut() -> Attempt,
    mut deadline: impl FnMut() -> Deadline,
    give_up: impl FnOnce() -> E,
) -> Result<T, E>
where
    Attempt: Future<Output = Result<T, E>>,
    Deadline: Future<Output = ()>,
{
    for _ in 0..2 {
        match race_timeout(attempt(), deadline()).await {
            Some(Ok(value)) => return Ok(value),
            Some(Err(failure)) => return Err(failure),
            // The deadline lapsed with the request still in flight:
            // fall through to the retry — or, on the second attempt,
            // to the give-up error.
            None => {}
        }
    }
    Err(give_up())
}

/// Races one task against one deadline: `Some(outcome)` when the task
/// lands first, `None` once the deadline lapses (the losing task
/// future is dropped with the race, so a stalled request never
/// lingers).
// WASM is single-threaded; the gloo timer/request types are not `Send`
// and `spawn_local` does not require it (same allowance as api.rs).
#[allow(clippy::future_not_send)]
async fn race_timeout<T, D>(task: impl Future<Output = T>, deadline: D) -> Option<T>
where
    D: Future<Output = ()>,
{
    futures::pin_mut!(task);
    futures::pin_mut!(deadline);
    match futures::future::select(task, deadline).await {
        Either::Left((outcome, _timer)) => Some(outcome),
        Either::Right(((), _task)) => None,
    }
}

/// A blank grow draft.
pub fn empty_grow() -> GrowInput {
    GrowInput {
        name: String::new(),
        feed_template_id: None,
        planned_veg_weeks: None,
        strain_label: String::new(),
        photoperiod: PhotoperiodType::default(),
        heat_tolerance: Tolerance::default(),
        cold_tolerance: Tolerance::default(),
        light_tolerance: Tolerance::default(),
        real_watts: None,
        dimmable: true,
        substrate: "soil".to_owned(),
        started_on: String::new(),
        status: GrowStatus::default(),
        notes: None,
    }
}

/// A blank action draft: a `note` with no structured fields,
/// timestamped now (UTC) in the shape a `datetime-local` input
/// accepts.
pub fn empty_action() -> ActionInput {
    ActionInput {
        plant_ids: Vec::new(),
        occurred_at: datetime_local_now(),
        action_type: ActionType::Note,
        details: ActionDetails::default(),
        notes: None,
        appearance_tags: Vec::new(),
    }
}

/// `Date::toISOString()` yields `YYYY-MM-DDTHH:MM:SS.sssZ`; a
/// `datetime-local` input's sanitization rejects the seconds and
/// offset tail, leaving the field empty (`required` then blocks
/// submit). The value it accepts is the `YYYY-MM-DDTHH:MM` prefix;
/// the backend normalizes it back to RFC 3339 UTC on save.
fn datetime_local_now() -> String {
    js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_default()
        .get(..16)
        .unwrap_or_default()
        .to_owned()
}

/// The browser's locale rendering of an RFC 3339 timestamp (`Nov 15,
/// 2026, 7:20 PM`): the timeline is a diary read by a human in their
/// own zone, so machine-formatted UTC would read like an error.
/// Explicit components (no seconds — diary precision is minutes);
/// falls back to the raw string when the browser APIs refuse the
/// value.
fn local_stamp(iso: &str) -> String {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(iso));
    if date.get_time().is_nan() {
        return iso.to_owned();
    }
    let Ok(fmt) = js_sys::Reflect::get(&date, &wasm_bindgen::JsValue::from_str("toLocaleString"))
        .and_then(wasm_bindgen::JsValue::dyn_into::<js_sys::Function>)
    else {
        return iso.to_owned();
    };
    let options = js_sys::Object::new();
    for (key, value) in [
        ("year", "numeric"),
        ("month", "short"),
        ("day", "numeric"),
        ("hour", "numeric"),
        ("minute", "2-digit"),
    ] {
        let _ = js_sys::Reflect::set(
            &options,
            &wasm_bindgen::JsValue::from_str(key),
            &wasm_bindgen::JsValue::from_str(value),
        );
    }
    fmt.call1(&date, &options)
        .ok()
        .and_then(|value| value.as_string())
        .unwrap_or_else(|| iso.to_owned())
}

/// The spot-it-without-reading glyph shown beside a grow's medium in
/// the header cluster (grow-page spec): one emoji per known
/// substrate, in the house aria-hidden-span style. A user-defined
/// medium gets no glyph — an unknown value must never render a lie.
fn substrate_glyph(substrate: &str) -> Option<&'static str> {
    match substrate {
        "soil" => Some("🪴"),
        "coco" => Some("🥥"),
        "hydro" => Some("💧"),
        _ => None,
    }
}

/// The Photoperiod/Autoflower choices for a grow form select: stable
/// enum-label values, glossary terms as the visible text. Shared by
/// the grow form and the onboarding wizard's grow step.
pub fn photoperiod_options() -> impl IntoView {
    [
        (PhotoperiodType::Photoperiod, "Photoperiod"),
        (PhotoperiodType::Auto, "Autoflower"),
    ]
    .into_iter()
    .map(|(kind, display)| {
        view! { <option value=kind.label()>{display}</option> }
    })
    .collect::<Vec<_>>()
}

/// The Sensitive/Average/Tolerant choices for a grow form select.
fn tolerance_options() -> impl IntoView {
    [
        (Tolerance::Sensitive, "Sensitive"),
        (Tolerance::Average, "Average"),
        (Tolerance::Tolerant, "Tolerant"),
    ]
    .into_iter()
    .map(|(level, display)| {
        view! { <option value=level.label()>{display}</option> }
    })
    .collect::<Vec<_>>()
}

#[cfg(test)]
mod landing_pick_tests {
    use super::pick_landing_grow;
    use leaflog_core::grows::{Grow, GrowStatus, PhotoperiodType, Tolerance};

    fn grow(id: &str, status: GrowStatus, updated_at: &str) -> Grow {
        Grow {
            id: id.to_owned(),
            name: id.to_owned(),
            feed_template_id: None,
            planned_veg_weeks: None,
            strain_label: String::new(),
            photoperiod: PhotoperiodType::default(),
            heat_tolerance: Tolerance::default(),
            cold_tolerance: Tolerance::default(),
            light_tolerance: Tolerance::default(),
            real_watts: None,
            dimmable: true,
            light_cycle: None,
            substrate: "soil".to_owned(),
            started_on: "2026-01-01".to_owned(),
            status,
            notes: None,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: updated_at.to_owned(),
        }
    }

    #[test]
    fn prefers_the_newest_active_grow() {
        let grows = [
            grow("old-active", GrowStatus::Active, "2026-01-02T00:00:00Z"),
            grow(
                "newest-archived",
                GrowStatus::Archived,
                "2026-03-01T00:00:00Z",
            ),
            grow("newest-active", GrowStatus::Active, "2026-02-01T00:00:00Z"),
        ];
        assert_eq!(pick_landing_grow(&grows), "newest-active");
    }

    #[test]
    fn falls_back_to_the_newest_grow_when_none_is_active() {
        let grows = [
            grow(
                "older-archived",
                GrowStatus::Archived,
                "2026-01-01T00:00:00Z",
            ),
            grow(
                "newest-archived",
                GrowStatus::Archived,
                "2026-03-01T00:00:00Z",
            ),
        ];
        assert_eq!(pick_landing_grow(&grows), "newest-archived");
    }

    #[test]
    fn empty_when_there_are_no_grows() {
        assert_eq!(pick_landing_grow(&[]), "");
    }
}

#[cfg(test)]
mod substrate_glyph_tests {
    use super::substrate_glyph;

    #[test]
    fn known_substrates_get_their_glyph() {
        assert_eq!(substrate_glyph("soil"), Some("🪴"));
        assert_eq!(substrate_glyph("coco"), Some("🥥"));
        assert_eq!(substrate_glyph("hydro"), Some("💧"));
    }

    // A user-defined medium must never render a glyph that claims
    // something the grower did not pick.
    #[test]
    fn unknown_or_empty_substrate_gets_no_glyph() {
        assert_eq!(substrate_glyph("perlite"), None);
        assert_eq!(substrate_glyph(""), None);
    }
}

#[cfg(test)]
mod timeout_retry_tests {
    use super::retry_once_after_timeout;
    use std::cell::Cell;
    use std::future::{Future, pending, ready};
    use std::pin::Pin;
    use std::time::Duration;

    /// A native deadline that lapses after `ms` milliseconds (std
    /// thread + oneshot). gloo's timer is wasm-only, so the tests
    /// drive the retry loop with plain futures instead.
    fn native_deadline(ms: u64) -> Pin<Box<dyn Future<Output = ()>>> {
        let (sender, receiver) = futures::channel::oneshot::channel();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(ms));
            let _ = sender.send(());
        });
        Box::pin(async move {
            let _ = receiver.await;
        })
    }

    // The ticket-02 recovery: a stalled request is retried by itself,
    // and the page takes the retry's answer — a fresh attempt, not
    // the future that never answered.
    #[test]
    fn stalled_first_attempt_is_retried_and_the_retry_lands() {
        let attempts = Cell::new(0_u32);
        let outcome = futures::executor::block_on(retry_once_after_timeout(
            || -> Pin<Box<dyn Future<Output = Result<&'static str, &'static str>>>> {
                let stalled = attempts.get() == 0;
                attempts.set(attempts.get() + 1);
                if stalled {
                    Box::pin(pending()) // the stalled request never answers
                } else {
                    Box::pin(ready(Ok("landed"))) // the retry answers
                }
            },
            || native_deadline(1),
            || "gave up",
        ));
        assert_eq!(outcome, Ok("landed"));
        assert_eq!(attempts.get(), 2);
    }

    // A request that keeps timing out gives up: two attempts total,
    // then the give-up error — never a page pinned forever.
    #[test]
    fn kept_stalling_gives_up_after_one_retry() {
        let attempts = Cell::new(0_u32);
        let outcome = futures::executor::block_on(retry_once_after_timeout(
            || -> Pin<Box<dyn Future<Output = Result<&'static str, &'static str>>>> {
                attempts.set(attempts.get() + 1);
                Box::pin(pending()) // every attempt stalls
            },
            || native_deadline(1),
            || "gave up",
        ));
        assert_eq!(outcome, Err("gave up"));
        assert_eq!(attempts.get(), 2);
    }
}
