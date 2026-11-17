//! Light Cycle editor on the grow overview (ticket 05): the daily
//! lights-on local time plus hours-on per lit phase. Empty fields
//! keep the cited preset defaults (shown as placeholders, photoperiod
//! aware); saving stores the override, reset clears it back to the
//! defaults. The strings are plain English like every other form —
//! no locale catalog exists yet (see docs/DECISIONS.md).

use leptos::prelude::*;
use leptos::task::spawn_local;

use leaflog_core::Phase;
use leaflog_core::grows::PhotoperiodType;
use leaflog_core::light_cycle::{
    LIT_PHASES, LightCycle, LightCycleInput, PhaseHoursInput, default_hours_on,
};

use crate::api;
use crate::phase_pick::phase_display;

/// The four phase hour fields as raw input text, keyed like
/// [`PhaseHoursInput`].
#[derive(Debug, Clone, Default)]
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
fn build_input(lights_on_text: &str, hours: &HoursText) -> Result<LightCycleInput, String> {
    let hour_field = |text: &str| -> Result<Option<i64>, String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        trimmed
            .parse::<i64>()
            .map(Some)
            .map_err(|_| "Hours on must be whole numbers.".to_owned())
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

/// The Light Cycle editor form: one lights-on time input plus
/// hours-on per lit phase. Placeholders show the photoperiod-aware preset
/// defaults; reset clears the whole override.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn LightCycleEditor(
    grow_id: Signal<String>,
    photoperiod: Signal<PhotoperiodType>,
) -> impl IntoView {
    let lights_on = RwSignal::<String>::new(String::new());
    let hours = RwSignal::<HoursText>::new(HoursText::default());
    let stored = RwSignal::<bool>::new(false);
    let status = RwSignal::<Option<String>>::new(None);

    // Loads whatever is stored (the endpoint answers the empty cycle
    // when nothing is, so blanks always mean "on the default").
    let reload = move || {
        let id = grow_id.get();
        spawn_local(async move {
            match api::get::<LightCycle>(&format!("api/grows/{id}/light-cycle")).await {
                Ok(cycle) => {
                    let (time, fields) = fields_of(&cycle);
                    stored.set(cycle.has_override());
                    lights_on.set(time);
                    hours.set(fields);
                }
                Err(failure) => status.set(Some(failure.message)),
            }
        });
    };
    Effect::new(move |_| {
        reload();
    });

    // prevent_default first — see docs/DECISIONS.md D061.
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let built = build_input(&lights_on.get(), &hours.get());
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
                            status.set(Some("Saved.".to_owned()));
                        }
                        Err(failure) => status.set(Some(failure.message)),
                    }
                });
            }
            Err(message) => status.set(Some(message)),
        }
    };

    let reset = move |_: leptos::ev::MouseEvent| {
        let id = grow_id.get();
        spawn_local(async move {
            match api::delete(&format!("api/grows/{id}/light-cycle")).await {
                Ok(()) => {
                    stored.set(false);
                    lights_on.set(String::new());
                    hours.set(HoursText::default());
                    status.set(Some("Reset to default.".to_owned()));
                }
                Err(failure) => status.set(Some(failure.message)),
            }
        });
    };

    view! {
        <h3>"Light cycle"</h3>
        <p class="text-base-content/70 text-sm">
            "Empty fields use the cited preset defaults; hover a field for its source."
        </p>
        <form class="flex flex-wrap gap-2 items-end my-2" on:submit=save>
            <label class="fieldset">
                <span class="fieldset-label">"Lights on time (local)"</span>
                <input class="input input-sm" type="time" placeholder="06:00"
                    prop:value=move || lights_on.get()
                    on:input=move |ev| lights_on.set(event_target_value(&ev))
                />
            </label>
            {LIT_PHASES
                .iter()
                .map(|phase| {
                    let phase = *phase;
                    view! {
                        <label
                            class="fieldset"
                            title=move || {
                                default_hours_on(phase, photoperiod.get())
                                    .map_or_else(String::new, |default| default.source.to_owned())
                            }
                        >
                            <span class="fieldset-label">
                                {phase_display(phase)}" hours on"
                            </span>
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
                        </label>
                    }
                })
                .collect::<Vec<_>>()}
            <button class="btn btn-sm btn-primary self-end" type="submit">"Apply cycle"</button>
            <Show when=move || stored.get() fallback=|| ()>
                <button class="btn btn-sm btn-ghost self-end" type="button" on:click=reset>
                    "Reset to default"
                </button>
            </Show>
            {move || {
                status.get().map(|message| {
                    view! { <span class="text-base-content/60 text-sm">{message}</span> }
                })
            }}
        </form>
    }
}

#[cfg(test)]
mod tests {
    use super::{HoursText, build_input, fields_of};
    use leaflog_core::light_cycle::{ClockTime, LightCycle, PhaseHours};

    #[test]
    fn blank_fields_stay_unset_and_numbers_become_the_payload() {
        let hours = HoursText {
            germination: String::new(),
            seedling: "  ".to_owned(),
            vegetative: "20".to_owned(),
            flower: "24".to_owned(),
        };
        let input = build_input("07:30", &hours).unwrap();
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
        let error = build_input("06:00", &hours).unwrap_err();
        assert_eq!(error, "Hours on must be whole numbers.");

        let letters = HoursText {
            vegetative: "many".to_owned(),
            ..HoursText::default()
        };
        assert!(build_input("", &letters).is_err());
    }

    #[test]
    fn an_all_blank_form_sends_the_empty_cycle() {
        let input = build_input("", &HoursText::default()).unwrap();
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
}
