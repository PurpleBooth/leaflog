//! The appearance vocabulary: fourteen canonical traits in five
//! groups, each with a display label and a one-line meaning.
//!
//! Stored entries keep raw tag strings, so rendering goes through
//! [`display_label`]: canonical tags show their label, the old
//! `taco`/`canoe` spellings show the merged entry's label, and any
//! unknown free-form tag renders as typed.

/// One of the five appearance groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    /// General plant condition.
    OverallHealth,
    /// Leaf and stem colour.
    Colour,
    /// Too much light (or heat).
    LightStress,
    /// Feeding and watering problems.
    NutrientAndWater,
    /// How the plant grows.
    GrowthShape,
}

impl Category {
    /// All categories in form order.
    #[must_use]
    pub const fn all() -> [Self; 5] {
        [
            Self::OverallHealth,
            Self::Colour,
            Self::LightStress,
            Self::NutrientAndWater,
            Self::GrowthShape,
        ]
    }

    /// The display name shown above the group's checkboxes.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OverallHealth => "Overall health",
            Self::Colour => "Colour",
            Self::LightStress => "Light stress",
            Self::NutrientAndWater => "Nutrient & water",
            Self::GrowthShape => "Growth shape",
        }
    }
}

/// One canonical appearance trait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trait {
    /// The stable `snake_case` tag stored in the database.
    pub tag: &'static str,
    /// The short display label shown in the form and on badges.
    pub label: &'static str,
    /// The one-line meaning shown as help text.
    pub meaning: &'static str,
    /// The group this trait belongs to.
    pub category: Category,
}

/// All canonical traits, grouped by category in [`Category::all`]
/// order.
pub const TRAITS: &[Trait] = &[
    Trait {
        tag: "healthy",
        label: "Healthy",
        meaning: "Normal growth and colour. Nothing to fix.",
        category: Category::OverallHealth,
    },
    Trait {
        tag: "praying",
        label: "Praying",
        meaning: "Leaves tilt upward. Usually a good sign of strong growth.",
        category: Category::OverallHealth,
    },
    Trait {
        tag: "dark_green",
        label: "Dark green",
        meaning: "Leaves darker than normal. Often too much nitrogen.",
        category: Category::Colour,
    },
    Trait {
        tag: "light_green",
        label: "Light green",
        meaning: "Pale leaves. Often a mild nutrient shortage.",
        category: Category::Colour,
    },
    Trait {
        tag: "yellowing",
        label: "Yellowing",
        meaning: "Yellow leaves. Check nutrients and pH.",
        category: Category::Colour,
    },
    Trait {
        tag: "purple_stems",
        label: "Purple stems",
        meaning: "Purple stems. Often genetic, sometimes a phosphorus shortage.",
        category: Category::Colour,
    },
    Trait {
        tag: "canoeing",
        label: "Canoeing",
        meaning: "Leaves fold along the middle. Usually light or heat stress.",
        category: Category::LightStress,
    },
    Trait {
        tag: "bleaching",
        label: "Bleaching",
        meaning: "White or pale patches near the lamp. Too much light.",
        category: Category::LightStress,
    },
    Trait {
        tag: "droop",
        label: "Droop",
        meaning: "Limp, hanging leaves. Check watering first.",
        category: Category::NutrientAndWater,
    },
    Trait {
        tag: "tip_burn",
        label: "Tip burn",
        meaning: "Brown burnt leaf tips. Often a high nutrient dose.",
        category: Category::NutrientAndWater,
    },
    Trait {
        tag: "nutrient_burn",
        label: "Nutrient burn",
        meaning: "Burnt leaf edges from too many nutrients.",
        category: Category::NutrientAndWater,
    },
    Trait {
        tag: "fox_tailing",
        label: "Fox tailing",
        meaning: "Sparse bud spires. Often heat or light stress in flower.",
        category: Category::GrowthShape,
    },
    Trait {
        tag: "stretchy",
        label: "Stretchy",
        meaning: "Fast stem growth with long gaps between nodes.",
        category: Category::GrowthShape,
    },
    Trait {
        tag: "short",
        label: "Short",
        meaning: "Compact growth with tight node spacing.",
        category: Category::GrowthShape,
    },
];

/// The canonical tag a stored tag stands for: the old `taco` and
/// `canoe` spellings map to the merged `canoeing`; everything else
/// (including unknown free-form tags) stays as typed.
#[must_use]
pub fn canonical_tag(tag: &str) -> &str {
    match tag {
        "taco" | "canoe" => "canoeing",
        other => other,
    }
}

/// How a stored tag renders in the interface: the display label for
/// canonical tags (old spellings under the merged entry's label) and
/// the raw string for unknown free-form tags.
#[must_use]
pub fn display_label(tag: &str) -> std::borrow::Cow<'static, str> {
    let canonical = canonical_tag(tag);
    TRAITS.iter().find(|t| t.tag == canonical).map_or_else(
        || std::borrow::Cow::Owned(tag.to_owned()),
        |found| std::borrow::Cow::Borrowed(found.label),
    )
}

/// Whether a stored appearance tag means stretch (the lamp advisor
/// raises the lamp when such a tag is logged).
#[must_use]
pub fn is_stretch_tag(tag: &str) -> bool {
    canonical_tag(tag) == "stretchy"
}

/// Whether a stored appearance tag means light stress (bleaching or
/// canoeing, including the old `taco`/`canoe` spellings — the lamp
/// advisor dims or raises when such a tag is logged).
#[must_use]
pub fn is_light_stress_tag(tag: &str) -> bool {
    matches!(canonical_tag(tag), "bleaching" | "canoeing")
}

#[cfg(test)]
mod tests {
    use super::{Category, TRAITS};

    #[test]
    fn fourteen_traits_across_five_categories() {
        assert_eq!(TRAITS.len(), 14, "the vocabulary has fourteen traits");
        for category in Category::all() {
            assert!(
                TRAITS.iter().any(|t| t.category == category),
                "{category:?} has no traits"
            );
        }
        assert!(
            TRAITS.iter().all(|t| Category::all().contains(&t.category)),
            "every trait sits in a known category"
        );
    }

    #[test]
    fn every_trait_has_label_meaning_category() {
        for t in TRAITS {
            assert!(!t.label.is_empty(), "{} has no label", t.tag);
            assert!(!t.meaning.is_empty(), "{} has no meaning", t.tag);
            assert_ne!(t.label, t.tag, "{} must not show its raw tag", t.tag);
        }
    }

    #[test]
    fn tags_are_unique() {
        let mut tags: Vec<&str> = TRAITS.iter().map(|t| t.tag).collect();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(tags.len(), TRAITS.len(), "trait tags must be unique");
    }

    #[test]
    fn taco_and_canoe_gone_canoeing_present() {
        assert!(
            TRAITS.iter().all(|t| t.tag != "taco"),
            "taco must not be canonical"
        );
        assert!(
            TRAITS.iter().all(|t| t.tag != "canoe"),
            "canoe must not be canonical"
        );
        assert!(
            TRAITS.iter().any(|t| t.tag == "canoeing"),
            "the merged canoeing entry must be canonical"
        );
    }

    #[test]
    fn praying_means_a_good_sign() {
        let praying = TRAITS
            .iter()
            .find(|t| t.tag == "praying")
            .expect("praying is listed");
        assert!(
            praying.meaning.contains("good sign"),
            "praying meaning must say it is usually a good sign: {}",
            praying.meaning
        );
    }

    #[test]
    fn canonical_tag_maps_old_spellings_to_canoeing() {
        assert_eq!(super::canonical_tag("taco"), "canoeing");
        assert_eq!(super::canonical_tag("canoe"), "canoeing");
        assert_eq!(super::canonical_tag("canoeing"), "canoeing");
        // Canonical and free-form tags pass through untouched.
        assert_eq!(super::canonical_tag("yellowing"), "yellowing");
        assert_eq!(super::canonical_tag("weird_freeform"), "weird_freeform");
    }

    #[test]
    fn display_label_shows_labels_and_keeps_unknowns() {
        let label = |tag: &str| super::display_label(tag).into_owned();
        // Every canonical tag renders as its own label.
        for t in TRAITS {
            assert_eq!(label(t.tag), t.label, "{} renders its label", t.tag);
        }
        // Old spellings render under the merged entry's label.
        assert_eq!(label("taco"), "Canoeing");
        assert_eq!(label("canoe"), "Canoeing");
        // Unknown free-form tags render as typed.
        assert_eq!(label("weird_freeform"), "weird_freeform");
        assert_eq!(label("Crispy Leaves"), "Crispy Leaves");
    }

    #[test]
    fn stretch_detector_matches_only_stretchy() {
        assert!(super::is_stretch_tag("stretchy"));
        assert!(!super::is_stretch_tag("canoeing"));
        assert!(!super::is_stretch_tag("healthy"));
        assert!(!super::is_stretch_tag("weird_freeform"));
    }

    #[test]
    fn stress_detector_matches_bleaching_and_canoeing() {
        // The merged tag and both old spellings mean light stress.
        for tag in ["bleaching", "canoeing", "taco", "canoe"] {
            assert!(super::is_light_stress_tag(tag), "{tag} means light stress");
        }
        for tag in [
            "stretchy",
            "healthy",
            "yellowing",
            "droop",
            "weird_freeform",
        ] {
            assert!(
                !super::is_light_stress_tag(tag),
                "{tag} is not light stress"
            );
        }
    }
}
