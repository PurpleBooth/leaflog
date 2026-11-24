//! The grow edit page (grow-page rework ticket 01): the single home
//! for grow configuration — the grow form with its Advanced
//! disclosure, the Entity bindings for all seven roles, the Light
//! Cycle editor and the delete danger zone. The overview's header
//! Edit control links here; saving or deleting returns to the
//! overview. Nothing here actuates (ADR 0002): every write goes to
//! the same endpoints the overview's old inline surfaces used.
//! Leaving a dirty form asks first (D125); a clean form and a
//! successful save never do.

use leaflog_core::grows::{Grow, GrowInput, GrowStatus, PhotoperiodType, Tolerance};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;
use wasm_bindgen::{JsCast, closure::Closure};

use crate::ErrorIf;
use crate::api;
use crate::bindings::EntityBindings;
use crate::grows::{GrowForm, empty_grow};
use crate::i18n::{t, t_string, use_i18n};
use crate::light_cycle::LightCycleEditor;
use crate::ui;

/// The trim-normalized shape of every field the grow form can edit:
/// the baseline the unsaved-changes guard compares the live draft
/// against. Text is trimmed at snapshot time and blank notes become
/// `None` (matching the form's `non_empty` write path), so a stray
/// space or a blanked notes field is never an edit worth a confirm.
/// The fitted feed template is not editable through this form and is
/// deliberately left out.
#[derive(Debug, Clone, PartialEq)]
struct FormSnapshot {
    /// User-chosen name.
    name: String,
    /// Free-text strain label.
    strain_label: String,
    /// What the roots grow in (the medium, stored as `substrate`).
    substrate: String,
    /// Seed/clone start date, `YYYY-MM-DD`.
    started_on: String,
    /// Planned vegetative weeks for photoperiods.
    planned_veg_weeks: Option<u8>,
    /// Life-cycle status.
    status: GrowStatus,
    /// Free-form notes; whitespace-only reads as unset.
    notes: Option<String>,
    /// Photoperiod behaviour.
    photoperiod: PhotoperiodType,
    /// Heat tolerance level.
    heat_tolerance: Tolerance,
    /// Cold tolerance level.
    cold_tolerance: Tolerance,
    /// Light tolerance level.
    light_tolerance: Tolerance,
    /// The lamp's measured wall watts; `None` = unknown.
    real_watts: Option<f64>,
    /// Whether the lamp is dimmable.
    dimmable: bool,
    /// The assigned feeding schedule; `None` = unassigned.
    feed_schedule_id: Option<String>,
}

impl FormSnapshot {
    /// Captures the form-editable fields of a stored grow: the
    /// pristine baseline taken when the page loads it.
    #[must_use]
    pub fn from_grow(grow: &Grow) -> Self {
        Self::from_input(&GrowInput::from_grow(grow))
    }

    /// Captures the live draft the form writes into.
    #[must_use]
    pub fn from_input(input: &GrowInput) -> Self {
        Self {
            name: input.name.trim().to_owned(),
            strain_label: input.strain_label.trim().to_owned(),
            substrate: input.substrate.trim().to_owned(),
            started_on: input.started_on.trim().to_owned(),
            planned_veg_weeks: input.planned_veg_weeks,
            status: input.status,
            notes: notes_snapshot(input.notes.as_deref()),
            photoperiod: input.photoperiod,
            heat_tolerance: input.heat_tolerance,
            cold_tolerance: input.cold_tolerance,
            light_tolerance: input.light_tolerance,
            real_watts: input.real_watts,
            dimmable: input.dimmable,
            feed_schedule_id: input.feed_schedule_id.clone(),
        }
    }
}

/// Trims free-form notes; blank (whitespace-only) notes read as
/// `None`, matching the form's `non_empty` write path.
fn notes_snapshot(notes: Option<&str>) -> Option<String> {
    notes
        .map(str::trim)
        .filter(|trimmed| !trimmed.is_empty())
        .map(str::to_owned)
}

/// Whether the form moved away from its pristine baseline — the one
/// decision point of the unsaved-changes guard, so every page exit
/// asks the same question about the same state.
#[must_use]
fn edits_dirty(before: &FormSnapshot, after: &FormSnapshot) -> bool {
    before != after
}

/// The closest enclosing anchor of a click target, if the click
/// landed inside a link (the target may be a child of it).
fn enclosing_anchor(element: &web_sys::Element) -> Option<web_sys::Element> {
    let mut current = Some(element.clone());
    while let Some(node) = current {
        if node.tag_name() == "A" {
            return Some(node);
        }
        current = node.parent_element();
    }
    None
}

/// Turns an anchor's `href` attribute into the plain route path the
/// router's `navigate` resolves: `route_href` baked the runtime
/// ingress base into the attribute, and a base-prefixed path handed
/// back to `navigate` would double the prefix. `None` for anything
/// that is not an in-app route path — foreign URLs, bare hash links
/// — which the guard then leaves alone.
#[must_use]
fn route_path_from_href(href: &str) -> Option<String> {
    let base = crate::ingress_base();
    let path = if base.is_empty() {
        href
    } else {
        href.strip_prefix(&base)?
    };
    (path.starts_with('/')).then(|| path.to_owned())
}

/// The `/grows/:id/edit` route: reads the path id and renders the
/// edit page.
#[component]
pub fn GrowEditRoute() -> impl IntoView {
    let params = use_params_map();
    let grow_id = Signal::derive(move || params.get().get("id").unwrap_or_default());
    view! { <GrowEditPage grow_id /> }
}

/// The grow edit page. Loads the grow, hosts the form, the bindings,
/// the Light Cycle editor and the danger zone. While the form is
/// dirty, leaving it (nav links, Cancel, reload) asks first (D125);
/// a clean form and a successful save never do, and the delete flow
/// keeps its own confirmation.
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
    // The pristine baseline of the unsaved-changes guard (D125):
    // anchored when the grow loads, re-anchored after a successful
    // save, so the guard always compares the live draft against the
    // last saved truth.
    let pristine = StoredValue::<Option<FormSnapshot>>::new(None);
    // The guard's single decision point (D125): live draft vs the
    // last saved truth. `None` (grow not loaded yet) reads clean.
    let unsaved_changes = move || {
        pristine.get_value().is_some_and(|before| {
            edits_dirty(&before, &FormSnapshot::from_input(&draft.get_untracked()))
        })
    };
    // One localized question for every guarded exit (nav-link guard
    // and Cancel alike); a missing window answers "stay".
    let confirm_unsaved = move || {
        leptos::web_sys::window()
            .and_then(|window| {
                window
                    .confirm_with_message(t_string!(i18n, grow_edit.unsaved_confirm).as_ref())
                    .ok()
            })
            .unwrap_or(false)
    };

    let refresh = move || {
        let id = grow_id.get();
        spawn_local(async move {
            match api::get::<Grow>(&format!("api/grows/{id}")).await {
                // The draft must carry the stored values before the
                // gated body starts rendering the form.
                Ok(value) => {
                    pristine.set_value(Some(FormSnapshot::from_grow(&value)));
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
                    // Re-anchor the baseline before navigating: the
                    // route change unmounts this page, but the guard
                    // must already read clean if anything pokes the
                    // form during the transition.
                    pristine.set_value(Some(FormSnapshot::from_input(&input)));
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
        // Cancel is a button, not an anchor: the document-level
        // guard never sees it, so it runs the same dirty check.
        if unsaved_changes() && !confirm_unsaved() {
            return;
        }
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

    // The unsaved-changes guard (D125). leptos_router 0.8 ships no
    // navigation blocker, so a dirty page answers for itself:
    //
    // - A document-level capture-phase click listener runs before
    //   the router's global anchor handler (bubble phase), which
    //   honours `default_prevented` — preventing here suppresses
    //   the SPA navigation, and a confirmed leave re-navigates with
    //   the plain route path (the ingress base stripped back off
    //   the href).
    // - A window `beforeunload` listener covers reloads and tab
    //   closes with the browser's native prompt — not localizable,
    //   and deliberately not duplicated in-page.
    //
    // Both check dirtiness at event time, so a clean form is never
    // disturbed. Save re-anchors the baseline before it navigates,
    // and the delete flow keeps its own confirmation (its button
    // fires no navigation the guard would see).
    if let (Some(window), Some(document)) = (
        leptos::web_sys::window(),
        leptos::web_sys::window().and_then(|window| window.document()),
    ) {
        let on_click = Closure::<dyn Fn(web_sys::Event)>::new(move |ev: web_sys::Event| {
            if !unsaved_changes() {
                return;
            }
            // Modified clicks and anchors with an explicit
            // target open elsewhere — they never abandon the
            // dirty form, so leave them alone.
            if let Some(mouse) = ev.dyn_ref::<web_sys::MouseEvent>()
                && (mouse.ctrl_key() || mouse.meta_key() || mouse.shift_key() || mouse.alt_key())
            {
                return;
            }
            let Some(anchor) = ev
                .target()
                .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                .as_ref()
                .and_then(enclosing_anchor)
            else {
                return;
            };
            if anchor
                .get_attribute("target")
                .is_some_and(|target| !target.is_empty())
            {
                return;
            }
            let Some(path) = anchor
                .get_attribute("href")
                .as_deref()
                .and_then(route_path_from_href)
            else {
                return;
            };
            // Suppress the router's own anchor handler and any
            // default browser navigation before asking.
            ev.prevent_default();
            if !confirm_unsaved() {
                return;
            }
            let navigate = navigate.get_value();
            navigate(&path, leptos_router::NavigateOptions::default());
        });
        let on_unload = Closure::<dyn Fn(web_sys::Event)>::new(move |ev: web_sys::Event| {
            if !unsaved_changes() {
                return;
            }
            ev.prevent_default();
            if let Some(unload) = ev.dyn_ref::<web_sys::BeforeUnloadEvent>() {
                unload.set_return_value("");
            }
        });
        let _ = document.add_event_listener_with_callback_and_bool(
            "click",
            on_click.as_ref().unchecked_ref(),
            true,
        );
        let _ = window
            .add_event_listener_with_callback("beforeunload", on_unload.as_ref().unchecked_ref());
        // `on_cleanup` demands `Send + Sync`, which wasm handles are
        // not: the guards live in an owner-local stored value instead
        // (the live.rs pattern), and cleanup removes then drops them
        // exactly once.
        let guards = StoredValue::new_local(Some((document, window, on_click, on_unload)));
        on_cleanup(move || {
            guards.update_value(|slot| {
                if let Some((document, window, on_click, on_unload)) = slot.take() {
                    let _ = document.remove_event_listener_with_callback(
                        "click",
                        on_click.as_ref().unchecked_ref(),
                    );
                    let _ = window.remove_event_listener_with_callback(
                        "beforeunload",
                        on_unload.as_ref().unchecked_ref(),
                    );
                    drop(on_click);
                    drop(on_unload);
                }
            });
        });
    }

    view! {
        <section>
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
                            <form class={format!("{} my-4", ui::card())} on:submit=save>
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
                            <div class={format!("{} border border-error/30 my-4", ui::card())}>
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

#[cfg(test)]
mod form_snapshot_tests {
    use super::{FormSnapshot, edits_dirty};
    use leaflog_core::grows::{Grow, GrowInput, GrowStatus, PhotoperiodType, Tolerance};

    /// A stored grow exercising every editable field with a
    /// distinctive value.
    fn grow() -> Grow {
        Grow {
            id: "01J0000000000000000000000".to_owned(),
            name: "Balcony tent".to_owned(),
            feed_schedule_id: Some("tpl-bloom".to_owned()),
            planned_veg_weeks: Some(5),
            strain_label: "Northern Lights".to_owned(),
            photoperiod: PhotoperiodType::Photoperiod,
            heat_tolerance: Tolerance::Average,
            cold_tolerance: Tolerance::Sensitive,
            light_tolerance: Tolerance::Tolerant,
            real_watts: Some(105.5),
            dimmable: true,
            light_cycle: None,
            light_unavailable_as_off: false,
            substrate: "coco".to_owned(),
            started_on: "2026-05-01".to_owned(),
            status: GrowStatus::Active,
            notes: Some("top-dressed once".to_owned()),
            created_at: "2026-05-01T00:00:00Z".to_owned(),
            updated_at: "2026-05-01T00:00:00Z".to_owned(),
        }
    }

    fn pristine() -> (FormSnapshot, GrowInput) {
        let grow = grow();
        let input = GrowInput::from_grow(&grow);
        (FormSnapshot::from_grow(&grow), input)
    }

    #[test]
    fn a_loaded_untouched_form_is_clean() {
        let (before, input) = pristine();
        assert!(!edits_dirty(&before, &FormSnapshot::from_input(&input)));
    }

    #[test]
    fn changing_each_editable_field_is_dirty() {
        let (before, _) = pristine();
        let dirty_after = |change: &dyn Fn(&mut FormSnapshot)| {
            let mut after = before.clone();
            change(&mut after);
            edits_dirty(&before, &after)
        };
        assert!(dirty_after(&|s| s.name = "Renamed".to_owned()));
        assert!(dirty_after(
            &|s| s.strain_label = "Other cultivar".to_owned()
        ));
        assert!(dirty_after(&|s| s.substrate = "hydro".to_owned()));
        assert!(dirty_after(&|s| s.started_on = "2026-06-01".to_owned()));
        assert!(dirty_after(&|s| s.planned_veg_weeks = Some(7)));
        assert!(dirty_after(&|s| s.status = GrowStatus::Archived));
        assert!(dirty_after(&|s| s.notes = Some("changed".to_owned())));
        assert!(dirty_after(&|s| s.notes = None));
        assert!(dirty_after(&|s| s.photoperiod = PhotoperiodType::Auto));
        assert!(dirty_after(&|s| s.heat_tolerance = Tolerance::Sensitive));
        assert!(dirty_after(&|s| s.cold_tolerance = Tolerance::Average));
        assert!(dirty_after(&|s| s.light_tolerance = Tolerance::Average));
        assert!(dirty_after(&|s| s.real_watts = Some(120.0)));
        assert!(dirty_after(&|s| s.real_watts = None));
        assert!(dirty_after(&|s| s.dimmable = false));
    }

    #[test]
    fn whitespace_only_draft_edits_are_not_dirty() {
        let (before, mut input) = pristine();
        input.name = format!("  {}  ", input.name);
        input.strain_label = format!("{}  ", input.strain_label);
        input.notes = Some("top-dressed once ".to_owned());
        assert!(!edits_dirty(&before, &FormSnapshot::from_input(&input)));
    }

    #[test]
    fn blanking_the_notes_read_as_the_change_it_is() {
        // Cleared to blank on a grow that had notes: a real edit.
        let (before, mut input) = pristine();
        input.notes = Some("   ".to_owned());
        assert!(edits_dirty(&before, &FormSnapshot::from_input(&input)));

        // Whitespace typed into a grow without notes: not an edit.
        let mut unlabelled = grow();
        unlabelled.notes = None;
        let before = FormSnapshot::from_grow(&unlabelled);
        let mut input = GrowInput::from_grow(&unlabelled);
        input.notes = Some("   ".to_owned());
        assert!(!edits_dirty(&before, &FormSnapshot::from_input(&input)));
    }
}
