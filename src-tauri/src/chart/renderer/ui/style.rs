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
pub fn font_xs()       -> f32 { crate::dt_f32!(font.xs, 8.0) }
/// 9.0 — between xs and sm; used by watchlist section headers and badge overlays.
pub fn font_sm_tight() -> f32 { crate::dt_f32!(font.sm_tight, 9.0) }
pub fn font_sm()       -> f32 { crate::dt_f32!(font.sm, 10.0) }
pub fn font_md()  -> f32 { crate::dt_f32!(font.md, 11.0) }
pub fn font_lg()  -> f32 { crate::dt_f32!(font.lg, 14.0) }
pub fn font_xl()  -> f32 { crate::dt_f32!(font.xl, 15.0) }
pub fn font_2xl() -> f32 { crate::dt_f32!(font.xxl, 15.0) }

// Keep the old names as non-const for backwards compat with all call sites.
// Without design-mode feature, the compiler inlines these to the literal values.
pub const FONT_XS:  f32 = 8.0;
pub const FONT_SM:  f32 = 10.0;
pub const FONT_MD:  f32 = 11.0;
pub const FONT_LG:  f32 = 12.0;
pub const FONT_XL:  f32 = 13.0;
pub const FONT_2XL: f32 = 14.0;

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

pub const RADIUS_SM: f32 = 4.0;
pub const RADIUS_MD: f32 = 6.0;
pub const RADIUS_LG: f32 = 12.0;

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

// ─── Status color tokens ─────────────────────────────────────────────────────
/// Green — active / live / filled (status_ok).
pub fn status_ok()    -> Color32 { crate::dt_rgba!(status.ok,    [120, 180, 120, 255]) }
/// Orange — warning / pending (status_warn).
pub fn status_warn()  -> Color32 { crate::dt_rgba!(status.warn,  [255, 165,   0, 255]) }
/// Red — error / rejected (status_error).
pub fn status_error() -> Color32 { crate::dt_rgba!(status.error, [224,  85,  96, 255]) }
/// Blue/purple — informational (status_info).
pub fn status_info()  -> Color32 { crate::dt_rgba!(status.info,  [100, 200, 255, 255]) }

// ─── Drawing palette tokens ──────────────────────────────────────────────────
/// Four link-group identity colors: blue, green, orange, purple.
pub fn drawing_palette() -> [Color32; 4] {
    #[cfg(feature = "design-mode")]
    if let Some(t) = crate::design_tokens::get() {
        let p = t.drawing.palette;
        return p.map(|[r, g, b, a]| Color32::from_rgba_unmultiplied(r, g, b, a));
    }
    [
        Color32::from_rgb( 70, 130, 255),
        Color32::from_rgb( 80, 200, 120),
        Color32::from_rgb(255, 160,  60),
        Color32::from_rgb(180, 100, 255),
    ]
}

// ─── Semantic accent colors (design-system tokens) ───────────────────────────
/// Amber — used for "Active" status, R:R ≥ 1 indicator, and warning states.
pub const COLOR_AMBER: Color32 = Color32::from_rgb(255, 191, 0);
/// Teal — T2 target label color (second exit level).
pub const COLOR_T2: Color32 = Color32::from_rgb(26, 188, 156);
/// Blue — T3 target label color (third exit level).
pub const COLOR_T3: Color32 = Color32::from_rgb(52, 152, 219);

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
/// True when every char in `s` is in a Phosphor / icon private-use codepoint
/// range. Lets us detect "icon-only" toolbar buttons so we can render their
/// glyphs ~50% larger than text labels without breaking text-button sizing.
fn label_is_icon_only(s: &str) -> bool {
    if s.is_empty() { return false; }
    s.chars().all(|c| {
        let cp = c as u32;
        // Private Use Area (U+E000–U+F8FF) — where Phosphor glyphs live.
        // Allow ASCII whitespace as a separator (e.g., "{ICON} {count}").
        (0xE000..=0xF8FF).contains(&cp)
            || (0xF0000..=0x10FFFF).contains(&cp)
            || c.is_ascii_whitespace()
            || c.is_ascii_digit()
    })
}

pub fn tb_btn(ui: &mut egui::Ui, label: &str, active: bool, accent: Color32, dim: Color32, toolbar_bg: Color32, toolbar_border: Color32) -> egui::Response {
    let st = current();
    // Apply uppercase transform per active style (#5).
    let raw_label = style_label_case(label);
    // Icon-only buttons render their glyph ~50% larger than text labels.
    let label_size = if label_is_icon_only(label) { font_md() * 1.5 } else { font_md() };
    // Apply nav letter-spacing approximation via thin-spaces (U+2009).
    let nav_sp = st.nav_letter_spacing_px;
    let display_label = if nav_sp < 0.5 {
        raw_label
    } else {
        let sep = if nav_sp > 1.5 { "\u{2009}\u{2009}" } else { "\u{2009}" };
        raw_label.chars().map(|c| c.to_string()).collect::<Vec<_>>().join(sep)
    };
    let corner_r = st.r_sm as f32;

    // Resolve active fill/text from style overrides or fallback to accent.
    let active_fill = st.active_fill_color.unwrap_or(accent);
    let active_text = st.active_text_color.unwrap_or(accent);

    // Button treatment dispatch (#18).
    let (bg, fg, border) = match st.button_treatment {
        ButtonTreatment::UnderlineActive => {
            // Transparent idle; active uses active_fill/text overrides.
            let fg = if active { active_text } else { dim };
            (Color32::TRANSPARENT, fg, Color32::TRANSPARENT)
        }
        _ => {
            let bg = if active {
                if st.active_fill_color.is_some() { active_fill } else { color_alpha(accent, alpha_tint()) }
            } else { color_alpha(toolbar_border, alpha_ghost()) };
            let fg = if active { active_text } else { dim };
            let border = if active { color_alpha(accent, alpha_active()) } else { color_alpha(toolbar_border, alpha_muted()) };
            (bg, fg, border)
        }
    };

    // For UnderlineActive (Meridien), paint the column tint BEFORE the button
    // via the Background layer so the button's text/fill renders on top.
    if matches!(st.button_treatment, ButtonTreatment::UnderlineActive) {
        // We need the button rect first. Allocate exact size, paint bg, then add button inside.
        let btn_width = {
            let galley = ui.fonts(|f| f.layout_no_wrap(
                display_label.clone(),
                egui::FontId::monospace(label_size),
                Color32::WHITE,
            ));
            galley.rect.width() + 16.0 // approx button padding
        };
        let btn_size = egui::vec2(btn_width.max(0.0), 24.0);
        let (btn_rect, _btn_sense) = ui.allocate_exact_size(btn_size, egui::Sense::hover());
        let tb = toolbar_rect();
        let col_rect = egui::Rect::from_min_max(
            egui::pos2(btn_rect.left(), tb.top()),
            egui::pos2(btn_rect.right(), tb.bottom()),
        );
        // Paint column tint in Background layer so button draws on top.
        // nav_active_col_alpha controls the column tint alpha for the active nav button.
        let bg_painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("tb_btn_col_bg")));
        let col_tint = if active {
            color_alpha(toolbar_border, st.nav_active_col_alpha.max(alpha_ghost()))
        } else if ui.rect_contains_pointer(btn_rect) {
            color_alpha(dim, alpha_ghost())
        } else {
            Color32::TRANSPARENT
        };
        bg_painter.rect_filled(col_rect, 0.0, col_tint);
        if active {
            let ul_thickness = if st.tab_underline_thickness > 0.0 { st.tab_underline_thickness } else { st.stroke_bold };
            let underline_y = tb.bottom() - 1.0;
            bg_painter.line_segment(
                [egui::pos2(btn_rect.left(), underline_y), egui::pos2(btn_rect.right(), underline_y)],
                Stroke::new(ul_thickness, active_fill));
        }
        // Place the actual button in the already-allocated rect via put().
        let resp = ui.put(btn_rect, egui::Button::new(RichText::new(display_label).monospace().size(label_size).color(fg))
            .wrap_mode(egui::TextWrapMode::Extend)
            .fill(Color32::TRANSPARENT).stroke(Stroke::new(0.0, Color32::TRANSPARENT)).corner_radius(corner_r));
        hit(&resp.rect, "TOOLBAR_BTN", "Toolbar");
        if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let _ = toolbar_bg;
        return resp;
    }

    let resp = ui.add(egui::Button::new(RichText::new(display_label).monospace().size(label_size).color(fg))
        .wrap_mode(egui::TextWrapMode::Extend)
        .fill(bg).stroke(Stroke::new(stroke_thin(), border)).corner_radius(corner_r)
        .min_size(egui::vec2(0.0, 24.0)));
    hit(&resp.rect, "TOOLBAR_BTN", "Toolbar");

    if active {
        let r = resp.rect;
        // stroke_bold drives the active-state accent underline for non-Meridien styles.
        ui.painter().line_segment(
            [egui::pos2(r.left() + 4.0, r.bottom() + 0.5),
             egui::pos2(r.right() - 4.0, r.bottom() + 0.5)],
            Stroke::new(st.stroke_bold, color_alpha(accent, alpha_dim())));
    } else if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        // Bevel highlight on top edge
        let r = resp.rect;
        ui.painter().rect_filled(
            egui::Rect::from_min_max(r.min, egui::pos2(r.right(), r.top() + 1.0)),
            egui::CornerRadius { nw: corner_r as u8, ne: corner_r as u8, sw: 0, se: 0 },
            Color32::from_rgba_unmultiplied(255, 255, 255, 10));
    }
    let _ = toolbar_bg; // may be used for future hover tint
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

/// Theme-aware dialog window — rich shadow when shadows_enabled, flat hairline when not (#16).
pub fn dialog_window_themed(ctx: &egui::Context, id: &str, pos: egui::Pos2, width: f32, toolbar_bg: Color32, toolbar_border: Color32, border_color: Option<Color32>) -> egui::Window<'static> {
    let st = current();
    let border = border_color.unwrap_or(color_alpha(toolbar_border, alpha_strong()));
    let corner_r = if st.r_lg == 0 { 0.0 } else { radius_lg() };
    let shadow = if st.shadows_enabled {
        egui::epaint::Shadow {
            offset: [0, 8],
            blur: 28,
            spread: 2,
            color: Color32::from_black_alpha(80),
        }
    } else if st.card_floating_shadow {
        egui::epaint::Shadow {
            offset: [0, 3],
            blur: 8,
            spread: 0,
            color: Color32::from_black_alpha(st.card_floating_shadow_alpha),
        }
    } else {
        egui::epaint::Shadow::NONE
    };
    egui::Window::new(id.to_string())
        .fixed_pos(pos).fixed_size(egui::vec2(width, 0.0))
        .title_bar(false)
        .frame(egui::Frame::popup(&ctx.style())
            .fill(toolbar_bg)
            .inner_margin(0.0)
            .stroke(Stroke::new(st.stroke_std, border))
            .corner_radius(corner_r)
            .shadow(shadow))
}

/// Dialog header bar — auto-darkened bg, FONT_LG title, X close. Returns true if closed.
pub fn dialog_header(ui: &mut egui::Ui, title: &str, dim: Color32) -> bool {
    dialog_header_colored(ui, title, dim, None)
}

/// Dialog header bar with explicit header background.
pub fn dialog_header_colored(ui: &mut egui::Ui, title: &str, dim: Color32, header_bg: Option<Color32>) -> bool {
    use crate::ui_kit::icons::Icon;
    let darken = crate::dt_u8!(dialog.header_darken, 8);
    let fill = header_bg.unwrap_or_else(|| {
        let bg = ui.visuals().window_fill();
        Color32::from_rgb(bg.r().saturating_sub(darken), bg.g().saturating_sub(darken), bg.b().saturating_sub(darken))
    });
    let mut closed = false;
    let rlg = current().r_lg;
    egui::Frame::NONE.fill(fill)
        .inner_margin(egui::Margin { left: gap_lg() as i8, right: gap_lg() as i8, top: gap_lg() as i8, bottom: gap_lg() as i8 })
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
/// Uses `stroke_hair` when the active style has hairline_borders, otherwise `stroke_thin` —
/// giving Meridien its characteristic super-thin dividers.
#[inline]
pub fn separator(ui: &mut egui::Ui, color: Color32) {
    let st = current();
    let sw = if st.hairline_borders { st.stroke_hair } else { stroke_thin() };
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [egui::pos2(rect.left(), ui.cursor().min.y), egui::pos2(rect.right(), ui.cursor().min.y)],
        Stroke::new(sw, color));
    ui.add_space(crate::dt_f32!(separator.after_space, 1.0));
}

/// Inset separator with margins on both sides.
/// Uses `stroke_hair` when the active style has hairline_borders, otherwise `stroke_thin`.
pub fn dialog_separator(ui: &mut egui::Ui, margin: f32, color: Color32) {
    let st = current();
    let sw = if st.hairline_borders { st.stroke_hair } else { stroke_thin() };
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [egui::pos2(rect.left() + margin, ui.cursor().min.y),
         egui::pos2(rect.right() - margin, ui.cursor().min.y)],
        Stroke::new(sw, color));
    ui.add_space(crate::dt_f32!(separator.after_space, 1.0));
}

/// Inset separator + soft gradient shadow below (3 fading lines).
/// Uses `stroke_thick` for the main divider line so bold-separator sites are style-driven.
pub fn dialog_separator_shadow(ui: &mut egui::Ui, margin: f32, color: Color32) {
    let rect = ui.available_rect_before_wrap();
    let y = ui.cursor().min.y;
    let left = rect.left() + margin;
    let right = rect.right() - margin;
    ui.painter().line_segment([egui::pos2(left, y), egui::pos2(right, y)], Stroke::new(current().stroke_thick, color));
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

/// Section header — FONT_SM bold. Uppercases label when the active style requires it (#12).
/// Adds `section_label_padding_top` space before and `section_label_padding_bottom` after.
#[inline]
pub fn section_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    let st = current();
    if st.section_label_padding_top > 0.0 { ui.add_space(st.section_label_padding_top); }
    let label = style_label_case(text);
    ui.label(RichText::new(label).monospace().size(7.0).strong().color(color));
    if st.section_label_padding_bottom > 0.0 { ui.add_space(st.section_label_padding_bottom); }
}

/// Extra-small section label — dim monospace at 6 pt, uppercase when style requires (#12).
#[inline]
pub fn section_label_xs(ui: &mut egui::Ui, text: &str, color: Color32) {
    let label = style_label_case(text);
    ui.label(RichText::new(label).monospace().size(6.0).color(color));
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
            egui::Button::new(RichText::new(*label).monospace().size(font_md()).strong().color(fg))
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
    icon_btn(ui, crate::ui_kit::icons::Icon::X, dim, font_lg()).clicked()
}

/// Panel header — FONT_LG title + close button. Returns true if closed.
pub fn panel_header(ui: &mut egui::Ui, title: &str, accent: Color32, dim: Color32) -> bool {
    panel_header_sub(ui, title, None, accent, dim)
}

/// Panel header with optional subtitle text. Returns true if closed.
pub fn panel_header_sub(ui: &mut egui::Ui, title: &str, subtitle: Option<&str>, accent: Color32, dim: Color32) -> bool {
    let mut closed = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).monospace().size(font_md()).strong().color(accent));
        if let Some(sub) = subtitle {
            ui.label(RichText::new(sub).monospace().size(font_sm()).color(dim));
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
        if active && crate::chart_renderer::ui::style::current().show_active_tab_underline {
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
    // r_chip overrides r_sm for badges/chips when non-zero; allows pill chips alongside
    // square buttons on the same style.
    let st = current();
    let chip_r = if st.r_chip > 0 { st.r_chip as f32 } else { radius_sm() };
    let resp = ui.add(egui::Button::new(RichText::new(text).monospace().size(crate::dt_f32!(badge.font_size, 8.0)).strong().color(color))
        .fill(color_alpha(color, alpha_subtle()))
        .stroke(Stroke::new(stroke_thin(), color_alpha(color, alpha_dim())))
        .corner_radius(chip_r)
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
            let stripe_col = color_alpha(accent, current().card_stripe_alpha);
            ui.painter().rect_filled(stripe, egui::CornerRadius { nw: cr as u8, sw: cr as u8, ne: 0, se: 0 }, stripe_col);
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

/// Primary CTA button — solid filled accent, style-driven height/padding.
/// Use for the "REVIEW BUY" / "PLACE ORDER" terminal action at the bottom of order tickets.
/// The fill color and text color follow `active_fill_color` / `active_text_color` overrides
/// when set (Newsprint: black fill + white text), otherwise uses `color` directly.
pub fn cta_btn(ui: &mut egui::Ui, label: &str, color: Color32, enabled: bool) -> bool {
    let st = current();
    let fill = st.active_fill_color.unwrap_or(color);
    let fg   = st.active_text_color.unwrap_or(Color32::WHITE);
    let h    = st.cta_height_px;
    let px   = st.cta_padding_x;
    let cr   = st.r_sm as f32;
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(px, 4.0);
    let resp = ui.add_enabled(enabled,
        egui::Button::new(RichText::new(label).monospace().size(font_md()).strong().color(fg))
            .fill(if enabled { fill } else { color_alpha(fill, alpha_muted()) })
            .stroke(Stroke::NONE)
            .corner_radius(cr)
            .min_size(egui::vec2(ui.available_width(), h)));
    ui.spacing_mut().button_padding = prev_pad;
    hit(&resp.rect, "CTA_BTN", "Buttons");
    if resp.hovered() && enabled && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
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
    let st_dh = current();
    let handle_alpha_mult = if active { 1.0 } else { st_dh.drag_handle_alpha };
    let color = if active { dim.gamma_multiply(0.6) } else {
        color_alpha(dim, (alpha_faint() as f32 * (handle_alpha_mult / 0.5).min(1.0)) as u8)
    };

    // Active drag handle uses stroke_thick from the style preset for a prominent feel.
    let effective_active_sw = st_dh.stroke_thick.max(active_sw);
    p.line_segment(
        [egui::pos2(rect.left() + inset, rect.center().y),
         egui::pos2(rect.right() - inset, rect.center().y)],
        Stroke::new(if active { effective_active_sw } else { inactive_sw }, color));

    if active {
        let cy = rect.center().y;
        let cx = rect.center().x;
        let scaled_dot_r = dot_r * st_dh.drag_handle_dot_scale;
        for dx in [-dot_sp, 0.0, dot_sp] {
            p.circle_filled(egui::pos2(cx + dx, cy), scaled_dot_r, dim.gamma_multiply(0.4));
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }

    if resp.dragged() { resp.drag_delta().y } else { 0.0 }
}

// ─── Compatibility shims for in-session widget builders ───────────────────────
// These were introduced alongside the new widgets/* design-system primitives.
// They centralize per-style overrides; for now they return reasonable defaults.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonTreatment {
    SoftPill,
    OutlineAccent,
    UnderlineActive,
    RaisedActive,
    BlackFillActive,
}

#[derive(Clone)]
pub struct StyleSettings {
    pub r_xs: u8,
    pub r_sm: u8,
    pub r_md: u8,
    pub r_lg: u8,
    pub r_pill: u8,
    pub serif_headlines: bool,
    pub button_treatment: ButtonTreatment,
    pub hairline_borders: bool,
    pub stroke_hair: f32,
    pub stroke_thin: f32,
    pub stroke_std: f32,
    /// Bold stroke weight — Meridien collapses to 1 px, Relay/Aperture use 1.5.
    pub stroke_bold: f32,
    pub stroke_thick: f32,
    pub shadows_enabled: bool,
    pub solid_active_fills: bool,
    pub uppercase_section_labels: bool,
    /// Letter spacing approximation (px) applied to tracked-out section labels.
    pub label_letter_spacing_px: f32,
    /// Multiplier applied when scaling toolbar height (1.0 = baseline, 1.4 = Meridien tall).
    pub toolbar_height_scale: f32,
    /// Multiplier applied when scaling pane header height (1.0 = baseline, 1.1 = Meridien).
    pub header_height_scale: f32,
    /// Hero numeric font size in pt (22 for Relay, 36 for Meridien).
    pub font_hero: f32,
    /// Paint full-height vertical divider lines between toolbar button clusters.
    pub vertical_group_dividers: bool,
    /// Show active-tab accent underline in tab bars.
    pub show_active_tab_underline: bool,
    /// Active pane header fill multiplier (1.2 = brighter for Relay, 0.95 = near-transparent for Meridien).
    pub active_header_fill_multiply: f32,
    /// Paint a distinct fill for inactive pane headers.
    pub inactive_header_fill: bool,
    /// Account strip panel height in logical px.
    pub account_strip_height: f32,

    // ── Layout & spacing ──────────────────────────────────────────────────
    /// Pane border outline thickness in logical px.
    pub pane_border_width: f32,
    /// Gap between adjacent panes in px.
    pub pane_gap: f32,
    /// Card vertical inner padding in px.
    pub card_padding_y: f32,
    /// Card horizontal inner padding in px.
    pub card_padding_x: f32,
    /// Base list-row height in px.
    pub row_height_px: f32,
    /// Base button height in px.
    pub button_height_px: f32,
    /// Button horizontal padding in px.
    pub button_padding_x: f32,
    /// Tab strip height in px.
    pub tab_height: f32,

    // ── Typography overrides ──────────────────────────────────────────────
    /// Section/eyebrow label font size in pt.
    pub font_section_label: f32,
    /// Body text font size in pt.
    pub font_body: f32,
    /// Caption font size in pt.
    pub font_caption: f32,

    // ── Interaction tokens ────────────────────────────────────────────────
    /// Alpha for hover overlay (0-255).
    pub hover_bg_alpha: u8,
    /// Alpha for active/pressed state (0-255).
    pub active_bg_alpha: u8,
    /// Focus ring stroke width.
    pub focus_ring_width: f32,
    /// Focus ring alpha (0-255).
    pub focus_ring_alpha: u8,
    /// Opacity multiplier for disabled widgets (0.0-1.0).
    pub disabled_opacity: f32,

    // ── Drop shadow ───────────────────────────────────────────────────────
    /// Shadow blur radius in px.
    pub shadow_blur: f32,
    /// Shadow vertical offset in px.
    pub shadow_offset_y: f32,
    /// Shadow alpha (0-255).
    pub shadow_alpha: u8,

    // ── Density & accent ─────────────────────────────────────────────────
    /// Global density: 0=compact, 1=normal, 2=roomy. Drives row/tab/button
    /// height multipliers when explicit overrides are not set.
    pub density: u8,
    /// Saturation/brightness multiplier for accent on active elements.
    pub accent_emphasis: f32,

    // ── Reference-match fields (Newsprint/editorial style) ────────────────
    /// Fill color for active segments/buttons. `None` = use theme.accent.
    /// Meridien: Some(BLACK), Aperture/Octave: None.
    pub active_fill_color: Option<Color32>,
    /// Text color on active segments/buttons. `None` = contrast-auto.
    /// Meridien: Some(WHITE), Aperture/Octave: None.
    pub active_text_color: Option<Color32>,
    /// Outline color for idle connected-pill segments.
    /// `None` = use toolbar_border. Meridien: Some(near-black dim).
    pub idle_outline_color: Option<Color32>,
    /// Letter-spacing added between glyphs in toolbar nav buttons (px).
    /// Meridien: 1.5, others: 0.
    pub nav_letter_spacing_px: f32,
    /// Drop icon glyphs from right-side toolbar nav buttons (label-only).
    /// Meridien: true, others: false.
    pub nav_buttons_label_only: bool,
    /// Render right-side toolbar nav button labels in ALL CAPS.
    /// Meridien: true, others: false.
    pub nav_buttons_uppercase_labels: bool,
    /// Thickness of the tab-active underline in pane headers.
    /// Meridien: 2.0, Aperture: 0.0 (hidden), Octave: 1.0.
    pub tab_underline_thickness: f32,
    /// When true, draw the underline directly under active tab text (not at header bottom).
    /// Meridien: true, others: false.
    pub tab_underline_under_text: bool,
    /// Show a subtle floating shadow on card windows even when `shadows_enabled` is false.
    /// Meridien: true, Aperture: covered by shadows_enabled, Octave: false.
    pub card_floating_shadow: bool,
    /// Alpha for the card floating shadow (0-255). Meridien: 25, others: 0.
    pub card_floating_shadow_alpha: u8,
    /// Fill for idle segments in connected-pill SegmentedControl.
    /// `None` = transparent. Meridien: None.
    pub segmented_idle_fill: Option<Color32>,
    /// Text color for idle segments. `None` = use dim.
    pub segmented_idle_text: Option<Color32>,
    /// Height for the primary CTA button in px. Meridien: 36, Aperture: 40, Octave: 32.
    pub cta_height_px: f32,
    /// Horizontal padding for the primary CTA button in px. Meridien: 16, others: 12.
    pub cta_padding_x: f32,

    // ── New knobs added in design-pass 2 ─────────────────────────────────────

    /// Pane gap fill color alpha (0-255). 0 = transparent (gap shows bg).
    /// Controls the visible color of the gutter between panes.
    /// Meridien: 0 (flush), Aperture: 30, Octave: 15.
    pub pane_gap_alpha: u8,
    /// Pane active indicator: 0=none, 1=top border line, 2=header fill, 3=both.
    /// Meridien: 1, Aperture: 2, Octave: 3.
    pub pane_active_indicator: u8,
    /// Toolbar nav background alpha for active button column tint.
    /// Meridien: 18, Aperture: 0 (none), Octave: 25.
    pub nav_active_col_alpha: u8,
    /// Alpha for the dialog/popup backdrop overlay (0-255). 0 = no backdrop.
    pub dialog_backdrop_alpha: u8,
    /// Tab inactive text alpha multiplier (0.0-1.0). 0.5 = dimmed, 1.0 = full.
    pub tab_inactive_alpha: f32,
    /// Tab hover background alpha (0-255). Applied when hovering an inactive tab.
    pub tab_hover_bg_alpha: u8,
    /// Section label top padding in px (space above eyebrow labels).
    pub section_label_padding_top: f32,
    /// Section label bottom padding in px (space below eyebrow labels before content).
    pub section_label_padding_bottom: f32,
    /// Input border focus color override. None = use accent.
    pub input_focus_color: Option<Color32>,
    /// Pane gap (gutter) fill color override. None = use toolbar_border at pane_gap_alpha.
    pub pane_gap_color: Option<Color32>,
    /// Drag handle (split divider) color alpha multiplier (0.0-1.0).
    pub drag_handle_alpha: f32,
    /// Drag handle dot size multiplier (0.5-2.0). 1.0 = default.
    pub drag_handle_dot_scale: f32,
    /// Toast / status-bar background alpha (0-255).
    pub toast_bg_alpha: u8,
    /// Stripe/accent-banner fill alpha for order/alert cards (0-255).
    pub card_stripe_alpha: u8,
    /// Pill / chip border radius separate from r_sm. 0 = use r_sm.
    /// When non-zero, overrides r_sm for badge/chip corners specifically.
    pub r_chip: u8,
}

// Active style selection — set once at the top of each draw_chart frame
// from `gpu::style_id(watchlist)`. 0 = Meridien (editorial), 1 = Aperture
// (modern, soft), 2 = Octave (dense). All other indices alias to Meridien.
static ACTIVE_STYLE: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(0);

pub fn set_active_style(id: u8) {
    ACTIVE_STYLE.store(id, std::sync::atomic::Ordering::Relaxed);
}

// Toolbar rect — set once at the start of each toolbar frame so tb_btn can
// read it for full-height hover/active column overlays (Meridien only, #18).
// Encoded as four f32 bits packed into four AtomicU32 cells (min_x, min_y, max_x, max_y).
static TB_RECT: [std::sync::atomic::AtomicU32; 4] = [
    std::sync::atomic::AtomicU32::new(0),
    std::sync::atomic::AtomicU32::new(0),
    std::sync::atomic::AtomicU32::new(0),
    std::sync::atomic::AtomicU32::new(0),
];

/// Set the toolbar rect at the start of the toolbar panel (gpu.rs ~line 3700).
pub fn set_toolbar_rect(r: egui::Rect) {
    TB_RECT[0].store(r.min.x.to_bits(), std::sync::atomic::Ordering::Relaxed);
    TB_RECT[1].store(r.min.y.to_bits(), std::sync::atomic::Ordering::Relaxed);
    TB_RECT[2].store(r.max.x.to_bits(), std::sync::atomic::Ordering::Relaxed);
    TB_RECT[3].store(r.max.y.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

/// Read the stored toolbar rect. Returns a zero-sized rect if not yet set.
pub fn toolbar_rect() -> egui::Rect {
    let min_x = f32::from_bits(TB_RECT[0].load(std::sync::atomic::Ordering::Relaxed));
    let min_y = f32::from_bits(TB_RECT[1].load(std::sync::atomic::Ordering::Relaxed));
    let max_x = f32::from_bits(TB_RECT[2].load(std::sync::atomic::Ordering::Relaxed));
    let max_y = f32::from_bits(TB_RECT[3].load(std::sync::atomic::Ordering::Relaxed));
    egui::Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y))
}

// ─── Live-editable style storage ─────────────────────────────────────────────
// Three RwLock<StyleSettings> initialised once from the hardcoded defaults.
// `current()` clones the active one; `set_style_settings` overwrites it.

// ┌─ STYLE_DEFAULTS_BEGIN ─────────────────────────────────────────────────────
fn style_defaults(id: u8) -> StyleSettings {
    match id {
        1 => StyleSettings {
            r_xs: 4, r_sm: 6, r_md: 8, r_lg: 12, r_pill: 99,
            serif_headlines: false,
            button_treatment: ButtonTreatment::SoftPill,
            hairline_borders: false,
            stroke_hair: 0.5, stroke_thin: 1.0, stroke_std: 1.5,
            stroke_bold: 1.5, stroke_thick: 2.0,
            shadows_enabled: true, solid_active_fills: false,
            uppercase_section_labels: false, label_letter_spacing_px: 0.0,
            toolbar_height_scale: 1.0, header_height_scale: 1.0,
            font_hero: 22.0, vertical_group_dividers: false,
            show_active_tab_underline: true,
            active_header_fill_multiply: 1.2, inactive_header_fill: true,
            account_strip_height: 26.0,
            pane_border_width: 1.0, pane_gap: 8.0,
            card_padding_y: 12.0, card_padding_x: 14.0,
            row_height_px: 26.0, button_height_px: 28.0, button_padding_x: 14.0,
            tab_height: 32.0,
            font_section_label: 10.0, font_body: 11.0, font_caption: 9.0,
            hover_bg_alpha: 15, active_bg_alpha: 25,
            focus_ring_width: 2.0, focus_ring_alpha: 90, disabled_opacity: 0.5,
            shadow_blur: 24.0, shadow_offset_y: 8.0, shadow_alpha: 40,
            density: 2, accent_emphasis: 1.1,
            active_fill_color: None, active_text_color: None, idle_outline_color: None,
            nav_letter_spacing_px: 0.0, tab_underline_thickness: 0.0,
            nav_buttons_label_only: false, nav_buttons_uppercase_labels: false,
            tab_underline_under_text: false, card_floating_shadow: false,
            card_floating_shadow_alpha: 0, segmented_idle_fill: None, segmented_idle_text: None,
            cta_height_px: 40.0, cta_padding_x: 12.0,
            pane_gap_alpha: 30, pane_active_indicator: 2,
            nav_active_col_alpha: 0, dialog_backdrop_alpha: 0,
            tab_inactive_alpha: 0.55, tab_hover_bg_alpha: 18,
            section_label_padding_top: 6.0, section_label_padding_bottom: 2.0,
            input_focus_color: None, pane_gap_color: None,
            drag_handle_alpha: 0.7, drag_handle_dot_scale: 1.0,
            toast_bg_alpha: 200, card_stripe_alpha: 255,
            r_chip: 0,
        },
        2 => StyleSettings {
            r_xs: 1, r_sm: 2, r_md: 3, r_lg: 4, r_pill: 99,
            serif_headlines: false,
            button_treatment: ButtonTreatment::RaisedActive,
            hairline_borders: true,
            stroke_hair: 0.4, stroke_thin: 0.6, stroke_std: 1.0,
            stroke_bold: 1.0, stroke_thick: 1.4,
            shadows_enabled: false, solid_active_fills: true,
            uppercase_section_labels: true, label_letter_spacing_px: 0.0,
            toolbar_height_scale: 1.0, header_height_scale: 1.0,
            font_hero: 22.0, vertical_group_dividers: false,
            show_active_tab_underline: true,
            active_header_fill_multiply: 1.2, inactive_header_fill: true,
            account_strip_height: 26.0,
            pane_border_width: 0.6, pane_gap: 2.0,
            card_padding_y: 6.0, card_padding_x: 8.0,
            row_height_px: 20.0, button_height_px: 22.0, button_padding_x: 8.0,
            tab_height: 26.0,
            font_section_label: 8.0, font_body: 10.0, font_caption: 8.0,
            hover_bg_alpha: 18, active_bg_alpha: 30,
            focus_ring_width: 1.5, focus_ring_alpha: 110, disabled_opacity: 0.45,
            shadow_blur: 8.0, shadow_offset_y: 4.0, shadow_alpha: 20,
            density: 0, accent_emphasis: 0.95,
            active_fill_color: None, active_text_color: None, idle_outline_color: None,
            nav_letter_spacing_px: 0.0, tab_underline_thickness: 1.0,
            nav_buttons_label_only: false, nav_buttons_uppercase_labels: false,
            tab_underline_under_text: false, card_floating_shadow: false,
            card_floating_shadow_alpha: 0, segmented_idle_fill: None, segmented_idle_text: None,
            cta_height_px: 32.0, cta_padding_x: 12.0,
            pane_gap_alpha: 15, pane_active_indicator: 3,
            nav_active_col_alpha: 25, dialog_backdrop_alpha: 0,
            tab_inactive_alpha: 0.5, tab_hover_bg_alpha: 20,
            section_label_padding_top: 3.0, section_label_padding_bottom: 1.0,
            input_focus_color: None, pane_gap_color: None,
            drag_handle_alpha: 0.6, drag_handle_dot_scale: 0.85,
            toast_bg_alpha: 220, card_stripe_alpha: 255,
            r_chip: 0,
        },
        _ => StyleSettings {
            r_xs: 0, r_sm: 0, r_md: 0, r_lg: 0, r_pill: 0,
            serif_headlines: true,
            button_treatment: ButtonTreatment::UnderlineActive,
            hairline_borders: true,
            stroke_hair: 0.5, stroke_thin: 1.0, stroke_std: 1.0,
            stroke_bold: 1.0, stroke_thick: 1.0,
            shadows_enabled: true, solid_active_fills: true,
            uppercase_section_labels: true, label_letter_spacing_px: 0.0,
            toolbar_height_scale: 1.40, header_height_scale: 1.10,
            font_hero: 36.0, vertical_group_dividers: true,
            show_active_tab_underline: true,
            active_header_fill_multiply: 0.95, inactive_header_fill: false,
            account_strip_height: 36.0,
            pane_border_width: 0.5, pane_gap: 0.0,
            card_padding_y: 8.0, card_padding_x: 10.0,
            row_height_px: 22.0, button_height_px: 24.0, button_padding_x: 10.0,
            tab_height: 28.0,
            font_section_label: 8.0, font_body: 10.0, font_caption: 8.0,
            hover_bg_alpha: 20, active_bg_alpha: 35,
            focus_ring_width: 1.0, focus_ring_alpha: 120, disabled_opacity: 0.4,
            shadow_blur: 0.0, shadow_offset_y: 0.0, shadow_alpha: 0,
            density: 1, accent_emphasis: 1.0,
            active_fill_color: Some(Color32::BLACK), active_text_color: Some(Color32::WHITE),
            idle_outline_color: Some(Color32::from_rgb(60, 56, 44)),
            nav_letter_spacing_px: 0.0, tab_underline_thickness: 2.0,
            nav_buttons_label_only: true, nav_buttons_uppercase_labels: true,
            tab_underline_under_text: true, card_floating_shadow: true,
            card_floating_shadow_alpha: 25, segmented_idle_fill: None, segmented_idle_text: None,
            cta_height_px: 36.0, cta_padding_x: 16.0,
            pane_gap_alpha: 0, pane_active_indicator: 1,
            nav_active_col_alpha: 18, dialog_backdrop_alpha: 0,
            tab_inactive_alpha: 0.6, tab_hover_bg_alpha: 12,
            section_label_padding_top: 4.0, section_label_padding_bottom: 2.0,
            input_focus_color: None, pane_gap_color: None,
            drag_handle_alpha: 0.5, drag_handle_dot_scale: 1.0,
            toast_bg_alpha: 230, card_stripe_alpha: 255,
            r_chip: 0,
        },
    }
}
// └─ STYLE_DEFAULTS_END ───────────────────────────────────────────────────────

// ─── Dynamic style preset store ──────────────────────────────────────────────
// Vec of (name, settings) pairs. Ids 0/1/2 are the canonical three styles
// (Meridien/Aperture/Octave) and cannot be deleted. User-added presets append
// beyond index 2 and survive only for the session (in-memory, no source write).

static STYLE_STORE: std::sync::OnceLock<std::sync::RwLock<Vec<(String, StyleSettings)>>> =
    std::sync::OnceLock::new();

fn style_store() -> &'static std::sync::RwLock<Vec<(String, StyleSettings)>> {
    STYLE_STORE.get_or_init(|| {
        let mut v: Vec<(String, StyleSettings)> = vec![
            ("Meridien".to_string(), style_defaults(0)),
            ("Aperture".to_string(), style_defaults(1)),
            ("Octave".to_string(),   style_defaults(2)),
        ];
        // Alias the remaining STYLE_NAMES (indices 3-9) to Meridien's settings
        // so existing style_idx values don't out-of-range on first lookup.
        let meridien = style_defaults(0);
        let alias_names = ["Cadence", "Chord", "Lattice", "Tangent", "Tempo", "Contour", "Relay"];
        for name in alias_names {
            v.push((name.to_string(), meridien.clone()));
        }
        std::sync::RwLock::new(v)
    })
}

/// Get a clone of the settings for style `id`. Falls back to 0 (Meridien) if out of range.
pub fn get_style_settings(id: u8) -> StyleSettings {
    let store = style_store().read().unwrap();
    let idx = id as usize;
    if idx < store.len() { store[idx].1.clone() } else { store[0].1.clone() }
}

/// Overwrite the settings for style `id` — takes effect on the next frame.
/// Silently ignored if `id` is out of range.
pub fn set_style_settings(id: u8, settings: StyleSettings) {
    let mut store = style_store().write().unwrap();
    let idx = id as usize;
    if idx < store.len() { store[idx].1 = settings; }
}

/// Add a new named preset cloned from an existing style. Returns the new id.
pub fn add_style_preset(name: &str, settings: StyleSettings) -> u8 {
    let mut store = style_store().write().unwrap();
    let id = store.len() as u8;
    store.push((name.to_string(), settings));
    id
}

/// Delete a user preset. Ids 0/1/2 are protected (no-op). All ids above the
/// deleted slot are shifted down — callers should re-read `list_style_presets`
/// and update any stored `style_idx` values accordingly.
pub fn delete_style_preset(id: u8) {
    if id < 3 { return; }
    let mut store = style_store().write().unwrap();
    let idx = id as usize;
    if idx < store.len() { store.remove(idx); }
}

/// Rename a preset in-place. No-op if `id` is out of range.
pub fn rename_style_preset(id: u8, new_name: String) {
    let mut store = style_store().write().unwrap();
    let idx = id as usize;
    if idx < store.len() { store[idx].0 = new_name; }
}

/// Returns `(id, name)` pairs for all registered presets — use for dropdowns.
pub fn list_style_presets() -> Vec<(u8, String)> {
    style_store().read().unwrap()
        .iter().enumerate()
        .map(|(i, (name, _))| (i as u8, name.clone()))
        .collect()
}

pub fn current() -> StyleSettings {
    let id = ACTIVE_STYLE.load(std::sync::atomic::Ordering::Relaxed);
    get_style_settings(id)
}

// Style-aware corner radius helpers — route through `current()` so corners
// flip when the active style changes (Meridien 0/0/0/0/0, Aperture 4/6/8/12/99,
// Octave 1/2/3/4/99). Previously these used static tokens which broke the
// style cascade — a popup using r_lg_cr() always got 8px regardless of style.
pub fn r_xs() -> egui::CornerRadius { egui::CornerRadius::same(current().r_xs) }
pub fn r_sm_cr() -> egui::CornerRadius { egui::CornerRadius::same(current().r_sm) }
pub fn r_md_cr() -> egui::CornerRadius { egui::CornerRadius::same(current().r_md) }
pub fn r_lg_cr() -> egui::CornerRadius { egui::CornerRadius::same(current().r_lg) }
pub fn r_pill() -> egui::CornerRadius { egui::CornerRadius::same(current().r_pill) }

pub fn btn_compact_height() -> f32 { 22.0 }
pub fn btn_simple_height() -> f32 { 24.0 }
pub fn btn_small_height() -> f32 { 22.0 }
pub fn btn_trade_height() -> f32 { 28.0 }

// ── New style-setting helpers ────────────────────────────────────────────────
/// Density-aware row height. Reads `row_height_px` then scales by density vscale.
pub fn style_row_height() -> f32 {
    let st = current();
    let scale = match st.density { 0 => 0.85, 2 => 1.15, _ => 1.0 };
    st.row_height_px * scale
}
/// Density-aware button height. Reads `button_height_px` then scales by density vscale.
pub fn style_button_height() -> f32 {
    let st = current();
    let scale = match st.density { 0 => 0.85, 2 => 1.15, _ => 1.0 };
    st.button_height_px * scale
}
/// Density-aware tab height. Reads `tab_height` then scales by density vscale.
pub fn style_tab_height() -> f32 {
    let st = current();
    let scale = match st.density { 0 => 0.85, 2 => 1.15, _ => 1.0 };
    st.tab_height * scale
}
/// Accent color with emphasis multiplier applied (brightness boost for active elements).
pub fn accent_emphasised(color: egui::Color32) -> egui::Color32 {
    color.gamma_multiply(current().accent_emphasis)
}

pub fn contrast_fg(bg: egui::Color32) -> egui::Color32 {
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if lum > 140.0 { egui::Color32::BLACK } else { egui::Color32::WHITE }
}

pub fn rule_stroke_for(_bg: egui::Color32, border: egui::Color32) -> egui::Stroke {
    // Use pane_border_width so Meridien hairlines honour the style knob.
    egui::Stroke::new(current().pane_border_width, border)
}

/// Paint a full-height inter-cluster vertical divider line in the toolbar (#4).
/// Call between button groups when `current().vertical_group_dividers` is true.
/// `panel_rect` should be the full toolbar panel rect for correct top/bottom span.
pub fn tb_group_break(ui: &mut egui::Ui, panel_rect: egui::Rect, border: egui::Color32) {
    if !current().vertical_group_dividers { return; }
    ui.add_space(gap_md());
    let x = ui.cursor().left();
    // Use alpha_heavy (120) for clearly visible dividers even on dim toolbar_border colors.
    let color = color_alpha(border, alpha_heavy());
    ui.painter().line_segment(
        [egui::pos2(x, panel_rect.top() + 2.0), egui::pos2(x, panel_rect.bottom() - 2.0)],
        egui::Stroke::new(stroke_std(), color),
    );
    ui.add_space(gap_md());
}

/// Returns `s` uppercased (and letter-spaced) for active styles that request it (#5, #12).
///
/// # Letter-spacing limitation
/// egui does not support CSS `letter-spacing`. We approximate it by inserting Unicode
/// thin-spaces (U+2009) between characters. Threshold:
///   < 0.5 px  → no spacing
///   0.5–1.5 px → single thin-space between each char
///   > 1.5 px  → double thin-space between each char
/// This is a visual approximation; the effective gap depends on font rendering.
pub fn style_label_case(s: &str) -> String {
    let st = current();
    let base = if st.uppercase_section_labels { s.to_uppercase() } else { s.to_string() };
    // Apply letter-spacing approximation via Unicode thin-spaces (U+2009).
    let sp = st.label_letter_spacing_px;
    if sp < 0.5 {
        base
    } else {
        let sep = if sp > 1.5 { "\u{2009}\u{2009}" } else { "\u{2009}" };
        base.chars().map(|c| c.to_string()).collect::<Vec<_>>().join(sep)
    }
}

/// Returns a `FontId` appropriate for hero numerics — serif when the active
/// style requests it, monospace otherwise (#14).
pub fn hero_font_id(size: f32) -> egui::FontId {
    if current().serif_headlines {
        egui::FontId::new(size, egui::FontFamily::Name("serif".into()))
    } else {
        egui::FontId::monospace(size)
    }
}

/// Builds a `RichText` for large numeric displays using the hero font (#14).
pub fn hero_text(text: &str, color: egui::Color32) -> egui::RichText {
    let size = current().font_hero;
    egui::RichText::new(text).font(hero_font_id(size)).color(color)
}

/// Apply per-style egui::Style overrides (widget visuals, spacing, shadows)
/// to the given context. Call once per frame after `set_active_style` (#3).
///
/// This is intentionally a *supplement* to the rich visual block already
/// applied in `setup_theme`; it only overrides the fields that differ
/// between styles so that non-Meridien themes remain visually unchanged.
pub fn apply_ui_style(ctx: &egui::Context, settings: &StyleSettings, toolbar_border: egui::Color32, toolbar_bg: egui::Color32) {
    let mut style = (*ctx.style()).clone();
    let is_meridien = settings.hairline_borders && settings.serif_headlines;

    if is_meridien {
        // Meridien widget fills: transparent inactive, flat hairline borders
        let inact = &mut style.visuals.widgets.inactive;
        inact.bg_fill      = egui::Color32::TRANSPARENT;
        inact.weak_bg_fill = egui::Color32::TRANSPARENT;
        inact.bg_stroke    = egui::Stroke::new(1.0, color_alpha(toolbar_border, 70));
        inact.corner_radius = egui::CornerRadius::ZERO;

        let hov = &mut style.visuals.widgets.hovered;
        hov.bg_fill      = color_alpha(toolbar_border, 18);
        hov.corner_radius = egui::CornerRadius::ZERO;

        let act = &mut style.visuals.widgets.active;
        act.corner_radius = egui::CornerRadius::ZERO;

        let open = &mut style.visuals.widgets.open;
        open.corner_radius = egui::CornerRadius::ZERO;

        // Shadows → NONE for Meridien (#16)
        style.visuals.popup_shadow  = egui::epaint::Shadow::NONE;
        style.visuals.window_shadow = egui::epaint::Shadow::NONE;
        style.visuals.window_stroke = egui::Stroke::new(settings.stroke_std, toolbar_border);
        style.visuals.window_corner_radius = egui::CornerRadius::ZERO;
        style.visuals.menu_corner_radius   = egui::CornerRadius::ZERO;

        // Denser editorial spacing
        style.spacing.button_padding   = egui::vec2(gap_xl(), gap_xs());
        style.spacing.menu_margin      = egui::Margin { left: gap_md() as i8, right: gap_md() as i8, top: gap_sm() as i8, bottom: gap_sm() as i8 };
        style.spacing.interact_size.y  = 22.0;
        style.spacing.item_spacing     = egui::vec2(gap_sm(), gap_xs());
    }

    // input_focus_color: override the focus-ring stroke on text inputs.
    if let Some(focus_col) = settings.input_focus_color {
        style.visuals.selection.stroke = egui::Stroke::new(settings.focus_ring_width, focus_col);
    }

    ctx.set_style(style);
    let _ = (toolbar_bg,); // may be used in future for popup fill overrides
}

// ─── #19 chrome_tile_btn ──────────────────────────────────────────────────────

/// State passed to [`paint_chrome_tile_button`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ChromeTileState { Idle, Hovered, Active }

/// Paint the small square chrome tile button used for "+Tab" and template/star
/// buttons in pane headers. Uses `current().r_md` (0 for Meridien, rounded
/// otherwise) and `current().stroke_thin`.
///
/// Returns nothing — the caller owns the `Response` and acts on clicks.
///
/// # Example
/// ```ignore
/// let resp = ui.allocate_rect(rect, egui::Sense::click());
/// let state = if resp.hovered() { ChromeTileState::Hovered } else { ChromeTileState::Idle };
/// paint_chrome_tile_button(&ui.painter().with_clip_rect(rect), rect, state, t);
/// ```
pub fn paint_chrome_tile_button(
    painter: &egui::Painter,
    rect: egui::Rect,
    state: ChromeTileState,
    t: &crate::chart_renderer::gpu::Theme,
) {
    let cr = egui::CornerRadius::same(current().r_md);
    let sw = current().stroke_thin;
    let (bg, border) = match state {
        ChromeTileState::Active  => (
            color_alpha(t.accent, 38),
            color_alpha(t.accent, alpha_active()),
        ),
        ChromeTileState::Hovered => (
            color_alpha(t.toolbar_border, alpha_subtle()),
            color_alpha(t.accent, alpha_line()),
        ),
        ChromeTileState::Idle    => (
            color_alpha(t.toolbar_border, 18),
            color_alpha(t.toolbar_border, alpha_muted()),
        ),
    };
    painter.rect_filled(rect, cr, bg);
    painter.rect_stroke(rect, cr, egui::Stroke::new(sw, border),
        egui::StrokeKind::Outside);
}

// ─── Border stroke shorthands ─────────────────────────────────────────────────

/// Standard 1px border stroke using `t.toolbar_border`. Covers 90% of separator / divider use.
#[inline]
pub fn border_stroke(t: &crate::chart_renderer::gpu::Theme) -> Stroke {
    Stroke::new(stroke_std(), t.toolbar_border)
}

/// Hair-width border stroke for dense / compact UI regions.
#[inline]
pub fn border_stroke_thin(t: &crate::chart_renderer::gpu::Theme) -> Stroke {
    Stroke::new(stroke_thin(), t.toolbar_border)
}

// ─── Icon button size tokens ──────────────────────────────────────────────────

/// 16×16 — small icon button (close, delete, inline action).
pub const BTN_ICON_SM: egui::Vec2 = egui::vec2(16.0, 16.0);
/// 24×24 — standard icon button (toolbar action, panel header icon).
pub const BTN_ICON_MD: egui::Vec2 = egui::vec2(24.0, 24.0);
/// 32×24 — wide icon button (split actions, nav arrows with extra hit area).
pub const BTN_ICON_LG: egui::Vec2 = egui::vec2(32.0, 24.0);

/// Foreground color for a [`ChromeTileState`] — pair with [`paint_chrome_tile_button`].
pub fn chrome_tile_fg(state: ChromeTileState, t: &crate::chart_renderer::gpu::Theme) -> egui::Color32 {
    match state {
        ChromeTileState::Active  => t.accent,
        ChromeTileState::Hovered => t.text,
        ChromeTileState::Idle    => t.dim.gamma_multiply(0.8),
    }
}
