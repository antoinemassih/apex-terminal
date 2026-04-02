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

/// Standard toolbar button — 11px monospace, 3px radius, themed colors, pointer cursor.
pub fn tb_btn(ui: &mut egui::Ui, label: &str, active: bool, accent: Color32, dim: Color32, toolbar_bg: Color32, toolbar_border: Color32) -> egui::Response {
    let bg = if active { Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 51) } else { toolbar_bg };
    let fg = if active { accent } else { dim };
    let border = if active { Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 136) } else { toolbar_border };
    let resp = ui.add(egui::Button::new(RichText::new(label).monospace().size(11.0).color(fg))
        .fill(bg).stroke(Stroke::new(1.0, border)).corner_radius(3.0)
        .min_size(egui::vec2(0.0, 22.0)));
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
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

/// Application-quality dialog window — zero inner padding, border, rounded corners.
/// `bg` should be the theme's chart background; header will be one shade darker.
/// Use with `dialog_header()` inside the `.show()` closure.
pub fn dialog_window(ctx: &egui::Context, id: &str, pos: egui::Pos2, width: f32, border_color: Option<Color32>) -> egui::Window<'static> {
    // Default fill — caller should use dialog_window_themed for theme-aware colors
    let fill = Color32::from_rgb(26, 26, 32);
    let border = border_color.unwrap_or(Color32::from_rgba_unmultiplied(60, 60, 70, 80));
    egui::Window::new(id.to_string())
        .fixed_pos(pos)
        .fixed_size(egui::vec2(width, 0.0))
        .title_bar(false)
        .frame(egui::Frame::popup(&ctx.style()).fill(fill).inner_margin(0.0)
            .stroke(Stroke::new(1.0, border)).corner_radius(6.0))
}

/// Theme-aware dialog window — body is toolbar_bg, header will be one shade darker.
pub fn dialog_window_themed(ctx: &egui::Context, id: &str, pos: egui::Pos2, width: f32, toolbar_bg: Color32, toolbar_border: Color32, border_color: Option<Color32>) -> egui::Window<'static> {
    let border = border_color.unwrap_or(color_alpha(toolbar_border, 100));
    egui::Window::new(id.to_string())
        .fixed_pos(pos)
        .fixed_size(egui::vec2(width, 0.0))
        .title_bar(false)
        .frame(egui::Frame::popup(&ctx.style()).fill(toolbar_bg).inner_margin(0.0)
            .stroke(Stroke::new(1.0, border)).corner_radius(6.0))
}

/// Dialog header bar — one shade darker than body. Returns true if closed.
/// `toolbar_bg` is the panel body color; header is darkened from it.
pub fn dialog_header(ui: &mut egui::Ui, title: &str, dim: Color32) -> bool {
    dialog_header_colored(ui, title, dim, None)
}

/// Dialog header bar with explicit background color control.
pub fn dialog_header_colored(ui: &mut egui::Ui, title: &str, dim: Color32, header_bg: Option<Color32>) -> bool {
    use super::super::super::ui_kit::icons::Icon;
    let fill = header_bg.unwrap_or_else(|| {
        // Darken: subtract ~8 from each channel, clamped
        let bg = ui.visuals().window_fill();
        Color32::from_rgb(bg.r().saturating_sub(8), bg.g().saturating_sub(8), bg.b().saturating_sub(8))
    });
    let mut closed = false;
    egui::Frame::NONE.fill(fill)
        .inner_margin(egui::Margin { left: 10, right: 8, top: 8, bottom: 8 })
        .corner_radius(egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).monospace().size(11.0).strong()
                    .color(Color32::from_rgb(220, 220, 230)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(RichText::new(Icon::X).size(11.0)
                        .color(dim.gamma_multiply(0.6)))
                        .frame(false).min_size(egui::vec2(20.0, 20.0))).clicked() {
                        closed = true;
                    }
                });
            });
        });
    closed
}

/// Inset separator with gradient shadow — horizontal line with a soft shadow below it.
pub fn dialog_separator_shadow(ui: &mut egui::Ui, margin: f32, color: Color32) {
    let rect = ui.available_rect_before_wrap();
    let y = ui.cursor().min.y;
    let left = rect.left() + margin;
    let right = rect.right() - margin;
    // Main line
    ui.painter().line_segment(
        [egui::pos2(left, y), egui::pos2(right, y)],
        Stroke::new(0.5, color));
    // Soft shadow gradient (3 lines fading down)
    for i in 1..=3u8 {
        let alpha = (color.a() / 3).saturating_sub(i * 8);
        let shadow = Color32::from_rgba_unmultiplied(0, 0, 0, alpha);
        ui.painter().line_segment(
            [egui::pos2(left, y + i as f32), egui::pos2(right, y + i as f32)],
            Stroke::new(0.5, shadow));
    }
    ui.add_space(4.0);
}

/// Inset separator — horizontal line with margins on both sides.
pub fn dialog_separator(ui: &mut egui::Ui, margin: f32, color: Color32) {
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [egui::pos2(rect.left() + margin, ui.cursor().min.y),
         egui::pos2(rect.right() - margin, ui.cursor().min.y)],
        Stroke::new(0.5, color));
    ui.add_space(1.0);
}

/// Indented section label — label with left margin, used inside dialogs.
pub fn dialog_section(ui: &mut egui::Ui, text: &str, margin: f32, color: Color32) {
    ui.horizontal(|ui| {
        ui.add_space(margin);
        ui.label(RichText::new(text).monospace().size(9.0).strong().color(color));
    });
    ui.add_space(3.0);
}

/// Draw a dashed or dotted line between two points.
pub fn dashed_line(painter: &egui::Painter, a: egui::Pos2, b: egui::Pos2, stroke: Stroke, style: super::super::LineStyle) {
    use super::super::LineStyle;
    let dir = b - a;
    let len = dir.length();
    if len < 1.0 || !len.is_finite() || len > 20000.0 { return; } // safety: skip degenerate lines
    match style {
        LineStyle::Solid => { painter.line_segment([a, b], stroke); }
        LineStyle::Dashed | LineStyle::Dotted => {
            let (dash, gap) = if style == LineStyle::Dashed { (6.0, 3.0) } else { (2.0, 2.0) };
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

/// Color with alpha overlay — e.g. accent at 20% opacity for button backgrounds.
#[inline]
pub fn color_alpha(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

// ─── Form layout helpers ─────────────────────────────────────────────────────

/// Form row: fixed-width label + content. Used in order edit, indicator editor, etc.
/// Returns the inner response for chaining.
pub fn form_row(ui: &mut egui::Ui, label: &str, label_width: f32, dim: Color32, add_content: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.allocate_ui(egui::vec2(label_width, 18.0), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);
                ui.label(RichText::new(label).monospace().size(9.0).color(dim));
            });
        });
        add_content(ui);
    });
}

/// Horizontal separator line — thin divider between sections.
#[inline]
pub fn separator(ui: &mut egui::Ui, color: Color32) {
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [egui::pos2(rect.left(), ui.cursor().min.y), egui::pos2(rect.right(), ui.cursor().min.y)],
        Stroke::new(0.5, color),
    );
    ui.add_space(1.0);
}

// ─── Card / badge components ────────────────────────────────────────────────

/// Status badge — small colored pill label (e.g. "DRAFT", "PLACED", "EXEC").
pub fn status_badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    let bg = color_alpha(color, 24);
    let border = color_alpha(color, 68);
    ui.add(egui::Button::new(RichText::new(text).monospace().size(8.0).strong().color(color))
        .fill(bg).stroke(Stroke::new(0.5, border)).corner_radius(2.0)
        .min_size(egui::vec2(0.0, 14.0)));
}

/// Order card frame — paints a left-border accent stripe and a subtle card background.
/// Returns true if the card area was clicked (for selection).
pub fn order_card(ui: &mut egui::Ui, accent: Color32, bg: Color32, add_content: impl FnOnce(&mut egui::Ui)) -> bool {
    let available_w = ui.available_width();
    let resp = egui::Frame::NONE
        .fill(bg)
        .inner_margin(egui::Margin { left: 9, right: 6, top: 5, bottom: 5 })
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.set_min_width(available_w - 15.0);
            // Paint the left accent stripe
            let outer = ui.min_rect();
            let stripe = egui::Rect::from_min_max(
                egui::pos2(outer.left() - 9.0, outer.top() - 5.0),
                egui::pos2(outer.left() - 6.0, outer.bottom() + 5.0),
            );
            ui.painter().rect_filled(stripe, egui::CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 }, accent);
            add_content(ui);
        });
    // Make the whole card clickable
    let card_rect = resp.response.rect;
    let click_resp = ui.interact(card_rect, ui.id().with(("card_click", card_rect.min.x as i32, card_rect.min.y as i32)), egui::Sense::click());
    if click_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.add_space(4.0);
    click_resp.clicked()
}

/// Action button — accent-tinted background, used for Place/Cancel/Clear actions.
pub fn action_btn(ui: &mut egui::Ui, label: &str, color: Color32, enabled: bool) -> bool {
    let bg = if enabled { color_alpha(color, 30) } else { color_alpha(color, 10) };
    let fg = if enabled { color } else { color_alpha(color, 100) };
    let border = if enabled { color_alpha(color, 100) } else { color_alpha(color, 40) };
    let resp = ui.add_enabled(enabled, egui::Button::new(RichText::new(label).monospace().size(9.0).strong().color(fg))
        .fill(bg).stroke(Stroke::new(0.5, border)).corner_radius(3.0)
        .min_size(egui::vec2(0.0, 20.0)));
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    resp.clicked()
}

/// Trade button — full-color background for BUY/SELL. Contrasting text auto-selected.
pub fn trade_btn(ui: &mut egui::Ui, label: &str, color: Color32, width: f32) -> bool {
    // Pick text color based on background luminance for readability
    let lum = 0.299 * color.r() as f32 + 0.587 * color.g() as f32 + 0.114 * color.b() as f32;
    let text_color = if lum > 140.0 { Color32::from_rgb(10, 10, 10) } else { Color32::WHITE };
    let resp = ui.add(egui::Button::new(RichText::new(label).monospace().size(11.0).strong().color(text_color))
        .fill(color_alpha(color, 220))
        .min_size(egui::vec2(width, 26.0)).corner_radius(3.0));
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    resp.clicked()
}
