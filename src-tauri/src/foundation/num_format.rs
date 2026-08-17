//! Compact number rendering — the one place `1234567` becomes `1.23M`.
//!
//! # Why this exists
//!
//! Five implementations of this existed, and they rendered the same value five
//! different ways:
//!
//! | Where | `1_234_567` | `4_500` | Notes |
//! |-------|-------------|---------|-------|
//! | `trading::fmt_notional` | `$1.2M` | `$4.5K` | no B tier |
//! | `command_palette::human_volume` | `1.23M` | `4.5K` | has B tier |
//! | `bottom_dock::money` | `$1.23M` | `$4.5K` | no B tier |
//! | `portfolio_pane::fmt_money` | `1.23M` | `4500` | K tier only above **10_000** |
//! | `scanner_panel::fmt_volume` | `1.2M` | `4K` | integer K |
//!
//! This is not a tidiness problem. A user reading a P&L in the bottom dock and
//! the same figure in the portfolio pane saw `$1.23M` and `1.23M`; a volume in
//! the scanner and the command palette read `1.2M` and `1.23M`. The design
//! system's whole claim is that a value looks the same wherever it appears, and
//! five formatters quietly broke that in the one place a trader looks hardest.
//!
//! # The shape
//!
//! One core (`compact`), three named renderings. They differ where the DOMAIN
//! differs, not where the author differed:
//!
//! * [`money`] — carries the currency symbol; two decimals at M and above.
//! * [`plain`] — the same number without a symbol, for panels that label the
//!   currency in a column header instead of on every cell.
//! * [`volume`] — share counts. Reaches B, and uses one decimal, because a
//!   volume's second decimal is noise at these magnitudes.
//!
//! Anything else should use one of these rather than adding a fourth.

/// Which magnitude suffix a value lands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tier {
    Billion,
    Million,
    Thousand,
    Unit,
}

impl Tier {
    fn of(abs: f64, allow_billions: bool) -> Self {
        if allow_billions && abs >= 1e9 {
            Tier::Billion
        } else if abs >= 1e6 {
            Tier::Million
        } else if abs >= 1e3 {
            Tier::Thousand
        } else {
            Tier::Unit
        }
    }

    fn divisor(self) -> f64 {
        match self {
            Tier::Billion => 1e9,
            Tier::Million => 1e6,
            Tier::Thousand => 1e3,
            Tier::Unit => 1.0,
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Tier::Billion => "B",
            Tier::Million => "M",
            Tier::Thousand => "K",
            Tier::Unit => "",
        }
    }
}

/// The one implementation. `major_dp` applies at B/M, `kilo_dp` at K; units are
/// always whole, because a compact form that prints `987.00` is not compact.
///
/// The threshold is taken on `abs`, so `-1_234_567` renders `-1.23M` and not
/// `-1234567`. Two of the five old formatters got that right and three did not.
fn compact(v: f64, prefix: &str, major_dp: usize, kilo_dp: usize, allow_billions: bool) -> String {
    if !v.is_finite() {
        // NaN/inf reach this from live feeds during a reconnect. Rendering
        // "NaN" into a P&L cell is worse than an em dash: it looks like a
        // number. None of the five old formatters handled it at all — they
        // printed "NaN" or "inf" straight into the UI.
        return format!("{prefix}—");
    }
    let tier = Tier::of(v.abs(), allow_billions);
    let scaled = v / tier.divisor();
    let dp = match tier {
        Tier::Billion | Tier::Million => major_dp,
        Tier::Thousand => kilo_dp,
        Tier::Unit => 0,
    };
    format!("{prefix}{scaled:.dp$}{suffix}", dp = dp, suffix = tier.suffix())
}

/// `$1.23M` / `$4.5K` / `$987` — currency figures.
#[must_use]
pub fn money(v: f64) -> String {
    // The `$` sits before the minus in a naive format (`$-1.2M`), which reads
    // badly and is not how any of the five old formatters wanted it either —
    // one of them special-cased it and the rest simply had the bug.
    if v < 0.0 {
        format!("-{}", compact(-v, "$", 2, 1, false))
    } else {
        compact(v, "$", 2, 1, false)
    }
}

/// `+$1.23M` / `-$4.5K` — a signed currency figure, for deltas and P&L.
#[must_use]
pub fn signed_money(v: f64) -> String {
    if v >= 0.0 {
        format!("+{}", money(v))
    } else {
        money(v)
    }
}

/// `1.23M` / `4.5K` / `987` — the same number with no currency symbol.
#[must_use]
pub fn plain(v: f64) -> String {
    compact(v, "", 2, 1, false)
}

/// `1.2B` / `1.2M` / `4.5K` / `987` — share counts and similar tallies.
#[must_use]
pub fn volume(v: f64) -> String {
    compact(v, "", 1, 1, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The consistency this exists for: one value, one rendering, whichever
    /// panel asks. Five formatters used to disagree here.
    #[test]
    fn one_value_renders_the_same_everywhere() {
        assert_eq!(money(1_234_567.0), "$1.23M");
        assert_eq!(plain(1_234_567.0), "1.23M");
        assert_eq!(volume(1_234_567.0), "1.2M");
    }

    #[test]
    fn tiers_land_on_the_right_suffix() {
        assert_eq!(plain(999.0), "999");
        assert_eq!(plain(1_000.0), "1.0K");
        assert_eq!(plain(999_999.0), "1000.0K");
        assert_eq!(plain(1_000_000.0), "1.00M");
        assert_eq!(volume(1_000_000_000.0), "1.0B");
        // `plain`/`money` stop at M on purpose — a currency figure in the
        // billions is a data error in this app far more often than it is real.
        assert_eq!(plain(1_000_000_000.0), "1000.00M");
    }

    /// Negative values must compact too. Three of the five old formatters
    /// thresholded on the raw value, so `-1_234_567` printed in full.
    #[test]
    fn negatives_compact_and_the_sign_leads() {
        assert_eq!(money(-1_234_567.0), "-$1.23M");
        assert_eq!(plain(-4_500.0), "-4.5K");
        assert_eq!(volume(-1_500_000.0), "-1.5M");
    }

    #[test]
    fn signed_money_always_carries_a_sign() {
        assert_eq!(signed_money(1_500.0), "+$1.5K");
        assert_eq!(signed_money(-1_500.0), "-$1.5K");
        assert_eq!(signed_money(0.0), "+$0");
    }

    /// A live feed can hand a panel NaN mid-reconnect. "NaN" in a P&L cell
    /// looks like a number; an em dash does not.
    #[test]
    fn non_finite_values_do_not_reach_the_screen_as_nan() {
        assert_eq!(money(f64::NAN), "$—");
        assert_eq!(plain(f64::INFINITY), "—");
        assert_eq!(volume(f64::NEG_INFINITY), "—");
    }

    #[test]
    fn zero_is_plain_zero() {
        assert_eq!(money(0.0), "$0");
        assert_eq!(plain(0.0), "0");
        assert_eq!(volume(0.0), "0");
    }
}
