//! Trade Journal placeholder panel — coming soon.

use egui;
use super::style::*;
use super::super::gpu::{Watchlist, Theme};

const PLANNED_FEATURES: &[(&str, &str)] = &[
    ("\u{2022}", "Auto-log trades from IB"),
    ("\u{2022}", "Entry/exit with chart snapshot"),
    ("\u{2022}", "P&L tracking per trade"),
    ("\u{2022}", "Win rate, profit factor, expectancy"),
    ("\u{2022}", "Filter by: symbol, strategy, date range"),
    ("\u{2022}", "Tags and notes per trade"),
    ("\u{2022}", "Equity curve visualization"),
    ("\u{2022}", "Performance by day of week / time of day"),
];

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, t: &Theme) {
    if !watchlist.journal_open { return; }

    let mut close = false;
    egui::Window::new("trade_journal")
        .default_pos(egui::pos2(350.0, 120.0))
        .default_size(egui::vec2(320.0, 420.0))
        .resizable(true)
        .movable(true)
        .title_bar(false)
        .frame(egui::Frame::popup(&ctx.style())
            .fill(t.toolbar_bg)
            .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })
            .stroke(egui::Stroke::new(STROKE_STD, color_alpha(t.toolbar_border, ALPHA_HEAVY)))
            .corner_radius(RADIUS_LG))
        .show(ctx, |ui| {
            let w = ui.available_width();

            // ── Header ──────────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                ui.label(egui::RichText::new("TRADE JOURNAL")
                    .monospace().size(11.0).strong().color(t.accent));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(6.0);
                    if close_button(ui, t.dim) { close = true; }
                });
            });
            ui.add_space(4.0);

            // Divider
            let div_rect = egui::Rect::from_min_size(
                egui::pos2(ui.cursor().min.x, ui.cursor().min.y),
                egui::vec2(w, 1.0),
            );
            ui.painter().rect_filled(div_rect, 0.0, color_alpha(t.toolbar_border, ALPHA_DIM));
            ui.add_space(12.0);

            // ── Coming Soon content ─────────────────────────────────────
            egui::ScrollArea::vertical()
                .id_salt("journal_content")
                .show(ui, |ui| {
                    ui.set_min_width(w - 4.0);
                    let m = 14.0;

                    // Badge
                    ui.vertical_centered(|ui| {
                        let badge_text = "COMING SOON";
                        let badge_rect = ui.allocate_space(egui::vec2(100.0, 22.0)).1;
                        ui.painter().rect_filled(badge_rect, 4.0, color_alpha(t.accent, ALPHA_SOFT));
                        ui.painter().rect_stroke(badge_rect, 4.0, egui::Stroke::new(STROKE_THIN, color_alpha(t.accent, ALPHA_DIM)), egui::StrokeKind::Outside);
                        ui.painter().text(
                            badge_rect.center(), egui::Align2::CENTER_CENTER,
                            badge_text, egui::FontId::monospace(9.0), t.accent,
                        );
                    });

                    ui.add_space(12.0);

                    // Description
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new("Track every trade with automatic logging,")
                            .monospace().size(9.0).color(t.dim.gamma_multiply(0.7)));
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new("chart snapshots, and performance analytics.")
                            .monospace().size(9.0).color(t.dim.gamma_multiply(0.7)));
                    });

                    ui.add_space(14.0);

                    // Section header
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new("PLANNED FEATURES")
                            .monospace().size(8.0).strong().color(t.dim.gamma_multiply(0.5)));
                    });
                    ui.add_space(6.0);

                    // Feature list
                    for (bullet, feature) in PLANNED_FEATURES {
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            ui.label(egui::RichText::new(*bullet)
                                .monospace().size(9.0).color(t.accent.gamma_multiply(0.6)));
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new(*feature)
                                .monospace().size(9.0).color(t.dim.gamma_multiply(0.8)));
                        });
                        ui.add_space(2.0);
                    }

                    ui.add_space(20.0);
                });
        });
    if close { watchlist.journal_open = false; }
}
