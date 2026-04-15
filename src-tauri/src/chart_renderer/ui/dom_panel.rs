//! DOM (Depth of Market) full sidebar panel — price ladder with bid/ask depth,
//! volume, delta, imbalance highlighting, and order management.

use egui;
use super::style::*;
use super::super::gpu::Theme;
use crate::chart_renderer::trading::{OrderLevel, OrderSide, OrderStatus};

pub(crate) const DOM_SIDEBAR_W: f32 = 240.0;
const DOM_MIN_W: f32 = 180.0;
const DOM_MAX_W: f32 = 450.0;
const ROW_H: f32 = 20.0;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DomOrderType { Market, Limit }

#[derive(Clone, Debug)]
pub(crate) struct DomLevel {
    pub(crate) price: f32,
    pub(crate) bid_size: u32,
    pub(crate) ask_size: u32,
    pub(crate) volume: u64,
    pub(crate) delta: i64,
}

pub(crate) fn generate_mock_levels(center_price: f32, tick_size: f32, count: i32) -> Vec<DomLevel> {
    let mut levels = Vec::with_capacity((count * 2 + 1) as usize);
    for row in (-count..=count).rev() {
        let price = center_price + row as f32 * tick_size;
        let dist = row.unsigned_abs();
        let base = 3000u32.saturating_sub(dist * 150).max(100);
        let hash = (price * 1000.0) as u32;
        let h1 = hash.wrapping_mul(2654435761);
        let h2 = hash.wrapping_mul(2246822519);
        let bid = base + (h1 % 2000); let ask = base + (h2 % 2000);
        let vol = (bid as u64 + ask as u64) * 3 + (h1 as u64 % 5000);
        let delta = bid as i64 - ask as i64 + ((h1 % 200) as i64 - 100);
        levels.push(DomLevel { price, bid_size: bid, ask_size: ask, volume: vol, delta });
    }
    levels
}

pub(crate) fn draw(
    ui: &mut egui::Ui, dom_rect: egui::Rect, current_price: f32, levels: &[DomLevel],
    tick_size: f32, center_price: &mut f32, dom_width: &mut f32,
    orders: &[OrderLevel], dom_selected_price: &mut Option<f32>,
    dom_order_type: &mut DomOrderType, order_qty: &mut u32,
    new_order: &mut Option<(OrderSide, f32, u32)>, cancel_all: &mut bool,
    cancel_order_id: &mut Option<u32>, move_order: &mut Option<(u32, f32)>,
    dom_armed: &mut bool, dom_col_mode: &mut u8,
    dom_dragging: &mut Option<(u32, f32)>, // (order_id, current_y) while dragging
    t: &Theme,
) {
    let painter = ui.painter_at(dom_rect);
    painter.rect_filled(dom_rect, 0.0, t.toolbar_bg);
    painter.line_segment([egui::pos2(dom_rect.right(), dom_rect.top()), egui::pos2(dom_rect.right(), dom_rect.bottom())],
        egui::Stroke::new(STROKE_STD, color_alpha(t.toolbar_border, ALPHA_HEAVY)));

    // Resize handle
    let hr = egui::Rect::from_min_size(egui::pos2(dom_rect.right()-3.0, dom_rect.top()), egui::vec2(6.0, dom_rect.height()));
    let hresp = ui.allocate_rect(hr, egui::Sense::drag());
    if hresp.hovered() || hresp.dragged() { ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal); }
    if hresp.dragged() { *dom_width = (*dom_width + hresp.drag_delta().x).clamp(DOM_MIN_W, DOM_MAX_W); }
    if hresp.hovered() { painter.line_segment([egui::pos2(dom_rect.right()-1.0, dom_rect.top()+14.0), egui::pos2(dom_rect.right()-1.0, dom_rect.bottom())], egui::Stroke::new(STROKE_THICK, color_alpha(t.accent, ALPHA_STRONG))); }

    let inner = dom_rect.shrink2(egui::vec2(3.0, 0.0));
    let aw = inner.width();
    let mode = *dom_col_mode; // 0=compact, 1=normal, 2=expanded
    let show_delta = mode >= 1;
    let show_vol = mode >= 1;
    let show_numbers = mode < 2; // expanded mode hides bid/ask numbers

    // Column widths adapt to mode
    let cd = if show_delta { aw * 0.09 } else { 0.0 };
    let co = aw * 0.14;
    let cv = if show_vol { aw * 0.14 } else { 0.0 };
    let remaining = aw - cd - cv - co;
    let cb = remaining * 0.27; let cp = remaining * 0.46; let ca = remaining * 0.27;
    let x0 = inner.left();
    let xb = x0+cd; let xp = xb+cb; let xa = xp+cp; let xv = xa+ca; let xo = xv+cv;

    // Header
    let hy = inner.top()+2.0;
    let hf = egui::FontId::monospace(8.5);
    let hc = t.dim.gamma_multiply(0.45);
    if show_delta { painter.text(egui::pos2(x0+cd*0.5, hy+6.0), egui::Align2::CENTER_CENTER, "\u{0394}", hf.clone(), hc); }
    painter.text(egui::pos2(xb+cb*0.5, hy+6.0), egui::Align2::CENTER_CENTER, "BID", hf.clone(), t.bull.gamma_multiply(0.5));
    // PRICE header — double-click to recenter
    let price_hdr_rect = egui::Rect::from_min_size(egui::pos2(xp, hy), egui::vec2(cp, 14.0));
    let price_hdr_resp = ui.allocate_rect(price_hdr_rect, egui::Sense::click());
    painter.text(egui::pos2(xp+cp*0.5, hy+6.0), egui::Align2::CENTER_CENTER, "PRICE", hf.clone(), hc);
    if price_hdr_resp.double_clicked() {
        *center_price = (current_price / tick_size).round() * tick_size;
    }
    if price_hdr_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    painter.text(egui::pos2(xa+ca*0.5, hy+6.0), egui::Align2::CENTER_CENTER, "ASK", hf.clone(), t.bear.gamma_multiply(0.5));
    if show_vol { painter.text(egui::pos2(xv+cv*0.5, hy+6.0), egui::Align2::CENTER_CENTER, "VOL", hf.clone(), hc); }
    // ORD header + column mode toggle [+/-]
    let ord_label_w = co * 0.5;
    painter.text(egui::pos2(xo+ord_label_w*0.5, hy+6.0), egui::Align2::CENTER_CENTER, "ORD", hf.clone(), hc);
    // [+] button
    let plus_r = egui::Rect::from_min_size(egui::pos2(xo+ord_label_w+1.0, hy+1.0), egui::vec2(10.0, 11.0));
    let plus_resp = ui.allocate_rect(plus_r, egui::Sense::click());
    painter.text(plus_r.center(), egui::Align2::CENTER_CENTER, "+", egui::FontId::monospace(8.0), if plus_resp.hovered() { t.accent } else { hc });
    if plus_resp.clicked() && mode < 2 { *dom_col_mode = mode + 1; }
    if plus_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    // [-] button
    let minus_r = egui::Rect::from_min_size(egui::pos2(plus_r.right()+1.0, hy+1.0), egui::vec2(10.0, 11.0));
    let minus_resp = ui.allocate_rect(minus_r, egui::Sense::click());
    painter.text(minus_r.center(), egui::Align2::CENTER_CENTER, "-", egui::FontId::monospace(8.0), if minus_resp.hovered() { t.accent } else { hc });
    if minus_resp.clicked() && mode > 0 { *dom_col_mode = mode - 1; }
    if minus_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

    let sep_y = hy+14.0;
    painter.line_segment([egui::pos2(inner.left(), sep_y), egui::pos2(inner.right(), sep_y)], egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_STRONG)));

    // ── Bottom controls ──
    let ctrl_h = 58.0;
    let ctrl_top = inner.bottom() - ctrl_h;
    painter.rect_filled(egui::Rect::from_min_max(egui::pos2(dom_rect.left(), ctrl_top), egui::pos2(dom_rect.right(), dom_rect.bottom())), 0.0, t.toolbar_bg);
    // Inset shadow
    for i in 0..4u32 { painter.line_segment([egui::pos2(inner.left(), ctrl_top-i as f32), egui::pos2(inner.right(), ctrl_top-i as f32)], egui::Stroke::new(STROKE_STD, egui::Color32::from_rgba_unmultiplied(0,0,0, 20u8.saturating_sub(i as u8*5)))); }
    painter.line_segment([egui::pos2(inner.left(), ctrl_top), egui::pos2(inner.right(), ctrl_top)], egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_LINE)));

    let fs = egui::FontId::monospace(7.5);
    let fm = egui::FontId::monospace(8.5);
    let is_mkt = *dom_order_type == DomOrderType::Market;

    // Row 1 (16px): [-] qty [+]  [MKT/LMT]  [A]
    //               ← half width →  ← rest →
    let r1y = ctrl_top+4.0; let r1h = 16.0;
    let half_w = aw * 0.48;
    let mut cx = inner.left()+1.0;

    // [-]
    let r = egui::Rect::from_min_size(egui::pos2(cx, r1y), egui::vec2(14.0, r1h));
    let resp = ui.allocate_rect(r, egui::Sense::click());
    painter.rect_filled(r, 2.0, if resp.hovered() { color_alpha(t.toolbar_border, ALPHA_DIM) } else { color_alpha(t.toolbar_border, ALPHA_SOFT) });
    painter.text(r.center(), egui::Align2::CENTER_CENTER, "-", egui::FontId::monospace(11.0), t.dim);
    if resp.clicked() && *order_qty > 1 { *order_qty -= 1; }
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    cx = r.right()+1.0;

    // qty
    let qw = half_w - 30.0;
    let qr = egui::Rect::from_min_size(egui::pos2(cx, r1y), egui::vec2(qw, r1h));
    painter.rect_filled(qr, 0.0, color_alpha(t.bg, 180));
    painter.text(qr.center(), egui::Align2::CENTER_CENTER, &format!("{}", *order_qty), fm.clone(), egui::Color32::from_rgb(220,220,230));
    cx = qr.right()+1.0;

    // [+]
    let r = egui::Rect::from_min_size(egui::pos2(cx, r1y), egui::vec2(14.0, r1h));
    let resp = ui.allocate_rect(r, egui::Sense::click());
    painter.rect_filled(r, 2.0, if resp.hovered() { color_alpha(t.toolbar_border, ALPHA_DIM) } else { color_alpha(t.toolbar_border, ALPHA_SOFT) });
    painter.text(r.center(), egui::Align2::CENTER_CENTER, "+", egui::FontId::monospace(11.0), t.dim);
    if resp.clicked() { *order_qty += 1; }
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    cx = r.right()+4.0;

    // [MKT/LMT] — bigger
    let mw = aw * 0.28;
    let r = egui::Rect::from_min_size(egui::pos2(cx, r1y), egui::vec2(mw, r1h));
    let resp = ui.allocate_rect(r, egui::Sense::click());
    painter.rect_filled(r, 2.0, color_alpha(t.accent, if resp.hovered() { 55 } else { 28 }));
    painter.text(r.center(), egui::Align2::CENTER_CENTER, if is_mkt {"MARKET"} else {"LIMIT"}, fs.clone(), t.accent);
    if resp.clicked() { *dom_order_type = if is_mkt { DomOrderType::Limit } else { DomOrderType::Market }; if !is_mkt { *dom_selected_price = None; } }
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    cx = r.right()+3.0;

    // [A] — armed, small
    let armw = inner.right()-cx-1.0;
    let r = egui::Rect::from_min_size(egui::pos2(cx, r1y), egui::vec2(armw, r1h));
    let resp = ui.allocate_rect(r, egui::Sense::click());
    let ac = if *dom_armed { egui::Color32::from_rgb(230,70,70) } else { t.dim.gamma_multiply(0.4) };
    painter.rect_filled(r, 2.0, if *dom_armed { color_alpha(ac, 35) } else { color_alpha(t.toolbar_border, ALPHA_GHOST) });
    painter.rect_stroke(r, 2.0, egui::Stroke::new(STROKE_THIN, color_alpha(ac, if *dom_armed {90} else {30})), egui::StrokeKind::Outside);
    painter.text(r.center(), egui::Align2::CENTER_CENTER, if *dom_armed {"!"} else {"A"}, fs.clone(), ac);
    if resp.clicked() { *dom_armed = !*dom_armed; }
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

    // Row 2+3 (32px total): [BUY] [FLATTEN/CANCEL stacked] [SELL]
    let r2y = r1y+r1h+3.0;
    let action_h = 32.0;
    let side_w = aw * 0.34;
    let mid_w = aw - side_w*2.0 - 6.0;
    let mid_half_h = action_h * 0.5 - 1.0;

    // BUY (spans full action height)
    let r = egui::Rect::from_min_size(egui::pos2(inner.left()+1.0, r2y), egui::vec2(side_w, action_h));
    let resp = ui.allocate_rect(r, egui::Sense::click());
    painter.rect_filled(r, 3.0, if resp.hovered() { color_alpha(t.bull, 70) } else { color_alpha(t.bull, ALPHA_TINT) });
    painter.rect_stroke(r, 3.0, egui::Stroke::new(STROKE_THIN, color_alpha(t.bull, 90)), egui::StrokeKind::Outside);
    painter.text(r.center(), egui::Align2::CENTER_CENTER, "BUY", fm.clone(), t.bull);
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    if resp.clicked() { let p = if !is_mkt { dom_selected_price.unwrap_or(current_price) } else { current_price }; *new_order = Some((OrderSide::Buy, p, *order_qty)); }

    // Middle: FLATTEN (top) + CANCEL (bottom)
    let mid_x = inner.left()+1.0+side_w+3.0;
    let fc = egui::Color32::from_rgb(200,150,50);

    // FLATTEN
    let r = egui::Rect::from_min_size(egui::pos2(mid_x, r2y), egui::vec2(mid_w, mid_half_h));
    let resp = ui.allocate_rect(r, egui::Sense::click());
    painter.rect_filled(r, 2.0, if resp.hovered() { color_alpha(fc, ALPHA_LINE) } else { color_alpha(fc, 18) });
    painter.rect_stroke(r, 2.0, egui::Stroke::new(STROKE_THIN, color_alpha(fc, ALPHA_LINE)), egui::StrokeKind::Outside);
    painter.text(r.center(), egui::Align2::CENTER_CENTER, "FLATTEN", fs.clone(), if resp.hovered() { fc } else { fc.gamma_multiply(0.6) });
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    if resp.clicked() { *cancel_all = true; }

    // CANCEL
    let r = egui::Rect::from_min_size(egui::pos2(mid_x, r2y+mid_half_h+2.0), egui::vec2(mid_w, mid_half_h));
    let resp = ui.allocate_rect(r, egui::Sense::click());
    painter.rect_filled(r, 2.0, if resp.hovered() { color_alpha(t.dim, ALPHA_MUTED) } else { color_alpha(t.toolbar_border, ALPHA_SOFT) });
    painter.rect_stroke(r, 2.0, egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_LINE)), egui::StrokeKind::Outside);
    painter.text(r.center(), egui::Align2::CENTER_CENTER, "CANCEL", fs.clone(), if resp.hovered() { t.dim } else { t.dim.gamma_multiply(0.5) });
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    if resp.clicked() { *cancel_all = true; }

    // SELL (spans full action height)
    let r = egui::Rect::from_min_size(egui::pos2(mid_x+mid_w+3.0, r2y), egui::vec2(side_w, action_h));
    let resp = ui.allocate_rect(r, egui::Sense::click());
    painter.rect_filled(r, 3.0, if resp.hovered() { color_alpha(t.bear, 70) } else { color_alpha(t.bear, ALPHA_TINT) });
    painter.rect_stroke(r, 3.0, egui::Stroke::new(STROKE_THIN, color_alpha(t.bear, 90)), egui::StrokeKind::Outside);
    painter.text(r.center(), egui::Align2::CENTER_CENTER, "SELL", fm.clone(), t.bear);
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    if resp.clicked() { let p = if !is_mkt { dom_selected_price.unwrap_or(current_price) } else { current_price }; *new_order = Some((OrderSide::Sell, p, *order_qty)); }

    // ── Price ladder ──
    let body_top = sep_y+2.0;
    let body_h = (ctrl_top - body_top - 2.0).max(60.0);
    let max_rows = (body_h / ROW_H) as i32;
    let half = max_rows / 2;

    let pil = ui.input(|i| i.pointer.hover_pos()).map_or(false, |p| p.x >= dom_rect.left() && p.x <= dom_rect.right() && p.y >= body_top && p.y <= ctrl_top);
    if pil { let s = ui.input(|i| i.raw_scroll_delta.y); if s.abs() > 0.5 { *center_price += if s > 0.0 { tick_size } else { -tick_size }; } }

    let sc = (*center_price / tick_size).round() * tick_size;
    let mb = levels.iter().map(|l| l.bid_size).max().unwrap_or(1).max(1);
    let ma = levels.iter().map(|l| l.ask_size).max().unwrap_or(1).max(1);
    let ms = mb.max(ma) as f32;
    let mv = levels.iter().map(|l| l.volume).max().unwrap_or(1).max(1);
    let ao: Vec<&OrderLevel> = orders.iter().filter(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed).collect();
    let font = egui::FontId::monospace(12.5);
    let font_sm = egui::FontId::monospace(10.0);
    let lp = ui.painter_at(egui::Rect::from_min_max(egui::pos2(dom_rect.left(), body_top), egui::pos2(dom_rect.right(), body_top+body_h)));

    for ri in (-half..=half).rev() {
        let price = sc + ri as f32 * tick_size * -1.0;
        let rit = half - ri;
        let ry = body_top + rit as f32 * ROW_H;
        if ry+ROW_H < body_top || ry > body_top+body_h { continue; }
        let rr = egui::Rect::from_min_size(egui::pos2(inner.left(), ry), egui::vec2(aw, ROW_H));
        let lv = levels.iter().find(|l| (l.price-price).abs() < tick_size*0.5);
        let (bs, ask, vol, delta) = lv.map_or((0,0,0u64,0i64), |l| (l.bid_size, l.ask_size, l.volume, l.delta));
        let ic = (price-current_price).abs() < tick_size*0.5;
        let ia = price > current_price + tick_size*0.5;
        let is = dom_selected_price.map_or(false, |sp| (sp-price).abs() < tick_size*0.5);
        let oap: Vec<&&OrderLevel> = ao.iter().filter(|o| (o.price-price).abs() < tick_size*0.5).collect();

        let rc = ui.allocate_rect(rr, egui::Sense::click());
        let hv = rc.hovered();
        if hv { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
        if rc.clicked() { *dom_selected_price = Some(price); *dom_order_type = DomOrderType::Limit; }

        // Row backgrounds
        if is { lp.rect_filled(rr, 0.0, color_alpha(t.accent, ALPHA_TINT)); lp.line_segment([egui::pos2(inner.left(), ry), egui::pos2(inner.left(), ry+ROW_H)], egui::Stroke::new(STROKE_THICK, t.accent)); }
        else if ic {
            // Current price: highlighted with accent border
            lp.rect_filled(rr, 0.0, color_alpha(t.accent, 35));
            lp.rect_stroke(rr, 0.0, egui::Stroke::new(STROKE_STD, color_alpha(t.accent, ALPHA_ACTIVE)), egui::StrokeKind::Outside);
        }
        else if hv { lp.rect_filled(rr, 0.0, color_alpha(t.toolbar_border, ALPHA_SUBTLE)); }
        if bs > 0 && ask > 0 { let r = bs as f32/ask as f32; if r > 3.0 { lp.rect_filled(rr, 0.0, color_alpha(t.bull, ALPHA_GHOST)); } else if r < 0.33 { lp.rect_filled(rr, 0.0, color_alpha(t.bear, ALPHA_GHOST)); } }

        let cy = ry + ROW_H*0.5;
        let dark = egui::Color32::from_rgb(12, 14, 18);

        // Delta
        if show_delta && delta != 0 {
            let dc = if delta > 0 { t.bull.gamma_multiply(0.6) } else { t.bear.gamma_multiply(0.6) };
            let ds = if delta > 0 { format!("+{}", delta) } else { format!("{}", delta) };
            lp.text(egui::pos2(x0+cd*0.5, cy), egui::Align2::CENTER_CENTER, &ds, font_sm.clone(), dc);
        }

        // Bid bar + split-color text
        if bs > 0 {
            let fr = bs as f32/ms; let bw = fr*cb*0.85;
            let bar_rect = egui::Rect::from_min_size(egui::pos2(xb+cb-bw-1.0, ry+2.0), egui::vec2(bw, ROW_H-4.0));
            lp.rect_filled(bar_rect, 1.5, color_alpha(t.bull, (60.0+fr*140.0) as u8));
            if show_numbers {
                let txt = fmt_size(bs);
                let txt_pos = egui::pos2(xb+cb*0.5, cy);
                let normal_c = if hv { t.bull } else { t.bull.gamma_multiply(0.7) };
                // Paint normal color first, then dark on top clipped to bar
                lp.text(txt_pos, egui::Align2::CENTER_CENTER, &txt, font.clone(), normal_c);
                if fr > 0.2 {
                    let bar_clip = ui.painter_at(bar_rect);
                    bar_clip.text(txt_pos, egui::Align2::CENTER_CENTER, &txt, font.clone(), dark);
                }
            }
        }

        // Price (max 5 chars when narrow)
        let pc = if ic { egui::Color32::WHITE } else if is { t.accent } else if ia { t.bull.gamma_multiply(0.7) } else { t.bear.gamma_multiply(0.7) };
        let ps = if tick_size >= 1.0 { format!("{:.0}", price) }
            else { let s = format!("{:.2}", price); if s.len() > 5 && cp < 60.0 { format!("{:.1}", price) } else { s } };
        lp.text(egui::pos2(xp+cp*0.5, cy), egui::Align2::CENTER_CENTER, &ps, if ic { egui::FontId::monospace(13.0) } else { font.clone() }, pc);

        // Ask bar + split-color text
        if ask > 0 {
            let fr = ask as f32/ms; let bw = fr*ca*0.85;
            let bar_rect = egui::Rect::from_min_size(egui::pos2(xa+1.0, ry+2.0), egui::vec2(bw, ROW_H-4.0));
            lp.rect_filled(bar_rect, 1.5, color_alpha(t.bear, (60.0+fr*140.0) as u8));
            if show_numbers {
                let txt = fmt_size(ask);
                let txt_pos = egui::pos2(xa+ca*0.5, cy);
                let normal_c = if hv { t.bear } else { t.bear.gamma_multiply(0.7) };
                lp.text(txt_pos, egui::Align2::CENTER_CENTER, &txt, font.clone(), normal_c);
                if fr > 0.2 {
                    let bar_clip = ui.painter_at(bar_rect);
                    bar_clip.text(txt_pos, egui::Align2::CENTER_CENTER, &txt, font.clone(), dark);
                }
            }
        }

        // Volume (with split-color)
        if show_vol && vol > 0 {
            let vf = vol as f32/mv as f32; let vw = vf*cv*0.8;
            let vol_bar = egui::Rect::from_min_size(egui::pos2(xv+1.0, ry+2.5), egui::vec2(vw, ROW_H-5.0));
            lp.rect_filled(vol_bar, 1.0, color_alpha(t.dim, ALPHA_SUBTLE));
            let vs = if vol >= 1_000_000 { format!("{:.1}M", vol as f64/1e6) } else if vol >= 1_000 { format!("{:.0}K", vol as f64/1e3) } else { format!("{}", vol) };
            let txt_pos = egui::pos2(xv+cv*0.5, cy);
            lp.text(txt_pos, egui::Align2::CENTER_CENTER, &vs, font_sm.clone(), t.dim.gamma_multiply(0.5));
            if vf > 0.3 {
                let bar_clip = ui.painter_at(vol_bar);
                bar_clip.text(txt_pos, egui::Align2::CENTER_CENTER, &vs, font_sm.clone(), dark);
            }
        }

        // Orders — compact "B100" or "S50", draggable, X cancel on hover
        let is_being_dragged_here = dom_dragging.map_or(false, |(_, dy)| {
            let drag_price_row = ((dy - body_top) / ROW_H).round() as i32;
            let this_row = rit;
            drag_price_row == this_row
        });
        for ord in &oap {
            let oc = ord.color(t.bull, t.bear);
            let side_ch = match ord.side { OrderSide::Buy | OrderSide::TriggerBuy => "B", _ => "S" };
            let label = format!("{}{}", side_ch, ord.qty);
            let br = egui::Rect::from_min_size(egui::pos2(xo+1.0, ry+2.0), egui::vec2(co-3.0, ROW_H-4.0));

            // Draggable badge
            let drag_resp = ui.allocate_rect(br, egui::Sense::click_and_drag());
            if drag_resp.drag_started() {
                *dom_dragging = Some((ord.id, ry));
            }
            if drag_resp.dragged() {
                if let Some((did, ref mut dy)) = dom_dragging {
                    if *did == ord.id {
                        *dy += drag_resp.drag_delta().y;
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    }
                }
            }
            if drag_resp.drag_stopped() {
                if let Some((did, dy)) = *dom_dragging {
                    if did == ord.id {
                        // Calculate target price from drop y position
                        let drop_row = ((dy - body_top) / ROW_H).round() as i32;
                        let target_ri = half - drop_row;
                        let target_price = sc + target_ri as f32 * tick_size * -1.0;
                        if (target_price - ord.price).abs() > tick_size * 0.1 {
                            *move_order = Some((ord.id, target_price));
                        }
                    }
                }
                *dom_dragging = None;
            }

            let currently_dragging_this = dom_dragging.map_or(false, |(did, _)| did == ord.id);

            // Draw badge (dim if being dragged away)
            let alpha_mult = if currently_dragging_this { 0.3 } else { 1.0 };
            lp.rect_filled(br, 2.0, color_alpha(oc, (140.0 * alpha_mult) as u8));
            lp.rect_stroke(br, 2.0, egui::Stroke::new(STROKE_THIN, color_alpha(oc, (180.0 * alpha_mult) as u8)), egui::StrokeKind::Outside);

            let ord_hovered = drag_resp.hovered() && !currently_dragging_this;
            if ord_hovered {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                // X cancel on right
                let xr = egui::Rect::from_min_size(egui::pos2(br.right()-12.0, br.top()), egui::vec2(12.0, br.height()));
                lp.rect_filled(xr, 1.0, color_alpha(t.bear, ALPHA_DIM));
                lp.text(xr.center(), egui::Align2::CENTER_CENTER, "x", egui::FontId::monospace(7.0), egui::Color32::WHITE);
                let label_rect = egui::Rect::from_min_max(br.min, egui::pos2(br.right()-12.0, br.max.y));
                draw_order_label(&lp, label_rect, side_ch, ord.qty, oc);
                // Cancel on click (not drag)
                if drag_resp.clicked() {
                    let ptr = ui.input(|i| i.pointer.hover_pos()).unwrap_or_default();
                    if ptr.x > br.right() - 14.0 { *cancel_order_id = Some(ord.id); }
                }
            } else if !currently_dragging_this {
                draw_order_label(&lp, br, side_ch, ord.qty, oc);
            }
        }

        // Draw ghost badge if an order is being dragged to this row
        if is_being_dragged_here {
            if let Some((did, _)) = *dom_dragging {
                if let Some(drag_ord) = ao.iter().find(|o| o.id == did) {
                    let oc = drag_ord.color(t.bull, t.bear);
                    let side_ch = match drag_ord.side { OrderSide::Buy | OrderSide::TriggerBuy => "B", _ => "S" };
                    let gr = egui::Rect::from_min_size(egui::pos2(xo+1.0, ry+2.0), egui::vec2(co-3.0, ROW_H-4.0));
                    lp.rect_filled(gr, 2.0, color_alpha(oc, 160));
                    lp.rect_stroke(gr, 2.0, egui::Stroke::new(STROKE_BOLD, oc), egui::StrokeKind::Outside);
                    draw_order_label(&lp, gr, side_ch, drag_ord.qty, oc);
                    // Also highlight the full row
                    lp.rect_stroke(rr, 0.0, egui::Stroke::new(STROKE_STD, color_alpha(oc, ALPHA_DIM)), egui::StrokeKind::Outside);
                }
            }
        }

        lp.line_segment([egui::pos2(inner.left(), ry+ROW_H), egui::pos2(inner.right(), ry+ROW_H)], egui::Stroke::new(STROKE_HAIR, color_alpha(t.toolbar_border, ALPHA_SUBTLE)));
    }
}

fn fmt_size(size: u32) -> String {
    if size >= 10_000 { format!("{:.1}K", size as f64 / 1_000.0) } else { format!("{}", size) }
}

/// Draw order badge text: side letter + large bold qty, high contrast against badge bg
fn draw_order_label(painter: &egui::Painter, rect: egui::Rect, side: &str, qty: u32, _color: egui::Color32) {
    let qty_str = format!("{}", qty);
    let side_font = egui::FontId::monospace(9.0);
    let qty_font = egui::FontId::monospace(12.0);
    let text_col = egui::Color32::from_rgb(10, 12, 16); // dark, high contrast against colored badge
    // Side letter on the left
    painter.text(egui::pos2(rect.left() + 8.0, rect.center().y), egui::Align2::CENTER_CENTER, side, side_font, text_col);
    // Qty number, large and bold, centered in remaining space
    painter.text(egui::pos2(rect.left() + 8.0 + (rect.width() - 8.0) * 0.5, rect.center().y), egui::Align2::CENTER_CENTER, &qty_str, qty_font, text_col);
}
