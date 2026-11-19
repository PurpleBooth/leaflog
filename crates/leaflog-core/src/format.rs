//! Locale-aware decimal number formatting (D123).
//!
//! Pure, dependency-free formatting for the two shipped locales:
//! English groups thousands with a comma and separates decimals with a
//! dot; German mirrors both. Rust's float formatting stays the rounding
//! authority (`{:.N}` rounds half-to-even on exact binary ties); this
//! module only moves separators and inserts grouping. No platform
//! `Intl` — local-first.

/// The decimal separator and matching thousands grouping of one shipped
/// locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DecimalStyle {
    /// `1,234.5` — dot decimals, comma grouping (English).
    #[default]
    Dot,
    /// `1.234,5` — comma decimals, dot grouping (German).
    Comma,
}

impl DecimalStyle {
    /// The decimal separator character.
    #[must_use]
    pub const fn decimal_separator(self) -> char {
        match self {
            Self::Dot => '.',
            Self::Comma => ',',
        }
    }

    /// The thousands-grouping character.
    #[must_use]
    pub const fn group_separator(self) -> char {
        match self {
            Self::Dot => ',',
            Self::Comma => '.',
        }
    }
}

/// Formats `value` with `decimals` fraction digits, thousands
/// grouping, and `style`'s separators.
///
/// Rounding is Rust's `{:.N}` (half-to-even on exact binary ties); a
/// rounded carry into the integer part regroups correctly (`999.995`
/// at one decimal becomes `1,000.0`).
#[must_use]
pub fn format_decimal(value: f64, decimals: usize, style: DecimalStyle) -> String {
    // Rust always emits a dot and no grouping, so only the separators
    // need to move. Splitting after formatting means a rounded carry
    // into the integer part regroups for free (`999.995` at one
    // decimal becomes `1,000.0`).
    let plain = format!("{value:.decimals$}");
    let (sign, digits) = plain
        .strip_prefix('-')
        .map_or(("", plain.as_str()), |rest| ("-", rest));
    let (integer, fraction) = digits
        .split_once('.')
        .map_or((digits, None), |(whole, frac)| (whole, Some(frac)));
    let len = integer.len();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 2);
    out.push_str(sign);
    for (index, digit) in integer.chars().enumerate() {
        if index > 0 && (len - index) % 3 == 0 {
            out.push(style.group_separator());
        }
        out.push(digit);
    }
    if let Some(fraction) = fraction {
        out.push(style.decimal_separator());
        out.push_str(fraction);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{DecimalStyle, format_decimal};

    #[test]
    fn group_boundaries_group_right_to_left() {
        assert_eq!(format_decimal(1234.5, 1, DecimalStyle::Dot), "1,234.5");
        assert_eq!(format_decimal(1234.5, 1, DecimalStyle::Comma), "1.234,5");
        assert_eq!(
            format_decimal(1_234_567.891, 2, DecimalStyle::Dot),
            "1,234,567.89"
        );
        assert_eq!(
            format_decimal(1_234_567.891, 2, DecimalStyle::Comma),
            "1.234.567,89"
        );
        // Below the first thousand there is no grouping.
        assert_eq!(format_decimal(999.9, 1, DecimalStyle::Dot), "999.9");
        assert_eq!(format_decimal(999.9, 1, DecimalStyle::Comma), "999,9");
        // Exactly 1000 groups once, 1000 000 twice.
        assert_eq!(format_decimal(1000.0, 0, DecimalStyle::Comma), "1.000");
        assert_eq!(
            format_decimal(1_234_567.0, 0, DecimalStyle::Comma),
            "1.234.567"
        );
    }

    #[test]
    fn rounded_carry_regroups() {
        // 999.995 rounds up into the integer part at one decimal.
        assert_eq!(format_decimal(999.995, 1, DecimalStyle::Dot), "1,000.0");
        assert_eq!(format_decimal(999.995, 1, DecimalStyle::Comma), "1.000,0");
        assert_eq!(
            format_decimal(999_999.999_9, 2, DecimalStyle::Dot),
            "1,000,000.00"
        );
    }

    #[test]
    fn negative_values_keep_the_sign() {
        assert_eq!(format_decimal(-1234.5, 1, DecimalStyle::Dot), "-1,234.5");
        assert_eq!(format_decimal(-1234.5, 1, DecimalStyle::Comma), "-1.234,5");
        assert_eq!(format_decimal(-0.5, 2, DecimalStyle::Comma), "-0,50");
    }

    #[test]
    fn zero_and_small_values() {
        assert_eq!(format_decimal(0.0, 2, DecimalStyle::Dot), "0.00");
        assert_eq!(format_decimal(0.0, 2, DecimalStyle::Comma), "0,00");
        assert_eq!(format_decimal(0.5, 1, DecimalStyle::Comma), "0,5");
    }

    #[test]
    fn rounding_follows_rust_format_precision() {
        // Exactly representable ties round half-to-even.
        assert_eq!(format_decimal(2.5, 0, DecimalStyle::Dot), "2");
        assert_eq!(format_decimal(3.5, 0, DecimalStyle::Dot), "4");
        assert_eq!(format_decimal(0.125, 2, DecimalStyle::Comma), "0,12");
        // Ordinary rounding rounds half away per the binary value.
        assert_eq!(format_decimal(1234.456, 2, DecimalStyle::Comma), "1.234,46");
        assert_eq!(format_decimal(1234.454, 2, DecimalStyle::Comma), "1.234,45");
    }

    #[test]
    fn separators_match_the_style() {
        assert_eq!(DecimalStyle::Dot.decimal_separator(), '.');
        assert_eq!(DecimalStyle::Dot.group_separator(), ',');
        assert_eq!(DecimalStyle::Comma.decimal_separator(), ',');
        assert_eq!(DecimalStyle::Comma.group_separator(), '.');
    }
}
