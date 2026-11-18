//! Build script: generate the leptos_i18n module from `locales/`
//! (D112). `en` is the default locale and the key source of truth; the
//! generated keys struct fails the build when a key is missing from any
//! catalog.

use std::collections::BTreeSet;
use std::path::PathBuf;

use leptos_i18n_build::{Config, TranslationsInfos};
use serde_json::Value;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = leptos_i18n_build::ParseOptions::new().interpolate_display(true);
    let cfg = Config::new("en")?.add_locale("de")?.parse_options(options);
    let translations_infos = TranslationsInfos::parse(cfg)?;
    translations_infos.emit_diagnostics();
    translations_infos.rerun_if_locales_changed();
    // Generate into OUT_DIR (the crate includes it from there); a
    // relative path would litter the package root with generated code.
    let mod_dir = PathBuf::from(std::env::var("OUT_DIR")?).join("i18n");
    translations_infos.generate_i18n_module(mod_dir)?;
    assert_key_parity()?;
    Ok(())
}

/// Flattens a catalog into its dotted key paths (nested objects only;
/// leaf strings — interpolated or not — are keys).
fn key_paths(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    if let Value::Object(map) = value {
        for (key, child) in map {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            key_paths(child, &path, out);
        }
    } else {
        out.insert(prefix.to_owned());
    }
}

/// Enforces EN↔DE key parity at compile time (D112): the codegen only
/// *warns* on a missing key (falling back to the default locale), but
/// the ticket requires the build to fail. Panics — a build-script
/// panic fails the build — listing every missing key per catalog.
fn assert_key_parity() -> Result<(), Box<dyn std::error::Error>> {
    let en: Value = serde_json::from_str(&std::fs::read_to_string("locales/en.json")?)?;
    let de: Value = serde_json::from_str(&std::fs::read_to_string("locales/de.json")?)?;
    let mut en_keys = BTreeSet::new();
    let mut de_keys = BTreeSet::new();
    key_paths(&en, "", &mut en_keys);
    key_paths(&de, "", &mut de_keys);
    let missing_in_de: Vec<&String> = en_keys.difference(&de_keys).collect();
    let missing_in_en: Vec<&String> = de_keys.difference(&en_keys).collect();
    if missing_in_de.is_empty() && missing_in_en.is_empty() {
        return Ok(());
    }
    panic!(
        "i18n key parity violated (en is the key source of truth, D112): \
         missing in de: {missing_in_de:?}; missing in en: {missing_in_en:?}"
    );
}
