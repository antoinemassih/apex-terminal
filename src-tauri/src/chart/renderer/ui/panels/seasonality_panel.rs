//! Seasonality panel — historical seasonal patterns for the active symbol.
//!
//! Always embedded via `draw_content` inside `analysis_panel`'s
//! [`SplitSectionPanel`] (Seasonality tab) — never standalone. Body
//! primitives only:
//!   - `PanelSection` for the chart section ("SEASONALITY — SYM") and the
//!     WIN RATE BY MONTH list.
//!   - `PanelEmpty` for the "not enough data" guard.
//!   - The lollipop month-return chart + win-rate mini-bars keep their
//!     bespoke painting — no canonical primitive maps to them yet.

use egui;
use crate::ui_kit::sx::Tone;
use super::super::style::*;
use super::super::super::gpu::{Watchlist, Chart, Theme};
use super::super::components::text::{DimLabel, MonospaceCode};
use crate::ui_kit::widgets::{PanelEmpty, PanelSection};
use crate::ui_kit::icons::Icon;

pub(crate) fn draw_content(
    ui: &mut egui::Ui,
    _watchlist: &mut Watchlist,
    panes: &[Chart],
    ap: usize,
    t: &Theme,
) {
    if panes.is_empty() { return; }
    let sym = panes[ap].symbol.clone();
    let bars = &panes[ap].bars;
    let timestamps = &panes[ap].timestamps;

    PanelSection::new(&format!("SEASONALITY — {}", sym))
        .show(ui, t, |ui, t| {
            if bars.len() < 252 || timestamps.len() < bars.len() {
                PanelEmpty::new("Insufficient data")
                    .glyph(Icon::CALENDAR_BLANK)
                    .hint("Need at least 1 year of bars")
                    .show(ui, t);
                return;
            }

            // Compute monthly average returns from bars
            let mut month_returns: [Vec<f32>; 12] = Default::default();
            for i in 1..bars.len() {
                let ts = timestamps[i];
                let days = ts / 86400;
                let month = estimate_month(days);
                let ret = if bars[i - 1].close > 0.0 {
                    (bars[i].close - bars[i - 1].close) / bars[i - 1].close * 100.0
                } else { 0.0 };
                month_returns[month as usize % 12].push(ret);
            }

            let month_labels = ["Jan", "Feb", "Mar", "Apr", "May", "Jun",
                                "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

            let avgs: Vec<f32> = month_returns.iter().map(|rets| {
                if rets.is_empty() { 0.0 }
                else { rets.iter().sum::<f32>() / rets.len() as f32 }
            }).collect();

            let max_abs = avgs.iter().map(|v| v.abs()).fold(0.0f32, f32::max).max(0.01);

            // Bar chart
            let bar_w = ui.available_width();
            let chart_h = 140.0;
            let (chart_rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, chart_h), egui::Sense::hover());
            let p = ui.painter();
            p.rect_filled(chart_rect, radius_sm(), tint(t, Tone::Border, alpha_faint()));

            let mid_y = chart_rect.center().y;
            p.line_segment(
                [egui::pos2(chart_rect.left(), mid_y), egui::pos2(chart_rect.right(), mid_y)],
                egui::Stroke::new(stroke_thin(), tint(t, Tone::Dim, alpha_muted())));

            let col_w = bar_w / 12.0;
            let scale = (chart_h * 0.4) / max_abs;

            let now_days = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64 / 86400;
            let cur_month = estimate_month(now_days) as usize % 12;

            for (i, avg) in avgs.iter().enumerate() {
                let cx = chart_rect.left() + col_w * (i as f32 + 0.5);
                let bar_h = avg * scale;
                let color = if *avg >= 0.0 { t.bull } else { t.bear };
                let is_current = i == cur_month;

                let tip_y = mid_y - bar_h;
                let line_alpha = if is_current { 220u8 } else { 120 };
                let circle_r = if is_current { 4.5 } else { 3.0 };

                p.line_segment(
                    [egui::pos2(cx, mid_y), egui::pos2(cx, tip_y)],
                    egui::Stroke::new(if is_current { stroke_extra_thick() } else { stroke_bold() },
                        color_alpha(color, line_alpha)));
                p.circle_filled(egui::pos2(cx, tip_y), circle_r, color);

                let label_y = if *avg >= 0.0 { tip_y - 8.0 } else { tip_y + 8.0 };
                p.text(egui::pos2(cx, label_y), egui::Align2::CENTER_CENTER,
                    &format!("{:+.1}%", avg), mono_sm(),
                    if is_current { color } else { color_muted(color) });

                let month_col = if is_current { t.accent } else { color_half(t.dim) };
                p.text(egui::pos2(cx, chart_rect.bottom() - 6.0), egui::Align2::CENTER_CENTER,
                    month_labels[i], mono_sm(), month_col);
            }

            ui.add_space(gap_md());

            // Current month highlight + best/worst
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
            let current_month = estimate_month(now_secs / 86400) as usize % 12;

            ui.horizontal(|ui| {
                ui.add(DimLabel::new("Current month:").color(t.dim));
                let avg = avgs[current_month];
                let col = if avg >= 0.0 { t.bull } else { t.bear };
                ui.add(MonospaceCode::new(&format!("{} avg {:+.2}%", month_labels[current_month], avg)).sm().color(col).strong(true));
            });
            ui.add_space(gap_sm());

            let best = avgs.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or((0, &0.0));
            let worst = avgs.iter().enumerate().min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or((0, &0.0));
            ui.horizontal(|ui| {
                ui.add(DimLabel::new("Best:").color(t.dim));
                ui.add(MonospaceCode::new(&format!("{} {:+.2}%", month_labels[best.0], best.1)).xs().color(t.bull));
                ui.add_space(gap_md());
                ui.add(DimLabel::new("Worst:").color(t.dim));
                ui.add(MonospaceCode::new(&format!("{} {:+.2}%", month_labels[worst.0], worst.1)).xs().color(t.bear));
            });

            // Win rate per month → its own sub-section in the body
            ui.add_space(gap_sm());
            PanelSection::new("WIN RATE BY MONTH").rule(false).show(ui, t, |ui, t| {
                for (i, rets) in month_returns.iter().enumerate() {
                    if rets.is_empty() { continue; }
                    let wins = rets.iter().filter(|r| **r > 0.0).count();
                    let total = rets.len();
                    let pct = wins as f32 / total as f32 * 100.0;
                    let col = if pct >= 60.0 { t.bull } else if pct >= 40.0 { t.dim } else { t.bear };

                    ui.horizontal(|ui| {
                        ui.add(MonospaceCode::new(month_labels[i]).xs().color(t.dim));
                        let bar_max = 60.0;
                        let (r, _) = ui.allocate_exact_size(egui::vec2(bar_max, 8.0), egui::Sense::hover());
                        let pp = ui.painter();
                        pp.rect_filled(r, 2.0, tint(t, Tone::Border, alpha_faint()));
                        pp.rect_filled(
                            egui::Rect::from_min_size(r.min, egui::vec2(bar_max * pct / 100.0, 8.0)),
                            2.0, color_alpha(col, alpha_dim()));
                        ui.add(MonospaceCode::new(&format!("{:.0}% ({}/{})", pct, wins, total)).xs().color(col));
                    });
                }
            });
        });
}

/// Rough month estimation from days since epoch.
fn estimate_month(days: i64) -> i32 {
    let y = (days as f64 / 365.25) as i64 + 1970;
    let day_of_year = days - ((y - 1970) as f64 * 365.25) as i64;
    let cum = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334, 365];
    for m in 0..12 {
        if day_of_year < cum[m + 1] { return m as i32; }
    }
    11
}
