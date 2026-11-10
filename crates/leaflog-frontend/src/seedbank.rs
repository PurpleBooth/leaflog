//! Seedbank page (spec §4.13): owned seed batches, oldest first within
//! each strain (use the oldest seeds first), with the §5.4 viability
//! window and a past-prime badge.

use leaflog_core::seedbank::{SeedBatchInput, SeedBatchView, StorageMethod};
use leaflog_core::strain::Strain;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;

use crate::api;
use crate::equipment::ErrorIf;
use crate::route_href;

/// The seedbank page.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn SeedbankPage() -> impl IntoView {
    let batches = RwSignal::<Vec<SeedBatchView>>::new(Vec::new());
    let strains = RwSignal::<Vec<Strain>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);
    let form_open = RwSignal::<bool>::new(false);
    let editing_id = RwSignal::<Option<String>>::new(None);
    let draft = RwSignal::<SeedBatchInput>::new(empty_batch());

    let refresh = move || {
        spawn_local(async move {
            match api::get::<Vec<SeedBatchView>>("api/seed-batches").await {
                Ok(list) => batches.set(list),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    Effect::new(move |_| {
        refresh();
        spawn_local(async move {
            if let Ok(list) = api::get::<Vec<Strain>>("api/strains").await {
                strains.set(list);
            }
        });
    });

    let start_create = move |_| {
        draft.set(empty_batch());
        editing_id.set(None);
        form_open.set(true);
    };

    let start_edit = move |batch: &SeedBatchView| {
        draft.set(SeedBatchInput::from_batch(&batch.batch));
        editing_id.set(Some(batch.batch.id.clone()));
        form_open.set(true);
    };

    // prevent_default first — see docs/DECISIONS.md D061.
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        spawn_local(async move {
            let input = draft.get();
            let result = match editing_id.get() {
                Some(id) => api::put::<SeedBatchInput, SeedBatchView>(
                    &format!("api/seed-batches/{id}"),
                    &input,
                )
                .await
                .map(|_| ()),
                None => api::post::<SeedBatchInput, SeedBatchView>("api/seed-batches", &input)
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

    let remove = move |batch: &SeedBatchView| {
        let id = batch.batch.id.clone();
        spawn_local(async move {
            match api::delete(&format!("api/seed-batches/{id}")).await {
                Ok(()) => refresh(),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    view! {
        <section class="page">
            <div class="page-head">
                <h2>"Seedbank"</h2>
                <button class="primary" on:click=start_create>"Add batch"</button>
            </div>

            <ErrorIf error />

            <Show when=move || form_open.get()>
                <form class="card form" on:submit=save>
                    <h3>{move || if editing_id.get().is_some() { "Edit batch" } else { "New batch" }}</h3>
                    <BatchForm draft strains />
                    <div class="form-actions">
                        <button class="primary" type="submit">"Save"</button>
                        <button type="button" on:click=move |_| form_open.set(false)>"Cancel"</button>
                    </div>
                </form>
            </Show>

            <ul class="cards">
                {move || {
                    batches
                        .get()
                        .into_iter()
                        .map(|batch| {
                            let for_edit = batch.clone();
                            let for_delete = batch.clone();
                            view! {
                                <li class="card">
                                    <div class="card-title">
                                        <strong>{batch.batch.quantity}" × "</strong>
                                        {move || {
                                            strains
                                                .get_untracked()
                                                .iter()
                                                .find(|strain| Some(&strain.id) == batch.batch.strain_id.as_ref())
                                                .map_or_else(
                                                    || "Unlinked seeds".to_owned(),
                                                    |strain| strain.name.clone(),
                                                )
                                        }}
                                        {batch.viability.past_prime
                                            .then(|| view! { <span class="badge">"past prime"</span> })}
                                    </div>
                                    <dl class="facts">
                                        <div><dt>"Acquired"</dt><dd>{batch.batch.acquisition_on.clone()}</dd></div>
                                        <div><dt>"Storage"</dt><dd>{batch.batch.storage_method.display()}</dd></div>
                                        {batch.batch.acquired_from.clone().map(|from| view! {
                                            <div><dt>"From"</dt><dd>{from}</dd></div>
                                        })}
                                        {batch.observed.map(|rate| view! {
                                            <div>
                                                <dt>"Observed germination"</dt>
                                                <dd>{format!(
                                                    "{}/{} sprouted",
                                                    rate.sprouted, rate.sown,
                                                )}</dd>
                                            </div>
                                        })}
                                        <div>
                                            <dt>"Viability window"</dt>
                                            <dd>
                                                {match batch.viability.window_months.1 {
                                                    Some(max) => format!(
                                                        "{}–{} months (age {})",
                                                        batch.viability.window_months.0, max, batch.viability.age_months,
                                                    ),
                                                    None => format!(
                                                        "{}+ months (age {})",
                                                        batch.viability.window_months.0, batch.viability.age_months,
                                                    ),
                                                }}
                                            </dd>
                                        </div>
                                    </dl>
                                    {batch.batch.notes.map(|notes| view! { <p class="notes">{notes}</p> })}
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

            <Show when=move || batches.get().is_empty() && error.get().is_none()>
                <p class="empty">
                    "No seeds logged yet — add a batch to see its §5.4 viability window. The oldest
                    batch of each strain is listed first: use those seeds first."
                </p>
            </Show>

            <p><A href=route_href("/")>"Back to home"</A></p>
        </section>
    }
}

/// The create/edit form fields for one seed batch.
#[component]
fn BatchForm(draft: RwSignal<SeedBatchInput>, strains: RwSignal<Vec<Strain>>) -> impl IntoView {
    view! {
        <div class="grid">
            <label class="field">
                <span>"Strain"</span>
                <select
                    prop:value=move || {
                        draft.get().strain_id.unwrap_or_default()
                    }
                    on:change=move |ev| {
                        draft.update(|d| d.strain_id = crate::non_empty(event_target_value(&ev)));
                    }
                >
                    <option value="">"— unlinked —"</option>
                    {move || {
                        strains
                            .get()
                            .into_iter()
                            .map(|strain| {
                                let id = strain.id.clone();
                                view! { <option value=id>{strain.name}</option> }
                            })
                            .collect::<Vec<_>>()
                    }}
                </select>
            </label>
            <label class="field">
                <span>"Seeds *"</span>
                <input type="number" min="0" step="1" required
                    prop:value=move || draft.get().quantity.to_string()
                    on:input=move |ev| draft.update(|d| d.quantity = event_target_value(&ev).parse().unwrap_or(0))
                />
            </label>
            <label class="field">
                <span>"Acquired (YYYY-MM-DD) *"</span>
                <input type="date" required
                    prop:value=move || draft.get().acquisition_on
                    on:input=move |ev| draft.update(|d| d.acquisition_on = event_target_value(&ev))
                />
            </label>
            <label class="field">
                <span>"Storage"</span>
                <select
                    prop:value=move || draft.get().storage_method.label().to_owned()
                    on:change=move |ev| {
                        let label = event_target_value(&ev);
                        if let Some(method) = StorageMethod::from_label(&label) {
                            draft.update(|d| d.storage_method = method);
                        }
                    }
                >
                    {StorageMethod::all()
                        .into_iter()
                        .map(|method| view! { <option value=method.label()>{method.display()}</option> })
                        .collect::<Vec<_>>()}
                </select>
            </label>
            <label class="field">
                <span>"Acquired from"</span>
                <input type="text" placeholder="bank, breeder, own harvest"
                    prop:value=move || draft.get().acquired_from.unwrap_or_default()
                    on:input=move |ev| {
                        draft.update(|d| d.acquired_from = crate::non_empty(event_target_value(&ev)));
                    }
                />
            </label>
            <label class="field wide">
                <span>"Notes"</span>
                <textarea rows="2"
                    prop:value=move || draft.get().notes.unwrap_or_default()
                    on:input=move |ev| {
                        draft.update(|d| d.notes = crate::non_empty(event_target_value(&ev)));
                    }
                />
            </label>
        </div>
    }
}

/// A blank batch draft (the §5.4 default storage: cool, dark, dry).
const fn empty_batch() -> SeedBatchInput {
    SeedBatchInput {
        strain_id: None,
        quantity: 1,
        acquired_from: None,
        acquisition_on: String::new(),
        storage_method: StorageMethod::CoolDarkDry,
        notes: None,
    }
}
