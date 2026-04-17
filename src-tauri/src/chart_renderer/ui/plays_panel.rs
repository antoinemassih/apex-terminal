//! Plays panel — shareable strategy cards (playbook).
//! Shows in the Feed sidebar under the "Plays" tab.
//! Each play is a visual card: ticker + entry + target + stop = one-click trade.

use egui;
use super::style::*;
use super::super::gpu::*;
use crate::chart_renderer::{Play, PlayDirection, PlayStatus};
use crate::ui_kit::icons::Icon;

/// Draw the plays tab content inside the Feed sidebar.
pub(crate) fn draw_content(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    t: &Theme,
) {
    ui.add_space(GAP_SM);

    // ── Header with "New Play" button ──
    ui.horizontal(|ui| {
        section_label(ui, &format!("PLAYBOOK ({})", watchlist.plays.len()), t.accent);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if small_action_btn(ui, "+ New Play", t.accent) {
                watchlist.play_editor_open = !watchlist.play_editor_open;
            }
        });
    });
    ui.add_space(GAP_SM);
    separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
    ui.add_space(GAP_SM);

    // ── Play editor (inline, toggle-able) ──
    if watchlist.play_editor_open {
        egui::Frame::NONE
            .fill(color_alpha(t.toolbar_border, ALPHA_FAINT))
            .inner_margin(egui::Margin::same(GAP_LG as i8))
            .corner_radius(RADIUS_MD)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("NEW PLAY").monospace().size(FONT_SM).strong().color(t.accent));
                ui.add_space(GAP_SM);

                // Direction toggle
                ui.horizontal(|ui| {
                    let is_long = watchlist.play_editor_direction == PlayDirection::Long;
                    let long_col = if is_long { t.bull } else { t.dim.gamma_multiply(0.5) };
                    let short_col = if !is_long { t.bear } else { t.dim.gamma_multiply(0.5) };
                    if ui.add(egui::Button::new(egui::RichText::new("LONG").monospace().size(FONT_SM).strong().color(long_col))
                        .fill(if is_long { color_alpha(t.bull, ALPHA_TINT) } else { egui::Color32::TRANSPARENT })
                        .corner_radius(RADIUS_SM).min_size(egui::vec2(50.0, 20.0))).clicked() {
                        watchlist.play_editor_direction = PlayDirection::Long;
                    }
                    if ui.add(egui::Button::new(egui::RichText::new("SHORT").monospace().size(FONT_SM).strong().color(short_col))
                        .fill(if !is_long { color_alpha(t.bear, ALPHA_TINT) } else { egui::Color32::TRANSPARENT })
                        .corner_radius(RADIUS_SM).min_size(egui::vec2(50.0, 20.0))).clicked() {
                        watchlist.play_editor_direction = PlayDirection::Short;
                    }
                });
                ui.add_space(GAP_XS);

                // Fields
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Symbol").monospace().size(FONT_XS).color(t.dim));
                    ui.add(egui::TextEdit::singleline(&mut watchlist.play_editor_symbol)
                        .desired_width(80.0).font(egui::FontId::monospace(FONT_SM)).hint_text("AAPL"));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Entry ").monospace().size(FONT_XS).color(t.dim));
                    ui.add(egui::TextEdit::singleline(&mut watchlist.play_editor_entry)
                        .desired_width(80.0).font(egui::FontId::monospace(FONT_SM)).hint_text("150.00"));
                    ui.label(egui::RichText::new("Target").monospace().size(FONT_XS).color(t.dim));
                    ui.add(egui::TextEdit::singleline(&mut watchlist.play_editor_target)
                        .desired_width(80.0).font(egui::FontId::monospace(FONT_SM)).hint_text("155.00"));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Stop  ").monospace().size(FONT_XS).color(t.dim));
                    ui.add(egui::TextEdit::singleline(&mut watchlist.play_editor_stop)
                        .desired_width(80.0).font(egui::FontId::monospace(FONT_SM)).hint_text("148.00"));
                });
                ui.add(egui::TextEdit::multiline(&mut watchlist.play_editor_notes)
                    .desired_rows(2).desired_width(ui.available_width())
                    .font(egui::FontId::monospace(FONT_SM)).hint_text("Strategy notes..."));
                ui.add_space(GAP_SM);

                // Create button
                ui.horizontal(|ui| {
                    let can_create = !watchlist.play_editor_symbol.trim().is_empty()
                        && watchlist.play_editor_entry.parse::<f32>().is_ok();
                    if action_btn(ui, "Create Play", t.accent, can_create) {
                        let entry = watchlist.play_editor_entry.parse::<f32>().unwrap_or(0.0);
                        let target = watchlist.play_editor_target.parse::<f32>().unwrap_or(entry * 1.02);
                        let stop = watchlist.play_editor_stop.parse::<f32>().unwrap_or(entry * 0.98);
                        let mut play = Play::new(
                            watchlist.play_editor_symbol.trim(),
                            watchlist.play_editor_direction,
                            entry, target, stop);
                        play.notes = watchlist.play_editor_notes.trim().to_string();
                        watchlist.plays.push(play);
                        // Clear editor
                        watchlist.play_editor_symbol.clear();
                        watchlist.play_editor_entry.clear();
                        watchlist.play_editor_target.clear();
                        watchlist.play_editor_stop.clear();
                        watchlist.play_editor_notes.clear();
                        watchlist.play_editor_open = false;
                    }
                    if small_action_btn(ui, "Cancel", t.dim) {
                        watchlist.play_editor_open = false;
                    }
                });
            });
        ui.add_space(GAP_LG);
        separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
        ui.add_space(GAP_SM);
    }

    // ── Play cards ──
    if watchlist.plays.is_empty() {
        ui.add_space(GAP_3XL);
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("No plays yet").monospace().size(FONT_SM).color(t.dim.gamma_multiply(0.5)));
            ui.add_space(GAP_SM);
            ui.label(egui::RichText::new("Create a play to share a trade idea")
                .monospace().size(FONT_XS).color(t.dim.gamma_multiply(0.3)));
        });
        return;
    }

    let mut remove_id: Option<String> = None;

    egui::ScrollArea::vertical().id_salt("plays_scroll").show(ui, |ui| {
        for play in &watchlist.plays {
            let is_long = play.direction == PlayDirection::Long;
            let dir_color = if is_long { t.bull } else { t.bear };
            let status_color = match play.status {
                PlayStatus::Draft => t.dim,
                PlayStatus::Published => t.accent,
                PlayStatus::Active => egui::Color32::from_rgb(255, 191, 0),
                PlayStatus::Won => t.bull,
                PlayStatus::Lost => t.bear,
                PlayStatus::Expired | PlayStatus::Cancelled => t.dim.gamma_multiply(0.5),
            };

            // ── Card ──
            order_card(ui, dir_color, color_alpha(t.toolbar_border, ALPHA_FAINT), |ui| {
                // Row 1: Direction badge + Symbol + Status + Delete
                ui.horizontal(|ui| {
                    // Direction pill
                    let dir_bg = color_alpha(dir_color, ALPHA_TINT);
                    ui.add(egui::Button::new(
                        egui::RichText::new(play.direction.label()).monospace().size(FONT_XS).strong().color(dir_color))
                        .fill(dir_bg).corner_radius(RADIUS_SM).min_size(egui::vec2(0.0, 14.0)));
                    // Symbol
                    ui.label(egui::RichText::new(&play.symbol).monospace().size(FONT_SM).strong().color(TEXT_PRIMARY));
                    // Status badge
                    status_badge(ui, play.status.label(), status_color);
                    // R:R
                    if play.risk_reward > 0.0 {
                        ui.label(egui::RichText::new(format!("{:.1}R", play.risk_reward))
                            .monospace().size(FONT_XS).color(t.accent));
                    }
                    // Delete
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_btn(ui, Icon::X, t.dim.gamma_multiply(0.4), FONT_SM).clicked() {
                            remove_id = Some(play.id.clone());
                        }
                    });
                });

                // Row 2: Entry / Target / Stop prices
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    // Entry
                    ui.label(egui::RichText::new("E").monospace().size(FONT_XS).color(t.dim));
                    ui.label(egui::RichText::new(format!("{:.2}", play.entry_price))
                        .monospace().size(FONT_SM).color(TEXT_PRIMARY));
                    ui.add_space(GAP_SM);
                    // Target
                    ui.label(egui::RichText::new("T").monospace().size(FONT_XS).color(t.bull));
                    ui.label(egui::RichText::new(format!("{:.2}", play.target_price))
                        .monospace().size(FONT_SM).color(t.bull));
                    ui.add_space(GAP_SM);
                    // Stop
                    ui.label(egui::RichText::new("S").monospace().size(FONT_XS).color(t.bear));
                    ui.label(egui::RichText::new(format!("{:.2}", play.stop_price))
                        .monospace().size(FONT_SM).color(t.bear));
                });

                // Row 3: Notes (if any)
                if !play.notes.is_empty() {
                    ui.label(egui::RichText::new(&play.notes)
                        .monospace().size(FONT_XS).color(t.dim.gamma_multiply(0.6)));
                }

                // Row 4: Tags
                if !play.tags.is_empty() {
                    ui.horizontal(|ui| {
                        for tag in &play.tags {
                            ui.add(egui::Button::new(
                                egui::RichText::new(tag).monospace().size(7.0).color(t.accent))
                                .fill(color_alpha(t.accent, ALPHA_FAINT))
                                .corner_radius(2.0).min_size(egui::vec2(0.0, 12.0)));
                        }
                    });
                }
            });
        }
    });

    if let Some(id) = remove_id {
        watchlist.plays.retain(|p| p.id != id);
    }
}
