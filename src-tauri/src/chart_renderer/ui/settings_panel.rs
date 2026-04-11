//! Settings panel — appearance, axes, font scale.

use egui;
use super::style::{color_alpha, dialog_window_themed, dialog_header, dialog_separator_shadow, dialog_section};
use super::super::gpu::{Watchlist, Theme};

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, t: &Theme) {
// ── Settings panel ──────────────────────────────────────────────────────
if watchlist.settings_open {
    let screen = ctx.screen_rect();
    dialog_window_themed(ctx, "settings_panel", egui::pos2(screen.center().x - 160.0, 60.0), 320.0, t.toolbar_bg, t.toolbar_border, None)
        .show(ctx, |ui| {
            if dialog_header(ui, "SETTINGS", t.dim) { watchlist.settings_open = false; }
            ui.add_space(8.0);
            let m = 10.0;

            // ── Appearance section ──
            dialog_section(ui, "APPEARANCE", m, t.dim.gamma_multiply(0.5));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Font Scale").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    // Display 60-160% maps to 0.96-2.56 ppp. 100% = 1.6 (baseline)
                    let display_pct = ((watchlist.font_scale - 0.96) / 0.016).round() as i32 + 60;
                    let mut dp = display_pct.clamp(60, 160);
                    if ui.add(egui::DragValue::new(&mut dp).range(60..=160).suffix("%").speed(1)
                        .custom_formatter(|v, _| format!("{}%", v as i32))).changed() {
                        watchlist.font_scale = 0.96 + (dp - 60) as f32 * 0.016;
                    }
                });
            });
            // Preset buttons (display % → internal ppp)
            ui.horizontal(|ui| {
                ui.add_space(m);
                // 100% = 1.6 ppp (baseline), 20% steps = 0.32 ppp each
                for (label, ppp) in [(60, 0.96_f32), (80, 1.28), (100, 1.6), (120, 1.92), (140, 2.24), (160, 2.56)] {
                    let active = (watchlist.font_scale - ppp).abs() < 0.05;
                    let fg = if active { t.accent } else { t.dim.gamma_multiply(0.6) };
                    let bg = if active { color_alpha(t.accent, 25) } else { egui::Color32::TRANSPARENT };
                    if ui.add(egui::Button::new(egui::RichText::new(format!("{}%", label)).monospace().size(9.0).color(fg))
                        .fill(bg).corner_radius(3.0).min_size(egui::vec2(32.0, 18.0))).clicked() {
                        watchlist.font_scale = ppp;
                    }
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Compact Mode").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut val = watchlist.compact_mode;
                    if ui.add(egui::Checkbox::without_text(&mut val)).changed() {
                        watchlist.compact_mode = val;
                    }
                });
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Auto-Hide Toolbar").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut val = watchlist.toolbar_auto_hide;
                    if ui.add(egui::Checkbox::without_text(&mut val)).changed() {
                        watchlist.toolbar_auto_hide = val;
                        if !val { watchlist.toolbar_hover_time = None; }
                    }
                });
            });
            ui.add_space(8.0);

            // ── Axes section ──
            dialog_section(ui, "AXES", m, t.dim.gamma_multiply(0.5));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Show X-Axis (time)").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut val = watchlist.show_x_axis;
                    if ui.add(egui::Checkbox::without_text(&mut val)).changed() {
                        watchlist.show_x_axis = val;
                    }
                });
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Show Y-Axis (price)").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut val = watchlist.show_y_axis;
                    if ui.add(egui::Checkbox::without_text(&mut val)).changed() {
                        watchlist.show_y_axis = val;
                    }
                });
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Shared X-Axis").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut val = watchlist.shared_x_axis;
                    if ui.add(egui::Checkbox::without_text(&mut val)).changed() {
                        watchlist.shared_x_axis = val;
                    }
                });
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Shared Y-Axis").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut val = watchlist.shared_y_axis;
                    if ui.add(egui::Checkbox::without_text(&mut val)).changed() {
                        watchlist.shared_y_axis = val;
                    }
                });
            });
            ui.add_space(8.0);
        });
}


}
