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
//! NOTE on dom_panel.rs migration: the live ladder in `dom_panel.rs`
//! allocates rects against a parent painter (`ui.painter_at(dom_rect)`)
//! and shares cross-row drag state (ghost preview on drop-target row).
//! Migrating that body to this widget would require funneling the parent
//! painter and drag-context through, which would break visual parity.
//! This widget is therefore "ladder-row capable" but not yet wired into
//! `dom_panel.rs::draw`. It can be used standalone (e.g. in a docked DOM
//! widget that lays out one row per egui call).

#![allow(dead_code, unused_imports)]

use egui::{Color32, Painter, Rect, Response, Sense, Stroke, StrokeKind, Ui};
use super::super::super::style::*;
use super::super::foundation::{
    interaction::InteractionState,
    shell::RowShell,
    tokens::Size,
    variants::RowVariant,
};

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

fn fallback_theme() -> &'static Theme {
    &crate::chart_renderer::gpu::THEMES[0]
}

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

impl<'a> DomRow<'a> {
    pub fn new(price: f32, bid_size: u32, ask_size: u32) -> Self {
        Self {
            price, bid_size, ask_size,
            bid_fill: 0.0, ask_fill: 0.0,
            volume: 0, volume_fill: 0.0, delta: 0,
            is_inside: false, selected: false, current_price: false,
            imbalance: 0.0,
            height: 18.0, price_fmt: "{:.2}",
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
        let theme_ref: &Theme = match self.theme { Some(t) => t, None => fallback_theme() };
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

                let f_lg = egui::FontId::monospace(12.5);
                let f_sm = egui::FontId::monospace(9.0);
                let dark = theme_ref.overlay_text;
                let cy = rect.center().y;

                // ── Δ column ──
                if let Some(dr) = find(DomColumn::Delta) {
                    if delta != 0 {
                        let dc = if delta > 0 { bull.gamma_multiply(0.6) } else { bear.gamma_multiply(0.6) };
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
                        painter.rect_filled(bar, 1.5, color_alpha(bull, (60.0 + bid_fill * 140.0) as u8));
                        bar_rect_opt = Some(bar);
                    }
                    if show_numbers && bid_size > 0 {
                        let txt = fmt_size(bid_size);
                        let pos = br.center();
                        let normal = bull.gamma_multiply(0.7);
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
                    let pc = if is_current { Color32::WHITE }
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
                        painter.rect_filled(bar, 1.5, color_alpha(bear, (60.0 + ask_fill * 140.0) as u8));
                        bar_rect_opt = Some(bar);
                    }
                    if show_numbers && ask_size > 0 {
                        let txt = fmt_size(ask_size);
                        let pos = ar.center();
                        let normal = bear.gamma_multiply(0.7);
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
                        painter.rect_filled(bar, 1.0, color_alpha(dim, alpha_subtle()));
                        let s = if volume >= 1_000_000 { format!("{:.1}M", volume as f64/1e6) }
                            else if volume >= 1_000 { format!("{:.0}K", volume as f64/1e3) }
                            else { format!("{}", volume) };
                        let pos = vr.center();
                        painter.text(pos, egui::Align2::CENTER_CENTER, &s, f_sm.clone(), dim.gamma_multiply(0.5));
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
                            painter.rect_filled(cr, 2.0, color_alpha(ord.color, 140));
                            painter.rect_stroke(cr, 2.0,
                                Stroke::new(stroke_thin(), color_alpha(ord.color, 180)),
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
        let theme_ref: &Theme = match self.theme { Some(t) => t, None => fallback_theme() };
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

        let font = egui::FontId::monospace(12.5);
        let font_sm = egui::FontId::monospace(9.0);
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

        // Δ
        if layout.show_delta && self.delta != 0 {
            let dc = if self.delta > 0 { bull.gamma_multiply(0.6) } else { bear.gamma_multiply(0.6) };
            let s = if self.delta > 0 { format!("+{}", self.delta) } else { format!("{}", self.delta) };
            painter.text(egui::pos2(x0 + cd * 0.5, cy), egui::Align2::CENTER_CENTER, &s, font_sm.clone(), dc);
        }

        // BID
        if self.bid_size > 0 {
            let fr = self.bid_fill.max(0.0).min(1.0);
            let bw = fr * cb * 0.85;
            let bar_rect = Rect::from_min_size(
                egui::pos2(xb + cb - bw - 1.0, ry + 1.0),
                egui::vec2(bw, row_h - 2.0),
            );
            painter.rect_filled(bar_rect, 1.5, color_alpha(bull, (60.0 + fr * 140.0) as u8));
            if layout.show_numbers {
                let txt = fmt_size(self.bid_size);
                let pos = egui::pos2(xb + cb * 0.5, cy);
                let normal = if hv { bull } else { bull.gamma_multiply(0.7) };
                painter.text(pos, egui::Align2::CENTER_CENTER, &txt, font.clone(), normal);
                if fr > 0.2 {
                    let clip = ui.painter_at(bar_rect);
                    clip.text(pos, egui::Align2::CENTER_CENTER, &txt, font.clone(), dark);
                }
            }
        }

        // PRICE — compact 5-char formatting when narrow
        let pc = self.price_color_override.unwrap_or_else(|| {
            if self.current_price { Color32::WHITE }
            else if self.selected { accent }
            else if self.imbalance > 0.0 { bull.gamma_multiply(0.7) }
            else { bear.gamma_multiply(0.7) }
        });
        let _ = fg;
        let ps = if self.compact_price {
            let s = format!("{:.2}", self.price);
            if s.len() > 5 && cp < 60.0 { format!("{:.1}", self.price) } else { s }
        } else if self.price >= 1.0 && (self.price.fract() == 0.0) {
            format!("{:.0}", self.price)
        } else { format!("{:.2}", self.price) };
        let price_font = if self.current_price { egui::FontId::monospace(font_md()) } else { font.clone() };
        painter.text(egui::pos2(xp + cp * 0.5, cy), egui::Align2::CENTER_CENTER, &ps, price_font, pc);

        // ASK
        if self.ask_size > 0 {
            let fr = self.ask_fill.max(0.0).min(1.0);
            let bw = fr * ca * 0.85;
            let bar_rect = Rect::from_min_size(
                egui::pos2(xa + 1.0, ry + 1.0),
                egui::vec2(bw, row_h - 2.0),
            );
            painter.rect_filled(bar_rect, 1.5, color_alpha(bear, (60.0 + fr * 140.0) as u8));
            if layout.show_numbers {
                let txt = fmt_size(self.ask_size);
                let pos = egui::pos2(xa + ca * 0.5, cy);
                let normal = if hv { bear } else { bear.gamma_multiply(0.7) };
                painter.text(pos, egui::Align2::CENTER_CENTER, &txt, font.clone(), normal);
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
            painter.rect_filled(vol_bar, 1.0, color_alpha(dim, alpha_subtle()));
            let vs = if self.volume >= 1_000_000 { format!("{:.1}M", self.volume as f64 / 1e6) }
                else if self.volume >= 1_000 { format!("{:.0}K", self.volume as f64 / 1e3) }
                else { format!("{}", self.volume) };
            let pos = egui::pos2(xv + cv * 0.5, cy);
            painter.text(pos, egui::Align2::CENTER_CENTER, &vs, font_sm.clone(), dim.gamma_multiply(0.5));
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
            painter.rect_filled(br, 2.0, color_alpha(oc, (140.0 * alpha_mult) as u8));
            painter.rect_stroke(br, 2.0,
                Stroke::new(stroke_thin(), color_alpha(oc, (180.0 * alpha_mult) as u8)),
                StrokeKind::Outside);

            let ord_hovered = drag_resp.hovered() && !currently_dragging_this;
            if ord_hovered {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                let xr = Rect::from_min_size(egui::pos2(br.right() - 12.0, br.top()), egui::vec2(12.0, br.height()));
                painter.rect_filled(xr, 1.0, color_alpha(bear, alpha_dim()));
                painter.text(xr.center(), egui::Align2::CENTER_CENTER, "x", egui::FontId::monospace(7.0), Color32::WHITE);
                let label_rect = Rect::from_min_max(br.min, egui::pos2(br.right() - 12.0, br.max.y));
                draw_order_chip_label(painter, label_rect, side_ch, qty);
                if drag_resp.clicked() {
                    let ptr = ui.input(|i| i.pointer.hover_pos()).unwrap_or_default();
                    if ptr.x > br.right() - 14.0 { out.order_cancel = Some(oid); }
                }
            } else if !currently_dragging_this {
                draw_order_chip_label(painter, br, side_ch, qty);
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
                painter.rect_filled(gr, 2.0, color_alpha(oc, 160));
                painter.rect_stroke(gr, 2.0, Stroke::new(stroke_bold(), oc), StrokeKind::Outside);
                draw_order_chip_label(painter, gr, side_ch, self.drag_cx.ghost_qty);
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

fn draw_order_chip_label(painter: &Painter, rect: Rect, side: char, qty: u32) {
    let qty_str = format!("{}", qty);
    let side_font = egui::FontId::monospace(9.0);
    let qty_font = egui::FontId::monospace(12.0);
    let text_col = fallback_theme().overlay_text; // high-contrast label on colored chip
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
