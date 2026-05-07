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
    let cy = c.rect.center().y;
    let chg_col = if c.item.change_pct >= 0.0 { c.bull } else { c.bear };
    let chg_str = format!("{:+.2}%", c.item.change_pct);
    c.painter.text(
        egui::pos2(c.rect.left(), cy),
        egui::Align2::LEFT_CENTER,
        &chg_str,
        egui::FontId::proportional(c.font_size),
        chg_col,
    );
}

fn render_sparkline(c: &mut ColumnCtx) {
    let s = match c.item.spark { Some(s) if s.len() >= 2 => s, _ => return };
    let cy = c.rect.center().y;
    let chg_col = if c.item.change_pct >= 0.0 { c.bull } else { c.bear };
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for &v in s { if v < lo { lo = v; } if v > hi { hi = v; } }
    let span = (hi - lo).max(1e-6);
    let sw = 32.0;
    let sh = 12.0;
    let sy = cy - sh * 0.5;
    let n = s.len();
    let x0_base = c.rect.left();
    for j in 1..n {
        let x0 = x0_base + (j - 1) as f32 * sw / (n - 1) as f32;
        let y0 = sy + sh - (s[j - 1] - lo) / span * sh;
        let x1 = x0_base + j as f32 * sw / (n - 1) as f32;
        let y1 = sy + sh - (s[j] - lo) / span * sh;
        c.painter.line_segment(
            [egui::pos2(x0, y0), egui::pos2(x1, y1)],
            Stroke::new(stroke_std(), color_alpha(chg_col, 120)),
        );
    }
}

fn render_rvol_badge(c: &mut ColumnCtx) {
    let rv = match c.item.rvol { Some(rv) if rv > 0.0 => rv, _ => return };
    let cy = c.rect.center().y;
    let rcol = if rv > 2.0 { color_alpha(c.theme.accent, ALPHA_HEAVY) }
        else if rv > 1.2 { c.bull }
        else { c.dim.gamma_multiply(0.4) };
    c.painter.text(
        egui::pos2(c.rect.left(), cy),
        egui::Align2::LEFT_CENTER,
        &format!("{:.1}x", rv),
        egui::FontId::monospace(11.0),
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
        Stroke::new(stroke_thick(), color_alpha(c.border, ALPHA_MUTED)),
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
        Stroke::new(stroke_thick(), color_alpha(c.border, ALPHA_MUTED)),
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
        egui::FontId::monospace(11.0),
        c.dim.gamma_multiply(0.6),
    );
}

fn render_atr(c: &mut ColumnCtx) {
    let v = match c.item.atr { Some(v) if v > 0.0 => v, _ => return };
    let cy = c.rect.center().y;
    c.painter.text(
        egui::pos2(c.rect.left(), cy),
        egui::Align2::LEFT_CENTER,
        &format!("{:.2}", v),
        egui::FontId::monospace(11.0),
        c.dim.gamma_multiply(0.6),
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
        egui::FontId::monospace(11.0),
        c.dim.gamma_multiply(0.6),
    );
}

// ── Applicability ───────────────────────────────────────────────────────────

fn always(_: &WatchlistItemData) -> bool { true }
fn has_spark(d: &WatchlistItemData) -> bool { d.spark.map_or(false, |s| s.len() >= 2) }
fn has_rvol(d: &WatchlistItemData) -> bool { d.rvol.map_or(false, |v| v > 0.0) }
fn has_day_range(d: &WatchlistItemData) -> bool { d.range_today.map_or(false, |(l, h, _)| h > l) }
fn has_week52(d: &WatchlistItemData) -> bool { d.week52.map_or(false, |(l, h, _)| h > l) }
fn has_volume(d: &WatchlistItemData) -> bool { d.volume.is_some() }
fn has_atr(d: &WatchlistItemData) -> bool { d.atr.map_or(false, |v| v > 0.0) }
fn has_market_cap(d: &WatchlistItemData) -> bool { d.market_cap.map_or(false, |v| v > 0.0) }

pub static BUILTIN: &[WatchlistColumnSpec] = &[
    WatchlistColumnSpec { id: WatchlistColumnId::ChangePct,   label: "Change %",   default_width: 70.0, applicable: always,         render: render_change_pct },
    WatchlistColumnSpec { id: WatchlistColumnId::Sparkline,   label: "Sparkline",  default_width: 38.0, applicable: has_spark,      render: render_sparkline },
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
        WatchlistColumnId::Sparkline,
        WatchlistColumnId::RvolBadge,
        WatchlistColumnId::DayRange,
    ]
}
