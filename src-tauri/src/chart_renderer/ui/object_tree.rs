//! Object tree panel — drawings, indicators, overlays management.

use egui;
use super::style::{close_button, separator, color_alpha, hex_to_color};
use super::super::gpu::{Watchlist, Chart, Theme, DrawingAction, drawing_kind_short};
use super::super::{Drawing, DrawingKind};
use crate::ui_kit::icons::Icon;

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme) {
// ── Object Tree side panel ─────────────────────────────────────────────────
if watchlist.object_tree_open {
    egui::SidePanel::right("object_tree")
        .default_width(200.0)
        .min_width(160.0)
        .max_width(300.0)
        .resizable(true)
        .frame(egui::Frame::NONE.fill(t.toolbar_bg)
            .inner_margin(egui::Margin { left: 6, right: 6, top: 6, bottom: 6 })
            .stroke(egui::Stroke::new(1.0, color_alpha(t.toolbar_border, 80))))
        .show(ctx, |ui| {
            let panel_w = ui.available_width();
            ui.set_max_width(panel_w);
            // Header
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("OBJECTS").monospace().size(11.0).strong().color(t.accent));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if close_button(ui, t.dim) { watchlist.object_tree_open = false; }
                });
            });
            ui.add_space(4.0);

            let chart = &mut panes[ap];

            // ── DRAWINGS section ──
            ui.label(egui::RichText::new("DRAWINGS").monospace().size(8.0).color(t.dim));
            ui.add_space(2.0);
            if chart.drawings.is_empty() {
                ui.label(egui::RichText::new("  No drawings").monospace().size(8.0).color(t.dim.gamma_multiply(0.5)));
            } else {
                let mut del_id: Option<String> = None;
                for d in chart.drawings.iter_mut() {
                    let kind_name = drawing_kind_short(&d.kind);
                    let dc = hex_to_color(&d.color, 1.0);
                    let hidden = chart.hidden_groups.contains(&d.group_id);
                    ui.horizontal(|ui| {
                        ui.set_height(18.0);
                        ui.spacing_mut().item_spacing.x = 2.0;
                        // Color dot
                        ui.painter().circle_filled(
                            egui::pos2(ui.cursor().min.x + 5.0, ui.cursor().min.y + 9.0), 3.0, dc);
                        ui.add_space(12.0);
                        // Kind label
                        ui.label(egui::RichText::new(kind_name).monospace().size(8.0).color(
                            if hidden { t.dim.gamma_multiply(0.3) } else { egui::Color32::from_white_alpha(180) }));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            // Delete button
                            if ui.add(egui::Button::new(
                                egui::RichText::new(Icon::TRASH).size(8.0).color(egui::Color32::from_rgb(224, 85, 96)))
                                .frame(false).min_size(egui::vec2(16.0, 16.0))).clicked() {
                                del_id = Some(d.id.clone());
                            }
                            // Eye toggle
                            let eye_icon = if hidden { Icon::EYE_SLASH } else { Icon::EYE };
                            let eye_col = if hidden { t.dim.gamma_multiply(0.3) } else { t.dim };
                            if ui.add(egui::Button::new(
                                egui::RichText::new(eye_icon).size(8.0).color(eye_col))
                                .frame(false).min_size(egui::vec2(16.0, 16.0))).clicked() {
                                let gid = d.group_id.clone();
                                if hidden {
                                    chart.hidden_groups.retain(|g| g != &gid);
                                } else if !chart.hidden_groups.contains(&gid) {
                                    chart.hidden_groups.push(gid);
                                }
                            }
                        });
                    });
                }
                if let Some(id) = del_id {
                    if let Some(d) = chart.drawings.iter().find(|d| d.id == id) {
                        chart.undo_stack.push(DrawingAction::Remove(d.clone()));
                    }
                    crate::drawing_db::remove(&id);
                    chart.drawings.retain(|d| d.id != id);
                    chart.redo_stack.clear();
                    if chart.selected_id.as_deref() == Some(&id) { chart.selected_id = None; }
                    chart.selected_ids.retain(|s| s != &id);
                }
            }

            ui.add_space(6.0);
            ui.add(egui::Separator::default().spacing(2.0));
            ui.add_space(4.0);

            // ── INDICATORS section ──
            ui.label(egui::RichText::new("INDICATORS").monospace().size(8.0).color(t.dim));
            ui.add_space(2.0);
            if chart.indicators.is_empty() {
                ui.label(egui::RichText::new("  No indicators").monospace().size(8.0).color(t.dim.gamma_multiply(0.5)));
            } else {
                let mut edit_ind: Option<u32> = None;
                for ind in chart.indicators.iter_mut() {
                    let ic = hex_to_color(&ind.color, 1.0);
                    ui.horizontal(|ui| {
                        ui.set_height(18.0);
                        ui.spacing_mut().item_spacing.x = 2.0;
                        // Color dot
                        ui.painter().circle_filled(
                            egui::pos2(ui.cursor().min.x + 5.0, ui.cursor().min.y + 9.0), 3.0, ic);
                        ui.add_space(12.0);
                        // Name + period
                        let label = format!("{} {}", ind.kind.label(), ind.period);
                        let label_resp = ui.label(egui::RichText::new(&label).monospace().size(8.0).color(
                            if ind.visible { egui::Color32::from_white_alpha(180) } else { t.dim.gamma_multiply(0.3) }));
                        if label_resp.clicked() { edit_ind = Some(ind.id); }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            // Eye toggle
                            let eye_icon = if ind.visible { Icon::EYE } else { Icon::EYE_SLASH };
                            let eye_col = if ind.visible { t.dim } else { t.dim.gamma_multiply(0.3) };
                            if ui.add(egui::Button::new(
                                egui::RichText::new(eye_icon).size(8.0).color(eye_col))
                                .frame(false).min_size(egui::vec2(16.0, 16.0))).clicked() {
                                ind.visible = !ind.visible;
                            }
                        });
                    });
                }
                if let Some(id) = edit_ind {
                    chart.editing_indicator = Some(id);
                }
            }

            ui.add_space(6.0);
            ui.add(egui::Separator::default().spacing(2.0));
            ui.add_space(4.0);

            // ── OVERLAYS section ──
            ui.label(egui::RichText::new("OVERLAYS").monospace().size(8.0).color(t.dim));
            ui.add_space(2.0);
            if chart.symbol_overlays.is_empty() {
                ui.label(egui::RichText::new("  No overlays").monospace().size(8.0).color(t.dim.gamma_multiply(0.5)));
            } else {
                let mut del_ov: Option<usize> = None;
                let mut toggle_ov: Option<usize> = None;
                // Snapshot data for iteration to avoid borrow conflicts
                let ov_snap: Vec<(String, String, bool)> = chart.symbol_overlays.iter()
                    .map(|ov| (ov.symbol.clone(), ov.color.clone(), ov.visible)).collect();
                for (oi, (sym, color, vis)) in ov_snap.iter().enumerate() {
                    let oc = hex_to_color(color, 1.0);
                    ui.horizontal(|ui| {
                        ui.set_height(18.0);
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.painter().circle_filled(
                            egui::pos2(ui.cursor().min.x + 5.0, ui.cursor().min.y + 9.0), 3.0, oc);
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new(sym).monospace().size(8.0).color(
                            if *vis { egui::Color32::from_white_alpha(180) } else { t.dim.gamma_multiply(0.3) }));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            if ui.add(egui::Button::new(
                                egui::RichText::new(Icon::TRASH).size(8.0).color(egui::Color32::from_rgb(224, 85, 96)))
                                .frame(false).min_size(egui::vec2(16.0, 16.0))).clicked() {
                                del_ov = Some(oi);
                            }
                            let eye_icon = if *vis { Icon::EYE } else { Icon::EYE_SLASH };
                            let eye_col = if *vis { t.dim } else { t.dim.gamma_multiply(0.3) };
                            if ui.add(egui::Button::new(
                                egui::RichText::new(eye_icon).size(8.0).color(eye_col))
                                .frame(false).min_size(egui::vec2(16.0, 16.0))).clicked() {
                                toggle_ov = Some(oi);
                            }
                        });
                    });
                }
                if let Some(idx) = toggle_ov {
                    chart.symbol_overlays[idx].visible = !chart.symbol_overlays[idx].visible;
                }
                if let Some(idx) = del_ov {
                    chart.symbol_overlays.remove(idx);
                }
            }
        });
}


}
