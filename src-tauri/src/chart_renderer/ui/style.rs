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

// ─── Font size tokens ─────────────────────────────────────────────────────────
pub const FONT_XS:  f32 = 8.0;   // column headers, status chips
pub const FONT_SM:  f32 = 10.0;  // body text, labels, most buttons
pub const FONT_MD:  f32 = 11.0;  // panel section headers
pub const FONT_LG:  f32 = 13.0;  // primary headings, toolbar buttons
pub const FONT_XL:  f32 = 14.0;  // large price values in cards
pub const FONT_2XL: f32 = 15.0;  // featured prices (chain, big display)

// ─── Spacing tokens ───────────────────────────────────────────────────────────
pub const GAP_XS:  f32 = 2.0;
pub const GAP_SM:  f32 = 4.0;
pub const GAP_MD:  f32 = 6.0;
pub const GAP_LG:  f32 = 8.0;
pub const GAP_XL:  f32 = 10.0;
pub const GAP_2XL: f32 = 12.0;
pub const GAP_3XL: f32 = 20.0;

// ─── Corner radius tokens ─────────────────────────────────────────────────────
pub const RADIUS_SM: f32 = 3.0;   // small buttons, badges, chips
pub const RADIUS_MD: f32 = 4.0;   // primary buttons, cards
pub const RADIUS_LG: f32 = 8.0;   // dialogs, panels, modals

// ─── Stroke width tokens ─────────────────────────────────────────────────────
pub const STROKE_HAIR:   f32 = 0.3;   // ultra-fine grid/DOM separators
pub const STROKE_THIN:   f32 = 0.5;   // separators, card borders, badges
pub const STROKE_STD:    f32 = 1.0;   // panel frames, dialog windows
pub const STROKE_BOLD:   f32 = 1.5;   // emphasis outlines
pub const STROKE_THICK:  f32 = 2.0;   // tab underlines, accent stripes

// ─── Semantic alpha tokens ────────────────────────────────────────────────────
/// Use with `color_alpha(color, ALPHA_*)` for consistent opacity tiers.
pub const ALPHA_FAINT:  u8 = 10;   // barely-visible tints
pub const ALPHA_GHOST:  u8 = 15;   // hover row bg, chart bar fills
pub const ALPHA_SOFT:   u8 = 20;   // soft hover states, selector backgrounds
pub const ALPHA_SUBTLE: u8 = 25;   // section backgrounds, dim overlays
pub const ALPHA_TINT:   u8 = 30;   // action button fill (enabled)
pub const ALPHA_MUTED:  u8 = 40;   // section separators, secondary borders
pub const ALPHA_LINE:   u8 = 50;   // card borders, row dividers
pub const ALPHA_DIM:    u8 = 60;   // mid-tone chart elements
pub const ALPHA_STRONG: u8 = 80;   // panel frame borders
pub const ALPHA_ACTIVE: u8 = 100;  // active/enabled button borders
pub const ALPHA_HEAVY:  u8 = 120;  // dialog borders, high-contrast lines

// ─── Drop shadow tokens ───────────────────────────────────────────────────────
pub const SHADOW_OFFSET: f32 = 2.0;   // drop shadow x/y offset
pub const SHADOW_ALPHA:  u8  = 60;    // drop shadow darkness (pure black)
pub const SHADOW_SPREAD: f32 = 4.0;   // corner radius for shadow rect

// ─── Fixed color constants ────────────────────────────────────────────────────
/// Primary text in cards, labels, list rows.
pub const TEXT_PRIMARY:   Color32 = Color32::from_rgb(220, 220, 230);
/// Secondary text in dimmed/triggered card labels.
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(200, 200, 210);

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
        .inner_margin(egui::Margin { left: GAP_XL as i8, right: GAP_XL as i8, top: GAP_XL as i8, bottom: GAP_LG as i8 })
        .stroke(Stroke::new(STROKE_STD, color_alpha(toolbar_border, ALPHA_HEAVY)))
}

/// Compact panel frame — tighter margins for narrow info-dense panels (scanner, tape).
pub fn panel_frame_compact(toolbar_bg: Color32, toolbar_border: Color32) -> egui::Frame {
    egui::Frame::NONE
        .fill(toolbar_bg)
        .inner_margin(egui::Margin { left: GAP_LG as i8, right: GAP_LG as i8, top: GAP_LG as i8, bottom: GAP_MD as i8 })
        .stroke(Stroke::new(STROKE_STD, color_alpha(toolbar_border, ALPHA_HEAVY)))
}

// ─── Toolbar button ───────────────────────────────────────────────────────────

/// Toolbar button — FONT_LG, RADIUS_MD, themed, pointer cursor.
/// Active state: accent fill + accent border + soft glow halo + bottom underline.
/// Hover state: subtle bg tint + accent border.
pub fn tb_btn(ui: &mut egui::Ui, label: &str, active: bool, accent: Color32, dim: Color32, toolbar_bg: Color32, toolbar_border: Color32) -> egui::Response {
    let bg = if active {
        color_alpha(accent, 32)
    } else {
        color_alpha(toolbar_border, 18)
    };
    let fg = if active { accent } else { dim };
    let border = if active {
        color_alpha(accent, ALPHA_ACTIVE)
    } else {
        color_alpha(toolbar_border, ALPHA_MUTED)
    };

    let resp = ui.add(egui::Button::new(RichText::new(label).monospace().size(FONT_LG).color(fg))
        .fill(bg).stroke(Stroke::new(STROKE_THIN, border)).corner_radius(RADIUS_MD)
        .min_size(egui::vec2(0.0, 24.0)));

    if active {
        // Bottom underline only — cleaner, no glow halo
        let r = resp.rect;
        ui.painter().line_segment(
            [egui::pos2(r.left() + RADIUS_MD, r.bottom() + 0.5),
             egui::pos2(r.right() - RADIUS_MD, r.bottom() + 0.5)],
            Stroke::new(STROKE_STD, color_alpha(accent, ALPHA_DIM)));
    } else if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        // Hover: crisp bg tint + accent border repainted
        ui.painter().rect_filled(resp.rect, RADIUS_MD,
            color_alpha(toolbar_border, ALPHA_SUBTLE));
        ui.painter().rect_stroke(resp.rect, RADIUS_MD,
            Stroke::new(STROKE_THIN, color_alpha(accent, ALPHA_LINE)), egui::StrokeKind::Inside);
        ui.painter().text(resp.rect.center(), egui::Align2::CENTER_CENTER,
            label, egui::FontId::monospace(FONT_LG), TEXT_PRIMARY);
    }
    resp
}

// ─── Dialog / popup windows ───────────────────────────────────────────────────

/// Standard popup window frame — dark background, no title bar.
pub fn popup_frame(ctx: &egui::Context, id: &str, pos: egui::Pos2, width: f32, fill: Color32, border_color: Option<Color32>) -> egui::Window<'static> {
    let mut frame = egui::Frame::popup(&ctx.style()).fill(fill).inner_margin(GAP_LG);
    if let Some(bc) = border_color {
        frame = frame.stroke(Stroke::new(STROKE_STD, bc));
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
            .stroke(Stroke::new(STROKE_STD, border)).corner_radius(RADIUS_LG))
}

/// Theme-aware dialog window.
pub fn dialog_window_themed(ctx: &egui::Context, id: &str, pos: egui::Pos2, width: f32, toolbar_bg: Color32, toolbar_border: Color32, border_color: Option<Color32>) -> egui::Window<'static> {
    let border = border_color.unwrap_or(color_alpha(toolbar_border, ALPHA_ACTIVE));
    egui::Window::new(id.to_string())
        .fixed_pos(pos).fixed_size(egui::vec2(width, 0.0))
        .title_bar(false)
        .frame(egui::Frame::popup(&ctx.style()).fill(toolbar_bg).inner_margin(0.0)
            .stroke(Stroke::new(STROKE_STD, border)).corner_radius(RADIUS_LG))
}

/// Dialog header bar — auto-darkened bg, FONT_LG title, X close. Returns true if closed.
pub fn dialog_header(ui: &mut egui::Ui, title: &str, dim: Color32) -> bool {
    dialog_header_colored(ui, title, dim, None)
}

/// Dialog header bar with explicit header background.
pub fn dialog_header_colored(ui: &mut egui::Ui, title: &str, dim: Color32, header_bg: Option<Color32>) -> bool {
    use super::super::super::ui_kit::icons::Icon;
    let fill = header_bg.unwrap_or_else(|| {
        let bg = ui.visuals().window_fill();
        Color32::from_rgb(bg.r().saturating_sub(8), bg.g().saturating_sub(8), bg.b().saturating_sub(8))
    });
    let mut closed = false;
    egui::Frame::NONE.fill(fill)
        .inner_margin(egui::Margin { left: GAP_XL as i8, right: GAP_LG as i8, top: GAP_LG as i8, bottom: GAP_LG as i8 })
        .corner_radius(egui::CornerRadius { nw: RADIUS_LG as u8, ne: RADIUS_LG as u8, sw: 0, se: 0 })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).monospace().size(FONT_LG).strong().color(TEXT_PRIMARY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Clean X close button — uses icon_btn which has pointer cursor + hover highlight
                    if icon_btn(ui, Icon::X, dim.gamma_multiply(0.7), FONT_XL).clicked() {
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
        Stroke::new(STROKE_THIN, color));
    ui.add_space(1.0);
}

/// Inset separator with margins on both sides.
pub fn dialog_separator(ui: &mut egui::Ui, margin: f32, color: Color32) {
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [egui::pos2(rect.left() + margin, ui.cursor().min.y),
         egui::pos2(rect.right() - margin, ui.cursor().min.y)],
        Stroke::new(STROKE_THIN, color));
    ui.add_space(1.0);
}

/// Inset separator + soft gradient shadow below (3 fading lines).
pub fn dialog_separator_shadow(ui: &mut egui::Ui, margin: f32, color: Color32) {
    let rect = ui.available_rect_before_wrap();
    let y = ui.cursor().min.y;
    let left = rect.left() + margin;
    let right = rect.right() - margin;
    ui.painter().line_segment([egui::pos2(left, y), egui::pos2(right, y)], Stroke::new(STROKE_THIN, color));
    // Fading shadow gradient: 3 strokes at decreasing black alpha
    let shadow_alphas = [20u8, 12, 4];
    for (i, &a) in shadow_alphas.iter().enumerate() {
        ui.painter().line_segment(
            [egui::pos2(left, y + (i + 1) as f32), egui::pos2(right, y + (i + 1) as f32)],
            Stroke::new(STROKE_THIN, Color32::from_rgba_unmultiplied(0, 0, 0, a)));
    }
    ui.add_space(GAP_SM);
}

/// Indented section label with left margin — used inside dialogs.
pub fn dialog_section(ui: &mut egui::Ui, text: &str, margin: f32, color: Color32) {
    ui.horizontal(|ui| {
        ui.add_space(margin);
        ui.label(RichText::new(text).monospace().size(FONT_SM).strong().color(color));
    });
    ui.add_space(GAP_XS + 1.0);
}

// ─── Labels ───────────────────────────────────────────────────────────────────

/// Section header — FONT_SM bold.
#[inline]
pub fn section_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).monospace().size(FONT_SM).strong().color(color));
}

/// Dim info label — FONT_SM regular.
#[inline]
pub fn dim_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).monospace().size(FONT_SM).color(color));
}

/// Column header cell — FONT_XS dim monospace, fixed width.
/// `right_align = true` for numeric columns (PRICE, SIZE), false for text (SYMBOL, TIME).
pub fn col_header(ui: &mut egui::Ui, text: &str, width: f32, color: Color32, right_align: bool) {
    let layout = if right_align {
        egui::Layout::right_to_left(egui::Align::Center)
    } else {
        egui::Layout::left_to_right(egui::Align::Center)
    };
    ui.allocate_ui_with_layout(egui::vec2(width, 12.0), layout, |ui| {
        ui.label(RichText::new(text).monospace().size(FONT_XS).color(color));
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

    let trough = Color32::from_rgb(
        toolbar_bg.r().saturating_sub(12),
        toolbar_bg.g().saturating_sub(12),
        toolbar_bg.b().saturating_sub(12),
    );
    let border_col = color_alpha(toolbar_border, ALPHA_STRONG);

    // Reserve a painter slot — trough bg will be painted BEHIND buttons
    let bg_slot = ui.painter().add(egui::Shape::Noop);

    let prev_spacing = ui.spacing().item_spacing.x;
    ui.spacing_mut().item_spacing.x = 2.0;

    let mut union_rect: Option<egui::Rect> = None;
    let n = labels.len();

    for (i, label) in labels.iter().enumerate() {
        let active = i == active_idx;
        let fg = if active { accent } else { dim };
        let bg = if active { color_alpha(accent, ALPHA_TINT + 5) } else { Color32::TRANSPARENT };
        let cr = match (i, n) {
            (0, 1) => egui::CornerRadius::same(RADIUS_SM as u8),
            (0, _) => egui::CornerRadius { nw: RADIUS_SM as u8, sw: RADIUS_SM as u8, ne: 0, se: 0 },
            (x, n) if x == n - 1 => egui::CornerRadius { nw: 0, sw: 0, ne: RADIUS_SM as u8, se: RADIUS_SM as u8 },
            _ => egui::CornerRadius::ZERO,
        };
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(7.0, prev_pad.y);
        let resp = ui.add(
            egui::Button::new(RichText::new(*label).monospace().size(FONT_LG).strong().color(fg))
                .fill(bg).stroke(Stroke::NONE).corner_radius(cr)
                .min_size(egui::vec2(0.0, 24.0))
        );
        ui.spacing_mut().button_padding = prev_pad;
        union_rect = Some(union_rect.map_or(resp.rect, |r: egui::Rect| r.union(resp.rect)));
        if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
        if resp.clicked() { clicked = Some(i); }
    }

    ui.spacing_mut().item_spacing.x = prev_spacing;

    // Fill trough background behind buttons — no vertical expand so it stays the exact
    // button height (centered by the parent horizontal_centered layout)
    if let Some(ur) = union_rect {
        let trough_rect = ur.expand2(egui::vec2(4.0, 0.0));
        let r = RADIUS_MD as f32 + 1.0;
        ui.painter().set(bg_slot, egui::Shape::rect_filled(trough_rect, r, trough));
        ui.painter().rect_stroke(trough_rect, r, Stroke::new(STROKE_THIN, border_col), egui::StrokeKind::Outside);
    }

    clicked
}

// ─── Panel chrome ─────────────────────────────────────────────────────────────

/// Square icon button with hover highlight — always renders as a true square hit target.
/// Internally zeroes button_padding so egui doesn't add asymmetric whitespace around the icon.
/// Returns the full Response so callers can chain `.clicked()`, `.on_hover_text()`, etc.
pub fn icon_btn(ui: &mut egui::Ui, icon: &str, color: Color32, size: f32) -> egui::Response {
    let side = (size + 10.0).max(26.0); // 5px padding on each side, min 26px square
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0); // suppress egui's own padding — we control the square
    let resp = ui.add(
        egui::Button::new(RichText::new(icon).size(size).color(color))
            .frame(false)
            .min_size(egui::vec2(side, side))
    );
    ui.spacing_mut().button_padding = prev_pad;
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(resp.rect, RADIUS_SM, color_alpha(color, ALPHA_GHOST));
        ui.painter().rect_stroke(resp.rect, RADIUS_SM,
            egui::Stroke::new(STROKE_THIN, color_alpha(color, ALPHA_MUTED)), egui::StrokeKind::Inside);
    }
    resp
}

/// Close button (X icon) — square icon_btn, standard panel close.
#[inline]
pub fn close_button(ui: &mut egui::Ui, dim: Color32) -> bool {
    icon_btn(ui, super::super::super::ui_kit::icons::Icon::X, dim, FONT_LG).clicked()
}

/// Panel header — FONT_LG title + close button. Returns true if closed.
pub fn panel_header(ui: &mut egui::Ui, title: &str, accent: Color32, dim: Color32) -> bool {
    panel_header_sub(ui, title, None, accent, dim)
}

/// Panel header with optional subtitle text. Returns true if closed.
pub fn panel_header_sub(ui: &mut egui::Ui, title: &str, subtitle: Option<&str>, accent: Color32, dim: Color32) -> bool {
    let mut closed = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).monospace().size(FONT_LG).strong().color(accent));
        if let Some(sub) = subtitle {
            ui.label(RichText::new(sub).monospace().size(FONT_SM).color(dim));
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
    for (tab, label) in tabs {
        let active = *current == *tab;
        let color = if active { accent } else { dim };
        let resp = ui.add(
            egui::Button::new(RichText::new(*label).monospace().size(FONT_LG).strong().color(color))
                .frame(false)
        );
        if resp.clicked() { *current = *tab; }
        if active {
            let r = resp.rect;
            ui.painter().rect_filled(
                egui::Rect::from_min_max(egui::pos2(r.left(), r.max.y - 2.0), egui::pos2(r.right(), r.max.y)),
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
        .stroke(Stroke::new(STROKE_THIN, color_alpha(toolbar_border, ALPHA_STRONG)))
        .inner_margin(GAP_LG)
        .corner_radius(RADIUS_LG)
}

/// Single stat row inside a tooltip — label left, value right.
/// Keeps all stat tables consistent: label at FONT_XS dim, value at FONT_SM colored.
pub fn stat_row(ui: &mut egui::Ui, label: &str, value: &str, label_color: Color32, value_color: Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).monospace().size(FONT_XS).color(label_color));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).monospace().size(FONT_SM).strong().color(value_color));
        });
    });
}

/// Paint a drop shadow behind a painter-based tooltip rect (call BEFORE painting the bg).
/// Used in OHLC tooltip, drawing significance tooltip, etc.
pub fn paint_tooltip_shadow(painter: &egui::Painter, rect: egui::Rect, radius: f32) {
    let shadow_rect = rect.translate(egui::vec2(SHADOW_OFFSET, SHADOW_OFFSET));
    painter.rect_filled(shadow_rect, radius, Color32::from_rgba_unmultiplied(0, 0, 0, SHADOW_ALPHA));
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
        ui.allocate_ui(egui::vec2(label_width, 18.0), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(GAP_SM);
                ui.label(RichText::new(label).monospace().size(FONT_SM).color(dim));
            });
        });
        add_content(ui);
    });
}

// ─── Cards / badges ───────────────────────────────────────────────────────────

/// Status badge — small tinted pill (e.g. "DRAFT", "PLACED", "TRIGGERED").
pub fn status_badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.add(egui::Button::new(RichText::new(text).monospace().size(8.0).strong().color(color))
        .fill(color_alpha(color, ALPHA_SUBTLE))
        .stroke(Stroke::new(STROKE_THIN, color_alpha(color, ALPHA_DIM)))
        .corner_radius(RADIUS_SM)
        .min_size(egui::vec2(0.0, 16.0)));
}

/// Order card — left accent stripe + subtle bg. Returns true if the card area was clicked.
pub fn order_card(ui: &mut egui::Ui, accent: Color32, bg: Color32, add_content: impl FnOnce(&mut egui::Ui)) -> bool {
    let available_w = ui.available_width();
    let resp = egui::Frame::NONE
        .fill(bg)
        .inner_margin(egui::Margin { left: 9, right: 6, top: 5, bottom: 5 })
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.set_min_width(available_w - 15.0);
            let outer = ui.min_rect();
            let stripe = egui::Rect::from_min_max(
                egui::pos2(outer.left() - 9.0, outer.top() - 5.0),
                egui::pos2(outer.left() - 6.0, outer.bottom() + 5.0));
            ui.painter().rect_filled(stripe, egui::CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 }, accent);
            add_content(ui);
        });
    let card_rect = resp.response.rect;
    let click_resp = ui.interact(card_rect, ui.id().with(("card_click", card_rect.min.x as i32, card_rect.min.y as i32)), egui::Sense::click());
    if click_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    ui.add_space(GAP_SM);
    click_resp.clicked()
}

// ─── Buttons ──────────────────────────────────────────────────────────────────

/// Action button — tinted bg, for Place/Cancel/Clear. Disabled = greyed out.
pub fn action_btn(ui: &mut egui::Ui, label: &str, color: Color32, enabled: bool) -> bool {
    let bg     = if enabled { color_alpha(color, ALPHA_MUTED)  } else { color_alpha(color, ALPHA_FAINT)  };
    let fg     = if enabled { color                            } else { color_alpha(color, ALPHA_ACTIVE) };
    let border = if enabled { color_alpha(color, ALPHA_ACTIVE) } else { color_alpha(color, ALPHA_LINE)   };
    let resp = ui.add_enabled(enabled,
        egui::Button::new(RichText::new(label).monospace().size(FONT_SM).strong().color(fg))
            .fill(bg).stroke(Stroke::new(STROKE_THIN, border))
            .corner_radius(RADIUS_MD).min_size(egui::vec2(0.0, 24.0)));
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    resp.clicked()
}

/// Trade button — deep saturated bg for BUY/SELL. White bold text.
pub fn trade_btn(ui: &mut egui::Ui, label: &str, color: Color32, width: f32) -> bool {
    let bg = Color32::from_rgb(
        (color.r() as f32 * 0.55) as u8,
        (color.g() as f32 * 0.55) as u8,
        (color.b() as f32 * 0.55) as u8);
    let resp = ui.add(egui::Button::new(RichText::new(label).monospace().size(FONT_LG).strong().color(Color32::WHITE))
        .fill(bg).min_size(egui::vec2(width, 30.0)).corner_radius(RADIUS_MD));
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        // Brighten on hover
        let hover_bg = Color32::from_rgb(
            (color.r() as f32 * 0.7).min(255.0) as u8,
            (color.g() as f32 * 0.7).min(255.0) as u8,
            (color.b() as f32 * 0.7).min(255.0) as u8);
        ui.painter().rect_filled(resp.rect, RADIUS_MD, hover_bg);
        ui.painter().text(resp.rect.center(), egui::Align2::CENTER_CENTER,
            label, egui::FontId::monospace(FONT_LG), Color32::WHITE);
    }
    resp.clicked()
}

/// Small action button — for inline header actions like "Clear All", "Close All".
pub fn small_action_btn(ui: &mut egui::Ui, label: &str, color: Color32) -> bool {
    let resp = ui.add(egui::Button::new(RichText::new(label).monospace().size(FONT_SM).strong().color(color))
        .fill(color_alpha(color, ALPHA_SOFT))
        .corner_radius(RADIUS_SM)
        .stroke(Stroke::new(STROKE_THIN, color_alpha(color, ALPHA_DIM)))
        .min_size(egui::vec2(0.0, 18.0)));
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    resp.clicked()
}

/// Simple button — subtle border, for form actions (Create, Cancel).
pub fn simple_btn(ui: &mut egui::Ui, label: &str, color: Color32, min_width: f32) -> bool {
    let resp = ui.add(egui::Button::new(RichText::new(label).monospace().size(FONT_SM).color(color))
        .fill(color_alpha(color, ALPHA_FAINT))
        .stroke(Stroke::new(STROKE_THIN, color_alpha(color, ALPHA_MUTED)))
        .corner_radius(RADIUS_SM)
        .min_size(egui::vec2(min_width, 20.0)));
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
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
