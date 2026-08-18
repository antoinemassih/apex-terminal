//! Column-spec system for watchlist rows.
//!
//! Replaces the hardcoded `OptionalCols` flags so rows can be reused by other
//! list panels (scanner, holdings, etc.) with arbitrary column sets.
//!
//! Each column declares its width, label, applicability check, and a render fn
//! that paints into an x-slice rect. The row widget allocates rects across the
//! middle area and dispatches to each column's render fn in order.


use egui::{Color32, Painter, Rect, Stroke};
use serde::{Deserialize, Serialize};
use super::super::super::style::*;

type Theme = crate::chart_renderer::gpu::Theme;

/// Identity of a column. Persisted in user config as a list of these ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WatchlistColumnId {
    ChangePct,
    Sparkline,
    ExtHours,
    RvolBadge,
    DayRange,
    Week52Range,
    Volume,
    Atr,
    MarketCap,
}

/// All data a column might need to render a single row.
#[derive(Clone, Copy)]
pub struct WatchlistItemData<'a> {
    pub symbol: &'a str,
    pub price: f32,
    /// The day's percent change, or `None` when the previous close has not
    /// arrived. Never substitute `0.0` — see [`crate::foundation::market`] for
    /// what that did to the scanner's gainer/loser filters.
    pub change_pct: Option<f32>,
    pub spark: Option<&'a [f32]>,
    /// Extended-hours (pre/post-market) % move vs the last regular close.
    /// `Some` only while in extended hours.
    pub ext_change: Option<f32>,
    pub rvol: Option<f32>,
    pub range_today: Option<(f32, f32, f32)>,
    pub week52: Option<(f32, f32, f32)>,
    pub volume: Option<u64>,
    pub atr: Option<f32>,
    pub market_cap: Option<f64>,
}

impl<'a> Default for WatchlistItemData<'a> {
    fn default() -> Self {
        Self {
            symbol: "",
            price: 0.0,
            change_pct: None,
            spark: None,
            ext_change: None,
            rvol: None,
            range_today: None,
            week52: None,
            volume: None,
            atr: None,
            market_cap: None,
        }
    }
}

/// Render context handed to each column's render fn.
pub struct ColumnCtx<'a> {
    pub painter: &'a Painter,
    pub rect: Rect,
    pub theme: &'a Theme,
    pub fg: Color32,
    pub bull: Color32,
    pub bear: Color32,
    pub dim: Color32,
    pub border: Color32,
    pub item: &'a WatchlistItemData<'a>,
    pub font_size: f32,
}

pub struct WatchlistColumnSpec {
    pub id: WatchlistColumnId,
    pub label: &'static str,
    pub default_width: f32,
    pub applicable: fn(&WatchlistItemData) -> bool,
    pub render: fn(&mut ColumnCtx),
}

// ── Render helpers ──────────────────────────────────────────────────────────

/// Bull / bear / neutral for a possibly-unknown day change.
///
/// Three columns tinted themselves with `if change_pct >= 0.0 { bull } else
/// { bear }`, which has no branch for "we do not know" — so an unknown, which
/// arrived as `0.0`, painted bull green. A sparkline and a day-range dot in
/// confident green are a claim about direction; `dim` makes no claim.
fn direction_color(pct: Option<f32>, bull: Color32, bear: Color32, dim: Color32) -> Color32 {
    match pct {
        Some(p) if p >= 0.0 => bull,
        Some(_) => bear,
        None => color_dim(dim),
    }
}

fn render_change_pct(c: &mut ColumnCtx) {
    paint_change_chip(c.painter, c.rect, c.item.change_pct, c.font_size, c.bull, c.bear, c.dim);
}

/// Paint a red/green filled "chip" behind the whole change value (number + %),
/// with contrast text on top — so the highlight covers the entire figure
/// instead of just coloring the glyphs. Shared by the ChangePct column and the
/// row's inline change render.
/// `pct` is `None` when the previous close has not arrived. That case paints a
/// dim em dash and NO coloured chip, because a chip is an assertion: the old
/// signature took a plain `f32`, an unknown reached it as `0.0`, and `0.0 >= 0.0`
/// painted a confident BULL-green `+0.00%` over data the terminal did not have.
pub(crate) fn paint_change_chip(
    painter: &Painter, rect: Rect, pct: Option<f32>, font_size: f32,
    bull: Color32, bear: Color32, dim: Color32,
) {
    let Some(pct) = pct else {
        painter.text(
            egui::pos2(rect.left(), rect.center().y),
            egui::Align2::LEFT_CENTER,
            crate::foundation::market::fmt_change_pct(None),
            crate::ui_kit::style::prop_at(font_size),
            color_dim(dim),
        );
        return;
    };
    let base = if pct >= 0.0 { bull } else { bear };
    let txt = crate::foundation::market::fmt_change_pct(Some(pct));
    let font = crate::ui_kit::style::prop_at(font_size);
    let galley = painter.layout_no_wrap(txt, font.clone(), base);
    let pad = egui::vec2(3.0, 2.0);
    // CLAMP the chip to its column slot.
    //
    // The chip sized itself purely from the measured galley and ignored
    // `rect`, while the column advances by a fixed `default_width: 50.0`. At
    // the live proportional font a value like "+12.34%" measures ~58px, so the
    // chip ran ~8px INTO the next column — and the next column (Ext Hours)
    // draws flush at its own left edge, so the two collided and the change
    // figure was painted over its own neighbour.
    let max_w = rect.width().max(0.0);
    let chip_w = (galley.size().x + pad.x * 2.0).min(max_w);
    let chip = Rect::from_min_size(
        egui::pos2(rect.left(), rect.center().y - galley.size().y * 0.5 - pad.y),
        egui::vec2(chip_w, galley.size().y + pad.y * 2.0),
    );
    painter.rect_filled(chip, radius_sm(), color_alpha(base, 38));
    // Clip the text to the chip so a value too long for the slot is cut at the
    // chip edge rather than escaping it.
    painter
        .with_clip_rect(painter.clip_rect().intersect(chip))
        .galley(egui::pos2(chip.left() + pad.x, chip.top() + pad.y), galley, base);
}

/// Extended-hours (pre/post-market) % move, shown where the sparkline used to
/// live. Only drawn while `ext_change` is `Some` (i.e. in extended hours).
fn render_ext_hours(c: &mut ColumnCtx) {
    let Some(ext) = c.item.ext_change else { return; };
    // Say nothing when the extended-hours move IS the session move.
    //
    // During pre-market the feed reports the same figure for both, so the row
    // rendered the identical number twice, side by side, in two columns. A
    // second copy of a number carries no information — it just reads as a
    // rendering fault, which is exactly how it looked.
    // A `None` session change cannot be equal to the extended-hours figure, so
    // the duplicate-suppression does not apply and the ext value still shows.
    if c.item.change_pct.is_some_and(|ch| (ext - ch).abs() < 0.005) {
        return;
    }
    let col = if ext >= 0.0 { c.bull } else { c.bear };
    let s = format!("{:+.2}%", ext);
    // Dimmer than the main column so it reads as the secondary (after-hours)
    // figure; a tiny dot prefix hints "extended".
    c.painter.text(
        // Inset from the column edge. This drew flush at `rect.left()`, so it
        // sat hard against whatever the previous column ended with.
        egui::pos2(c.rect.left() + gap_xs(), c.rect.center().y),
        egui::Align2::LEFT_CENTER,
        &s,
        crate::ui_kit::style::prop_at(c.font_size - 1.0),
        color_alpha(col, crate::ui_kit::style::alpha_solid()),
    );
}

fn render_sparkline(c: &mut ColumnCtx) {
    let s = match c.item.spark { Some(s) if s.len() >= 2 => s, _ => return };
    let cy = c.rect.center().y;
    let chg_col = direction_color(c.item.change_pct, c.bull, c.bear, c.dim);
    let sw = 32.0;
    let sh = 12.0;
    let spark_rect = egui::Rect::from_min_size(
        egui::pos2(c.rect.left(), cy - sh * 0.5),
        egui::vec2(sw, sh),
    );
    crate::ui_kit::widgets::Sparkline::new(s)
        .color(color_alpha(chg_col, crate::ui_kit::style::alpha_heavy()))
        .size(sw, sh)
        .paint(c.painter, spark_rect);
}

fn render_rvol_badge(c: &mut ColumnCtx) {
    let rv = match c.item.rvol { Some(rv) if rv > 0.0 => rv, _ => return };
    let cy = c.rect.center().y;
    let rcol = if rv > 2.0 { color_alpha(c.theme.accent, alpha_heavy()) }
        else if rv > 1.2 { c.bull }
        else { color_dim(c.dim) };
    c.painter.text(
        egui::pos2(c.rect.left(), cy),
        egui::Align2::LEFT_CENTER,
        &format!("{:.1}x", rv),
        mono_sm(),
        rcol,
    );
}

fn render_day_range(c: &mut ColumnCtx) {
    let (lo, hi, last) = match c.item.range_today { Some(t) if t.1 > t.0 => t, _ => return };
    let cy = c.rect.center().y;
    let chg_col = direction_color(c.item.change_pct, c.bull, c.bear, c.dim);
    let rw = 24.0;
    let pos = ((last - lo) / (hi - lo)).clamp(0.0, 1.0);
    let x0 = c.rect.left();
    c.painter.line_segment(
        [egui::pos2(x0, cy), egui::pos2(x0 + rw, cy)],
        Stroke::new(stroke_thick(), color_alpha(c.border, alpha_muted())),
    );
    c.painter.circle_filled(egui::pos2(x0 + rw * pos, cy), 2.5, chg_col);
}

fn render_week52(c: &mut ColumnCtx) {
    let (lo, hi, last) = match c.item.week52 { Some(t) if t.1 > t.0 => t, _ => return };
    let cy = c.rect.center().y;
    let rw = 24.0;
    let pos = ((last - lo) / (hi - lo)).clamp(0.0, 1.0);
    let x0 = c.rect.left();
    c.painter.line_segment(
        [egui::pos2(x0, cy), egui::pos2(x0 + rw, cy)],
        Stroke::new(stroke_thick(), color_alpha(c.border, alpha_muted())),
    );
    c.painter.circle_filled(egui::pos2(x0 + rw * pos, cy), 2.5, c.fg);
}

fn render_volume(c: &mut ColumnCtx) {
    let v = match c.item.volume { Some(v) => v, None => return };
    let cy = c.rect.center().y;
    let s = if v >= 1_000_000_000 { format!("{:.1}B", v as f64 / 1e9) }
        else if v >= 1_000_000 { format!("{:.1}M", v as f64 / 1e6) }
        else if v >= 1_000 { format!("{:.0}K", v as f64 / 1e3) }
        else { format!("{}", v) };
    c.painter.text(
        egui::pos2(c.rect.left(), cy),
        egui::Align2::LEFT_CENTER,
        &s,
        mono_sm(),
        color_muted(c.dim),
    );
}

fn render_atr(c: &mut ColumnCtx) {
    let v = match c.item.atr { Some(v) if v > 0.0 => v, _ => return };
    let cy = c.rect.center().y;
    c.painter.text(
        egui::pos2(c.rect.left(), cy),
        egui::Align2::LEFT_CENTER,
        &format!("{:.2}", v),
        mono_sm(),
        color_muted(c.dim),
    );
}

fn render_market_cap(c: &mut ColumnCtx) {
    let v = match c.item.market_cap { Some(v) if v > 0.0 => v, _ => return };
    let cy = c.rect.center().y;
    let s = if v >= 1e12 { format!("{:.1}T", v / 1e12) }
        else if v >= 1e9 { format!("{:.1}B", v / 1e9) }
        else if v >= 1e6 { format!("{:.1}M", v / 1e6) }
        else { format!("{:.0}", v) };
    c.painter.text(
        egui::pos2(c.rect.left(), cy),
        egui::Align2::LEFT_CENTER,
        &s,
        mono_sm(),
        color_muted(c.dim),
    );
}

// ── Applicability ───────────────────────────────────────────────────────────

fn always(_: &WatchlistItemData) -> bool { true }
fn has_spark(d: &WatchlistItemData) -> bool { d.spark.map_or(false, |s| s.len() >= 2) }
fn has_ext_hours(d: &WatchlistItemData) -> bool { d.ext_change.is_some() }
fn has_rvol(d: &WatchlistItemData) -> bool { d.rvol.map_or(false, |v| v > 0.0) }
fn has_day_range(d: &WatchlistItemData) -> bool { d.range_today.map_or(false, |(l, h, _)| h > l) }
fn has_week52(d: &WatchlistItemData) -> bool { d.week52.map_or(false, |(l, h, _)| h > l) }
fn has_volume(d: &WatchlistItemData) -> bool { d.volume.is_some() }
fn has_atr(d: &WatchlistItemData) -> bool { d.atr.map_or(false, |v| v > 0.0) }
fn has_market_cap(d: &WatchlistItemData) -> bool { d.market_cap.map_or(false, |v| v > 0.0) }

pub static BUILTIN: &[WatchlistColumnSpec] = &[
    // 64, not 50. The chip is MEASURED text; 50 fitted a narrower figure and a
    // smaller face. "+12.34%" lays out at ~54px at the live proportional size,
    // plus 3px chip padding each side = ~60. At 50 the chip either overran the
    // next column (before the clamp) or truncated its own "%" (after it).
    //
    // Still a fitted constant, and it is honest to say so: it fits TODAY'S type
    // scale, not every future one. What changed is the failure mode — with the
    // clamp in `paint_change_chip`, outgrowing this width now truncates inside
    // the chip instead of painting over the neighbouring column.
    WatchlistColumnSpec { id: WatchlistColumnId::ChangePct,   label: "Change %",   default_width: 64.0, applicable: always,         render: render_change_pct },
    WatchlistColumnSpec { id: WatchlistColumnId::Sparkline,   label: "Sparkline",  default_width: 38.0, applicable: has_spark,      render: render_sparkline },
    WatchlistColumnSpec { id: WatchlistColumnId::ExtHours,    label: "Ext Hours",  default_width: 50.0, applicable: has_ext_hours,  render: render_ext_hours },
    WatchlistColumnSpec { id: WatchlistColumnId::RvolBadge,   label: "RVOL",       default_width: 26.0, applicable: has_rvol,       render: render_rvol_badge },
    WatchlistColumnSpec { id: WatchlistColumnId::DayRange,    label: "Day Range",  default_width: 30.0, applicable: has_day_range,  render: render_day_range },
    WatchlistColumnSpec { id: WatchlistColumnId::Week52Range, label: "52W Range",  default_width: 30.0, applicable: has_week52,     render: render_week52 },
    WatchlistColumnSpec { id: WatchlistColumnId::Volume,      label: "Volume",     default_width: 36.0, applicable: has_volume,     render: render_volume },
    WatchlistColumnSpec { id: WatchlistColumnId::Atr,         label: "ATR",        default_width: 32.0, applicable: has_atr,        render: render_atr },
    WatchlistColumnSpec { id: WatchlistColumnId::MarketCap,   label: "Market Cap", default_width: 40.0, applicable: has_market_cap, render: render_market_cap },
];

pub fn spec(id: WatchlistColumnId) -> &'static WatchlistColumnSpec {
    BUILTIN.iter().find(|s| s.id == id).expect("unknown WatchlistColumnId")
}

/// Default order shown to new users.
pub fn default_columns() -> Vec<WatchlistColumnId> {
    // Change% chip + Ext-Hours only by default. (Day Range — whose little
    // green/red position dot was the "weird dot" — and the others remain
    // available via the column picker but don't crowd the narrow sidebar.)
    vec![
        WatchlistColumnId::ChangePct,
        WatchlistColumnId::ExtHours,
    ]
}
