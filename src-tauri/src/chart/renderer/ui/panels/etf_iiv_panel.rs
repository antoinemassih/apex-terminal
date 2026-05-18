//! ETF IIV (indicative intraday NAV) panel.
//!
//! Lists the curated ETF universe with per-row market price / indicative NAV /
//! premium-discount in basis points / staleness%. Refreshes every 10s via
//! `apex_data::live_state::get_or_fetch_etf_iiv` (TTL is also 10s in the cache).
//!
//! Color thresholds for the premium/discount cell:
//!   |bps| > 50 → bear (red), > 20 → yellow, else dim text.

use egui;
use super::super::super::gpu::Theme;
use crate::data::feeds::apex_data::live_state as projector;
use crate::data::feeds::apex_data::types::EtfIivReading;

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

    let yellow = egui::Color32::from_rgb(0xE6, 0xB8, 0x00);

    egui::Grid::new("etf_iiv_grid")
        .num_columns(5)
        .spacing(egui::vec2(12.0, 4.0))
        .striped(true)
        .show(ui, |ui| {
            // Header
            for h in ["ETF", "Mark", "iNAV", "Prem/Disc", "Stale%"] {
                ui.label(egui::RichText::new(h).color(t.dim).strong().small());
            }
            ui.end_row();

            for e in etfs {
                let upper = e.to_uppercase();
                let reading = projector::get_etf_iiv(&upper);
                render_row(ui, &upper, reading.as_ref(), t, yellow);
                ui.end_row();
            }
        });
}

fn render_row(ui: &mut egui::Ui, etf: &str, r: Option<&EtfIivReading>, t: &Theme, yellow: egui::Color32) {
    ui.label(egui::RichText::new(etf).color(t.text).monospace());
    match r {
        None => {
            for _ in 0..4 { ui.label(egui::RichText::new("—").color(t.dim)); }
        }
        Some(r) => {
            ui.label(egui::RichText::new(fmt_price(r.market_price)).color(t.text).monospace());
            ui.label(egui::RichText::new(fmt_price(r.iiv)).color(t.text).monospace());
            let tier = bps_tier(r.premium_disc_bps);
            let color = match tier {
                BpsTier::Calm   => t.dim,
                BpsTier::Yellow => yellow,
                BpsTier::Red    => t.bear,
            };
            ui.label(egui::RichText::new(fmt_bps(r.premium_disc_bps)).color(color).monospace());
            let stale_col = if r.staleness_pct > 0.5 { t.bear } else { t.dim };
            ui.label(egui::RichText::new(format!("{:.0}%", r.staleness_pct * 100.0)).color(stale_col).monospace());
        }
    }
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
