//! Numbers.
//!
//! All marks, faults, times and percentages are exact decimals, never floating point: 0.1 + 0.2
//! is exactly 0.3, and a value is only rounded where a rule says so, with [`round`].
//!
//! Rosson used JavaScript floating point, which can differ from exact arithmetic in the last
//! digit (for example `(1.005).toFixed(2)` gives "1.00"). docs/deviations.md E5.

use std::str::FromStr;

pub use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;

/// Round to `places` decimals, halves away from zero, as on paper: 64.755 → 64.76,
/// 0.125 → 0.13.
pub fn round(value: Decimal, places: u32) -> Decimal {
    value.round_dp_with_strategy(places, RoundingStrategy::MidpointAwayFromZero)
}

/// Parse a number as typed at the show ("61.2", "0", "7.5"). Returns `None` for an empty
/// field. A comma as decimal separator is accepted ("61,2").
pub fn parse(text: &str) -> Result<Option<Decimal>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Decimal::from_str(&trimmed.replace(',', "."))
        .map(Some)
        .map_err(|_| format!("not a number: {trimmed:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn rounds_halves_away_from_zero() {
        assert_eq!(round(d("64.755"), 2), d("64.76"));
        assert_eq!(round(d("1.005"), 2), d("1.01"));
        assert_eq!(round(d("64.754"), 2), d("64.75"));
        assert_eq!(round(d("2.25"), 1), d("2.3"));
    }

    #[test]
    fn parses_typed_numbers() {
        assert_eq!(parse("61.2").unwrap(), Some(d("61.2")));
        assert_eq!(parse(" 61,2 ").unwrap(), Some(d("61.2")));
        assert_eq!(parse("").unwrap(), None);
        assert!(parse("abc").is_err());
    }
}
