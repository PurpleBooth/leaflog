//! Equipment inventory page (spec §4.1): list with category filter,
//! create/edit form, delete.

use leaflog_core::equipment::{EquipmentCategory, EquipmentInput, EquipmentItem};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;

use crate::api;

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

    let start_edit = move |item: &EquipmentItem| {
        draft.set(EquipmentInput::from_item(item));
        editing_id.set(Some(item.id.clone()));
        form_open.set(true);
    };

    let save = move |_ev: leptos::ev::SubmitEvent| {
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
        <section class="page">
            <div class="page-head">
                <h2>"Equipment"</h2>
                <button class="primary" on:click=start_create>"Add equipment"</button>
            </div>

            <ErrorIf error />

            <nav class="chips" aria-label="Filter by category">
                <button
                    class:chip-active=move || filter.get().is_none()
                    on:click=move |_| filter.set(None)
                >
                    "All"
                </button>
                {EquipmentCategory::all()
                    .into_iter()
                    .map(|category| {
                        let current = filter;
                        view! {
                            <button
                                class:chip-active=move || current.get() == Some(category)
                                on:click=move |_| current.set(Some(category))
                            >
                                {category.display()}
                            </button>
                        }
                    })
                    .collect::<Vec<_>>()}
            </nav>

            <Show when=move || form_open.get()>
                <form class="card form" on:submit:prevent=save>
                    <h3>{move || if editing_id.get().is_some() { "Edit equipment" } else { "New equipment" }}</h3>
                    <EquipmentForm draft />
                    <div class="form-actions">
                        <button class="primary" type="submit">"Save"</button>
                        <button type="button" on:click=move |_| form_open.set(false)>"Cancel"</button>
                    </div>
                </form>
            </Show>

            <ul class="cards">
                {move || {
                    items
                        .get()
                        .into_iter()
                        .map(|item| {
                            let for_edit = item.clone();
                            let for_delete = item.clone();
                            view! {
                                <li class="card">
                                    <div class="card-title">
                                        <strong>{item.name.clone()}</strong>
                                        <span class="badge">{item.category.display()}</span>
                                    </div>
                                    <dl class="facts">
                        {item.brand_model.clone().map(|brand| view! { <div><dt>"Brand / model"</dt><dd>{brand}</dd></div> })}
                        {item.real_watts.map(|watts| view! { <div><dt>"Wall watts"</dt><dd>{format!("{watts:.0} W")}</dd></div> })}
                        {item.dimmable.then(|| view! { <div><dt>"Dimmable"</dt><dd>"yes"</dd></div> })}
                        {item.purchase_price.zip(item.currency.clone()).map(|(price, currency)| {
                            view! { <div><dt>"Price"</dt><dd>{format!("{price:.2} {currency}")}</dd></div> }
                        })}
                                    </dl>
                        {item.notes.map(|notes| view! { <p class="notes">{notes}</p> })}
                                    <div class="card-actions">
                                        <button on:click=move |_| start_edit(&for_edit)>"Edit"</button>
                                        <button class="danger" on:click=move |_| remove(&for_delete)>"Delete"</button>
                                    </div>
                                </li>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </ul>

            <Show when=move || items.get().is_empty() && error.get().is_none()>
                <p class="empty">"Nothing here yet — add your light, tent, fan, meters…"</p>
            </Show>

            <p><A href="/">"Back to home"</A></p>
        </section>
    }
}

/// The create/edit form fields for one equipment item.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
fn EquipmentForm(draft: RwSignal<EquipmentInput>) -> impl IntoView {
    view! {
        <div class="grid">
            <label class="field">
                <span>"Name *"</span>
                <input type="text" required
                    prop:value=move || draft.get().name
                    on:input=move |ev| draft.update(|d| d.name = event_target_value(&ev))
                />
            </label>
            <label class="field">
                <span>"Category *"</span>
                <select
                    prop:value=move || draft.get().category.label().to_owned()
                    on:change=move |ev| {
                        let label = event_target_value(&ev);
                        if let Some(category) =
                            EquipmentCategory::all().into_iter().find(|c| c.label() == label)
                        {
                            draft.update(|d| d.category = category);
                        }
                    }
                >
                    {EquipmentCategory::all()
                        .into_iter()
                        .map(|category| view! { <option value=category.label()>{category.display()}</option> })
                        .collect::<Vec<_>>()}
                </select>
            </label>
            <label class="field">
                <span>"Brand / model"</span>
                <input type="text"
                    prop:value=move || draft.get().brand_model.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.brand_model = non_empty(event_target_value(&ev)))
                />
            </label>
            <label class="field">
                <span>"Real wall watts"</span>
                <input type="number" min="0" step="1"
                    prop:value=move || draft.get().real_watts.map(|w| w.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.real_watts = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="field">
                <span>"Purchased (YYYY-MM-DD)"</span>
                <input type="date"
                    prop:value=move || draft.get().purchase_date.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.purchase_date = non_empty(event_target_value(&ev)))
                />
            </label>
            <label class="field">
                <span>"Price"</span>
                <input type="number" min="0" step="0.01"
                    prop:value=move || draft.get().purchase_price.map(|p| p.to_string()).unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.purchase_price = event_target_value(&ev).parse().ok())
                />
            </label>
            <label class="field">
                <span>"Currency"</span>
                <input type="text" placeholder="EUR"
                    prop:value=move || draft.get().currency.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.currency = non_empty(event_target_value(&ev).to_uppercase()))
                />
            </label>
            <label class="field checkbox">
                <input type="checkbox"
                    prop:checked=move || draft.get().dimmable
                    on:change=move |ev| draft.update(|d| d.dimmable = event_target_checked(&ev))
                />
                <span>"Dimmable"</span>
            </label>
            <label class="field wide">
                <span>"Notes"</span>
                <textarea rows="2"
                    prop:value=move || draft.get().notes.unwrap_or_default()
                    on:input=move |ev| draft.update(|d| d.notes = non_empty(event_target_value(&ev)))
                />
            </label>
        </div>
    }
}

/// Shows the error banner when set.
#[component]
fn ErrorIf(error: RwSignal<Option<String>>) -> impl IntoView {
    move || {
        error.get().map(|message| {
            view! { <p class="error" role="alert">{message}</p> }
        })
    }
}

/// Empty strings become `None` (API nulls).
fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
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
    }
}
