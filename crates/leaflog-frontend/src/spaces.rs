//! Grow spaces UI (spec §4.1): a named physical setup combining
//! equipment catalogue entries, with grows running inside it.

use leaflog_core::equipment::EquipmentItem;
use leaflog_core::grows::Grow;
use leaflog_core::spaces::{GrowSpace, GrowSpaceInput};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use serde::Deserialize;

use crate::api;
use crate::equipment::ErrorIf;
use crate::route_href;

/// One `/api/spaces` response entry: a space with its equipment and
/// grows resolved (mirrors the backend `GrowSpaceDetail` DTO).
#[derive(Debug, Clone, Deserialize)]
pub struct SpaceDetail {
    /// The space itself.
    pub space: GrowSpace,
    /// Catalogue entries referenced by this space.
    pub equipment: Vec<EquipmentItem>,
    /// Grows inside this space.
    pub grows: Vec<Grow>,
}

/// The grow spaces overview page.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn SpacesPage() -> impl IntoView {
    let spaces = RwSignal::<Vec<SpaceDetail>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);
    let form_open = RwSignal::<bool>::new(false);
    let draft = RwSignal::<GrowSpaceInput>::new(empty_space());

    let refresh = move || {
        spawn_local(async move {
            match api::get::<Vec<SpaceDetail>>("api/spaces").await {
                Ok(list) => spaces.set(list),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };
    Effect::new(move |_| {
        refresh();
    });

    // prevent_default first — see docs/DECISIONS.md D061.
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        spawn_local(async move {
            match api::post::<GrowSpaceInput, GrowSpace>("api/spaces", &draft.get()).await {
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
                <h2>"Grow spaces"</h2>
                <button class="btn btn-primary btn-sm" on:click=move |_| {
                    draft.set(empty_space());
                    form_open.set(true);
                }>"New space"</button>
            </div>

            <ErrorIf error />

            <Show when=move || form_open.get()>
                <form class="card bg-base-100 shadow-sm px-5 py-4 my-4" on:submit=save>
                    <h3>"New space"</h3>
                    <SpaceForm draft />
                    <div class="flex flex-wrap gap-2 mt-4">
                        <button class="btn btn-primary btn-sm" type="submit">"Save"</button>
                        <button class="btn btn-sm" type="button" on:click=move |_| form_open.set(false)>"Cancel"</button>
                    </div>
                </form>
            </Show>

            <ul class="grid gap-4 lg:grid-cols-2">
                {move || {
                    spaces
                        .get()
                        .into_iter()
                        .map(|detail| {
                            // Owned values only: view children are lazy
                            // and must not borrow from `detail`.
                            let href = route_href(&format!("/spaces/{}", detail.space.id));
                            let name = detail.space.name.clone();
                            let dims = dims_label(&detail.space);
                            let gear = detail.equipment.len();
                            let grow_count = detail.grows.len();
                            view! {
                                <li class="card bg-base-100 shadow-sm px-5 py-4">
                                    <div class="flex flex-wrap items-baseline gap-2">
                                        <A href=href>
                                            <strong>{name}</strong>
                                        </A>
                                    </div>
                                    {dims
                                        .map(|dims| view! { <p class="text-base-content/70 text-sm">{dims}</p> })}
                                    <dl class="facts">
                                        <div><dt>"Gear"</dt><dd>{gear}</dd></div>
                                        <div><dt>"Grows"</dt><dd>{grow_count}</dd></div>
                                    </dl>
                                </li>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </ul>

            <Show when=move || spaces.get().is_empty() && error.get().is_none()>
                <p class="py-6 text-center text-base-content/60">"No spaces yet — set up your tent or room, then fill it with gear."</p>
            </Show>
            <p><A href=route_href("/")>"Back to home"</A></p>
        </section>
    }
}

/// One grow space: editable fields, its equipment (add from the
/// catalogue, remove), and its grows.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn SpaceDetail() -> impl IntoView {
    let params = use_params_map();
    let space_id = move || params.get().get("id").unwrap_or_default();

    let detail = RwSignal::<Option<SpaceDetail>>::new(None);
    let catalog = RwSignal::<Vec<EquipmentItem>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);
    let form_open = RwSignal::<bool>::new(false);
    let draft = RwSignal::<GrowSpaceInput>::new(empty_space());
    let pick = RwSignal::<String>::new(String::new());

    let refresh = move || {
        let id = space_id();
        spawn_local(async move {
            match api::get::<SpaceDetail>(&format!("api/spaces/{id}")).await {
                Ok(value) => detail.set(Some(value)),
                Err(failure) => error.set(Some(failure.message)),
            }
            match api::get::<Vec<EquipmentItem>>("api/equipment").await {
                Ok(value) => catalog.set(value),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };
    Effect::new(move |_| {
        params.track();
        refresh();
    });

    // prevent_default first — see docs/DECISIONS.md D061.
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let id = space_id();
        spawn_local(async move {
            match api::put::<GrowSpaceInput, GrowSpace>(&format!("api/spaces/{id}"), &draft.get())
                .await
            {
                Ok(_) => {
                    form_open.set(false);
                    error.set(None);
                    refresh();
                }
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    let add_equipment = move |_ev: leptos::ev::MouseEvent| {
        let equipment_id = pick.get_untracked();
        if equipment_id.is_empty() {
            return;
        }
        let id = space_id();
        spawn_local(async move {
            match api::post_action(&format!("api/spaces/{id}/equipment/{equipment_id}")).await {
                Ok(()) => {
                    pick.set(String::new());
                    error.set(None);
                    refresh();
                }
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    let remove_equipment = move |equipment_id: &str| {
        let equipment_id = equipment_id.to_owned();
        let id = space_id();
        spawn_local(async move {
            match api::delete(&format!("api/spaces/{id}/equipment/{equipment_id}")).await {
                Ok(()) => refresh(),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    let remove_space = move || {
        let id = space_id();
        spawn_local(async move {
            match api::delete(&format!("api/spaces/{id}")).await {
                Ok(()) => detail.set(None),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    view! {
        <section class="pb-12">
            {move || {
                detail.get().map(|detail| {
                    // Destructure into owned parts: view children are
                    // lazy and must not borrow from `detail`.
                    let SpaceDetail {
                        space,
                        equipment,
                        grows,
                    } = detail;
                    let for_edit = space.clone();
                    let present: Vec<String> = equipment
                        .iter()
                        .map(|item| item.id.clone())
                        .collect();
                    view! {
                        <div>
                            <div class="flex flex-wrap items-center justify-between gap-4">
                                <h2>{space.name.clone()}</h2>
                                {dims_label(&space)
                                    .map(|dims| view! { <span class="badge badge-ghost">{dims}</span> })}
                            </div>

                            <Show when=move || form_open.get()>
                                <form class="card bg-base-100 shadow-sm px-5 py-4 my-4" on:submit=save>
                                    <h3>"Edit space"</h3>
                                    <SpaceForm draft />
                                    <div class="flex flex-wrap gap-2 mt-4">
                                        <button class="btn btn-primary btn-sm" type="submit">"Save"</button>
                                        <button class="btn btn-sm" type="button" on:click=move |_| form_open.set(false)>"Cancel"</button>
                                    </div>
                                </form>
                            </Show>

                            <div class="grid gap-6 items-start lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
                                <div>
                                    <h3>"Equipment"</h3>
                                    <ul class="grid gap-4 lg:grid-cols-2">
                                        {equipment
                                            .into_iter()
                                            .map(|item| {
                                                let for_remove = item.id.clone();
                                                view! {
                                                    <li class="card bg-base-100 shadow-sm px-5 py-4">
                                                        <div class="flex flex-wrap items-baseline gap-2">
                                                            <strong>{item.name.clone()}</strong>
                                                            <span class="badge badge-ghost">{item.category.display()}</span>
                                                        </div>
                                                        {item.real_watts
                                                            .map(|watts| view! {
                                                                <p class="text-base-content/70 text-sm">
                                                                    {format!("{watts:.0} W")}
                                                                </p>
                                                            })}
                                                        <div class="flex flex-wrap gap-2 mt-3">
                                                            <button class="btn btn-error btn-outline btn-sm"
                                                                on:click=move |_| remove_equipment(&for_remove)
                                                            >
                                                                "Remove"
                                                            </button>
                                                        </div>
                                                    </li>
                                                }
                                            })
                                            .collect::<Vec<_>>()}
                                    </ul>
                                    <div class="flex flex-wrap gap-2 mt-4">
                                        <select class="select"
                                            prop:value=move || pick.get()
                                            on:change=move |ev| pick.set(event_target_value(&ev))
                                        >
                                            <option value="">"— add gear —"</option>
                                            {move || {
                                                catalog
                                                    .get()
                                                    .into_iter()
                                                    .filter(|item| !present.contains(&item.id))
                                                    .map(|item| {
                                                        view! {
                                                            <option value=item.id>
                                                                {format!(
                                                                    "{} — {}",
                                                                    item.name,
                                                                    item.category.display(),
                                                                )}
                                                            </option>
                                                        }
                                                    })
                                                    .collect::<Vec<_>>()
                                            }}
                                        </select>
                                        <button class="btn btn-sm" on:click=add_equipment disabled=move || pick.get().is_empty()>
                                            "Add"
                                        </button>
                                    </div>
                                </div>

                                <div>
                                    <h3>"Grows in this space"</h3>
                                    <ul class="grid gap-4 lg:grid-cols-2">
                                        {grows
                                            .into_iter()
                                            .map(|grow| {
                                                view! {
                                                    <li class="card bg-base-100 shadow-sm px-5 py-4">
                                                        <div class="flex flex-wrap items-baseline gap-2">
                                                            <A href=route_href(&format!("/grows/{}", grow.id))>
                                                                <strong>{grow.name.clone()}</strong>
                                                            </A>
                                                            <span class="badge badge-ghost">{grow.status.label()}</span>
                                                        </div>
                                                        <p class="text-base-content/70 text-sm">"Started "{grow.started_on}</p>
                                                    </li>
                                                }
                                            })
                                            .collect::<Vec<_>>()}
                                    </ul>
                                    <p>
                                        <A href=route_href("/grows")>"Start a grow here → pick this space on the form"</A>
                                    </p>
                                </div>
                            </div>

                            {space.notes
                                .map(|notes| view! { <p class="mt-2 text-base-content/70">{notes}</p> })}

                            <div class="flex flex-wrap gap-2 mt-3">
                                <button class="btn btn-sm" on:click=move |_| {
                                    draft.set(space_input(&for_edit));
                                    form_open.set(true);
                                }>"Edit space"</button>
                                <button class="btn btn-error btn-outline btn-sm" on:click=move |_| remove_space()>
                                    "Delete space"
                                </button>
                            </div>
                        </div>
                    }
                })
            }}

            <Show when=move || detail.get().is_none() && error.get().is_none()>
                <p class="py-6 text-center text-base-content/60">"This space is gone. "<A href=route_href("/spaces")>"Back to spaces"</A></p>
            </Show>

            <ErrorIf error />
            <p><A href=route_href("/spaces")>"Back to spaces"</A></p>
        </section>
    }
}

/// The create/edit form fields for one grow space.
#[component]
fn SpaceForm(draft: RwSignal<GrowSpaceInput>) -> impl IntoView {
    view! {
        <div class="grid gap-3 grid-cols-[repeat(auto-fill,minmax(14rem,1fr))]">
            <label class="fieldset">
                <span class="fieldset-label">"Name *"</span>
                <input class="input" type="text" required placeholder="60×60 tent"
                    prop:value=move || draft.get().name
                    on:input=move |ev| draft.update(|d| d.name = event_target_value(&ev))
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Width (cm)"</span>
                <input class="input" type="number" min="1" step="1"
                    prop:value=move || draft.get().width_cm.map(|v| v.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.width_cm = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Depth (cm)"</span>
                <input class="input" type="number" min="1" step="1"
                    prop:value=move || draft.get().depth_cm.map(|v| v.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.depth_cm = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Height (cm)"</span>
                <input class="input" type="number" min="1" step="1"
                    prop:value=move || draft.get().height_cm.map(|v| v.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.height_cm = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="fieldset col-span-full">
                <span class="fieldset-label">"Notes"</span>
                <textarea class="textarea" rows="2"
                    prop:value=move || draft.get().notes.unwrap_or_default()
                    on:input=move |ev| {
                        let value = event_target_value(&ev);
                        draft.update(|d| {
                            d.notes = (!value.trim().is_empty()).then_some(value);
                        });
                    }
                />
            </label>
        </div>
    }
}

/// Formats a space's dimensions, labelling whichever parts are known
/// (for example `W 60 × D 60 × H 180 cm`).
fn dims_label(space: &GrowSpace) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(width) = space.width_cm {
        parts.push(format!("W {width:.0}"));
    }
    if let Some(depth) = space.depth_cm {
        parts.push(format!("D {depth:.0}"));
    }
    if let Some(height) = space.height_cm {
        parts.push(format!("H {height:.0}"));
    }
    (!parts.is_empty()).then(|| format!("{} cm", parts.join(" × ")))
}

/// The create/replace payload of a stored space.
fn space_input(space: &GrowSpace) -> GrowSpaceInput {
    GrowSpaceInput {
        name: space.name.clone(),
        width_cm: space.width_cm,
        depth_cm: space.depth_cm,
        height_cm: space.height_cm,
        notes: space.notes.clone(),
    }
}

/// A blank space draft.
const fn empty_space() -> GrowSpaceInput {
    GrowSpaceInput {
        name: String::new(),
        width_cm: None,
        depth_cm: None,
        height_cm: None,
        notes: None,
    }
}
