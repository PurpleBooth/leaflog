//! Grows UI (spec §4.2, §4.3): grow list/creation and the grow detail
//! page with quick-add action chips, the timeline, plant management and
//! the photo gallery with uploads.

use futures::future::Either;
use gloo_timers::future::TimeoutFuture;
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
use serde::de::DeserializeOwned;
use std::future::Future;

use crate::api::{self, ApiFailure};
use crate::dashboard::EnvDashboard;
use crate::equipment::ErrorIf;
use crate::feed::FeedPanel;
use crate::gantt::GanttTimeline;
use crate::live::LivePanel;
use crate::route_href;
use crate::spaces::SpaceDetail;

/// The grows overview page.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn GrowsPage() -> impl IntoView {
    let grows = RwSignal::<Vec<Grow>>::new(Vec::new());
    let strains = RwSignal::<Vec<Strain>>::new(Vec::new());
    let spaces = RwSignal::<Vec<SpaceDetail>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);
    let loading = RwSignal::<bool>::new(true);
    let form_open = RwSignal::<bool>::new(false);
    let draft = RwSignal::<GrowInput>::new(empty_grow());

    // Each list resource fetches on its own: the grows list renders
    // from its own request whatever the strains or spaces requests
    // did, and every failure lands in the shared error banner.
    let refresh = move || {
        loading.set(true);
        spawn_local(async move {
            // All three in flight at once: the list lands as soon as its
            // own request answers, however long strains or spaces take.
            let (grow_list, strain_list, space_list) = futures::join!(
                fetch_list::<Vec<Grow>>("api/grows"),
                fetch_list::<Vec<Strain>>("api/strains"),
                fetch_list::<Vec<SpaceDetail>>("api/spaces"),
            );
            let mut failures = Vec::new();
            match grow_list {
                Ok(list) => grows.set(list),
                Err(failure) => failures.push(failure.message),
            }
            match strain_list {
                Ok(list) => strains.set(list),
                Err(failure) => failures.push(failure.message),
            }
            match space_list {
                Ok(list) => spaces.set(list),
                Err(failure) => failures.push(failure.message),
            }
            error.set(if failures.is_empty() {
                None
            } else {
                Some(failures.join("; "))
            });
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
                <h2>"Grows"</h2>
                <button class="btn btn-primary btn-sm" on:click=move |_| {
                    draft.set(empty_grow());
                    form_open.set(true);
                }>"Start a grow"</button>
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
                    "Retry"
                </button>
            </Show>

            <Show when=move || form_open.get()>
                <form class="card bg-base-100 shadow-sm px-5 py-4 my-4" on:submit=save>
                    <h3>"New grow"</h3>
                    <GrowForm draft strains spaces />
                    <div class="flex flex-wrap gap-2 mt-4">
                        <button class="btn btn-primary btn-sm" type="submit">"Save"</button>
                        <button class="btn btn-sm" type="button" on:click=move |_| form_open.set(false)>"Cancel"</button>
                    </div>
                </form>
            </Show>

            <ul class="grid gap-4 lg:grid-cols-2">
                {move || {
                    let space_of = |id: &Option<String>| {
                        id.as_ref().and_then(|space_id| {
                            spaces
                                .get()
                                .into_iter()
                                .find(|detail| detail.space.id == *space_id)
                        })
                    };
                    grows
                        .get()
                        .into_iter()
                        .map(|grow| {
                            let space = space_of(&grow.grow_space_id);
                            view! {
                                <li class="card bg-base-100 shadow-sm px-5 py-4">
                                    <div class="flex flex-wrap items-baseline gap-2">
                                        <A href=route_href(&format!("/grows/{}", grow.id))>
                                            <strong>{grow.name.clone()}</strong>
                                        </A>
                                        <span class="badge badge-ghost">{grow.status.label()}</span>
                                        <span class="text-base-content/70">{grow.substrate.clone()}</span>
                                        {space.map(|detail| view! {
                                            <span class="badge badge-ghost">
                                                {detail.space.name}
                                            </span>
                                        })}
                                    </div>
                                    <p class="text-base-content/70 text-sm">"Started "{grow.started_on}</p>
                                </li>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </ul>
            <Show when=move || loading.get()>
                <p class="py-6 text-center text-base-content/70">"Loading grows…"</p>
            </Show>
            <Show
                when=move || {
                    !loading.get()
                        && grows.get().is_empty()
                        && error.get().is_none()
                }
            >
                <p class="py-6 text-center text-base-content/60">"No grows yet — start your first one."</p>
            </Show>
            <p><A href=route_href("/")>"Back to home"</A></p>
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
    let spaces = RwSignal::<Vec<SpaceDetail>>::new(Vec::new());
    let strains = RwSignal::<Vec<Strain>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);

    let chip = RwSignal::<Option<ActionType>>::new(None);
    let draft = RwSignal::<ActionInput>::new(empty_action());
    let upload_note = RwSignal::<Option<String>>::new(None);
    let edit_open = RwSignal::<bool>::new(false);
    let grow_draft = RwSignal::<GrowInput>::new(empty_grow());
    let deleted = RwSignal::<bool>::new(false);

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
            match api::get::<Vec<SpaceDetail>>("api/spaces").await {
                Ok(value) => spaces.set(value),
                Err(failure) => error.set(Some(failure.message)),
            }
            match api::get::<Vec<Strain>>("api/strains").await {
                Ok(value) => strains.set(value),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };
    Effect::new(move |_| {
        params.track();
        refresh();
    });

    // prevent_default first — see docs/DECISIONS.md D061.
    let save_action = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
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

    // Opens the edit form pre-filled from the payload the header
    // cloned out of the current grow.
    let start_edit = move |payload: &GrowInput| {
        grow_draft.set(payload.clone());
        edit_open.set(true);
    };

    // prevent_default first — see docs/DECISIONS.md D061.
    let save_edit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let id = grow_id();
        spawn_local(async move {
            let input = grow_draft.get();
            match api::put::<GrowInput, Grow>(&format!("api/grows/{id}"), &input).await {
                Ok(_) => {
                    edit_open.set(false);
                    error.set(None);
                    refresh();
                }
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    // Deleting a grow takes its plants, log entries and photos with it,
    // so it asks for confirmation first.
    let delete_grow = move |_ev: leptos::ev::MouseEvent| {
        let id = grow_id();
        let name = grow.get().map(|current| current.name).unwrap_or_default();
        let confirmed = leptos::web_sys::window()
            .and_then(|window| {
                window
                    .confirm_with_message(&format!(
                        "Delete '{name}' with all its plants, log entries and photos? \
                         This cannot be undone."
                    ))
                    .ok()
            })
            .unwrap_or(false);
        if !confirmed {
            return;
        }
        spawn_local(async move {
            match api::delete(&format!("api/grows/{id}")).await {
                Ok(()) => deleted.set(true),
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
        <section class="pb-12">
            <Show when=move || deleted.get()>
                // The plain route path: unlike an <A href>, Redirect
                // routes through navigate(), which resolves the path
                // against the router base itself — a base-prefixed
                // path would double the ingress prefix.
                <leptos_router::components::Redirect path="/grows" />
            </Show>
            {move || {
                grow.get().map_or_else(
                    || ().into_any(),
                    |current| {
                        let space_id = current.grow_space_id.clone();
                        let strain_id = current.strain_id.clone();
                        let archived = current.status == GrowStatus::Archived;
                        view! {
                            <div>
                                <div class="flex flex-wrap items-center justify-between gap-4">
                                    // One left-aligned quiet cluster:
                                    // name · glyph + medium · strain,
                                    // with the status and space badges
                                    // joining it. The buttons are the
                                    // row's second child, so
                                    // justify-between keeps them clear
                                    // on the right. A grow with no
                                    // strain (or an unresolvable one)
                                    // leaves the strain slot empty —
                                    // no placeholder.
                                    <div class="flex flex-wrap items-center gap-2">
                                        <h2>{current.name.clone()}</h2>
                                        <span class="badge badge-ghost">
                                            {substrate_glyph(&current.substrate).map(|glyph| view! {
                                                <span aria-hidden="true">{glyph}</span>
                                            })}
                                            {current.substrate.clone()}
                                        </span>
                                        {move || {
                                            strain_id
                                                .as_ref()
                                                .and_then(|id| {
                                                    strains
                                                        .get()
                                                        .into_iter()
                                                        .find(|strain| strain.id == *id)
                                                })
                                                .map(|strain| view! {
                                                    <span class="badge badge-ghost">
                                                        {strain.name}
                                                    </span>
                                                })
                                        }}
                                        {archived
                                            .then(|| view! {
                                                <span class="badge badge-ghost">
                                                    {current.status.label()}
                                                </span>
                                            })}
                                        {move || {
                                            space_id.as_ref().and_then(|id| {
                                                spaces
                                                    .get()
                                                    .into_iter()
                                                    .find(|detail| detail.space.id == *id)
                                            })
                                            .map(|detail| view! {
                                                <A href=route_href(
                                                    &format!("/spaces/{}", detail.space.id),
                                                )>
                                                    <span class="badge badge-ghost">
                                                        {detail.space.name.clone()}
                                                    </span>
                                                </A>
                                            })
                                        }}
                                    </div>
                                    {let for_edit = GrowInput::from_grow(&current);
                                    view! {
                                        <div class="flex flex-wrap gap-2">
                                            <button class="btn btn-sm" on:click=move |_| {
                                                start_edit(&for_edit);
                                            }>"Edit grow"</button>
                                            <button class="btn btn-error btn-outline btn-sm"
                                                on:click=delete_grow>
                                                "Delete grow"
                                            </button>
                                        </div>
                                    }}
                                </div>
                                <p class="text-base-content/70">"Started "{current.started_on}</p>
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

            <Show when=move || edit_open.get()>
                <form class="card bg-base-100 shadow-sm px-5 py-4 my-4" on:submit=save_edit>
                    <h3>"Edit grow"</h3>
                    <GrowForm draft=grow_draft strains spaces />
                    <div class="flex flex-wrap gap-2 mt-4">
                        <button class="btn btn-primary btn-sm" type="submit">"Save"</button>
                        <button class="btn btn-sm" type="button" on:click=move |_| edit_open.set(false)>
                            "Cancel"
                        </button>
                    </div>
                </form>
            </Show>

            <h3>"Forecast"</h3>
            <GanttTimeline grow_id=grow_id() />

            <EnvDashboard grow_id=grow_id() />

            <h3>"Feed schedule"</h3>
            <FeedPanel grow_id=grow_id() />

            <h3>"Quick add"</h3>
            <nav class="flex flex-wrap gap-2 my-2" aria-label="Quick add action">
                {ActionType::all()
                    .into_iter()
                    .map(|kind| {
                        let current = chip;
                        view! {
                            <button
                                class=move || if current.get() == Some(kind) {
                                    "btn btn-sm rounded-full btn-primary"
                                } else {
                                    "btn btn-sm rounded-full"
                                }
                                on:click=move |_| {
                                    let selected = current.get() == Some(kind);
                                    let next = (!selected).then_some(kind);
                                    // The chip picks the entry's type; the
                                    // form below shows its fields. A plain
                                    // reset left the draft as `note`, so
                                    // every quick add was logged as one
                                    // (found by the e2e suite, D069).
                                    let mut reset = empty_action();
                                    if let Some(next) = next {
                                        reset.action_type = next;
                                    }
                                    current.set(next);
                                    draft.set(reset);
                                }
                            >
                                {kind.display()}
                            </button>
                        }
                    })
                    .collect::<Vec<_>>()}
            </nav>

            <Show when=move || chip.get().is_some()>
                <form class="card bg-base-100 shadow-sm px-5 py-4 my-4" on:submit=save_action>
                    <ActionForm draft kind=chip />
                    <div class="flex flex-wrap gap-2 mt-4">
                        <button class="btn btn-primary btn-sm" type="submit">"Log it"</button>
                        <button class="btn btn-sm" type="button" on:click=move |_| chip.set(None)>"Cancel"</button>
                    </div>
                </form>
            </Show>

            <div class="grid gap-6 items-start lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
                <div>
                    <h3>"Timeline"</h3>
                    <ul class="list-none m-0 p-0 grid gap-2">
                        {move || {
                            actions
                                .get()
                                .into_iter()
                                .map(|entry| {
                                    view! {
                                        <li class="card bg-base-100 shadow-sm px-5 py-4">
                                            <div class="flex flex-wrap items-baseline gap-2">
                                                <strong>{entry.action_type.display()}</strong>
                                                <span class="badge badge-ghost">{"day "}{entry.grow_day}</span>
                                                {entry.phase.map(|phase| view! {
                                                    <span class="badge badge-ghost">{phase.label()}</span>
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
                                                <p class="mt-2 text-base-content/70">{notes}</p>
                                            })}
                                            {(!entry.appearance_tags.is_empty()).then(|| {
                                                view! {
                                                    <p class="flex flex-wrap gap-1.5 mt-2">
                                                        {entry
                                                            .appearance_tags
                                                            .iter()
                                                            .map(|tag| {
                                                                view! { <span class="badge badge-sm badge-ghost">{tag.clone()}</span> }
                                                            })
                                                            .collect::<Vec<_>>()}
                                                    </p>
                                                }
                                            })}
                                            <p class="text-base-content/70 text-sm">{entry.occurred_at.clone()}</p>
                                        </li>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </ul>
                    <Show when=move || actions.get().is_empty()>
                        <p class="py-6 text-center text-base-content/60">"Nothing logged yet."</p>
                    </Show>
                </div>

                <div>
                    <LivePanel grow_id=grow_id() />
                    <h3>"Plants"</h3>
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
                    <button class="btn btn-sm" on:click=add_plant>"Add plant"</button>

                    <h3>"Photos"</h3>
                    <label class="fieldset">
                        <span class="fieldset-label">"Upload photos (JPEG/PNG/WebP; EXIF/GPS stripped on store)"</span>
                        <input type="file" class="file-input file-input-sm" accept="image/jpeg,image/png,image/webp" multiple
                            on:change=upload
                        />
                    </label>
                    {move || upload_note.get().map(|note| view! { <p class="text-base-content/70 text-sm">{note}</p> })}
                    <div class="grid gap-2 mt-3 grid-cols-[repeat(auto-fill,minmax(7rem,1fr))]">
                        {move || {
                            photos
                                .get()
                                .into_iter()
                                .map(|photo| {
                                    let url = format!("api/photos/{}/thumbnail", photo.id);
                                    let full = format!("api/photos/{}/original", photo.id);
                                    view! {
                                        <a href=full target="_blank" rel="noreferrer"
                                            class="grid gap-1 text-center no-underline"
                                        >
                                            <img class="w-full h-auto rounded-lg border border-base-300"
                                                src=url alt=format!("day {}", photo.grow_day) />
                                            <span class="text-base-content/70 text-sm">{"day "}{photo.grow_day}</span>
                                        </a>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </div>
                </div>
            </div>

            <p><A href=route_href("/grows")>"All grows"</A></p>
        </section>
    }
}

/// The create/edit form fields for one grow (spec §4.2): identity,
/// strain, space, substrate, start date, status and notes. Used by both
/// the grows page (create) and the grow detail page (edit).
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
fn GrowForm(
    draft: RwSignal<GrowInput>,
    strains: RwSignal<Vec<Strain>>,
    spaces: RwSignal<Vec<SpaceDetail>>,
) -> impl IntoView {
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
                <span class="fieldset-label">"Strain"</span>
                <select class="select"
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
                        // Freshly mounted <option> nodes carry no selected
                        // state, so an options list that arrives late (or is
                        // refetched after a save) resets the visible pick to
                        // the placeholder while the draft keeps the strain.
                        // Encode the draft's pick into the rebuilt options:
                        // the matching node mounts `selected` and the browser
                        // honours it at insertion, whatever order the effects
                        // run in. Untracked: `prop:value` already re-applies
                        // the value when the draft itself changes. That
                        // mount-time `selected` goes stale after a manual
                        // re-pick, until the next options rebuild — inert
                        // today (`select.value` governs, and no form reset
                        // exists), noted so a future reset button doesn't
                        // regress the pick.
                        let picked = draft.get_untracked().strain_id;
                        strains
                            .get()
                            .into_iter()
                            .map(|strain| {
                                let is_selected =
                                    picked.as_deref() == Some(strain.id.as_str());
                                view! {
                                    <option value=strain.id selected=is_selected>
                                        {strain.name}
                                    </option>
                                }
                            })
                            .collect::<Vec<_>>()
                    }}
                </select>
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Space"</span>
                <select class="select"
                    prop:value=move || draft.get().grow_space_id.unwrap_or_default()
                    on:change=move |ev| {
                        let value = event_target_value(&ev);
                        draft.update(|d| {
                            d.grow_space_id = (!value.is_empty()).then_some(value);
                        });
                    }
                >
                    <option value="">"— no space —"</option>
                    {move || {
                        // Same mount-time `selected` encoding as the strain
                        // select above: a spaces list that arrives late or is
                        // refetched after a save must not visibly reset the
                        // draft's binding to the placeholder.
                        let picked = draft.get_untracked().grow_space_id;
                        spaces
                            .get()
                            .into_iter()
                            .map(|detail| {
                                let is_selected =
                                    picked.as_deref() == Some(detail.space.id.as_str());
                                view! {
                                    <option value=detail.space.id selected=is_selected>
                                        {detail.space.name}
                                    </option>
                                }
                            })
                            .collect::<Vec<_>>()
                    }}
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
            {leaflog_core::actions::APPEARANCE_TAGS
                .iter()
                .map(|tag| {
                    view! {
                        <label class="flex items-center gap-2 cursor-pointer">
                            <input type="checkbox" class="checkbox checkbox-sm"
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
                            <span class="text-sm">{*tag}</span>
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
async fn fetch_list<T: DeserializeOwned>(path: &str) -> Result<T, ApiFailure> {
    retry_once_after_timeout(
        || api::get::<T>(path),
        || TimeoutFuture::new(REQUEST_TIMEOUT_MS),
        || ApiFailure {
            status: 0,
            message: format!("The request to {path} kept timing out."),
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
fn empty_grow() -> GrowInput {
    GrowInput {
        name: String::new(),
        strain_id: None,
        grow_space_id: None,
        seed_batch_id: None,
        feed_template_id: None,
        substrate: "soil".to_owned(),
        started_on: String::new(),
        status: GrowStatus::default(),
        notes: None,
    }
}

/// A blank action draft, timestamped now (UTC) in the shape a
/// `datetime-local` input accepts.
fn empty_action() -> ActionInput {
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
