//! Drawing properties bar — horizontal toolbar shown above the selected drawing.

use egui::Context;
use crate::chart_renderer::gpu::{Theme, Chart, DrawingAction, drawing_persist_key, drawing_to_db};
use crate::chart_renderer::{Drawing, DrawingKind, LineStyle};
use crate::chart_renderer::ui::style::{hex_to_color, color_alpha, COLOR_AMBER, gap_sm, gap_md, font_xs, font_sm, font_md};
use crate::ui_kit::icons::Icon;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Post-render actions that gpu.rs must apply after the call.
pub struct PropertiesBarOutput {
    /// User clicked the trash icon — remove the drawing.
    pub delete_sel: bool,
    /// User clicked "New Group…" — open the group manager.
    pub open_group_manager: bool,
}

/// Show the drawing properties bar for the currently selected drawing.
///
/// Mutates `chart` in-place (undo stack, drawing fields).
pub fn show_drawing_properties_bar(
    ctx: &Context,
    t: &Theme,
    chart: &mut Chart,
    pane_idx: usize,
) -> PropertiesBarOutput {
    let mut delete_sel = false;
    let mut open_group_manager = false;

    // Guard: we need a selected id and it must resolve to a drawing.
    let sel_id = match chart.selected_id.clone() {
        Some(id) => id,
        None => return PropertiesBarOutput { delete_sel, open_group_manager },
    };
    let sel_draw = match chart.drawings.iter().find(|d| d.id == sel_id).cloned() {
        Some(d) => d,
        None => return PropertiesBarOutput { delete_sel, open_group_manager },
    };

    let bar_y = 4.0; // caller adds rect.top() + pt — we receive the absolute Y externally.
    // NOTE: gpu.rs still computes the position; this fn only draws inside the Area.
    // We receive the absolute position via the Area anchor set in gpu.rs.
    // However to keep the widget self-contained we accept an offset.
    // gpu.rs passes `bar_x`/`bar_y` as the Area pos — we use a sentinel here
    // and let gpu.rs set it up via Area::fixed_pos before calling show_drawing_properties_bar_inner.
    // See call site in gpu.rs.

    let sym = drawing_persist_key(chart);
    let tf = chart.timeframe.clone();
    let dim = t.dim;

    // NOTE: This top-level function is a stub. gpu.rs calls show_drawing_properties_bar_ui
    // directly with an egui::Ui reference obtained from egui::Area::show + Frame::show.
    let _ = (bar_y, sym, tf, dim);
    PropertiesBarOutput { delete_sel, open_group_manager }
}

/// Inner painter — called by gpu.rs with the ui already inside an Area + Frame.
///
/// Returns the post-render output.
pub fn show_drawing_properties_bar_ui(
    ui: &mut egui::Ui,
    ctx: &Context,
    t: &Theme,
    chart: &mut Chart,
    pane_idx: usize,
) -> PropertiesBarOutput {
    let mut delete_sel = false;
    let mut open_group_manager = false;

    let sel_id = match chart.selected_id.clone() {
        Some(id) => id,
        None => return PropertiesBarOutput { delete_sel, open_group_manager },
    };
    let sel_draw = match chart.drawings.iter().find(|d| d.id == sel_id).cloned() {
        Some(d) => d,
        None => return PropertiesBarOutput { delete_sel, open_group_manager },
    };

    let sym = drawing_persist_key(chart);
    let tf = chart.timeframe.clone();
    let dim = t.dim;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap_sm();

        // Color swatches
        for hex in &["#4a9eff","#ff6b6b","#51cf66","#ffc125","#cc5de8","#ff922b","#ffffff","#82dcb4"] {
            let c = hex_to_color(hex, 1.0);
            let is_cur = sel_draw.color == *hex;
            let resp = ui.add(egui::Button::new("").fill(c).min_size(egui::vec2(18.0, 18.0)).corner_radius(3.0)
                .stroke(if is_cur { egui::Stroke::new(1.5, egui::Color32::WHITE) } else { egui::Stroke::NONE }));
            if resp.clicked() {
                if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                    chart.undo_stack.push(DrawingAction::Modify(d.id.clone(), d.clone()));
                    chart.redo_stack.clear();
                    d.color = hex.to_string();
                    crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                }
            }
        }

        ui.add(egui::Separator::default().spacing(gap_md()));

        // Style dropdown
        let style_label = match sel_draw.line_style { LineStyle::Solid => "Solid", LineStyle::Dashed => "Dashed", LineStyle::Dotted => "Dotted" };
        egui::ComboBox::from_id_salt(format!("style_{}", pane_idx))
            .selected_text(egui::RichText::new(style_label).monospace().size(font_sm()))
            .width(65.0)
            .show_ui(ui, |ui| {
                for (ls, label) in [(LineStyle::Solid, "Solid"), (LineStyle::Dashed, "Dashed"), (LineStyle::Dotted, "Dotted")] {
                    if ui.selectable_label(sel_draw.line_style == ls, egui::RichText::new(label).monospace().size(font_sm())).clicked() {
                        if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                            chart.undo_stack.push(DrawingAction::Modify(d.id.clone(), d.clone()));
                            chart.redo_stack.clear();
                            d.line_style = ls;
                            crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                        }
                    }
                }
            });

        // Thickness dropdown
        egui::ComboBox::from_id_salt(format!("thick_{}", pane_idx))
            .selected_text(egui::RichText::new(format!("{:.1}px", sel_draw.thickness)).monospace().size(font_sm()))
            .width(55.0)
            .show_ui(ui, |ui| {
                for &thick in &[0.5_f32, 1.0, 1.5, 2.0, 3.0, 4.0] {
                    if ui.selectable_label((sel_draw.thickness - thick).abs() < 0.05, egui::RichText::new(format!("{:.1}px", thick)).monospace().size(font_sm())).clicked() {
                        if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                            chart.undo_stack.push(DrawingAction::Modify(d.id.clone(), d.clone()));
                            chart.redo_stack.clear();
                            d.thickness = thick;
                            crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                        }
                    }
                }
            });

        // Opacity dropdown
        egui::ComboBox::from_id_salt(format!("opacity_{}", pane_idx))
            .selected_text(egui::RichText::new(format!("{}%", (sel_draw.opacity * 100.0) as i32)).monospace().size(font_sm()))
            .width(50.0)
            .show_ui(ui, |ui| {
                for &op in &[0.2_f32, 0.3, 0.5, 0.7, 0.85, 1.0] {
                    if ui.selectable_label((sel_draw.opacity - op).abs() < 0.05, egui::RichText::new(format!("{}%", (op * 100.0) as i32)).monospace().size(font_sm())).clicked() {
                        if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                            chart.undo_stack.push(DrawingAction::Modify(d.id.clone(), d.clone()));
                            chart.redo_stack.clear();
                            d.opacity = op;
                            crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                        }
                    }
                }
            });

        ui.add(egui::Separator::default().spacing(gap_md()));

        // Group dropdown
        let group_label = chart.groups.iter().find(|g| g.id == sel_draw.group_id).map_or("default".to_string(), |g| g.name.clone());
        egui::ComboBox::from_id_salt(format!("group_{}", pane_idx))
            .selected_text(egui::RichText::new(&group_label).monospace().size(font_sm()))
            .width(80.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(sel_draw.group_id == "default", egui::RichText::new("default").monospace().size(font_sm())).clicked() {
                    if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                        chart.undo_stack.push(DrawingAction::Modify(d.id.clone(), d.clone()));
                        chart.redo_stack.clear();
                        d.group_id = "default".into();
                        crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                    }
                }
                for g in &chart.groups.clone() {
                    if g.id == "default" { continue; }
                    if ui.selectable_label(sel_draw.group_id == g.id, egui::RichText::new(&g.name).monospace().size(font_sm())).clicked() {
                        if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                            chart.undo_stack.push(DrawingAction::Modify(d.id.clone(), d.clone()));
                            chart.redo_stack.clear();
                            d.group_id = g.id.clone();
                            crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                        }
                    }
                }
                ui.separator();
                if ui.selectable_label(false, egui::RichText::new(format!("{} New Group...", Icon::PLUS)).monospace().size(font_sm()).color(t.accent)).clicked() {
                    open_group_manager = true;
                }
            });

        ui.add(egui::Separator::default().spacing(gap_md()));

        // Extension toggles (lines only)
        if matches!(&sel_draw.kind, DrawingKind::TrendLine{..} | DrawingKind::Ray{..}) {
            if ui.add(egui::Button::new(egui::RichText::new("\u{2190}").monospace().size(font_md()).color(if sel_draw.extend_left { t.accent } else { dim })).fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(18.0, 18.0))).clicked() {
                if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                    chart.undo_stack.push(DrawingAction::Modify(d.id.clone(), d.clone()));
                    chart.redo_stack.clear();
                    d.extend_left = !d.extend_left;
                    crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                }
            }
            if ui.add(egui::Button::new(egui::RichText::new("\u{2192}").monospace().size(font_md()).color(if sel_draw.extend_right { t.accent } else { dim })).fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(18.0, 18.0))).clicked() {
                if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                    chart.undo_stack.push(DrawingAction::Modify(d.id.clone(), d.clone()));
                    chart.redo_stack.clear();
                    d.extend_right = !d.extend_right;
                    crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                }
            }
            ui.add(egui::Separator::default().spacing(gap_sm()));
        }

        // Lock
        if ui.add(egui::Button::new(egui::RichText::new(if sel_draw.locked { "Locked" } else { "Lock" }).monospace().size(font_sm()).color(if sel_draw.locked { t.accent } else { dim })).fill(egui::Color32::TRANSPARENT)).clicked() {
            if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) { d.locked = !d.locked; }
        }

        ui.add(egui::Separator::default().spacing(gap_md()));

        // Delete
        if ui.add(egui::Button::new(egui::RichText::new(Icon::TRASH).size(font_md()).color(t.bear)).fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(18.0, 18.0))).clicked() {
            if let Some(d) = chart.drawings.iter().find(|d| d.id == sel_id) {
                chart.undo_stack.push(DrawingAction::Remove(d.clone()));
            }
            crate::drawing_db::remove(&sel_id);
            chart.drawings.retain(|d| d.id != sel_id);
            chart.redo_stack.clear();
            delete_sel = true;
        }

        // Alert bell toggle
        ui.add(egui::Separator::default().spacing(gap_sm()));
        let has_alert = sel_draw.alert_enabled;
        let bell_col = if has_alert { COLOR_AMBER } else { dim };
        let bell_label = if has_alert { "\u{1F514} ON" } else { "\u{1F514}" };
        if ui.add(egui::Button::new(egui::RichText::new(bell_label).monospace().size(font_sm()).color(bell_col)).fill(egui::Color32::TRANSPARENT)).clicked() {
            if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                d.alert_enabled = !d.alert_enabled;
                let drawing_id = d.id.clone();
                let new_state = d.alert_enabled;
                std::thread::spawn(move || {
                    let url = format!("http://localhost:8100/drawings/{}/alert", drawing_id);
                    let body = format!("{{\"alert_enabled\":{}}}", new_state);
                    #[cfg(target_os = "windows")]
                    let _ = std::process::Command::new("curl")
                        .args(["-s", "-X", "PATCH", &url, "-H", "Content-Type: application/json", "-d", &body])
                        .creation_flags(0x08000000)
                        .output();
                    #[cfg(not(target_os = "windows"))]
                    let _ = std::process::Command::new("curl")
                        .args(["-s", "-X", "PATCH", &url, "-H", "Content-Type: application/json", "-d", &body])
                        .output();
                });
            }
        }
    });

    PropertiesBarOutput { delete_sel, open_group_manager }
}
