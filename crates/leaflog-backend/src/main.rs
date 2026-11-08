//! Leaflog backend binary: loads the environment configuration and runs
//! the server. See the library docs (`leaflog_backend`) for the ingress
//! model and configuration variables.

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    leaflog_backend::run()
}
