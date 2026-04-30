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

use egui::{Color32, Rect, Response, Sense, Stroke, StrokeKind, Ui};
use super::super::super::style::*;
use super::super::foundation::{
    interaction::InteractionState,
    shell::RowShell,
    tokens::Size,
    variants::RowVariant,
};

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
        }
    }
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
                let dark = Color32::from_rgb(12, 14, 18);
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
                                &label, f_sm.clone(), Color32::from_rgb(10, 12, 16));

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
