//! Home Assistant theme adoption (ticket 07, D100): parsing the
//! parent frontend's theme variables, the sRGB→oklch colour math, and
//! the HA → daisyUI variable map.
//!
//! Pure and I/O-free like the rest of this crate: the frontend reads
//! the parent document and hands raw strings to [`HaTheme`]; this
//! module decides whether a usable theme is present and produces the
//! daisyUI overrides. Any missing or unparsable core variable means
//! the whole adoption is `None`, which the frontend turns into the
//! mandatory silent fallback to the built-in theme.

/// A colour in sRGB, channels `0..=255`, alpha `0..=1`.
///
/// Intermediate type for parsing and compositing before conversion to
/// [`Oklch`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb {
    /// Red channel, `0..=255`.
    pub red: f64,
    /// Green channel, `0..=255`.
    pub green: f64,
    /// Blue channel, `0..=255`.
    pub blue: f64,
    /// Opacity, `0..=1` (unparsed colours are opaque).
    pub alpha: f64,
}

impl Rgb {
    /// Opaque colour from `0..=255` channels.
    #[must_use]
    pub const fn new(red: f64, green: f64, blue: f64) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: 1.0,
        }
    }

    /// The same colour with `alpha` (`0..=1`).
    #[must_use]
    pub const fn with_alpha(mut self, alpha: f64) -> Self {
        self.alpha = alpha;
        self
    }

    /// WCAG 2.x relative luminance (alpha ignored, like Home
    /// Assistant's own contrast helper).
    #[must_use]
    pub fn relative_luminance(self) -> f64 {
        let linear = |channel: f64| -> f64 {
            let c = channel / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126_f64.mul_add(
            linear(self.red),
            0.7152_f64.mul_add(linear(self.green), 0.0722 * linear(self.blue)),
        )
    }
}

/// Home Assistant's text-on-primary rule
/// (`rgbContrast(color, [33, 33, 33]) < 6 ? "#fff" : "#212121"` in
/// `ha-style.ts`): white unless near-black passes a 6:1 contrast.
#[must_use]
pub fn on_color_for(base: Rgb) -> Rgb {
    let near_black = Rgb::new(33.0, 33.0, 33.0);
    if contrast_ratio(base, near_black) < 6.0 {
        Rgb::new(255.0, 255.0, 255.0)
    } else {
        near_black
    }
}

/// WCAG 2.x contrast ratio between two colours (`1..=21`).
#[must_use]
pub fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
    let la = a.relative_luminance();
    let lb = b.relative_luminance();
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Source-over alpha compositing of `foreground` onto `background`.
///
/// Home Assistant's `--divider-color` is commonly translucent
/// (`rgba(0, 0, 0, 0.12)`); daisyUI's `base-300` must be an opaque
/// colour, so the divider is flattened onto the page background.
#[must_use]
pub fn flatten(foreground: Rgb, background: Rgb) -> Rgb {
    let out_alpha = background
        .alpha
        .mul_add(1.0 - foreground.alpha, foreground.alpha);
    if out_alpha <= f64::EPSILON {
        return background;
    }
    let mix = |fg: f64, bg: f64| -> f64 {
        fg.mul_add(
            foreground.alpha,
            bg * background.alpha * (1.0 - foreground.alpha),
        ) / out_alpha
    };
    Rgb {
        red: mix(foreground.red, background.red),
        green: mix(foreground.green, background.green),
        blue: mix(foreground.blue, background.blue),
        alpha: out_alpha,
    }
}

/// A colour in the Oklab-derived cylindrical space: lightness `0..=1`,
/// chroma (roughly saturation), and hue in degrees `0..360`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    /// Perceptual lightness, `0..=1`.
    pub lightness: f64,
    /// Chroma; `0` for greys.
    pub chroma: f64,
    /// Hue angle in degrees, `0..360` (`0` when chroma is `0`).
    pub hue: f64,
}

/// Converts sRGB to Oklch via Ottosson's Oklab matrices
/// (<https://bottosson.github.io/posts/okpaper/>): linearise, apply the
/// LMS matrix, take the cube root, apply the Lab matrix, then convert
/// Lab to `LCh`.
#[must_use]
pub fn rgb_to_oklch(color: Rgb) -> Oklch {
    let linear = |channel: f64| -> f64 {
        let c = channel / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let (red, green, blue) = (linear(color.red), linear(color.green), linear(color.blue));

    // Linear sRGB → cone response.
    let l = 0.412_221_470_8_f64.mul_add(
        red,
        0.536_332_536_3_f64.mul_add(green, 0.051_445_992_9 * blue),
    );
    let m = 0.211_903_498_2_f64.mul_add(
        red,
        0.680_699_545_1_f64.mul_add(green, 0.107_396_956_6 * blue),
    );
    let s = 0.088_302_461_9_f64.mul_add(
        red,
        0.281_718_837_6_f64.mul_add(green, 0.629_978_700_5 * blue),
    );

    // Perceptual non-linearity.
    let l = l.cbrt();
    let m = m.cbrt();
    let s = s.cbrt();

    // Cone response → opponent axes.
    let lightness =
        0.210_454_255_3_f64.mul_add(l, 0.793_617_785_0_f64.mul_add(m, -0.004_072_046_8 * s));
    let a = 1.977_998_495_1_f64.mul_add(l, (-2.428_592_205_0_f64).mul_add(m, 0.450_593_709_9 * s));
    let b_axis =
        0.025_904_037_1_f64.mul_add(l, 0.782_771_766_2_f64.mul_add(m, -0.808_675_766_0 * s));

    let chroma = a.hypot(b_axis);
    let hue = if chroma <= f64::EPSILON {
        0.0
    } else {
        b_axis.atan2(a).to_degrees().rem_euclid(360.0)
    };
    Oklch {
        lightness,
        chroma,
        hue,
    }
}

/// Formats a colour as a CSS `oklch()` function value, the format
/// daisyUI 5 itself uses for its theme variables.
#[must_use]
pub fn oklch_string(color: Oklch) -> String {
    format!(
        "oklch({:.5} {:.5} {:.3})",
        color.lightness, color.chroma, color.hue
    )
}

/// Parses a CSS colour as it can appear in a Home Assistant theme
/// variable.
///
/// Supported: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` hex, the
/// `rgb()`/`rgba()` functions (comma and space syntax, optional
/// `/ <alpha>`, percentage channels), a small named-colour set, and
/// `transparent`. Everything else — including unresolved `var()`
/// references — yields `None`, which feeds the silent fallback.
#[must_use]
pub fn parse_css_color(input: &str) -> Option<Rgb> {
    let text = input.trim();
    if let Some(hex) = text.strip_prefix('#') {
        return parse_hex(hex);
    }
    let lowered = text.to_ascii_lowercase();
    if let Some(rest) = lowered
        .strip_prefix("rgb(")
        .or_else(|| lowered.strip_prefix("rgba("))
    {
        let body = rest.strip_suffix(')')?;
        return parse_rgb_function(body);
    }
    match lowered.as_str() {
        "transparent" => Some(Rgb::new(0.0, 0.0, 0.0).with_alpha(0.0)),
        "white" => Some(Rgb::new(255.0, 255.0, 255.0)),
        "black" => Some(Rgb::new(0.0, 0.0, 0.0)),
        "red" => Some(Rgb::new(255.0, 0.0, 0.0)),
        "green" => Some(Rgb::new(0.0, 128.0, 0.0)),
        "blue" => Some(Rgb::new(0.0, 0.0, 255.0)),
        "yellow" => Some(Rgb::new(255.0, 255.0, 0.0)),
        "orange" => Some(Rgb::new(255.0, 165.0, 0.0)),
        "gray" | "grey" => Some(Rgb::new(128.0, 128.0, 128.0)),
        _ => None,
    }
}

/// `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`.
fn parse_hex(hex: &str) -> Option<Rgb> {
    let channel = |pair: &str| -> Option<f64> { u8::from_str_radix(pair, 16).ok().map(f64::from) };
    match hex.len() {
        3 | 4 => {
            let digits: Vec<char> = hex.chars().collect();
            let expand = |c: char| -> Option<f64> { channel(&format!("{c}{c}")) };
            let red = expand(*digits.first()?)?;
            let green = expand(*digits.get(1)?)?;
            let blue = expand(*digits.get(2)?)?;
            let alpha = digits
                .get(3)
                .map_or(Some(1.0), |c| expand(*c).map(|v| v / 255.0))?;
            Some(Rgb::new(red, green, blue).with_alpha(alpha))
        }
        6 | 8 => {
            let red = channel(hex.get(0..2)?)?;
            let green = channel(hex.get(2..4)?)?;
            let blue = channel(hex.get(4..6)?)?;
            let alpha = if hex.len() == 8 {
                channel(hex.get(6..8)?)? / 255.0
            } else {
                1.0
            };
            Some(Rgb::new(red, green, blue).with_alpha(alpha))
        }
        _ => None,
    }
}

/// The inside of an `rgb()`/`rgba()`: `r, g, b[, a]` or
/// `r g b[ / a]`, channels `0..=255` or percentages, alpha `0..=1` or
/// a percentage.
fn parse_rgb_function(body: &str) -> Option<Rgb> {
    let channel = |token: &str| -> Option<f64> {
        let token = token.trim();
        if let Some(percent) = token.strip_suffix('%') {
            let value: f64 = percent.trim().parse().ok()?;
            Some((value / 100.0) * 255.0)
        } else {
            token.parse().ok()
        }
    };
    let alpha = |token: &str| -> Option<f64> {
        let token = token.trim();
        if let Some(percent) = token.strip_suffix('%') {
            let value: f64 = percent.trim().parse().ok()?;
            Some(value / 100.0)
        } else {
            token.parse().ok()
        }
    };

    let (rgb_part, alpha_part) = if let Some((channels, a)) = body.split_once('/') {
        (channels, Some(a))
    } else {
        (body, None)
    };
    let tokens: Vec<&str> = if rgb_part.contains(',') {
        rgb_part.split(',').collect()
    } else {
        rgb_part.split_whitespace().collect()
    };
    // Comma syntax carries the alpha as a fourth token.
    let (channels, trailing_alpha) = match tokens.as_slice() {
        [r, g, b] => ([*r, *g, *b], None),
        [r, g, b, a] => ([*r, *g, *b], Some(*a)),
        _ => return None,
    };
    let red = channel(channels[0])?;
    let green = channel(channels[1])?;
    let blue = channel(channels[2])?;
    let alpha_token = trailing_alpha.or(alpha_part);
    let alpha = match alpha_token {
        Some(token) => alpha(token)?,
        None => 1.0,
    };
    if !(0.0..=255.0).contains(&red)
        || !(0.0..=255.0).contains(&green)
        || !(0.0..=255.0).contains(&blue)
        || !(0.0..=1.0).contains(&alpha)
    {
        return None;
    }
    Some(Rgb::new(red, green, blue).with_alpha(alpha))
}

/// The raw Home Assistant theme variables read off the parent
/// document's root element.
///
/// Fields stay unparsed strings (`None` when the variable is absent)
/// so that [`adopt`] can decide the required-set policy in one place.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HaTheme {
    /// `--primary-color`.
    pub primary: Option<String>,
    /// `--accent-color`.
    pub accent: Option<String>,
    /// `--primary-background-color`.
    pub primary_background: Option<String>,
    /// `--card-background-color`.
    pub card_background: Option<String>,
    /// `--divider-color`.
    pub divider: Option<String>,
    /// `--primary-text-color`.
    pub primary_text: Option<String>,
    /// `--text-primary-color` (text *on* the primary colour); derived
    /// by contrast when absent.
    pub text_on_primary: Option<String>,
    /// `--success-color`.
    pub success: Option<String>,
    /// `--warning-color`.
    pub warning: Option<String>,
    /// `--error-color`.
    pub error: Option<String>,
    /// `--info-color`.
    pub info: Option<String>,
}

impl HaTheme {
    /// Reads one variable, `None` when absent or blank.
    #[must_use]
    pub fn from_values(get: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            primary: get("--primary-color"),
            accent: get("--accent-color"),
            primary_background: get("--primary-background-color"),
            card_background: get("--card-background-color"),
            divider: get("--divider-color"),
            primary_text: get("--primary-text-color"),
            text_on_primary: get("--text-primary-color"),
            success: get("--success-color"),
            warning: get("--warning-color"),
            error: get("--error-color"),
            info: get("--info-color"),
        }
    }
}

/// A successfully adopted theme: the daisyUI variable overrides plus
/// the colour scheme they imply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptedTheme {
    /// `"dark"` or `"light"`, from the page background's lightness.
    color_scheme: &'static str,
    /// `(css variable, oklch value)` pairs in application order.
    entries: Vec<(&'static str, String)>,
}

impl AdoptedTheme {
    /// `"dark"` or `"light"` for the `color-scheme` property.
    #[must_use]
    pub const fn color_scheme(&self) -> &'static str {
        self.color_scheme
    }

    /// The daisyUI `--color-*` overrides to set on the app root.
    #[must_use]
    pub fn entries(&self) -> &[(&'static str, String)] {
        &self.entries
    }
}

/// Maps a parent theme onto daisyUI's semantic variables.
///
/// `None` — the caller's cue to fall back to the built-in theme — when
/// any of the six core variables (primary, accent, primary background,
/// card background, divider, primary text) is missing or unparsable.
/// The four state colours are individually optional: a missing one
/// leaves the built-in value in place.
///
/// The map (D100): HA `--card-background-color` → `--color-base-100`,
/// `--primary-background-color` → `--color-base-200`,
/// `--divider-color` flattened onto the page background →
/// `--color-base-300`, `--primary-text-color` → `--color-base-content`,
/// `--primary-color`/`--accent-color` → `--color-primary` /
/// `--color-accent`. Content colours for primary and accent are the
/// HA-provided `--text-primary-color` when present (primary only) and
/// otherwise HA's own contrast rule ([`on_color_for`]); state content
/// colours always use the contrast rule. HA's
/// `--secondary-background-color` has no daisyUI counterpart and stays
/// unused.
#[must_use]
pub fn adopt(ha: &HaTheme) -> Option<AdoptedTheme> {
    let parse =
        |value: &Option<String>| -> Option<Rgb> { value.as_deref().and_then(parse_css_color) };

    let primary = parse(&ha.primary)?;
    let accent = parse(&ha.accent)?;
    let page = flatten_over_white(parse(&ha.primary_background)?);
    let card = flatten_over_white(parse(&ha.card_background)?);
    let divider = flatten(parse(&ha.divider)?, page);
    let text = flatten_over_white(parse(&ha.primary_text)?);

    let scheme_is_dark = rgb_to_oklch(page).lightness < 0.5;

    let mut entries = Vec::with_capacity(16);
    let push = |name: &'static str, color: Rgb, entries: &mut Vec<(&'static str, String)>| {
        entries.push((name, oklch_string(rgb_to_oklch(color))));
    };

    push("--color-primary", primary, &mut entries);
    push(
        "--color-primary-content",
        parse(&ha.text_on_primary).unwrap_or_else(|| on_color_for(primary)),
        &mut entries,
    );
    push("--color-accent", accent, &mut entries);
    push("--color-accent-content", on_color_for(accent), &mut entries);
    push("--color-base-100", card, &mut entries);
    push("--color-base-200", page, &mut entries);
    push("--color-base-300", divider, &mut entries);
    push("--color-base-content", text, &mut entries);

    // State colours are individually optional: a missing one leaves the
    // built-in value in place, so only present ones get entries.
    for (name, content_name, value) in [
        ("--color-success", "--color-success-content", &ha.success),
        ("--color-warning", "--color-warning-content", &ha.warning),
        ("--color-error", "--color-error-content", &ha.error),
        ("--color-info", "--color-info-content", &ha.info),
    ] {
        if let Some(color) = parse(value) {
            push(name, color, &mut entries);
            push(content_name, on_color_for(color), &mut entries);
        }
    }

    Some(AdoptedTheme {
        color_scheme: if scheme_is_dark { "dark" } else { "light" },
        entries,
    })
}

/// Translucent non-divider colours are composited onto white (the
/// HA light page default); HA's own backgrounds are opaque in
/// practice, this only guards degenerate themes.
fn flatten_over_white(color: Rgb) -> Rgb {
    if color.alpha < 1.0 {
        flatten(color, Rgb::new(255.0, 255.0, 255.0))
    } else {
        color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ottosson's published reference values. Lightness/chroma are
    /// printed to five decimals; hue only two, so its tolerance is
    /// wider.
    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.0005,
            "expected ≈{expected}, got {actual}"
        );
    }

    /// Like [`assert_close`], but for hue angles published at two
    /// decimals.
    fn assert_close_hue(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.01,
            "expected ≈{expected}, got {actual}"
        );
    }

    #[test]
    fn oklch_matches_ottosson_reference_vectors() {
        // <https://bottosson.github.io/posts/okpaper/> reference table.
        for (rgb, l, c, h) in [
            (Rgb::new(255.0, 0.0, 0.0), 0.627_955, 0.257_683, 29.2338),
            (Rgb::new(0.0, 255.0, 0.0), 0.866_44, 0.294_82, 142.49),
            (Rgb::new(0.0, 0.0, 255.0), 0.452_01, 0.313_21, 264.05),
        ] {
            let ok = rgb_to_oklch(rgb);
            assert_close(ok.lightness, l);
            assert_close(ok.chroma, c);
            assert_close_hue(ok.hue, h);
        }
        let white = rgb_to_oklch(Rgb::new(255.0, 255.0, 255.0));
        assert_close(white.lightness, 1.0);
        assert_close(white.chroma, 0.0);
        let black = rgb_to_oklch(Rgb::new(0.0, 0.0, 0.0));
        assert_close(black.lightness, 0.0);
        assert_close(black.chroma, 0.0);
        assert_close(black.hue, 0.0);
    }

    #[test]
    fn parses_hex_forms() {
        assert_eq!(parse_css_color("#ff0000"), Some(Rgb::new(255.0, 0.0, 0.0)));
        assert_eq!(parse_css_color("#abc"), Some(Rgb::new(170.0, 187.0, 204.0)));
        assert_eq!(
            parse_css_color("#ff000080").map(|c| c.alpha),
            Some(128.0 / 255.0)
        );
        assert_eq!(
            parse_css_color("#f00c").map(|c| c.alpha),
            Some(204.0 / 255.0)
        );
        assert_eq!(parse_css_color("#12"), None);
        assert_eq!(parse_css_color("#gg0000"), None);
    }

    #[test]
    fn parses_rgb_function_forms() {
        assert_eq!(
            parse_css_color("rgb(1, 2, 3)"),
            Some(Rgb::new(1.0, 2.0, 3.0))
        );
        assert_eq!(
            parse_css_color("rgba(1 2 3 / 0.5)"),
            Some(Rgb::new(1.0, 2.0, 3.0).with_alpha(0.5))
        );
        assert_eq!(
            parse_css_color("rgb(100% 0% 0%)"),
            Some(Rgb::new(255.0, 0.0, 0.0))
        );
        assert_eq!(
            parse_css_color("rgba(0,0,0,.12)"),
            Some(Rgb::new(0.0, 0.0, 0.0).with_alpha(0.12))
        );
        assert_eq!(parse_css_color("rgb(1, 2)"), None);
        assert_eq!(parse_css_color("rgb(300, 0, 0)"), None);
        assert_eq!(parse_css_color("var(--ha-color-primary-40)"), None);
    }

    #[test]
    fn parses_named_subset_and_transparent() {
        assert_eq!(
            parse_css_color("  White "),
            Some(Rgb::new(255.0, 255.0, 255.0))
        );
        assert_eq!(parse_css_color("transparent").map(|c| c.alpha), Some(0.0));
        assert_eq!(parse_css_color("rebeccapurple"), None);
    }

    #[test]
    fn flatten_composites_source_over() {
        // 12% black over ~#fafafa: HA's default light divider.
        let page = Rgb::new(250.0, 250.0, 250.0);
        let divider = flatten(Rgb::new(0.0, 0.0, 0.0).with_alpha(0.12), page);
        for channel in [divider.red, divider.green, divider.blue] {
            assert_close(channel, 250.0 * 0.88);
        }
        assert_close(divider.alpha, 1.0);
    }

    #[test]
    fn on_color_mirrors_ha_contrast_rule() {
        // Leaflog's own green primary: near-black fails 6:1 → white.
        assert_eq!(on_color_for(Rgb::new(46.0, 125.0, 79.0)).red, 255.0);
        // A bright HA default blue: near-black just passes 6:1
        // (ratio ≈ 6.1) → #212121, the rule's explicit outcome.
        assert_eq!(on_color_for(Rgb::new(3.0, 169.0, 244.0)).red, 33.0);
        // Pale yellow passes against near-black → #212121.
        assert_eq!(on_color_for(Rgb::new(255.0, 253.0, 200.0)).red, 33.0);
    }

    #[test]
    fn adopt_requires_the_six_core_variables() {
        let full = HaTheme {
            primary: Some("#3f51b5".into()),
            accent: Some("#ff9800".into()),
            primary_background: Some("#fafafa".into()),
            card_background: Some("#ffffff".into()),
            divider: Some("rgba(0,0,0,.12)".into()),
            primary_text: Some("#212121".into()),
            ..HaTheme::default()
        };
        assert!(adopt(&full).is_some());

        // Each core variable individually removed → no adoption.
        let break_one = |f: fn(&mut HaTheme)| {
            let mut broken = full.clone();
            f(&mut broken);
            assert!(adopt(&broken).is_none());
        };
        break_one(|ha| ha.primary = None);
        break_one(|ha| ha.accent = None);
        break_one(|ha| ha.primary_background = None);
        break_one(|ha| ha.card_background = None);
        break_one(|ha| ha.divider = None);
        break_one(|ha| ha.primary_text = None);
        // Unparsable counts as missing too.
        break_one(|ha| ha.primary = Some("var(--x)".into()));
    }

    #[test]
    fn adopt_maps_the_daisyui_slots() {
        let ha = HaTheme {
            primary: Some("#3f51b5".into()),
            accent: Some("#ff9800".into()),
            primary_background: Some("#fafafa".into()),
            card_background: Some("#ffffff".into()),
            divider: Some("rgba(0,0,0,.12)".into()),
            primary_text: Some("#212121".into()),
            text_on_primary: Some("#ffffff".into()),
            success: Some("#43a047".into()),
            ..HaTheme::default()
        };
        let adopted = adopt(&ha).expect("full theme adopts");
        assert_eq!(adopted.color_scheme(), "light");

        let value = |name: &str| -> String {
            adopted
                .entries()
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        assert_eq!(
            value("--color-primary"),
            oklch_string(rgb_to_oklch(Rgb::new(63.0, 81.0, 181.0)))
        );
        assert_eq!(
            value("--color-primary-content"),
            oklch_string(rgb_to_oklch(Rgb::new(255.0, 255.0, 255.0)))
        );
        assert_eq!(
            value("--color-base-100"),
            oklch_string(rgb_to_oklch(Rgb::new(255.0, 255.0, 255.0)))
        );
        assert_eq!(
            value("--color-base-300"),
            oklch_string(rgb_to_oklch(flatten(
                Rgb::new(0.0, 0.0, 0.0).with_alpha(0.12),
                Rgb::new(250.0, 250.0, 250.0)
            )))
        );
        // Missing warning/error/info entries: the built-in values stay.
        assert_eq!(value("--color-warning"), "");
        assert_eq!(value("--color-error"), "");
        assert_eq!(value("--color-info"), "");
        assert_ne!(value("--color-success"), "");
        assert_ne!(value("--color-success-content"), "");
    }

    #[test]
    fn adopt_flags_dark_schemes() {
        let ha = HaTheme {
            primary: Some("#b39ddb".into()),
            accent: Some("#ff9800".into()),
            primary_background: Some("#111111".into()),
            card_background: Some("#1c1c1c".into()),
            divider: Some("rgba(225,225,225,.12)".into()),
            primary_text: Some("#e1e1e1".into()),
            ..HaTheme::default()
        };
        assert_eq!(
            adopt(&ha).expect("dark theme adopts").color_scheme(),
            "dark"
        );
    }

    #[test]
    fn oklch_string_formats_like_daisyui() {
        let red = rgb_to_oklch(Rgb::new(255.0, 0.0, 0.0));
        let text = oklch_string(red);
        assert!(text.starts_with("oklch(0.627"), "got {text}");
        assert!(text.ends_with(')'));
    }
}
