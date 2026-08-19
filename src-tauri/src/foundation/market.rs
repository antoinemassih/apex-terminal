//! The arithmetic of a market day — where "we don't know" is representable.
//!
//! # Why this exists
//!
//! Six places computed a day's percent change, and five of them spelled the
//! unknown case the same wrong way:
//!
//! ```ignore
//! let change_pct = if prev_close > 0.0 {
//!     (price - prev_close) / prev_close * 100.0
//! } else {
//!     0.0            // <-- "we have no previous close" rendered as "unchanged"
//! };
//! ```
//!
//! `0.0` is not a neutral placeholder here. It is a claim, and it is one the
//! UI then acts on:
//!
//! * The scanner's "Top Gainers" preset filters `change_pct >= 0.0`. A symbol
//!   whose previous close never arrived scores exactly `0.0`, passes the
//!   filter, and is listed as a gainer. "Top Losers" filters `<= 0.0`, so the
//!   same symbol is simultaneously listed as a loser. A scanner whose entire
//!   job is "which names are moving" was including names it had no move data
//!   for, in both directions at once.
//! * A change cell colours on `>= 0.0`, so the unknown paints BULL green and
//!   reads `+0.00%` — the most confident thing a price cell can say.
//!
//! # The inverse is worse
//!
//! Three sites then reconstructed the previous close BACK out of the fabricated
//! percentage:
//!
//! ```ignore
//! prev_close: if change_pct != 0.0 { price / (1.0 + change_pct / 100.0) }
//!             else { price }        // <-- unknown laundered into a real datum
//! ```
//!
//! With `change_pct == 0.0` that writes `prev_close = price`. Downstream code
//! tests `prev_close > 0.0` to decide whether the value is known — and now it
//! passes. "Save scan as watchlist" wrote that to disk, so the unknown became a
//! confident `0.00%` permanently, in a file, with `loaded: true` next to it.
//!
//! The fix is not a better guess. It is to keep the raw datum (`prev_close`)
//! and derive the percentage through [`day_change_pct`], which returns `None`
//! when there is nothing to divide by, so every caller has to decide what to
//! show rather than inheriting a lie.
//!
//! # Why `> 0.0` and not an epsilon
//!
//! A previous close is a traded price. Zero means "not populated"; there is no
//! genuine equity or option whose prior settle is 0.0000001, so an epsilon band
//! would only widen the window in which a division blows up to `inf`. The
//! existing correct implementation (`WatchlistState::get_change_pct`) already
//! used `> 0.0`, and this generalises it rather than inventing a new rule.

/// Percent change from `from` to `to`, or `None` when there is no base to
/// measure against.
///
/// The one implementation. [`day_change_pct`] is this with the base named
/// "previous close"; a pane's range badge is this with the base named "first
/// loaded bar". They are the same arithmetic and the same unknown case, and
/// splitting them into separate `if base > 0.0 { .. } else { 0.0 }` expressions
/// is how the second one came to paint a confident green `+0.00%` for a pane
/// with no bars at all.
#[must_use]
pub fn pct_change(from: f32, to: f32) -> Option<f32> {
    if from > 0.0 && from.is_finite() && to.is_finite() {
        Some((to - from) / from * 100.0)
    } else {
        None
    }
}

/// A day's percent change, or `None` when the previous close is unknown.
///
/// `None` means "we have no basis to compute this" — render it as an em dash
/// in a neutral colour, exclude it from a change-band filter, and sort it last.
/// Do NOT substitute `0.0`; see the module docs for what that costs.
#[must_use]
pub fn day_change_pct(price: f32, prev_close: f32) -> Option<f32> {
    pct_change(prev_close, price)
}

/// `+1.23%` / `-0.40%` / `—` — the one rendering of a day change.
///
/// The em dash is deliberate and matches [`crate::foundation::num_format`],
/// which uses the same glyph for a non-finite figure. A user who sees `—`
/// knows the terminal has nothing; a user who sees `+0.00%` believes it does.
#[must_use]
pub fn fmt_change_pct(v: Option<f32>) -> String {
    match v {
        Some(p) if p.is_finite() => format!("{p:+.2}%"),
        _ => "—".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_change_is_computed() {
        let v = day_change_pct(110.0, 100.0).expect("known");
        assert!((v - 10.0).abs() < 1e-4, "got {v}");
        let v = day_change_pct(95.0, 100.0).expect("known");
        assert!((v + 5.0).abs() < 1e-4, "got {v}");
    }

    /// The defect this module exists for: an absent previous close must not
    /// come back as "unchanged".
    #[test]
    fn an_unknown_previous_close_is_none_and_not_zero() {
        assert_eq!(day_change_pct(100.0, 0.0), None);
        assert_eq!(day_change_pct(100.0, -1.0), None);
    }

    /// `Some(0.0)` and `None` are different facts and must stay distinguishable
    /// — a genuinely flat symbol is not the same as an unknown one.
    #[test]
    fn a_genuinely_flat_symbol_is_some_zero() {
        assert_eq!(day_change_pct(100.0, 100.0), Some(0.0));
        assert_ne!(day_change_pct(100.0, 100.0), day_change_pct(100.0, 0.0));
    }

    /// A live feed hands NaN through on a reconnect. Dividing on it yields NaN,
    /// which colours BULL (`NaN >= 0.0` is false, so it colours BEAR) and
    /// formats as "NaN%". Neither is acceptable in a price cell.
    #[test]
    fn non_finite_inputs_are_unknown() {
        assert_eq!(day_change_pct(f32::NAN, 100.0), None);
        assert_eq!(day_change_pct(100.0, f32::NAN), None);
        assert_eq!(day_change_pct(f32::INFINITY, 100.0), None);
    }

    /// `day_change_pct` is `pct_change` with its base named. If they ever
    /// diverge, one surface will disagree with another about the same move.
    #[test]
    fn the_day_change_is_the_general_form_with_its_base_named() {
        for (base, now) in [(100.0f32, 110.0f32), (100.0, 90.0), (100.0, 100.0), (0.0, 50.0)] {
            assert_eq!(day_change_pct(now, base), pct_change(base, now),
                "base={base} now={now}");
        }
    }

    /// A pane with no loaded bars has no base, and must not report "unchanged".
    #[test]
    fn an_absent_base_is_unknown_not_flat() {
        assert_eq!(pct_change(0.0, 123.0), None);
        assert_eq!(pct_change(f32::NAN, 123.0), None);
        assert_eq!(pct_change(100.0, f32::NAN), None);
    }

    #[test]
    fn the_unknown_renders_as_a_dash_not_a_number() {
        assert_eq!(fmt_change_pct(None), "—");
        assert_eq!(fmt_change_pct(Some(1.234)), "+1.23%");
        assert_eq!(fmt_change_pct(Some(-0.4)), "-0.40%");
        assert_eq!(fmt_change_pct(Some(0.0)), "+0.00%");
        assert_eq!(fmt_change_pct(Some(f32::NAN)), "—");
    }
}
