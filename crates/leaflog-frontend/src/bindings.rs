//! Entity bindings for one grow (grow-page rework ticket 01): the
//! pickers for the seven binding roles, relocated from the overview's
//! live panel to the grow edit page — the single home for binding
//! setup. Onboarding step 1 (D078) picks the first bindings when a
//! grow is created; everything after that happens here. Writes go to
//! the unchanged bindings endpoint.

use std::collections::BTreeMap;

use crate::ErrorIf;
use crate::api;
use crate::i18n::{t, use_i18n};
use crate::labels::binding_role_label;
use crate::ui;
use leptos::prelude::*;
use leptos::task::spawn_local;

/// The canonical sensor roles, as stored in the entity map and
/// matched by the live merge: one spelling each, never re-typed
/// literals.
pub const ROLE_TEMPERATURE: &str = "temperature";

/// See [`ROLE_TEMPERATURE`].
pub const ROLE_HUMIDITY: &str = "humidity";

/// See [`ROLE_TEMPERATURE`].
pub const ROLE_CO2: &str = "co2";

/// See [`ROLE_TEMPERATURE`].
pub const ROLE_SOIL_MOISTURE: &str = "soil_moisture";

/// See [`ROLE_TEMPERATURE`].
pub const ROLE_LIGHT: &str = "light";

/// See [`ROLE_TEMPERATURE`].
pub const ROLE_RESERVOIR_PH: &str = "reservoir_ph";

/// See [`ROLE_TEMPERATURE`].
pub const ROLE_RESERVOIR_EC: &str = "reservoir_ec";

/// The binding roles the UI offers (shared by the live tiles, the
/// onboarding wizard's step 1 and this section's pickers). Display
/// labels come from the catalogs via
/// [`crate::labels::binding_role_label`], keyed per locale.
pub const BINDING_ROLES: &[&str] = &[
    ROLE_TEMPERATURE,
    ROLE_HUMIDITY,
    ROLE_CO2,
    ROLE_SOIL_MOISTURE,
    ROLE_LIGHT,
    ROLE_RESERVOIR_PH,
    ROLE_RESERVOIR_EC,
];

/// The Entity bindings section of the grow edit page: one picker per
/// role. A change saves the whole map immediately — the endpoint and
/// payload are unchanged from the overview's old live-panel pickers.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn EntityBindings(grow_id: String, bindings_epoch: RwSignal<u64>) -> impl IntoView {
    let bindings = RwSignal::<BTreeMap<String, String>>::new(BTreeMap::new());
    let entities = RwSignal::<Vec<serde_json::Value>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);
    let i18n = use_i18n();
    let locale = i18n.get_locale();

    let refresh = {
        let grow_id = grow_id.clone();
        move || {
            let grow_id = grow_id.clone();
            spawn_local(async move {
                match api::get::<BTreeMap<String, String>>(&format!("api/grows/{grow_id}/bindings"))
                    .await
                {
                    Ok(data) => bindings.set(data),
                    Err(failure) => error.set(Some(api::failure_text(
                        &failure,
                        i18n.get_locale_untracked(),
                    ))),
                }
                match api::get::<serde_json::Value>("api/ha/entities").await {
                    Ok(value) => {
                        entities.set(value["entities"].as_array().cloned().unwrap_or_default());
                    }
                    Err(_) => entities.set(Vec::new()),
                }
            });
        }
    };
    Effect::new(move |_| {
        refresh();
    });

    let save_binding = move |role: &'static str, entity_id: String| {
        let grow_id = grow_id.clone();
        spawn_local(async move {
            let mut current = bindings.get();
            if entity_id.is_empty() {
                current.remove(role);
            } else {
                current.insert(role.to_owned(), entity_id);
            }
            let body = serde_json::json!({ "bindings": current });
            match api::put::<serde_json::Value, serde_json::Value>(
                &format!("api/grows/{grow_id}/bindings"),
                &body,
            )
            .await
            {
                Ok(_) => {
                    error.set(None);
                    // Saved bindings change what the grow's observed
                    // reality says: bump the epoch so the Light Cycle
                    // caption re-derives (D102).
                    bindings_epoch.update(|epoch| *epoch += 1);
                }
                Err(failure) => error.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
            }
        });
    };

    view! {
        <div class={format!("{} my-4", ui::card())}>
            <ErrorIf error />
            <h3 class="mt-0">{t!(i18n, binding.heading)}</h3>
            <p class="text-base-content/70 text-sm">
                {t!(i18n, binding.intro)}
            </p>
            <div class="grid gap-2 grid-cols-[repeat(auto-fill,minmax(14rem,1fr))]">
                {BINDING_ROLES
                    .iter()
                    .map(|&role| {
                        let label = binding_role_label(locale, role);
                        let current = bindings;
                        let options = entities;
                        let save = save_binding.clone();
                        view! {
                            <ui::Field label=label>
                                <select class="select"
                                    prop:value=move || {
                                        // Re-apply when the option list lands
                                        // too: a value set before its option
                                        // exists clamps to the empty string.
                                        let _ = options.get();
                                        current.get().get(role).cloned().unwrap_or_default()
                                    }
                                    on:change=move |ev| {
                                        save(role, event_target_value(&ev));
                                    }
                                >
                                    <option value="">{t!(i18n, binding.unbound)}</option>
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
                            </ui::Field>
                        }
                    })
                    .collect::<Vec<_>>()}
            </div>
        </div>
    }
}
