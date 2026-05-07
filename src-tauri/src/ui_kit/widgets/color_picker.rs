//! ColorPicker — popover-based color selector.
//!
//! Modes:
//!   - Compact swatch: shows a small color swatch button; click opens
//!     the picker popover.
//!   - Inline: full picker UI rendered in place (no popover).
//!
//! Picker UI:
//!   - HSV hue strip + saturation/value square
//!   - Hex input (3/6/8 char)
//!   - RGBA sliders (0-255)
//!   - Optional preset swatches (theme accent/bull/bear/warn + recent)
//!   - Optional alpha
//!
//! API:
//!   let mut color = Color32::WHITE;
//!   ColorPicker::new(&mut color).show(ui, theme);

use egui::{
    ecolor::Hsva, Color32, CornerRadius, FontId, Id, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, Vec2,
};

use super::motion;
use super::popover::Popover;
use super::theme::ComponentTheme;
use super::tokens::Size;
use super::{Input, Slider};
use crate::chart::renderer::ui::style as st;

const SV_SIZE: f32 = 180.0;
const HUE_H: f32 = 16.0;
const PRESET_SWATCH: f32 = 20.0;

#[must_use = "ColorPicker does nothing until `.show(ui, theme)` is called"]
pub struct ColorPicker<'a> {
    color: &'a mut Color32,
    label: Option<String>,
    compact: bool,
    inline: bool,
    with_alpha: bool,
    presets: Option<&'a [Color32]>,
    size: Size,
    disabled: bool,
}

impl<'a> ColorPicker<'a> {
    pub fn new(color: &'a mut Color32) -> Self {
        Self {
            color,
            label: None,
            compact: true,
            inline: false,
            with_alpha: false,
            presets: None,
            size: Size::Md,
            disabled: false,
        }
    }

    pub fn label(mut self, text: impl Into<String>) -> Self {
        self.label = Some(text.into());
        self
    }
    pub fn compact(mut self, v: bool) -> Self {
        self.compact = v;
        self
    }
    pub fn inline(mut self, v: bool) -> Self {
        self.inline = v;
        self.compact = !v;
        self
    }
    pub fn with_alpha(mut self, v: bool) -> Self {
        self.with_alpha = v;
        self
    }
    pub fn presets(mut self, colors: &'a [Color32]) -> Self {
        self.presets = Some(colors);
        self
    }
    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        if self.inline {
            paint_inline(ui, theme, self)
        } else {
            paint_compact(ui, theme, self)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct PickerState {
    open: bool,
    hex_buf: String,
    /// Hue snapshot (0..1) — kept so dragging into pure white/black
    /// doesn't lose the active hue.
    hue: f32,
    /// Saturation snapshot (0..1) — kept so dragging value→0 (black)
    /// doesn't lose the saturation choice.
    sat: f32,
    /// The color we last synced state from. If color changes externally
    /// we re-derive hsv from it.
    last_color: Color32,
}

impl PickerState {
    fn from_color(c: Color32) -> Self {
        let hsva = Hsva::from(c);
        Self {
            open: false,
            hex_buf: color_to_hex(c, true),
            hue: hsva.h,
            sat: hsva.s,
            last_color: c,
        }
    }

    fn sync_external(&mut self, c: Color32) {
        if c != self.last_color {
            let hsva = Hsva::from(c);
            // Only overwrite hue/sat when they're meaningful (avoid losing
            // the user's active hue when value collapses to 0).
            if hsva.s > 0.001 {
                self.sat = hsva.s;
            }
            if hsva.v > 0.001 && hsva.s > 0.001 {
                self.hue = hsva.h;
            }
            self.hex_buf = color_to_hex(c, true);
            self.last_color = c;
        }
    }
}

fn state_id(id: Id) -> Id {
    id.with("color_picker_state")
}

fn load_state(ui: &Ui, id: Id, color: Color32) -> PickerState {
    ui.ctx()
        .memory(|m| m.data.get_temp::<PickerState>(state_id(id)))
        .map(|mut s| {
            s.sync_external(color);
            s
        })
        .unwrap_or_else(|| PickerState::from_color(color))
}

fn store_state(ui: &Ui, id: Id, state: PickerState) {
    ui.ctx()
        .memory_mut(|m| m.data.insert_temp(state_id(id), state));
}

// ─────────────────────────────────────────────────────────────────────────────
// Hex parsing
// ─────────────────────────────────────────────────────────────────────────────

fn color_to_hex(c: Color32, include_alpha: bool) -> String {
    if include_alpha && c.a() != 255 {
        format!("#{:02X}{:02X}{:02X}{:02X}", c.r(), c.g(), c.b(), c.a())
    } else {
        format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
    }
}

/// Parse "#RGB", "#RRGGBB", or "#RRGGBBAA" (case-insensitive, # optional).
fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    if !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match s.len() {
        3 => {
            let r = u8::from_str_radix(&s[0..1], 16).ok()?;
            let g = u8::from_str_radix(&s[1..2], 16).ok()?;
            let b = u8::from_str_radix(&s[2..3], 16).ok()?;
            Some(Color32::from_rgba_unmultiplied(
                r * 17,
                g * 17,
                b * 17,
                255,
            ))
        }
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            Some(Color32::from_rgba_unmultiplied(r, g, b, 255))
        }
        8 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            let a = u8::from_str_radix(&s[6..8], 16).ok()?;
            Some(Color32::from_rgba_unmultiplied(r, g, b, a))
        }
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Compact swatch trigger
// ─────────────────────────────────────────────────────────────────────────────

fn paint_compact<'a>(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    cp: ColorPicker<'a>,
) -> Response {
    let swatch_d = match cp.size {
        Size::Sm | Size::Xs => 24.0,
        _ => 28.0,
    };
    let label = cp.label.clone();

    let resp = ui
        .horizontal(|ui| {
            let (rect, mut resp) =
                ui.allocate_exact_size(Vec2::splat(swatch_d), Sense::click());
            let id = resp.id;

            paint_swatch(ui, rect, *cp.color, theme, id, resp.hovered());

            if cp.disabled {
                resp = resp.on_hover_cursor(egui::CursorIcon::NotAllowed);
            } else if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            // Toggle state.
            let mut state = load_state(ui, id, *cp.color);
            if resp.clicked() && !cp.disabled {
                state.open = !state.open;
            }

            if let Some(text) = &label {
                ui.add_space(st::gap_xs());
                ui.painter().text(
                    Pos2::new(
                        rect.right() + st::gap_xs(),
                        rect.center().y,
                    ),
                    egui::Align2::LEFT_CENTER,
                    text,
                    FontId::proportional(st::font_sm()),
                    st::color_alpha(theme.text(), 200),
                );
                let galley = ui.fonts(|f| {
                    f.layout_no_wrap(
                        text.clone(),
                        FontId::proportional(st::font_sm()),
                        Color32::WHITE,
                    )
                });
                ui.allocate_exact_size(
                    Vec2::new(galley.rect.width(), swatch_d),
                    Sense::hover(),
                );
            }

            // Popover with picker UI.
            if state.open {
                let mut open_flag = state.open;
                let popover_id = id.with("popover");
                Popover::new()
                    .open(&mut open_flag)
                    .anchor(rect)
                    .id(popover_id)
                    .show(ui, theme, |ui| {
                        ui.set_min_width(SV_SIZE + st::gap_sm() * 2.0);
                        paint_picker_body(
                            ui,
                            theme,
                            id,
                            cp.color,
                            &mut state,
                            cp.with_alpha,
                            cp.presets,
                        );
                    });
                state.open = open_flag;
            }

            store_state(ui, id, state);
            resp
        })
        .inner;

    resp
}

// ─────────────────────────────────────────────────────────────────────────────
// Inline mode
// ─────────────────────────────────────────────────────────────────────────────

fn paint_inline<'a>(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    cp: ColorPicker<'a>,
) -> Response {
    // Allocate a stable id for this inline picker.
    let id = ui.make_persistent_id(("color_picker_inline", cp.label.as_deref().unwrap_or("")));
    let mut state = load_state(ui, id, *cp.color);

    let inner = ui.vertical(|ui| {
        if let Some(text) = &cp.label {
            ui.label(
                egui::RichText::new(text)
                    .size(st::font_sm())
                    .color(theme.dim()),
            );
            ui.add_space(st::gap_2xs());
        }
        paint_picker_body(
            ui,
            theme,
            id,
            cp.color,
            &mut state,
            cp.with_alpha,
            cp.presets,
        );
    });

    store_state(ui, id, state);
    inner.response
}

// ─────────────────────────────────────────────────────────────────────────────
// Trigger swatch paint (with checkered alpha pattern)
// ─────────────────────────────────────────────────────────────────────────────

fn paint_swatch(
    ui: &Ui,
    rect: Rect,
    color: Color32,
    theme: &dyn ComponentTheme,
    id: Id,
    hovered: bool,
) {
    let painter = ui.painter_at(rect);
    let radius = CornerRadius::same(4);

    // Checkered transparency background if alpha < 255.
    if color.a() < 255 {
        paint_checker(&painter, rect, 4.0);
    }
    painter.rect_filled(rect, radius, color);

    // Animated border.
    let hover_t = motion::ease_bool(ui.ctx(), id.with("swatch_hover"), hovered, motion::FAST);
    let border = motion::lerp_color(theme.border(), theme.accent(), hover_t);
    painter.rect_stroke(rect, radius, Stroke::new(1.0, border), StrokeKind::Inside);
}

fn paint_checker(painter: &egui::Painter, rect: Rect, cell: f32) {
    let light = Color32::from_gray(180);
    let dark = Color32::from_gray(120);
    painter.rect_filled(rect, CornerRadius::same(4), light);
    let cols = (rect.width() / cell).ceil() as i32;
    let rows = (rect.height() / cell).ceil() as i32;
    for y in 0..rows {
        for x in 0..cols {
            if (x + y) % 2 == 0 {
                continue;
            }
            let r = Rect::from_min_size(
                Pos2::new(rect.left() + x as f32 * cell, rect.top() + y as f32 * cell),
                Vec2::splat(cell),
            )
            .intersect(rect);
            painter.rect_filled(r, CornerRadius::ZERO, dark);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Picker body
// ─────────────────────────────────────────────────────────────────────────────

fn paint_picker_body(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    base_id: Id,
    color: &mut Color32,
    state: &mut PickerState,
    with_alpha: bool,
    presets: Option<&[Color32]>,
) {
    ui.spacing_mut().item_spacing.y = st::gap_sm();

    // ── 1. SV square ──
    paint_sv_square(ui, base_id, color, state);

    // ── 2. Hue strip ──
    paint_hue_strip(ui, base_id, color, state);

    // ── 3. Hex input + small preview ──
    ui.horizontal(|ui| {
        let preview_size = Vec2::splat(20.0);
        let (prect, _) = ui.allocate_exact_size(preview_size, Sense::hover());
        paint_swatch(ui, prect, *color, theme, base_id.with("hex_preview"), false);

        ui.add_space(st::gap_xs());

        let pre = state.hex_buf.clone();
        let resp = Input::new(&mut state.hex_buf)
            .min_width(110.0)
            .size(Size::Sm)
            .placeholder("#RRGGBB")
            .show(ui, theme);

        // On submit or focus loss, parse & commit (or revert).
        if resp.submitted || resp.response.lost_focus() {
            match parse_hex(&state.hex_buf) {
                Some(c) => {
                    *color = c;
                    state.last_color = c;
                    state.hex_buf = color_to_hex(c, true);
                    let hsva = Hsva::from(c);
                    if hsva.s > 0.001 {
                        state.sat = hsva.s;
                    }
                    if hsva.v > 0.001 {
                        state.hue = hsva.h;
                    }
                }
                None => {
                    // Revert.
                    state.hex_buf = color_to_hex(*color, true);
                }
            }
        } else if state.hex_buf != pre {
            // Live-parse without reverting buffer (so user can keep typing).
            if let Some(c) = parse_hex(&state.hex_buf) {
                *color = c;
                state.last_color = c;
                let hsva = Hsva::from(c);
                if hsva.s > 0.001 {
                    state.sat = hsva.s;
                }
                if hsva.v > 0.001 {
                    state.hue = hsva.h;
                }
            }
        }
    });

    // ── 4. RGBA sliders ──
    paint_rgba_sliders(ui, theme, color, state, with_alpha);

    // ── 5. Presets ──
    let default_presets;
    let preset_slice: &[Color32] = if let Some(p) = presets {
        p
    } else {
        default_presets = [
            theme.accent(),
            theme.bull(),
            theme.bear(),
            theme.warn(),
            theme.text(),
            theme.dim(),
        ];
        &default_presets
    };
    if !preset_slice.is_empty() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = st::gap_2xs();
            for (i, &c) in preset_slice.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(
                    Vec2::splat(PRESET_SWATCH),
                    Sense::click(),
                );
                paint_swatch(ui, rect, c, theme, base_id.with(("preset", i)), resp.hovered());
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    *color = c;
                    state.last_color = c;
                    state.hex_buf = color_to_hex(c, true);
                    let hsva = Hsva::from(c);
                    if hsva.s > 0.001 {
                        state.sat = hsva.s;
                    }
                    if hsva.v > 0.001 {
                        state.hue = hsva.h;
                    }
                }
            }
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SV square
// ─────────────────────────────────────────────────────────────────────────────

fn paint_sv_square(
    ui: &mut Ui,
    base_id: Id,
    color: &mut Color32,
    state: &mut PickerState,
) {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::splat(SV_SIZE), Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    // Pure hue at full saturation/value.
    let hue_color: Color32 = Hsva::new(state.hue, 1.0, 1.0, 1.0).into();

    // Horizontal: white → hue. Vertical: top → bottom darkens to black.
    // We approximate the SV square via a grid of small filled rects.
    // Coarse enough for 60fps — 18×18 cells.
    let cells = 24usize;
    let cell_w = SV_SIZE / cells as f32;
    for j in 0..cells {
        for i in 0..cells {
            let s = (i as f32 + 0.5) / cells as f32;
            let v = 1.0 - (j as f32 + 0.5) / cells as f32;
            let c: Color32 = Hsva::new(state.hue, s, v, 1.0).into();
            let r = Rect::from_min_size(
                Pos2::new(
                    rect.left() + i as f32 * cell_w,
                    rect.top() + j as f32 * cell_w,
                ),
                Vec2::splat(cell_w + 0.5),
            );
            painter.rect_filled(r, CornerRadius::ZERO, c);
        }
    }

    painter.rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(1.0, Color32::from_black_alpha(80)),
        StrokeKind::Inside,
    );

    // Drag/click → update s,v.
    let cur_hsva = Hsva::from(*color);
    let mut s = cur_hsva.s;
    let mut v = cur_hsva.v;
    let dragging = resp.dragged() || resp.clicked();
    if dragging {
        if let Some(p) = resp.interact_pointer_pos() {
            s = ((p.x - rect.left()) / SV_SIZE).clamp(0.0, 1.0);
            v = 1.0 - ((p.y - rect.top()) / SV_SIZE).clamp(0.0, 1.0);
            state.sat = s;
            let new_c: Color32 = Hsva::new(state.hue, s, v, cur_hsva.a).into();
            *color = preserve_alpha(new_c, color.a());
            state.last_color = *color;
            state.hex_buf = color_to_hex(*color, true);
        }
    }

    // Picker dot — ease toward current S/V when not dragging.
    let dot_x_target = rect.left() + s * SV_SIZE;
    let dot_y_target = rect.top() + (1.0 - v) * SV_SIZE;
    let (dot_x, dot_y) = if dragging {
        (dot_x_target, dot_y_target)
    } else {
        (
            motion::ease_value(ui.ctx(), base_id.with("sv_dot_x"), dot_x_target, motion::FAST),
            motion::ease_value(ui.ctx(), base_id.with("sv_dot_y"), dot_y_target, motion::FAST),
        )
    };
    let center = Pos2::new(dot_x, dot_y);
    painter.circle_stroke(center, 6.0, Stroke::new(2.0, Color32::WHITE));
    painter.circle_stroke(center, 6.0, Stroke::new(1.0, Color32::from_black_alpha(180)));

    let _ = hue_color; // silence unused if optimized
}

// ─────────────────────────────────────────────────────────────────────────────
// Hue strip
// ─────────────────────────────────────────────────────────────────────────────

fn paint_hue_strip(
    ui: &mut Ui,
    base_id: Id,
    color: &mut Color32,
    state: &mut PickerState,
) {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(SV_SIZE, HUE_H), Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    let segments = 36usize;
    let seg_w = SV_SIZE / segments as f32;
    for i in 0..segments {
        let h = (i as f32 + 0.5) / segments as f32;
        let c: Color32 = Hsva::new(h, 1.0, 1.0, 1.0).into();
        let r = Rect::from_min_size(
            Pos2::new(rect.left() + i as f32 * seg_w, rect.top()),
            Vec2::new(seg_w + 0.5, HUE_H),
        );
        painter.rect_filled(r, CornerRadius::ZERO, c);
    }

    painter.rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(1.0, Color32::from_black_alpha(80)),
        StrokeKind::Inside,
    );

    let dragging = resp.dragged() || resp.clicked();
    if dragging {
        if let Some(p) = resp.interact_pointer_pos() {
            let new_h = ((p.x - rect.left()) / SV_SIZE).clamp(0.0, 1.0);
            state.hue = new_h;
            let cur = Hsva::from(*color);
            // If saturation collapsed, restore from snapshot so rotating
            // hue on white still lets the color travel.
            let s = if cur.s > 0.001 { cur.s } else { state.sat.max(0.5) };
            let v = if cur.v > 0.001 { cur.v } else { 1.0 };
            let new_c: Color32 = Hsva::new(new_h, s, v, cur.a).into();
            *color = preserve_alpha(new_c, color.a());
            state.last_color = *color;
            state.hex_buf = color_to_hex(*color, true);
            state.sat = s;
        }
    }

    // Caret (animated x).
    let caret_target = rect.left() + state.hue * SV_SIZE;
    let caret_x = if dragging {
        caret_target
    } else {
        motion::ease_value(ui.ctx(), base_id.with("hue_caret"), caret_target, motion::FAST)
    };
    let caret = Rect::from_min_max(
        Pos2::new(caret_x - 2.0, rect.top() - 2.0),
        Pos2::new(caret_x + 2.0, rect.bottom() + 2.0),
    );
    painter.rect_filled(caret, CornerRadius::same(1), Color32::WHITE);
    painter.rect_stroke(
        caret,
        CornerRadius::same(1),
        Stroke::new(1.0, Color32::from_black_alpha(180)),
        StrokeKind::Inside,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// RGBA sliders
// ─────────────────────────────────────────────────────────────────────────────

fn paint_rgba_sliders(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    color: &mut Color32,
    state: &mut PickerState,
    with_alpha: bool,
) {
    let (mut r, mut g, mut b, mut a) = (
        color.r() as f32,
        color.g() as f32,
        color.b() as f32,
        color.a() as f32,
    );

    let pre = (r, g, b, a);

    Slider::new(&mut r, 0.0..=255.0)
        .step(1.0)
        .label("R")
        .full_width()
        .show(ui, theme);
    Slider::new(&mut g, 0.0..=255.0)
        .step(1.0)
        .label("G")
        .full_width()
        .show(ui, theme);
    Slider::new(&mut b, 0.0..=255.0)
        .step(1.0)
        .label("B")
        .full_width()
        .show(ui, theme);
    if with_alpha {
        Slider::new(&mut a, 0.0..=255.0)
            .step(1.0)
            .label("A")
            .full_width()
            .show(ui, theme);
    }

    if (r, g, b, a) != pre {
        let nc = Color32::from_rgba_unmultiplied(r as u8, g as u8, b as u8, a as u8);
        *color = nc;
        state.last_color = nc;
        state.hex_buf = color_to_hex(nc, true);
        let hsva = Hsva::from(nc);
        if hsva.s > 0.001 {
            state.sat = hsva.s;
        }
        if hsva.v > 0.001 && hsva.s > 0.001 {
            state.hue = hsva.h;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn preserve_alpha(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_3char() {
        let c = parse_hex("#abc").unwrap();
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (0xAA, 0xBB, 0xCC, 255));
    }

    #[test]
    fn parse_hex_6char() {
        let c = parse_hex("#1A2B3C").unwrap();
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (0x1A, 0x2B, 0x3C, 255));
    }

    #[test]
    fn parse_hex_8char() {
        let c = parse_hex("#1A2B3C80").unwrap();
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (0x1A, 0x2B, 0x3C, 0x80));
    }

    #[test]
    fn parse_hex_no_hash() {
        assert!(parse_hex("FFFFFF").is_some());
    }

    #[test]
    fn parse_hex_invalid() {
        assert!(parse_hex("xyz").is_none());
        assert!(parse_hex("#12345").is_none());
    }
}
