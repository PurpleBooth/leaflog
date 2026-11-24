//! Entity bindings for one grow (grow-page rework ticket 01): the
//! pickers for the seven binding roles, relocated from the overview's
//! live panel to the grow edit page — the single home for binding
//! setup. Onboarding step 1 (D078) picks the first bindings when a
//! grow is created; everything after that happens here. Writes go to
//! the unchanged bindings endpoint.

use std::collections::BTreeMap;

use crate::ErrorIf;
use crate::api;
use crate::i18n::{t, t_string, use_i18n};
use crate::labels::binding_role_label;
use crate::ui;
use crate::ui::CardTitle;
use leptos::prelude::*;
use leptos::task::spawn_local;

/// The bindings endpoint's response: the role→entity map plus the
/// grow's dead-light stance (D159).
#[derive(Debug, Default, serde::Deserialize)]
pub struct BindingsView {
    /// The role→entity-id map the pickers edit.
    #[serde(default)]
    pub bindings: BTreeMap<String, String>,
    /// Whether a dead light entity reads as off for this grow.
    #[serde(default)]
    pub light_unavailable_as_off: bool,
}

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
    let light_stance = RwSignal::<bool>::new(false);
    let entities = RwSignal::<Vec<serde_json::Value>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);
    let i18n = use_i18n();

    let refresh = {
        let grow_id = grow_id.clone();
        move || {
            let grow_id = grow_id.clone();
            spawn_local(async move {
                match api::get::<BindingsView>(&format!("api/grows/{grow_id}/bindings")).await {
                    Ok(data) => {
                        bindings.set(data.bindings);
                        light_stance.set(data.light_unavailable_as_off);
                    }
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

    // The stance save needs its own grow-id: `save_binding` moves the
    // original into its closure.
    let stance_grow_id = grow_id.clone();
    let save_binding = move |role: &'static str, entity_id: String| {
        let grow_id = grow_id.clone();
        spawn_local(async move {
            let mut current = bindings.get();
            if entity_id.is_empty() {
                current.remove(role);
            } else {
                current.insert(role.to_owned(), entity_id);
            }
            let body = serde_json::json!({
                "bindings": current,
                // Picker saves preserve the stance (D159).
                "light_unavailable_as_off": light_stance.get(),
            });
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

    // The stance toggle's save: same endpoint, same map, the flag
    // flipped. The stance changes what dead hours mean, so the
    // observed caption re-derives too (D102/D159).
    let save_stance = move |checked: bool| {
        let grow_id = stance_grow_id.clone();
        spawn_local(async move {
            let body = serde_json::json!({
                "bindings": bindings.get(),
                "light_unavailable_as_off": checked,
            });
            match api::put::<serde_json::Value, serde_json::Value>(
                &format!("api/grows/{grow_id}/bindings"),
                &body,
            )
            .await
            {
                Ok(_) => {
                    error.set(None);
                    light_stance.set(checked);
                    bindings_epoch.update(|epoch| *epoch += 1);
                }
                Err(failure) => {
                    // A failed flip reverts the checkbox to the
                    // stored stance.
                    light_stance.set(!checked);
                    error.set(Some(api::failure_text(
                        &failure,
                        i18n.get_locale_untracked(),
                    )));
                }
            }
        });
    };

    view! {
        <div class={format!("{} my-4", ui::card())}>
            <ErrorIf error />
            <CardTitle title=Signal::derive(move || t_string!(i18n, binding.heading).into_owned()) />
            <p class="text-base-content/70 text-sm">
                {t!(i18n, binding.intro)}
            </p>
            <div class="grid gap-2 grid-cols-[repeat(auto-fill,minmax(14rem,1fr))]">
                {BINDING_ROLES
                    .iter()
                    .map(|&role| {
                        let label = Signal::derive(move || binding_role_label(i18n.get_locale(), role));
                        let current = bindings;
                        let options = entities;
                        let save = save_binding.clone();
                        let bound_roles = bindings;
                        let stance = light_stance;
                        let save_flip = save_stance.clone();
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
                                // The dead-light stance rides on the light
                                // row: only a bound light can be flaky
                                // (D159).
                                {move || {
                                    if role == ROLE_LIGHT
                                        && bound_roles.get().contains_key(ROLE_LIGHT)
                                    {
                                        let save_flip = save_flip.clone();
                                        view! {
                                            <label class="fieldset mt-1">
                                                <span class="fieldset-label">
                                                    {t_string!(i18n, binding.unavailable_as_off)}
                                                </span>
                                                <input class="checkbox" type="checkbox"
                                                    data-stance="light_unavailable_as_off"
                                                    prop:checked=move || stance.get()
                                                    on:change=move |ev| {
                                                        save_flip(event_target_checked(&ev));
                                                    }
                                                />
                                                <span class="text-xs text-base-content/70">
                                                    {t_string!(i18n, binding.unavailable_as_off_hint)}
                                                </span>
                                            </label>
                                        }
                                        .into_any()
                                    } else {
                                        ().into_any()
                                    }
                                }}
                            </ui::Field>
                        }
                    })
                    .collect::<Vec<_>>()}
            </div>
        </div>
    }
}
