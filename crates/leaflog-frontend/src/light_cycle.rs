//! Light Cycle editor on the grow overview (ticket 05): the daily
//! lights-on local time plus hours-on per lit phase. Empty fields
//! keep the cited preset defaults (shown as placeholders, photoperiod
//! aware); saving stores the override, reset clears it back to the
//! defaults. Saving stores the override, reset clears it back to the
//! defaults. All user-visible strings come from the locale catalogs
//! (localisation ticket 03; D112).

use leptos::prelude::*;
use leptos::task::spawn_local;

use leaflog_core::Phase;
use leaflog_core::grows::PhotoperiodType;
use leaflog_core::light_cycle::{
    LIT_PHASES, LightCycle, LightCycleInput, LightCycleResponse, ObservedReality, PhaseHoursInput,
    default_hours_on,
};

use crate::api;
use crate::i18n::{Locale, t, t_string, use_i18n};
use crate::labels::phase_label;
use crate::ui::{Field, PendingUndo, UNDO_WINDOW_MS, UndoBanner, card, live_undo, now_ms};
use leptos_i18n::td_string;

/// The four phase hour fields as raw input text, keyed like
/// [`PhaseHoursInput`].
#[derive(Debug, Clone, Default, PartialEq)]
struct HoursText {
    germination: String,
    seedling: String,
    vegetative: String,
    flower: String,
}

impl HoursText {
    /// One phase's raw text (blank for the unlit phases).
    fn get(&self, phase: Phase) -> String {
        match phase {
            Phase::Germination => self.germination.clone(),
            Phase::Seedling => self.seedling.clone(),
            Phase::Vegetative => self.vegetative.clone(),
            Phase::Flower => self.flower.clone(),
            Phase::Drying | Phase::Curing => String::new(),
        }
    }

    /// Writes one phase's raw text (no-op for the unlit phases).
    fn set(&mut self, phase: Phase, text: String) {
        match phase {
            Phase::Germination => self.germination = text,
            Phase::Seedling => self.seedling = text,
            Phase::Vegetative => self.vegetative = text,
            Phase::Flower => self.flower = text,
            Phase::Drying | Phase::Curing => {}
        }
    }
}

/// Assembles the wire payload from the editor's raw text fields. A
/// blank field stays unset (`None`); text that is not a whole number
/// is an error naming the fix (range checks stay with the API, whose
/// messages name the phase and the accepted range).
fn build_input(
    locale: Locale,
    lights_on_text: &str,
    hours: &HoursText,
) -> Result<LightCycleInput, String> {
    let hour_field = |text: &str| -> Result<Option<i64>, String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        trimmed
            .parse::<i64>()
            .map(Some)
            .map_err(|_| td_string!(locale, light_cycle.hours_whole).into_owned())
    };
    Ok(LightCycleInput {
        lights_on: (!lights_on_text.trim().is_empty()).then(|| lights_on_text.trim().to_owned()),
        hours_on: PhaseHoursInput {
            germination: hour_field(&hours.germination)?,
            seedling: hour_field(&hours.seedling)?,
            vegetative: hour_field(&hours.vegetative)?,
            flower: hour_field(&hours.flower)?,
        },
    })
}

/// The raw text fields for a stored cycle: its lights-on string and
/// per-phase hours, blanks where unset.
fn fields_of(cycle: &LightCycle) -> (String, HoursText) {
    let hours = HoursText {
        germination: optional_field(cycle.hours_on.germination),
        seedling: optional_field(cycle.hours_on.seedling),
        vegetative: optional_field(cycle.hours_on.vegetative),
        flower: optional_field(cycle.hours_on.flower),
    };
    (
        cycle
            .lights_on
            .map_or_else(String::new, |time| time.to_string()),
        hours,
    )
}

/// A stored hour as input text (blank when unset).
fn optional_field(hours: Option<u8>) -> String {
    hours.map_or_else(String::new, |hours| hours.to_string())
}

/// The one-line reality caption for a bound light entity (grow-page
/// rework, ticket 03): names the entity and the pattern learned from
/// it, so the defaults' override status is visible. Display only —
/// the learned values never reach the editable fields (ADR 0002).
fn reality_caption(locale: Locale, observed: &ObservedReality) -> String {
    td_string!(
        locale,
        light_cycle.reality,
        entity = &observed.entity_id,
        lights_on = &observed.cycle.lights_on,
        hours = observed.cycle.hours_on
    )
    .into_owned()
}

/// The Light Cycle editor form: one lights-on time input plus
/// hours-on per lit phase. Placeholders show the photoperiod-aware preset
/// defaults; reset clears the whole override. When a light entity is
/// bound, a caption reports the pattern learned from its history
/// (D102); `bindings_epoch` bumps when the entity bindings save, so
/// the caption re-derives without a page reload.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn LightCycleEditor(
    grow_id: Signal<String>,
    photoperiod: Signal<PhotoperiodType>,
    bindings_epoch: Signal<u64>,
) -> impl IntoView {
    let i18n = use_i18n();
    let locale = i18n.get_locale();
    let lights_on = RwSignal::<String>::new(String::new());
    let hours = RwSignal::<HoursText>::new(HoursText::default());
    let stored = RwSignal::<bool>::new(false);
    let status = RwSignal::<Option<String>>::new(None);
    let observed = RwSignal::<Option<ObservedReality>>::new(None);

    // Loads whatever is stored (the endpoint answers the empty cycle
    // when nothing is, so blanks always mean "on the default").
    let reload = move || {
        let id = grow_id.get();
        spawn_local(async move {
            match api::get::<LightCycleResponse>(&format!("api/grows/{id}/light-cycle")).await {
                Ok(response) => {
                    let (time, fields) = fields_of(&response.cycle);
                    stored.set(response.cycle.has_override());
                    observed.set(response.observed);
                    lights_on.set(time);
                    hours.set(fields);
                }
                Err(failure) => status.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
            }
        });
    };
    Effect::new(move |_| {
        // A binding change re-derives the observed reality (D102):
        // tracking the epoch reloads alongside the grow id.
        bindings_epoch.get();
        reload();
    });

    // prevent_default first — see docs/DECISIONS.md D061.
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let built = build_input(locale, &lights_on.get(), &hours.get());
        match built {
            Ok(input) => {
                let id = grow_id.get();
                spawn_local(async move {
                    match api::put::<LightCycleInput, LightCycle>(
                        &format!("api/grows/{id}/light-cycle"),
                        &input,
                    )
                    .await
                    {
                        Ok(cycle) => {
                            let (time, fields) = fields_of(&cycle);
                            stored.set(true);
                            lights_on.set(time);
                            hours.set(fields);
                            status.set(Some(t_string!(i18n, light_cycle.saved).into_owned()));
                        }
                        Err(failure) => status.set(Some(api::failure_text(
                            &failure,
                            i18n.get_locale_untracked(),
                        ))),
                    }
                });
            }
            Err(message) => status.set(Some(message)),
        }
    };

    // Undoable reset (D126): the values on screen before the reset are
    // captured first, so one Undo re-PUTs them through the ordinary
    // save endpoint. Same one-banner, last-delete-wins rule as the
    // timeline: dismissing, undoing or lapsing clears the stack.
    let undo_pending = RwSignal::<Vec<PendingUndo<(String, HoursText)>>>::new(Vec::new());
    let undo_message = RwSignal::<Option<String>>::new(None);
    let dismiss_undo = move || {
        undo_pending.set(Vec::new());
        undo_message.set(None);
    };
    let undo_reset = move || {
        let records = undo_pending.get();
        let Some(record) = live_undo(&records, now_ms(), UNDO_WINDOW_MS).cloned() else {
            // The window lapsed between the timer firing and the
            // click — the same as dismissing.
            dismiss_undo();
            return;
        };
        dismiss_undo();
        let (time, fields) = record.payload;
        let built = build_input(locale, &time, &fields);
        match built {
            Ok(input) => {
                let id = grow_id.get();
                spawn_local(async move {
                    match api::put::<LightCycleInput, LightCycle>(
                        &format!("api/grows/{id}/light-cycle"),
                        &input,
                    )
                    .await
                    {
                        Ok(cycle) => {
                            let (time, fields) = fields_of(&cycle);
                            stored.set(true);
                            lights_on.set(time);
                            hours.set(fields);
                            status.set(Some(t_string!(i18n, light_cycle.saved).into_owned()));
                        }
                        Err(failure) => status.set(Some(api::failure_text(
                            &failure,
                            i18n.get_locale_untracked(),
                        ))),
                    }
                });
            }
            Err(message) => status.set(Some(message)),
        }
    };

    let reset = move |_: leptos::ev::MouseEvent| {
        let id = grow_id.get();
        // Capture what the form shows now: undo restores exactly that,
        // through the same save endpoint reset replaces.
        let previous = (lights_on.get_untracked(), hours.get_untracked());
        undo_pending.update(|records| {
            records.push(PendingUndo {
                recorded_at_ms: now_ms(),
                payload: previous.clone(),
            });
        });
        spawn_local(async move {
            match api::delete(&format!("api/grows/{id}/light-cycle")).await {
                Ok(()) => {
                    stored.set(false);
                    lights_on.set(String::new());
                    hours.set(HoursText::default());
                    status.set(None);
                    undo_message.set(Some(t_string!(i18n, light_cycle.reset_done).into_owned()));
                }
                Err(failure) => {
                    // Nothing was reset: the capture must not offer an
                    // undo of a no-op. Remove exactly this reset's
                    // record, not whichever sits on top — two resets
                    // can be in flight at once (reviewer P2, follows
                    // the same payload-keyed rule as grows.rs).
                    undo_pending.update(|records| {
                        if let Some(pos) =
                            records.iter().position(|record| record.payload == previous)
                        {
                            records.remove(pos);
                        }
                    });
                    status.set(Some(api::failure_text(
                        &failure,
                        i18n.get_locale_untracked(),
                    )));
                }
            }
        });
    };

    view! {
        <div class={format!("{} my-4", card())}>
            <h3 class="mt-0">{t!(i18n, light_cycle.heading)}</h3>
            <p class="text-base-content/70 text-sm">
                {t!(i18n, light_cycle.intro)}
            </p>
            {move || {
                observed.get().map(|reality| {
                    view! {
                        <p class="text-base-content/70 text-sm" data-testid="light-reality">
                            {reality_caption(locale, &reality)}
                        </p>
                    }
                })
            }}
            <UndoBanner message=undo_message.into() on_undo=undo_reset on_dismiss=dismiss_undo />
            <form class="flex flex-wrap gap-2 items-end my-2" on:submit=save>
            <Field label=t_string!(i18n, light_cycle.lights_on).into_owned()>
                <input class="input input-sm" type="time" placeholder="06:00"
                    prop:value=move || lights_on.get()
                    on:input=move |ev| lights_on.set(event_target_value(&ev))
                />
            </Field>
            {LIT_PHASES
                .iter()
                .map(|phase| {
                    let phase = *phase;
                    view! {
                        <Field
                            label=td_string!(locale, light_cycle.hours_on, phase = phase_label(locale, phase)).into_owned()
                            title=Signal::derive(move || {
                                default_hours_on(phase, photoperiod.get())
                                    .map_or_else(String::new, |default| default.source.to_owned())
                            })
                        >
                            <input class="input input-sm w-24" type="number"
                                prop:placeholder=move || {
                                    default_hours_on(phase, photoperiod.get())
                                        .map_or_else(String::new, |default| {
                                            default.hours.to_string()
                                        })
                                }
                                prop:value=move || hours.get().get(phase)
                                on:input=move |ev| {
                                    let text = event_target_value(&ev);
                                    hours.update(|fields| fields.set(phase, text));
                                }
                            />
                        </Field>
                    }
                })
                .collect::<Vec<_>>()}
            <button class="btn btn-sm btn-primary self-end" type="submit">{t!(i18n, light_cycle.apply)}</button>
            <Show when=move || stored.get() fallback=|| ()>
                <button class="btn btn-sm btn-ghost self-end" type="button" on:click=reset>
                    {t!(i18n, light_cycle.reset)}
                </button>
            </Show>
            {move || {
                status.get().map(|message| {
                    view! { <span class="text-base-content/60 text-sm">{message}</span> }
                })
            }}
            </form>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{HoursText, build_input, fields_of};
    use crate::i18n::Locale;
    use leaflog_core::light_cycle::{ClockTime, LightCycle, PhaseHours};

    #[test]
    fn blank_fields_stay_unset_and_numbers_become_the_payload() {
        let hours = HoursText {
            germination: String::new(),
            seedling: "  ".to_owned(),
            vegetative: "20".to_owned(),
            flower: "24".to_owned(),
        };
        let input = build_input(Locale::en, "07:30", &hours).unwrap();
        assert_eq!(input.lights_on.as_deref(), Some("07:30"));
        assert_eq!(input.hours_on.germination, None);
        assert_eq!(input.hours_on.seedling, None);
        assert_eq!(input.hours_on.vegetative, Some(20));
        assert_eq!(input.hours_on.flower, Some(24));
    }

    #[test]
    fn non_whole_hours_are_rejected_without_a_roundtrip() {
        let hours = HoursText {
            flower: "12.5".to_owned(),
            ..HoursText::default()
        };
        let error = build_input(Locale::en, "06:00", &hours).unwrap_err();
        assert_eq!(error, "Hours on must be whole numbers.");

        let letters = HoursText {
            vegetative: "many".to_owned(),
            ..HoursText::default()
        };
        assert!(build_input(Locale::en, "", &letters).is_err());
    }

    #[test]
    fn an_all_blank_form_sends_the_empty_cycle() {
        let input = build_input(Locale::en, "", &HoursText::default()).unwrap();
        assert_eq!(input.lights_on, None);
        let cycle = input.validate().unwrap();
        assert_eq!(cycle, LightCycle::default());
    }

    #[test]
    fn stored_cycles_load_back_into_the_fields() {
        let cycle = LightCycle {
            lights_on: Some(ClockTime {
                hour: 21,
                minute: 15,
            }),
            hours_on: PhaseHours {
                flower: Some(11),
                ..PhaseHours::default()
            },
        };
        let (time, fields) = fields_of(&cycle);
        assert_eq!(time, "21:15");
        assert_eq!(fields.flower, "11");
        assert_eq!(fields.vegetative, "");
    }

    #[test]
    fn the_caption_names_the_entity_and_its_learned_pattern() {
        let reality = leaflog_core::light_cycle::ObservedReality {
            entity_id: "switch.grow_led".to_owned(),
            cycle: leaflog_core::light_cycle::ObservedCycle {
                lights_on: ClockTime { hour: 6, minute: 0 },
                hours_on: 18,
            },
        };
        assert_eq!(
            super::reality_caption(Locale::en, &reality),
            "Reality from: switch.grow_led · learned 06:00 · 18 h"
        );
    }
}
