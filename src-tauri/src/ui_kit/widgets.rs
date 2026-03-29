//! Reusable widgets for the chart UI.

use egui::{Color32, Ui, Response, Sense, RichText};
use super::theme::{ChartTheme, DRAW_COLORS};
use super::icons::Icon;
use crate::chart_renderer::LineStyle;

/// Color picker — row of colored circles, returns selected hex color if clicked.
pub fn color_picker(ui: &mut Ui, current: &str) -> Option<String> {
    let mut result = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for &(hex, color) in DRAW_COLORS {
            let is_cur = current == hex;
            let (r, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), Sense::click());
            ui.painter().circle_filled(r.center(), if is_cur { 7.0 } else { 5.5 }, color);
            if is_cur { ui.painter().circle_stroke(r.center(), 8.0, egui::Stroke::new(1.5, Color32::WHITE)); }
            if resp.clicked() { result = Some(hex.to_string()); }
        }
    });
    result
}

/// Line style dropdown — shows visual preview, returns new style if changed.
pub fn line_style_dropdown(ui: &mut Ui, id: &str, current: LineStyle) -> Option<LineStyle> {
    let mut result = None;
    let label = match current { LineStyle::Solid => "____", LineStyle::Dashed => "- - -", LineStyle::Dotted => ". . ." };
    egui::ComboBox::from_id_salt(id).selected_text(label).width(65.0).show_ui(ui, |ui| {
        if ui.selectable_label(current == LineStyle::Solid, "_____ Solid").clicked() { result = Some(LineStyle::Solid); }
        if ui.selectable_label(current == LineStyle::Dashed, "- - - -  Dash").clicked() { result = Some(LineStyle::Dashed); }
        if ui.selectable_label(current == LineStyle::Dotted, ". . . . .  Dot").clicked() { result = Some(LineStyle::Dotted); }
    });
    result
}

/// Thickness dropdown — returns new thickness if changed.
pub fn thickness_dropdown(ui: &mut Ui, id: &str, current: f32) -> Option<f32> {
    let mut result = None;
    egui::ComboBox::from_id_salt(id).selected_text(format!("{:.1}px", current)).width(52.0).show_ui(ui, |ui| {
        for &th in &[0.5_f32, 1.0, 1.5, 2.5] {
            if ui.selectable_label((current - th).abs() < 0.1, format!("{:.1}px", th)).clicked() { result = Some(th); }
        }
    });
    result
}

/// Opacity dropdown — returns new opacity if changed.
pub fn opacity_dropdown(ui: &mut Ui, id: &str, current: f32) -> Option<f32> {
    let mut result = None;
    egui::ComboBox::from_id_salt(id).selected_text(format!("{}%", (current * 100.0) as u32)).width(48.0).show_ui(ui, |ui| {
        for &op in &[1.0_f32, 0.75, 0.5, 0.25] {
            if ui.selectable_label((current - op).abs() < 0.01, format!("{}%", (op * 100.0) as u32)).clicked() { result = Some(op); }
        }
    });
    result
}

/// Tool button — selectable button with icon for drawing tools.
pub fn tool_button(ui: &mut Ui, icon: &str, label: &str, active: bool) -> bool {
    let text = RichText::new(format!("{} {}", icon, label)).small();
    ui.selectable_label(active, text).clicked()
}

/// Delete button — red X icon.
pub fn delete_button(ui: &mut Ui) -> bool {
    Icon::button_colored(ui, Icon::X, Color32::from_rgb(224, 85, 96), "Delete").clicked()
}

/// OHLC label — colored price display for top-left of chart.
pub fn ohlc_label(ui: &mut Ui, o: f32, h: f32, l: f32, c: f32, v: f32, theme: &ChartTheme) {
    let color = if c >= o { theme.bull } else { theme.bear };
    ui.label(RichText::new(format!("O{:.2} H{:.2} L{:.2} C{:.2} V{:.0}", o, h, l, c, v))
        .monospace().size(11.0).color(color));
}
