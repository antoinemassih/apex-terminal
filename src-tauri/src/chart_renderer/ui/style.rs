//! Shared styling helpers — eliminate duplicated RichText/Button/Frame patterns.

use egui::{self, Color32, RichText, Stroke};

/// Monospace text with size and color.
#[inline]
pub fn mono(text: &str, size: f32, color: Color32) -> RichText {
    RichText::new(text).monospace().size(size).color(color)
}

/// Bold monospace text.
#[inline]
pub fn mono_bold(text: &str, size: f32, color: Color32) -> RichText {
    RichText::new(text).monospace().size(size).strong().color(color)
}

/// Standard toolbar button — 11px monospace, 3px radius, themed colors.
pub fn tb_btn(ui: &mut egui::Ui, label: &str, active: bool, accent: Color32, dim: Color32, toolbar_bg: Color32, toolbar_border: Color32) -> egui::Response {
    let bg = if active { Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 51) } else { toolbar_bg };
    let fg = if active { accent } else { dim };
    let border = if active { Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 136) } else { toolbar_border };
    ui.add(egui::Button::new(RichText::new(label).monospace().size(11.0).color(fg))
        .fill(bg).stroke(Stroke::new(1.0, border)).corner_radius(3.0)
        .min_size(egui::vec2(0.0, 22.0)))
}

/// Standard popup window frame — dark background, no title bar.
pub fn popup_frame(ctx: &egui::Context, id: &str, pos: egui::Pos2, width: f32, fill: Color32, border_color: Option<Color32>) -> egui::Window<'static> {
    let mut frame = egui::Frame::popup(&ctx.style()).fill(fill).inner_margin(8.0);
    if let Some(bc) = border_color {
        frame = frame.stroke(Stroke::new(1.0, bc));
    }
    egui::Window::new(id.to_string())
        .fixed_pos(pos)
        .fixed_size(egui::vec2(width, 0.0))
        .title_bar(false)
        .frame(frame)
}

/// Draw a dashed or dotted line between two points.
pub fn dashed_line(painter: &egui::Painter, a: egui::Pos2, b: egui::Pos2, stroke: Stroke, style: super::super::LineStyle) {
    use super::super::LineStyle;
    match style {
        LineStyle::Solid => { painter.line_segment([a, b], stroke); }
        LineStyle::Dashed | LineStyle::Dotted => {
            let (dash, gap) = if style == LineStyle::Dashed { (6.0, 3.0) } else { (2.0, 2.0) };
            let dir = b - a;
            let len = dir.length();
            if len < 1.0 { return; }
            let norm = dir / len;
            let mut d = 0.0;
            while d < len {
                let p0 = a + norm * d;
                let p1 = a + norm * (d + dash).min(len);
                painter.line_segment([p0, p1], stroke);
                d += dash + gap;
            }
        }
    }
}

/// Draw a thick line into an RGBA buffer (for icon generation).
pub fn draw_line_rgba(rgba: &mut [u8], width: u32, x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32, color: [u8; 4]) {
    let len = ((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0)).sqrt();
    let steps = (len * 3.0) as i32;
    let w = thickness as i32;
    for i in 0..=steps {
        let t = i as f32 / steps.max(1) as f32;
        let px = (x0 + (x1 - x0) * t) as i32;
        let py = (y0 + (y1 - y0) * t) as i32;
        for dy in -w..=w {
            for dx in -w..=w {
                let ix = px + dx;
                let iy = py + dy;
                if ix >= 0 && ix < width as i32 && iy >= 0 && iy < width as i32 {
                    let idx = ((iy as u32 * width + ix as u32) * 4) as usize;
                    if idx + 3 < rgba.len() {
                        rgba[idx..idx + 4].copy_from_slice(&color);
                    }
                }
            }
        }
    }
}

// ─── Consistent UI widget components ─────────────────────────────────────────

/// Section header label — used in context menus, panels, popups.
#[inline]
pub fn section_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).monospace().size(9.0).strong().color(color));
}

/// Dim info label — used for counts, subtitles, status text.
#[inline]
pub fn dim_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).monospace().size(9.0).color(color));
}

/// Close button (X icon) — consistent across all popups and panels.
#[inline]
pub fn close_button(ui: &mut egui::Ui, dim: Color32) -> bool {
    ui.add(egui::Button::new(RichText::new(super::super::super::ui_kit::icons::Icon::X).size(10.0).color(dim)).frame(false)).clicked()
}

/// Panel header row — title + close button, used in watchlist, order book, connection panel, etc.
pub fn panel_header(ui: &mut egui::Ui, title: &str, accent: Color32, dim: Color32) -> bool {
    let mut closed = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).monospace().size(10.0).strong().color(accent));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if close_button(ui, dim) { closed = true; }
        });
    });
    closed
}

/// Convert hex color string to egui Color32 with opacity.
pub fn hex_to_color(hex: &str, opacity: f32) -> Color32 {
    let h = hex.trim_start_matches('#');
    let r = u8::from_str_radix(h.get(0..2).unwrap_or("80"), 16).unwrap_or(128);
    let g = u8::from_str_radix(h.get(2..4).unwrap_or("80"), 16).unwrap_or(128);
    let b = u8::from_str_radix(h.get(4..6).unwrap_or("80"), 16).unwrap_or(128);
    Color32::from_rgba_unmultiplied(r, g, b, (opacity * 255.0) as u8)
}
