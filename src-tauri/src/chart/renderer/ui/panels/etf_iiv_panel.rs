//! ETF IIV (indicative intraday NAV) panel.
//!
//! Lists the curated ETF universe with per-row market price / indicative NAV /
//! premium-discount in basis points / staleness%. Refreshes every 10s via
//! `apex_data::live_state::get_or_fetch_etf_iiv` (TTL is also 10s in the cache).
//!
//! Color thresholds for the premium/discount cell:
//!   |bps| > 50 → bear (red), > 20 → warn (yellow), else dim text.

use egui;
use super::super::super::gpu::Theme;
use crate::data::feeds::apex_data::live_state as projector;
use crate::ui_kit::widgets::{PanelListRow, PanelColumn, PanelSection};

/// Default ETF universe — large-cap broad / sectors / fixed income flow.
pub(crate) const DEFAULT_ETFS: &[&str] = &[
    "SPY", "QQQ", "IWM", "DIA",
    // SPDR sectors
    "XLF", "XLK", "XLE", "XLV", "XLI", "XLY", "XLP", "XLB", "XLU", "XLRE", "XLC",
    // Bond / credit flow
    "HYG", "LQD",
];

/// Color tier for a premium/discount in bps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BpsTier { Calm, Yellow, Red }

/// `|bps| > 50` → red, `> 20` → yellow, else calm.
pub(crate) fn bps_tier(bps: f64) -> BpsTier {
    let a = bps.abs();
    if a > 50.0 { BpsTier::Red }
    else if a > 20.0 { BpsTier::Yellow }
    else { BpsTier::Calm }
}

/// Render the panel into an existing `Ui`. Caller owns the window/modal.
pub(crate) fn draw_content(ui: &mut egui::Ui, t: &Theme) {
    draw_content_with(ui, DEFAULT_ETFS, t);
}

pub(crate) fn draw_content_with(ui: &mut egui::Ui, etfs: &[&str], t: &Theme) {
    // Kick refreshes (TTL-gated).
    for e in etfs { projector::get_or_fetch_etf_iiv(e); }

    PanelSection::new("ETF IIV").show(ui, t, |ui, t| {
        // Header row — dim labels matching the 5 data columns.
        PanelListRow::new("etf_iiv_header")
            .columns(&[
                PanelColumn::left("ETF").color(t.dim),
                PanelColumn::right("Mark").color(t.dim),
                PanelColumn::right("iNAV").color(t.dim),
                PanelColumn::right("Prem/Disc").color(t.dim),
                PanelColumn::right("Stale%").color(t.dim),
            ])
            .hoverable(false)
            .show(ui, t);

        for e in etfs {
            let upper = e.to_uppercase();
            let reading = projector::get_etf_iiv(&upper);

            // Build column strings; borrow them for the slice.
            let mark_str;
            let inav_str;
            let bps_str;
            let stale_str;
            let bps_color;
            let stale_color;

            match reading.as_ref() {
                None => {
                    mark_str  = "—".to_string();
                    inav_str  = "—".to_string();
                    bps_str   = "—".to_string();
                    stale_str = "—".to_string();
                    bps_color   = t.dim;
                    stale_color = t.dim;
                }
                Some(r) => {
                    mark_str  = fmt_price(r.market_price);
                    inav_str  = fmt_price(r.iiv);
                    bps_str   = fmt_bps(r.premium_disc_bps);
                    stale_str = format!("{:.0}%", r.staleness_pct * 100.0);
                    bps_color = match bps_tier(r.premium_disc_bps) {
                        BpsTier::Calm   => t.dim,
                        BpsTier::Yellow => t.warn,
                        BpsTier::Red    => t.bear,
                    };
                    stale_color = if r.staleness_pct > 0.5 { t.bear } else { t.dim };
                }
            }

            // Use the ticker string as the row id salt — stable per ETF.
            let id_salt = upper.as_str();
            PanelListRow::new(id_salt)
                .columns(&[
                    PanelColumn::left(&upper).color(t.text),
                    PanelColumn::right(&mark_str).color(t.text),
                    PanelColumn::right(&inav_str).color(t.text),
                    PanelColumn::right(&bps_str).color(bps_color),
                    PanelColumn::right(&stale_str).color(stale_color),
                ])
                .hoverable(false)
                .show(ui, t);
        }
    });
}

fn fmt_price(p: f64) -> String { if p > 0.0 { format!("{:.2}", p) } else { "—".into() } }
fn fmt_bps(b: f64) -> String { format!("{:+.1} bps", b) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bps_tier_calm_under_twenty() {
        assert_eq!(bps_tier(0.0),  BpsTier::Calm);
        assert_eq!(bps_tier(5.0),  BpsTier::Calm);
        assert_eq!(bps_tier(20.0), BpsTier::Calm);
        assert_eq!(bps_tier(-19.9), BpsTier::Calm);
    }

    #[test]
    fn bps_tier_yellow_between_twenty_and_fifty() {
        assert_eq!(bps_tier(21.0), BpsTier::Yellow);
        assert_eq!(bps_tier(50.0), BpsTier::Yellow);
        assert_eq!(bps_tier(-35.0), BpsTier::Yellow);
    }

    #[test]
    fn bps_tier_red_above_fifty() {
        assert_eq!(bps_tier(50.1),  BpsTier::Red);
        assert_eq!(bps_tier(-100.0), BpsTier::Red);
    }

    #[test]
    fn default_etfs_contains_core_set() {
        for sym in ["SPY", "QQQ", "IWM", "DIA", "HYG", "LQD", "XLF", "XLK"] {
            assert!(DEFAULT_ETFS.iter().any(|e| *e == sym), "missing {sym}");
        }
    }

    #[test]
    fn fmt_bps_includes_sign_and_unit() {
        assert!(fmt_bps(25.0).contains("+25.0"));
        assert!(fmt_bps(-25.0).contains("-25.0"));
        assert!(fmt_bps(0.0).ends_with("bps"));
    }
}
