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

/// A bound entity id chip: the id is data, not copy — it renders
/// verbatim as one ellipsized line (`.entity-id` truncates in CSS,
/// so the DOM text stays complete for screen readers and tests) and
/// carries the full id on the title tooltip.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn EntityIdChip(entity_id: String) -> impl IntoView {
    let title = entity_id.clone();
    view! {
        <span class="badge badge-ghost entity-id" title=title>
            <span class="entity-id-text">{entity_id}</span>
        </span>
    }
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
/// quiet uppercase register, above its content, with an optional
/// legend hint beside it (the Air group names its night shading
/// there). The title and hint arrive as signals so a locale swap
/// re-translates them.
#[component]
#[allow(clippy::must_use_candidate)] // #[component] macro drops #[must_use]; see docs/DECISIONS.md
pub fn SectionHeader(
    title: Signal<String>,
    #[prop(default = None)] hint: Option<Signal<String>>,
) -> impl IntoView {
    view! {
        <div class="flex flex-wrap items-baseline gap-2 mt-4">
            <h4 class="m-0 text-xs font-semibold uppercase tracking-wider text-base-content/60">
                {move || title.get()}
            </h4>
            {hint.map(|hint| {
                view! { <span class="text-xs text-base-content/50">{move || hint.get()}</span> }
            })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{card, tile};

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
}
