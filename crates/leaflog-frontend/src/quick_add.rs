//! Quick add menu: the sixteen-button chip row collapsed into one
//! daisyUI dropdown with Lucide SVG icons (ticket 06, spec 95b2e66).

use crate::grows::empty_action;
use leaflog_core::actions::ActionType;
use leaflog_core::grows::ActionInput;
use leptos::prelude::*;
use leptos_icons::Icon;
use wasm_bindgen::JsCast;

/// The actions the quick-add menu offers: every [`ActionType`] except
/// `PhaseChange` (which only the forecast/phase UIs create), in chip
/// order (spec §4.2).
fn menu_actions() -> Vec<ActionType> {
    ActionType::all()
        .into_iter()
        .filter(|kind| *kind != ActionType::PhaseChange)
        .collect()
}

/// The Lucide icon for a quick-add action. Actions without a fitting
/// glyph in the family render no icon rather than a wrong one
/// (ticket 06); `PhaseChange` never reaches the menu and maps to
/// `None` for the same reason.
fn icon_for(kind: ActionType) -> Option<icondata_core::Icon> {
    let icon = match kind {
        ActionType::Lights => icondata_lu::LuLightbulb,
        ActionType::Water => icondata_lu::LuDroplets,
        ActionType::Feed => icondata_lu::LuFlaskConical,
        ActionType::TrimDefoliate => icondata_lu::LuScissors,
        ActionType::Train => icondata_lu::LuWaypoints,
        ActionType::Transplant => icondata_lu::LuReplace,
        ActionType::Flip => icondata_lu::LuSunMoon,
        ActionType::PestIpm => icondata_lu::LuBug,
        ActionType::EnvChange => icondata_lu::LuFan,
        ActionType::LightChange => icondata_lu::LuSlidersVertical,
        ActionType::Measurement => icondata_lu::LuRuler,
        ActionType::Photo => icondata_lu::LuCamera,
        ActionType::Note => icondata_lu::LuPencil,
        ActionType::TrichomeCheck => icondata_lu::LuZoomIn,
        ActionType::Harvest => icondata_lu::LuWheat,
        ActionType::DryCureCheck => icondata_lu::LuHourglass,
        ActionType::PhaseChange => return None,
    };
    Some(icon)
}

/// Step `current` one slot forward (or back) in a list of `len`
/// items, wrapping around at both ends; an empty list stays at 0.
const fn wrapped_step(len: usize, current: usize, forward: bool) -> usize {
    if len == 0 {
        return 0;
    }
    let len = len - 1; // highest valid index
    if forward {
        if current >= len { 0 } else { current + 1 }
    } else if current == 0 {
        len
    } else {
        current - 1
    }
}

/// The quick-add menu items, or `None` before the menu is in the DOM.
fn menu_items(menu: &impl AsRef<web_sys::Element>) -> Option<web_sys::NodeList> {
    menu.as_ref().query_selector_all("[role=menuitem]").ok()
}

/// Focus the item at `index` (a no-op when it is out of range).
fn focus_item(items: &web_sys::NodeList, index: usize) {
    let index = u32::try_from(index).unwrap_or(u32::MAX);
    if let Some(el) = items
        .item(index)
        .as_ref()
        .and_then(|node| node.dyn_ref::<web_sys::HtmlElement>())
    {
        let _ = el.focus();
    }
}

/// The index of the focused item (0 when nothing in the list has
/// focus, so arrowing from "nowhere" starts at the top).
fn active_index(items: &web_sys::NodeList) -> usize {
    let Some(active) = document().active_element() else {
        return 0;
    };
    let active = active.unchecked_into::<web_sys::Node>();
    (0..items.length())
        .find(|&i| items.get(i).is_some_and(|node| node == active))
        .map_or(0, |i| i as usize)
}

/// Focus the first (`last` false) or last menu item on the next frame,
/// after the open state has rendered (focus on a hidden element is a
/// silent no-op).
fn focus_menu_edge(menu: &NodeRef<leptos::html::Ul>, last: bool) {
    let menu = *menu;
    request_animation_frame(move || {
        if let Some(list) = menu.get_untracked()
            && let Some(items) = menu_items(&list)
        {
            let index = if last {
                items.length().saturating_sub(1) as usize
            } else {
                0
            };
            focus_item(&items, index);
        }
    });
}

/// The sixteen quick-add actions in one daisyUI dropdown (ticket 06):
/// a trigger button with plus and chevron icons, a Lucide SVG per
/// action, and menu-button keyboard semantics (ARIA APG: arrows,
/// Home/End, Escape back to the trigger, Tab-out and outside clicks
/// close). Picking an action opens the inline logging form card with
/// the same draft semantics the chip row had (D069).
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub(crate) fn QuickAddMenu(
    chip: RwSignal<Option<ActionType>>,
    draft: RwSignal<ActionInput>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let root = NodeRef::<leptos::html::Div>::new();
    let trigger = NodeRef::<leptos::html::Button>::new();
    let menu = NodeRef::<leptos::html::Ul>::new();

    // Picking sets the entry's type on a fresh draft and opens the
    // form card — a plain reset left the draft as `note`, so every
    // quick add was logged as one (found by the e2e suite, D069).
    let pick = move |kind: ActionType| {
        let mut reset = empty_action();
        reset.action_type = kind;
        chip.set(Some(kind));
        draft.set(reset);
        open.set(false);
        // The form card just appeared below; hand focus to it so the
        // click that started the flow continues in the form.
        request_animation_frame(move || {
            if let Some(card) = document().get_element_by_id("quick-add-form")
                && let Some(el) = card.dyn_ref::<web_sys::HtmlElement>()
            {
                let _ = el.focus();
            }
        });
    };

    let menu_for_trigger = menu;
    let on_trigger_keydown = move |ev: leptos::ev::KeyboardEvent| match ev.key().as_str() {
        "ArrowDown" => {
            ev.prevent_default();
            open.set(true);
            focus_menu_edge(&menu_for_trigger, false);
        }
        "ArrowUp" => {
            ev.prevent_default();
            open.set(true);
            focus_menu_edge(&menu_for_trigger, true);
        }
        // Enter and Space click the button, which toggles via on:click.
        "Escape" => open.set(false),
        _ => {}
    };

    let on_menu_keydown = move |ev: leptos::ev::KeyboardEvent| {
        let Some(list) = menu.get_untracked() else {
            return;
        };
        let Some(items) = menu_items(&list) else {
            return;
        };
        let len = items.length() as usize;
        match ev.key().as_str() {
            "ArrowDown" => {
                ev.prevent_default();
                focus_item(&items, wrapped_step(len, active_index(&items), true));
            }
            "ArrowUp" => {
                ev.prevent_default();
                focus_item(&items, wrapped_step(len, active_index(&items), false));
            }
            "Home" => {
                ev.prevent_default();
                focus_item(&items, 0);
            }
            "End" => {
                ev.prevent_default();
                focus_item(&items, len.saturating_sub(1));
            }
            "Escape" => {
                open.set(false);
                if let Some(el) = trigger
                    .get_untracked()
                    .as_ref()
                    .and_then(|btn| btn.dyn_ref::<web_sys::HtmlElement>())
                {
                    let _ = el.focus();
                }
            }
            _ => {}
        }
    };

    // A click outside the dropdown closes it (mousedown so the menu is
    // gone before whatever was clicked reacts).
    let handle = window_event_listener(leptos::ev::mousedown, {
        move |ev: web_sys::MouseEvent| {
            let ev = ev.unchecked_ref::<web_sys::Event>();
            let Some(container) = root.get_untracked() else {
                return;
            };
            let container = container.unchecked_into::<web_sys::Element>();
            let outside = ev
                .target()
                .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                .is_none_or(|clicked| {
                    !container.contains(Some(clicked.unchecked_ref::<web_sys::Node>()))
                });
            if outside {
                open.set(false);
            }
        }
    });
    on_cleanup(move || handle.remove());

    view! {
        // Opening is controlled by `dropdown-open` only (see the
        // `.quick-add` override in tailwind.css): daisyUI's
        // :focus-within auto-open would disagree with aria-expanded.
        <div
            node_ref=root
            class=move || if open.get() {
                "dropdown dropdown-open quick-add"
            } else {
                "dropdown quick-add"
            }
            on:focusout=move |ev: leptos::ev::FocusEvent| {
                if let Some(container) = root.get_untracked() {
                    let container = container.unchecked_into::<web_sys::Element>();
                    let left = ev
                        .related_target()
                        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                        .is_none_or(|next| {
                            !container.contains(Some(next.unchecked_ref::<web_sys::Node>()))
                        });
                    if left {
                        open.set(false);
                    }
                }
            }
        >
            <button
                node_ref=trigger
                type="button"
                class="btn btn-primary btn-sm"
                aria-haspopup="menu"
                aria-expanded=move || if open.get() { "true" } else { "false" }
                aria-controls="quick-add-menu"
                on:click=move |_| open.set(!open.get())
                on:keydown=on_trigger_keydown
            >
                <Icon icon=icondata_lu::LuCirclePlus />
                "Quick add"
                <Icon icon=icondata_lu::LuChevronDown />
            </button>
            <ul
                node_ref=menu
                id="quick-add-menu"
                role="menu"
                aria-label="Quick add action"
                class="dropdown-content menu bg-base-100 rounded-box shadow-sm z-1 w-56 p-2"
                on:keydown=on_menu_keydown
            >
                {menu_actions()
                    .into_iter()
                    .map(|kind| {
                        view! {
                            <li role="none">
                                <button type="button" role="menuitem" on:click=move |_| pick(kind)>
                                    {icon_for(kind)
                                        .map(|icon| view! { <Icon icon=icon /> })}
                                    {kind.display()}
                                </button>
                            </li>
                        }
                    })
                    .collect::<Vec<_>>()}
            </ul>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_offers_sixteen_actions_in_chip_order_without_phase_change() {
        let actions = menu_actions();
        let labels: Vec<&str> = actions.iter().map(|kind| kind.label()).collect();
        assert_eq!(
            labels,
            [
                "lights",
                "water",
                "feed",
                "trim_defoliate",
                "train",
                "transplant",
                "flip",
                "pest_ipm",
                "env_change",
                "light_change",
                "measurement",
                "photo",
                "note",
                "trichome_check",
                "harvest",
                "dry_cure_check",
            ],
        );
    }

    #[test]
    fn every_menu_action_has_a_distinct_icon() {
        let mut seen = Vec::new();
        for kind in menu_actions() {
            let icon = icon_for(kind)
                .unwrap_or_else(|| panic!("menu action without icon: {}", kind.label()));
            assert!(!seen.contains(&icon), "duplicate icon for {}", kind.label());
            seen.push(icon);
        }
        assert_eq!(seen.len(), 16);
    }

    #[test]
    fn unmapped_actions_render_no_icon_rather_than_a_wrong_one() {
        assert_eq!(icon_for(ActionType::PhaseChange), None);
    }

    #[test]
    fn wrapped_step_cycles_in_both_directions() {
        assert_eq!(wrapped_step(3, 0, true), 1);
        assert_eq!(wrapped_step(3, 1, true), 2);
        // wraps around at both ends
        assert_eq!(wrapped_step(3, 2, true), 0);
        assert_eq!(wrapped_step(3, 0, false), 2);
        assert_eq!(wrapped_step(3, 2, false), 1);
        // single item stays put
        assert_eq!(wrapped_step(1, 0, true), 0);
        assert_eq!(wrapped_step(1, 0, false), 0);
        // empty menu: no crash, no move
        assert_eq!(wrapped_step(0, 0, true), 0);
        assert_eq!(wrapped_step(0, 0, false), 0);
    }
}
