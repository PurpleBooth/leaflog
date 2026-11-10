//! Live sensor tiles fed by the HA bridge (M4).

use crate::api;
use crate::equipment::ErrorIf;
use leptos::prelude::*;
use leptos::task::spawn_local;

/// The binding roles the UI offers.
pub const BINDING_ROLES: &[(&str, &str)] = &[
    ("temperature", "Temperature"),
    ("humidity", "Humidity"),
    ("co2", "CO2"),
    ("soil_moisture", "Soil moisture"),
    ("light", "Light"),
    ("reservoir_ph", "Reservoir pH"),
    ("reservoir_ec", "Reservoir EC"),
];

/// Live tiles + the binding pickers for one grow.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn LivePanel(grow_id: String) -> impl IntoView {
    let live = RwSignal::<Option<api::LiveData>>::new(None);
    let bindings = RwSignal::<std::collections::BTreeMap<String, String>>::new(
        std::collections::BTreeMap::new(),
    );
    let entities = RwSignal::<Vec<serde_json::Value>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);

    let refresh = {
        let grow_id = grow_id.clone();
        move || {
            let grow_id = grow_id.clone();
            spawn_local(async move {
                match api::get::<api::LiveData>("api/live").await {
                    Ok(data) => live.set(Some(data)),
                    Err(failure) => error.set(Some(failure.message)),
                }
                match api::get::<std::collections::BTreeMap<String, String>>(&format!(
                    "api/grows/{grow_id}/bindings"
                ))
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

    // Poll live data (SSE would be nicer; polling keeps it simple).
    Effect::new(move |_| {
        let poll = {
            let live = live;
            move || {
                spawn_local(async move {
                    if let Ok(data) = api::get::<api::LiveData>("api/live").await {
                        live.set(Some(data));
                    }
                });
            }
        };
        set_interval(poll, std::time::Duration::from_secs(5));
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
            <h3>"Live sensors"</h3>
            <p class="text-base-content/70 text-sm">
                {move || {
                    live.get().map_or_else(
                        || "Loading…".to_owned(),
                        |data| {
                            if data.connected {
                                "Connected to Home Assistant".to_owned()
                            } else {
                                "Home Assistant unreachable (data still works)".to_owned()
                            }
                        },
                    )
                }}
            </p>
            <div class="grid gap-2 grid-cols-[repeat(auto-fill,minmax(9rem,1fr))]">
                {move || {
                    let Some(data) = live.get() else {
                        return Vec::<_>::new();
                    };
                    let bound = bindings.get();
                    BINDING_ROLES
                        .iter()
                        .filter_map(|(role, label)| {
                            let entity_id = bound.get(*role)?;
                            let entity = data.entities.get(entity_id)?;
                            let unit = entity
                                .attributes
                                .get("unit_of_measurement")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or_default();
                            Some(
                                view! {
                                    <div class="rounded-box border border-base-300 bg-base-100 px-3 py-2 grid gap-0.5">
                                        <span class="text-xs uppercase tracking-wide text-base-content/60">{*label}</span>
                                        <span class="text-lg font-semibold">
                                            {format!("{} {}", entity.state, unit)}
                                        </span>
                                    </div>
                                }
                                .into_any(),
                            )
                        })
                        .collect::<Vec<_>>()
                }}
            </div>

            <h4>"Entity bindings"</h4>
            <p class="text-base-content/70 text-sm">
                "A grow starts from its space's sensor items (add them on the equipment page);
                the picks here are this grow's own overrides."
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
