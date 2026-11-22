//! Shared surface primitives: the two tier classes (full card and
//! tile), the chart-card grid, and the small reusable components the
//! pages compose from them. Class literals live here as plain
//! strings so the Tailwind scanner sees every utility a primitive
//! emits.

use leptos::prelude::*;

/// The full-card tier (the section and form surface): daisyUI card
/// chrome with the spec's `px-5 py-4` padding. Rhythm margins (the
/// spec's `my-4` between stacked cards) stay with the call site, so
/// cards inside gap-managed grids do not double the spacing.
#[must_use]
pub const fn card() -> &'static str {
    "card bg-base-100 shadow-sm px-5 py-4"
}

/// The tile tier (the compact surface): hairline-bordered,
/// radius-boxed box with the spec's `px-4 py-3` padding.
#[must_use]
pub const fn tile() -> &'static str {
    "rounded-box border border-base-300 bg-base-100 px-4 py-3"
}

/// The chart-card grid: two-up on wide screens (the paired roles
/// read side by side and the scroll shortens by half). Explicit
/// `grid-cols-1` at the base keeps every track `minmax(0, 1fr)`: an
/// implicit auto track would let a not-yet-resized chart SVG
/// stretch the column, and `resize()` — which re-reads the
/// container — would then confirm the stale width instead of
/// shrinking into it.
pub const CARD_GRID: &str = "grid grid-cols-1 gap-4 items-start lg:grid-cols-2";

use crate::i18n::{t_string, use_i18n};
use gloo_timers::callback::Timeout;

/// A page title and its actions, aligned and wrapped the same way on
/// every list page. Use small buttons for the page-level actions.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn PageHeader(children: Children) -> impl IntoView {
    view! { <div class="flex flex-wrap items-center justify-between gap-4">{children()}</div> }
}

/// The undo window: how long a pending undo stays live after its
/// record was made (docs/DECISIONS.md D126).
pub const UNDO_WINDOW_MS: u64 = 8_000;

/// One pending undo: what an Undo would restore, plus the caller-clock
/// instant it was recorded. The clock is a plain millisecond count so
/// the expiry rule below stays pure and testable on every target.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PendingUndo<T> {
    /// Caller-clock instant the record was made, in milliseconds.
    pub recorded_at_ms: u64,
    /// What an Undo would restore.
    pub payload: T,
}

/// A record is live while its own window has not passed: measured
/// from when it was recorded, not from when the banner was last
/// touched, so a rapid second delete cannot extend the first one.
#[must_use]
pub const fn undo_is_live(recorded_at_ms: u64, now_ms: u64, window_ms: u64) -> bool {
    now_ms.saturating_sub(recorded_at_ms) < window_ms
}

/// The surviving undo: the newest record, if it is still live — one
/// banner, last delete wins; earlier records never surface again
/// (docs/DECISIONS.md D126).
#[must_use]
pub fn live_undo<T>(
    pending: &[PendingUndo<T>],
    now_ms: u64,
    window_ms: u64,
) -> Option<&PendingUndo<T>> {
    pending
        .last()
        .filter(|record| undo_is_live(record.recorded_at_ms, now_ms, window_ms))
}

/// The wall clock in milliseconds — the caller-clock the undo window
/// is measured against (also used to time-stamp recorded undoes).
// f64 -> u64 is the one unavoidable primitive cast; `Date::now()` is
// always a positive, finite millisecond count.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // see docs/DECISIONS.md
#[must_use]
pub fn now_ms() -> u64 {
    js_sys::Date::now() as u64
}

/// A tag chip: the `badge badge-ghost` recipe (DESIGN.md chips) around
/// one short label. Callers that need the small data-row variant add
/// their own `badge-sm` through `class`.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn Chip(
    children: Children,
    #[prop(optional)] class: &'static str,
    #[prop(optional)] id: Option<&'static str>,
) -> impl IntoView {
    let class = if class.is_empty() {
        "badge badge-ghost".to_owned()
    } else {
        format!("badge badge-ghost {class}")
    };
    view! { <span class=class id=id>{children()}</span> }
}

/// One labelled figure in a [`FactGrid`]: the uppercase micro-caption
/// over its value, the app's signature data register (DESIGN.md).
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn Fact(
    label: Signal<String>,
    children: Children,
    #[prop(optional)] class: &'static str,
    #[prop(optional)] testid: Option<&'static str>,
) -> impl IntoView {
    let class = if class.is_empty() {
        String::new()
    } else {
        format!(" {class}")
    };
    view! {
        <div class=class data-testid=testid>
            <dt>{move || label.get()}</dt>
            <dd>{children()}</dd>
        </div>
    }
}

/// The definition grid that holds [`Fact`]s. `single` is for cards
/// that report one fact: the auto-fill columns would otherwise hold
/// the value in a 10rem gutter and wrap it while the rest of the card
/// sits empty.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn FactGrid(children: Children, #[prop(optional)] single: bool) -> impl IntoView {
    let class = if single {
        "facts facts--single"
    } else {
        "facts"
    };
    view! { <dl class=class>{children()}</dl> }
}

/// A card's own title: the `h3` that opens a card opts out of the
/// document heading rhythm, because the card's padding already holds
/// it off the block above.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn CardTitle(title: Signal<String>) -> impl IntoView {
    view! { <h3 class="mt-0">{move || title.get()}</h3> }
}

/// The quiet empty state: one centered, muted line where a list or
/// table has nothing to show. The message stays with the caller as
/// children so it keeps its reactive translation.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn EmptyState(children: Children) -> impl IntoView {
    view! { <p class="py-6 text-center text-base-content/60">{children()}</p> }
}

/// The section micro-label header: the group's name in the section's
/// quiet uppercase register, above its content. The title arrives as
/// a signal so a locale swap re-translates it.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn SectionHeader(title: Signal<String>) -> impl IntoView {
    view! {
        <div class="flex flex-wrap items-baseline gap-2 mt-4 mb-2">
            <h4 class="m-0 text-xs font-semibold uppercase tracking-wider text-base-content/60">
                {move || title.get()}
            </h4>
        </div>
    }
}

/// The shared form-field wrapper: a daisyUI fieldset with the small
/// fieldset-label above one control. Every labelled input, select and
/// textarea in the app renders through this so the label register
/// stays uniform; the control itself arrives as children. The label
/// arrives as a signal so a locale swap re-translates it (the i18n
/// context resolves asynchronously — `LocaleChain`). `title` carries
/// an optional hover tooltip (e.g. the light-cycle default hours'
/// provenance); `class` appends layout modifiers such as
/// `col-span-full` without touching the base recipe.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn Field(
    label: Signal<String>,
    #[prop(optional)] title: Option<Signal<String>>,
    #[prop(optional)] class: &'static str,
    children: Children,
) -> impl IntoView {
    view! {
        <label
            class=move || {
                if class.is_empty() {
                    "fieldset".to_owned()
                } else {
                    format!("fieldset {class}")
                }
            }
            title=title
        >
            <span class="fieldset-label">{move || label.get()}</span>
            {children()}
        </label>
    }
}

/// The shared numeric field: the [`Field`] wrapper around a number
/// input bound to a draft struct. `read` renders the current value
/// (`display` reformats when a field wants fixed decimals), `write`
/// stores the parsed input, `None` clearing the value. `minimum`/
/// `maximum`/`placeholder` carry static `min`/`max`/`placeholder`
/// attributes; an empty string means the attribute is left off.
///
/// The value binds via `prop:`, the property write: once the user
/// types into an input, the browser stops following the value
/// *attribute* (the dirty-value flag), so an attribute write can no
/// longer repaint the box. When `write` rejects an entry, the
/// property write is what actually clears the field.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn NumberField<T: Clone + Send + Sync + 'static>(
    /// A signal so a locale swap re-translates the label.
    label: Signal<String>,
    step: &'static str,
    draft: RwSignal<T>,
    read: impl Fn(&T) -> Option<f64> + Clone + Send + 'static,
    write: impl Fn(&mut T, Option<f64>) + Clone + Send + 'static,
    #[prop(optional)] minimum: &'static str,
    #[prop(optional)] maximum: &'static str,
    #[prop(optional)] placeholder: &'static str,
    /// Shared so the field can hold it without a second generic slot.
    #[prop(optional)]
    display: Option<std::sync::Arc<dyn Fn(f64) -> String + Send + Sync>>,
) -> impl IntoView {
    let render = move || {
        read(&draft.get()).map_or_else(String::new, |value| {
            display
                .as_ref()
                .map_or_else(|| value.to_string(), |format| format(value))
        })
    };
    view! {
        <label class="fieldset">
            <span class="fieldset-label">{move || label.get()}</span>
            <input
                class="input"
                type="number"
                step=step
                min=(!minimum.is_empty()).then_some(minimum)
                max=(!maximum.is_empty()).then_some(maximum)
                placeholder=(!placeholder.is_empty()).then_some(placeholder)
                // Property, not attribute: the property write repaints
                // the box even after the user typed into it (the
                // attribute path stops applying once the dirty-value
                // flag is set), so a rejected entry clears visibly.
                prop:value=render
                on:input=move |ev| {
                    let parsed = event_target_value(&ev).parse::<f64>().ok();
                    draft.update(|current| write(current, parsed));
                }
            />
        </label>
    }
}

/// The shared two-step destructive action: a quiet ghost button that
/// arms on click and swaps to the loud confirm button. The armed
/// state belongs to the caller (a list arm per row, an editor one
/// flag); the component only renders the pair and forwards the
/// clicks, so an armed row never confirms a different row's delete.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn ArmConfirmButton(
    /// Whether this instance currently shows the confirm button.
    armed: Signal<bool>,
    /// The quiet button's caption ("Delete"), as a signal so a
    /// locale swap re-translates it.
    label: Signal<String>,
    /// The armed button's caption ("Confirm delete"), likewise a signal.
    confirm_label: Signal<String>,
    on_arm: impl Fn(leptos::ev::MouseEvent) + Clone + Send + 'static,
    on_confirm: impl Fn(leptos::ev::MouseEvent) + Clone + Send + 'static,
) -> impl IntoView {
    // The render closure clones the callbacks and captions so the
    // whole closure stays `Send` (leptos requires that for view
    // closures) while re-running on every `armed` flip.
    view! {
        {move || {
            if armed.get() {
                // Signals are `Copy`; only the callbacks need cloning
                // for the closure to stay `Send`.
                let on_confirm = on_confirm.clone();
                view! {
                    <button class="btn btn-error btn-sm" on:click=on_confirm>
                        {move || confirm_label.get()}
                    </button>
                }
                    .into_any()
            } else {
                let on_arm = on_arm.clone();
                view! {
                    <button class="btn btn-ghost btn-sm text-error" on:click=on_arm>
                        {move || label.get()}
                    </button>
                }
                    .into_any()
            }
        }}
    }
}

/// The shared undo banner: an alert-style card (the [`card`] recipe)
/// shown above whatever it undoes while `message` holds a caption,
/// with one localized Undo action. The caller owns the pending state
/// and the caption; the banner owns the 8 s auto-dismiss and re-arms
/// it on every caption change, so a rapid second delete never cuts
/// the first window short. A cleared caption cancels the stale timer,
/// so an early Undo can never fire a late dismissal. Quiet by design:
/// `role=status` + `aria-live=polite`, no animation.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn UndoBanner(
    /// Localized caption for the live undo; `None` renders nothing.
    message: Signal<Option<String>>,
    /// Runs when the user clicks Undo (the caller replays the record).
    on_undo: impl Fn() + Clone + Send + 'static,
    /// Runs when the window lapses (the caller prunes its records and
    /// the banner hides once nothing is live).
    on_dismiss: impl Fn() + Clone + Send + 'static,
) -> impl IntoView {
    let i18n = use_i18n();
    // The auto-dismiss timer lives in a local slot: wasm handles are
    // neither Send nor Sync, and the effect re-arms on every caption
    // change, so a rapid second delete must not cut the first window
    // short. Cancelling is just taking the slot empty (env_section.rs
    // keeps its arming timer the same way).
    let dismiss_slot = StoredValue::new_local(Option::<Timeout>::None);
    let stop_dismiss = move || {
        dismiss_slot.update_value(|slot| {
            if let Some(timer) = slot.take() {
                timer.cancel();
            }
        });
    };
    Effect::new(move |_| {
        stop_dismiss();
        if message.get().is_some() {
            let on_dismiss = on_dismiss.clone();
            let timer = Timeout::new(u32::try_from(UNDO_WINDOW_MS).unwrap_or(0), move || {
                on_dismiss();
            });
            dismiss_slot.set_value(Some(timer));
        }
    });
    on_cleanup(stop_dismiss);
    move || {
        message.get().map(|text| {
            let undo_label = t_string!(i18n, common.undo).into_owned();
            let on_undo = on_undo.clone();
            let banner_class = format!("{} flex flex-wrap items-center gap-2 my-2", card());
            view! {
                <div class=banner_class role="status" aria-live="polite">
                    <span class="min-w-0 flex-1 text-sm">{text}</span>
                    <button
                        class="btn btn-ghost btn-sm min-h-11"
                        type="button"
                        aria-label=undo_label
                        on:click=move |_| on_undo()
                    >
                        {undo_label.clone()}
                    </button>
                </div>
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{PendingUndo, UNDO_WINDOW_MS, card, live_undo, tile, undo_is_live};

    #[test]
    fn card_is_the_full_card_tier_recipe() {
        assert_eq!(card(), "card bg-base-100 shadow-sm px-5 py-4");
    }

    #[test]
    fn tile_is_the_tile_tier_recipe() {
        assert_eq!(
            tile(),
            "rounded-box border border-base-300 bg-base-100 px-4 py-3"
        );
    }

    #[test]
    fn an_undo_is_live_only_inside_its_own_window() {
        let recorded_at_ms = 1_000;
        assert!(undo_is_live(recorded_at_ms, recorded_at_ms, UNDO_WINDOW_MS));
        assert!(undo_is_live(
            recorded_at_ms,
            recorded_at_ms + UNDO_WINDOW_MS - 1,
            UNDO_WINDOW_MS
        ));
        // The window lapsing expires the record, never the clock itself.
        assert!(!undo_is_live(
            recorded_at_ms,
            recorded_at_ms + UNDO_WINDOW_MS,
            UNDO_WINDOW_MS
        ));
    }

    #[test]
    fn the_newest_record_inside_the_window_wins() {
        let pending = vec![
            PendingUndo {
                recorded_at_ms: 1_000,
                payload: "earlier delete",
            },
            PendingUndo {
                recorded_at_ms: 7_500,
                payload: "latest delete",
            },
        ];
        let live = live_undo(&pending, 8_000, UNDO_WINDOW_MS);
        assert_eq!(live.map(|record| record.payload), Some("latest delete"));
    }

    #[test]
    fn an_expired_newest_record_hides_the_banner_entirely() {
        let pending = vec![
            PendingUndo {
                recorded_at_ms: 1_000,
                payload: "earlier delete",
            },
            PendingUndo {
                recorded_at_ms: 7_500,
                payload: "latest delete",
            },
        ];
        // The newest record's window lapsed: nothing is live, even
        // though an older record would still be inside its own window
        // — one banner, last delete wins (docs/DECISIONS.md D126).
        assert!(live_undo(&pending, 7_500 + UNDO_WINDOW_MS, UNDO_WINDOW_MS).is_none());
        assert!(live_undo::<&str>(&[], 0, UNDO_WINDOW_MS).is_none());
    }
}
