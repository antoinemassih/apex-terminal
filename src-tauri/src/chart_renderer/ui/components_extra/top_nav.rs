//! Top-nav buttons + toggles, menu trigger + menu items, pane tabs,
//! timeframe selector.

use super::super::style::*;
use egui::{self, Color32, Response, RichText, Stroke, Ui};

// ─── TopNavButton ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopNavTreatment {
    Raised,
    Underline,
    SoftPill,
}

pub fn top_nav_btn(
    ui: &mut Ui,
    label: &str,
    active: bool,
    treatment: TopNavTreatment,
    accent: Color32,
    dim: Color32,
) -> Response {
    let fg = if active { accent } else { dim };
    let (bg, border) = match treatment {
        TopNavTreatment::Raised => {
            let b = if active { color_alpha(accent, alpha_tint()) } else { Color32::TRANSPARENT };
            let s = if active { color_alpha(accent, alpha_line()) } else { Color32::TRANSPARENT };
            (b, s)
        }
        TopNavTreatment::Underline => (Color32::TRANSPARENT, Color32::TRANSPARENT),
        TopNavTreatment::SoftPill => {
            let b = if active { color_alpha(accent, alpha_soft()) } else { Color32::TRANSPARENT };
            (b, Color32::TRANSPARENT)
        }
    };
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_lg(), gap_md());
    let resp = ui.add(
        egui::Button::new(RichText::new(label).size(font_md()).strong().color(fg))
            .fill(bg)
            .stroke(Stroke::new(stroke_thin(), border))
            .corner_radius(radius_sm())
            .min_size(egui::vec2(0.0, gap_3xl())),
    );
    ui.spacing_mut().button_padding = prev_pad;
    if active && treatment == TopNavTreatment::Underline {
        let r = resp.rect;
        ui.painter().line_segment(
            [egui::pos2(r.left() + gap_sm(), r.bottom()), egui::pos2(r.right() - gap_sm(), r.bottom())],
            Stroke::new(stroke_std(), accent),
        );
    }
    if resp.hovered() && !active && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(accent, alpha_faint()));
    }
    resp
}

// ─── TopNavToggle ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopNavToggleSize {
    Small,
    Medium,
}

pub fn top_nav_toggle(
    ui: &mut Ui,
    icon: &str,
    active: bool,
    size: TopNavToggleSize,
    accent: Color32,
    dim: Color32,
) -> Response {
    let side = match size { TopNavToggleSize::Small => 22.0_f32, TopNavToggleSize::Medium => 28.0_f32 };
    let font = match size { TopNavToggleSize::Small => font_md(), TopNavToggleSize::Medium => font_lg() };
    let fg = if active { accent } else { dim };
    let bg = if active { color_alpha(accent, alpha_tint()) } else { Color32::TRANSPARENT };
    let border = if active { color_alpha(accent, alpha_muted()) } else { color_alpha(dim, alpha_subtle()) };
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);
    let resp = ui.add(
        egui::Button::new(RichText::new(icon).size(font).color(fg))
            .fill(bg)
            .stroke(Stroke::new(stroke_thin(), border))
            .corner_radius(radius_sm())
            .min_size(egui::vec2(side, side)),
    );
    ui.spacing_mut().button_padding = prev_pad;
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        if !active {
            ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(accent, alpha_ghost()));
        }
    }
    resp
}

// ─── MenuTrigger ─────────────────────────────────────────────────────────────

pub fn menu_trigger(ui: &mut Ui, label: &str, open: bool, accent: Color32, dim: Color32) -> Response {
    let fg = if open { accent } else { dim };
    let bg = if open { color_alpha(accent, alpha_soft()) } else { Color32::TRANSPARENT };
    let border = if open { color_alpha(accent, alpha_muted()) } else { Color32::TRANSPARENT };
    let display = format!("{} \u{25BE}", label);
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_md(), gap_sm());
    let resp = ui.add(
        egui::Button::new(RichText::new(display).size(font_sm()).color(fg))
            .fill(bg)
            .stroke(Stroke::new(stroke_thin(), border))
            .corner_radius(radius_sm())
            .min_size(egui::vec2(0.0, 20.0)),
    );
    ui.spacing_mut().button_padding = prev_pad;
    if resp.hovered() && !open && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(accent, alpha_ghost()));
    }
    resp
}

// ─── MenuItem ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum MenuItemVariant {
    Default,
    Submenu,
    Checkbox(bool),
    Separator,
}

pub fn menu_item(
    ui: &mut Ui,
    label: &str,
    variant: MenuItemVariant,
    shortcut: Option<&str>,
    accent: Color32,
    dim: Color32,
) -> Response {
    if variant == MenuItemVariant::Separator {
        let (sep_rect, resp) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 1.0),
            egui::Sense::hover(),
        );
        ui.painter().line_segment(
            [
                egui::pos2(sep_rect.left() + gap_sm(), sep_rect.center().y),
                egui::pos2(sep_rect.right() - gap_sm(), sep_rect.center().y),
            ],
            Stroke::new(stroke_hair(), color_alpha(dim, alpha_line())),
        );
        ui.add_space(gap_xs());
        return resp;
    }
    let prefix = match &variant {
        MenuItemVariant::Checkbox(true)  => "\u{2713} ",
        MenuItemVariant::Checkbox(false) => "  ",
        _ => "",
    };
    let suffix = match &variant {
        MenuItemVariant::Submenu => " \u{25B8}",
        _ => "",
    };
    let display = format!("{}{}{}", prefix, label, suffix);
    let fg = dim;
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_lg(), gap_xs());
    let resp = ui.horizontal(|ui| {
        let r = ui.add(
            egui::Button::new(RichText::new(&display).size(font_sm()).color(fg))
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
                .min_size(egui::vec2(ui.available_width().max(80.0), 20.0)),
        );
        if let Some(sc) = shortcut {
            let sc_color = color_alpha(dim, alpha_muted());
            let max_x = r.rect.right() - gap_sm();
            let y = r.rect.center().y;
            ui.painter().text(
                egui::pos2(max_x, y),
                egui::Align2::RIGHT_CENTER,
                sc,
                egui::FontId::monospace(font_xs()),
                sc_color,
            );
        }
        r
    }).inner;
    ui.spacing_mut().button_padding = prev_pad;
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(accent, alpha_ghost()));
    }
    resp
}

// ─── PaneTabButton ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneTabStyle {
    Underline,
    Filled,
    Border,
}

pub fn pane_tab_btn(
    ui: &mut Ui,
    icon: Option<&str>,
    label: &str,
    active: bool,
    style: PaneTabStyle,
    accent: Color32,
    dim: Color32,
) -> Response {
    let text = match icon {
        Some(ic) => format!("{} {}", ic, label),
        None => label.to_owned(),
    };
    let fg = if active { accent } else { dim };
    let (bg, border) = match (active, style) {
        (true, PaneTabStyle::Filled) => (color_alpha(accent, alpha_tint()), color_alpha(accent, alpha_active())),
        (true, PaneTabStyle::Border) => (Color32::TRANSPARENT, color_alpha(accent, alpha_active())),
        _ => (Color32::TRANSPARENT, Color32::TRANSPARENT),
    };
    let cr = egui::CornerRadius::same(radius_sm() as u8);
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_md(), gap_sm());
    let resp = ui.add(
        egui::Button::new(RichText::new(&text).monospace().size(font_sm()).color(fg))
            .fill(bg)
            .stroke(Stroke::new(stroke_thin(), border))
            .corner_radius(cr)
            .min_size(egui::vec2(0.0, 22.0)),
    );
    ui.spacing_mut().button_padding = prev_pad;
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if active && style == PaneTabStyle::Underline {
        let r = resp.rect;
        ui.painter().line_segment(
            [egui::pos2(r.left() + 3.0, r.bottom() + 1.0), egui::pos2(r.right() - 3.0, r.bottom() + 1.0)],
            Stroke::new(stroke_thick(), color_alpha(accent, alpha_strong())),
        );
    }
    resp
}

// ─── TimeframeSelector ───────────────────────────────────────────────────────

pub fn timeframe_selector(
    ui: &mut Ui,
    options: &[&str],
    active_idx: usize,
    accent: Color32,
    dim: Color32,
) -> Option<usize> {
    let mut clicked = None;
    let pill_r = egui::CornerRadius::same(99);
    let prev_item_spacing = ui.spacing().item_spacing.x;
    ui.spacing_mut().item_spacing.x = gap_xs();
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_md(), gap_xs());
    for (i, &label) in options.iter().enumerate() {
        let active = i == active_idx;
        let fg = if active { accent } else { dim };
        let (bg, border) = if active {
            (color_alpha(accent, alpha_tint()), color_alpha(accent, alpha_dim()))
        } else {
            (Color32::TRANSPARENT, Color32::TRANSPARENT)
        };
        let resp = ui.add(
            egui::Button::new(RichText::new(label).monospace().size(font_sm()).strong().color(fg))
                .fill(bg)
                .stroke(Stroke::new(stroke_thin(), border))
                .corner_radius(pill_r)
                .min_size(egui::vec2(0.0, 20.0)),
        );
        if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() && i != active_idx {
            clicked = Some(i);
        }
    }
    ui.spacing_mut().button_padding = prev_pad;
    ui.spacing_mut().item_spacing.x = prev_item_spacing;
    clicked
}
