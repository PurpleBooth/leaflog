//! Equipment catalogue page (spec §4.1): bundled defaults plus the
//! user's own entries, with a category filter, create/edit form and
//! delete. Grow spaces reference entries from here.

use leaflog_core::equipment::{EquipmentCategory, EquipmentInput, EquipmentItem};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;

use crate::api;
use crate::non_empty;
use crate::route_href;

/// The equipment inventory page.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn EquipmentPage() -> impl IntoView {
    let items = RwSignal::<Vec<EquipmentItem>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);
    let filter = RwSignal::<Option<EquipmentCategory>>::new(None);
    let form_open = RwSignal::<bool>::new(false);
    let editing_id = RwSignal::<Option<String>>::new(None);
    let draft = RwSignal::<EquipmentInput>::new(empty_equipment());
    // The HA sensor picker (spec §4.1): one candidate per entity, plus
    // whether HA is reachable so the flow can defer instead of block.
    let candidates = RwSignal::<Vec<(String, String, String)>>::new(Vec::new());
    let picker_source = RwSignal::<Option<String>>::new(None);

    let refresh = move || {
        spawn_local(async move {
            let query = filter
                .get()
                .map(|category| format!("?category={}", category.label()))
                .unwrap_or_default();
            match api::get::<Vec<EquipmentItem>>(&format!("api/equipment{query}")).await {
                Ok(list) => items.set(list),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    Effect::new(move |_| {
        filter.track();
        refresh();
    });

    let start_create = move |_| {
        draft.set(empty_equipment());
        editing_id.set(None);
        form_open.set(true);
    };

    // Sensor items come only from the HA entity picker: load the
    // temperature/humidity entities and offer them; picking one
    // pre-fills the form. With HA unreachable the list is empty and the
    // flow defers (skip now, pick later) — nothing else blocks.
    let open_picker = move |_| {
        spawn_local(async move {
            match api::get::<serde_json::Value>("api/ha/entities").await {
                Ok(payload) => {
                    picker_source.set(payload["source"].as_str().map(str::to_owned));
                    let offered: Vec<(String, String, String)> = payload["entities"]
                        .as_array()
                        .map(|entities| {
                            entities
                                .iter()
                                .filter_map(|entity| {
                                    let id = entity["entity_id"].as_str()?;
                                    let attributes = &entity["attributes"];
                                    let class = attributes["device_class"].as_str()?.to_owned();
                                    if class != "temperature" && class != "humidity" {
                                        return None;
                                    }
                                    let name = attributes["friendly_name"]
                                        .as_str()
                                        .unwrap_or(id)
                                        .to_owned();
                                    Some((id.to_owned(), name, class))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    candidates.set(offered);
                }
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    let pick_entity = move |(entity_id, name, class): &(String, String, String)| {
        let mut input = empty_equipment();
        input.name.clone_from(name);
        input.category = EquipmentCategory::Sensor;
        input.ha_entity_id = Some(entity_id.clone());
        input.ha_device_class = Some(class.clone());
        draft.set(input);
        editing_id.set(None);
        form_open.set(true);
    };

    let start_edit = move |item: &EquipmentItem| {
        draft.set(EquipmentInput::from_item(item));
        editing_id.set(Some(item.id.clone()));
        form_open.set(true);
    };

    // prevent_default first — see docs/DECISIONS.md D061.
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        spawn_local(async move {
            let input = draft.get();
            let result = match editing_id.get() {
                Some(id) => api::put::<EquipmentInput, EquipmentItem>(
                    &format!("api/equipment/{id}"),
                    &input,
                )
                .await
                .map(|_| ()),
                None => api::post::<EquipmentInput, EquipmentItem>("api/equipment", &input)
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

    let clone_item = move |item: &EquipmentItem| {
        let id = item.id.clone();
        spawn_local(async move {
            match api::post_empty::<EquipmentItem>(&format!("api/equipment/{id}/clone")).await {
                Ok(_) => refresh(),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    let remove = move |item: &EquipmentItem| {
        let id = item.id.clone();
        spawn_local(async move {
            match api::delete(&format!("api/equipment/{id}")).await {
                Ok(()) => refresh(),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    view! {
        <section class="pb-12">
            <div class="flex flex-wrap items-center justify-between gap-4">
                <h2>"Equipment"</h2>
                <div class="flex gap-2">
                    <button class="btn btn-sm" on:click=open_picker>"Add sensor from HA"</button>
                    <button class="btn btn-primary btn-sm" on:click=start_create>"Add equipment"</button>
                </div>
            </div>

            <ErrorIf error />

            <nav class="flex flex-wrap gap-2 my-2" aria-label="Filter by category">
                <button
                    class=move || if filter.get().is_none() {
                        "btn btn-sm rounded-full btn-primary"
                    } else {
                        "btn btn-sm rounded-full"
                    }
                    on:click=move |_| filter.set(None)
                >
                    "All"
                </button>
                {EquipmentCategory::catalogue()
                    .into_iter()
                    .map(|category| {
                        let current = filter;
                        view! {
                            <button
                                class=move || if current.get() == Some(category) {
                                    "btn btn-sm rounded-full btn-primary"
                                } else {
                                    "btn btn-sm rounded-full"
                                }
                                on:click=move |_| current.set(Some(category))
                            >
                                {category.display()}
                            </button>
                        }
                    })
                    .collect::<Vec<_>>()}
            </nav>

            <Show when=move || !candidates.get().is_empty() || picker_source.get().is_some()>
                <div class="card bg-base-100 shadow-sm px-5 py-4 my-4">
                    <h3>"Sensors from Home Assistant"</h3>
                    <ul class="flex flex-wrap gap-2 my-2">
                        {move || {
                            candidates
                                .get()
                                .iter()
                                .map(|candidate| {
                                    let for_pick = candidate.clone();
                                    view! {
                                        <li>
                                            <button class="btn btn-sm" on:click=move |_| pick_entity(&for_pick)>
                                                {format!(
                                                    "{} ({})",
                                                    for_pick.1,
                                                    for_pick.0,
                                                )}
                                            </button>
                                        </li>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </ul>
                    <Show when=move || candidates.get().is_empty()>
                        <p class="py-6 text-center text-base-content/60">
                            "No temperature or humidity sensors visible — Home Assistant may be
                            unreachable. Skip for now and pick the sensor later; nothing else
                            is blocked."
                        </p>
                    </Show>
                </div>
            </Show>

            <Show when=move || form_open.get()>
                <form class="card bg-base-100 shadow-sm px-5 py-4 my-4" on:submit=save>
                    <h3>{move || if editing_id.get().is_some() { "Edit equipment" } else { "New equipment" }}</h3>
                    <EquipmentForm draft />
                    <div class="flex flex-wrap gap-2 mt-4">
                        <button class="btn btn-primary btn-sm" type="submit">"Save"</button>
                        <button class="btn btn-sm" type="button" on:click=move |_| form_open.set(false)>"Cancel"</button>
                    </div>
                </form>
            </Show>

            <ul class="grid gap-4 lg:grid-cols-2">
                {move || {
                    items
                        .get()
                        .into_iter()
                        .map(|item| {
                            let for_edit = item.clone();
                            let for_delete = item.clone();
                            let for_clone = item.clone();
                            let bundled = item.is_builtin;
                            view! {
                                <li class="card bg-base-100 shadow-sm px-5 py-4">
                                    <div class="flex flex-wrap items-baseline gap-2">
                                        <strong>{item.name.clone()}</strong>
                                        <span class="badge badge-ghost">{item.category.display()}</span>
                                        {item.is_builtin
                                            .then(|| view! { <span class="badge badge-outline">"bundled"</span> })}
                                    </div>
                                    <dl class="facts">
                        {item.brand_model.clone().map(|brand| view! { <div><dt>"Brand / model"</dt><dd>{brand}</dd></div> })}
                        {item.real_watts.map(|watts| view! { <div><dt>"Wall watts"</dt><dd>{format!("{watts:.0} W")}</dd></div> })}
                        {item.dimmable.then(|| view! { <div><dt>"Dimmable"</dt><dd>"yes"</dd></div> })}
                        {item.ha_entity_id.clone().map(|entity| view! {
                            <div><dt>"HA entity"</dt><dd>{entity}</dd></div>
                        })}
                        {item.purchase_price.zip(item.currency.clone()).map(|(price, currency)| {
                            view! { <div><dt>"Price"</dt><dd>{format!("{price:.2} {currency}")}</dd></div> }
                        })}
                                    </dl>
                        {item.notes.map(|notes| view! { <p class="mt-2 text-base-content/70">{notes}</p> })}
                                    <div class="flex flex-wrap gap-2 mt-3">
                                        // Bundled presets are read-only (the API rejects
                                        // edit and delete); clone-to-edit is the workflow,
                                        // and cloning a user entry covers "two identical
                                        // lights are two entries" (§4.1).
                                        {(!bundled)
                                            .then(|| view! { <button class="btn btn-sm" on:click=move |_| start_edit(&for_edit)>"Edit"</button> })}
                                        <button class="btn btn-sm" on:click=move |_| clone_item(&for_clone)>
                                            {move || if bundled { "Clone to edit" } else { "Clone" }}
                                        </button>
                                        {(!bundled)
                                            .then(|| view! { <button class="btn btn-error btn-outline btn-sm" on:click=move |_| remove(&for_delete)>"Delete"</button> })}
                                    </div>
                                </li>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </ul>

            <Show when=move || items.get().is_empty() && error.get().is_none()>
                <p class="py-6 text-center text-base-content/60">"Nothing here yet — add your light, tent, fan, meters…"</p>
            </Show>

            <p><A href=route_href("/")>"Back to home"</A></p>
        </section>
    }
}

/// The create/edit form fields for one equipment item.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
fn EquipmentForm(draft: RwSignal<EquipmentInput>) -> impl IntoView {
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
                <span class="fieldset-label">"Category *"</span>
                <select class="select"
                    prop:value=move || draft.get().category.label().to_owned()
                    on:change=move |ev| {
                        let label = event_target_value(&ev);
                        if let Some(category) =
                            EquipmentCategory::catalogue().into_iter().find(|c| c.label() == label)
                        {
                            draft.update(|d| d.category = category);
                        }
                    }
                >
                    {EquipmentCategory::catalogue()
                        .into_iter()
                        .map(|category| view! { <option value=category.label()>{category.display()}</option> })
                        .collect::<Vec<_>>()}
                </select>
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Brand / model"</span>
                <input class="input" type="text"
                    prop:value=move || draft.get().brand_model.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.brand_model = non_empty(event_target_value(&ev)))
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Real wall watts"</span>
                <input class="input" type="number" min="0" step="1"
                    prop:value=move || draft.get().real_watts.map(|w| w.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.real_watts = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Purchased (YYYY-MM-DD)"</span>
                <input class="input" type="date"
                    prop:value=move || draft.get().purchase_date.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.purchase_date = non_empty(event_target_value(&ev)))
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Price"</span>
                <input class="input" type="number" min="0" step="0.01"
                    prop:value=move || draft.get().purchase_price.map(|p| p.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.purchase_price = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Currency"</span>
                <input class="input" type="text" placeholder="EUR"
                    prop:value=move || draft.get().currency.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.currency = non_empty(event_target_value(&ev).to_uppercase()))
                />
            </label>
            <label class="flex items-center gap-2 cursor-pointer">
                <input type="checkbox" class="checkbox checkbox-sm"
                    prop:checked=move || draft.get().dimmable
                    on:change=move |ev| draft.update(|d| d.dimmable = event_target_checked(&ev))
                />
                <span class="fieldset-label">"Dimmable"</span>
            </label>
            <label class="fieldset">
                <span class="fieldset-label">{move || if draft.get().category == EquipmentCategory::Sensor {
                    "HA entity *"
                } else {
                    "HA entity binding (optional)"
                }}</span>
                <input class="input" type="text" placeholder="sensor.grow_temperature"
                    prop:value=move || draft.get().ha_entity_id.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.ha_entity_id = non_empty(event_target_value(&ev)))
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Device class"</span>
                <select class="select"
                    prop:value=move || {
                        draft.get().ha_device_class.unwrap_or_default()
                    }
                    on:change=move |ev| {
                        draft.update(|d| d.ha_device_class = non_empty(event_target_value(&ev)));
                    }
                >
                    <option value="">"—"</option>
                    <option value="temperature">"temperature"</option>
                    <option value="humidity">"humidity"</option>
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

/// Shows the error banner when set.
#[component]
pub(crate) fn ErrorIf(error: RwSignal<Option<String>>) -> impl IntoView {
    move || {
        error.get().map(|message| {
            view! { <p class="alert alert-error py-2 px-4 text-sm" role="alert">{message}</p> }
        })
    }
}

/// A blank equipment draft (a light, the most common first entry).
const fn empty_equipment() -> EquipmentInput {
    EquipmentInput {
        name: String::new(),
        category: EquipmentCategory::Light,
        brand_model: None,
        real_watts: None,
        dimmable: false,
        purchase_date: None,
        purchase_price: None,
        currency: None,
        notes: None,
        ha_entity_id: None,
        ha_device_class: None,
    }
}
