//! Column-spec system for watchlist rows.
//!
//! Replaces the hardcoded `OptionalCols` flags so rows can be reused by other
//! list panels (scanner, holdings, etc.) with arbitrary column sets.
//!
//! Each column declares its width, label, applicability check, and a render fn
//! that paints into an x-slice rect. The row widget allocates rects across the
//! middle area and dispatches to each column's render fn in order.

#![allow(dead_code)]

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
    pub change_pct: f32,
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
            change_pct: 0.0,
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

fn render_change_pct(c: &mut ColumnCtx) {
    paint_change_chip(c.painter, c.rect, c.item.change_pct, c.font_size, c.bull, c.bear);
}

/// Paint a red/green filled "chip" behind the whole change value (number + %),
/// with contrast text on top — so the highlight covers the entire figure
/// instead of just coloring the glyphs. Shared by the ChangePct column and the
/// row's inline change render.
pub(crate) fn paint_change_chip(
    painter: &Painter, rect: Rect, pct: f32, font_size: f32, bull: Color32, bear: Color32,
) {
    let base = if pct >= 0.0 { bull } else { bear };
    let txt = format!("{:+.2}%", pct);
    let font = egui::FontId::proportional(font_size);
    let galley = painter.layout_no_wrap(txt, font.clone(), base);
    let pad = egui::vec2(5.0, 2.0);
    let chip = Rect::from_min_size(
        egui::pos2(rect.left(), rect.center().y - galley.size().y * 0.5 - pad.y),
        galley.size() + pad * 2.0,
    );
    painter.rect_filled(chip, 4.0, color_alpha(base, 38));
    painter.galley(egui::pos2(chip.left() + pad.x, chip.top() + pad.y), galley, base);
}

/// Extended-hours (pre/post-market) % move, shown where the sparkline used to
/// live. Only drawn while `ext_change` is `Some` (i.e. in extended hours).
fn render_ext_hours(c: &mut ColumnCtx) {
    let Some(ext) = c.item.ext_change else { return; };
    let col = if ext >= 0.0 { c.bull } else { c.bear };
    let s = format!("{:+.2}%", ext);
    // Dimmer than the main column so it reads as the secondary (after-hours)
    // figure; a tiny dot prefix hints "extended".
    c.painter.text(
        egui::pos2(c.rect.left(), c.rect.center().y),
        egui::Align2::LEFT_CENTER,
        &s,
        egui::FontId::proportional(c.font_size - 1.0),
        color_alpha(col, 200),
    );
}

fn render_sparkline(c: &mut ColumnCtx) {
    let s = match c.item.spark { Some(s) if s.len() >= 2 => s, _ => return };
    let cy = c.rect.center().y;
    let chg_col = if c.item.change_pct >= 0.0 { c.bull } else { c.bear };
    let sw = 32.0;
    let sh = 12.0;
    let spark_rect = egui::Rect::from_min_size(
        egui::pos2(c.rect.left(), cy - sh * 0.5),
        egui::vec2(sw, sh),
    );
    crate::ui_kit::widgets::Sparkline::new(s)
        .color(color_alpha(chg_col, 120))
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
    let chg_col = if c.item.change_pct >= 0.0 { c.bull } else { c.bear };
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
    WatchlistColumnSpec { id: WatchlistColumnId::ChangePct,   label: "Change %",   default_width: 70.0, applicable: always,         render: render_change_pct },
    WatchlistColumnSpec { id: WatchlistColumnId::Sparkline,   label: "Sparkline",  default_width: 38.0, applicable: has_spark,      render: render_sparkline },
    WatchlistColumnSpec { id: WatchlistColumnId::ExtHours,    label: "Ext Hours",  default_width: 58.0, applicable: has_ext_hours,  render: render_ext_hours },
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
    vec![
        WatchlistColumnId::ChangePct,
        WatchlistColumnId::ExtHours,
        WatchlistColumnId::DayRange,
    ]
}
