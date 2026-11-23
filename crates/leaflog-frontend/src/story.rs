//! The grow story: local days, plant filters and independent photographs.

use crate::{
    ErrorIf, api,
    i18n::{Locale, t, t_string, td_string, use_i18n},
    labels, ui,
};
use leaflog_core::actions::ActionType;
use leaflog_core::grows::{ActionEntry, Photo, Plant};
use leaflog_core::story::{Period, StoryDay, StoryPage};
use leptos::{prelude::*, task::spawn_local};
use leptos_router::components::A;
use wasm_bindgen::JsCast;

/// The day-card diary. Plant choices also seed the next quick-add draft.
#[component]
#[allow(clippy::too_many_lines)] // Declarative day-card composition; see docs/DECISIONS.md.
pub fn Story(
    grow_id: Signal<String>,
    plants: Signal<Vec<Plant>>,
    selected_plants: RwSignal<Vec<String>>,
    timezone: RwSignal<String>,
    revision: Signal<u64>,
    on_add: Callback<(String, ActionType)>,
    on_delete: Callback<ActionEntry>,
) -> impl IntoView {
    let i18n = use_i18n();
    let pages = RwSignal::<Vec<StoryDay>>::new(Vec::new());
    let cursor = RwSignal::<Option<String>>::new(None);
    let loading = RwSignal::new(false);
    let error = RwSignal::<Option<String>>::new(None);
    let armed_photo = RwSignal::<Option<String>>::new(None);
    let loaded = RwSignal::new(false);
    Effect::new(move |_| {
        let id = grow_id.get();
        revision.track();
        pages.set(Vec::new());
        cursor.set(None);
        loaded.set(false);
        loading.set(true);
        spawn_local(async move {
            match api::get::<StoryPage>(&format!("api/grows/{id}/story")).await {
                Ok(page) => {
                    timezone.set(page.timezone);
                    pages.set(page.days);
                    cursor.set(page.before);
                }
                Err(failure) => error.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
            }
            loading.set(false);
            loaded.set(true);
        });
    });
    let older = move |_| {
        let Some(before) = cursor.get_untracked() else {
            return;
        };
        let id = grow_id.get_untracked();
        loading.set(true);
        spawn_local(async move {
            match api::get::<StoryPage>(&format!("api/grows/{id}/story?before={before}")).await {
                Ok(page) => {
                    timezone.set(page.timezone);
                    pages.update(|days| days.extend(page.days));
                    cursor.set(page.before);
                    error.set(None);
                }
                Err(failure) => error.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
            }
            loading.set(false);
        });
    };
    let add_photo = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let form = event_target::<web_sys::HtmlFormElement>(&ev);
        let Ok(data) = web_sys::FormData::new_with_form(&form) else {
            return;
        };
        let Some(file) = data.get("file").dyn_into::<web_sys::File>().ok() else {
            return;
        };
        let id = grow_id.get_untracked();
        loading.set(true);
        spawn_local(async move {
            let data = web_sys::FormData::new().expect("form data constructs");
            data.append_with_blob("file", &file)
                .expect("file is a blob");
            // Per-photo metadata keeps separate photographs independent,
            // including two photographs with different plant selections.
            for value in form_values(&form, "plant_id") {
                data.append_with_str("plant_id", &value)
                    .expect("string field");
            }
            if let Some(at) = form_values(&form, "captured_at").first() {
                data.append_with_str("captured_at", at)
                    .expect("string field");
            }
            let result = api::post_form(&format!("api/grows/{id}/photos"), &data).await;
            loading.set(false);
            match result {
                Ok(()) => {
                    form.reset();
                    // A completed upload closes its day-local form;
                    // the next photo starts with fresh metadata.
                    if let Some(details) =
                        form.closest("details").ok().flatten().and_then(|element| {
                            element.dyn_into::<web_sys::HtmlDetailsElement>().ok()
                        })
                    {
                        details.set_open(false);
                    }
                    error.set(None);
                    // A capture date can put the photo in an already-loaded
                    // older day, so reload the story rather than guessing.
                    reload_story(grow_id.get_untracked(), pages, cursor, timezone, error);
                }
                Err(failure) => error.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
            }
        });
    };
    let delete_photo = move |id: String| {
        armed_photo.set(None);
        spawn_local(async move {
            match api::delete(&format!("api/photos/{id}")).await {
                Ok(()) => {
                    pages.update(|days| {
                        for day in days {
                            day.photos.retain(|photo| photo.id != id);
                        }
                    });
                    error.set(None);
                }
                Err(failure) => error.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
            }
        });
    };
    view! {
        <section aria-labelledby="story-heading">
            <div class="flex flex-wrap items-center justify-between gap-3">
                <h3 id="story-heading" class="m-0">{t!(i18n, story.heading)}</h3>
                <button class="btn btn-sm" type="button" on:click=move |_| on_add.run((String::new(), ActionType::Note))>
                    {t!(i18n, story.add_entry)}
                </button>
            </div>
            <fieldset class="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1 border-y border-base-300 py-2">
                <legend class="sr-only">{t!(i18n, story.filter)}</legend>
                <span class="text-sm font-medium">{t!(i18n, grow.plants)}</span>
                <label class="flex min-h-11 items-center gap-2 text-sm">
                    <input type="checkbox" class="checkbox checkbox-sm" prop:checked=move || selected_plants.get().is_empty()
                        on:change=move |_| selected_plants.set(Vec::new()) />
                    {t!(i18n, story.all_plants)}
                </label>
                <For each=move || plants.get() key=|plant| plant.id.clone() let:plant>
                    {{
                        let id = plant.id.clone();
                        let checked_id = id.clone();
                        view! {
                            <span class="flex min-h-11 items-center gap-2 text-sm">
                                <input aria-label=plant.name.clone() type="checkbox" class="checkbox checkbox-sm"
                                    prop:checked=move || selected_plants.get().contains(&checked_id)
                                    on:change=move |_| selected_plants.update(|selected| toggle(selected, &id)) />
                                <A href=crate::route_href(&format!("/plants/{}", plant.id)) attr:class="link link-hover py-3">{plant.name.clone()}</A>
                            </span>
                        }
                    }}
                </For>
                <button class="btn btn-ghost btn-sm" type="button" on:click=move |_| on_add.run((String::new(), ActionType::Photo))>
                    {t!(i18n, story.add_photo)}
                </button>
            </fieldset>
            <ErrorIf error />
            <Show when=move || loaded.get() && visible_days(&pages.get(), &selected_plants.get()).is_empty() && !loading.get()>
                <ui::EmptyState>{t!(i18n, story.empty)}</ui::EmptyState>
            </Show>
            <div class="mt-6 grid gap-8">
                <For each=move || visible_days(&pages.get(), &selected_plants.get()) key=|day| day.date.clone() let:day>
                    <DayCard day plants selected_plants timezone=timezone.into() on_add on_delete armed_photo delete_photo add_photo />
                </For>
            </div>
            <Show when=move || loading.get()><p role="status">{t!(i18n, page.loading)}</p></Show>
            <Show when=move || cursor.get().is_some()>
                <button class="btn btn-outline mt-6 w-full" type="button" disabled=move || loading.get() on:click=older>
                    {t!(i18n, story.load_older)}
                </button>
            </Show>
        </section>
    }
}

#[component]
#[allow(clippy::too_many_lines)] // A single day's semantic layout, not shared logic.
fn DayCard(
    day: StoryDay,
    plants: Signal<Vec<Plant>>,
    selected_plants: RwSignal<Vec<String>>,
    timezone: Signal<String>,
    on_add: Callback<(String, ActionType)>,
    on_delete: Callback<ActionEntry>,
    armed_photo: RwSignal<Option<String>>,
    delete_photo: impl Fn(String) + Copy + Send + Sync + 'static,
    add_photo: impl Fn(leptos::ev::SubmitEvent) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let i18n = use_i18n();
    let locale = i18n.get_locale_untracked();
    let date = day.date.clone();
    let date_for_add = date.clone();
    let day_label = local_date(&date, locale);
    let light = day.light_hours.map(|hours| format!("{hours:.1}"));
    // One caveat per day, not one per half of it.
    let sparse = day.periods.iter().any(|period| period.sparse);
    let periods = day.periods;
    let photos = day.photos.clone();
    view! {
        <section class="border-t-2 border-primary/50 pt-3" data-testid="story-day">
            <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
                <h4 class="m-0 text-xl font-semibold">{day_label}</h4>
                <span class="text-sm text-base-content/70">{td_string!(locale, grow.day, day = day.grow_day.to_string())}</span>
                <ui::Chip class="badge-sm">{labels::phase_label(locale, day.phase)}</ui::Chip>
                <span class="ml-auto flex gap-1">
                    <button class="btn btn-ghost btn-sm" type="button" aria-label={td_string!(locale, story.add_photo).into_owned()}
                        on:click=move |_| on_add.run((date_for_add.clone(), ActionType::Photo))><leptos_icons::Icon icon=icondata_lu::LuCamera /></button>
                    <button class="btn btn-ghost btn-sm" type="button" aria-label={td_string!(locale, story.add_entry).into_owned()}
                        on:click=move |_| on_add.run((date.clone(), ActionType::Note))><leptos_icons::Icon icon=icondata_lu::LuPencil /></button>
                </span>
            </div>
            <div class="mt-3 grid gap-x-6 gap-y-2 border-y border-base-300 py-3 sm:grid-cols-2" data-testid="story-environment">
                {periods.into_iter().enumerate().map(|(index, period)| view! {
                    <PeriodLine locale period on=index == 0 />
                }).collect::<Vec<_>>()}
            </div>
            <p class="my-2 text-sm text-base-content/70">
                {light.map_or_else(|| td_string!(locale, story.no_light).into_owned(), |hours| td_string!(locale, story.light_hours, hours = hours).into_owned())}
                {day.light_partial.then(|| view! { <span>{" · "}{t!(i18n, story.partial)}</span> })}
            </p>
            {sparse.then(|| view! { <p class="my-2 text-sm text-base-content/70">{t!(i18n, story.sparse)}</p> })}
            {(!day.has_observations).then(|| view! { <p class="text-sm text-base-content/70">{t!(i18n, story.no_data)}</p> })}
            {day.automatic.then(|| view! { <p class="text-sm text-base-content/70">{t!(i18n, story.automatic)}</p> })}
            <ul class="m-0 grid list-none gap-2 p-0">
                {day.actions.into_iter().map(|entry| crate::grows::timeline_entry_in_zone(locale, entry, move |entry| on_delete.run(entry), timezone.get_untracked())).collect::<Vec<_>>()}
            </ul>
            {(!photos.is_empty()).then(|| view! {
                <ul class="m-0 mt-4 grid list-none grid-cols-[repeat(auto-fill,minmax(8rem,1fr))] gap-3 p-0">
                    {photos.into_iter().map(|photo| view! { <PhotoCard photo plants timezone armed_photo delete_photo /> }).collect::<Vec<_>>()}
                </ul>
            })}
            <details class="mt-3">
                <summary class="cursor-pointer text-sm font-medium">{t!(i18n, story.add_photo)}</summary>
                <form class="mt-3 grid gap-3 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] sm:items-end" on:submit=add_photo>
                    <ui::Field label=Signal::derive(move || t_string!(i18n, form.photo_files).into_owned())>
                        <input class="file-input file-input-sm w-full" name="file" type="file" required accept="image/jpeg,image/png,image/webp" />
                    </ui::Field>
                    <ui::Field label=Signal::derive(move || t_string!(i18n, story.captured_at).into_owned())>
                        <input class="input input-sm" name="captured_at" type="datetime-local" value=format!("{}T12:00", day.date) />
                    </ui::Field>
                    <fieldset class="col-span-full flex flex-wrap gap-3">
                        <legend class="text-sm">{t!(i18n, story.photo_plants)}</legend>
                        <For each=move || plants.get() key=|plant| plant.id.clone() let:plant>
                            <label class="flex min-h-11 items-center gap-2 text-sm">
                                <input class="checkbox checkbox-sm" type="checkbox" name="plant_id" value=plant.id.clone()
                                    checked=selected_plants.get_untracked().contains(&plant.id) />
                                {plant.name.clone()}
                            </label>
                        </For>
                    </fieldset>
                    <button class="btn btn-primary btn-sm sm:col-start-3 sm:row-start-1" type="submit">{t!(i18n, story.upload)}</button>
                </form>
            </details>
        </section>
    }
}

#[component]
fn PeriodLine(locale: Locale, period: Period, on: bool) -> impl IntoView {
    let style = crate::decimal_style(locale);
    let units = crate::presentation_units();
    let temperature = move || {
        period.temperature.map_or_else(
            || "—".to_owned(),
            |value| leaflog_core::units::temp_preferred(value, units.get(), style),
        )
    };
    let humidity = period.humidity.map_or_else(
        || "—".to_owned(),
        |value| crate::chart::format_pct(value, style),
    );
    let vpd = period.vpd.map_or_else(
        || "—".to_owned(),
        |value| crate::chart::format_kpa(value, style),
    );
    let heading = if on {
        td_string!(locale, story.day_period).into_owned()
    } else {
        td_string!(locale, story.night_period).into_owned()
    };
    view! {
        <div>
            <h5 class="m-0 text-sm font-semibold">{heading}</h5>
            <p class="my-1 text-sm"><span>{temperature}</span>{" · "}<span>{humidity}</span>{" · VPD "}<span>{vpd}</span></p>
        </div>
    }
}

#[component]
pub(crate) fn PhotoCard(
    photo: Photo,
    plants: Signal<Vec<Plant>>,
    timezone: Signal<String>,
    armed_photo: RwSignal<Option<String>>,
    delete_photo: impl Fn(String) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let i18n = use_i18n();
    let id = photo.id.clone();
    let delete_id = id.clone();
    let plant_ids = photo.plant_ids.clone();
    let captured_at = photo.captured_at.clone();
    let caption = Signal::derive(move || {
        let plants = plants.get();
        let names = plant_ids
            .iter()
            .filter_map(|id| plants.iter().find(|plant| &plant.id == id))
            .map(|plant| plant.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{} · {}",
            stamp(&captured_at, &timezone.get(), i18n.get_locale()),
            if names.is_empty() {
                t_string!(i18n, story.whole_grow).into_owned()
            } else {
                names
            }
        )
    });
    let label = Signal::derive(move || t_string!(i18n, entries.delete).into_owned());
    let confirm = Signal::derive(move || t_string!(i18n, schedules.confirm_delete).into_owned());
    let armed = Signal::derive(move || armed_photo.get().as_deref() == Some(&id));
    view! {
        <li>
            // The link says what it does; the thumbnail is its preview
            // and the caption below carries the date once, not three
            // times (impeccable audit).
            <a href=format!("api/photos/{}/original", photo.id) target="_blank" rel="noreferrer"
                class="block overflow-hidden rounded-lg border border-base-300"
                aria-label=t_string!(i18n, story.open_original).into_owned()
            >
                <img class="aspect-square w-full object-cover" loading="lazy" src=format!("api/photos/{}/thumbnail", photo.id) alt="" />
            </a>
            <p class="my-1 text-xs text-base-content/70">{caption}</p>
            <ui::ArmConfirmButton armed label confirm_label=confirm
                on_arm=move |_| armed_photo.set(Some(delete_id.clone()))
                on_confirm=move |_| delete_photo(photo.id.clone()) />
        </li>
    }
}

fn reload_story(
    id: String,
    pages: RwSignal<Vec<StoryDay>>,
    cursor: RwSignal<Option<String>>,
    timezone: RwSignal<String>,
    error: RwSignal<Option<String>>,
) {
    spawn_local(async move {
        match api::get::<StoryPage>(&format!("api/grows/{id}/story")).await {
            Ok(page) => {
                timezone.set(page.timezone);
                pages.set(page.days);
                cursor.set(page.before);
            }
            Err(failure) => error.set(Some(failure.message.unwrap_or_default())),
        }
    });
}

fn form_values(form: &web_sys::HtmlFormElement, name: &str) -> Vec<String> {
    web_sys::FormData::new_with_form(form)
        .ok()
        .map_or_else(Vec::new, |data| {
            data.get_all(name)
                .iter()
                .filter_map(|value| value.as_string())
                .collect()
        })
}

fn visible_days(days: &[StoryDay], plants: &[String]) -> Vec<StoryDay> {
    days.iter()
        .filter_map(|day| {
            let matches = |ids: &[String]| {
                plants.is_empty() || ids.is_empty() || ids.iter().any(|id| plants.contains(id))
            };
            let actions = day
                .actions
                .iter()
                .filter(|entry| matches(&entry.plant_ids))
                .cloned()
                .collect::<Vec<_>>();
            let photos = day
                .photos
                .iter()
                .filter(|photo| matches(&photo.plant_ids))
                .cloned()
                .collect::<Vec<_>>();
            // Grow-wide environment is context, not a plant-specific match.
            (!actions.is_empty()
                || !photos.is_empty()
                || day.has_observations
                || day.automatic
                || plants.is_empty())
            .then(|| StoryDay {
                actions,
                photos,
                ..day.clone()
            })
        })
        .collect()
}

fn toggle(ids: &mut Vec<String>, id: &str) {
    if let Some(index) = ids.iter().position(|current| current == id) {
        ids.remove(index);
    } else {
        ids.push(id.to_owned());
    }
}

fn local_date(date: &str, locale: Locale) -> String {
    format_stamp(&format!("{date}T12:00:00Z"), "UTC", locale, true)
}

fn stamp(iso: &str, timezone: &str, locale: Locale) -> String {
    // Date captions are intentionally independent of the browser's zone.
    format_stamp(iso, timezone, locale, false)
}

fn format_stamp(iso: &str, timezone: &str, locale: Locale, date_only: bool) -> String {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(iso));
    if date.get_time().is_nan() {
        return iso.to_owned();
    }
    let Ok(fmt) = js_sys::Reflect::get(
        &date,
        &wasm_bindgen::JsValue::from_str(if date_only {
            "toLocaleDateString"
        } else {
            "toLocaleString"
        }),
    )
    .and_then(wasm_bindgen::JsValue::dyn_into::<js_sys::Function>) else {
        return iso.to_owned();
    };
    let options = js_sys::Object::new();
    for (key, value) in [
        ("timeZone", timezone),
        ("year", "numeric"),
        ("month", "short"),
        ("day", "numeric"),
    ] {
        let _ = js_sys::Reflect::set(&options, &key.into(), &value.into());
    }
    if !date_only {
        for (key, value) in [("hour", "numeric"), ("minute", "2-digit")] {
            let _ = js_sys::Reflect::set(&options, &key.into(), &value.into());
        }
    }
    fmt.call2(
        &date,
        &match locale {
            Locale::de => "de",
            Locale::en => "en",
        }
        .into(),
        &options,
    )
    .ok()
    .and_then(|value| value.as_string())
    .unwrap_or_else(|| iso.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{toggle, visible_days};
    use leaflog_core::{
        actions::{ActionDetails, ActionType},
        grows::{ActionEntry, Photo},
        story::StoryDay,
    };

    fn day() -> StoryDay {
        serde_json::from_str(r#"{"date":"2026-11-22","grow_day":1,"phase":"vegetative","actions":[],"photos":[],"automatic":false,"has_observations":false,"light_hours":null,"light_partial":false,"periods":[{"sparse":false},{"sparse":false}]}"#).expect("test day")
    }

    #[test]
    fn plant_toggles_are_independent() {
        let mut ids = Vec::new();
        toggle(&mut ids, "a");
        toggle(&mut ids, "b");
        toggle(&mut ids, "a");
        assert_eq!(ids, ["b"]);
    }

    #[test]
    fn no_filter_shows_plant_and_grow_wide_photos() {
        let mut day = day();
        day.photos = vec![
            serde_json::from_str::<Photo>(r#"{"id":"plant-photo","grow_id":"g1","plant_ids":["plant-1"],"captured_at":"2026-11-22T12:00:00Z","grow_day":1,"tags":[],"original_name":"a.jpg","thumbnail_name":"a.webp","bytes_stored":1,"created_at":"2026-11-22T12:00:00Z","updated_at":"2026-11-22T12:00:00Z"}"#).expect("plant photo"),
            serde_json::from_str::<Photo>(r#"{"id":"grow-photo","grow_id":"g1","plant_ids":[],"captured_at":"2026-11-22T12:00:00Z","grow_day":1,"tags":[],"original_name":"b.jpg","thumbnail_name":"b.webp","bytes_stored":1,"created_at":"2026-11-22T12:00:00Z","updated_at":"2026-11-22T12:00:00Z"}"#).expect("grow photo"),
        ];
        let days = visible_days(&[day], &[]);
        assert_eq!(days[0].photos.len(), 2);
    }

    #[test]
    fn selected_plants_keep_grow_wide_and_matching_entries() {
        let mut day = day();
        day.actions = ["plant-1", ""]
            .map(|plant| ActionEntry {
                id: format!("note-{plant}"),
                grow_id: "g1".to_owned(),
                plant_ids: (!plant.is_empty())
                    .then(|| plant.to_owned())
                    .into_iter()
                    .collect(),
                occurred_at: "2026-11-22T12:00:00Z".to_owned(),
                action_type: ActionType::Note,
                phase: None,
                grow_day: 1,
                details: ActionDetails::default(),
                notes: Some("test note".to_owned()),
                appearance_tags: Vec::new(),
                created_at: "2026-11-22T12:00:00Z".to_owned(),
                updated_at: "2026-11-22T12:00:00Z".to_owned(),
            })
            .to_vec();
        day.photos = vec![serde_json::from_str::<Photo>(r#"{"id":"plant-photo","grow_id":"g1","plant_ids":["plant-1"],"captured_at":"2026-11-22T12:00:00Z","grow_day":1,"tags":[],"original_name":"a.jpg","thumbnail_name":"a.webp","bytes_stored":1,"created_at":"2026-11-22T12:00:00Z","updated_at":"2026-11-22T12:00:00Z"}"#).expect("plant photo")];
        let days = visible_days(&[day], &["plant-1".to_owned()]);
        assert_eq!(days[0].actions.len(), 2);
        assert_eq!(days[0].photos.len(), 1);
    }

    #[test]
    fn grow_wide_environment_remains_under_a_plant_filter() {
        let mut day = day();
        day.has_observations = true;
        let days = visible_days(&[day], &["plant-1".to_owned()]);
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].date, "2026-11-22");
    }

    #[test]
    fn selected_plants_hide_other_plant_entries() {
        let mut day = day();
        day.photos = vec![serde_json::from_str::<Photo>(r#"{"id":"other-photo","grow_id":"g1","plant_ids":["plant-2"],"captured_at":"2026-11-22T12:00:00Z","grow_day":1,"tags":[],"original_name":"a.jpg","thumbnail_name":"a.webp","bytes_stored":1,"created_at":"2026-11-22T12:00:00Z","updated_at":"2026-11-22T12:00:00Z"}"#).expect("other plant photo")];
        assert_eq!(visible_days(&[day], &["plant-1".to_owned()]).len(), 0);
    }
}
