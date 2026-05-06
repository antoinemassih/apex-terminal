//! Painter-positioned bespoke components — DOM ladder action buttons,
//! search/command pill, window control buttons, pane-header right-cluster
//! actions.
//!
//! These look custom because they use `allocate_rect + Painter::*` instead of
//! `Ui::add(Button)`, but they ARE design-system components — they read from
//! the same primitives (font_*, gap_*, stroke_*, radius_*, alpha_*, palette
//! bindings) so editing a token in the inspector recolors / resizes them in
//! lock-step with the canonical buttons.

use super::super::style::*;
use egui::{self, Color32, Response, Ui};

/// Search / command-launcher pill. Painter-positioned because it sits inside
/// the toolbar at a fixed-width pill, not inside an `egui::Ui` layout flow.
/// Visual primitives (border thickness, corner radius, font) match the
/// canonical text-input components — same family, different layout.
pub fn paint_search_command_pill(
    ui: &mut Ui,
    rect: egui::Rect,
    panel_rect: egui::Rect,
    icon: &str,
    label: &str,
    bg: Color32,
    bg_hover: Color32,
    border: egui::Stroke,
    icon_color: Color32,
    label_color: Color32,
) -> Response {
    let resp = ui.allocate_rect(rect, egui::Sense::click());
    let p = ui.painter_at(panel_rect);
    let r_cr = egui::CornerRadius::same(crate::dt_f32!(radius.xs, 2.0) as u8);
    let actual_bg = if resp.hovered() { bg_hover } else { bg };
    p.rect_filled(rect, r_cr, actual_bg);
    p.rect_stroke(rect, r_cr, border, egui::StrokeKind::Inside);
    let icon_x = rect.left() + gap_lg();
    p.text(
        egui::pos2(icon_x, rect.center().y),
        egui::Align2::LEFT_CENTER,
        icon,
        egui::FontId::proportional(font_md()),
        icon_color,
    );
    p.text(
        egui::pos2(icon_x + gap_2xl() + gap_xs(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::monospace(font_sm()),
        label_color,
    );
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Window control button (close / maximize / minimize) — painter-positioned
/// because hover paints the full toolbar-height column behind the icon, which
/// requires access to the panel's outer rect. Caller paints the icon glyph
/// (X / square / dash) on top of the returned rect.
///
/// `danger` = true → hover bg uses `danger_bg` (red for close); false → uses
/// `border_hover_bg` (subtle grey).
pub fn paint_window_control_button(
    ui: &mut Ui,
    button_rect: egui::Rect,
    panel_rect: egui::Rect,
    danger: bool,
    danger_bg: Color32,
    neutral_hover_bg: Color32,
) -> Response {
    let resp = ui.allocate_rect(button_rect, egui::Sense::click());
    if resp.hovered() {
        let bg = if danger { danger_bg } else { neutral_hover_bg };
        let full = egui::Rect::from_min_max(
            egui::pos2(button_rect.left(), panel_rect.top()),
            egui::pos2(button_rect.right(), panel_rect.bottom()),
        );
        let p = ui.ctx().layer_painter(ui.layer_id());
        p.rect_filled(full, egui::CornerRadius::ZERO, bg);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Visual tier for a DOM ladder action button. The DOM bottom row uses 6
/// distinct paint treatments — encode them here so the bottom row's call
/// sites become declarative.
#[derive(Debug, Clone, Copy)]
pub enum DomActionTier {
    /// Small `[-]` / `[+]` qty stepper. Subtle bg, dark text in light themes.
    QtyStepper,
    /// Static qty readout — non-interactive, looks like a text input.
    QtyReadout,
    /// `MARKET` / `LIMIT` toggle. Solid accent fill in light themes.
    SegmentChip,
    /// `[A]` armed-arm chip. Off → ghost grey, on → red-tinted.
    ArmedChip,
    /// Solid `BUY` action — bull color.
    Buy,
    /// Solid `SELL` action — bear color.
    Sell,
    /// Warning `FLATTEN` — orange.
    Warn,
    /// Subtle `CANCEL` — neutral grey.
    Subtle,
}

/// Inputs for `paint_dom_action` — bundle the theme/state once instead of
/// passing 8 parameters at every call site.
#[derive(Clone, Copy)]
pub struct DomActionContext<'a> {
    pub t: &'a super::super::super::gpu::Theme,
    pub is_light: bool,
    pub dark_ink: Color32,
    pub strong_text: Color32,
    pub armed: bool,
    pub mkt_active: bool,
}

/// Paint a single DOM ladder action button. Caller computes the rect (column-
/// aligned with the price ladder above) and supplies the click semantics; this
/// helper handles ALL visual primitives so every DOM button stays in sync with
/// the design system.
pub fn paint_dom_action(
    ui: &mut Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    label: &str,
    tier: DomActionTier,
    ctx: DomActionContext,
) -> Response {
    use DomActionTier::*;
    let resp = ui.allocate_rect(rect, egui::Sense::click());
    let hover = resp.hovered();
    let r_xs = egui::CornerRadius::same(crate::dt_f32!(radius.xs, 2.0) as u8);
    let r_sm = egui::CornerRadius::same(crate::dt_f32!(radius.sm, 3.0) as u8);
    let t = ctx.t;
    let border_stroke = rule_stroke_for(t.bg, t.toolbar_border);

    let font_label = egui::FontId::monospace(font_xs());
    let font_glyph = egui::FontId::monospace(font_sm());

    match tier {
        QtyStepper => {
            let fill = if ctx.is_light {
                if hover { color_alpha(ctx.dark_ink, 60) } else { color_alpha(ctx.dark_ink, 30) }
            } else if hover { color_alpha(t.toolbar_border, alpha_dim()) }
              else { color_alpha(t.toolbar_border, alpha_soft()) };
            painter.rect_filled(rect, r_xs, fill);
            painter.rect_stroke(rect, r_xs, border_stroke, egui::StrokeKind::Inside);
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font_glyph, ctx.strong_text);
        }
        QtyReadout => {
            let fill = if ctx.is_light { Color32::WHITE } else { color_alpha(t.bg, 180) };
            let text_col = if ctx.is_light { ctx.dark_ink } else { t.text };
            painter.rect_filled(rect, egui::CornerRadius::ZERO, fill);
            painter.rect_stroke(rect, egui::CornerRadius::ZERO, border_stroke, egui::StrokeKind::Inside);
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font_label.clone(), text_col);
        }
        SegmentChip => {
            let (fill, text_col) = if ctx.is_light {
                (t.accent, Color32::WHITE)
            } else {
                (color_alpha(t.accent, if hover { 55 } else { 28 }), t.accent)
            };
            painter.rect_filled(rect, r_xs, fill);
            painter.rect_stroke(rect, r_xs, border_stroke, egui::StrokeKind::Inside);
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font_label, text_col);
        }
        ArmedChip => {
            let ac = if ctx.armed { t.notification_red } else { t.dim.gamma_multiply(0.4) };
            let fill = if ctx.armed { color_alpha(ac, 35) } else { color_alpha(t.toolbar_border, alpha_ghost()) };
            painter.rect_filled(rect, r_xs, fill);
            let stroke_a = if ctx.armed { 90 } else { 30 };
            painter.rect_stroke(rect, r_xs,
                egui::Stroke::new(stroke_thin(), color_alpha(ac, stroke_a)),
                egui::StrokeKind::Outside);
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font_label, ac);
        }
        Buy | Sell => {
            let semantic = if matches!(tier, Buy) { t.bull } else { t.bear };
            let (fill, text_col) = if ctx.is_light {
                (if hover { semantic } else { semantic.gamma_multiply(0.92) }, Color32::WHITE)
            } else {
                (if hover { color_alpha(semantic, 70) } else { color_alpha(semantic, alpha_tint()) }, semantic)
            };
            painter.rect_filled(rect, r_sm, fill);
            painter.rect_stroke(rect, r_sm,
                egui::Stroke::new(stroke_thin(), color_alpha(semantic, if ctx.is_light { 200 } else { 90 })),
                egui::StrokeKind::Outside);
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font_glyph, text_col);
        }
        Warn => {
            let fc = t.warn;
            painter.rect_filled(rect, r_xs,
                if hover { color_alpha(fc, alpha_line()) } else { color_alpha(fc, 18) });
            painter.rect_stroke(rect, r_xs,
                egui::Stroke::new(stroke_thin(), color_alpha(fc, alpha_line())),
                egui::StrokeKind::Outside);
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font_label,
                if hover { fc } else { fc.gamma_multiply(0.6) });
        }
        Subtle => {
            painter.rect_filled(rect, r_xs,
                if hover { color_alpha(t.dim, alpha_muted()) } else { color_alpha(t.toolbar_border, alpha_soft()) });
            painter.rect_stroke(rect, r_xs,
                egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, alpha_line())),
                egui::StrokeKind::Outside);
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font_label,
                if hover { t.dim } else { t.dim.gamma_multiply(0.5) });
        }
    }
    if hover && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let _ = ctx.mkt_active;
    resp
}

/// Pane-header right-cluster action button (`+ Compare`, `Order`, `DOM`,
/// `Options`). Painter-positioned because the cluster manages its own
/// right-to-left layout cursor + full-height vertical dividers, but each
/// button's visual flows through this single helper so all four stay in sync.
pub fn paint_pane_header_action(
    ui: &mut Ui,
    header_painter: &egui::Painter,
    rect: egui::Rect,
    label: &str,
    active: bool,
    text_color: Color32,
    dim_color: Color32,
) -> Response {
    let resp = ui.allocate_rect(rect, egui::Sense::click());
    let fg = if active {
        text_color
    } else if resp.hovered() {
        text_color
    } else {
        dim_color.gamma_multiply(0.85)
    };
    header_painter.text(
        egui::pos2(rect.left(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::monospace(font_md()),
        fg,
    );
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

