//! Localised display labels for core domain enums (loc ticket 02).
//!
//! Pure mapping helpers: a [`Locale`] in, a catalog-backed label out.
//! Call sites read the locale reactively — `i18n.get_locale()` inside
//! a render closure or `td_string!` through the context — so a locale
//! swap re-renders. Native tests pin the mappings via `Locale::en`
//! (which must reproduce the core `display()` spellings) and
//! `Locale::de`.

use crate::i18n::Locale;
use leaflog_core::Phase;
use leaflog_core::actions::ActionType;
use leaflog_core::appearance;
use leaflog_core::grows::GrowStatus;
use leptos_i18n::td_string;

/// The display label for a grow phase in the given locale.
#[must_use]
pub fn phase_label(locale: Locale, phase: Phase) -> String {
    match phase {
        Phase::Germination => td_string!(locale, phases.germination).to_owned(),
        Phase::Seedling => td_string!(locale, phases.seedling).to_owned(),
        Phase::Vegetative => td_string!(locale, phases.vegetative).to_owned(),
        Phase::Flower => td_string!(locale, phases.flower).to_owned(),
        Phase::Drying => td_string!(locale, phases.drying).to_owned(),
        Phase::Curing => td_string!(locale, phases.curing).to_owned(),
    }
}

/// The short action caption for the given locale (same wording
/// contract as the core `ActionType::display()` spellings).
#[must_use]
pub fn action_label(locale: Locale, kind: ActionType) -> String {
    match kind {
        ActionType::Lights => td_string!(locale, actions.lights).to_owned(),
        ActionType::Water => td_string!(locale, actions.water).to_owned(),
        ActionType::Feed => td_string!(locale, actions.feed).to_owned(),
        ActionType::TrimDefoliate => td_string!(locale, actions.trim_defoliate).to_owned(),
        ActionType::Train => td_string!(locale, actions.train).to_owned(),
        ActionType::Transplant => td_string!(locale, actions.transplant).to_owned(),
        ActionType::Flip => td_string!(locale, actions.flip).to_owned(),
        ActionType::PhaseChange => td_string!(locale, actions.phase_change).to_owned(),
        ActionType::PestIpm => td_string!(locale, actions.pest_ipm).to_owned(),
        ActionType::EnvChange => td_string!(locale, actions.env_change).to_owned(),
        ActionType::LightChange => td_string!(locale, actions.light_change).to_owned(),
        ActionType::Measurement => td_string!(locale, actions.measurement).to_owned(),
        ActionType::Photo => td_string!(locale, actions.photo).to_owned(),
        ActionType::Note => td_string!(locale, actions.note).to_owned(),
        ActionType::TrichomeCheck => td_string!(locale, actions.trichome_check).to_owned(),
        ActionType::Harvest => td_string!(locale, actions.harvest).to_owned(),
        ActionType::DryCureCheck => td_string!(locale, actions.dry_cure_check).to_owned(),
    }
}

/// The action caption for a stored wire label (e.g. from a timeline
/// entry): the catalog label for known kinds, the raw label for
/// unknown ones.
#[must_use]
pub fn action_label_for_kind(locale: Locale, kind: &str) -> String {
    ActionType::all()
        .iter()
        .find(|known| known.label() == kind)
        .map_or_else(|| kind.to_owned(), |&known| action_label(locale, known))
}

/// How an appearance tag renders in the given locale: the catalog
/// label for canonical tags (old spellings under the merged entry's
/// label) and the raw string for unknown free-form tags.
#[must_use]
pub fn appearance_label(locale: Locale, tag: &str) -> String {
    match appearance::canonical_tag(tag) {
        "healthy" => td_string!(locale, appearance.healthy).to_owned(),
        "praying" => td_string!(locale, appearance.praying).to_owned(),
        "dark_green" => td_string!(locale, appearance.dark_green).to_owned(),
        "light_green" => td_string!(locale, appearance.light_green).to_owned(),
        "yellowing" => td_string!(locale, appearance.yellowing).to_owned(),
        "purple_stems" => td_string!(locale, appearance.purple_stems).to_owned(),
        "canoeing" => td_string!(locale, appearance.canoeing).to_owned(),
        "bleaching" => td_string!(locale, appearance.bleaching).to_owned(),
        "droop" => td_string!(locale, appearance.droop).to_owned(),
        "tip_burn" => td_string!(locale, appearance.tip_burn).to_owned(),
        "nutrient_burn" => td_string!(locale, appearance.nutrient_burn).to_owned(),
        "fox_tailing" => td_string!(locale, appearance.fox_tailing).to_owned(),
        "stretchy" => td_string!(locale, appearance.stretchy).to_owned(),
        "short" => td_string!(locale, appearance.short).to_owned(),
        other => other.to_owned(),
    }
}

/// The grow status caption for the given locale.
#[must_use]
pub fn status_label(locale: Locale, status: GrowStatus) -> String {
    match status {
        GrowStatus::Active => td_string!(locale, status.active).to_owned(),
        GrowStatus::Archived => td_string!(locale, status.archived).to_owned(),
    }
}

/// The appearance category heading for the given locale (the group
/// caption above one set of trait checkboxes).
#[must_use]
pub fn appearance_category_label(locale: Locale, category: appearance::Category) -> String {
    match category {
        appearance::Category::OverallHealth => {
            td_string!(locale, form.cat_overall_health).to_owned()
        }
        appearance::Category::Colour => td_string!(locale, form.cat_colour).to_owned(),
        appearance::Category::LightStress => td_string!(locale, form.cat_light_stress).to_owned(),
        appearance::Category::NutrientAndWater => {
            td_string!(locale, form.cat_nutrient_water).to_owned()
        }
        appearance::Category::GrowthShape => td_string!(locale, form.cat_growth_shape).to_owned(),
    }
}

/// The feed template phase name for a `FeedPhase` wire label; the
/// 8 known labels map to catalog strings, unknown ones echo through
/// (matching the backend's tolerant rendering of stray phases).
#[must_use]
pub fn feed_phase_label(locale: Locale, label: &str) -> String {
    match label {
        "seedling" => td_string!(locale, feed.phase_seedling).to_owned(),
        "early_veg" => td_string!(locale, feed.phase_early_veg).to_owned(),
        "late_veg" => td_string!(locale, feed.phase_late_veg).to_owned(),
        "early_flower" => td_string!(locale, feed.phase_early_flower).to_owned(),
        "mid_flower" => td_string!(locale, feed.phase_mid_flower).to_owned(),
        "late_flower" => td_string!(locale, feed.phase_late_flower).to_owned(),
        "ripen" => td_string!(locale, feed.phase_ripen).to_owned(),
        "flush" => td_string!(locale, feed.phase_flush).to_owned(),
        other => other.to_owned(),
    }
}

/// The chart card title for a history role in the given locale; the
/// titles double as the not-measured card's chip labels, so both go
/// through this one mapping. Unknown roles echo through.
#[must_use]
pub fn chart_title(locale: Locale, role: &str) -> String {
    match role {
        "temperature" => td_string!(locale, env.chart_temperature).to_owned(),
        "humidity" => td_string!(locale, env.chart_humidity).to_owned(),
        "vpd" => td_string!(locale, env.chart_vpd).to_owned(),
        "co2" => td_string!(locale, env.chart_co2).to_owned(),
        "soil_moisture" => td_string!(locale, env.chart_soil_moisture).to_owned(),
        "ph" => td_string!(locale, env.chart_ph).to_owned(),
        "ec" => td_string!(locale, env.chart_ec).to_owned(),
        other => other.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Locale, action_label, action_label_for_kind, appearance_category_label, appearance_label,
        chart_title, feed_phase_label, phase_label, status_label,
    };
    use leaflog_core::Phase;
    use leaflog_core::actions::ActionType;
    use leaflog_core::grows::GrowStatus;

    /// English reproduces the core spellings one-to-one: the charts,
    /// timeline and pickers showed these exact strings before keying.
    #[test]
    fn english_matches_the_core_spellings() {
        for phase in Phase::all() {
            assert_eq!(
                phase_label(Locale::en, phase),
                crate::phase_pick::phase_display(phase)
            );
        }
        for kind in ActionType::all() {
            assert_eq!(action_label(Locale::en, kind), kind.display());
        }
        assert_eq!(status_label(Locale::en, GrowStatus::Active), "Active");
        assert_eq!(status_label(Locale::en, GrowStatus::Archived), "Archived");
    }

    /// The German catalog is reachable through the same helpers.
    #[test]
    fn german_labels_render() {
        assert_eq!(phase_label(Locale::de, Phase::Flower), "Blüte");
        assert_eq!(phase_label(Locale::de, Phase::Drying), "Trocknung");
        assert_eq!(action_label(Locale::de, ActionType::Water), "Gießen");
        assert_eq!(
            action_label(Locale::de, ActionType::TrichomeCheck),
            "Trichome"
        );
        assert_eq!(status_label(Locale::de, GrowStatus::Active), "Aktiv");
    }

    /// Old appearance spellings render under the merged entry; free
    /// form tags echo through in both locales.
    #[test]
    fn appearance_tags_map_and_echo() {
        assert_eq!(appearance_label(Locale::en, "canoe"), "Canoeing");
        assert_eq!(appearance_label(Locale::de, "canoeing"), "Kahnblätter");
        assert_eq!(appearance_label(Locale::en, "frost bite"), "frost bite");
        assert_eq!(appearance_label(Locale::de, "frost bite"), "frost bite");
    }

    /// Chart titles spell the card headings (and chip labels); roles
    /// outside the known set echo through.
    #[test]
    fn chart_titles_cover_every_role() {
        assert_eq!(chart_title(Locale::en, "temperature"), "Temperature");
        assert_eq!(chart_title(Locale::de, "soil_moisture"), "Substratfeuchte");
        assert_eq!(chart_title(Locale::en, "mystery"), "mystery");
    }

    /// Stored action labels render localised; unknown ones echo.
    #[test]
    fn action_kind_labels_map_and_echo() {
        assert_eq!(action_label_for_kind(Locale::en, "trim_defoliate"), "Trim");
        assert_eq!(action_label_for_kind(Locale::de, "flip"), "Flip 12/12");
        assert_eq!(
            action_label_for_kind(Locale::en, "seeds popped"),
            "seeds popped"
        );
    }

    /// The appearance category headings spell out in both locales.
    /// Feed phase names: English matches the core wire spellings
    /// spelled out; German renders the horticulture terms; unknown
    /// labels echo through.
    #[test]
    fn feed_phase_labels_map_and_echo() {
        assert_eq!(feed_phase_label(Locale::en, "seedling"), "Seedling");
        assert_eq!(feed_phase_label(Locale::en, "late_flower"), "Late flower");
        assert_eq!(feed_phase_label(Locale::de, "seedling"), "Sämling");
        assert_eq!(feed_phase_label(Locale::de, "flush"), "Wässern");
        assert_eq!(
            feed_phase_label(Locale::en, "something_else"),
            "something_else"
        );
    }

    #[test]
    fn appearance_categories_map() {
        use leaflog_core::appearance::Category;
        assert_eq!(
            appearance_category_label(Locale::en, Category::OverallHealth),
            "Overall health"
        );
        assert_eq!(
            appearance_category_label(Locale::de, Category::NutrientAndWater),
            "Düngung & Wasser"
        );
    }
}
