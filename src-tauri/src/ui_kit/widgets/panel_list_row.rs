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
//! ## Columns mode
//!
//! For streaming-data panels (tape prints, scanner T&S, journal entries)
//! where each row is N free-form text columns instead of the
//! leading/primary/secondary/trailing layout, call `.columns(&[...])`:
//!
//! ```ignore
//! PanelListRow::new("print_123")
//!     .columns(&[
//!         Column::left("09:31:42.123").color(t.dim),
//!         Column::right("$145.32").color(t.bull),
//!         Column::right("250").color(t.text),
//!     ])
//!     .show(ui, t);
//! ```
//!
//! Selected + hover backgrounds STILL apply. When `.columns()` is set,
//! `.primary()` / `.secondary()` / `.leading()` / `.trailing()` are
//! ignored. The slice is borrowed for one frame; no per-cell heap
//! allocations.
//!
//! When to use:
//! - Any repeating list item inside a `PanelSection` body.
//! - Streaming data rows — pass `.columns(&[...])`.
//!
//! When NOT to use:
//! - Menus / dropdown items — use `SelectableRow`.
//! - Label/value metric rows — use `PanelKeyValueRow` or `MetricRow`.
//! - Form fields — use `FormRow`.
//!
//! Sister widgets: `SelectableRow`, `PanelKeyValueRow`, `PanelCard`.

use egui::{Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Ui, Vec2};

use crate::chart::renderer::ui::style::{
    color_alpha, color_muted, font_sm, font_xs, gap_md, gap_xs, radius_sm,
};
use crate::chart_renderer::gpu::Theme;

/// Horizontal alignment for a `Column` cell in `PanelListRow::columns` mode.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum ColAlign {
    #[default]
    Left,
    Right,
    Center,
}

/// One cell in a column-mode `PanelListRow`. Borrow-friendly: holds a
/// `&str` so callers can build a `&[Column]` on the stack each frame with
/// no allocation per cell.
#[derive(Copy, Clone, Debug)]
pub struct Column<'a> {
    pub text: &'a str,
    pub align: ColAlign,
    pub color: Color32,
    /// Flex weight against the remaining row width after `min_width`
    /// reservations. Default `1.0`.
    pub weight: f32,
    pub min_width: Option<f32>,
    /// Monospace cell? Default `true` — dense data rows are the use case.
    pub mono: bool,
}

impl<'a> Column<'a> {
    /// Left-aligned cell with default styling (mono, white text color via
    /// `Color32::PLACEHOLDER` — caller is expected to set `.color()` to a
    /// theme value, but if they don't we fall back to white at paint time).
    pub fn left(text: &'a str) -> Self {
        Self {
            text,
            align: ColAlign::Left,
            color: Color32::PLACEHOLDER,
            weight: 1.0,
            min_width: None,
            mono: true,
        }
    }

    pub fn right(text: &'a str) -> Self {
        Self { align: ColAlign::Right, ..Self::left(text) }
    }

    pub fn center(text: &'a str) -> Self {
        Self { align: ColAlign::Center, ..Self::left(text) }
    }

    pub fn color(mut self, c: Color32) -> Self {
        self.color = c;
        self
    }

    pub fn weight(mut self, w: f32) -> Self {
        self.weight = w;
        self
    }

    pub fn min_width(mut self, px: f32) -> Self {
        self.min_width = Some(px);
        self
    }

    pub fn mono(mut self, on: bool) -> Self {
        self.mono = on;
        self
    }
}

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
    /// When `Some`, the row paints as N free-form columns and IGNORES
    /// `primary` / `secondary` / `leading` / `trailing`. Selected and
    /// hover backgrounds still apply.
    columns: Option<&'a [Column<'a>]>,
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
            columns: None,
            selected: false,
            dense: true,
        }
    }

    /// Switch this row into column-layout mode for streaming-data panels
    /// (tape, scanner T&S, journal). The slice is borrowed for one frame
    /// and copied internally only for layout math — no per-cell heap
    /// allocations. Selected and hover backgrounds still apply.
    ///
    /// When this is set, the `primary` / `secondary` / `leading` /
    /// `trailing` slots are ignored.
    pub fn columns(mut self, cols: &'a [Column<'a>]) -> Self {
        self.columns = Some(cols);
        self
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
            columns,
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

        // ── Columns mode ────────────────────────────────────────────
        // When `.columns()` was called, paint a free-form column row and
        // return. Streaming-data hot path — no nested UIs, no closures,
        // just the painter.
        if let Some(cols) = columns {
            paint_columns(ui, rect, cols, t);
            return resp;
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

/// Paint a column-mode row. Pure painter calls — no nested `Ui` allocations,
/// no per-cell closures. Suitable for streaming panels that emit hundreds of
/// rows per frame (tape, scanner T&S).
///
/// Layout: each column gets `min_width` reserved up front; remaining width is
/// divided among columns by `weight`. Text is clipped to its cell rect.
fn paint_columns(ui: &mut Ui, rect: Rect, cols: &[Column<'_>], t: &Theme) {
    if cols.is_empty() {
        return;
    }
    let painter = ui.painter_at(rect);

    let inner_left = rect.left() + gap_md();
    let inner_right = rect.right() - gap_md();
    let content_w = (inner_right - inner_left).max(0.0);
    let n = cols.len();
    // Inter-column gap. Small — these rows are dense by design.
    let gap = gap_xs();
    let total_gaps = gap * (n as f32 - 1.0).max(0.0);

    // Sum mins + weights.
    let mut min_total = 0.0_f32;
    let mut weight_total = 0.0_f32;
    for c in cols {
        min_total += c.min_width.unwrap_or(0.0);
        weight_total += c.weight.max(0.0);
    }
    let flex_w = (content_w - total_gaps - min_total).max(0.0);

    let cy = rect.center().y;
    let mut x = inner_left;
    for (i, c) in cols.iter().enumerate() {
        let base = c.min_width.unwrap_or(0.0);
        let extra = if weight_total > 0.0 {
            flex_w * (c.weight.max(0.0) / weight_total)
        } else {
            0.0
        };
        let cell_w = base + extra;
        let cell_rect = Rect::from_min_max(
            Pos2::new(x, rect.top()),
            Pos2::new(x + cell_w, rect.bottom()),
        );

        let font = if c.mono {
            FontId::monospace(font_sm())
        } else {
            FontId::proportional(font_sm())
        };
        let color = if c.color == Color32::PLACEHOLDER { t.text } else { c.color };
        let galley = ui.fonts(|f| f.layout_no_wrap(c.text.to_string(), font, color));
        let tw = galley.size().x;
        let th = galley.size().y;
        let tx = match c.align {
            ColAlign::Left => cell_rect.left(),
            ColAlign::Right => cell_rect.right() - tw,
            ColAlign::Center => cell_rect.center().x - tw * 0.5,
        };
        // Clip to cell so overflow doesn't bleed into the next column.
        let cell_painter = painter.with_clip_rect(cell_rect);
        cell_painter.galley(Pos2::new(tx, cy - th * 0.5), galley, color);

        x += cell_w;
        if i + 1 < n {
            x += gap;
        }
    }
}
