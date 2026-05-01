//! `PainterPaneHeader` — absolute-rect, painter-mode pane header chrome.
//!
//! This is a NEW widget (not yet wired into the chart pane chrome at
//! `gpu.rs:5370+`). The existing pane chrome computes geometry directly on
//! `pane_rect.min` + `pane_top_offset` and paints into a `painter_at(rect)`,
//! which is fundamentally incompatible with egui's flow-layout-based widgets
//! in `widgets/pane.rs` (`PaneHeaderBar` etc.).
//!
//! `PainterPaneHeader` mirrors the gpu.rs paint code exactly but:
//!   - takes an absolute `egui::Rect` instead of consuming layout flow,
//!   - exposes click outcomes via a `PainterPaneHeaderResponse` struct,
//!   - stays read-only over chart state (caller drives all mutations).
//!
//! The intent is for `gpu.rs:5370+` to eventually adopt this widget, but
//! that migration is a follow-up wave — the chart paint is sacred.
//!
//! ```ignore
//! let resp = PainterPaneHeader::new(header_rect, theme)
//!     .symbol("SPY")
//!     .timeframe("5m")
//!     .indicators(&["EMA 20", "VWAP"])
//!     .show_close(true)
//!     .show_link_dot(true)
//!     .link_group(0)
//!     .show(ui);
//! if resp.clicked_close { /* close pane */ }
//! if let Some(i) = resp.clicked_indicator_remove { /* remove */ }
//! ```

#![allow(dead_code, unused_imports)]

use egui::{Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, pos2};

use super::super::style::{
    alpha_active, alpha_line, alpha_muted, alpha_subtle, color_alpha, font_md, font_sm, gap_sm,
    gap_xs, stroke_thin,
};
use crate::ui_kit::icons::Icon;

type Theme = super::super::super::gpu::Theme;

/// Absolute-rect pane header chrome — paints a single header strip into a
/// caller-supplied `Rect` and reports per-zone click outcomes.
#[must_use = "PainterPaneHeader must be shown with `.show(ui)` to render"]
pub struct PainterPaneHeader<'a> {
    rect: Rect,
    theme: &'a Theme,
    is_active: bool,
    visible_count: usize,

    symbol: Option<&'a str>,
    timeframe: Option<&'a str>,
    asset_class: Option<&'a str>,
    price_text: Option<&'a str>,
    price_color: Option<Color32>,
    indicators: &'a [&'a str],

    show_link_dot: bool,
    link_group: u8,
    show_back_fwd: bool,
    can_go_back: bool,
    can_go_fwd: bool,
    show_close: bool,
    show_plus_tab: bool,

    /// Tab strip data — `(symbol, price_text, change_pct)` per tab. Empty =
    /// simple single-symbol header (no tabs).
    tabs: &'a [(&'a str, &'a str, f32)],
    active_tab: usize,
    hovered_tab: Option<usize>,

    title_font_size: f32,

    // ── New knobs ──────────────────────────────────────────────────────────
    /// Option badges: `(side, expiry_str)` — paints C/P pill + DTE countdown badge.
    option_badges: Option<(&'a str, &'a str)>,
    /// Whether to show the star/template button after symbol/tabs.
    show_template_btn: bool,
    /// Whether the template button is currently active (popup open).
    template_btn_active: bool,
    /// Sense for tab strip interactions — use `Sense::click_and_drag()` for cross-pane drag.
    tab_sense: Option<Sense>,
    /// Pane index — used to build unique egui Ids for tab interactions.
    pane_index: usize,
}

impl<'a> PainterPaneHeader<'a> {
    pub fn new(rect: Rect, theme: &'a Theme) -> Self {
        Self {
            rect,
            theme,
            is_active: false,
            visible_count: 1,
            symbol: None,
            timeframe: None,
            asset_class: None,
            price_text: None,
            price_color: None,
            indicators: &[],
            show_link_dot: false,
            link_group: 0,
            show_back_fwd: false,
            can_go_back: false,
            can_go_fwd: false,
            show_close: false,
            show_plus_tab: false,
            tabs: &[],
            active_tab: 0,
            hovered_tab: None,
            title_font_size: font_md(),
            option_badges: None,
            show_template_btn: false,
            template_btn_active: false,
            tab_sense: None,
            pane_index: 0,
        }
    }

    pub fn is_active(mut self, v: bool) -> Self { self.is_active = v; self }
    pub fn visible_count(mut self, v: usize) -> Self { self.visible_count = v; self }

    pub fn symbol(mut self, s: &'a str) -> Self { self.symbol = Some(s); self }
    pub fn timeframe(mut self, t: &'a str) -> Self { self.timeframe = Some(t); self }
    pub fn asset_class(mut self, a: &'a str) -> Self { self.asset_class = Some(a); self }
    pub fn price(mut self, text: &'a str, color: Color32) -> Self {
        self.price_text = Some(text); self.price_color = Some(color); self
    }
    pub fn indicators(mut self, idx: &'a [&'a str]) -> Self { self.indicators = idx; self }

    pub fn show_link_dot(mut self, v: bool) -> Self { self.show_link_dot = v; self }
    pub fn link_group(mut self, g: u8) -> Self { self.link_group = g; self }
    pub fn show_back_fwd(mut self, v: bool) -> Self { self.show_back_fwd = v; self }
    pub fn can_go_back(mut self, v: bool) -> Self { self.can_go_back = v; self }
    pub fn can_go_fwd(mut self, v: bool) -> Self { self.can_go_fwd = v; self }
    pub fn show_close(mut self, v: bool) -> Self { self.show_close = v; self }
    pub fn show_plus_tab(mut self, v: bool) -> Self { self.show_plus_tab = v; self }

    pub fn tabs(mut self, t: &'a [(&'a str, &'a str, f32)]) -> Self { self.tabs = t; self }
    pub fn active_tab(mut self, i: usize) -> Self { self.active_tab = i; self }
    pub fn hovered_tab(mut self, i: Option<usize>) -> Self { self.hovered_tab = i; self }

    pub fn title_font_size(mut self, s: f32) -> Self { self.title_font_size = s; self }

    /// Paint C/P pill + DTE countdown badge after the symbol/tab.
    /// `side` = "C" or "P"; `expiry` = "YYYY-MM-DD" date string.
    pub fn option_badges(mut self, side: &'a str, expiry: &'a str) -> Self {
        self.option_badges = Some((side, expiry)); self
    }
    /// Show star/template button. `active` = popup is already open.
    pub fn show_template_btn(mut self, active: bool) -> Self {
        self.show_template_btn = true; self.template_btn_active = active; self
    }
    /// Override tab `Sense` — use `Sense::click_and_drag()` for cross-pane drag support.
    pub fn tab_sense(mut self, s: Sense) -> Self { self.tab_sense = Some(s); self }
    /// Pane index — used to form unique egui Ids for tabs and drag state.
    pub fn pane_index(mut self, i: usize) -> Self { self.pane_index = i; self }

    pub fn show(self, ui: &mut Ui) -> PainterPaneHeaderResponse {
        let t = self.theme;
        let rect = self.rect;
        let h = rect.height();

        // Reserve the whole header rect for hover (matches gpu.rs pattern).
        let bg_resp = ui.allocate_rect(rect, Sense::click());

        let painter = ui.painter_at(rect);

        // Background — active darker, inactive slightly lighter.
        let header_bg = if self.is_active && self.visible_count > 1 {
            t.bg.gamma_multiply(0.6)
        } else {
            t.bg.gamma_multiply(1.2)
        };
        painter.rect_filled(rect, 0.0, header_bg);

        // Active underline.
        if self.is_active && self.visible_count > 1 {
            let y = rect.bottom() - 1.0;
            painter.line_segment(
                [pos2(rect.left(), y), pos2(rect.right(), y)],
                Stroke::new(2.0, t.accent),
            );
        }

        let mut out = PainterPaneHeaderResponse {
            response: bg_resp,
            clicked_close: false,
            clicked_plus: false,
            clicked_link: false,
            clicked_back: false,
            clicked_fwd: false,
            clicked_indicator_remove: None,
            clicked_tab: None,
            hover_pos: None,
            clicked_template: false,
            tab_drag_started: None,
            tab_drag_pos: None,
            tab_drag_released: None,
            symbol_rect: None,
            clicked_symbol: false,
        };
        out.hover_pos = ui.ctx().pointer_hover_pos().filter(|p| rect.contains(*p));

        // ── Cursor x walks left → right ────────────────────────────────────
        let mut cx = rect.left() + gap_sm();

        // Link dot
        if self.show_link_dot {
            let dot_size = 10.0_f32;
            let center = pos2(cx + dot_size / 2.0, rect.center().y);
            let hit = Rect::from_center_size(center, Vec2::new(dot_size + 4.0, h));
            let resp = ui.allocate_rect(hit, Sense::click());
            if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            let link_colors: [Color32; 4] = [
                Color32::from_rgb(70, 130, 255),
                Color32::from_rgb(80, 200, 120),
                Color32::from_rgb(255, 160, 60),
                Color32::from_rgb(180, 100, 255),
            ];
            if self.link_group > 0 && self.link_group <= 4 {
                painter.circle_filled(center, dot_size / 2.0, link_colors[(self.link_group - 1) as usize]);
            } else {
                painter.circle_stroke(center, dot_size / 2.0 - 0.5,
                    Stroke::new(stroke_thin() * 2.0, t.dim.gamma_multiply(0.4)));
            }
            if resp.clicked() { out.clicked_link = true; }
            cx += dot_size + gap_md_local();
        }

        // Back / Fwd
        if self.show_back_fwd {
            let nav = 18.0_f32;
            // Back
            {
                let r = Rect::from_center_size(pos2(cx + nav / 2.0, rect.center().y), Vec2::splat(nav));
                let resp = ui.allocate_rect(r, Sense::click());
                let (bg, fg) = nav_colors(self.can_go_back, resp.hovered(), t, ui);
                painter.rect_filled(r, 3.0, bg);
                painter.text(r.center(), Align2::CENTER_CENTER, Icon::CARET_LEFT,
                    FontId::proportional(font_md() + 1.0), fg);
                if resp.clicked() && self.can_go_back { out.clicked_back = true; }
                cx += nav + gap_xs();
            }
            // Fwd
            {
                let r = Rect::from_center_size(pos2(cx + nav / 2.0, rect.center().y), Vec2::splat(nav));
                let resp = ui.allocate_rect(r, Sense::click());
                let (bg, fg) = nav_colors(self.can_go_fwd, resp.hovered(), t, ui);
                painter.rect_filled(r, 3.0, bg);
                painter.text(r.center(), Align2::CENTER_CENTER, Icon::CARET_RIGHT,
                    FontId::proportional(font_md() + 1.0), fg);
                if resp.clicked() && self.can_go_fwd { out.clicked_fwd = true; }
                cx += nav + gap_sm();
            }
        }

        // ── Tab strip OR simple symbol label ──
        let title_font = FontId::monospace(self.title_font_size);

        if !self.tabs.is_empty() {
            // Tab bar
            let tab_h = h - 2.0;
            let tab_y = rect.top() + 1.0;
            let close_w = 14.0_f32;
            let tab_pad = gap_md_local() + 4.0;
            let gap_between = gap_md_local();

            for (ti, (sym, price_text, _chg)) in self.tabs.iter().enumerate() {
                let is_active_tab = ti == self.active_tab;
                let sym_galley = painter.layout_no_wrap(sym.to_string(), title_font.clone(), t.dim);
                let price_font = FontId::monospace((self.title_font_size - 1.0).max(font_sm()));
                let price_galley = painter.layout_no_wrap(
                    price_text.to_string(), price_font.clone(), t.dim);
                let tab_w = tab_pad + sym_galley.size().x + gap_between
                    + price_galley.size().x + gap_between + close_w + tab_pad;

                let tab_rect = Rect::from_min_size(pos2(cx, tab_y), Vec2::new(tab_w, tab_h));
                let effective_sense = self.tab_sense.unwrap_or_else(Sense::click);
                let tab_resp = ui.interact(
                    tab_rect,
                    egui::Id::new(("painter_pane_tab", self.pane_index, ti)),
                    effective_sense,
                );
                if tab_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                // Cross-pane drag support — only fires when tab_sense includes drag
                if tab_resp.drag_started_by(egui::PointerButton::Primary) {
                    out.tab_drag_started = Some(ti);
                }
                if tab_resp.dragged_by(egui::PointerButton::Primary) {
                    if let Some(p) = tab_resp.interact_pointer_pos() {
                        out.tab_drag_pos = Some((ti, p));
                    }
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
                if tab_resp.drag_stopped() {
                    out.tab_drag_released = Some(ti);
                }

                // Bg
                let tab_bg = if is_active_tab {
                    t.bg.gamma_multiply(0.5)
                } else if tab_resp.hovered() {
                    t.bg.gamma_multiply(0.75)
                } else {
                    t.bg.gamma_multiply(0.9)
                };
                painter.rect_filled(
                    tab_rect,
                    egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 },
                    tab_bg,
                );
                if is_active_tab {
                    painter.line_segment(
                        [pos2(tab_rect.left() + 1.0, tab_rect.bottom()),
                         pos2(tab_rect.right() - 1.0, tab_rect.bottom())],
                        Stroke::new(2.0, t.accent),
                    );
                }

                let sym_col = if is_active_tab {
                    if self.is_active && self.visible_count > 1 { t.accent } else { t.text }
                } else {
                    t.dim.gamma_multiply(0.7)
                };
                painter.text(
                    pos2(tab_rect.left() + tab_pad, tab_rect.center().y),
                    Align2::LEFT_CENTER, sym, title_font.clone(), sym_col,
                );
                let mut price_x = tab_rect.left() + tab_pad + sym_galley.size().x + gap_between;
                // Option badges in tab strip (C/P pill + DTE)
                if let Some((side, expiry)) = self.option_badges {
                    let bh = (tab_rect.height() - 6.0).min(16.0);
                    let by = tab_rect.center().y - bh / 2.0;
                    let badge_font = FontId::monospace(9.5);
                    let dark_fg = Color32::from_rgb(24, 24, 28);
                    if side == "C" || side == "P" {
                        let g = painter.layout_no_wrap(side.to_string(), badge_font.clone(), dark_fg);
                        let bw = g.size().x + 8.0;
                        let r = Rect::from_min_size(pos2(price_x, by), Vec2::new(bw, bh));
                        let accent_color = if side == "C" { t.bull } else { t.bear };
                        painter.rect_filled(r, 3.0, color_alpha(accent_color, 200));
                        painter.text(r.center(), Align2::CENTER_CENTER, side, badge_font.clone(), dark_fg);
                        price_x += bw + 4.0;
                    }
                    if !expiry.is_empty() {
                        use chrono::NaiveDate;
                        let today = chrono::Utc::now().date_naive();
                        let dte = NaiveDate::parse_from_str(expiry, "%Y-%m-%d")
                            .ok().map(|d| (d - today).num_days()).unwrap_or(0);
                        let lbl = if dte <= 0 { "0D".to_string() } else { format!("{}D", dte) };
                        let g = painter.layout_no_wrap(lbl.clone(), badge_font.clone(), dark_fg);
                        let bw = g.size().x + 6.0;
                        let r = Rect::from_min_size(pos2(price_x, by), Vec2::new(bw, bh));
                        painter.rect_filled(r, 3.0, color_alpha(t.accent, 200));
                        painter.text(r.center(), Align2::CENTER_CENTER, &lbl, badge_font, dark_fg);
                        price_x += bw + 6.0;
                    }
                }
                let price_color = self.price_color.unwrap_or(t.dim);
                painter.text(
                    pos2(price_x, tab_rect.center().y),
                    Align2::LEFT_CENTER, price_text, price_font, price_color,
                );

                // Close × on hover or active
                let show_close_x = self.tabs.len() > 1
                    && (self.hovered_tab == Some(ti) || is_active_tab);
                if show_close_x {
                    let close_rect = Rect::from_center_size(
                        pos2(tab_rect.right() - tab_pad - close_w / 2.0, tab_rect.center().y),
                        Vec2::splat(close_w),
                    );
                    let resp = ui.allocate_rect(close_rect, Sense::click());
                    let close_col = if resp.hovered() { t.bear } else { t.dim.gamma_multiply(0.6) };
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        painter.rect_filled(close_rect, 2.0, color_alpha(t.bear, alpha_tint_local()));
                    }
                    painter.text(close_rect.center(), Align2::CENTER_CENTER,
                        "\u{00D7}", FontId::proportional(font_md() + 2.0), close_col);
                    if resp.clicked() {
                        // tab close — surface as clicked_tab + clicked_close pair? Use clicked_tab w/ clicked_close set.
                        out.clicked_close = true;
                        out.clicked_tab = Some(ti);
                    }
                }

                if tab_resp.clicked() && !out.clicked_close {
                    out.clicked_tab = Some(ti);
                }
                cx += tab_w + 1.0;
            }
        } else if let Some(sym) = self.symbol {
            // Simple label
            let label_color = if self.is_active { t.bull } else { t.text };
            let sym_galley = painter.layout_no_wrap(sym.to_string(), title_font.clone(), label_color);
            // Allocate a click rect for the symbol label so callers can anchor pickers.
            let sym_label_rect = Rect::from_min_size(
                pos2(cx, rect.center().y - sym_galley.size().y / 2.0),
                Vec2::new(sym_galley.size().x + 4.0, sym_galley.size().y + 2.0),
            );
            let sym_resp = ui.allocate_rect(sym_label_rect, Sense::click());
            if sym_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            out.symbol_rect = Some(sym_label_rect);
            if sym_resp.clicked() {
                // Caller handles by checking symbol_rect + bg_resp.clicked(); surface via symbol_rect presence.
                // We set a dedicated flag by re-using clicked_tab = None and symbol_rect set.
                // Actually signal via a dedicated field — use out.symbol_rect + allocate_rect clicked:
                // Re-use the trick: mark symbol_rect and let caller check sym_resp.clicked().
                // Since we can't embed a Response in out cleanly, we repurpose clicked_tab with sentinel.
                // Simplest: just expose symbol_rect; caller checks `out.symbol_rect.is_some()` and re-queries ctx.
                // For full fidelity keep it simple — store clicked state via a new mechanism:
                // We set clicked_plus temporarily with a different convention... instead, add a bool.
                // NOTE: symbol_rect is always set; caller must check `out.symbol_clicked`.
            }
            // Track symbol click separately
            let sym_clicked = sym_resp.clicked();
            let p0 = pos2(cx + 2.0, rect.center().y);
            painter.text(pos2(p0.x + 0.5, p0.y), Align2::LEFT_CENTER, sym,
                title_font.clone(), label_color);
            painter.text(p0, Align2::LEFT_CENTER, sym, title_font.clone(), label_color);
            cx += sym_galley.size().x + gap_md_local() + 4.0;

            // Option badges: C/P pill + DTE countdown
            if let Some((side, expiry)) = self.option_badges {
                let bh = (h - 6.0).min(16.0);
                let by = rect.center().y - bh / 2.0;
                let badge_font = FontId::monospace(9.5);
                let dark_fg = Color32::from_rgb(24, 24, 28);
                if side == "C" || side == "P" {
                    let g = painter.layout_no_wrap(side.to_string(), badge_font.clone(), dark_fg);
                    let bw = g.size().x + 8.0;
                    let r = Rect::from_min_size(pos2(cx, by), Vec2::new(bw, bh));
                    let accent_color = if side == "C" { t.bull } else { t.bear };
                    painter.rect_filled(r, 3.0, color_alpha(accent_color, 200));
                    painter.text(r.center(), Align2::CENTER_CENTER, side, badge_font.clone(), dark_fg);
                    cx += bw + 4.0;
                }
                if !expiry.is_empty() {
                    use chrono::NaiveDate;
                    let today = chrono::Utc::now().date_naive();
                    let dte = NaiveDate::parse_from_str(expiry, "%Y-%m-%d")
                        .ok().map(|d| (d - today).num_days()).unwrap_or(0);
                    let lbl = if dte <= 0 { "0D".to_string() } else { format!("{}D", dte) };
                    let g = painter.layout_no_wrap(lbl.clone(), badge_font.clone(), dark_fg);
                    let bw = g.size().x + 6.0;
                    let r = Rect::from_min_size(pos2(cx, by), Vec2::new(bw, bh));
                    painter.rect_filled(r, 3.0, color_alpha(t.accent, 200));
                    painter.text(r.center(), Align2::CENTER_CENTER, &lbl, badge_font, dark_fg);
                    cx += bw + 6.0;
                }
            }

            if let Some(tf) = self.timeframe {
                let tf_font = FontId::monospace(font_sm());
                let g = painter.layout_no_wrap(tf.to_string(), tf_font.clone(), t.dim);
                painter.text(pos2(cx, rect.center().y), Align2::LEFT_CENTER, tf,
                    tf_font, t.dim);
                cx += g.size().x + gap_md_local();
            }

            if let (Some(price_text), price_color) = (self.price_text, self.price_color.unwrap_or(t.dim)) {
                let price_font = FontId::monospace(self.title_font_size - 1.0);
                let g = painter.layout_no_wrap(price_text.to_string(), price_font.clone(), price_color);
                painter.text(pos2(cx, rect.center().y), Align2::LEFT_CENTER,
                    price_text, price_font, price_color);
                cx += g.size().x + gap_md_local() + 4.0;
            }

            // Surface symbol click to caller
            if sym_clicked { out.clicked_symbol = true; }
        }

        // ── Indicator chips with painted ✕ ──
        for (i, ind) in self.indicators.iter().enumerate() {
            let chip_font = FontId::monospace(font_sm());
            let g = painter.layout_no_wrap(ind.to_string(), chip_font.clone(), t.dim);
            let chip_pad = gap_md_local();
            let x_w = 12.0;
            let chip_w = chip_pad + g.size().x + gap_sm() + x_w + chip_pad;
            let chip_h = (h - 6.0).min(18.0);
            let chip_rect = Rect::from_min_size(
                pos2(cx, rect.center().y - chip_h / 2.0),
                Vec2::new(chip_w, chip_h),
            );
            painter.rect_stroke(
                chip_rect, 3.0,
                Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, alpha_muted())),
                StrokeKind::Inside,
            );
            painter.text(
                pos2(chip_rect.left() + chip_pad, chip_rect.center().y),
                Align2::LEFT_CENTER, ind, chip_font, t.dim,
            );
            // ✕ hit zone
            let x_rect = Rect::from_center_size(
                pos2(chip_rect.right() - chip_pad - x_w / 2.0, chip_rect.center().y),
                Vec2::splat(x_w),
            );
            let x_resp = ui.allocate_rect(x_rect, Sense::click());
            let x_col = if x_resp.hovered() { t.bear } else { t.dim.gamma_multiply(0.7) };
            if x_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            painter.text(x_rect.center(), Align2::CENTER_CENTER,
                "\u{00D7}", FontId::proportional(font_md()), x_col);
            if x_resp.clicked() {
                out.clicked_indicator_remove = Some(i);
            }
            cx += chip_w + gap_sm();
        }

        // ── + Tab button (right-aligned tile from current cursor) ──
        if self.show_plus_tab {
            let plus_w = 44.0;
            let plus_h = h - 6.0;
            let plus_rect = Rect::from_min_size(
                pos2(cx + 4.0, rect.center().y - plus_h / 2.0),
                Vec2::new(plus_w, plus_h),
            );
            let resp = ui.allocate_rect(plus_rect, Sense::click());
            let (bg, fg, border) = if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                (color_alpha(t.toolbar_border, alpha_subtle()),
                 t.text,
                 color_alpha(t.accent, alpha_line()))
            } else {
                (color_alpha(t.toolbar_border, 18),
                 t.dim.gamma_multiply(0.8),
                 color_alpha(t.toolbar_border, alpha_muted()))
            };
            painter.rect_filled(plus_rect, 4.0, bg);
            painter.rect_stroke(plus_rect, 4.0,
                Stroke::new(stroke_thin(), border), StrokeKind::Outside);
            painter.text(plus_rect.center(), Align2::CENTER_CENTER,
                "+ Tab",
                FontId::monospace((self.title_font_size - 2.0).max(font_sm())), fg);
            if resp.clicked() { out.clicked_plus = true; }
            cx += plus_w + gap_md_local();
        }

        // ── Template / star button ──
        if self.show_template_btn {
            let t_w = 22.0;
            let t_h = h - 6.0;
            let t_rect = Rect::from_min_size(
                pos2(cx, rect.center().y - t_h / 2.0),
                Vec2::new(t_w, t_h),
            );
            let t_resp = ui.allocate_rect(t_rect, Sense::click());
            let t_active = self.template_btn_active;
            let (bg, fg, border) = if t_active {
                (color_alpha(t.accent, 38), t.accent, color_alpha(t.accent, alpha_active()))
            } else if t_resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                (color_alpha(t.toolbar_border, alpha_subtle()),
                 t.text,
                 color_alpha(t.accent, alpha_line()))
            } else {
                (color_alpha(t.toolbar_border, 18),
                 t.dim.gamma_multiply(0.8),
                 color_alpha(t.toolbar_border, alpha_muted()))
            };
            painter.rect_filled(t_rect, 4.0, bg);
            painter.rect_stroke(t_rect, 4.0,
                Stroke::new(stroke_thin(), border), StrokeKind::Outside);
            painter.text(t_rect.center(), Align2::CENTER_CENTER,
                crate::ui_kit::icons::Icon::STAR,
                FontId::proportional((self.title_font_size - 2.0).max(font_sm())), fg);
            if t_resp.clicked() { out.clicked_template = true; }
            cx += t_w + gap_sm();
        }

        // ── Close button (right-anchored) ──
        if self.show_close {
            let close_size = 18.0_f32;
            let close_rect = Rect::from_center_size(
                pos2(rect.right() - gap_md_local() - close_size / 2.0, rect.center().y),
                Vec2::splat(close_size),
            );
            let resp = ui.allocate_rect(close_rect, Sense::click());
            let col = if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                t.bear
            } else { t.dim.gamma_multiply(0.7) };
            if resp.hovered() {
                painter.rect_filled(close_rect, 3.0, color_alpha(t.bear, alpha_tint_local()));
            }
            painter.text(close_rect.center(), Align2::CENTER_CENTER,
                "\u{00D7}", FontId::proportional(font_md() + 2.0), col);
            if resp.clicked() && out.clicked_tab.is_none() {
                out.clicked_close = true;
            }
        }

        let _ = cx; // keep cursor walk ergonomic for future extensions
        out
    }
}

/// Per-zone click outcomes from a `PainterPaneHeader::show`.
pub struct PainterPaneHeaderResponse {
    /// Background hover/click — useful for "click rest of header to activate pane".
    pub response: Response,
    pub clicked_close: bool,
    pub clicked_plus: bool,
    pub clicked_link: bool,
    pub clicked_back: bool,
    pub clicked_fwd: bool,
    /// Index of an indicator chip whose ✕ was clicked.
    pub clicked_indicator_remove: Option<usize>,
    /// Index of a tab strip entry that was clicked.
    pub clicked_tab: Option<usize>,
    /// Pointer position relative to viewport, if hovering inside `rect`.
    pub hover_pos: Option<egui::Pos2>,

    // ── New response fields ────────────────────────────────────────────────
    /// Star/template button was clicked.
    pub clicked_template: bool,
    /// Index of the tab whose drag just started (first frame of drag).
    pub tab_drag_started: Option<usize>,
    /// Pointer position reported during drag, per dragging tab index.
    pub tab_drag_pos: Option<(usize, Pos2)>,
    /// Index of the tab whose drag was just released.
    pub tab_drag_released: Option<usize>,
    /// Screen rect of the painted symbol label (for anchoring picker popups).
    pub symbol_rect: Option<Rect>,
    /// Symbol label was clicked (simple-label mode only).
    pub clicked_symbol: bool,
}

// ─── Local helpers ──────────────────────────────────────────────────────────

fn gap_md_local() -> f32 { super::super::style::gap_md() }
fn alpha_tint_local() -> u8 { super::super::style::alpha_tint() }

fn nav_colors(enabled: bool, hovered: bool, t: &Theme, ui: &mut Ui) -> (Color32, Color32) {
    if enabled {
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            (color_alpha(t.toolbar_border, 60), t.text)
        } else {
            (Color32::TRANSPARENT, t.dim.gamma_multiply(0.8))
        }
    } else {
        (Color32::TRANSPARENT, t.dim.gamma_multiply(0.25))
    }
}
