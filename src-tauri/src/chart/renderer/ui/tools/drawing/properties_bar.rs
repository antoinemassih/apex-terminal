//! Drawing properties bar — horizontal toolbar shown above the selected drawing.

use egui::Context;
use crate::chart_renderer::gpu::{Theme, Chart, DrawingAction, drawing_persist_key, drawing_to_db};
use crate::chart_renderer::{DrawingKind, LineStyle};
use crate::chart_renderer::ui::style::{hex_to_color, COLOR_AMBER, gap_xs, font_xs, font_md, row_height_compact, row_height_dense, stroke_bold, radius_sm};
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::{Button as KitButton, Tooltip, tokens::{Variant as KitVariant, Size as KitSize}};
use crate::ui_kit::widgets::icon_placement::IconPlacement;
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
    let delete_sel = false;
    let open_group_manager = false;

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
        // Tight density: swatches and dropdowns sit close together; separators
        // get the smaller gap so the bar matches the original compact look.
        ui.spacing_mut().item_spacing.x = gap_xs();
        ui.spacing_mut().button_padding = egui::vec2(gap_xs(), 2.0);

        // ── Color swatch — single button showing the current color, opens a
        //    grid of preset swatches in a popup. ──
        const SWATCHES: &[&str] = &[
            "#4a9eff", "#ff6b6b", "#51cf66", "#ffc125",
            "#cc5de8", "#ff922b", "#ffffff", "#82dcb4",
        ];
        let cur_color = hex_to_color(&sel_draw.color, 1.0);
        // The trigger glyph (U+25A0 filled square) is colored to the current
        // drawing color via Button::menu's fg override — a colored text glyph
        // IS the swatch, so no dedicated Button::swatch API is needed.
        let color_menu = KitButton::menu("\u{25A0}")
            .glyph_size(font_md() + 2.0)
            .fg(cur_color)
            .show_menu(ui, t, |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;
                ui.spacing_mut().item_spacing = egui::vec2(gap_xs(), gap_xs());
                ui.label(egui::RichText::new("COLOR").monospace().size(font_xs()).color(t.dim));
                ui.horizontal(|ui| {
                    for hex in SWATCHES {
                        let c = hex_to_color(hex, 1.0);
                        let is_cur = sel_draw.color == *hex;
                        // Chrome variant: swatch color fills the button surface.
                        let resp = KitButton::new("").variant(KitVariant::Chrome)
                            .fill(c)
                            .min_size(egui::vec2(20.0, row_height_compact()))
                            .corner_radius(radius_sm())
                            .stroke(if is_cur { egui::Stroke::new(stroke_bold(), t.text) } else { egui::Stroke::NONE })
                            .show(ui, t);
                        if resp.clicked() {
                            if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                                chart.undo_stack.push(DrawingAction::Modify(d.id.clone(), d.clone()));
                                chart.redo_stack.clear();
                                d.color = hex.to_string();
                                crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                            }
                            ui.close_menu();
                        }
                    }
                });
            });
        Tooltip::new("Color").show(ui, &color_menu.response, t);

        ui.add(egui::Separator::default().spacing(gap_xs()));

        // Style dropdown — `ui_kit::widgets::Select` (replaces hand-built ComboBox).
        {
            const STYLE_OPTS: &[&str] = &["Solid", "Dashed", "Dotted"];
            let mut idx: usize = match sel_draw.line_style {
                LineStyle::Solid => 0,
                LineStyle::Dashed => 1,
                LineStyle::Dotted => 2,
            };
            let prev_idx = idx;
            crate::ui_kit::widgets::Select::new(&mut idx, STYLE_OPTS).show(ui, t);
            if idx != prev_idx {
                let new_style = match idx {
                    0 => LineStyle::Solid,
                    1 => LineStyle::Dashed,
                    _ => LineStyle::Dotted,
                };
                if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                    chart.undo_stack.push(DrawingAction::Modify(d.id.clone(), d.clone()));
                    chart.redo_stack.clear();
                    d.line_style = new_style;
                    crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                }
            }
        }

        // Thickness dropdown — `ui_kit::widgets::Select`.
        {
            const THICK_VALS: &[f32] = &[0.5, 1.0, 1.5, 2.0, 3.0, 4.0];
            const THICK_LABELS: &[&str] = &["0.5px", "1.0px", "1.5px", "2.0px", "3.0px", "4.0px"];
            let mut idx: usize = THICK_VALS
                .iter()
                .position(|&v| (sel_draw.thickness - v).abs() < 0.05)
                .unwrap_or(1);
            let prev_idx = idx;
            crate::ui_kit::widgets::Select::new(&mut idx, THICK_LABELS).show(ui, t);
            if idx != prev_idx {
                let new_thick = THICK_VALS[idx];
                if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                    chart.undo_stack.push(DrawingAction::Modify(d.id.clone(), d.clone()));
                    chart.redo_stack.clear();
                    d.thickness = new_thick;
                    crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                }
            }
        }

        // Opacity dropdown — `ui_kit::widgets::Select`.
        {
            const OP_VALS: &[f32] = &[0.2, 0.3, 0.5, 0.7, 0.85, 1.0];
            const OP_LABELS: &[&str] = &["20%", "30%", "50%", "70%", "85%", "100%"];
            let mut idx: usize = OP_VALS
                .iter()
                .position(|&v| (sel_draw.opacity - v).abs() < 0.05)
                .unwrap_or(OP_VALS.len() - 1);
            let prev_idx = idx;
            crate::ui_kit::widgets::Select::new(&mut idx, OP_LABELS).show(ui, t);
            if idx != prev_idx {
                let new_op = OP_VALS[idx];
                if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                    chart.undo_stack.push(DrawingAction::Modify(d.id.clone(), d.clone()));
                    chart.redo_stack.clear();
                    d.opacity = new_op;
                    crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                }
            }
        }

        ui.add(egui::Separator::default().spacing(gap_xs()));

        // Group dropdown — unified DropdownOwned. The trailing sentinel
        // "__new_group__" option acts as the "+ New Group…" action footer.
        const NEW_GROUP_SENTINEL: &str = "__new_group__";
        let mut grp_opts: Vec<(String, String)> = vec![("default".to_string(), "default".to_string())];
        for g in &chart.groups {
            if g.id != "default" {
                grp_opts.push((g.id.clone(), g.name.clone()));
            }
        }
        grp_opts.push((NEW_GROUP_SENTINEL.to_string(), format!("{} New Group...", Icon::PLUS)));
        let mut grp_sel = sel_draw.group_id.clone();
        if crate::chart_renderer::ui::inputs::select::DropdownOwned::new()
            .options(grp_opts)
            .width(80.0)
            .theme(t)
            .show(ui, &mut grp_sel)
        {
            if grp_sel == NEW_GROUP_SENTINEL {
                open_group_manager = true;
            } else if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                chart.undo_stack.push(DrawingAction::Modify(d.id.clone(), d.clone()));
                chart.redo_stack.clear();
                d.group_id = grp_sel.clone();
                crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
            }
        }

        ui.add(egui::Separator::default().spacing(gap_xs()));

        // Extension toggles (lines only)
        if matches!(&sel_draw.kind, DrawingKind::TrendLine{..} | DrawingKind::Ray{..}) {
            let r = KitButton::new("\u{2190}").variant(KitVariant::Ghost).size(KitSize::Sm)
                .fg(if sel_draw.extend_left { t.accent } else { dim })
                .min_size(egui::vec2(18.0, row_height_dense())).show(ui, t);
            Tooltip::new("Extend left").show(ui, &r, t);
            if r.clicked() {
                if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                    chart.undo_stack.push(DrawingAction::Modify(d.id.clone(), d.clone()));
                    chart.redo_stack.clear();
                    d.extend_left = !d.extend_left;
                    crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                }
            }
            let r = KitButton::new("\u{2192}").variant(KitVariant::Ghost).size(KitSize::Sm)
                .fg(if sel_draw.extend_right { t.accent } else { dim })
                .min_size(egui::vec2(18.0, row_height_dense())).show(ui, t);
            Tooltip::new("Extend right").show(ui, &r, t);
            if r.clicked() {
                if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) {
                    chart.undo_stack.push(DrawingAction::Modify(d.id.clone(), d.clone()));
                    chart.redo_stack.clear();
                    d.extend_right = !d.extend_right;
                    crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                }
            }
            ui.add(egui::Separator::default().spacing(gap_xs()));
        }

        // Lock
        if KitButton::new(if sel_draw.locked { "Locked" } else { "Lock" })
            .variant(KitVariant::Ghost).size(KitSize::Sm)
            .fg(if sel_draw.locked { t.accent } else { dim }).show(ui, t).clicked() {
            if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == sel_id) { d.locked = !d.locked; }
        }

        ui.add(egui::Separator::default().spacing(gap_xs()));

        // Delete
        let r = KitButton::icon(Icon::TRASH).variant(KitVariant::Ghost).placement(IconPlacement::ChartChrome).tone_destructive().show(ui, t);
        Tooltip::new("Delete").show(ui, &r, t);
        if r.clicked() {
            if let Some(d) = chart.drawings.iter().find(|d| d.id == sel_id) {
                chart.undo_stack.push(DrawingAction::Remove(d.clone()));
            }
            crate::drawing_db::remove(&sel_id);
            chart.drawings.retain(|d| d.id != sel_id);
            chart.redo_stack.clear();
            delete_sel = true;
        }

        // Alert bell toggle
        ui.add(egui::Separator::default().spacing(gap_xs()));
        let has_alert = sel_draw.alert_enabled;
        let bell_col = if has_alert { COLOR_AMBER } else { dim };
        let bell_label = if has_alert { "\u{1F514} ON" } else { "\u{1F514}" };
        if KitButton::new(bell_label).variant(KitVariant::Ghost).size(KitSize::Sm)
            .fg(bell_col).show(ui, t).clicked() {
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
