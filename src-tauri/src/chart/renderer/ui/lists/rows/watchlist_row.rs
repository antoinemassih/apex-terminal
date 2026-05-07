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

use egui::{Color32, Rect, Response, Sense, Stroke, Ui, Widget};
use std::cell::RefCell;
use std::rc::Rc;
use super::super::super::style::*;
use crate::chart::renderer::ui::foundation::{
    interaction::InteractionState,
    shell::RowShell,
    tokens::Size,
    variants::RowVariant,
};
use crate::chart::renderer::ui::widgets::rows::ListRow;
use super::watchlist_columns::{
    spec as col_spec, ColumnCtx, WatchlistColumnId, WatchlistItemData,
};

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

/// Glyphs used for in-row decorations. Defaults are unicode escapes used by
/// the standalone widget; the watchlist panel overrides these with project
/// `Icon::*` constants (DOTS_SIX_VERTICAL, SPARKLE, X, LIGHTNING) so the row
/// matches the rest of the terminal chrome.
#[derive(Clone, Copy, Debug)]
pub struct IconSet {
    pub drag_handle: &'static str,
    pub star: &'static str,
    pub x: &'static str,
    pub alert: &'static str,
}

impl Default for IconSet {
    fn default() -> Self {
        Self {
            drag_handle: "\u{2807}",
            star: "\u{2605}",
            x: "\u{00D7}",
            alert: "!",
        }
    }
}

/// Hit-tested zone within a watchlist row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WatchlistRowZone {
    #[default]
    None,
    Body,
    Star,
    X,
    DragHandle,
    Alert,
    Earnings,
}

/// Rich response returned by `WatchlistRow::show`.
pub struct WatchlistRowResponse {
    pub response: Response,
    pub star_clicked: bool,
    pub x_clicked: bool,
    pub drag_started: bool,
    pub alert_clicked: bool,
    pub earnings_clicked: bool,
    pub hovered_zone: WatchlistRowZone,
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
    columns: &'a [WatchlistColumnId],
    range_today: Option<(f32, f32, f32)>, // (low, high, last) — column data
    week52: Option<(f32, f32, f32)>,      // (low, high, last) — column data
    volume_v: Option<u64>,
    atr_v: Option<f32>,
    market_cap_v: Option<f64>,
    compact: bool,
    extreme_move: Option<f32>,            // signed change_pct vs avg_daily_range; tint applied if abs(change)>1.5*avg
    avg_daily_range: f32,
    active: bool,
    font_size_override: Option<f32>,

    // Project-decoration knobs (panel-specific look).
    icon_set: IconSet,
    sense: Sense,
    row_tint: Option<Color32>,
    separator: bool,
    hover_overlay: Option<Color32>,
    show_x_on_hover: bool,
    drag_confirmed: bool,
    sym_font_id: Option<egui::FontId>,
    chg_font_id: Option<egui::FontId>,
    price_font_id: Option<egui::FontId>,
    price_str_override: Option<String>,
    price_right_inset: f32,
    star_x_offset: f32,
    sym_x_offset: f32,
    sym_x_offset_no_star: f32,
    fg_override: Option<Color32>,
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
            columns: &[],
            range_today: None,
            week52: None,
            volume_v: None,
            atr_v: None,
            market_cap_v: None,
            compact: false,
            extreme_move: None,
            avg_daily_range: 0.0,
            active: false,
            font_size_override: None,
            icon_set: IconSet::default(),
            sense: Sense::click(),
            row_tint: None,
            separator: false,
            hover_overlay: None,
            show_x_on_hover: false,
            drag_confirmed: false,
            sym_font_id: None,
            chg_font_id: None,
            price_font_id: None,
            price_str_override: None,
            price_right_inset: 8.0,
            star_x_offset: 0.0,
            sym_x_offset: 10.0,
            sym_x_offset_no_star: 10.0,
            fg_override: None,
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
    /// Specify which columns to render in the middle area, in order.
    pub fn columns(mut self, cols: &'a [WatchlistColumnId]) -> Self { self.columns = cols; self }
    pub fn day_range(mut self, low: f32, high: f32, last: f32) -> Self {
        self.range_today = Some((low, high, last)); self
    }
    pub fn week52(mut self, low: f32, high: f32, last: f32) -> Self {
        self.week52 = Some((low, high, last)); self
    }
    pub fn volume(mut self, v: u64) -> Self { self.volume_v = Some(v); self }
    pub fn atr(mut self, v: f32) -> Self { self.atr_v = Some(v); self }
    pub fn market_cap(mut self, v: f64) -> Self { self.market_cap_v = Some(v); self }
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

    // ── Project-decoration knobs ────────────────────────────────────────
    pub fn icon_set(mut self, s: IconSet) -> Self { self.icon_set = s; self }
    pub fn sense(mut self, s: Sense) -> Self { self.sense = s; self }
    pub fn row_tint(mut self, c: Color32) -> Self { self.row_tint = Some(c); self }
    pub fn separator(mut self, v: bool) -> Self { self.separator = v; self }
    pub fn hover_overlay(mut self, c: Color32) -> Self { self.hover_overlay = Some(c); self }
    pub fn show_x_on_hover(mut self, v: bool) -> Self { self.show_x_on_hover = v; self }
    /// When true, hover-only effects (X glyph, hover overlay, hover star, cursor)
    /// are suppressed. Mirrors panel's `drag_confirmed` gating.
    pub fn drag_confirmed(mut self, v: bool) -> Self { self.drag_confirmed = v; self }
    pub fn sym_font(mut self, f: egui::FontId) -> Self { self.sym_font_id = Some(f); self }
    pub fn chg_font(mut self, f: egui::FontId) -> Self { self.chg_font_id = Some(f); self }
    pub fn price_font(mut self, f: egui::FontId) -> Self { self.price_font_id = Some(f); self }
    pub fn price_string(mut self, s: String) -> Self { self.price_str_override = Some(s); self }
    pub fn price_right_inset(mut self, px: f32) -> Self { self.price_right_inset = px; self }
    /// Override the foreground (symbol + price) colour. Used by pinned rows
    /// to render active-row symbol text in white.
    pub fn fg(mut self, c: Color32) -> Self { self.fg_override = Some(c); self }
    pub fn sym_layout(mut self, star_x_offset: f32, sym_x_after_star: f32, sym_x_no_star: f32) -> Self {
        self.star_x_offset = star_x_offset;
        self.sym_x_offset = sym_x_after_star;
        self.sym_x_offset_no_star = sym_x_no_star;
        self
    }

    pub fn show(self, ui: &mut Ui) -> WatchlistRowResponse {
        let theme_ref: &Theme = match self.theme { Some(t) => t, None => fallback_theme() };
        let bull = self.theme_bull.unwrap_or(fallback_theme().bull);
        let bear = self.theme_bear.unwrap_or(fallback_theme().bear);
        let dim = self.theme_dim.unwrap_or(fallback_theme().dim);
        let fg = self.fg_override.unwrap_or_else(|| self.theme_fg.unwrap_or(fallback_theme().text));
        let accent = self.theme_accent.unwrap_or(fallback_theme().accent);
        let border = self.theme_border.unwrap_or(fallback_theme().toolbar_border);
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
        let columns: Vec<WatchlistColumnId> = self.columns.to_vec();
        let range_today = self.range_today;
        let week52 = self.week52;
        let volume_v = self.volume_v;
        let atr_v = self.atr_v;
        let market_cap_v = self.market_cap_v;
        let extreme_move = self.extreme_move;
        let avg_daily_range = self.avg_daily_range;
        let active_flag = self.active;

        // Project decoration locals (moved into body).
        let icon_set = self.icon_set;
        let row_tint = self.row_tint;
        let separator_on = self.separator;
        let sym_font_id = self.sym_font_id.clone()
            .unwrap_or_else(|| egui::FontId::monospace(font_sz));
        let chg_font_id = self.chg_font_id.clone()
            .unwrap_or_else(|| egui::FontId::proportional(font_sz));
        let price_font_id = self.price_font_id.clone()
            .unwrap_or_else(|| egui::FontId::proportional(font_sz));
        let price_str_override = self.price_str_override.clone();
        let price_right_inset = self.price_right_inset;
        let star_x_offset = self.star_x_offset;
        let sym_x_offset_after_star = self.sym_x_offset;
        let sym_x_offset_no_star = self.sym_x_offset_no_star;
        let drag_confirmed = self.drag_confirmed;
        let show_star_on_hover_flag = self.show_star_on_hover;
        let self_show_x_on_hover = self.show_x_on_hover;
        let hover_overlay_col = self.hover_overlay;
        let user_sense = self.sense;

        // Pre-compute hover so the body knows whether to paint hover-conditional
        // glyphs (star, X). Use the cursor position + available_width + row_h to
        // build the same rect RowShell will allocate.
        let est_top_left = ui.cursor().min;
        let est_rect = egui::Rect::from_min_size(
            est_top_left,
            egui::vec2(ui.available_width(), row_h),
        );
        let pointer_pos_pre = ui.ctx().pointer_hover_pos();
        let pre_hovered = pointer_pos_pre
            .map(|p| est_rect.contains(p))
            .unwrap_or(false)
            && ui.is_enabled();

        // Shared cell so the painter body can publish per-zone rects we hit-test
        // post-show against the captured pointer position.
        #[derive(Default, Clone, Copy)]
        struct ZoneRects {
            drag: Option<Rect>,
            star: Option<Rect>,
            earnings: Option<Rect>,
            alert: Option<Rect>,
            x: Option<Rect>,
        }
        let zones: Rc<RefCell<ZoneRects>> = Rc::new(RefCell::new(ZoneRects::default()));
        let zones_body = zones.clone();

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

                // ── Project row tint (e.g. pinned-row faint bg) ─────────
                if let Some(tint) = row_tint {
                    painter.rect_filled(rect, 0.0, tint);
                }

                // ── Extreme-move full-row tint ───────────────────────────
                if let Some(chg) = extreme_move {
                    if avg_daily_range > 0.0 && chg.abs() > avg_daily_range * 1.5 {
                        let tint = if chg >= 0.0 {
                            color_alpha(bull, ALPHA_GHOST)
                        } else {
                            color_alpha(bear, ALPHA_GHOST)
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
                        (color_alpha(theme_ref.accent, 220), 4.0)
                    } else if rv > 2.0 {
                        (color_alpha(theme_ref.accent, 160), 3.0)
                    } else if rv > 0.8 {
                        (color_alpha(bull, ALPHA_ACTIVE), 2.0)
                    } else {
                        (color_alpha(accent, ALPHA_STRONG), 2.0)
                    };
                    painter.rect_filled(
                        egui::Rect::from_min_size(rect.min, egui::vec2(rw, rect.height())),
                        0.0, rcol);
                }

                let left = rect.left();

                // ── Drag-handle grip ────────────────────────────────────
                if drag_handle {
                    painter.text(egui::pos2(left + 6.0, cy), egui::Align2::LEFT_CENTER,
                        icon_set.drag_handle, egui::FontId::proportional(11.0), dim.gamma_multiply(0.2));
                    zones_body.borrow_mut().drag = Some(egui::Rect::from_min_size(
                        egui::pos2(left, rect.top()), egui::vec2(14.0, rect.height())));
                }

                // ── Star pin ────────────────────────────────────────────
                // Visible when pinned, OR (hovered && show_star_on_hover && !drag_confirmed).
                let show_star = matches!(pin_state, PinState::Pinned)
                    || (show_star_on_hover_flag && pre_hovered && !drag_confirmed);
                let star_visible_here = show_star;
                if star_visible_here {
                    let star_col = match pin_state {
                        PinState::Pinned => color_alpha(theme_ref.accent, ALPHA_HEAVY),
                        PinState::NotPinned => dim.gamma_multiply(0.3),
                    };
                    let star_x = left + 16.0 + star_x_offset;
                    painter.text(egui::pos2(star_x, cy), egui::Align2::CENTER_CENTER,
                        icon_set.star, egui::FontId::proportional(11.0), star_col);
                }
                // Star click-zone always covers left..left+26 when pinned-or-hoverable
                // so panel-style click partitioning works.
                if matches!(pin_state, PinState::Pinned) || show_star_on_hover_flag {
                    zones_body.borrow_mut().star = Some(egui::Rect::from_min_max(
                        egui::pos2(left, rect.top()),
                        egui::pos2(left + 26.0, rect.bottom()),
                    ));
                }

                // ── Symbol ──────────────────────────────────────────────
                let sym_x = if star_visible_here {
                    left + 16.0 + star_x_offset + sym_x_offset_after_star
                } else {
                    left + sym_x_offset_no_star
                };
                painter.text(egui::pos2(sym_x, cy), egui::Align2::LEFT_CENTER,
                    symbol, sym_font_id.clone(), fg);
                let mut ind_x = sym_x + symbol.len() as f32 * 8.5 + 6.0;

                // ── Earnings pill ───────────────────────────────────────
                if let Some(days) = earnings_days {
                    if days <= 14 {
                        let e_text = format!("E:{}", days);
                        let e_galley = painter.layout_no_wrap(e_text.clone(),
                            egui::FontId::monospace(11.0), Color32::BLACK);
                        let pw = e_galley.size().x + 6.0;
                        let pill_rect = egui::Rect::from_min_size(
                            egui::pos2(ind_x, cy - 6.0), egui::vec2(pw, 12.0));
                        painter.rect_filled(pill_rect, 6.0, color_alpha(theme_ref.accent, ALPHA_HEAVY));
                        painter.text(egui::pos2(ind_x + pw / 2.0, cy), egui::Align2::CENTER_CENTER,
                            &e_text, egui::FontId::monospace(11.0), Color32::BLACK);
                        zones_body.borrow_mut().earnings = Some(pill_rect);
                        ind_x += pw + 3.0;
                    }
                }

                // ── Alert bell ──────────────────────────────────────────
                if alert_indicator {
                    painter.circle_filled(egui::pos2(ind_x + 5.0, cy), 5.5,
                        theme_ref.bear);
                    painter.text(egui::pos2(ind_x + 5.0, cy), egui::Align2::CENTER_CENTER,
                        icon_set.alert, egui::FontId::proportional(11.0), Color32::WHITE);
                    zones_body.borrow_mut().alert = Some(egui::Rect::from_center_size(
                        egui::pos2(ind_x + 5.0, cy), egui::vec2(12.0, 12.0)));
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

                // ── Column-spec dispatch ────────────────────────────────
                // Build the per-row item-data view from row-level fields, then
                // allocate x-slices across the middle area (left = end of
                // indicator strip, right = price column inset).
                let item_data = WatchlistItemData {
                    symbol,
                    price,
                    change_pct,
                    spark,
                    rvol,
                    range_today,
                    week52,
                    volume: volume_v,
                    atr: atr_v,
                    market_cap: market_cap_v,
                };
                if !columns.is_empty() {
                    // Middle area starts after the indicator strip; for legacy
                    // visual parity with the old hand-tuned mid_x = 45% layout,
                    // start at max(ind_x + gap, rect.left()+45%).
                    let middle_left = (rect.left() + rect.width() * 0.45).max(ind_x + 4.0);
                    let middle_right = rect.right() - price_right_inset - 60.0;
                    let mut x = middle_left;
                    let gap = 6.0;
                    for cid in columns.iter().copied() {
                        let s = col_spec(cid);
                        if !(s.applicable)(&item_data) { continue; }
                        let w = s.default_width;
                        if x + w > middle_right { break; }
                        let col_rect = egui::Rect::from_min_max(
                            egui::pos2(x, rect.top()),
                            egui::pos2(x + w, rect.bottom()),
                        );
                        let mut cctx = ColumnCtx {
                            painter,
                            rect: col_rect,
                            theme: theme_ref,
                            fg, bull, bear, dim, border,
                            item: &item_data,
                            font_size: font_sz,
                        };
                        // ChangePct uses the row's chg_font_id for parity with
                        // the legacy renderer; override by re-painting here so
                        // proportional/monospace font is honored.
                        if matches!(cid, WatchlistColumnId::ChangePct) {
                            let chg_str = format!("{:+.2}%", change_pct);
                            painter.text(
                                egui::pos2(col_rect.left(), col_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                &chg_str,
                                chg_font_id.clone(),
                                chg_col,
                            );
                        } else {
                            (s.render)(&mut cctx);
                        }
                        x += w + gap;
                    }
                }

                // ── Price (right-aligned) ───────────────────────────────
                let price_str = price_str_override
                    .clone()
                    .unwrap_or_else(|| format!("{:.2}", price));
                painter.text(
                    egui::pos2(rect.right() - price_right_inset, cy), egui::Align2::RIGHT_CENTER,
                    &price_str, price_font_id.clone(), fg,
                );

                // ── Faint inter-row separator (project-specific) ────────
                if separator_on {
                    painter.line_segment(
                        [
                            egui::pos2(rect.left() + 16.0, rect.bottom() - 0.5),
                            egui::pos2(rect.right() - 4.0, rect.bottom() - 0.5),
                        ],
                        Stroke::new(stroke_thin(), color_alpha(border, ALPHA_MUTED)),
                    );
                }

                // ── Hover X glyph (project-specific) ────────────────────
                if pre_hovered && !drag_confirmed {
                    if self_show_x_on_hover {
                        painter.text(
                            egui::pos2(rect.right() - 8.0, cy),
                            egui::Align2::CENTER_CENTER,
                            icon_set.x,
                            egui::FontId::proportional(11.0),
                            dim.gamma_multiply(0.5),
                        );
                    }
                }

                // Reserve right-edge X click zone (caller paints the X on hover).
                zones_body.borrow_mut().x = Some(egui::Rect::from_min_max(
                    egui::pos2(rect.right() - 16.0, rect.top()),
                    egui::pos2(rect.right(), rect.bottom()),
                ));
            })
            .show(ui);

        // Re-interact the same rect with the caller-provided sense so we
        // can detect drag_started even though RowShell uses Sense::click().
        let resp = ui.interact(
            resp.rect,
            ui.id().with(("watchlist_row", resp.rect.min.x as i32, resp.rect.min.y as i32)),
            user_sense,
        );

        // ── Hover overlay (panel-specific bg tint on hover, !drag) ──────
        // Animated fade-in/out so cursor enter/leave eases instead of snapping.
        if let Some(ovl) = hover_overlay_col {
            use crate::chart::renderer::ui::components::motion;
            let hover_id = ui.id().with((
                "wl_row_hover",
                resp.rect.min.x as i32,
                resp.rect.min.y as i32,
            ));
            let want_hover = resp.hovered() && !drag_confirmed && !active_flag;
            let hover_t = motion::ease_bool(ui.ctx(), hover_id, want_hover, motion::FAST);
            if hover_t > 0.001 {
                let faded = motion::fade_in(ovl, hover_t);
                ui.painter().rect_filled(resp.rect, 0.0, faded);
            }
        }
        if resp.hovered() && !drag_confirmed && !active_flag {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        crate::design_tokens::register_hit(
            [resp.rect.min.x, resp.rect.min.y, resp.rect.width(), resp.rect.height()],
            "WATCHLIST_ROW", "Rows",
        );

        // Hit-test pointer against published zone rects.
        let z = *zones.borrow();
        let hover_pos = resp.hover_pos();
        let click_pos = resp.interact_pointer_pos();
        let zone_at = |pos: egui::Pos2| -> WatchlistRowZone {
            if z.x.map_or(false, |r| r.contains(pos)) { WatchlistRowZone::X }
            else if z.star.map_or(false, |r| r.contains(pos)) { WatchlistRowZone::Star }
            else if z.drag.map_or(false, |r| r.contains(pos)) { WatchlistRowZone::DragHandle }
            else if z.alert.map_or(false, |r| r.contains(pos)) { WatchlistRowZone::Alert }
            else if z.earnings.map_or(false, |r| r.contains(pos)) { WatchlistRowZone::Earnings }
            else if resp.rect.contains(pos) { WatchlistRowZone::Body }
            else { WatchlistRowZone::None }
        };
        let hovered_zone = hover_pos.map(zone_at).unwrap_or(WatchlistRowZone::None);
        let clicked = resp.clicked();
        let click_zone = click_pos.filter(|_| clicked).map(zone_at).unwrap_or(WatchlistRowZone::None);
        let drag_started = resp.drag_started();

        WatchlistRowResponse {
            star_clicked:     click_zone == WatchlistRowZone::Star,
            x_clicked:        click_zone == WatchlistRowZone::X,
            alert_clicked:    click_zone == WatchlistRowZone::Alert,
            earnings_clicked: click_zone == WatchlistRowZone::Earnings,
            drag_started:     drag_started && hovered_zone == WatchlistRowZone::DragHandle,
            hovered_zone,
            response: resp,
        }
    }
}
