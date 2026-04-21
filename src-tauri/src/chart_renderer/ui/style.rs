//! Shared styling helpers — single source of truth for all UI style decisions.
//!
//! # Changing the look in one place
//! - Font sizes   → `FONT_*` constants
//! - Spacing      → `GAP_*` constants
//! - Corner radii → `RADIUS_*` constants
//! - Stroke widths → `STROKE_*` constants
//! - Alpha tiers  → `ALPHA_*` constants
//! - Drop shadows → `SHADOW_*` constants
//! - Fixed colors → `TEXT_*` constants
//!
//! All helpers below use these constants internally, so a single change propagates everywhere.

use egui::{self, Color32, RichText, Stroke};

/// Register an element hit for inspect mode. No-op when design-mode is off.
#[inline(always)]
fn hit(r: &egui::Rect, family: &'static str, category: &'static str) {
    crate::design_tokens::register_hit(
        [r.min.x, r.min.y, r.width(), r.height()], family, category);
}

// ─── Font size tokens ─────────────────────────────────────────────────────────
// In design-mode, these read from the global DesignTokens at runtime.
// Without design-mode, they compile to the same constants as before (zero overhead).
pub fn font_xs()  -> f32 { crate::dt_f32!(font.xs, 8.0) }
pub fn font_sm()  -> f32 { crate::dt_f32!(font.sm, 10.0) }
pub fn font_md()  -> f32 { crate::dt_f32!(font.md, 11.0) }
pub fn font_lg()  -> f32 { crate::dt_f32!(font.lg, 13.0) }
pub fn font_xl()  -> f32 { crate::dt_f32!(font.xl, 14.0) }
pub fn font_2xl() -> f32 { crate::dt_f32!(font.xxl, 15.0) }

// Keep the old names as non-const for backwards compat with all call sites.
// Without design-mode feature, the compiler inlines these to the literal values.
pub const FONT_XS:  f32 = 7.0;
pub const FONT_SM:  f32 = 9.0;
pub const FONT_MD:  f32 = 10.0;
pub const FONT_LG:  f32 = 11.0;
pub const FONT_XL:  f32 = 12.0;
pub const FONT_2XL: f32 = 13.0;

// ─── Spacing tokens ───────────────────────────────────────────────────────────
pub fn gap_xs()  -> f32 { crate::dt_f32!(spacing.xs, 2.0) }
pub fn gap_sm()  -> f32 { crate::dt_f32!(spacing.sm, 4.0) }
pub fn gap_md()  -> f32 { crate::dt_f32!(spacing.md, 6.0) }
pub fn gap_lg()  -> f32 { crate::dt_f32!(spacing.lg, 8.0) }
pub fn gap_xl()  -> f32 { crate::dt_f32!(spacing.xl, 10.0) }
pub fn gap_2xl() -> f32 { crate::dt_f32!(spacing.xxl, 12.0) }
pub fn gap_3xl() -> f32 { crate::dt_f32!(spacing.xxxl, 20.0) }

pub const GAP_XS:  f32 = 1.0;
pub const GAP_SM:  f32 = 3.0;
pub const GAP_MD:  f32 = 5.0;
pub const GAP_LG:  f32 = 6.0;
pub const GAP_XL:  f32 = 8.0;
pub const GAP_2XL: f32 = 10.0;
pub const GAP_3XL: f32 = 16.0;

// ─── Corner radius tokens ─────────────────────────────────────────────────────
pub fn radius_sm() -> f32 { crate::dt_f32!(radius.sm, 3.0) }
pub fn radius_md() -> f32 { crate::dt_f32!(radius.md, 4.0) }
pub fn radius_lg() -> f32 { crate::dt_f32!(radius.lg, 8.0) }

pub const RADIUS_SM: f32 = 3.0;
pub const RADIUS_MD: f32 = 4.0;
pub const RADIUS_LG: f32 = 8.0;

// ─── Stroke width tokens ─────────────────────────────────────────────────────
pub fn stroke_hair()  -> f32 { crate::dt_f32!(stroke.hair, 0.3) }
pub fn stroke_thin()  -> f32 { crate::dt_f32!(stroke.thin, 0.5) }
pub fn stroke_std()   -> f32 { crate::dt_f32!(stroke.std, 1.0) }
pub fn stroke_bold()  -> f32 { crate::dt_f32!(stroke.bold, 1.5) }
pub fn stroke_thick() -> f32 { crate::dt_f32!(stroke.thick, 2.0) }

pub const STROKE_HAIR:   f32 = 0.3;
pub const STROKE_THIN:   f32 = 0.5;
pub const STROKE_STD:    f32 = 1.0;
pub const STROKE_BOLD:   f32 = 1.5;
pub const STROKE_THICK:  f32 = 2.0;

// ─── Semantic alpha tokens ────────────────────────────────────────────────────
pub fn alpha_faint()  -> u8 { crate::dt_u8!(alpha.faint, 10) }
pub fn alpha_ghost()  -> u8 { crate::dt_u8!(alpha.ghost, 15) }
pub fn alpha_soft()   -> u8 { crate::dt_u8!(alpha.soft, 20) }
pub fn alpha_subtle() -> u8 { crate::dt_u8!(alpha.subtle, 25) }
pub fn alpha_tint()   -> u8 { crate::dt_u8!(alpha.tint, 30) }
pub fn alpha_muted()  -> u8 { crate::dt_u8!(alpha.muted, 40) }
pub fn alpha_line()   -> u8 { crate::dt_u8!(alpha.line, 50) }
pub fn alpha_dim()    -> u8 { crate::dt_u8!(alpha.dim, 60) }
pub fn alpha_strong() -> u8 { crate::dt_u8!(alpha.strong, 80) }
pub fn alpha_active() -> u8 { crate::dt_u8!(alpha.active, 100) }
pub fn alpha_heavy()  -> u8 { crate::dt_u8!(alpha.heavy, 120) }

/// Use with `color_alpha(color, ALPHA_*)` for consistent opacity tiers.
pub const ALPHA_FAINT:  u8 = 10;
pub const ALPHA_GHOST:  u8 = 15;
pub const ALPHA_SOFT:   u8 = 20;
pub const ALPHA_SUBTLE: u8 = 25;
pub const ALPHA_TINT:   u8 = 30;
pub const ALPHA_MUTED:  u8 = 40;
pub const ALPHA_LINE:   u8 = 50;
pub const ALPHA_DIM:    u8 = 60;
pub const ALPHA_STRONG: u8 = 80;
pub const ALPHA_ACTIVE: u8 = 100;
pub const ALPHA_HEAVY:  u8 = 120;

// ─── Drop shadow tokens ───────────────────────────────────────────────────────
pub fn shadow_offset() -> f32 { crate::dt_f32!(shadow.offset, 2.0) }
pub fn shadow_alpha()  -> u8  { crate::dt_u8!(shadow.alpha, 60) }
pub fn shadow_spread() -> f32 { crate::dt_f32!(shadow.spread, 4.0) }

pub const SHADOW_OFFSET: f32 = 2.0;
pub const SHADOW_ALPHA:  u8  = 60;
pub const SHADOW_SPREAD: f32 = 4.0;

// ─── Fixed text colors (fallback for code without Theme access) ──────────────
// Prefer `t.text` when Theme is in scope — these are dark-theme defaults.
pub static TEXT_PRIMARY: Color32 = Color32::from_rgb(220, 220, 230);
pub static TEXT_SECONDARY: Color32 = Color32::from_rgb(200, 200, 210);

// ─── Raw text helpers ─────────────────────────────────────────────────────────

#[inline]
pub fn mono(text: &str, size: f32, color: Color32) -> RichText {
    RichText::new(text).monospace().size(size).color(color)
}

#[inline]
pub fn mono_bold(text: &str, size: f32, color: Color32) -> RichText {
    RichText::new(text).monospace().size(size).strong().color(color)
}

// ─── Panel frame helpers ──────────────────────────────────────────────────────

/// Standard side-panel frame — toolbar bg + faint border (8px margin).
/// Used by card-heavy panels: orders, alerts, DOM.
pub fn panel_frame(toolbar_bg: Color32, toolbar_border: Color32) -> egui::Frame {
    egui::Frame::NONE
        .fill(toolbar_bg)
        .inner_margin(egui::Margin { left: gap_xl() as i8, right: gap_xl() as i8, top: gap_xl() as i8, bottom: gap_lg() as i8 })
        .stroke(Stroke::new(stroke_std(), color_alpha(toolbar_border, alpha_heavy())))
}

/// Compact panel frame — tighter margins for narrow info-dense panels (scanner, tape).
pub fn panel_frame_compact(toolbar_bg: Color32, toolbar_border: Color32) -> egui::Frame {
    egui::Frame::NONE
        .fill(toolbar_bg)
        .inner_margin(egui::Margin { left: gap_lg() as i8, right: gap_lg() as i8, top: gap_lg() as i8, bottom: gap_md() as i8 })
        .stroke(Stroke::new(stroke_std(), color_alpha(toolbar_border, alpha_heavy())))
}

// ─── Toolbar button ───────────────────────────────────────────────────────────

/// Toolbar button — FONT_LG, RADIUS_MD, themed, pointer cursor.
/// Active state: accent fill + accent border + soft glow halo + bottom underline.
/// Hover state: subtle bg tint + accent border.
pub fn tb_btn(ui: &mut egui::Ui, label: &str, active: bool, accent: Color32, dim: Color32, toolbar_bg: Color32, toolbar_border: Color32) -> egui::Response {
    let bg = if active {
        color_alpha(accent, alpha_tint())
    } else {
        color_alpha(toolbar_border, alpha_ghost())
    };
    let fg = if active { accent } else { dim };
    let border = if active {
        color_alpha(accent, alpha_active())
    } else {
        color_alpha(toolbar_border, alpha_muted())
    };

    let resp = ui.add(egui::Button::new(RichText::new(label).monospace().size(11.0).color(fg))
        .fill(bg).stroke(Stroke::new(0.5, border)).corner_radius(3.0)
        .min_size(egui::vec2(0.0, 20.0)));
    hit(&resp.rect, "TOOLBAR_BTN", "Toolbar");

    if active {
        let r = resp.rect;
        ui.painter().line_segment(
            [egui::pos2(r.left() + radius_md(), r.bottom() + 0.5),
             egui::pos2(r.right() - radius_md(), r.bottom() + 0.5)],
            Stroke::new(stroke_std(), color_alpha(accent, alpha_dim())));
    } else if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(resp.rect, radius_md(),
            color_alpha(toolbar_border, alpha_subtle()));
        ui.painter().rect_stroke(resp.rect, radius_md(),
            Stroke::new(stroke_thin(), color_alpha(accent, alpha_line())), egui::StrokeKind::Inside);
        let text_col = ui.style().visuals.override_text_color.unwrap_or(TEXT_PRIMARY);
        ui.painter().text(resp.rect.center(), egui::Align2::CENTER_CENTER,
            label, egui::FontId::monospace(font_lg()), text_col);
    }
    resp
}

// ─── Dialog / popup windows ───────────────────────────────────────────────────

/// Standard popup window frame — dark background, no title bar.
pub fn popup_frame(ctx: &egui::Context, id: &str, pos: egui::Pos2, width: f32, fill: Color32, border_color: Option<Color32>) -> egui::Window<'static> {
    let mut frame = egui::Frame::popup(&ctx.style()).fill(fill).inner_margin(gap_lg());
    if let Some(bc) = border_color {
        frame = frame.stroke(Stroke::new(stroke_std(), bc));
    }
    egui::Window::new(id.to_string())
        .fixed_pos(pos).fixed_size(egui::vec2(width, 0.0))
        .title_bar(false).frame(frame)
}

/// Application-quality dialog window — zero inner padding, RADIUS_LG corners.
pub fn dialog_window(ctx: &egui::Context, id: &str, pos: egui::Pos2, width: f32, border_color: Option<Color32>) -> egui::Window<'static> {
    let fill = Color32::from_rgb(26, 26, 32);
    let border = border_color.unwrap_or(Color32::from_rgba_unmultiplied(60, 60, 70, 80));
    egui::Window::new(id.to_string())
        .fixed_pos(pos).fixed_size(egui::vec2(width, 0.0))
        .title_bar(false)
        .frame(egui::Frame::popup(&ctx.style()).fill(fill).inner_margin(0.0)
            .stroke(Stroke::new(stroke_std(), border)).corner_radius(radius_lg()))
}

/// Theme-aware dialog window — rich shadow, beveled border, crisp edges.
pub fn dialog_window_themed(ctx: &egui::Context, id: &str, pos: egui::Pos2, width: f32, toolbar_bg: Color32, toolbar_border: Color32, border_color: Option<Color32>) -> egui::Window<'static> {
    let border = border_color.unwrap_or(color_alpha(toolbar_border, 80));
    egui::Window::new(id.to_string())
        .fixed_pos(pos).fixed_size(egui::vec2(width, 0.0))
        .title_bar(false)
        .frame(egui::Frame::popup(&ctx.style())
            .fill(toolbar_bg)
            .inner_margin(0.0)
            .stroke(Stroke::new(1.0, border))
            .corner_radius(12.0)
            .shadow(egui::epaint::Shadow {
                offset: [0, 8],
                blur: 28,
                spread: 2,
                color: Color32::from_black_alpha(80),
            }))
}

/// Dialog header bar — auto-darkened bg, FONT_LG title, X close. Returns true if closed.
pub fn dialog_header(ui: &mut egui::Ui, title: &str, dim: Color32) -> bool {
    dialog_header_colored(ui, title, dim, None)
}

/// Dialog header bar with explicit header background.
pub fn dialog_header_colored(ui: &mut egui::Ui, title: &str, dim: Color32, header_bg: Option<Color32>) -> bool {
    use super::super::super::ui_kit::icons::Icon;
    let darken = crate::dt_u8!(dialog.header_darken, 8);
    let fill = header_bg.unwrap_or_else(|| {
        let bg = ui.visuals().window_fill();
        Color32::from_rgb(bg.r().saturating_sub(darken), bg.g().saturating_sub(darken), bg.b().saturating_sub(darken))
    });
    let mut closed = false;
    let rlg = 12u8;
    egui::Frame::NONE.fill(fill)
        .inner_margin(egui::Margin { left: 12, right: 10, top: 10, bottom: 10 })
        .corner_radius(egui::CornerRadius { nw: rlg, ne: rlg, sw: 0, se: 0 })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let text_col = ui.style().visuals.override_text_color.unwrap_or(TEXT_PRIMARY);
                ui.label(RichText::new(title).monospace().size(font_lg()).strong().color(text_col));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_btn(ui, Icon::X, dim.gamma_multiply(0.7), font_xl()).clicked() {
                        closed = true;
                    }
                });
            });
        });
    closed
}

// ─── Separators ───────────────────────────────────────────────────────────────

/// Full-width horizontal separator.
#[inline]
pub fn separator(ui: &mut egui::Ui, color: Color32) {
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [egui::pos2(rect.left(), ui.cursor().min.y), egui::pos2(rect.right(), ui.cursor().min.y)],
        Stroke::new(stroke_thin(), color));
    ui.add_space(crate::dt_f32!(separator.after_space, 1.0));
}

/// Inset separator with margins on both sides.
pub fn dialog_separator(ui: &mut egui::Ui, margin: f32, color: Color32) {
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [egui::pos2(rect.left() + margin, ui.cursor().min.y),
         egui::pos2(rect.right() - margin, ui.cursor().min.y)],
        Stroke::new(stroke_thin(), color));
    ui.add_space(crate::dt_f32!(separator.after_space, 1.0));
}

/// Inset separator + soft gradient shadow below (3 fading lines).
pub fn dialog_separator_shadow(ui: &mut egui::Ui, margin: f32, color: Color32) {
    let rect = ui.available_rect_before_wrap();
    let y = ui.cursor().min.y;
    let left = rect.left() + margin;
    let right = rect.right() - margin;
    ui.painter().line_segment([egui::pos2(left, y), egui::pos2(right, y)], Stroke::new(stroke_thin(), color));
    // Fading shadow gradient: 3 strokes at decreasing black alpha
    #[cfg(feature = "design-mode")]
    let shadow_alphas = {
        if let Some(t) = crate::design_tokens::get() { t.shadow.gradient } else { [20u8, 12, 4] }
    };
    #[cfg(not(feature = "design-mode"))]
    let shadow_alphas = [20u8, 12, 4];
    for (i, &a) in shadow_alphas.iter().enumerate() {
        ui.painter().line_segment(
            [egui::pos2(left, y + (i + 1) as f32), egui::pos2(right, y + (i + 1) as f32)],
            Stroke::new(stroke_thin(), Color32::from_rgba_unmultiplied(0, 0, 0, a)));
    }
    ui.add_space(crate::dt_f32!(separator.shadow_space, 4.0));
}

/// Indented section label with left margin — used inside dialogs.
pub fn dialog_section(ui: &mut egui::Ui, text: &str, margin: f32, color: Color32) {
    ui.horizontal(|ui| {
        ui.add_space(margin);
        ui.label(RichText::new(text).monospace().size(font_sm()).strong().color(color));
    });
    ui.add_space(gap_xs() + 1.0);
}

// ─── Labels ───────────────────────────────────────────────────────────────────

/// Section header — FONT_SM bold.
#[inline]
pub fn section_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).monospace().size(7.0).strong().color(color));
}

/// Dim info label — FONT_SM regular.
#[inline]
pub fn dim_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).monospace().size(font_sm()).color(color));
}

/// Column header cell — FONT_XS dim monospace, fixed width.
/// `right_align = true` for numeric columns (PRICE, SIZE), false for text (SYMBOL, TIME).
pub fn col_header(ui: &mut egui::Ui, text: &str, width: f32, color: Color32, right_align: bool) {
    let layout = if right_align {
        egui::Layout::right_to_left(egui::Align::Center)
    } else {
        egui::Layout::left_to_right(egui::Align::Center)
    };
    ui.allocate_ui_with_layout(egui::vec2(width, crate::dt_f32!(table.header_height, 12.0)), layout, |ui| {
        ui.label(RichText::new(text).monospace().size(font_xs()).color(color));
    });
}

// ─── Segmented control ───────────────────────────────────────────────────────

/// Pill group of buttons with a sunken inset trough. Returns `Some(index)` of the clicked
/// segment, `None` if nothing clicked. Caller updates state on `Some(i)`.
///
/// Uses a painter-reservation approach: buttons are rendered in the normal horizontal flow
/// (so `horizontal_centered` can center them correctly), and the trough background is
/// painted behind them via a reserved painter slot — avoiding Frame centering issues.
pub fn segmented_control(
    ui: &mut egui::Ui,
    active_idx: usize,
    labels: &[&str],
    toolbar_bg: Color32,
    toolbar_border: Color32,
    accent: Color32,
    dim: Color32,
) -> Option<usize> {
    let mut clicked = None;

    let td = crate::dt_u8!(segmented.trough_darken, 12);
    let trough = Color32::from_rgb(
        toolbar_bg.r().saturating_sub(td),
        toolbar_bg.g().saturating_sub(td),
        toolbar_bg.b().saturating_sub(td),
    );
    let border_col = color_alpha(toolbar_border, alpha_strong());

    let bg_slot = ui.painter().add(egui::Shape::Noop);

    let prev_spacing = ui.spacing().item_spacing.x;
    ui.spacing_mut().item_spacing.x = gap_xs();

    let mut union_rect: Option<egui::Rect> = None;
    let n = labels.len();
    let rsm = radius_sm() as u8;
    let seg_btn_h = 20.0;
    let seg_pad_x = 5.0;

    for (i, label) in labels.iter().enumerate() {
        let active = i == active_idx;
        let fg = if active { accent } else { dim };
        let bg = if active { color_alpha(accent, alpha_tint() + 5) } else { Color32::TRANSPARENT };
        let cr = match (i, n) {
            (0, 1) => egui::CornerRadius::same(rsm),
            (0, _) => egui::CornerRadius { nw: rsm, sw: rsm, ne: 0, se: 0 },
            (x, n) if x == n - 1 => egui::CornerRadius { nw: 0, sw: 0, ne: rsm, se: rsm },
            _ => egui::CornerRadius::ZERO,
        };
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(seg_pad_x, prev_pad.y);
        let resp = ui.add(
            egui::Button::new(RichText::new(*label).monospace().size(11.0).strong().color(fg))
                .fill(bg).stroke(Stroke::NONE).corner_radius(cr)
                .min_size(egui::vec2(0.0, seg_btn_h))
        );
        ui.spacing_mut().button_padding = prev_pad;
        union_rect = Some(union_rect.map_or(resp.rect, |r: egui::Rect| r.union(resp.rect)));
        if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
        if resp.clicked() { clicked = Some(i); }
    }

    ui.spacing_mut().item_spacing.x = prev_spacing;

    if let Some(ur) = union_rect {
        let trough_expand = crate::dt_f32!(segmented.trough_expand_x, 4.0);
        let trough_rect = ur.expand2(egui::vec2(trough_expand, 0.0));
        let r = radius_md() + 1.0;
        ui.painter().set(bg_slot, egui::Shape::rect_filled(trough_rect, r, trough));
        ui.painter().rect_stroke(trough_rect, r, Stroke::new(stroke_thin(), border_col), egui::StrokeKind::Outside);
    }

    clicked
}

// ─── Panel chrome ─────────────────────────────────────────────────────────────

/// Square icon button with hover highlight — always renders as a true square hit target.
/// Internally zeroes button_padding so egui doesn't add asymmetric whitespace around the icon.
/// Returns the full Response so callers can chain `.clicked()`, `.on_hover_text()`, etc.
pub fn icon_btn(ui: &mut egui::Ui, icon: &str, color: Color32, size: f32) -> egui::Response {
    let side = (size + 8.0).max(22.0);
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);
    let resp = ui.add(
        egui::Button::new(RichText::new(icon).size(size).color(color))
            .frame(false)
            .min_size(egui::vec2(side, side))
    );
    ui.spacing_mut().button_padding = prev_pad;
    hit(&resp.rect, "ICON_BTN", "Icon Buttons");
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(color, alpha_ghost()));
        ui.painter().rect_stroke(resp.rect, radius_sm(),
            egui::Stroke::new(stroke_thin(), color_alpha(color, alpha_muted())), egui::StrokeKind::Inside);
    }
    resp
}

/// Close button (X icon) — square icon_btn, standard panel close.
#[inline]
pub fn close_button(ui: &mut egui::Ui, dim: Color32) -> bool {
    icon_btn(ui, super::super::super::ui_kit::icons::Icon::X, dim, font_lg()).clicked()
}

/// Panel header — FONT_LG title + close button. Returns true if closed.
pub fn panel_header(ui: &mut egui::Ui, title: &str, accent: Color32, dim: Color32) -> bool {
    panel_header_sub(ui, title, None, accent, dim)
}

/// Panel header with optional subtitle text. Returns true if closed.
pub fn panel_header_sub(ui: &mut egui::Ui, title: &str, subtitle: Option<&str>, accent: Color32, dim: Color32) -> bool {
    let mut closed = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).monospace().size(11.0).strong().color(accent));
        if let Some(sub) = subtitle {
            ui.label(RichText::new(sub).monospace().size(9.0).color(dim));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if close_button(ui, dim) { closed = true; }
        });
    });
    closed
}

/// Horizontal tab bar — 2px underline on active tab. Renders inline; wrap in `ui.horizontal`.
pub fn tab_bar<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    current: &mut T,
    tabs: &[(T, &str)],
    accent: Color32,
    dim: Color32,
) {
    let tab_ul = crate::dt_f32!(tab.underline_thickness, 2.0);
    for (tab, label) in tabs {
        let active = *current == *tab;
        let color = if active { accent } else { dim };
        let resp = ui.add(
            egui::Button::new(RichText::new(*label).monospace().size(font_lg()).strong().color(color))
                .frame(false)
        );
        if resp.clicked() { *current = *tab; }
        if active {
            let r = resp.rect;
            ui.painter().rect_filled(
                egui::Rect::from_min_max(egui::pos2(r.left(), r.max.y - tab_ul), egui::pos2(r.right(), r.max.y)),
                0.0, accent);
        }
    }
}

// ─── Tooltip infrastructure ───────────────────────────────────────────────────

/// Standard tooltip `egui::Frame` — use with `resp.on_hover_ui(|ui| { tooltip_frame(...).show(ui, |ui| { ... }) })`.
/// Matches the watchlist deferred tooltip style.
pub fn tooltip_frame(toolbar_bg: Color32, toolbar_border: Color32) -> egui::Frame {
    egui::Frame::NONE
        .fill(toolbar_bg)
        .stroke(Stroke::new(stroke_thin(), color_alpha(toolbar_border, alpha_strong())))
        .inner_margin(crate::dt_f32!(tooltip.padding, 8.0))
        .corner_radius(crate::dt_f32!(tooltip.corner_radius, 8.0))
}

/// Single stat row inside a tooltip — label left, value right.
pub fn stat_row(ui: &mut egui::Ui, label: &str, value: &str, label_color: Color32, value_color: Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).monospace().size(crate::dt_f32!(tooltip.stat_label_size, 8.0)).color(label_color));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).monospace().size(crate::dt_f32!(tooltip.stat_value_size, 10.0)).strong().color(value_color));
        });
    });
}

/// Paint a drop shadow behind a painter-based tooltip rect (call BEFORE painting the bg).
pub fn paint_tooltip_shadow(painter: &egui::Painter, rect: egui::Rect, radius: f32) {
    let shadow_rect = rect.translate(egui::vec2(shadow_offset(), shadow_offset()));
    painter.rect_filled(shadow_rect, radius, Color32::from_rgba_unmultiplied(0, 0, 0, shadow_alpha()));
}

// ─── Utility ──────────────────────────────────────────────────────────────────

/// Convert hex color string to Color32 with opacity.
pub fn hex_to_color(hex: &str, opacity: f32) -> Color32 {
    let h = hex.trim_start_matches('#');
    let r = u8::from_str_radix(h.get(0..2).unwrap_or("80"), 16).unwrap_or(128);
    let g = u8::from_str_radix(h.get(2..4).unwrap_or("80"), 16).unwrap_or(128);
    let b = u8::from_str_radix(h.get(4..6).unwrap_or("80"), 16).unwrap_or(128);
    Color32::from_rgba_unmultiplied(r, g, b, (opacity * 255.0) as u8)
}

/// Color with alpha — shorthand for `Color32::from_rgba_unmultiplied(r, g, b, alpha)`.
#[inline]
pub fn color_alpha(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

// ─── Form layout ──────────────────────────────────────────────────────────────

/// Form row: right-aligned fixed-width label + content widget.
pub fn form_row(ui: &mut egui::Ui, label: &str, label_width: f32, dim: Color32, add_content: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.allocate_ui(egui::vec2(label_width, crate::dt_f32!(form.row_height, 18.0)), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(gap_sm());
                ui.label(RichText::new(label).monospace().size(font_sm()).color(dim));
            });
        });
        add_content(ui);
    });
}

// ─── Cards / badges ───────────────────────────────────────────────────────────

/// Status badge — small tinted pill (e.g. "DRAFT", "PLACED", "TRIGGERED").
pub fn status_badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    let resp = ui.add(egui::Button::new(RichText::new(text).monospace().size(crate::dt_f32!(badge.font_size, 8.0)).strong().color(color))
        .fill(color_alpha(color, alpha_subtle()))
        .stroke(Stroke::new(stroke_thin(), color_alpha(color, alpha_dim())))
        .corner_radius(radius_sm())
        .min_size(egui::vec2(0.0, crate::dt_f32!(badge.height, 16.0))));
    hit(&resp.rect, "BADGE", "Badges");
}

/// Order card — left accent stripe + subtle bg. Returns true if the card area was clicked.
pub fn order_card(ui: &mut egui::Ui, accent: Color32, bg: Color32, add_content: impl FnOnce(&mut egui::Ui)) -> bool {
    let ml = crate::dt_i8!(card.margin_left, 9);
    let mr = crate::dt_i8!(card.margin_right, 6);
    let my = crate::dt_i8!(card.margin_y, 5);
    let cr = crate::dt_f32!(card.radius, 4.0);
    let available_w = ui.available_width();
    let resp = egui::Frame::NONE
        .fill(bg)
        .inner_margin(egui::Margin { left: ml, right: mr, top: my, bottom: my })
        .corner_radius(cr)
        .show(ui, |ui| {
            ui.set_min_width(available_w - 15.0);
            let outer = ui.min_rect();
            let stripe = egui::Rect::from_min_max(
                egui::pos2(outer.left() - ml as f32, outer.top() - my as f32),
                egui::pos2(outer.left() - ml as f32 + crate::dt_f32!(card.stripe_width, 3.0), outer.bottom() + my as f32));
            ui.painter().rect_filled(stripe, egui::CornerRadius { nw: cr as u8, sw: cr as u8, ne: 0, se: 0 }, accent);
            add_content(ui);
        });
    let card_rect = resp.response.rect;
    let click_resp = ui.interact(card_rect, ui.id().with(("card_click", card_rect.min.x as i32, card_rect.min.y as i32)), egui::Sense::click());
    if click_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    ui.add_space(gap_sm());
    click_resp.clicked()
}

// ─── Buttons ──────────────────────────────────────────────────────────────────

/// Action button — tinted bg, for Place/Cancel/Clear. Disabled = greyed out.
pub fn action_btn(ui: &mut egui::Ui, label: &str, color: Color32, enabled: bool) -> bool {
    let bg     = if enabled { color_alpha(color, alpha_muted())  } else { color_alpha(color, alpha_faint())  };
    let fg     = if enabled { color                              } else { color_alpha(color, alpha_active()) };
    let border = if enabled { color_alpha(color, alpha_active()) } else { color_alpha(color, alpha_line())   };
    let resp = ui.add_enabled(enabled,
        egui::Button::new(RichText::new(label).monospace().size(9.0).strong().color(fg))
            .fill(bg).stroke(Stroke::new(0.5, border))
            .corner_radius(3.0).min_size(egui::vec2(0.0, 20.0)));
    hit(&resp.rect, "ACTION_BTN", "Buttons");
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    resp.clicked()
}

/// Trade button — deep saturated bg for BUY/SELL. White bold text.
pub fn trade_btn(ui: &mut egui::Ui, label: &str, color: Color32, width: f32) -> bool {
    let bright = crate::dt_f32!(button.trade_brightness, 0.55);
    let bg = Color32::from_rgb(
        (color.r() as f32 * bright) as u8,
        (color.g() as f32 * bright) as u8,
        (color.b() as f32 * bright) as u8);
    let resp = ui.add(egui::Button::new(RichText::new(label).monospace().size(11.0).strong().color(Color32::WHITE))
        .fill(bg).min_size(egui::vec2(width, 24.0)).corner_radius(3.0));
    hit(&resp.rect, "TRADE_BTN", "Buttons");
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        let hb = crate::dt_f32!(button.trade_hover_brightness, 0.7);
        let hover_bg = Color32::from_rgb(
            (color.r() as f32 * hb).min(255.0) as u8,
            (color.g() as f32 * hb).min(255.0) as u8,
            (color.b() as f32 * hb).min(255.0) as u8);
        ui.painter().rect_filled(resp.rect, radius_md(), hover_bg);
        ui.painter().text(resp.rect.center(), egui::Align2::CENTER_CENTER,
            label, egui::FontId::monospace(font_lg()), Color32::WHITE);
    }
    resp.clicked()
}

/// Small action button — for inline header actions like "Clear All", "Close All".
pub fn small_action_btn(ui: &mut egui::Ui, label: &str, color: Color32) -> bool {
    let resp = ui.add(egui::Button::new(RichText::new(label).monospace().size(font_sm()).strong().color(color))
        .fill(color_alpha(color, alpha_soft()))
        .corner_radius(radius_sm())
        .stroke(Stroke::new(stroke_thin(), color_alpha(color, alpha_dim())))
        .min_size(egui::vec2(0.0, 16.0)));
    hit(&resp.rect, "SMALL_BTN", "Buttons");
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    resp.clicked()
}

/// Simple button — subtle border, for form actions (Create, Cancel).
pub fn simple_btn(ui: &mut egui::Ui, label: &str, color: Color32, min_width: f32) -> bool {
    let resp = ui.add(egui::Button::new(RichText::new(label).monospace().size(font_sm()).color(color))
        .fill(color_alpha(color, alpha_faint()))
        .stroke(Stroke::new(stroke_thin(), color_alpha(color, alpha_muted())))
        .corner_radius(radius_sm())
        .min_size(egui::vec2(min_width, 18.0)));
    hit(&resp.rect, "SIMPLE_BTN", "Buttons");
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    resp.clicked()
}

// ─── Drawing helpers ──────────────────────────────────────────────────────────

/// Draw a dashed or dotted line between two points.
pub fn dashed_line(painter: &egui::Painter, a: egui::Pos2, b: egui::Pos2, stroke: Stroke, style: super::super::LineStyle) {
    use super::super::LineStyle;
    let dir = b - a;
    let len = dir.length();
    if len < 1.0 || !len.is_finite() || len > 20000.0 { return; }
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
                    if idx + 3 < rgba.len() { rgba[idx..idx + 4].copy_from_slice(&color); }
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Split-section sidebar helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Draggable divider between two split sections. Returns vertical drag delta.
pub fn split_divider(ui: &mut egui::Ui, _id_salt: &str, dim: Color32) -> f32 {
    let div_h = crate::dt_f32!(split_divider.height, 6.0);
    let inset = crate::dt_f32!(split_divider.inset, 8.0);
    let dot_r = crate::dt_f32!(split_divider.dot_radius, 1.5);
    let dot_sp = crate::dt_f32!(split_divider.dot_spacing, 8.0);
    let active_sw = crate::dt_f32!(split_divider.active_stroke, 2.0);
    let inactive_sw = crate::dt_f32!(split_divider.inactive_stroke, 1.0);

    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), div_h), egui::Sense::drag());
    let p = ui.painter();

    let active = resp.hovered() || resp.dragged();
    let color = if active { dim.gamma_multiply(0.6) } else { color_alpha(dim, alpha_faint()) };

    p.line_segment(
        [egui::pos2(rect.left() + inset, rect.center().y),
         egui::pos2(rect.right() - inset, rect.center().y)],
        Stroke::new(if active { active_sw } else { inactive_sw }, color));

    if active {
        let cy = rect.center().y;
        let cx = rect.center().x;
        for dx in [-dot_sp, 0.0, dot_sp] {
            p.circle_filled(egui::pos2(cx + dx, cy), dot_r, dim.gamma_multiply(0.4));
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }

    if resp.dragged() { resp.drag_delta().y } else { 0.0 }
}
