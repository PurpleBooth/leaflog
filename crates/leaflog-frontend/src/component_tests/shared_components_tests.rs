//! Component tests for the shared form widgets introduced with the
//! component-library start: [`crate::ui::Field`],
//! [`crate::ui::NumberField`] and [`crate::ui::ArmConfirmButton`]
//! (docs/DECISIONS.md "component library"). Each renders in its
//! resting state and its interacting state.

use super::support::*;
use leptos::prelude::*;
use std::sync::Arc as StdArc;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

use crate::ui::{ArmConfirmButton, Field, NumberField};

#[wasm_bindgen_test(async)]
async fn field_wraps_the_control_in_a_fieldset_label() {
    fresh_environment();
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <Field label="Plant width".to_owned()>
                    <input class="input" type="text" />
                </Field>
            }
        })
    });
    let fieldset = mount.q("label.fieldset").expect("fieldset label rendered");
    let label = mount.q("span.fieldset-label").expect("label span");
    assert!(
        label
            .text_content()
            .unwrap_or_default()
            .contains("Plant width"),
        "label text renders above the control"
    );
    // The control is the fieldset's child, directly after the label.
    let control = fieldset.query_selector("input").unwrap();
    assert!(
        control.is_some(),
        "the field's control sits inside the label"
    );
}

#[wasm_bindgen_test(async)]
async fn field_carries_title_and_layout_class_when_set() {
    fresh_environment();
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <Field
                    label="Lights on".to_owned()
                    title=Signal::derive(|| "mock source".to_owned())
                    class="col-span-full"
                >
                    <input class="input" type="time" />
                </Field>
            }
        })
    });
    let fieldset = mount
        .q("label.fieldset.col-span-full")
        .expect("layout class lands on the fieldset");
    assert_eq!(
        fieldset.get_attribute("title"),
        Some("mock source".into()),
        "title signal renders as the tooltip"
    );
}

#[wasm_bindgen_test(async)]
async fn field_without_extras_renders_the_bare_recipe() {
    fresh_environment();
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <Field label="Name".to_owned()>
                    <input class="input" type="text" />
                </Field>
            }
        })
    });
    let fieldset = mount.q("label.fieldset").expect("fieldset rendered");
    assert_eq!(
        fieldset.get_attribute("title"),
        None,
        "no title attribute without the prop"
    );
    assert_eq!(
        fieldset.get_attribute("class"),
        Some("fieldset".into()),
        "class stays the bare recipe without extras"
    );
}

#[wasm_bindgen_test(async)]
async fn number_field_round_trips_typed_values_into_the_draft() {
    fresh_environment();
    let draft = RwSignal::new(Draft::default());
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <NumberField
                    label="Plant width".to_owned()
                    step="0.1"
                    draft=draft
                    read={|d: &Draft| d.width_cm}
                    write={|d: &mut Draft, v: Option<f64>| d.width_cm = v}
                />
            }
        })
    });
    let input = mount
        .q("input[type=number]")
        .expect("number input rendered")
        .unchecked_into::<web_sys::HtmlInputElement>();
    assert_eq!(input.step(), "0.1", "step attribute reaches the DOM");
    assert!(
        input.value().is_empty(),
        "without data the field renders empty, got: {:?}",
        input.value()
    );
    type_input(&input, "12.5");
    assert_eq!(
        draft.get().width_cm,
        Some(12.5),
        "typed value parses into the draft"
    );
    // Clearing the field clears the value, not the draft's other fields.
    type_input(&input, "");
    assert_eq!(
        draft.get().width_cm,
        None,
        "empty input stores None via write"
    );
}

#[wasm_bindgen_test(async)]
async fn number_field_formats_with_display_and_carries_min() {
    fresh_environment();
    let draft = RwSignal::new(Draft {
        width_cm: Some(120.5),
    });
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <NumberField
                    label="Weight".to_owned()
                    step="1"
                    draft=draft
                    read={|d: &Draft| d.width_cm}
                    write={|d: &mut Draft, v: Option<f64>| d.width_cm = v}
                    minimum="0"
                    display=StdArc::new(|value| format!("{value:.0}"))
                />
            }
        })
    });
    let input = mount
        .q("input[type=number]")
        .expect("number input rendered")
        .unchecked_into::<web_sys::HtmlInputElement>();
    assert_eq!(input.min(), "0", "minimum attribute reaches the DOM");
    wait_for(|| !input.value().is_empty()).await;
    // Rust's `{:.0}` rounds half-to-even, so 120.5 formats as "120".
    assert_eq!(input.value(), "120", "display formats the stored value");
}

#[wasm_bindgen_test(async)]
async fn number_field_carries_max_and_placeholder_but_omits_empty_ones() {
    fresh_environment();
    let draft = RwSignal::new(Draft::default());
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <NumberField
                    label="Veg weeks".to_owned()
                    step="1"
                    draft=draft
                    read={|d: &Draft| d.width_cm}
                    write={|d: &mut Draft, v: Option<f64>| d.width_cm = v}
                    minimum="1"
                    maximum="52"
                    placeholder="—"
                />
            }
        })
    });
    let input = mount
        .q("input[type=number]")
        .expect("number input rendered")
        .unchecked_into::<web_sys::HtmlInputElement>();
    assert_eq!(input.min(), "1", "min attribute reaches the DOM");
    assert_eq!(input.max(), "52", "max attribute reaches the DOM");
    assert_eq!(
        input.placeholder(),
        "—",
        "em-dash placeholder reaches the DOM"
    );
    let bare = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <NumberField
                    label="Bare".to_owned()
                    step="1"
                    draft=draft
                    read={|d: &Draft| d.width_cm}
                    write={|d: &mut Draft, v: Option<f64>| d.width_cm = v}
                />
            }
        })
    });
    let bare_input = bare
        .q("input[type=number]")
        .expect("bare input rendered")
        .unchecked_into::<web_sys::HtmlInputElement>();
    assert_eq!(
        bare_input.get_attribute("min"),
        None,
        "empty min renders no attribute"
    );
    assert_eq!(
        bare_input.get_attribute("placeholder"),
        None,
        "empty placeholder renders no attribute"
    );
}

#[wasm_bindgen_test(async)]
async fn arm_confirm_button_walks_arm_then_confirm() {
    fresh_environment();
    let armed = RwSignal::new(false);
    let confirmed = RwSignal::new(false);
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <ArmConfirmButton
                    armed=Signal::derive(move || armed.get())
                    label="Delete".to_owned()
                    confirm_label="Confirm delete".to_owned()
                    on_arm=move |_| armed.set(true)
                    on_confirm=move |_| confirmed.set(true)
                />
            }
        })
    });
    let quiet = mount
        .q("button.btn-ghost")
        .expect("quiet button while unarmed");
    assert!(
        quiet.text_content().unwrap_or_default().contains("Delete"),
        "unarmed caption is the quiet label"
    );
    let quiet_btn = quiet.unchecked_into::<web_sys::HtmlButtonElement>();
    click(&quiet_btn);
    wait_for(|| mount.text().contains("Confirm delete")).await;
    assert!(
        mount.q("button.btn-error").is_some(),
        "armed state swaps to the loud confirm button"
    );
    let loud = mount
        .q("button.btn-error")
        .expect("confirm button")
        .unchecked_into::<web_sys::HtmlButtonElement>();
    click(&loud);
    wait_for(|| confirmed.get()).await;
}

#[wasm_bindgen_test(async)]
async fn arm_confirm_button_stays_quiet_while_unarmed() {
    fresh_environment();
    let armed = RwSignal::new(false);
    let mount = Mounted::new(move || {
        with_i18n(move || {
            view! {
                <ArmConfirmButton
                    armed=Signal::derive(move || armed.get())
                    label="Delete".to_owned()
                    confirm_label="Confirm delete".to_owned()
                    on_arm={|_| ()}
                    on_confirm={|_| ()}
                />
            }
        })
    });
    assert!(
        mount.q("button.btn-ghost").is_some(),
        "unarmed renders the ghost button"
    );
    assert!(
        mount.q("button.btn-error").is_none(),
        "no confirm button before arming"
    );
}

/// Local draft shape mirroring the callers' structs (e.g.
/// `ActionInput`): plain `Option<f64>` fields behind read/write
/// closures.
#[derive(Clone, Debug, Default)]
struct Draft {
    width_cm: Option<f64>,
}
