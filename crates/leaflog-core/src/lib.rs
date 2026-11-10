//! Shared domain types and pure grow-science engines for Leaflog.
//!
//! This crate is the common core used by both the backend and the frontend
//! (via WebAssembly). It contains domain types and the pure calculation
//! engines (phase forecast, feed-schedule fitting, VPD, lamp-height
//! advising, recommendations). It performs **no I/O** — everything here
//! must be unit-testable and must compile for native targets and for
//! `wasm32-unknown-unknown`.

#![warn(clippy::nursery)]
#![deny(
    unused,
    nonstandard_style,
    future_incompatible,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    clippy::cargo,
    clippy::complexity,
    clippy::correctness,
    clippy::perf,
    clippy::style,
    clippy::suspicious,
    clippy::pedantic,
    non_fmt_panics
)]
#![allow(clippy::multiple_crate_versions)]

use serde::{Deserialize, Serialize};

pub mod actions;
pub mod env;
pub mod equipment;
pub mod feed;
pub mod forecast;
pub mod grows;
pub mod history;
pub mod seedbank;
pub mod spaces;
pub mod strain;
pub mod tz;
pub mod units;

/// A phase of a cannabis grow, in chronological order.
///
/// The forecast engine (milestone M5) derives start/end dates for each
/// phase; the phase coach and recommendation engines key off the current
/// phase. `Harvest` is modelled as a logged *event* that ends the
/// [`Phase::Flower`] phase, not as a phase of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Seed meets water (or paper towel) until the taproot shows.
    Germination,
    /// First true leaves until the plant is established enough to feed.
    Seedling,
    /// Vegetative growth; length is the grower's choice for photoperiods.
    Vegetative,
    /// Flowering, from the flip (or auto onset) until harvest.
    Flower,
    /// Drying after harvest, until stems snap.
    Drying,
    /// Curing in jars.
    Curing,
}

impl Phase {
    /// All phases in chronological order.
    #[must_use]
    pub const fn all() -> [Self; 6] {
        [
            Self::Germination,
            Self::Seedling,
            Self::Vegetative,
            Self::Flower,
            Self::Drying,
            Self::Curing,
        ]
    }

    /// A stable, lowercase machine label used in the database and API
    /// (for example `"vegetative"`).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Germination => "germination",
            Self::Seedling => "seedling",
            Self::Vegetative => "vegetative",
            Self::Flower => "flower",
            Self::Drying => "drying",
            Self::Curing => "curing",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Phase;

    #[test]
    fn labels_are_stable_and_unique() {
        let all = Phase::all();
        let labels: Vec<&str> = all.iter().map(|p| p.label()).collect();
        let mut sorted = labels.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(labels.len(), sorted.len(), "phase labels must be unique");
    }

    #[test]
    fn phases_are_ordered_germination_first() {
        assert_eq!(Phase::all().first(), Some(&Phase::Germination));
        assert_eq!(Phase::all().last(), Some(&Phase::Curing));
    }
}
