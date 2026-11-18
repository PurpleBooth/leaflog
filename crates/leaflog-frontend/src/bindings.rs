//! Entity bindings for one grow (grow-page rework ticket 01): the
//! pickers for the seven binding roles, relocated from the overview's
//! live panel to the grow edit page — the single home for binding
//! setup. Onboarding step 1 (D078) picks the first bindings when a
//! grow is created; everything after that happens here. Writes go to
//! the unchanged bindings endpoint.

use std::collections::BTreeMap;

use crate::ErrorIf;
use crate::api;
use leptos::prelude::*;
use leptos::task::spawn_local;

/// The binding roles the UI offers (shared by the live tiles, the
/// onboarding wizard's step 1 and this section's pickers).
pub const BINDING_ROLES: &[(&str, &str)] = &[
    ("temperature", "Temperature"),
    ("humidity", "Humidity"),
    ("co2", "CO2"),
    ("soil_moisture", "Soil moisture"),
    ("light", "Light"),
    ("reservoir_ph", "Reservoir pH"),
    ("reservoir_ec", "Reservoir EC"),
];

/// The Entity bindings section of the grow edit page: one picker per
/// role. A change saves the whole map immediately — the endpoint and
/// payload are unchanged from the overview's old live-panel pickers.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn EntityBindings(grow_id: String) -> impl IntoView {
    let bindings = RwSignal::<BTreeMap<String, String>>::new(BTreeMap::new());
    let entities = RwSignal::<Vec<serde_json::Value>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);

    let refresh = {
        let grow_id = grow_id.clone();
        move || {
            let grow_id = grow_id.clone();
            spawn_local(async move {
                match api::get::<BTreeMap<String, String>>(&format!("api/grows/{grow_id}/bindings"))
                    .await
                {
                    Ok(data) => bindings.set(data),
                    Err(failure) => error.set(Some(failure.message)),
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
                Ok(_) => error.set(None),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    view! {
        <div class="my-4">
            <ErrorIf error />
            <h3>"Entity bindings"</h3>
            <p class="text-base-content/70 text-sm">
                "Bind a Home Assistant entity to each role — bindings are the only
                sensor setup there is, and the picks here are this grow's own."
            </p>
            <div class="grid gap-3 grid-cols-[repeat(auto-fill,minmax(14rem,1fr))]">
                {BINDING_ROLES
                    .iter()
                    .map(|(role, label)| {
                        let role = *role;
                        let current = bindings;
                        let options = entities;
                        let save = save_binding.clone();
                        view! {
                            <label class="fieldset">
                                <span class="fieldset-label">{*label}</span>
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
                                    <option value="">"— unbound —"</option>
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
                            </label>
                        }
                    })
                    .collect::<Vec<_>>()}
            </div>
        </div>
    }
}
