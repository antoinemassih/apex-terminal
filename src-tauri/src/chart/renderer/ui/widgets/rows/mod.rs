//! Row primitives for list/table views (watchlist, option chain, DOM, orders,
//! news, alerts, trade history). Each row is a builder + `impl Widget`.
//!
//! `ListRow` is the generic selectable/hoverable row vehicle used by
//! `tape_panel`, `object_tree`, and `discord_panel`. Domain-specific rows
//! (`WatchlistRow`, `OrderRow`, `NewsRow`, etc.) use `RowShell` (painter mode)
//! for pixel-exact layouts. Both paths share design tokens from `style`.
//!
//! Common shape: every row exposes `.selected(bool)`, `.hover_enabled(bool)`,
//! `.divider(bool)`, optional `.left_icon(...)`, optional `.trailing_actions(...)`,
//! and a `.theme(&Theme)` knob. The `ListRow` base in this file is the
//! generic vehicle — domain rows wrap it with column slots.

#![allow(unused_imports)]

use egui::{Color32, Response, Sense, Stroke, Ui, Widget};
use super::super::style::*;

type Theme = crate::chart_renderer::gpu::Theme;
fn ft() -> &'static Theme { &crate::chart_renderer::gpu::THEMES[0] }



// ─── ListRow — generic selectable/hoverable row primitive ────────────────────

/// Generic list row: a single horizontal strip with optional left icon, a body
/// closure, optional trailing-action closure, hover/selection backgrounds, and
/// an optional bottom divider line.
///
/// ```ignore
/// let resp = ListRow::new(28.0)
///     .selected(is_active)
///     .divider(true)
///     .left_icon("●", t.accent)
///     .body(|ui| ui.label("AAPL"))
///     .trailing_actions(|ui| { ui.add(IconBtn::new("×")); })
///     .theme(t)
///     .show(ui);
/// ```
#[must_use = "ListRow must be finalized with `.show(ui)` to render"]
pub struct ListRow<'a, B: FnOnce(&mut Ui) + 'a, T: FnOnce(&mut Ui) + 'a> {
    height: f32,
    selected: bool,
    hover_enabled: bool,
    divider: bool,
    left_icon: Option<(&'a str, Color32)>,
    left_dot: Option<Color32>,
    indent: f32,
    body: Option<B>,
    trailing: Option<T>,
    trailing_width: f32,
    theme_bg: Option<Color32>,
    theme_border: Option<Color32>,
    theme_accent: Option<Color32>,
    sense: Sense,
    row_tint: Option<(Color32, u8)>,
}

impl<'a> ListRow<'a, fn(&mut Ui), fn(&mut Ui)> {
    pub fn new(height: f32) -> ListRow<'a, fn(&mut Ui), fn(&mut Ui)> {
        ListRow {
            height,
            selected: false,
            hover_enabled: true,
            divider: false,
            left_icon: None,
            left_dot: None,
            indent: 0.0,
            body: None,
            trailing: None,
            trailing_width: 80.0,
            theme_bg: None,
            theme_border: None,
            theme_accent: None,
            sense: Sense::click(),
            row_tint: None,
        }
    }
}

impl<'a, B: FnOnce(&mut Ui) + 'a, T: FnOnce(&mut Ui) + 'a> ListRow<'a, B, T> {
    pub fn selected(mut self, v: bool) -> Self { self.selected = v; self }
    pub fn hover_enabled(mut self, v: bool) -> Self { self.hover_enabled = v; self }
    pub fn divider(mut self, v: bool) -> Self { self.divider = v; self }
    pub fn left_icon(mut self, glyph: &'a str, color: Color32) -> Self {
        self.left_icon = Some((glyph, color)); self
    }
    /// Paint a small filled circle (color dot) before the body. Stacks with
    /// `left_icon` if both are set.
    pub fn left_painter_circle(mut self, color: Color32) -> Self {
        self.left_dot = Some(color); self
    }
    /// Explicit left indent (px) before any left icon/dot.
    pub fn indent(mut self, px: f32) -> Self { self.indent = px; self }
    /// Width of the right-aligned trailing zone (default 80).
    pub fn trailing_width(mut self, w: f32) -> Self { self.trailing_width = w; self }
    pub fn sense(mut self, s: Sense) -> Self { self.sense = s; self }
    /// Paint a full-row tinted background at the given color + alpha (0–255),
    /// layered on top of the normal hover/selection background. Used for
    /// buy/sell direction tinting in tape rows and similar directional lists.
    pub fn row_tint(mut self, color: Color32, alpha: u8) -> Self {
        self.row_tint = Some((color, alpha)); self
    }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme_bg = Some(t.toolbar_bg);
        self.theme_border = Some(t.toolbar_border);
        self.theme_accent = Some(t.accent);
        self
    }
    pub fn body<B2: FnOnce(&mut Ui) + 'a>(self, f: B2) -> ListRow<'a, B2, T> {
        ListRow {
            height: self.height, selected: self.selected, hover_enabled: self.hover_enabled,
            divider: self.divider, left_icon: self.left_icon, left_dot: self.left_dot,
            indent: self.indent, body: Some(f),
            trailing: self.trailing, trailing_width: self.trailing_width,
            theme_bg: self.theme_bg, theme_border: self.theme_border,
            theme_accent: self.theme_accent, sense: self.sense, row_tint: self.row_tint,
        }
    }
    pub fn trailing_actions<T2: FnOnce(&mut Ui) + 'a>(self, f: T2) -> ListRow<'a, B, T2> {
        ListRow {
            height: self.height, selected: self.selected, hover_enabled: self.hover_enabled,
            divider: self.divider, left_icon: self.left_icon, left_dot: self.left_dot,
            indent: self.indent, body: self.body,
            trailing: Some(f), trailing_width: self.trailing_width,
            theme_bg: self.theme_bg, theme_border: self.theme_border,
            theme_accent: self.theme_accent, sense: self.sense, row_tint: self.row_tint,
        }
    }
    /// Alias of `trailing_actions` — paints right-aligned action icons inside
    /// the trailing zone.
    pub fn right_actions<T2: FnOnce(&mut Ui) + 'a>(self, f: T2) -> ListRow<'a, B, T2> {
        self.trailing_actions(f)
    }

    /// Render the row. Returns the row's `Response` so callers can detect
    /// click/hover.
    pub fn show(self, ui: &mut Ui) -> Response {
        let w = ui.available_width();
        let rect = egui::Rect::from_min_size(
            egui::pos2(ui.cursor().min.x, ui.cursor().min.y),
            egui::vec2(w, self.height),
        );
        let resp = ui.allocate_rect(rect, self.sense);

        let border = self.theme_border.unwrap_or(ft().toolbar_border);
        let accent = self.theme_accent.unwrap_or(ft().accent);

        let bg = if self.selected {
            color_alpha(accent, alpha_subtle())
        } else if self.hover_enabled && resp.hovered() {
            color_alpha(border, alpha_muted())
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 2.0, bg);

        if let Some((tint_color, tint_alpha)) = self.row_tint {
            ui.painter().rect_filled(rect, 0.0, color_alpha(tint_color, tint_alpha));
        }

        if self.selected {
            // Left accent bar like watchlist active rows.
            let bar = egui::Rect::from_min_size(rect.min, egui::vec2(2.0, rect.height()));
            ui.painter().rect_filled(bar, 0.0, accent);
        }

        // Run body inside an inner Ui clipped to the row rect.
        let mut inner_x = rect.min.x + 6.0 + self.indent;
        if let Some(dot) = self.left_dot {
            ui.painter().circle_filled(
                egui::pos2(inner_x + 4.0, rect.center().y),
                3.0,
                dot,
            );
            inner_x += 12.0;
        }
        if let Some((glyph, col)) = self.left_icon {
            ui.painter().text(
                egui::pos2(inner_x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                glyph,
                egui::FontId::monospace(11.0),
                col,
            );
            inner_x += 14.0;
        }

        if let Some(body) = self.body {
            let body_rect = egui::Rect::from_min_max(
                egui::pos2(inner_x, rect.min.y),
                egui::pos2(rect.max.x - 6.0, rect.max.y),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(body_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            body(&mut child);
        }

        if let Some(trailing) = self.trailing {
            let t_rect = egui::Rect::from_min_max(
                egui::pos2(rect.max.x - self.trailing_width, rect.min.y),
                egui::pos2(rect.max.x - 4.0, rect.max.y),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(t_rect)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            trailing(&mut child);
        }

        if self.divider {
            let y = rect.max.y - 0.5;
            ui.painter().line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                Stroke::new(stroke_thin(), color_alpha(border, alpha_dim())),
            );
        }

        if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        crate::design_tokens::register_hit(
            [rect.min.x, rect.min.y, rect.width(), rect.height()],
            "LIST_ROW",
            "Rows",
        );

        resp
    }
}

// Domain rows moved to lists::rows — re-export for backward compat
pub mod alert_row {
    pub use crate::chart::renderer::ui::lists::rows::alert_row::*;
}
pub mod dom_row {
    pub use crate::chart::renderer::ui::lists::rows::dom_row::*;
}
pub mod news_row {
    pub use crate::chart::renderer::ui::lists::rows::news_row::*;
}
pub mod option_chain_row {
    pub use crate::chart::renderer::ui::lists::rows::option_chain_row::*;
}
pub mod order_row {
    pub use crate::chart::renderer::ui::lists::rows::order_row::*;
}
pub mod watchlist_row {
    pub use crate::chart::renderer::ui::lists::rows::watchlist_row::*;
}
// Re-exports for direct items
pub use crate::chart::renderer::ui::lists::rows::*;
