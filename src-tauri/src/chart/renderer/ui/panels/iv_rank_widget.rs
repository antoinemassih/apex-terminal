//! Small IV-rank widget — current ATM IV + percentile rank bar.
//!
//! Reads the `vol_surface`-backed projector via
//! `apex_data::live_state::get_or_fetch_iv_rank`. Embeddable inside the
//! options chain panel or wherever a 240×80 card fits.
//!
//! Visual: row 1 "<underlying>  ATM IV xx.x%  rank YY"
//!         row 2 horizontal bar with ticks at 25 / 50 / 75 and a marker
//!               glyph at the current rank.
//!         when `insufficient_history`, the bar is replaced by an
//!         "insufficient history (N samples)" line.

use egui;
use crate::chart_renderer::ui::style::tint;
use crate::ui_kit::sx::Tone;
use super::super::super::gpu::Theme;
use super::super::style::{
    alpha_muted, gap_2xs, gap_xs, stroke_std,
};
use crate::data::feeds::apex_data::live_state as projector;
use crate::data::feeds::apex_data::types::IvRankV2;

/// Default lookback in sessions (≈ 1 year). Matches the projector default but
/// passed explicitly so the URL is deterministic in tests.
pub(crate) const DEFAULT_LOOKBACK: u32 = 252;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IvRankState {
    Loading,
    Ready,
    InsufficientHistory,
}

pub(crate) fn classify(reading: &Option<IvRankV2>) -> IvRankState {
    match reading {
        None => IvRankState::Loading,
        Some(r) if r.insufficient_history => IvRankState::InsufficientHistory,
        Some(r) if r.n_samples == 0 && r.atm_iv == 0.0 => IvRankState::Loading,
        Some(_) => IvRankState::Ready,
    }
}

/// Render the widget into `ui` for the given `underlying`. Triggers a fetch
/// on each call (TTL-gated, see `live_state.rs`).
pub(crate) fn show(ui: &mut egui::Ui, underlying: &str, t: &Theme) {
    if !underlying.is_empty() {
        projector::get_or_fetch_iv_rank(underlying, Some(DEFAULT_LOOKBACK));
    }
    let reading = projector::get_iv_rank(underlying);
    show_with(ui, underlying, reading.as_ref(), t);
}

/// Pure render path with the reading pre-fetched. Easier to test.
pub(crate) fn show_with(ui: &mut egui::Ui, underlying: &str, reading: Option<&IvRankV2>, t: &Theme) {
    let header = match reading {
        Some(r) => format!(
            "{}  ATM IV {:.1}%  rank {:.0}",
            underlying.to_uppercase(), r.atm_iv * 100.0, r.iv_rank
        ),
        None => format!("{}  ATM IV --  rank --", underlying.to_uppercase()),
    };
    ui.label(egui::RichText::new(header).color(t.text).monospace());

    let state = classify(&reading.cloned());
    match state {
        IvRankState::Loading => {
            ui.add_space(gap_xs());
            ui.label(egui::RichText::new("loading…").color(t.dim));
        }
        IvRankState::InsufficientHistory => {
            ui.add_space(gap_xs());
            let n = reading.map(|r| r.n_samples).unwrap_or(0);
            ui.label(
                egui::RichText::new(format!("insufficient history ({n} samples)"))
                    .color(t.dim)
                    .italics(),
            );
        }
        IvRankState::Ready => {
            let r = reading.expect("Ready implies Some");
            ui.add_space(gap_xs());
            draw_bar(ui, r.iv_rank.clamp(0.0, 100.0) as f32, t);
            ui.add_space(gap_2xs());
            ui.label(
                egui::RichText::new(format!("n={} · lookback={}", r.n_samples, r.lookback))
                    .color(t.dim)
                    .small(),
            );
        }
    }
}

/// Paints a percentile-rank bar with ticks at 25 / 50 / 75.
///
/// Kept custom (not replaced by `ui_kit::Progress`) because:
/// - `Progress` has no tick-marker API.
/// - The fill colour changes semantically with rank (bull ≤ 25, bear ≥ 75,
///   accent otherwise) — `Progress` only expresses `Primary` (accent) or
///   `Danger` (bear).
/// All colours and stroke widths route through `Theme` fields and style tokens.
fn draw_bar(ui: &mut egui::Ui, rank_0_100: f32, t: &Theme) {
    let avail_w = ui.available_width().min(220.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(avail_w, 10.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let cr = crate::ui_kit::style::r_xs_cr();
    // Background track — dim at muted alpha (~25% opacity).
    painter.rect_filled(rect, cr, tint(t, Tone::Dim, alpha_muted()));
    // Filled portion — semantic colour by rank zone.
    let fill_w = (rank_0_100 / 100.0) * rect.width();
    let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height()));
    let bar_color = if rank_0_100 >= 75.0 { t.bear }
        else if rank_0_100 <= 25.0 { t.bull }
        else { t.accent };
    painter.rect_filled(fill_rect, cr, bar_color);
    // Ticks at 25 / 50 / 75 — hairline rule in dim.
    for tick_pct in [25.0_f32, 50.0, 75.0] {
        let x = rect.min.x + (tick_pct / 100.0) * rect.width();
        painter.line_segment(
            [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
            egui::Stroke::new(stroke_std(), t.dim),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_none_is_loading() {
        assert_eq!(classify(&None), IvRankState::Loading);
    }

    #[test]
    fn classify_insufficient_history_when_flag_set() {
        let r = IvRankV2 { insufficient_history: true, n_samples: 10, atm_iv: 0.2, ..Default::default() };
        assert_eq!(classify(&Some(r)), IvRankState::InsufficientHistory);
    }

    #[test]
    fn classify_empty_reading_is_loading() {
        // Default-ish reading (n=0, iv=0) — projector hasn't responded yet.
        assert_eq!(classify(&Some(IvRankV2::default())), IvRankState::Loading);
    }

    #[test]
    fn classify_populated_is_ready() {
        let r = IvRankV2 { atm_iv: 0.245, iv_rank: 62.0, n_samples: 200, lookback: 252, ..Default::default() };
        assert_eq!(classify(&Some(r)), IvRankState::Ready);
    }

    #[test]
    fn classify_insufficient_overrides_populated() {
        let r = IvRankV2 { atm_iv: 0.245, iv_rank: 62.0, n_samples: 200, lookback: 252, insufficient_history: true, ..Default::default() };
        assert_eq!(classify(&Some(r)), IvRankState::InsufficientHistory);
    }
}
