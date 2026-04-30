//! WatchlistRow — symbol + price + change% with optional decorations.
//!
//! Rich variant matches the inline stock-row rendering used in
//! `watchlist_panel.rs`: RVOL left strip, drag-handle grip, star pin, earnings
//! pill, alert bell, correlation dot, optional sparkline / range bar / 52wk
//! position columns, extreme-move tint, active-row accent stripe, and
//! compact (pinned) mode with font-size overrides.
//!
//! Built on `RowShell` painter mode — the body owns painter geometry while
//! the shell handles base fill + hover/selected overlays.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, Stroke, Ui, Widget};
use super::super::super::style::*;
use super::super::foundation::{
    interaction::InteractionState,
    shell::RowShell,
    tokens::Size,
    variants::RowVariant,
};
use super::ListRow;

type Theme = crate::chart_renderer::gpu::Theme;

/// Pin state for the star icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinState {
    /// Not pinned; star hidden by default (shown on hover when `show_star_on_hover`).
    NotPinned,
    /// Pinned; star always visible (gold).
    Pinned,
}

impl Default for PinState {
    fn default() -> Self { PinState::NotPinned }
}

/// Toggles for which optional middle-section columns appear.
#[derive(Clone, Copy, Debug, Default)]
pub struct OptionalCols {
    pub sparkline: bool,
    pub range_bar: bool,
    pub week52: bool,
    pub rvol_badge: bool,
}

/// Fallback theme for theme-less callers — first registered project theme.
fn fallback_theme() -> &'static Theme {
    &crate::chart_renderer::gpu::THEMES[0]
}

#[must_use = "WatchlistRow must be finalized with `.show(ui)` to render"]
pub struct WatchlistRow<'a> {
    symbol: &'a str,
    price: f32,
    change_pct: f32,

    // Existing decorations.
    spark: Option<&'a [f32]>,
    selected: bool,
    height: f32,

    // Theme.
    theme: Option<&'a Theme>,
    theme_bg: Option<Color32>,
    theme_border: Option<Color32>,
    theme_accent: Option<Color32>,
    theme_bull: Option<Color32>,
    theme_bear: Option<Color32>,
    theme_dim: Option<Color32>,
    theme_fg: Option<Color32>,

    // New rich-row fields.
    rvol: Option<f32>,
    drag_handle: bool,
    pin_state: PinState,
    show_star_on_hover: bool,
    earnings_days: Option<u32>,
    alert_indicator: bool,
    correlation_dot: Option<f32>,
    optional_cols: OptionalCols,
    range_today: Option<(f32, f32, f32)>, // (low, high, last)
    week52: Option<(f32, f32, f32)>,      // (low, high, last)
    compact: bool,
    extreme_move: Option<f32>,            // signed change_pct vs avg_daily_range; tint applied if abs(change)>1.5*avg
    avg_daily_range: f32,
    active: bool,
    font_size_override: Option<f32>,
}

impl<'a> WatchlistRow<'a> {
    pub fn new(symbol: &'a str, price: f32, change_pct: f32) -> Self {
        Self {
            symbol, price, change_pct,
            spark: None, selected: false, height: 22.0,
            theme: None,
            theme_bg: None, theme_border: None, theme_accent: None,
            theme_bull: None, theme_bear: None, theme_dim: None, theme_fg: None,
            rvol: None,
            drag_handle: false,
            pin_state: PinState::NotPinned,
            show_star_on_hover: false,
            earnings_days: None,
            alert_indicator: false,
            correlation_dot: None,
            optional_cols: OptionalCols::default(),
            range_today: None,
            week52: None,
            compact: false,
            extreme_move: None,
            avg_daily_range: 0.0,
            active: false,
            font_size_override: None,
        }
    }
    pub fn spark(mut self, s: &'a [f32]) -> Self { self.spark = Some(s); self }
    pub fn selected(mut self, v: bool) -> Self { self.selected = v; self }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn theme(mut self, t: &'a Theme) -> Self {
        self.theme = Some(t);
        self.theme_bg = Some(t.toolbar_bg);
        self.theme_border = Some(t.toolbar_border);
        self.theme_accent = Some(t.accent);
        self.theme_bull = Some(t.bull);
        self.theme_bear = Some(t.bear);
        self.theme_dim = Some(t.dim);
        self.theme_fg = Some(t.text);
        self
    }

    // ── Rich-row builders ────────────────────────────────────────────────
    pub fn rvol(mut self, v: Option<f32>) -> Self { self.rvol = v; self }
    pub fn drag_handle(mut self, v: bool) -> Self { self.drag_handle = v; self }
    pub fn pin_state(mut self, v: PinState) -> Self { self.pin_state = v; self }
    pub fn show_star_on_hover(mut self, v: bool) -> Self { self.show_star_on_hover = v; self }
    pub fn earnings_days(mut self, v: Option<u32>) -> Self { self.earnings_days = v; self }
    pub fn alert_indicator(mut self, v: bool) -> Self { self.alert_indicator = v; self }
    pub fn correlation_dot(mut self, v: Option<f32>) -> Self { self.correlation_dot = v; self }
    pub fn optional_columns(mut self, c: OptionalCols) -> Self { self.optional_cols = c; self }
    pub fn range_bar(mut self, low_today: f32, high_today: f32, last: f32) -> Self {
        self.range_today = Some((low_today, high_today, last)); self
    }
    pub fn week52_pos(mut self, low: f32, high: f32, last: f32) -> Self {
        self.week52 = Some((low, high, last)); self
    }
    pub fn compact(mut self, v: bool) -> Self { self.compact = v; self }
    /// Provide avg_daily_range; if `Some(change_pct)` exceeds 1.5x of it the
    /// row paints a green/red full-row tint.
    pub fn extreme_move_tint(mut self, avg_daily_range: Option<f32>) -> Self {
        if let Some(adr) = avg_daily_range {
            self.avg_daily_range = adr;
            self.extreme_move = Some(self.change_pct);
        }
        self
    }
    pub fn active(mut self, v: bool) -> Self { self.active = v; self }
    pub fn font_size_override(mut self, sz: f32) -> Self { self.font_size_override = Some(sz); self }

    pub fn show(self, ui: &mut Ui) -> Response {
        let theme_ref: &Theme = match self.theme { Some(t) => t, None => fallback_theme() };
        let bull = self.theme_bull.unwrap_or(Color32::from_rgb(0, 200, 120));
        let bear = self.theme_bear.unwrap_or(Color32::from_rgb(220, 80, 80));
        let dim = self.theme_dim.unwrap_or(Color32::from_gray(120));
        let fg = self.theme_fg.unwrap_or(Color32::from_gray(220));
        let accent = self.theme_accent.unwrap_or(Color32::from_rgb(80, 140, 220));
        let border = self.theme_border.unwrap_or(Color32::from_gray(60));
        let symbol = self.symbol;
        let price = self.price;
        let change_pct = self.change_pct;
        let spark = self.spark;

        // Resolve sizing — compact pinned rows are taller (34px) per panel.
        let row_h = if self.compact { 34.0 } else { self.height };
        let font_sz = self.font_size_override.unwrap_or(if self.compact { 15.0 } else { 14.0 });

        let rvol = self.rvol;
        let drag_handle = self.drag_handle;
        let pin_state = self.pin_state;
        let show_star_on_hover = self.show_star_on_hover;
        let earnings_days = self.earnings_days;
        let alert_indicator = self.alert_indicator;
        let correlation_dot = self.correlation_dot;
        let optional_cols = self.optional_cols;
        let range_today = self.range_today;
        let week52 = self.week52;
        let extreme_move = self.extreme_move;
        let avg_daily_range = self.avg_daily_range;
        let active_flag = self.active;

        let resp = RowShell::new(theme_ref, "")
            .variant(RowVariant::Default)
            .size(Size::Md)
            .state(InteractionState::default().selected(self.selected))
            .painter_mode(true)
            .painter_height(row_h)
            .painter_body(move |ui, rect| {
                let painter = ui.painter();
                let cy = rect.center().y;
                let chg_col = if change_pct >= 0.0 { bull } else { bear };

                // ── Extreme-move full-row tint ───────────────────────────
                if let Some(chg) = extreme_move {
                    if avg_daily_range > 0.0 && chg.abs() > avg_daily_range * 1.5 {
                        let tint = if chg >= 0.0 {
                            Color32::from_rgba_unmultiplied(46, 204, 113, ALPHA_GHOST)
                        } else {
                            Color32::from_rgba_unmultiplied(231, 76, 60, ALPHA_GHOST)
                        };
                        painter.rect_filled(rect, 0.0, tint);
                    }
                }

                // ── Active-row 2.5px accent left stripe ─────────────────
                if active_flag {
                    painter.rect_filled(
                        egui::Rect::from_min_max(rect.min, egui::pos2(rect.min.x + 2.5, rect.max.y)),
                        1.0, accent);
                }

                // ── RVOL left-border strip ──────────────────────────────
                if let Some(rv) = rvol {
                    let (rcol, rw) = if rv > 3.0 {
                        (Color32::from_rgba_unmultiplied(240, 160, 40, 220), 4.0)
                    } else if rv > 2.0 {
                        (Color32::from_rgba_unmultiplied(240, 160, 40, 160), 3.0)
                    } else if rv > 0.8 {
                        (Color32::from_rgba_unmultiplied(46, 204, 113, ALPHA_ACTIVE), 2.0)
                    } else {
                        (Color32::from_rgba_unmultiplied(100, 150, 255, ALPHA_STRONG), 2.0)
                    };
                    painter.rect_filled(
                        egui::Rect::from_min_size(rect.min, egui::vec2(rw, rect.height())),
                        0.0, rcol);
                }

                let left = rect.left();
                let mut sym_x = left + 8.0;

                // ── Drag-handle grip ────────────────────────────────────
                if drag_handle {
                    painter.text(egui::pos2(left + 6.0, cy), egui::Align2::LEFT_CENTER,
                        "\u{2807}", egui::FontId::proportional(9.0), dim.gamma_multiply(0.2));
                    sym_x = left + 18.0;
                }

                // ── Star pin ────────────────────────────────────────────
                let show_star = matches!(pin_state, PinState::Pinned)
                    || (show_star_on_hover); // body has no hover info; controller decides
                if show_star {
                    let star_col = match pin_state {
                        PinState::Pinned => Color32::from_rgb(255, 193, 37),
                        PinState::NotPinned => dim.gamma_multiply(0.3),
                    };
                    let star_x = sym_x;
                    painter.text(egui::pos2(star_x, cy), egui::Align2::CENTER_CENTER,
                        "\u{2605}", egui::FontId::proportional(9.0), star_col);
                    sym_x += 10.0;
                }

                // ── Symbol ──────────────────────────────────────────────
                painter.text(egui::pos2(sym_x, cy), egui::Align2::LEFT_CENTER,
                    symbol, egui::FontId::monospace(font_sz), fg);
                let mut ind_x = sym_x + symbol.len() as f32 * 8.5 + 6.0;

                // ── Earnings pill ───────────────────────────────────────
                if let Some(days) = earnings_days {
                    if days <= 14 {
                        let e_text = format!("E:{}", days);
                        let e_galley = painter.layout_no_wrap(e_text.clone(),
                            egui::FontId::monospace(7.0), Color32::BLACK);
                        let pw = e_galley.size().x + 6.0;
                        painter.rect_filled(
                            egui::Rect::from_min_size(egui::pos2(ind_x, cy - 6.0), egui::vec2(pw, 12.0)),
                            6.0, Color32::from_rgb(255, 193, 37));
                        painter.text(egui::pos2(ind_x + pw / 2.0, cy), egui::Align2::CENTER_CENTER,
                            &e_text, egui::FontId::monospace(7.0), Color32::BLACK);
                        ind_x += pw + 3.0;
                    }
                }

                // ── Alert bell ──────────────────────────────────────────
                if alert_indicator {
                    painter.circle_filled(egui::pos2(ind_x + 5.0, cy), 5.5,
                        Color32::from_rgb(231, 76, 60));
                    painter.text(egui::pos2(ind_x + 5.0, cy), egui::Align2::CENTER_CENTER,
                        "!", egui::FontId::proportional(7.0), Color32::WHITE);
                    ind_x += 14.0;
                }

                // ── Correlation dot ─────────────────────────────────────
                if let Some(corr) = correlation_dot {
                    let dot_col = if corr >= 0.5 { bull }
                        else if corr <= -0.5 { bear }
                        else { dim.gamma_multiply(0.5) };
                    painter.circle_filled(egui::pos2(ind_x + 5.0, cy), 3.0, dot_col);
                    ind_x += 12.0;
                }
                let _ = ind_x;

                // ── Change % (mid column) ───────────────────────────────
                let mid_x = rect.left() + rect.width() * 0.45;
                let chg_str = format!("{:+.2}%", change_pct);
                painter.text(egui::pos2(mid_x, cy), egui::Align2::LEFT_CENTER,
                    &chg_str, egui::FontId::proportional(font_sz), chg_col);
                let mut extra_x = mid_x + chg_str.len() as f32 * 8.0 + 8.0;

                // ── Optional sparkline ──────────────────────────────────
                if optional_cols.sparkline {
                    if let Some(s) = spark {
                        if s.len() >= 2 {
                            let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
                            for &v in s { if v < lo { lo = v; } if v > hi { hi = v; } }
                            let span = (hi - lo).max(1e-6);
                            let sw = 32.0;
                            let sh = 12.0;
                            let sy = cy - sh * 0.5;
                            let n = s.len();
                            for j in 1..n {
                                let x0 = extra_x + (j - 1) as f32 * sw / (n - 1) as f32;
                                let y0 = sy + sh - (s[j - 1] - lo) / span * sh;
                                let x1 = extra_x + j as f32 * sw / (n - 1) as f32;
                                let y1 = sy + sh - (s[j] - lo) / span * sh;
                                painter.line_segment([egui::pos2(x0, y0), egui::pos2(x1, y1)],
                                    Stroke::new(1.0, color_alpha(chg_col, 120)));
                            }
                            extra_x += sw + 6.0;
                        }
                    }
                }

                // ── Optional RVOL badge ─────────────────────────────────
                if optional_cols.rvol_badge {
                    if let Some(rv) = rvol {
                        if rv > 0.0 {
                            let rcol = if rv > 2.0 { Color32::from_rgb(255, 193, 37) }
                                else if rv > 1.2 { bull }
                                else { dim.gamma_multiply(0.4) };
                            painter.text(egui::pos2(extra_x, cy), egui::Align2::LEFT_CENTER,
                                &format!("{:.1}x", rv), egui::FontId::monospace(7.0), rcol);
                            extra_x += 26.0;
                        }
                    }
                }

                // ── Optional intraday range bar ─────────────────────────
                if optional_cols.range_bar {
                    if let Some((lo, hi, last)) = range_today {
                        if hi > lo {
                            let rw = 24.0;
                            let pos = ((last - lo) / (hi - lo)).clamp(0.0, 1.0);
                            painter.line_segment(
                                [egui::pos2(extra_x, cy), egui::pos2(extra_x + rw, cy)],
                                Stroke::new(2.0, color_alpha(border, ALPHA_MUTED)));
                            painter.circle_filled(egui::pos2(extra_x + rw * pos, cy), 2.5, chg_col);
                            extra_x += rw + 6.0;
                        }
                    }
                }

                // ── Optional 52-week position dot ───────────────────────
                if optional_cols.week52 {
                    if let Some((lo, hi, last)) = week52 {
                        if hi > lo {
                            let rw = 24.0;
                            let pos = ((last - lo) / (hi - lo)).clamp(0.0, 1.0);
                            painter.line_segment(
                                [egui::pos2(extra_x, cy), egui::pos2(extra_x + rw, cy)],
                                Stroke::new(2.0, color_alpha(border, ALPHA_MUTED)));
                            painter.circle_filled(egui::pos2(extra_x + rw * pos, cy), 2.5, fg);
                            extra_x += rw + 6.0;
                        }
                    }
                }
                let _ = extra_x;

                // ── Price (right-aligned) ───────────────────────────────
                let price_str = format!("{:.2}", price);
                painter.text(
                    egui::pos2(rect.right() - 8.0, cy), egui::Align2::RIGHT_CENTER,
                    &price_str, egui::FontId::proportional(font_sz), fg,
                );
            })
            .show(ui);

        crate::design_tokens::register_hit(
            [resp.rect.min.x, resp.rect.min.y, resp.rect.width(), resp.rect.height()],
            "WATCHLIST_ROW", "Rows",
        );
        resp
    }
}
