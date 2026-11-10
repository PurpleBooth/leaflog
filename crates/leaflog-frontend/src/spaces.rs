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

    let save = move |_ev: leptos::ev::SubmitEvent| {
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
        <section class="page">
            <div class="page-head">
                <h2>"Grow spaces"</h2>
                <button class="primary" on:click=move |_| {
                    draft.set(empty_space());
                    form_open.set(true);
                }>"New space"</button>
            </div>

            <ErrorIf error />

            <Show when=move || form_open.get()>
                <form class="card form" on:submit:prevent=save>
                    <h3>"New space"</h3>
                    <SpaceForm draft />
                    <div class="form-actions">
                        <button class="primary" type="submit">"Save"</button>
                        <button type="button" on:click=move |_| form_open.set(false)>"Cancel"</button>
                    </div>
                </form>
            </Show>

            <ul class="cards">
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
                                <li class="card">
                                    <div class="card-title">
                                        <A href=href>
                                            <strong>{name}</strong>
                                        </A>
                                    </div>
                                    {dims
                                        .map(|dims| view! { <p class="muted small">{dims}</p> })}
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
                <p class="empty">"No spaces yet — set up your tent or room, then fill it with gear."</p>
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

    let save = move |_ev: leptos::ev::SubmitEvent| {
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
        <section class="page">
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
                            <div class="page-head">
                                <h2>{space.name.clone()}</h2>
                                {dims_label(&space)
                                    .map(|dims| view! { <span class="badge">{dims}</span> })}
                            </div>

                            <Show when=move || form_open.get()>
                                <form class="card form" on:submit:prevent=save>
                                    <h3>"Edit space"</h3>
                                    <SpaceForm draft />
                                    <div class="form-actions">
                                        <button class="primary" type="submit">"Save"</button>
                                        <button type="button" on:click=move |_| form_open.set(false)>"Cancel"</button>
                                    </div>
                                </form>
                            </Show>

                            <div class="split">
                                <div>
                                    <h3>"Equipment"</h3>
                                    <ul class="cards">
                                        {equipment
                                            .into_iter()
                                            .map(|item| {
                                                let for_remove = item.id.clone();
                                                view! {
                                                    <li class="card">
                                                        <div class="card-title">
                                                            <strong>{item.name.clone()}</strong>
                                                            <span class="badge">{item.category.display()}</span>
                                                        </div>
                                                        {item.real_watts
                                                            .map(|watts| view! {
                                                                <p class="muted small">
                                                                    {format!("{watts:.0} W")}
                                                                </p>
                                                            })}
                                                        <div class="card-actions">
                                                            <button class="danger"
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
                                    <div class="form-actions">
                                        <select
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
                                        <button on:click=add_equipment disabled=move || pick.get().is_empty()>
                                            "Add"
                                        </button>
                                    </div>
                                </div>

                                <div>
                                    <h3>"Grows in this space"</h3>
                                    <ul class="cards">
                                        {grows
                                            .into_iter()
                                            .map(|grow| {
                                                view! {
                                                    <li class="card">
                                                        <div class="card-title">
                                                            <A href=route_href(&format!("/grows/{}", grow.id))>
                                                                <strong>{grow.name.clone()}</strong>
                                                            </A>
                                                            <span class="badge">{grow.status.label()}</span>
                                                        </div>
                                                        <p class="muted small">"Started "{grow.started_on}</p>
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
                                .map(|notes| view! { <p class="notes">{notes}</p> })}

                            <div class="card-actions">
                                <button on:click=move |_| {
                                    draft.set(space_input(&for_edit));
                                    form_open.set(true);
                                }>"Edit space"</button>
                                <button class="danger" on:click=move |_| remove_space()>
                                    "Delete space"
                                </button>
                            </div>
                        </div>
                    }
                })
            }}

            <Show when=move || detail.get().is_none() && error.get().is_none()>
                <p class="empty">"This space is gone. "<A href=route_href("/spaces")>"Back to spaces"</A></p>
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
        <div class="grid">
            <label class="field">
                <span>"Name *"</span>
                <input type="text" required placeholder="60×60 tent"
                    prop:value=move || draft.get().name
                    on:input=move |ev| draft.update(|d| d.name = event_target_value(&ev))
                />
            </label>
            <label class="field">
                <span>"Width (cm)"</span>
                <input type="number" min="1" step="1"
                    prop:value=move || draft.get().width_cm.map(|v| v.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.width_cm = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="field">
                <span>"Depth (cm)"</span>
                <input type="number" min="1" step="1"
                    prop:value=move || draft.get().depth_cm.map(|v| v.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.depth_cm = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="field">
                <span>"Height (cm)"</span>
                <input type="number" min="1" step="1"
                    prop:value=move || draft.get().height_cm.map(|v| v.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.height_cm = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="field wide">
                <span>"Notes"</span>
                <textarea rows="2"
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
