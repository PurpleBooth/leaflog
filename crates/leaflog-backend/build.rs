//! Build script for the Leaflog backend.
//!
//! Ensures the frontend `dist/` directory exists before compilation so the
//! optional `include_dir!` embedding (see the `embed-assets` feature) works
//! on fresh checkouts, where `trunk build` has not run yet. An empty
//! directory embeds as an empty bundle; serving then reports that the
//! frontend has not been built.

use std::path::Path;

fn main() {
    let manifest =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set by cargo");
    let dist = Path::new(&manifest).join("../leaflog-frontend/dist");
    std::fs::create_dir_all(&dist).expect("could not create frontend dist directory");
    println!("cargo:rerun-if-changed={}", dist.display());
}
