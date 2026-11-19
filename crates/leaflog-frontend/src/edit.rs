//! The grow edit page (grow-page rework ticket 01): the single home
//! for grow configuration — the grow form with its Advanced
//! disclosure, the Entity bindings for all seven roles, the Light
//! Cycle editor and the delete danger zone. The overview's header
//! Edit control links here; saving or deleting returns to the
//! overview. Nothing here actuates (ADR 0002): every write goes to
//! the same endpoints the overview's old inline surfaces used.

use leaflog_core::grows::{Grow, GrowInput};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

use crate::ErrorIf;
use crate::api;
use crate::bindings::EntityBindings;
use crate::grows::{GrowForm, empty_grow};
use crate::i18n::{t, t_string, use_i18n};
use crate::light_cycle::LightCycleEditor;

/// The `/grows/:id/edit` route: reads the path id and renders the
/// edit page.
#[component]
pub fn GrowEditRoute() -> impl IntoView {
    let params = use_params_map();
    let grow_id = Signal::derive(move || params.get().get("id").unwrap_or_default());
    view! { <GrowEditPage grow_id /> }
}

/// The grow edit page. Loads the grow, hosts the form, the bindings,
/// the Light Cycle editor and the danger zone; every exit but Cancel
/// writes first.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn GrowEditPage(grow_id: Signal<String>) -> impl IntoView {
    let i18n = use_i18n();
    let grow = RwSignal::<Option<Grow>>::new(None);
    let draft = RwSignal::<GrowInput>::new(empty_grow());
    let error = RwSignal::<Option<String>>::new(None);
    let deleted = RwSignal::<bool>::new(false);
    // Bumped by the entity bindings on save: the Light Cycle editor
    // tracks it to re-derive its observed-reality caption (D102).
    let bindings_epoch = RwSignal::<u64>::new(0);
    let navigate = StoredValue::new_local(leptos_router::hooks::use_navigate());

    let refresh = move || {
        let id = grow_id.get();
        spawn_local(async move {
            match api::get::<Grow>(&format!("api/grows/{id}")).await {
                // The draft must carry the stored values before the
                // gated body starts rendering the form.
                Ok(value) => {
                    draft.set(GrowInput::from_grow(&value));
                    grow.set(Some(value));
                    error.set(None);
                }
                Err(failure) => error.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
            }
        });
    };
    Effect::new(move |_| {
        grow_id.track();
        refresh();
    });

    let grow_photoperiod = Signal::derive(move || {
        grow.get()
            .as_ref()
            .map(|current| current.photoperiod)
            .unwrap_or_default()
    });

    // prevent_default first — see docs/DECISIONS.md D061.
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let id = grow_id.get();
        spawn_local(async move {
            let input = draft.get();
            match api::put::<GrowInput, Grow>(&format!("api/grows/{id}"), &input).await {
                Ok(_) => {
                    error.set(None);
                    // The plain route path: navigate() resolves it
                    // against the router base itself — a base-prefixed
                    // path would double the ingress prefix.
                    let navigate = navigate.get_value();
                    navigate(
                        &format!("/grows/{id}"),
                        leptos_router::NavigateOptions::default(),
                    );
                }
                Err(failure) => error.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
            }
        });
    };

    let cancel = move |_ev: leptos::ev::MouseEvent| {
        let id = grow_id.get();
        let navigate = navigate.get_value();
        navigate(
            &format!("/grows/{id}"),
            leptos_router::NavigateOptions::default(),
        );
    };

    // Deleting a grow takes its plants, log entries and photos with
    // it, so it asks for confirmation first (same dialog as the old
    // overview control).
    let delete_grow = move |_ev: leptos::ev::MouseEvent| {
        let id = grow_id.get();
        let name = grow.get().map(|current| current.name).unwrap_or_default();
        let confirmed = leptos::web_sys::window()
            .and_then(|window| {
                window
                    .confirm_with_message(&t_string!(i18n, grow_edit.delete_confirm, name = name))
                    .ok()
            })
            .unwrap_or(false);
        if !confirmed {
            return;
        }
        spawn_local(async move {
            match api::delete(&format!("api/grows/{id}")).await {
                Ok(()) => deleted.set(true),
                Err(failure) => error.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
            }
        });
    };

    view! {
        <section class="pb-12">
            <h2>{t!(i18n, page.edit_heading)}</h2>
            <Show when=move || deleted.get()>
                // The plain route path: unlike an <A href>, Redirect
                // routes through navigate(), which resolves the path
                // against the router base itself — a base-prefixed
                // path would double the ingress prefix. The grow's own
                // page would 404 now, so deletion lands on the list.
                <leptos_router::components::Redirect path="/grows" />
            </Show>
            <ErrorIf error />
            {move || {
                grow.get().map_or_else(
                    || view! { <p class="text-base-content/70">{t_string!(i18n, grow_edit.loading)}</p> }
                        .into_any(),
                    |_| {
                        view! {
                            <form class="card bg-base-100 shadow-sm px-5 py-4 my-4" on:submit=save>
                                <GrowForm draft />
                                <div class="flex flex-wrap gap-2 mt-4">
                                    <button class="btn btn-primary btn-sm" type="submit">
                                        {t!(i18n, grows.save)}
                                    </button>
                                    <button class="btn btn-sm" type="button" on:click=cancel>
                                        {t!(i18n, grows.cancel)}
                                    </button>
                                </div>
                            </form>
                            <EntityBindings grow_id=grow_id.get() bindings_epoch />
                            <LightCycleEditor
                                grow_id=grow_id
                                photoperiod=grow_photoperiod
                                bindings_epoch=Signal::from(bindings_epoch)
                            />
                            <h3>{t!(i18n, grow_edit.danger_heading)}</h3>
                            <div class="card bg-base-100 border border-error/30 px-5 py-4 my-2">
                                <p class="text-base-content/70 text-sm">
                                    {t!(i18n, grow_edit.danger_text)}
                                </p>
                                <button class="btn btn-error btn-outline btn-sm mt-3"
                                    on:click=delete_grow>
                                    {t!(i18n, grow_edit.delete_grow)}
                                </button>
                            </div>
                        }
                        .into_any()
                    },
                )
            }}
        </section>
    }
}
