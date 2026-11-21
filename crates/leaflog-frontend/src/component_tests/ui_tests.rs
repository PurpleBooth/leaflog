//! Component tests for the shared primitives in [`crate::ui`] and
//! [`crate::ErrorIf`]: rendering with and without data, and the
//! interactions each primitive owns.

use super::support::*;
use leptos::prelude::*;
use wasm_bindgen_test::*;

use crate::ui::{EmptyState, SectionHeader, UndoBanner};

#[wasm_bindgen_test(async)]
async fn empty_state_renders_children_centered() {
    fresh_environment();
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <EmptyState><span>"Nothing logged yet."</span></EmptyState>
            }
        })
    });
    let state = mount.q("p.text-center").expect("empty state paragraph");
    assert!(
        classes(&state).contains("text-base-content/60"),
        "empty state uses the muted text recipe, got: {}",
        classes(&state)
    );
    assert!(
        state
            .text_content()
            .unwrap_or_default()
            .contains("Nothing logged yet.")
    );
}

#[wasm_bindgen_test(async)]
async fn section_header_renders_the_title() {
    fresh_environment();
    let title = RwSignal::new("Water".to_string());
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <SectionHeader title=title.into() />
            }
        })
    });
    let header = mount.q("h4").expect("section header h4");
    assert!(header.text_content().unwrap_or_default().contains("Water"));
}

#[wasm_bindgen_test(async)]
async fn section_header_tracks_signal_updates() {
    fresh_environment();
    let title = RwSignal::new("Water".to_string());
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <SectionHeader title=title.into() />
            }
        })
    });
    assert!(mount.contains("Water"));
    title.set("Lights".to_string());
    // Leptos flushes reactive effects on a microtask, so the DOM
    // update lands after the current task; poll instead of asserting
    // synchronously.
    wait_for(|| mount.contains("Lights")).await;
    assert!(
        mount.contains("Lights"),
        "header re-renders on signal change"
    );
}

#[wasm_bindgen_test(async)]
async fn undo_banner_is_hidden_without_a_message() {
    fresh_environment();
    let message: RwSignal<Option<String>> = RwSignal::new(None);
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <UndoBanner
                    message=message.into()
                    on_undo=move || {}
                    on_dismiss=move || {}
                />
            }
        })
    });
    assert!(
        mount.q("[role='status']").is_none(),
        "no banner without a message"
    );
    assert!(
        mount.q("button").is_none(),
        "no action buttons without a message"
    );
}

#[wasm_bindgen_test(async)]
async fn undo_banner_shows_message_with_undo_and_dismiss() {
    fresh_environment();
    let message: RwSignal<Option<String>> = RwSignal::new(Some("Entry deleted.".to_string()));
    let undo_count = RwSignal::new(0_u32);
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <UndoBanner
                    message=message.into()
                    on_undo=move || undo_count.set(undo_count.get_untracked() + 1)
                    on_dismiss=move || message.set(None)
                />
            }
        })
    });
    let banner = mount.q("[role='status']").expect("banner rendered");
    assert!(
        banner
            .text_content()
            .unwrap_or_default()
            .contains("Entry deleted."),
        "banner shows the message"
    );
    // UndoBanner owns exactly one button (Undo); dismissal is the
    // 8 s auto-timer plus the caller's on_dismiss, not a button.
    let buttons = mount.q_all("button");
    assert_eq!(buttons.len(), 1, "single Undo button");
    click(&buttons[0]);
    assert_eq!(
        undo_count.get_untracked(),
        1,
        "undo button fires the handler"
    );
}

#[wasm_bindgen_test(async)]
async fn undo_banner_hides_when_message_clears() {
    fresh_environment();
    let message: RwSignal<Option<String>> = RwSignal::new(Some("Entry deleted.".to_string()));
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <UndoBanner
                    message=message.into()
                    on_undo=move || {}
                    on_dismiss=move || message.set(None)
                />
            }
        })
    });
    assert!(mount.q("[role='status']").is_some());
    message.set(None);
    wait_for(|| mount.q("[role='status']").is_none()).await;
    assert!(
        mount.q("[role='status']").is_none(),
        "banner disappears once cleared"
    );
}

#[wasm_bindgen_test(async)]
async fn undo_banner_replaces_a_new_message() {
    fresh_environment();
    let message: RwSignal<Option<String>> = RwSignal::new(Some("First.".to_string()));
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <UndoBanner
                    message=message.into()
                    on_undo=move || {}
                    on_dismiss=move || message.set(None)
                />
            }
        })
    });
    message.set(Some("Second.".to_string()));
    wait_text(&mount, "Second.").await;
    assert!(mount.contains("Second."), "banner shows the newest message");
}

// --- ErrorIf (lib.rs) ------------------------------------------------

#[wasm_bindgen_test(async)]
async fn error_if_is_hidden_without_an_error() {
    fresh_environment();
    let error: RwSignal<Option<String>> = RwSignal::new(None);
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <crate::ErrorIf error=error />
            }
        })
    });
    assert!(
        mount.q("[role='alert']").is_none(),
        "no alert without an error"
    );
    assert!(mount.text().trim().is_empty(), "renders nothing");
}

#[wasm_bindgen_test(async)]
async fn error_if_shows_the_message() {
    fresh_environment();
    let error: RwSignal<Option<String>> = RwSignal::new(Some("backend unreachable".to_string()));
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <crate::ErrorIf error=error />
            }
        })
    });
    let alert = mount.q("[role='alert']").expect("alert rendered");
    assert!(
        classes(&alert).contains("alert-error"),
        "alert uses the error recipe, got: {}",
        classes(&alert)
    );
    assert!(
        alert
            .text_content()
            .unwrap_or_default()
            .contains("backend unreachable")
    );
}

#[wasm_bindgen_test(async)]
async fn error_if_dismiss_clears_the_error() {
    fresh_environment();
    let error: RwSignal<Option<String>> = RwSignal::new(Some("backend unreachable".to_string()));
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <crate::ErrorIf error=error />
            }
        })
    });
    let buttons = mount.q_all("button");
    assert!(buttons.len() >= 2, "retry + dismiss buttons present");
    // The last button is dismiss (retry comes first in the markup).
    click(buttons.last().expect("dismiss button"));
    // Poll the DOM (not the signal): the view update is flushed on a
    // microtask after the click handler runs.
    wait_for(|| mount.q("[role='alert']").is_none()).await;
    assert!(
        mount.q("[role='alert']").is_none(),
        "alert disappears after dismiss"
    );
    assert!(
        error.get_untracked().is_none(),
        "dismiss clears the error signal"
    );
}

#[wasm_bindgen_test(async)]
async fn error_if_offers_retry() {
    fresh_environment();
    let error: RwSignal<Option<String>> = RwSignal::new(Some("backend unreachable".to_string()));
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <crate::ErrorIf error=error />
            }
        })
    });
    // The retry button reloads the page, so it is asserted, not
    // clicked (a reload would tear down the test runner).
    assert!(
        mount
            .q_all("button")
            .iter()
            .any(|b| b.get_attribute("aria-label").is_some()),
        "retry button carries an accessible label"
    );
}
