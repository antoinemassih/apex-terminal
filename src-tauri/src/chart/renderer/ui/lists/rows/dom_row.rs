//! DomRow — single price-ladder rung. Faithful rendition of the real DOM
//! row used by `dom_panel.rs` (Δ / BID / PRICE / ASK / VOL / ORD), with
//! adaptive column layout, depth-bar fills, split-color text-over-bar,
//! inside-spread highlight, current-price border, imbalance ghost fills,
//! selected-row accent stripe, and draggable order badges.
//!
//! The widget paints inside `RowShell::painter_mode` so it respects any
//! external clip set by the parent UI (e.g. `ui.painter_at(dom_rect)` in
//! the DOM sidebar). All inner geometry is derived from the row rect,
//! never from absolute screen coordinates.
//!
//! THIS IS THE LIVE LADDER ROW. `dom_panel.rs::draw` builds a `DomRow` per
//! price level and calls `.show_in(ui, &lp, rr)` with the clipped ladder
//! painter — see `dom_panel.rs` ~L700. Edits here change the real DOM.
//!
//! The module doc used to say the opposite ("ladder-row capable but not yet
//! wired into `dom_panel.rs::draw` ... can be used standalone"), describing a
//! migration that has since happened. Left uncorrected it tells the next
//! person their changes here are inert, which is the most expensive kind of
//! stale comment: it does not just fail to help, it argues against looking.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Painter, Rect, Response, Sense, Stroke, StrokeKind, Ui};
use super::super::super::style::*;
use crate::chart::renderer::ui::foundation::{
    interaction::InteractionState,
    shell::RowShell,
};
use crate::ui_kit::widgets::RowVariant;
use crate::ui_kit::widgets::tokens::Size;
use crate::chart_renderer::ui::foundation::text_style::TextStyle;

/// Pre-computed column geometry shared across all rows in a ladder.
/// Captured once at the top of `dom_panel::draw` and passed in via
/// `DomRow::column_layout()`. When provided, the widget uses these
/// absolute x-coordinates instead of computing column rects from
/// `ColumnSpec` fractions, so headers and rows align perfectly.
#[derive(Clone, Copy, Debug, Default)]
pub struct ColumnLayout {
    pub x0: f32,
    pub cd: f32, // delta width (0 if hidden)
    pub xb: f32, pub cb: f32,
    pub xp: f32, pub cp: f32,
    pub xa: f32, pub ca: f32,
    pub xv: f32, pub cv: f32, // vol (0 if hidden)
    pub xo: f32, pub co: f32, // orders
    pub show_delta: bool,
    pub show_vol: bool,
    pub show_numbers: bool,
}

/// Cross-row drag context for the ladder. Painted-as-ghost only by the row
/// that is currently the drop-target; the dragged source row dims its chip.
#[derive(Clone, Copy, Debug, Default)]
pub struct DomRowDragCx {
    /// The order id currently being dragged (None if no drag in progress).
    pub dragging_order_id: Option<u32>,
    /// True if THIS row is the drop-target (parent computes from y).
    pub is_drop_target: bool,
    /// If `is_drop_target`, the badge to render as a ghost on this row.
    pub ghost_side: Option<char>,
    pub ghost_qty: u32,
    pub ghost_color: Color32,
}

/// Result of a `show_in` row paint — extends `DomRowResponse` with
/// per-order drag callbacks the ladder needs to update parent state.
#[derive(Default)]
pub struct DomRowLadderResponse {
    pub row_clicked: bool,
    pub row_hovered: bool,
    /// (order_id, drag_started)
    pub order_drag_started: Option<u32>,
    /// (order_id, accumulated drag_delta_y on this frame)
    pub order_dragging: Option<(u32, f32)>,
    /// (order_id) drag stopped this frame
    pub order_drag_stopped: Option<u32>,
    /// (order_id) X-cancel clicked
    pub order_cancel: Option<u32>,
}

type Theme = crate::chart_renderer::gpu::Theme;

/// Which built-in column to show in addition to the bid/price/ask trio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomColumn {
    Delta,
    Bid,
    Price,
    Ask,
    Volume,
    Orders,
}

/// Width spec for a single column inside the row.
#[derive(Clone, Copy, Debug)]
pub struct ColumnSpec {
    pub kind: DomColumn,
    /// Fraction of the row width (0..=1). Specs are normalized at paint time.
    pub frac: f32,
}

/// One order chip rendered inside the ORD column.
#[derive(Clone, Copy, Debug)]
pub struct OrderBadge {
    pub id: u32,
    /// 'B' for buy/long-trigger, 'S' for sell/short-trigger.
    pub side: char,
    pub qty: u32,
    pub color: Color32,
}

/// Response returned by `DomRow::show`. Includes per-zone click flags so
/// callers can route clicks to bid/ask/price/order without re-hit-testing.
pub struct DomRowResponse {
    pub response: Response,
    pub bid_clicked: bool,
    pub ask_clicked: bool,
    pub price_clicked: bool,
    pub order_clicked: Option<u32>,
    pub order_drag_started: Option<u32>,
}

#[must_use = "DomRow must be finalized with `.show(ui)` to render"]
pub struct DomRow<'a> {
    price: f32,
    bid_size: u32,
    ask_size: u32,
    bid_fill: f32,
    ask_fill: f32,
    volume: u64,
    volume_fill: f32,
    delta: i64,
    is_inside: bool,
    selected: bool,
    current_price: bool,
    imbalance: f32, // -1..=1, positive = ask side, negative = bid side
    height: f32,
    price_fmt: &'a str,
    columns: Option<&'a [ColumnSpec]>,
    orders: &'a [OrderBadge],
    show_numbers: bool,
    theme: Option<&'a Theme>,
    column_layout: Option<ColumnLayout>,
    drag_cx: DomRowDragCx,
    hover_active: bool,
    compact_price: bool,
    price_color_override: Option<Color32>,
    /// (label, color) overrides for ladder-style rich badges.
    rich_orders: &'a [(u32, char, u32, Color32)],
}

/// The shortest rung that still contains the row's TALLEST text tier.
///
/// M5: the previous floor was `mono_sm().size + 4.0` (= 16.0), and its comment
/// said it existed so "the `mono_sm()` text the row paints always fits". But
/// this row's price and size columns paint `TextStyle::MonoMd` (see the
/// `f_lg` / `font` bindings below) — the taller tier, added specifically so
/// this ladder could join the cascade. The floor was never moved with it, so it
/// tracked the smaller tier and came up short:
///
///     needs  font_md() * line_dense() = 14 * 1.3 = 18.2
///     floor  mono_sm().size + 4.0     = 12 + 4   = 16.0
///
/// A floor that measures the wrong tier is the same defect as a pinned
/// dimension — it is derived, but from something that is not what it guards.
pub(crate) fn dom_row_min_height() -> f32 {
    crate::ui_kit::style::font_md() * crate::ui_kit::style::line_dense()
}

impl<'a> DomRow<'a> {
    pub fn new(price: f32, bid_size: u32, ask_size: u32) -> Self {
        Self {
            price, bid_size, ask_size,
            bid_fill: 0.0, ask_fill: 0.0,
            volume: 0, volume_fill: 0.0, delta: 0,
            is_inside: false, selected: false, current_price: false,
            imbalance: 0.0,
            // Density-aware default row height, floored so the tallest text
            // tier the row paints always fits inside the rung. See
            // `dom_row_min_height` — the floor used to measure `mono_sm()`
            // while the row paints `MonoMd`.
            height: crate::chart_renderer::ui::style::style_row_height()
                .max(dom_row_min_height()),
            price_fmt: "{:.2}",
            columns: None, orders: &[],
            show_numbers: true,
            theme: None,
            column_layout: None,
            drag_cx: DomRowDragCx::default(),
            hover_active: false,
            compact_price: false,
            price_color_override: None,
            rich_orders: &[],
        }
    }
    pub fn column_layout(mut self, l: ColumnLayout) -> Self { self.column_layout = Some(l); self }
    pub fn drag_cx(mut self, cx: DomRowDragCx) -> Self { self.drag_cx = cx; self }
    pub fn hover_active(mut self, v: bool) -> Self { self.hover_active = v; self }
    pub fn compact_price(mut self, v: bool) -> Self { self.compact_price = v; self }
    pub fn price_color(mut self, c: Color32) -> Self { self.price_color_override = Some(c); self }
    pub fn rich_orders(mut self, o: &'a [(u32, char, u32, Color32)]) -> Self { self.rich_orders = o; self }
    pub fn bid_fill(mut self, v: f32) -> Self { self.bid_fill = v.clamp(0.0, 1.0); self }
    pub fn ask_fill(mut self, v: f32) -> Self { self.ask_fill = v.clamp(0.0, 1.0); self }
    pub fn delta(mut self, d: i64) -> Self { self.delta = d; self }
    pub fn volume(mut self, v: u64, fill: f32) -> Self {
        self.volume = v; self.volume_fill = fill.clamp(0.0, 1.0); self
    }
    pub fn orders(mut self, o: &'a [OrderBadge]) -> Self { self.orders = o; self }
    pub fn columns(mut self, c: &'a [ColumnSpec]) -> Self { self.columns = Some(c); self }
    pub fn inside_spread(mut self, v: bool) -> Self { self.is_inside = v; self }
    pub fn is_inside(mut self, v: bool) -> Self { self.is_inside = v; self }
    pub fn current_price(mut self, v: bool) -> Self { self.current_price = v; self }
    pub fn imbalance_fill(mut self, dir: f32) -> Self {
        self.imbalance = dir.clamp(-1.0, 1.0); self
    }
    pub fn selected(mut self, v: bool) -> Self { self.selected = v; self }
    pub fn show_numbers(mut self, v: bool) -> Self { self.show_numbers = v; self }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn theme(mut self, t: &'a Theme) -> Self { self.theme = Some(t); self }

    pub fn show(self, ui: &mut Ui) -> DomRowResponse {
        let theme_ref: &Theme = self.theme.expect("DomRow requires a theme — call `.theme(t)` before `.show()`");
        let bull = theme_ref.bull;
        let bear = theme_ref.bear;
        let dim = theme_ref.dim;
        let fg = theme_ref.text;
        let accent = theme_ref.accent;

        // Snapshot fields used inside the painter closure.
        let price = self.price;
        let bid_size = self.bid_size;
        let ask_size = self.ask_size;
        let bid_fill = self.bid_fill;
        let ask_fill = self.ask_fill;
        let volume = self.volume;
        let volume_fill = self.volume_fill;
        let delta = self.delta;
        let is_inside = self.is_inside;
        let is_current = self.current_price;
        let imbalance = self.imbalance;
        let selected = self.selected;
        let show_numbers = self.show_numbers;

        // Resolve column layout.
        let default_cols = [
            ColumnSpec { kind: DomColumn::Bid, frac: 0.30 },
            ColumnSpec { kind: DomColumn::Price, frac: 0.40 },
            ColumnSpec { kind: DomColumn::Ask, frac: 0.30 },
        ];
        let cols: Vec<ColumnSpec> = match self.columns {
            Some(c) => c.to_vec(),
            None => default_cols.to_vec(),
        };
        // Order chips passed in.
        let orders: Vec<OrderBadge> = self.orders.to_vec();

        // Carry zone-click flags out of the closure.
        let zones = std::cell::RefCell::new(ZoneInfo::default());

        let resp = RowShell::new(theme_ref, "")
            .variant(RowVariant::Default)
            .size(Size::Md)
            .state(InteractionState::default().selected(selected))
            .painter_mode(true)
            .painter_height(self.height)
            .painter_body(|ui, rect| {
                let painter = ui.painter();

                // Selected accent stripe (left edge).
                if selected {
                    let bar = Rect::from_min_size(rect.min, egui::vec2(2.0, rect.height()));
                    painter.rect_filled(bar, 0.0, accent);
                }

                // Imbalance ghost fill — biased side.
                if imbalance.abs() > 0.001 {
                    let mag = imbalance.abs().min(1.0);
                    let col = if imbalance > 0.0 {
                        color_alpha(bear, (alpha_ghost() as f32 * mag) as u8)
                    } else {
                        color_alpha(bull, (alpha_ghost() as f32 * mag) as u8)
                    };
                    painter.rect_filled(rect, 0.0, col);
                }

                // Compute column rects.
                let total: f32 = cols.iter().map(|c| c.frac).sum::<f32>().max(0.001);
                let mut x = rect.min.x;
                let mut col_rects: Vec<(DomColumn, Rect)> = Vec::with_capacity(cols.len());
                for c in &cols {
                    let w = rect.width() * (c.frac / total);
                    let r = Rect::from_min_size(egui::pos2(x, rect.min.y), egui::vec2(w, rect.height()));
                    col_rects.push((c.kind, r));
                    x += w;
                }
                let find = |k: DomColumn| col_rects.iter().find(|(kk, _)| *kk == k).map(|(_, r)| *r);

                // Hot path (a ladder paints ~40 rungs/frame): one cascade lookup
                // per tier, cloned per string. `MonoMd` is the 14px tabular-data
                // rung — added specifically so this ladder could join the cascade.
                let f_lg = TextStyle::MonoMd.font_id_in(ui);
                let f_sm = TextStyle::MonoSm.font_id_in(ui);
                let dark = theme_ref.overlay_text;
                let cy = rect.center().y;

                // ── Δ column ──
                if let Some(dr) = find(DomColumn::Delta) {
                    if delta != 0 {
                        let dc = if delta > 0 { color_muted(bull) } else { color_muted(bear) };
                        let s = if delta > 0 { format!("+{}", delta) } else { format!("{}", delta) };
                        painter.text(dr.center(), egui::Align2::CENTER_CENTER, &s, f_sm.clone(), dc);
                    }
                }

                // ── BID column ──
                if let Some(br) = find(DomColumn::Bid) {
                    let mut bar_rect_opt: Option<Rect> = None;
                    if bid_fill > 0.0 {
                        let bw = br.width() * 0.85 * bid_fill;
                        let bar = Rect::from_min_size(
                            egui::pos2(br.right() - bw - 1.0, br.min.y + 1.0),
                            egui::vec2(bw, br.height() - 2.0),
                        );
                        painter.rect_filled(bar, radius_xs(), color_alpha(bull, (60.0 + bid_fill * 140.0) as u8));
                        bar_rect_opt = Some(bar);
                    }
                    if show_numbers && bid_size > 0 {
                        let txt = fmt_size(bid_size);
                        let pos = br.center();
                        let normal = color_subtle(bull);
                        painter.text(pos, egui::Align2::CENTER_CENTER, &txt, f_lg.clone(), normal);
                        if let Some(bar) = bar_rect_opt {
                            if bid_fill > 0.2 {
                                let clip = ui.painter_at(bar);
                                clip.text(pos, egui::Align2::CENTER_CENTER, &txt, f_lg.clone(), dark);
                            }
                        }
                    }
                }

                // ── PRICE column ──
                if let Some(pr) = find(DomColumn::Price) {
                    let pc = if is_current { contrast_fg(accent) }
                        else if is_inside { accent }
                        else if price > 0.0 { fg } else { fg };
                    painter.text(pr.center(), egui::Align2::CENTER_CENTER,
                        &format!("{:.2}", price), f_lg.clone(), pc);
                }

                // ── ASK column ──
                if let Some(ar) = find(DomColumn::Ask) {
                    let mut bar_rect_opt: Option<Rect> = None;
                    if ask_fill > 0.0 {
                        let bw = ar.width() * 0.85 * ask_fill;
                        let bar = Rect::from_min_size(
                            egui::pos2(ar.min.x + 1.0, ar.min.y + 1.0),
                            egui::vec2(bw, ar.height() - 2.0),
                        );
                        painter.rect_filled(bar, radius_xs(), color_alpha(bear, (60.0 + ask_fill * 140.0) as u8));
                        bar_rect_opt = Some(bar);
                    }
                    if show_numbers && ask_size > 0 {
                        let txt = fmt_size(ask_size);
                        let pos = ar.center();
                        let normal = color_subtle(bear);
                        painter.text(pos, egui::Align2::CENTER_CENTER, &txt, f_lg.clone(), normal);
                        if let Some(bar) = bar_rect_opt {
                            if ask_fill > 0.2 {
                                let clip = ui.painter_at(bar);
                                clip.text(pos, egui::Align2::CENTER_CENTER, &txt, f_lg.clone(), dark);
                            }
                        }
                    }
                }

                // ── VOL column ──
                if let Some(vr) = find(DomColumn::Volume) {
                    if volume > 0 {
                        let bw = vr.width() * 0.8 * volume_fill;
                        let bar = Rect::from_min_size(
                            egui::pos2(vr.min.x + 1.0, vr.min.y + 1.0),
                            egui::vec2(bw, vr.height() - 2.0),
                        );
                        painter.rect_filled(bar, radius_xs(), color_alpha(dim, alpha_subtle()));
                        let s = if volume >= 1_000_000 { format!("{:.1}M", volume as f64/1e6) }
                            else if volume >= 1_000 { format!("{:.0}K", volume as f64/1e3) }
                            else { format!("{}", volume) };
                        let pos = vr.center();
                        painter.text(pos, egui::Align2::CENTER_CENTER, &s, f_sm.clone(), color_half(dim));
                        if volume_fill > 0.3 {
                            let clip = ui.painter_at(bar);
                            clip.text(pos, egui::Align2::CENTER_CENTER, &s, f_sm.clone(), dark);
                        }
                    }
                }

                // ── ORD column: order badges ──
                if let Some(or) = find(DomColumn::Orders) {
                    if !orders.is_empty() {
                        // Stack chips horizontally inside the column.
                        let n = orders.len() as f32;
                        let chip_w = (or.width() - 2.0) / n.max(1.0);
                        for (i, ord) in orders.iter().enumerate() {
                            let cr = Rect::from_min_size(
                                egui::pos2(or.min.x + 1.0 + i as f32 * chip_w, or.min.y + 1.0),
                                egui::vec2(chip_w - 1.0, or.height() - 2.0),
                            );
                            painter.rect_filled(cr, radius_xs(), color_alpha(ord.color, alpha_intense()));
                            painter.rect_stroke(cr, radius_xs(),
                                Stroke::new(stroke_thin(), color_alpha(ord.color, alpha_prominent())),
                                StrokeKind::Outside);
                            let label = format!("{}{}", ord.side, ord.qty);
                            painter.text(cr.center(), egui::Align2::CENTER_CENTER,
                                &label, f_sm.clone(), theme_ref.overlay_text);

                            // Hit-test: per-chip click + drag.
                            let id = ui.id().with(("dom_row_chip", ord.id));
                            let chip_resp = ui.interact(cr, id, Sense::click_and_drag());
                            let mut z = zones.borrow_mut();
                            if chip_resp.clicked() { z.order_clicked = Some(ord.id); }
                            if chip_resp.drag_started() { z.order_drag_started = Some(ord.id); }
                        }
                    }
                }

                // ── Inside-spread highlight ──
                if is_inside {
                    painter.rect_filled(rect, 0.0, color_alpha(accent, alpha_subtle()));
                }

                // ── Current-price border ──
                if is_current {
                    painter.rect_stroke(rect, 0.0,
                        Stroke::new(stroke_std(), color_alpha(accent, alpha_active())),
                        StrokeKind::Outside);
                }

                // ── Per-zone click hit-tests for bid/price/ask ──
                let mut z = zones.borrow_mut();
                if let Some(br) = find(DomColumn::Bid) {
                    let r = ui.interact(br, ui.id().with(("dom_row_bid", rect.min.x as i32, rect.min.y as i32)), Sense::click());
                    if r.clicked() { z.bid_clicked = true; }
                }
                if let Some(pr) = find(DomColumn::Price) {
                    let r = ui.interact(pr, ui.id().with(("dom_row_price", rect.min.x as i32, rect.min.y as i32)), Sense::click());
                    if r.clicked() { z.price_clicked = true; }
                }
                if let Some(ar) = find(DomColumn::Ask) {
                    let r = ui.interact(ar, ui.id().with(("dom_row_ask", rect.min.x as i32, rect.min.y as i32)), Sense::click());
                    if r.clicked() { z.ask_clicked = true; }
                }
            })
            .show(ui);

        crate::design_tokens::register_hit(
            [resp.rect.min.x, resp.rect.min.y, resp.rect.width(), resp.rect.height()],
            "DOM_ROW", "Rows");

        let z = zones.into_inner();
        DomRowResponse {
            response: resp,
            bid_clicked: z.bid_clicked,
            ask_clicked: z.ask_clicked,
            price_clicked: z.price_clicked,
            order_clicked: z.order_clicked,
            order_drag_started: z.order_drag_started,
        }
    }

    /// Ladder-mode entry point: paint into an externally-clipped parent
    /// `Painter` at the supplied row `rect`. Used by `dom_panel::draw` so
    /// the ladder body all renders into one `ui.painter_at(body_clip)`.
    /// Requires `column_layout()` to be set; falls back gracefully if not.
    pub fn show_in(self, ui: &mut Ui, painter: &Painter, rr: egui::Rect) -> DomRowLadderResponse {
        let theme_ref: &Theme = self.theme.expect("DomRow requires a theme — call `.theme(t)` before `.show_in()`");
        let bull = theme_ref.bull;
        let bear = theme_ref.bear;
        let dim = theme_ref.dim;
        let fg = theme_ref.text;
        let accent = theme_ref.accent;

        let mut out = DomRowLadderResponse::default();

        // Hit-test the row.
        let row_id = ui.id().with(("dom_row_ladder", rr.min.x as i32, rr.min.y as i32));
        let row_resp = ui.interact(rr, row_id, Sense::click());
        out.row_hovered = row_resp.hovered();
        out.row_clicked = row_resp.clicked();
        let hv = out.row_hovered;

        let layout = self.column_layout.unwrap_or_default();
        let x0 = layout.x0;
        let cd = layout.cd;
        let xb = layout.xb; let cb = layout.cb;
        let xp = layout.xp; let cp = layout.cp;
        let xa = layout.xa; let ca = layout.ca;
        let xv = layout.xv; let cv = layout.cv;
        let xo = layout.xo; let co = layout.co;
        let row_h = rr.height();
        let ry = rr.min.y;
        let cy = rr.center().y;

        // Hot path: hoisted once per row, cloned per painted string.
        let font = TextStyle::MonoMd.font_id_in(ui);
        let font_sm = TextStyle::MonoSm.font_id_in(ui);
        let dark = theme_ref.overlay_text;

        // Backgrounds: selected / current / hovered
        if self.selected {
            painter.rect_filled(rr, 0.0, color_alpha(accent, alpha_tint()));
            painter.line_segment(
                [egui::pos2(rr.min.x, ry), egui::pos2(rr.min.x, ry + row_h)],
                Stroke::new(stroke_thick(), accent),
            );
        } else if self.current_price {
            painter.rect_filled(rr, 0.0, color_alpha(accent, 35));
            painter.rect_stroke(rr, 0.0,
                Stroke::new(stroke_std(), color_alpha(accent, alpha_active())),
                StrokeKind::Outside);
        } else if hv {
            painter.rect_filled(rr, 0.0, color_alpha(theme_ref.toolbar_border, alpha_subtle()));
        }

        // Imbalance ghost-fill from bid/ask ratio.
        if self.bid_size > 0 && self.ask_size > 0 {
            let r = self.bid_size as f32 / self.ask_size as f32;
            if r > 3.0 { painter.rect_filled(rr, 0.0, color_alpha(bull, alpha_ghost())); }
            else if r < 0.33 { painter.rect_filled(rr, 0.0, color_alpha(bear, alpha_ghost())); }
        }

        // Every numeric cell below paints through `cell` — a painter clipped to
        // its own column, inset by CELL_GUTTER. Without it a too-wide string
        // spills into both neighbours (see `fit_cell`). `fit_cell` keeps text
        // inside the column in the normal case; the clip is the backstop that
        // makes a collision structurally impossible rather than merely
        // unlikely.
        let cell = |x: f32, w: f32| {
            painter.with_clip_rect(
                Rect::from_min_size(egui::pos2(x, ry), egui::vec2(w, row_h))
                    .shrink2(egui::vec2(CELL_GUTTER, 0.0))
                    .intersect(painter.clip_rect()),
            )
        };

        // Δ
        if layout.show_delta && self.delta != 0 {
            let dc = if self.delta > 0 { color_muted(bull) } else { color_muted(bear) };
            let s = fit_cell(ui, &delta_forms(self.delta), &font_sm, cd - CELL_GUTTER * 2.0);
            // LEFT-aligned, unlike the other cells. If a delta ever does
            // overflow its column, clipping must eat the least significant
            // digits — never the sign. Centre-aligned text loses both ends,
            // which is how `-1437` became `1437`: the same number, the
            // opposite meaning.
            cell(x0, cd).text(
                egui::pos2(x0 + CELL_GUTTER, cy), egui::Align2::LEFT_CENTER, &s, font_sm.clone(), dc);
        }

        // BID
        if self.bid_size > 0 {
            let fr = self.bid_fill.max(0.0).min(1.0);
            let bw = fr * cb * 0.85;
            let bar_rect = Rect::from_min_size(
                egui::pos2(xb + cb - bw - 1.0, ry + 1.0),
                egui::vec2(bw, row_h - 2.0),
            );
            painter.rect_filled(bar_rect, radius_xs(), color_alpha(bull, (60.0 + fr * 140.0) as u8));
            if layout.show_numbers {
                let txt = fit_cell(ui, &size_forms(self.bid_size), &font, cb - CELL_GUTTER * 2.0);
                let pos = egui::pos2(xb + cb * 0.5, cy);
                let normal = if hv { bull } else { color_subtle(bull) };
                cell(xb, cb).text(pos, egui::Align2::CENTER_CENTER, &txt, font.clone(), normal);
                if fr > 0.2 {
                    let clip = ui.painter_at(bar_rect);
                    clip.text(pos, egui::Align2::CENTER_CENTER, &txt, font.clone(), dark);
                }
            }
        }

        // PRICE — compact 5-char formatting when narrow
        let pc = self.price_color_override.unwrap_or_else(|| {
            if self.current_price { contrast_fg(accent) }
            else if self.selected { accent }
            else if self.imbalance > 0.0 { color_subtle(bull) }
            else { color_subtle(bear) }
        });
        let _ = fg;
        let ps = if self.compact_price {
            let s = format!("{:.2}", self.price);
            if s.len() > 5 && cp < 60.0 { format!("{:.1}", self.price) } else { s }
        } else if self.price >= 1.0 && (self.price.fract() == 0.0) {
            format!("{:.0}", self.price)
        } else { format!("{:.2}", self.price) };
        // Current-price rows used a separate mono_md() here — identical to
        // `font` now that both resolve through the MonoMd cascade tier.
        let price_font = font.clone();
        cell(xp, cp).text(
            egui::pos2(xp + cp * 0.5, cy), egui::Align2::CENTER_CENTER, &ps, price_font, pc);

        // ASK
        if self.ask_size > 0 {
            let fr = self.ask_fill.max(0.0).min(1.0);
            let bw = fr * ca * 0.85;
            let bar_rect = Rect::from_min_size(
                egui::pos2(xa + 1.0, ry + 1.0),
                egui::vec2(bw, row_h - 2.0),
            );
            painter.rect_filled(bar_rect, radius_xs(), color_alpha(bear, (60.0 + fr * 140.0) as u8));
            if layout.show_numbers {
                let txt = fit_cell(ui, &size_forms(self.ask_size), &font, ca - CELL_GUTTER * 2.0);
                let pos = egui::pos2(xa + ca * 0.5, cy);
                let normal = if hv { bear } else { color_subtle(bear) };
                cell(xa, ca).text(pos, egui::Align2::CENTER_CENTER, &txt, font.clone(), normal);
                if fr > 0.2 {
                    let clip = ui.painter_at(bar_rect);
                    clip.text(pos, egui::Align2::CENTER_CENTER, &txt, font.clone(), dark);
                }
            }
        }

        // VOL
        if layout.show_vol && self.volume > 0 {
            let vf = self.volume_fill.max(0.0).min(1.0);
            let vw = vf * cv * 0.8;
            let vol_bar = Rect::from_min_size(
                egui::pos2(xv + 1.0, ry + 1.0),
                egui::vec2(vw, row_h - 2.0),
            );
            painter.rect_filled(vol_bar, radius_xs(), color_alpha(dim, alpha_subtle()));
            let vs = if self.volume >= 1_000_000 { format!("{:.1}M", self.volume as f64 / 1e6) }
                else if self.volume >= 1_000 { format!("{:.0}K", self.volume as f64 / 1e3) }
                else { format!("{}", self.volume) };
            let pos = egui::pos2(xv + cv * 0.5, cy);
            cell(xv, cv).text(pos, egui::Align2::CENTER_CENTER, &vs, font_sm.clone(), color_half(dim));
            if vf > 0.3 {
                let clip = ui.painter_at(vol_bar);
                clip.text(pos, egui::Align2::CENTER_CENTER, &vs, font_sm.clone(), dark);
            }
        }

        // ORDERS — rich draggable badges
        for &(oid, side_ch, qty, oc) in self.rich_orders.iter() {
            let br = Rect::from_min_size(
                egui::pos2(xo + 1.0, ry + 1.0),
                egui::vec2(co - 3.0, row_h - 2.0),
            );
            let drag_id = ui.id().with(("dom_row_chip", oid));
            let drag_resp = ui.interact(br, drag_id, Sense::click_and_drag());
            if drag_resp.drag_started() { out.order_drag_started = Some(oid); }
            if drag_resp.dragged() { out.order_dragging = Some((oid, drag_resp.drag_delta().y)); }
            if drag_resp.drag_stopped() { out.order_drag_stopped = Some(oid); }

            let currently_dragging_this = self.drag_cx.dragging_order_id == Some(oid);
            let alpha_mult = if currently_dragging_this { 0.3 } else { 1.0 };
            painter.rect_filled(br, radius_xs(), color_alpha(oc, (140.0 * alpha_mult) as u8));
            painter.rect_stroke(br, radius_xs(),
                Stroke::new(stroke_thin(), color_alpha(oc, (180.0 * alpha_mult) as u8)),
                StrokeKind::Outside);

            let ord_hovered = drag_resp.hovered() && !currently_dragging_this;
            if ord_hovered {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                let xr = Rect::from_min_size(egui::pos2(br.right() - 12.0, br.top()), egui::vec2(12.0, br.height()));
                painter.rect_filled(xr, radius_xs(), color_alpha(bear, alpha_dim()));
                painter.text(xr.center(), egui::Align2::CENTER_CENTER, "x", font_sm.clone(), contrast_fg(bear));
                let label_rect = Rect::from_min_max(br.min, egui::pos2(br.right() - 12.0, br.max.y));
                draw_order_chip_label(painter, label_rect, side_ch, qty, theme_ref.overlay_text);
                if drag_resp.clicked() {
                    let ptr = ui.input(|i| i.pointer.hover_pos()).unwrap_or_default();
                    if ptr.x > br.right() - 14.0 { out.order_cancel = Some(oid); }
                }
            } else if !currently_dragging_this {
                draw_order_chip_label(painter, br, side_ch, qty, theme_ref.overlay_text);
            }
        }

        // Cross-row drag ghost
        if self.drag_cx.is_drop_target {
            if let Some(side_ch) = self.drag_cx.ghost_side {
                let oc = self.drag_cx.ghost_color;
                let gr = Rect::from_min_size(
                    egui::pos2(xo + 1.0, ry + 1.0),
                    egui::vec2(co - 3.0, row_h - 2.0),
                );
                painter.rect_filled(gr, radius_xs(), color_alpha(oc, crate::ui_kit::style::alpha_dense()));
                painter.rect_stroke(gr, radius_xs(), Stroke::new(stroke_bold(), oc), StrokeKind::Outside);
                draw_order_chip_label(painter, gr, side_ch, self.drag_cx.ghost_qty, theme_ref.overlay_text);
                painter.rect_stroke(rr, 0.0,
                    Stroke::new(stroke_std(), color_alpha(oc, alpha_dim())),
                    StrokeKind::Outside);
            }
        }

        // Bottom hairline separator
        painter.line_segment(
            [egui::pos2(rr.min.x, ry + row_h), egui::pos2(rr.max.x, ry + row_h)],
            Stroke::new(stroke_hair(), color_alpha(theme_ref.toolbar_border, alpha_subtle())),
        );

        out
    }
}

fn draw_order_chip_label(painter: &Painter, rect: Rect, side: char, qty: u32, text_col: Color32) {
    let qty_str = format!("{}", qty);
    let side_font = mono_sm();
    let qty_font = mono_sm();
    // high-contrast label on colored chip (caller passes theme.overlay_text)
    let s = side.to_string();
    painter.text(egui::pos2(rect.left() + 8.0, rect.center().y), egui::Align2::CENTER_CENTER, &s, side_font, text_col);
    painter.text(egui::pos2(rect.left() + 8.0 + (rect.width() - 8.0) * 0.5, rect.center().y), egui::Align2::CENTER_CENTER, &qty_str, qty_font, text_col);
}

#[derive(Default)]
struct ZoneInfo {
    bid_clicked: bool,
    ask_clicked: bool,
    price_clicked: bool,
    order_clicked: Option<u32>,
    order_drag_started: Option<u32>,
}

fn fmt_size(size: u32) -> String {
    if size >= 10_000 { format!("{:.1}K", size as f64 / 1_000.0) } else { format!("{}", size) }
}

/// Gutter kept clear on each side of a ladder cell, so adjacent columns have
/// visible separation even when both are full.
const CELL_GUTTER: f32 = 2.0;

/// Progressively shorter size forms, widest first. The last entry is the
/// smallest representation we are willing to show.
fn size_forms(size: u32) -> [String; 3] {
    [
        fmt_size(size),
        if size >= 1_000 { format!("{:.1}K", size as f64 / 1_000.0) } else { format!("{size}") },
        if size >= 1_000 { format!("{}K", size / 1_000) } else { format!("{size}") },
    ]
}

/// Signed delta in progressively shorter forms, widest first.
///
/// The sign is never dropped — it is the single most important character in
/// the string, and losing it inverts the reading.
fn delta_forms(d: i64) -> [String; 3] {
    let sign = if d > 0 { "+" } else { "-" };
    let a = d.unsigned_abs();
    [
        format!("{sign}{a}"),
        if a >= 1_000 { format!("{sign}{:.1}K", a as f64 / 1_000.0) } else { format!("{sign}{a}") },
        if a >= 1_000 { format!("{sign}{}K", a / 1_000) } else { format!("{sign}{a}") },
    ]
}

/// Pick the widest form of a numeric cell that fits `avail`, falling back to
/// the abbreviated form.
///
/// Ladder cells are painted CENTER_CENTER on their column midpoint with no
/// clip, so a string wider than its column used to spill symmetrically into
/// BOTH neighbours. At the default 220px sidebar the Δ column is ~19px wide
/// while `-1437` measures ~30px, so it overhung ~5px each side: the digits
/// collided with BID (`-595703` for Δ=-595, BID=703) and — far worse — the
/// leading `-` fell off the panel's left edge entirely, turning `-1437` into
/// `1437`. **A depth ladder that silently drops the sign off a delta shows
/// the trader the wrong direction**, on exactly the large prints that matter
/// most.
///
/// Abbreviating loses precision; clipping loses the sign. Precision is the
/// cheaper loss — so try progressively shorter forms and only fall back to the
/// narrowest if none fit.
///
/// A single fallback was not enough: at the default width, BID's abbreviated
/// `1.8K` was still wider than its column in the row's larger mono tier, so
/// the "fix" produced centre-clipped `L.8H` — worse than the overlap it
/// replaced. Hence a ladder, ending in a form (`2K`) that fits anything.
fn fit_cell(ui: &Ui, forms: &[String], font: &egui::FontId, avail: f32) -> String {
    // Measured through the canonical helper, which exists precisely so
    // measurement sites do not each invent a throwaway colour — indistinguishable
    // from real off-token drift to the design-system ratchet.
    let width_of = |s: &String| {
        crate::ui_kit::style::measure_with(ui, s, font.clone()).x
    };
    for f in forms {
        if width_of(f) <= avail {
            return f.clone();
        }
    }
    forms.last().cloned().unwrap_or_default()
}

#[cfg(test)]
mod ladder_cell_tests {
    use super::*;

    /// EVERY form of a delta must keep its sign — including the narrowest.
    ///
    /// This is the invariant behind the worst defect the visual audit found:
    /// the Δ column was too narrow for a signed 4-digit value, cells painted
    /// centred with no clip, and the leading `-` fell off the panel's left
    /// edge. `-1437` rendered as `1437` — the same number, the opposite
    /// meaning, on exactly the large prints a trader most needs to read.
    ///
    /// `fit_cell` may pick any form depending on the column width, so it is
    /// not enough for the full form to be correct. Every rung of the ladder
    /// has to be safe.
    #[test]
    fn every_delta_form_keeps_its_sign() {
        for d in [-1_i64, -48, -282, -818, -1437, -1_698, -25_000, -1_000_000,
                  1, 48, 282, 818, 1437, 1_698, 25_000, 1_000_000] {
            let want = if d > 0 { '+' } else { '-' };
            for (i, f) in delta_forms(d).iter().enumerate() {
                assert_eq!(
                    f.chars().next(), Some(want),
                    "delta_forms({d})[{i}] = {f:?} lost its sign — a ladder that \
                     drops the sign shows the trader the wrong direction"
                );
            }
        }
    }

    /// Forms must be ordered widest-first, since `fit_cell` returns the first
    /// that fits and would otherwise abbreviate when it did not need to.
    #[test]
    fn forms_are_ordered_widest_first() {
        for d in [-1_437_i64, 1_698, -25_000] {
            let f = delta_forms(d);
            assert!(f[0].len() >= f[1].len() && f[1].len() >= f[2].len(),
                "delta_forms({d}) not widest-first: {f:?}");
        }
        for s in [488_u32, 1_836, 25_000, 1_000_000] {
            let f = size_forms(s);
            assert!(f[0].len() >= f[1].len() && f[1].len() >= f[2].len(),
                "size_forms({s}) not widest-first: {f:?}");
        }
    }

    /// The narrowest form must actually be narrow — otherwise `fit_cell`'s
    /// last resort still overflows and we are back to clipping.
    #[test]
    fn narrowest_form_is_short() {
        for s in [1_836_u32, 25_000, 1_000_000] {
            assert!(size_forms(s)[2].len() <= 5, "size_forms({s}) too wide: {:?}", size_forms(s));
        }
        for d in [-1_437_i64, 25_000, -1_000_000] {
            assert!(delta_forms(d)[2].len() <= 6, "delta_forms({d}) too wide: {:?}", delta_forms(d));
        }
    }
}
