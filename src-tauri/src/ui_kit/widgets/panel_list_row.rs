//! PanelListRow — the canonical row primitive for panel lists.
//!
//! Replaces the per-panel hand-rolled "row" patterns (watchlist rows, alert
//! rows, scanner rows, order rows, journal rows, etc.) with one builder that
//! handles hover, selection, leading/trailing slots, and consistent
//! typography.
//!
//! ```ignore
//! let resp = PanelListRow::new("aapl_row")
//!     .leading(|ui, t| { /* avatar / color swatch / drag handle */ })
//!     .primary("AAPL")
//!     .secondary("Apple Inc")
//!     .trailing(|ui, t| { /* price / X button / dropdown */ })
//!     .selected(is_selected)
//!     .show(ui, t);
//! if resp.clicked() { /* ... */ }
//! ```
//!
//! Visual spec (LOCKED per user decision):
//! - Hover: `color_alpha(t.text, 8)` background, `radius_sm()` corners.
//! - Selected: BOTH `color_alpha(t.accent, 24)` background fill AND a 2px
//!   accent stripe on the LEFT edge. User explicitly chose "both" — loud is OK.
//! - No stroke. No border. Just background and the stripe.
//! - Primary text: `mono_sm` in `t.text`.
//! - Secondary text: `mono_xs` in `color_muted(t.dim)`, on a second line.
//! - Row height: 22px default; 32px when `.dense(false)`.
//! - LR padding: `gap_md`.
//!
//! Returns the full-row `Response` so callers can call `.clicked()`,
//! `.hovered()`, etc. Cursor is routed to `PointingHand` on hover.
//!
//! When to use:
//! - Any repeating list item inside a `PanelSection` body.
//!
//! When NOT to use:
//! - Menus / dropdown items — use `SelectableRow`.
//! - Label/value metric rows — use `PanelKeyValueRow` or `MetricRow`.
//! - Form fields — use `FormRow`.
//!
//! Sister widgets: `SelectableRow`, `PanelKeyValueRow`, `PanelCard`.

use egui::{CornerRadius, FontId, Pos2, Rect, Response, Sense, Ui, Vec2};

use crate::chart::renderer::ui::style::{
    color_alpha, color_muted, font_sm, font_xs, gap_md, gap_xs, radius_sm,
};
use crate::chart_renderer::gpu::Theme;

/// Width (in px) of the left accent stripe for the selected state.
const SELECTED_STRIPE_W: f32 = 2.0;

/// Hover background alpha (out of 255), applied to `t.text`.
const HOVER_BG_ALPHA: u8 = 8;

/// Selected background alpha (out of 255), applied to `t.accent`.
const SELECTED_BG_ALPHA: u8 = 24;

#[must_use = "PanelListRow must be rendered with `.show(...)`"]
pub struct PanelListRow<'a> {
    id_salt: &'a str,
    primary: Option<&'a str>,
    secondary: Option<&'a str>,
    leading: Option<Box<dyn FnOnce(&mut Ui, &Theme) + 'a>>,
    trailing: Option<Box<dyn FnOnce(&mut Ui, &Theme) + 'a>>,
    selected: bool,
    dense: bool,
}

impl<'a> PanelListRow<'a> {
    pub fn new(id_salt: &'a str) -> Self {
        Self {
            id_salt,
            primary: None,
            secondary: None,
            leading: None,
            trailing: None,
            selected: false,
            dense: true,
        }
    }

    pub fn primary(mut self, p: &'a str) -> Self {
        self.primary = Some(p);
        self
    }

    pub fn secondary(mut self, s: &'a str) -> Self {
        self.secondary = Some(s);
        self
    }

    pub fn leading(mut self, f: impl FnOnce(&mut Ui, &Theme) + 'a) -> Self {
        self.leading = Some(Box::new(f));
        self
    }

    pub fn trailing(mut self, f: impl FnOnce(&mut Ui, &Theme) + 'a) -> Self {
        self.trailing = Some(Box::new(f));
        self
    }

    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    /// Dense rows are 22px tall (default). Non-dense rows are 32px and
    /// generally only used when the row carries a secondary line + leading
    /// graphics.
    pub fn dense(mut self, on: bool) -> Self {
        self.dense = on;
        self
    }

    pub fn show(self, ui: &mut Ui, t: &Theme) -> Response {
        let Self {
            id_salt,
            primary,
            secondary,
            leading,
            trailing,
            selected,
            dense,
        } = self;

        let h = if dense { 22.0 } else { 32.0 };
        let avail_w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(avail_w, h),
            Sense::click(),
        );
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let resp = ui.interact(rect, ui.id().with(("panel_list_row", id_salt)), Sense::click())
            .union(resp);

        if !ui.is_rect_visible(rect) {
            return resp;
        }

        let painter = ui.painter_at(rect);
        let cr = CornerRadius::same(radius_sm() as u8);

        // Background — selected wins over hover.
        if selected {
            painter.rect_filled(rect, cr, color_alpha(t.accent, SELECTED_BG_ALPHA));
        } else if resp.hovered() {
            painter.rect_filled(rect, cr, color_alpha(t.text, HOVER_BG_ALPHA));
        }

        // Left accent stripe — selected only.
        if selected {
            let stripe = Rect::from_min_max(
                Pos2::new(rect.left(), rect.top()),
                Pos2::new(rect.left() + SELECTED_STRIPE_W, rect.bottom()),
            );
            painter.rect_filled(stripe, 0.0, t.accent);
        }

        // Layout LTR: leading | text block | (RTL) trailing.
        let inner_left = rect.left() + gap_md();
        let inner_right = rect.right() - gap_md();
        let content_rect = Rect::from_min_max(
            Pos2::new(inner_left, rect.top()),
            Pos2::new(inner_right, rect.bottom()),
        );

        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        if let Some(lead) = leading {
            lead(&mut child, t);
            child.add_space(gap_xs());
        }

        // Reserve text block; trailing slot floats right.
        // We use right_to_left layout for the trailing slot via a nested with_layout.
        child.with_layout(
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                // Text block (vertical) — primary + optional secondary.
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if let Some(p) = primary {
                        let font = FontId::monospace(font_sm());
                        let galley = ui.fonts(|f| {
                            f.layout_no_wrap(p.to_string(), font.clone(), t.text)
                        });
                        let (r, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
                        ui.painter().galley(r.min, galley, t.text);
                    }
                    if let Some(s) = secondary {
                        let font = FontId::monospace(font_xs());
                        let col = color_muted(t.dim);
                        let galley = ui.fonts(|f| {
                            f.layout_no_wrap(s.to_string(), font.clone(), col)
                        });
                        let (r, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
                        ui.painter().galley(r.min, galley, col);
                    }
                });

                // Trailing slot: float to the right.
                if let Some(trail) = trailing {
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            trail(ui, t);
                        },
                    );
                }
            },
        );

        resp
    }
}
