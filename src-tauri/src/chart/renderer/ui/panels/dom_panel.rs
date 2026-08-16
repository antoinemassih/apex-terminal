//! DOM (Depth of Market) full sidebar panel — price ladder with bid/ask depth,
//! volume, delta, imbalance highlighting, and order management.

use egui;
use crate::ui_kit::sx::Tone;
use super::super::style::*;
use super::super::super::gpu::Theme;
use super::super::widgets::rows::dom_row::{ColumnLayout, DomRow, DomRowDragCx};
use crate::ui_kit::widgets::{Button, Select};
use crate::ui_kit::widgets::tokens::{Size as KitSize, Variant};
use crate::ui_kit::icons::Icon;
use crate::ui_kit::layout::{Flex, Item};
use crate::chart_renderer::trading::{OrderLevel, OrderSide, OrderStatus};
use crate::chart_renderer::ui::foundation::text_style::TextStyle;

/// Add a design-system widget at an absolute pixel rect inside the DOM panel.
/// The DOM panel uses hand-positioned rects (not flowed egui layouts), so we
/// host each design widget in its own pinned `Ui`.
fn place_at<R>(ui: &mut egui::Ui, rect: egui::Rect, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.set_min_size(rect.size());
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);
        add(ui)
    }).inner
}

pub(crate) const DOM_SIDEBAR_W: f32 = 220.0;
const DOM_MIN_W: f32 = 180.0;
const DOM_MAX_W: f32 = 450.0;
const ROW_H: f32 = 18.0;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DomOrderType { Market, Limit }

#[derive(Clone, Debug)]
pub struct DomLevel {
    pub price: f32,
    pub bid_size: u32,
    pub ask_size: u32,
    pub volume: u64,
    pub delta: i64,
    /// DOM Phase 2 order-flow flags, computed frame-to-frame in the DomLevels
    /// handler (gpu.rs): `absorbed` = heavy executed volume while the resting
    /// size held (a level soaking up flow — strength); `pulled` = resting size
    /// vanished without trades to explain it (liquidity yanked — weakness).
    pub absorbed: bool,
    pub pulled: bool,
    /// DOM Phase 2: a single outlier print (>= a self-calibrating multiple of the
    /// window's average trade size) executed at this price since the last frame —
    /// an institutional block / sweep worth flagging on the ladder.
    pub big_print: bool,
    /// DOM Phase 2: aggressive volume split at this price from the tape —
    /// `buy_vol` = size lifting the offer, `sell_vol` = size hitting the bid
    /// (Lee-Ready aggressor side). Rendered as a two-tone split bar.
    pub buy_vol: u64,
    pub sell_vol: u64,
}

// TODO(real-dom): replace with a live L2/depth-of-market feed.
// Requirements: a streaming order-book source (e.g. an ApexData WS `l2_snapshot` +
// `l2_delta` frame pair, or a broker L2 subscription) that pushes `DomLevel`
// values per price tick. The feeder should write into `Chart::dom_levels` (or a
// per-symbol live_state cache), and `core.rs` should read from that instead of
// calling this function. Until that feed exists, this deterministic-hash mock is
// kept so the DOM widget layout is exercisable.
pub(crate) fn generate_mock_levels(center_price: f32, tick_size: f32, count: i32) -> Vec<DomLevel> {
    let mut levels = Vec::with_capacity((count * 2 + 1) as usize);
    for row in (-count..=count).rev() {
        let price = center_price + row as f32 * tick_size;
        let dist = row.unsigned_abs();
        let base = 3000u32.saturating_sub(dist * 150).max(100);
        // Saturating cast: avoids UB for negative or very large prices.
        let hash = (price * 1000.0).max(0.0).min(u32::MAX as f32) as u32;
        let h1 = hash.wrapping_mul(2654435761);
        let h2 = hash.wrapping_mul(2246822519);
        let bid_raw = base + (h1 % 2000); let ask_raw = base + (h2 % 2000);
        // A real ladder is ONE-SIDED per level: resting bids sit at or below the
        // inside market, resting offers above it. The mock used to put BOTH
        // sizes on EVERY level, which is not a book anyone could trade — and it
        // made "best bid = highest price with bid_size > 0" resolve to the TOP
        // of the ladder and "best ask" to the BOTTOM, i.e. an inverted spread.
        // Split the sides so the mock is a plausible book and size-derived
        // best bid/ask agree with the positional reading.
        let (bid, ask) = if row > 0 { (0, ask_raw) } else { (bid_raw, 0) };
        let vol = (bid_raw as u64 + ask_raw as u64) * 3 + (h1 as u64 % 5000);
        let delta = bid_raw as i64 - ask_raw as i64 + ((h1 % 200) as i64 - 100);
        levels.push(DomLevel { price, bid_size: bid, ask_size: ask, volume: vol, delta, absorbed: false, pulled: false, big_print: false, buy_vol: 0, sell_vol: 0 });
    }
    levels
}

pub(crate) fn draw(
    ui: &mut egui::Ui, dom_rect: egui::Rect, current_price: f32, levels: &[DomLevel],
    tick_size: f32, center_price: &mut f32, dom_width: &mut f32,
    orders: &[OrderLevel], dom_selected_price: &mut Option<f32>,
    dom_order_type: &mut DomOrderType, order_qty: &mut u32,
    new_order: &mut Option<(OrderSide, f32, u32)>,
    dom_bracket: &mut Option<(OrderSide, f32, u32)>, // Shift+click → bracket (entry, auto stop/target)
    cancel_all: &mut bool,
    dom_flatten: &mut bool,
    cancel_order_id: &mut Option<u32>, move_order: &mut Option<(u32, f32)>,
    dom_armed: &mut bool, dom_col_mode: &mut u8,
    dom_dragging: &mut Option<(u32, f32)>, // (order_id, current_y) while dragging
    dom_position: &mut u8, dom_fullscreen: &mut bool,
    is_live: bool,
    dom_position_info: Option<(f32, i32, f32)>, // (avg_price, signed qty, $/point) of the open position, if any
    dom_tape: &[(f32, f32, bool)], // recent prints (price, size, is_buy), newest first — for the T&S strip
    dom_tape_speed: f32,           // prints/sec over the tape window (0 = unknown/not live)
    dom_cvd: f32,                  // session cumulative order-flow delta (0 = not live)
    t: &Theme,
) {
    let painter = ui.painter_at(dom_rect);
    painter.rect_filled(dom_rect, 0.0, t.toolbar_bg);
    // Border + resize handle live on the side that faces the chart canvas.
    // Position 0 (left-anchored) → right edge meets the chart; position 1
    // (right-anchored) → left edge meets the chart. Fullscreen has no chart
    // alongside it, so the handle is hidden.
    let on_left = *dom_position == 0;
    let edge_x = if on_left { dom_rect.right() } else { dom_rect.left() };
    painter.line_segment(
        [egui::pos2(edge_x, dom_rect.top()), egui::pos2(edge_x, dom_rect.bottom())],
        egui::Stroke::new(crate::ui_kit::style::stroke_std(), tint(t, Tone::Border, alpha_heavy())),
    );
    if !*dom_fullscreen {
        let hr = egui::Rect::from_min_size(
            egui::pos2(edge_x - 3.0, dom_rect.top()),
            egui::vec2(6.0, dom_rect.height()),
        );
        let hresp = ui.allocate_rect(hr, egui::Sense::drag());
        if hresp.hovered() || hresp.dragged() { ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal); }
        if hresp.dragged() {
            // Drag toward the chart shrinks the DOM, away widens it. The
            // sign flips for left vs right placement so the gesture feels
            // identical regardless of side.
            let delta = if on_left { hresp.drag_delta().x } else { -hresp.drag_delta().x };
            *dom_width = (*dom_width + delta).clamp(DOM_MIN_W, DOM_MAX_W);
        }
        if hresp.hovered() {
            let hx = if on_left { edge_x - 1.0 } else { edge_x + 1.0 };
            painter.line_segment(
                [egui::pos2(hx, dom_rect.top() + 14.0), egui::pos2(hx, dom_rect.bottom())],
                egui::Stroke::new(stroke_thick(), tint(t, Tone::Accent, alpha_strong())),
            );
        }
    }

    let inner = dom_rect.shrink2(egui::vec2(3.0, 0.0));
    let aw = inner.width();
    let mode = *dom_col_mode; // 0=compact, 1=normal, 2=expanded
    let show_delta = mode >= 1;
    let show_vol = mode >= 1;
    let show_numbers = mode < 2; // expanded mode hides bid/ask numbers

    // Column widths adapt to mode.
    //
    // Δ was a flat `aw * 0.09` — about 19px at the default 220px sidebar,
    // while a signed 4-digit delta (`-1437`) measures ~30px in the ladder's
    // mono face. It did not fit, and since cells paint centred with no clip it
    // overhung ~5px each side: into BID on the right, and off the panel edge
    // on the left, which ATE THE MINUS SIGN. `dom_row` now clips and abbreviates
    // so neither can happen again — but a column that cannot hold its own
    // typical value just forces permanent abbreviation, so give it a floor
    // derived from the font rather than leaving it at a fraction that was never
    // checked against its contents.
    //
    // Floor = the width of the ABBREVIATED worst case (`-1.4K`) in the tier
    // the row actually paints Δ with, so it tracks the type scale instead of
    // pinning a number that was right on the day it was written.
    //
    // Deliberately the abbreviated form, not the full `-1437`. Sizing to the
    // full form took ~20px from a 214px row, and since the other columns split
    // what is left, BID went too narrow for even its own abbreviated value —
    // trading a Δ/BID collision for centre-clipped BID cells (`L.8H`), which
    // is worse. Six columns do not fit at full precision in a 220px sidebar;
    // Δ gets enough to always show a signed abbreviated value, and `fit_cell`
    // handles the rest.
    let delta_floor = crate::ui_kit::style::measure_with(
        ui, "-1.4K", TextStyle::MonoSm.font_id_in(ui),
    ).x + 6.0; // + the cell's two gutters and a little air
    let cd = if show_delta { (aw * 0.09).max(delta_floor) } else { 0.0 };
    let co = aw * 0.14;
    let cv = if show_vol { aw * 0.14 } else { 0.0 };
    let remaining = aw - cd - cv - co;
    let cb = remaining * 0.27; let cp = remaining * 0.46; let ca = remaining * 0.27;
    let x0 = inner.left();
    let xb = x0+cd; let xp = xb+cb; let xa = xp+cp; let xv = xa+ca; let xo = xv+cv;

    // Header — bigger labels via the design-system tokens. Columns left of
    // PRICE align right (DELTA, BID); columns right of PRICE align left
    // (ASK, VOL, ORD); PRICE itself is centered. The whole header sits on
    // top of a hairline border that separates the labels from the ladder.
    let header_h = 24.0_f32;
    // The LIVE / SIMULATED badge gets its OWN band above the column headers.
    //
    // It used to be centred in the header strip itself — same rect as the
    // column labels — so `SIMULATED` (the wider of the two) painted straight
    // over `PRICE` and `ASK`, smearing two of the five headers illegible.
    // Which is precisely when a trader most needs to read them: the badge is
    // only wide enough to collide in the SIMULATED case, i.e. when the numbers
    // beneath it are fabricated.
    //
    // Derived from the tier the badge is painted in (MonoSm) + its 2px padding
    // + a gap, so the band tracks the type scale.
    let badge_band = TextStyle::MonoSm.font_id_in(ui).size + 4.0 + 3.0;
    let hy = inner.top() + 2.0 + badge_band;
    let label_y = hy + header_h * 0.5;
    let hf = TextStyle::MonoMd.font_id_in(ui);
    let hc = color_muted(t.dim);
    let pad_x = gap_xs();
    if show_delta {
        painter.text(egui::pos2(x0 + cd - pad_x, label_y), egui::Align2::RIGHT_CENTER,
            "\u{0394}", hf.clone(), hc);
    }
    painter.text(egui::pos2(xb + cb - pad_x, label_y), egui::Align2::RIGHT_CENTER,
        "BID", hf.clone(), color_muted(t.bull));
    // PRICE header — center-aligned, double-click to recenter.
    let price_hdr_rect = egui::Rect::from_min_size(egui::pos2(xp, hy), egui::vec2(cp, header_h));
    let price_hdr_resp = ui.allocate_rect(price_hdr_rect, egui::Sense::click());
    painter.text(egui::pos2(xp + cp * 0.5, label_y), egui::Align2::CENTER_CENTER,
        "PRICE", hf.clone(), hc);
    if price_hdr_resp.double_clicked() {
        *center_price = (current_price / tick_size).round() * tick_size;
    }
    if price_hdr_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    painter.text(egui::pos2(xa + pad_x, label_y), egui::Align2::LEFT_CENTER,
        "ASK", hf.clone(), color_muted(t.bear));
    if show_vol {
        painter.text(egui::pos2(xv + pad_x, label_y), egui::Align2::LEFT_CENTER,
            "VOL", hf.clone(), hc);
    }
    // ORD label + panel menu (compact icon Select). Combines display-mode
    // (Compact / Normal / Expanded) with placement actions (move left / move
    // right / fullscreen) so all per-panel toggles live in one menu.
    let mode_btn_size = 22.0_f32;
    let ord_label_w = co - mode_btn_size - pad_x;
    painter.text(egui::pos2(xo + pad_x, label_y), egui::Align2::LEFT_CENTER,
        "ORD", hf.clone(), hc);
    let mode_rect = egui::Rect::from_min_size(
        egui::pos2(xo + ord_label_w, hy + (header_h - mode_btn_size) * 0.5),
        egui::vec2(mode_btn_size, mode_btn_size),
    );
    // Subtle background tint so the trigger reads as a button even at rest.
    painter.rect_filled(mode_rect,
        crate::ui_kit::style::r_sm_cr(),
        tint(t, Tone::Border, alpha_subtle()));
    {
        // Action enum so item_render and trigger_render get a real value to
        // dispatch on (plain &str labels would skip the trigger render path).
        #[derive(Clone, Copy, PartialEq)]
        enum DomMenuItem {
            Compact, Normal, Expanded,
            Move, Fullscreen,
        }
        // Items are dynamic — only one of "Send to left" / "Send to right"
        // is shown depending on where the panel currently lives. The label
        // for `Move` is computed off `dom_position` at render time.
        let items: Vec<DomMenuItem> = vec![
            DomMenuItem::Compact,
            DomMenuItem::Normal,
            DomMenuItem::Expanded,
            DomMenuItem::Move,
            DomMenuItem::Fullscreen,
        ];
        let dom_on_left = *dom_position == 0;
        let move_label: &str = if dom_on_left { "Send to right" } else { "Send to left" };
        // Selected index reflects display-mode only. Action items act on
        // click without changing the persistent selection.
        let mut sel: usize = (mode as usize).min(2);
        let prev_sel = sel;
        let _ = place_at(ui, mode_rect, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            Select::new_with(&mut sel, &items, move |it: &DomMenuItem| match it {
                    DomMenuItem::Compact => "Compact".into(),
                    DomMenuItem::Normal => "Normal".into(),
                    DomMenuItem::Expanded => "Expanded".into(),
                    DomMenuItem::Move => move_label.into(),
                    DomMenuItem::Fullscreen => "Fullscreen".into(),
                })
                .size(KitSize::Sm)
                .compact_trigger(true)
                .trigger_render(|ui, theme, _idx, _opt| {
                    ui.painter().text(
                        ui.max_rect().center(),
                        egui::Align2::CENTER_CENTER,
                        Icon::SLIDERS,
                        crate::ui_kit::style::prop_at(font_md() + 3.0),
                        theme.text(),
                    );
                })
                .show(ui, t)
        });
        if sel != prev_sel {
            match items[sel] {
                DomMenuItem::Compact  => *dom_col_mode = 0,
                DomMenuItem::Normal   => *dom_col_mode = 1,
                DomMenuItem::Expanded => *dom_col_mode = 2,
                DomMenuItem::Move => {
                    // Toggle to the opposite side; clears fullscreen since
                    // there's no concept of "side" while fullscreen.
                    *dom_position = if dom_on_left { 1 } else { 0 };
                    *dom_fullscreen = false;
                }
                DomMenuItem::Fullscreen => { *dom_fullscreen = !*dom_fullscreen; }
            }
        }
    }

    // Dividing border below the header — separates the labels from the
    // ladder body so the column titles read as a header strip, not as the
    // first row of data.
    let sep_y = hy + header_h;
    painter.line_segment(
        [egui::pos2(inner.left(), sep_y), egui::pos2(inner.right(), sep_y)],
        egui::Stroke::new(crate::ui_kit::style::stroke_std(), tint(t, Tone::Border, alpha_strong())),
    );

    // ── LIVE / SIMULATED data badge ───────────────────────────────────────────
    // When a live `/ws/dom` frame arrived recently (`is_live`), the ladder is
    // real depth — show a green LIVE badge. Otherwise it's `generate_mock_levels`
    // (fabricated) — show a warn SIMULATED badge so a trader is never misled.
    {
        // Cascade-resolved. This same FontId lays out every galley below AND
        // sizes the badge box — measure and paint must not diverge.
        let badge_font = TextStyle::MonoSm.font_id_in(ui);
        let (badge_text, badge_fg, badge_bg) = if is_live {
            ("LIVE", t.bull, tint(t, Tone::Bull, 28))
        } else {
            ("SIMULATED", t.warn, tint(t, Tone::Warn, 28))
        };
        let badge_pad_x = gap_xs();
        let badge_pad_y = 2.0_f32;
        let galley = painter.layout_no_wrap(badge_text.to_string(), badge_font.clone(), badge_fg);
        // DOM Phase 2: net order-flow delta appended to the LIVE badge — the sum
        // of the per-price tape delta across the book (executed buy vs sell
        // pressure). Green when net-buying, red when net-selling. Only shown on
        // live tape; the mock generator's delta is meaningless so it's omitted.
        let flow_delta: i64 = if is_live { levels.iter().map(|l| l.delta).sum() } else { 0 };
        let delta_galley = if is_live {
            let dc = if flow_delta > 0 { t.bull } else if flow_delta < 0 { t.bear } else { t.dim };
            Some((painter.layout_no_wrap(format!("  \u{0394}{:+}", flow_delta), badge_font.clone(), dc), dc))
        } else {
            None
        };
        let gw = galley.size().x;
        let dgw = delta_galley.as_ref().map_or(0.0, |(g, _)| g.size().x);
        let bw = gw + dgw + badge_pad_x * 2.0;
        let bh = galley.size().y + badge_pad_y * 2.0;
        // Centred horizontally, and vertically inside its OWN band above the
        // column headers (see `badge_band`) — not inside the header strip,
        // which is what made SIMULATED overwrite PRICE and ASK.
        let bx = inner.left() + (aw - bw) * 0.5;
        let band_top = inner.top() + 2.0;
        let badge_rect = egui::Rect::from_min_size(
            egui::pos2(bx, band_top + (badge_band - bh) * 0.5),
            egui::vec2(bw, bh),
        );
        painter.rect_filled(badge_rect, crate::ui_kit::style::r_sm_cr(), badge_bg);
        let tx = badge_rect.left() + badge_pad_x;
        let ty = badge_rect.top() + badge_pad_y;
        painter.galley(egui::pos2(tx, ty), galley, badge_fg);
        if let Some((dg, dc)) = delta_galley {
            painter.galley(egui::pos2(tx + gw, ty), dg, dc);
        }
    }

    // ── Bottom controls ──
    // Padding tokens: gap_2xs above the row 1 (combo) and again below the
    // action row before the panel edge. Heights chosen so the FLATTEN/CANCEL
    // stack can grow without crowding the BUY/SELL pills.
    // Larger bottom inset so the action row sits above the panel edge with
    // breathing room — keeps CANCEL from kissing the bottom border.
    let pad_top = gap_2xs();
    let pad_bot = gap_sm();
    let r1h = 22.0_f32;
    // BUY/SELL pills bumped 16px taller — also drives the FLATTEN/CANCEL
    // stack via mid_half_h, so the four action buttons read as the same
    // total height regardless of stacking.
    let action_h = 52.0_f32;
    let row_gap = gap_xs();
    let ctrl_h = pad_top + r1h + row_gap + action_h + pad_bot;
    let ctrl_top = inner.bottom() - ctrl_h;
    painter.rect_filled(
        egui::Rect::from_min_max(egui::pos2(dom_rect.left(), ctrl_top), egui::pos2(dom_rect.right(), dom_rect.bottom())),
        0.0, t.toolbar_bg,
    );
    // Inset shadow above the controls so the row reads as separate from the
    // ladder above. Uses theme.shadow_color so light themes don't render black.
    for i in 0..4u32 {
        painter.line_segment(
            [egui::pos2(inner.left(), ctrl_top - i as f32), egui::pos2(inner.right(), ctrl_top - i as f32)],
            egui::Stroke::new(crate::ui_kit::style::stroke_std(), shadow_color_alpha(t, 20u8.saturating_sub(i as u8 * 5))),
        );
    }
    // Hairline divider between DOM ladder and the controls.
    painter.line_segment(
        [egui::pos2(inner.left(), ctrl_top), egui::pos2(inner.right(), ctrl_top)],
        egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_strong())),
    );

    let fm = TextStyle::MonoMd.font_id_in(ui);
    let is_mkt = *dom_order_type == DomOrderType::Market;

    // Row 1: [stepper: − qty +]  [MKT/LMT combo]  [A]
    //
    // AUDIT 2026-08 — solved, not walked. This was a cursor: `cx = left()+1`,
    // then `cx = stepper.right()+4`, then `cx = combo.right()+3`, with the arm
    // button sized by `inner.right() - cx - 1`. Hand-rolled flexbox, where each
    // control's position depends on the measured edge of the one before it, so
    // a change to any width silently walks everything after it.
    //
    // The 4px and 3px gutters are kept EXACTLY rather than normalised onto the
    // gap ladder: this is the order-entry row, and a 1px reflow here is not
    // worth taking on a money path for tidiness. They are now declared in the
    // layout spec instead of accumulated into a cursor.
    let r1y = ctrl_top + pad_top;
    let row1 = egui::Rect::from_min_size(
        egui::pos2(inner.left(), r1y),
        egui::vec2(inner.width(), r1h),
    );
    let row1_slots = Flex::row()
        .padding_sides(1.0, 1.0, 0.0, 0.0)
        .slot("stepper", Item::fixed(aw * 0.48))
        .pad(4.0)
        .slot("combo", Item::fixed(aw * 0.30))
        .pad(3.0)
        .slot("arm", Item::grow(1.0))
        .solve_in(row1);

    // ── Quantity stepper — single rounded pill housing the - / qty / + cells.
    let stepper_rect = row1_slots.rect("stepper");
    let stepper_radius = egui::CornerRadius::same((r1h * 0.5) as u8);
    painter.rect_filled(stepper_rect, stepper_radius, tint(t, Tone::Bg, 200));
    painter.rect_stroke(
        stepper_rect, stepper_radius,
        egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_line())),
        egui::StrokeKind::Inside,
    );
    let btn_w = r1h; // square hit areas at each end
    // [−]
    let minus_rect = egui::Rect::from_min_size(stepper_rect.min, egui::vec2(btn_w, r1h));
    let minus_resp = ui.allocate_rect(minus_rect, egui::Sense::click());
    if minus_resp.hovered() {
        painter.rect_filled(minus_rect, stepper_radius, tint(t, Tone::Text, alpha_subtle()));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.text(minus_rect.center(), egui::Align2::CENTER_CENTER, "−", fm.clone(),
        if minus_resp.hovered() { t.text } else { t.dim });
    if minus_resp.clicked() && *order_qty > 1 { *order_qty -= 1; }
    // qty (centered)
    painter.text(stepper_rect.center(), egui::Align2::CENTER_CENTER,
        &format!("{}", *order_qty), fm.clone(), t.text);
    // [+]
    let plus_rect = egui::Rect::from_min_size(
        egui::pos2(stepper_rect.right() - btn_w, stepper_rect.top()),
        egui::vec2(btn_w, r1h));
    let plus_resp = ui.allocate_rect(plus_rect, egui::Sense::click());
    if plus_resp.hovered() {
        painter.rect_filled(plus_rect, stepper_radius, tint(t, Tone::Text, alpha_subtle()));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.text(plus_rect.center(), egui::Align2::CENTER_CENTER, "+", fm.clone(),
        if plus_resp.hovered() { t.text } else { t.dim });
    if plus_resp.clicked() { *order_qty += 1; }
    // Scroll-to-size over the stepper — NinjaTrader/Bookmap feel: wheel up
    // raises the working qty, wheel down lowers it (floor 1). The stepper lives
    // in the controls area (below `ctrl_top`), so this never fights the ladder's
    // scroll-to-recenter above. One increment per wheel notch.
    let step_hover = ui.input(|i| i.pointer.hover_pos()).map_or(false, |p| stepper_rect.contains(p));
    if step_hover {
        let s = ui.input(|i| i.raw_scroll_delta.y);
        if s > 0.5 { *order_qty += 1; }
        else if s < -0.5 && *order_qty > 1 { *order_qty -= 1; }
    }
    // ── MKT / LMT combobox (proper Select, not a click-cycle).
    const ORDER_TYPE_LABELS: &[&str] = &["MKT", "LMT"];
    let combo_rect = row1_slots.rect("combo");
    {
        let mut sel: usize = if is_mkt { 0 } else { 1 };
        let prev = sel;
        let _ = place_at(ui, combo_rect, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            Select::new(&mut sel, ORDER_TYPE_LABELS)
                .size(KitSize::Sm)
                .full_width()
                .show(ui, t)
        });
        if sel != prev {
            *dom_order_type = if sel == 0 { DomOrderType::Market } else { DomOrderType::Limit };
            if sel == 0 { *dom_selected_price = None; }
        }
    }
    // ── Arm toggle — when armed: red bg + live PULSE icon. When disarmed:
    // neutral Secondary with shield-style icon. Tooltip explains the state.
    let arm_rect = row1_slots.rect("arm");
    let arm_radius = crate::ui_kit::style::r_sm_cr();
    let arm_resp = ui.allocate_rect(arm_rect, egui::Sense::click());
    if *dom_armed {
        // Filled red bg, white icon, subtle border.
        painter.rect_filled(arm_rect, arm_radius, t.bear);
        painter.rect_stroke(arm_rect, arm_radius,
            egui::Stroke::new(stroke_thin(), tint(t, Tone::Bear, alpha_strong())),
            egui::StrokeKind::Inside);
        // Armed badge glyph on `t.bear` fill — use contrast_fg so a light
        // bear (if ever introduced) gets black instead of unreadable white.
        painter.text(arm_rect.center(), egui::Align2::CENTER_CENTER,
            Icon::PULSE, crate::ui_kit::style::prop_at(font_md() + 1.0),
            contrast_fg(t.bear));
    } else {
        let bg = if arm_resp.hovered() {
            tint(t, Tone::Border, alpha_dim())
        } else {
            tint(t, Tone::Border, alpha_subtle())
        };
        painter.rect_filled(arm_rect, arm_radius, bg);
        painter.rect_stroke(arm_rect, arm_radius,
            egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_line())),
            egui::StrokeKind::Inside);
        painter.text(arm_rect.center(), egui::Align2::CENTER_CENTER,
            Icon::PULSE, crate::ui_kit::style::prop_at(font_md() + 1.0),
            t.dim);
    }
    let arm_resp = arm_resp.on_hover_text(if *dom_armed {
        "Armed — clicks on the ladder fire live orders. Click again to disarm."
    } else {
        "Disarmed — clicks on the ladder are inactive. Arm to enable one-click trading."
    });
    if arm_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    if arm_resp.clicked() { *dom_armed = !*dom_armed; }

    // Row 2: [BUY] [FLATTEN/CANCEL stacked] [SELL]
    // BUY/SELL narrower; FLATTEN/CANCEL wider so they read as the primary
    // management actions. BUY/SELL text bumped to Md so the labels feel as
    // weighty as the trade they fire.
    let r2y = r1y + r1h + row_gap;
    let side_w = aw * 0.30;
    let mid_w = aw - side_w * 2.0 - 6.0;
    let mid_half_h = action_h * 0.5 - 1.0;

    // BUY (spans full action height) — UX-1 Fix 3: kbd hint from default_hotkeys
    let r = egui::Rect::from_min_size(egui::pos2(inner.left()+1.0, r2y), egui::vec2(side_w, action_h));
    let resp = place_at(ui, r, |ui| {
        ui.add(Button::buy("BUY")
            .tint(t.bull)
            .size(KitSize::Md)
            .kbd("Ctrl+B")
            // `button.action`, not `button.primary`.
            //
            // These four are large block controls in a trading action row, and
            // `button.primary`'s `RadiusTier::Pill` resolves to min(w, h) / 2 —
            // a full ELLIPSE at this near-square aspect ratio. That used to be
            // escaped with a hardcoded `corner_radius_asymmetric` at each call
            // site, which fixed the look but pinned the shape: a style could no
            // longer decide it.
            //
            // The dedicated key is authored by all six styles, so the action
            // row's corner treatment is a per-style decision again — and all
            // four buttons resolve through ONE key, which is what makes them
            // agree rather than four call sites happening to pass the same
            // constant.
            .recipe_key("button.action")
            // `fixed_size`, not `min_size`. These four slots are computed to
            // tile `aw` exactly, so any button that exceeds its slot paints
            // over its neighbour. With `min_size` (a floor only) that is what
            // happened: the `kbd` hints added here made "BUY  Ctrl+B" wider
            // than `side_w`, and BUY/SELL overdrew FLATTEN and CANCEL — the
            // four highest-stakes controls in the app, overlapping.
            .fixed_size(egui::vec2(side_w, action_h)))
    });
    if resp.clicked() { let p = if !is_mkt { dom_selected_price.unwrap_or(current_price) } else { current_price }; if p > 0.0 { *new_order = Some((OrderSide::Buy, p, *order_qty)); } }

    // Middle: FLATTEN (top) + CANCEL (bottom) — proper Secondary buttons,
    // matching the BUY/SELL pill treatment so the row reads as a unit.
    let mid_x = inner.left()+1.0+side_w+3.0;

    // FLATTEN — Variant::NeutralAction (grey fill, black text). Reads as the
    // structural "close all" action: high contrast, low chroma, no direction.
    // UX-1 Fix 3: kbd hint.
    let r = egui::Rect::from_min_size(egui::pos2(mid_x, r2y), egui::vec2(mid_w, mid_half_h));
    let resp = place_at(ui, r, |ui| {
        ui.add(Button::new("FLATTEN")
            .variant(Variant::NeutralAction)
            .size(KitSize::Md)
            .kbd("Ctrl+Shift+F")
            // `button.action`, not `button.primary`.
            //
            // These four are large block controls in a trading action row, and
            // `button.primary`'s `RadiusTier::Pill` resolves to min(w, h) / 2 —
            // a full ELLIPSE at this near-square aspect ratio. That used to be
            // escaped with a hardcoded `corner_radius_asymmetric` at each call
            // site, which fixed the look but pinned the shape: a style could no
            // longer decide it.
            //
            // The dedicated key is authored by all six styles, so the action
            // row's corner treatment is a per-style decision again — and all
            // four buttons resolve through ONE key, which is what makes them
            // agree rather than four call sites happening to pass the same
            // constant.
            .recipe_key("button.action")
            .fixed_size(egui::vec2(mid_w, mid_half_h)))
    });
    // DOM Phase 1: FLATTEN closes the POSITION (cancel working orders +
    // market-close) via OrderManager::flatten — distinct from CANCEL, which
    // only cancels working orders. (They used to do the same thing.)
    if resp.clicked() { *dom_flatten = true; }

    // CANCEL — pastel red fill so it reads as a destructive (but not alarm)
    // action paired with FLATTEN. Centered text, dark fg for readable
    // contrast against the soft red. Colors come from the order-status
    // semantic tokens (style::order_cancel_bg / order_cancel_fg).
    // UX-1 Fix 3: kbd hint.
    let cancel_fill = order_cancel_bg();
    let r = egui::Rect::from_min_size(egui::pos2(mid_x, r2y+mid_half_h+2.0), egui::vec2(mid_w, mid_half_h));
    let resp = place_at(ui, r, |ui| {
        ui.add(Button::new("CANCEL")
            .variant(Variant::Secondary)
            .size(KitSize::Md)
            .fill(cancel_fill)
            .fg(order_cancel_fg())
            .kbd("Ctrl+Shift+Q")
            // `button.action`, not `button.primary`.
            //
            // These four are large block controls in a trading action row, and
            // `button.primary`'s `RadiusTier::Pill` resolves to min(w, h) / 2 —
            // a full ELLIPSE at this near-square aspect ratio. That used to be
            // escaped with a hardcoded `corner_radius_asymmetric` at each call
            // site, which fixed the look but pinned the shape: a style could no
            // longer decide it.
            //
            // The dedicated key is authored by all six styles, so the action
            // row's corner treatment is a per-style decision again — and all
            // four buttons resolve through ONE key, which is what makes them
            // agree rather than four call sites happening to pass the same
            // constant.
            .recipe_key("button.action")
            .fixed_size(egui::vec2(mid_w, mid_half_h)))
    });
    if resp.clicked() { *cancel_all = true; }

    // SELL (spans full action height) — UX-1 Fix 3: kbd hint.
    let r = egui::Rect::from_min_size(egui::pos2(mid_x+mid_w+3.0, r2y), egui::vec2(side_w, action_h));
    let resp = place_at(ui, r, |ui| {
        ui.add(Button::sell("SELL")
            .tint(t.bear)
            .size(KitSize::Md)
            .kbd("Ctrl+Shift+B")
            // `button.action`, not `button.primary`.
            //
            // These four are large block controls in a trading action row, and
            // `button.primary`'s `RadiusTier::Pill` resolves to min(w, h) / 2 —
            // a full ELLIPSE at this near-square aspect ratio. That used to be
            // escaped with a hardcoded `corner_radius_asymmetric` at each call
            // site, which fixed the look but pinned the shape: a style could no
            // longer decide it.
            //
            // The dedicated key is authored by all six styles, so the action
            // row's corner treatment is a per-style decision again — and all
            // four buttons resolve through ONE key, which is what makes them
            // agree rather than four call sites happening to pass the same
            // constant.
            .recipe_key("button.action")
            .fixed_size(egui::vec2(side_w, action_h)))
    });
    if resp.clicked() { let p = if !is_mkt { dom_selected_price.unwrap_or(current_price) } else { current_price }; if p > 0.0 { *new_order = Some((OrderSide::Sell, p, *order_qty)); } }

    // ── Price ladder ──
    let body_top = sep_y+1.0;
    // DOM Phase 2: reserve a bottom band for the reconstructed time & sales
    // strip when live prints are available. `tape_row_h` per print + a header
    // row; capped so it never starves the ladder on a short pane.
    // Deliberately dense streaming strip, but tall enough that the print text
    // actually fits (13px flat clipped a 12px glyph box). Derived from the SAME
    // tier `tsf` paints the prints with (below), so the strip's row pitch always
    // tracks the cascade — including a subtree override of `apex.MonoSm`.
    let tape_row_h: f32 = TextStyle::MonoSm.font_id_in(ui).size + 2.0;
    let tape_rows = if dom_tape.is_empty() { 0 } else {
        dom_tape.len().min(8).min(((ctrl_top - body_top - 60.0) / tape_row_h) as usize)
    };
    let tape_h = if tape_rows == 0 { 0.0 } else { tape_row_h * (tape_rows as f32 + 1.0) + 4.0 };
    let body_h = (ctrl_top - body_top - 2.0 - tape_h).max(60.0);
    let max_rows = (body_h / ROW_H) as i32;
    let half = max_rows / 2;

    let pil = ui.input(|i| i.pointer.hover_pos()).map_or(false, |p| p.x >= dom_rect.left() && p.x <= dom_rect.right() && p.y >= body_top && p.y <= ctrl_top);
    if pil { let s = ui.input(|i| i.raw_scroll_delta.y); if s.abs() > 0.5 { *center_price += if s > 0.0 { tick_size } else { -tick_size }; } }
    // DOM Phase 1: middle-click the ladder → snap the view back to the live
    // market, undoing any scroll drift (recenter/freeze). The middle button
    // avoids the left-click-to-trade and right-click-to-cancel gestures.
    if pil && current_price > 0.0 && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Middle)) {
        *center_price = current_price;
    }
    // Clamp scroll so the ladder can't drift more than 200 ticks from current price.
    if current_price > 0.0 {
        let max_drift = tick_size * 200.0;
        *center_price = center_price.clamp(current_price - max_drift, current_price + max_drift);
    }

    let sc = (*center_price / tick_size).round() * tick_size;
    let mb = levels.iter().map(|l| l.bid_size).max().unwrap_or(1).max(1);
    let ma = levels.iter().map(|l| l.ask_size).max().unwrap_or(1).max(1);
    let ms = mb.max(ma) as f32;
    let mv = levels.iter().map(|l| l.volume).max().unwrap_or(1).max(1);
    let ao: Vec<&OrderLevel> = orders.iter().filter(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed).collect();
    let lp = ui.painter_at(egui::Rect::from_min_max(egui::pos2(dom_rect.left(), body_top), egui::pos2(dom_rect.right(), body_top+body_h)));

    // Shared column geometry — computed once, passed to every row.
    let col_layout = ColumnLayout {
        x0, cd,
        xb, cb,
        xp, cp,
        xa, ca,
        xv, cv,
        xo, co,
        show_delta, show_vol, show_numbers,
    };

    // Resolve the drop-target row index from the live drag y, if any.
    let drop_target_rit: Option<i32> = dom_dragging.map(|(_, dy)| ((dy - body_top) / ROW_H).round() as i32);

    // Ladder-overlay fonts, hoisted OUT of the per-rung loop (this loop runs
    // once per visible price level, every frame) — one cascade lookup each,
    // cloned at the paint site.
    let f_ovl = TextStyle::MonoSm.font_id_in(ui);   // L/S position tag
    let f_pnl = TextStyle::Numeric.font_id_in(ui);  // PnL-at-price

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

        // Build the rich-orders array for this row.
        let rich: Vec<(u32, char, u32, egui::Color32)> = oap.iter().map(|ord| {
            let oc = ord.color(t.bull, t.bear);
            let side_ch = match ord.side { OrderSide::Buy | OrderSide::TriggerBuy => 'B', _ => 'S' };
            (ord.id, side_ch, ord.qty, oc)
        }).collect();

        // Build cross-row drag context.
        let mut drag_cx = DomRowDragCx::default();
        if let Some((did, _)) = *dom_dragging {
            drag_cx.dragging_order_id = Some(did);
            if drop_target_rit == Some(rit) {
                if let Some(drag_ord) = ao.iter().find(|o| o.id == did) {
                    drag_cx.is_drop_target = true;
                    drag_cx.ghost_side = Some(match drag_ord.side {
                        OrderSide::Buy | OrderSide::TriggerBuy => 'B', _ => 'S',
                    });
                    drag_cx.ghost_qty = drag_ord.qty;
                    drag_cx.ghost_color = drag_ord.color(t.bull, t.bear);
                }
            }
        }

        // Compute fills (parent-normalized).
        let bid_fill_v = if bs > 0 { bs as f32 / ms } else { 0.0 };
        let ask_fill_v = if ask > 0 { ask as f32 / ms } else { 0.0 };
        let vol_fill_v = if vol > 0 { vol as f32 / mv as f32 } else { 0.0 };

        // Imbalance hint (positive → ask-side bull, negative → bear). The
        // widget only uses sign to pick price color in our fallback path.
        let imb = if ia { 1.0 } else { -1.0 };

        let resp = DomRow::new(price, bs, ask)
            .bid_fill(bid_fill_v).ask_fill(ask_fill_v)
            .delta(delta).volume(vol, vol_fill_v)
            .selected(is).current_price(ic).inside_spread(false)
            .imbalance_fill(imb)
            // M5: `ROW_H` is a deliberate DENSITY choice — the ladder wants as
            // many price levels on screen as will fit, so it is kept as the
            // target rather than replaced by the density-derived default.
            // But it is a target, not a licence to clip: floor it at the height
            // the row's tallest text tier actually needs. At today's ladder
            // (font_md 14) that is 18.2 against ROW_H's 18.0, so the ladder
            // barely moves; the point is that it can no longer silently go
            // short if the type scale moves again.
            .height(ROW_H.max(super::super::lists::rows::dom_row::dom_row_min_height()))
            .theme(t)
            .column_layout(col_layout)
            .drag_cx(drag_cx)
            .compact_price(tick_size < 1.0)
            .rich_orders(&rich)
            .show_in(ui, &lp, rr);

        if resp.row_hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
        if resp.row_clicked {
            // DOM Phase 1: direct bid/ask click-to-trade (ThinkorSwim Active
            // Trader feel). Left-click the BID cell → BUY limit at that price;
            // the ASK cell → SELL limit. Requires ARM (one-click trading is
            // dangerous); disarmed OR a click in the PRICE column falls back to
            // the safe select-for-BUY/SELL-buttons behavior. Order placement
            // itself goes through the existing guarded submit_order path
            // (OrderSource::DomLadder) via `new_order`.
            let cx = ui.input(|i| i.pointer.interact_pos()).map(|p| p.x).unwrap_or(f32::NAN);
            let in_bid = cx >= xb && cx < xb + cb;
            let in_ask = cx >= xa && cx < xa + ca;
            // DOM Phase 1: stale-book trade guard. Only fire a one-click order
            // when the depth feed is LIVE — never against a SIMULATED/frozen book
            // (mock levels), where the price under the cursor is fabricated. When
            // stale, the click falls through to select-for-buttons (the deliberate
            // two-step path), and the SIMULATED badge already warns the trader.
            if *dom_armed && is_live && (in_bid || in_ask) && price > 0.0 {
                let side = if in_bid { OrderSide::Buy } else { OrderSide::Sell };
                // DOM Phase 1: Shift+click → bracket order. The entry is a limit
                // at the clicked price and OrderManager attaches an auto stop /
                // target (tick offsets derived in core.rs) — the NinjaTrader ATM
                // feel. A plain click stays a single limit order.
                if ui.input(|i| i.modifiers.shift) {
                    *dom_bracket = Some((side, price, *order_qty));
                } else {
                    *new_order = Some((side, price, *order_qty));
                }
            } else {
                *dom_selected_price = Some(price);
                *dom_order_type = DomOrderType::Limit;
            }
        }

        // DOM Phase 2: pull / absorption wash. Absorption = gold tint (a level
        // soaking up executed flow — likely support/resistance strength). Pull =
        // accent/blue tint (resting liquidity yanked without trading — weakness).
        // Low alpha so the row's size/vol/delta numbers still read through it.
        if let Some(l) = lv {
            if l.absorbed {
                lp.rect_filled(rr, 0.0, color_alpha(t.gold, crate::ui_kit::style::alpha_hint()));
            } else if l.pulled {
                lp.rect_filled(rr, 0.0, color_alpha(t.accent, 22));
            }
            // DOM Phase 2: big-print outline — a crisp bright inset border on a
            // row that saw an outlier block/sweep this frame. A stroke (not a
            // fill) so it reads distinctly on top of the absorption/pull washes.
            if l.big_print {
                lp.rect_stroke(
                    rr.shrink(0.5), 0.0,
                    egui::Stroke::new(1.2, color_alpha(t.text, 210)),
                    egui::StrokeKind::Inside,
                );
            }
            // DOM Phase 2: aggressive buy/sell split bar — a 2px band at the row
            // bottom divided by aggressor volume (green = lifting the offer, red =
            // hitting the bid). At-a-glance who won the trading at each price.
            let split_tot = l.buy_vol + l.sell_vol;
            if split_tot > 0 {
                let bar_y = rr.bottom() - 2.0;
                let bwid = aw * (l.buy_vol as f32 / split_tot as f32);
                lp.rect_filled(
                    egui::Rect::from_min_size(egui::pos2(rr.left(), bar_y), egui::vec2(bwid, 2.0)),
                    0.0, color_alpha(t.bull, crate::ui_kit::style::alpha_solid()),
                );
                lp.rect_filled(
                    egui::Rect::from_min_size(egui::pos2(rr.left() + bwid, bar_y), egui::vec2(aw - bwid, 2.0)),
                    0.0, color_alpha(t.bear, crate::ui_kit::style::alpha_solid()),
                );
            }
        }

        // DOM Phase 1: position avg-price marker on the ladder (snapped to the
        // nearest tick row). Faint side-tinted band + left accent bar + a center
        // line + a compact L/S-qty tag — so the trader's average entry is
        // unmistakable on the DOM, matching TOS Active Trader.
        if let Some((avg_px, pos_qty, per_point)) = dom_position_info {
            if pos_qty != 0 && (price - avg_px).abs() < tick_size * 0.5 {
                let long = pos_qty > 0;
                let pc = if long { t.bull } else { t.bear };
                lp.rect_filled(rr, 0.0, color_alpha(pc, 22));
                lp.rect_filled(egui::Rect::from_min_size(egui::pos2(rr.left(), ry), egui::vec2(3.0, ROW_H)), 0.0, pc);
                lp.line_segment(
                    [egui::pos2(rr.left(), ry + ROW_H * 0.5), egui::pos2(rr.right(), ry + ROW_H * 0.5)],
                    egui::Stroke::new(crate::ui_kit::style::stroke_std(), color_alpha(pc, 110)));
                let tag = format!("{}{}", if long { "L" } else { "S" }, pos_qty.unsigned_abs());
                lp.text(egui::pos2(rr.left() + 5.0, ry + ROW_H * 0.5), egui::Align2::LEFT_CENTER, &tag, f_ovl.clone(), pc);
            }
            // DOM Phase 3: PnL-at-price. With an open position, show the $ this
            // position would be worth if the market printed at this row's price —
            // (price − avg) × $/point, exact from the broker's own P&L. Green in
            // profit / red in loss, right-aligned; only on rows with no working
            // order (so it never collides with an order badge) and skipping the
            // near-zero clutter around entry.
            if per_point != 0.0 && oap.is_empty() && price > 0.0 {
                let pnl = (price - avg_px) * per_point;
                if pnl.abs() >= 1.0 {
                    let pc = if pnl >= 0.0 { t.bull } else { t.bear };
                    lp.text(
                        egui::pos2(rr.right() - 4.0, ry + ROW_H * 0.5),
                        egui::Align2::RIGHT_CENTER,
                        &format!("{:+.0}", pnl), f_pnl.clone(), color_alpha(pc, 205),
                    );
                }
            }
        }

        // Drag-state plumbing.
        if let Some(oid) = resp.order_drag_started {
            *dom_dragging = Some((oid, ry));
        }
        if let Some((oid, dy_delta)) = resp.order_dragging {
            if let Some((did, ref mut dy)) = dom_dragging {
                if *did == oid {
                    *dy += dy_delta;
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
            }
        }
        if let Some(oid) = resp.order_drag_stopped {
            if let Some((did, dy)) = *dom_dragging {
                if did == oid {
                    let drop_row = ((dy - body_top) / ROW_H).round() as i32;
                    let target_ri = half - drop_row;
                    let target_price = sc + target_ri as f32 * tick_size * -1.0;
                    if let Some(ord) = ao.iter().find(|o| o.id == oid) {
                        if (target_price - ord.price).abs() > tick_size * 0.1 {
                            *move_order = Some((oid, target_price));
                        }
                    }
                }
            }
            *dom_dragging = None;
        }
        if let Some(oid) = resp.order_cancel { *cancel_order_id = Some(oid); }
        // DOM Phase 1: right-click a row holding a working order → cancel it
        // (TOS/NinjaTrader gesture), in addition to the badge's cancel affordance.
        // Reuses the existing cancel_order_id signal. No arm gate — cancelling is
        // risk-reducing and must always be available. If several orders stack on
        // one price the first is cancelled (repeat to clear the rest).
        if !oap.is_empty() {
            let rc = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Secondary)
                && i.pointer.interact_pos().map_or(false, |p| rr.contains(p)));
            if rc { *cancel_order_id = Some(oap[0].id); }
        }
    }

    // ── DOM Phase 2: reconstructed Time & Sales strip ──────────────────────────
    // Newest print at top, price coloured by aggressor side (green=buy/red=sell)
    // with a faint side-tinted row wash, size right-aligned. Sits in the band
    // reserved above the controls (`tape_h`), so it never overlaps the ladder.
    if tape_rows > 0 {
        let ts_top = ctrl_top - tape_h + 2.0;
        painter.line_segment(
            [egui::pos2(inner.left(), ts_top), egui::pos2(inner.right(), ts_top)],
            egui::Stroke::new(crate::ui_kit::style::stroke_std(), tint(t, Tone::Border, alpha_strong())),
        );
        // Same tier `tape_row_h` was derived from — see above.
        let tsf = TextStyle::MonoSm.font_id_in(ui);
        let hy_ts = ts_top + tape_row_h * 0.5 + 1.0;
        // Three-zone header: label (left) · session CVD (center) · speed (right).
        painter.text(
            egui::pos2(inner.left() + 4.0, hy_ts),
            egui::Align2::LEFT_CENTER, "T&S", tsf.clone(), color_muted(t.dim),
        );
        // Session cumulative delta (Σ) — whole-session order-flow bias, green
        // net-buying / red net-selling. Hidden at exactly 0 (no live prints).
        if dom_cvd != 0.0 {
            let cc = if dom_cvd > 0.0 { t.bull } else { t.bear };
            painter.text(
                egui::pos2(inner.left() + aw * 0.5, hy_ts),
                egui::Align2::CENTER_CENTER,
                &format!("\u{03A3}{:+.0}", dom_cvd), tsf.clone(), cc,
            );
        }
        // Speed-of-tape (prints/sec) — how fast the tape is running (urgency /
        // momentum). Hidden when the rate is unknown (0, e.g. a clockless feed).
        if dom_tape_speed > 0.0 {
            painter.text(
                egui::pos2(inner.right() - 4.0, hy_ts),
                egui::Align2::RIGHT_CENTER,
                &format!("{:.0}/s", dom_tape_speed), tsf.clone(), color_muted(t.accent),
            );
        }
        for (i, &(px, sz, is_buy)) in dom_tape.iter().take(tape_rows).enumerate() {
            let ry = ts_top + tape_row_h * (i as f32 + 1.0) + 1.0;
            let sidc = if is_buy { t.bull } else { t.bear };
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(inner.left(), ry), egui::vec2(aw, tape_row_h)),
                0.0, color_alpha(sidc, 14),
            );
            painter.text(
                egui::pos2(inner.left() + 4.0, ry + tape_row_h * 0.5),
                egui::Align2::LEFT_CENTER, &format!("{:.2}", px), tsf.clone(), sidc,
            );
            painter.text(
                egui::pos2(inner.right() - 4.0, ry + tape_row_h * 0.5),
                egui::Align2::RIGHT_CENTER, &fmt_size(sz as u32), tsf.clone(), color_muted(t.text),
            );
        }
    }
}

fn fmt_size(size: u32) -> String {
    if size >= 10_000 { format!("{:.1}K", size as f64 / 1_000.0) } else { format!("{}", size) }
}

/// Draw order badge text: side letter + large bold qty, high contrast against badge bg
fn draw_order_label(painter: &egui::Painter, rect: egui::Rect, side: &str, qty: u32, _color: egui::Color32) {
    let qty_str = format!("{}", qty);
    let side_font = mono_sm();
    let qty_font = mono_sm();
    let text_col = _color; // high contrast against colored badge (caller provides)
    // Side letter on the left
    painter.text(egui::pos2(rect.left() + 8.0, rect.center().y), egui::Align2::CENTER_CENTER, side, side_font, text_col);
    // Qty number, large and bold, centered in remaining space
    painter.text(egui::pos2(rect.left() + 8.0 + (rect.width() - 8.0) * 0.5, rect.center().y), egui::Align2::CENTER_CENTER, &qty_str, qty_font, text_col);
}
