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
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None => leaflog_backend::run(),
        Some("migrate") => leaflog_backend::run_migrate(&args.collect::<Vec<_>>()),
        Some(other) => Err(format!(
            "unknown command {other}; usage: leaflog-backend [migrate <apply|status>]"
        )
        .into()),
    }
}
