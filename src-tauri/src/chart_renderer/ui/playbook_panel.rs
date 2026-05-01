//! Playbook panel — standalone sidebar for trade idea plays.

use egui;
use super::style::*;
use super::super::gpu::{Watchlist, Chart, Theme};

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !watchlist.playbook_panel_open { return; }

    egui::SidePanel::right("playbook_panel")
        .default_width(280.0)
        .min_width(240.0)
        .max_width(440.0)
        .resizable(true)
        .frame(panel_frame(t.toolbar_bg, t.toolbar_border))
        .show(ctx, |ui| {
            // Header with close button
            let header = ui.horizontal(|ui| {
                ui.set_min_height(26.0);
                ui.add(super::widgets::text::SectionLabel::new("PLAYBOOK").color(t.accent));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if close_button(ui, t.dim) { watchlist.playbook_panel_open = false; }
                });
            });
            let line_y = header.response.rect.max.y;
            ui.painter().line_segment(
                [egui::pos2(ui.min_rect().left(), line_y),
                 egui::pos2(ui.min_rect().right(), line_y)],
                egui::Stroke::new(1.0, color_alpha(t.toolbar_border, ALPHA_MUTED)));
            ui.add_space(GAP_SM);

            super::plays_panel::draw_content(ui, watchlist, panes, ap, t);
        });
}
