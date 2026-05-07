//! `PainterPaneHeader` — absolute-rect, painter-mode pane header chrome.
//!
//! **Wiring status (verified 2026-05-02):**
//! This widget IS the sole header renderer for all chart pane types
//! (Chart, Portfolio, Dashboard, Heatmap, Spreadsheet, DesignPreview).
//! It is called at `gpu.rs:3816` inside `render_chart_pane`, which runs
//! for every pane regardless of `PaneType`.  All pane types share this
//! one header path; non-chart panes receive their body rect AFTER the
//! widget has already painted the header strip.
//!
//! The style-aware background fills (active indicator, underline, hairline
//! borders) are painted by the caller (gpu.rs ~3694–3745) directly onto
//! a `painter_at(header_rect)` BEFORE calling this widget, because they
//! depend on style-token knobs (`pane_active_indicator`, `hairline_borders`)
//! that would add complexity to the widget builder.  That is intentional
//! and is NOT a parallel header path.
//!
//! Non-chart panes (Portfolio, Heatmap) also render an inner section header
//! via `PaneHeader` (from `widgets/headers.rs`) INSIDE their body rect —
//! this is a title bar for the body content, not a duplicate of the pane
//! chrome header.
//!
//! `PainterPaneHeader`:
//!   - takes an absolute `egui::Rect` instead of consuming layout flow,
//!   - exposes click outcomes via a `PainterPaneHeaderResponse` struct,
//!   - stays read-only over chart state (caller drives all mutations).
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
    alpha_active, alpha_ghost, alpha_line, alpha_muted, alpha_solid, alpha_subtle, alpha_tint,
    color_alpha, contrast_fg, current, drawing_palette, font_md, font_sm, gap_md, gap_sm, gap_xs,
    radius_sm, radius_md, stroke_hair, stroke_std, stroke_thin,
};
use crate::ui_kit::icons::Icon;

type Theme = super::super::super::gpu::Theme;

// ─── Sizing constants ────────────────────────────────────────────────────────
// Promoted from inline magic numbers. Grouped here so visual tuning lands in one place.

/// Diameter of the link-group dot at the left edge of the header.
const LINK_DOT_SIZE: f32 = 10.0;
/// Square edge of the back / forward navigation buttons.
const NAV_BTN_SIZE: f32 = 18.0;
/// Square edge of the per-tab close `×` hit zone.
const TAB_CLOSE_SIZE: f32 = 14.0;
/// Width of the "+ Tab" affordance.
const PLUS_TAB_W: f32 = 44.0;
/// Square edge of the pane-level close button (right-anchored).
const CLOSE_BTN_SIZE: f32 = 18.0;
/// Width of the ORDER / DOM icon-label buttons in the right cluster.
const ICON_BTN_W: f32 = 28.0;
/// Maximum height for option side / DTE badges.
const BADGE_HEIGHT_MAX: f32 = 16.0;
/// Vertical inset reserved around badges (top + bottom combined).
const BADGE_INSET_V: f32 = 6.0;
/// Font size used inside option badges (C/P pill, DTE text).
const BADGE_FONT_SIZE: f32 = 9.5;
/// Font size for the small labels under the icon buttons (ORDER, DOM).
const ICON_BTN_LABEL_SIZE: f32 = 5.5;
/// Maximum height for indicator chips. Chips inset by `BADGE_INSET_V` from header.
const CHIP_HEIGHT_MAX: f32 = 18.0;
/// Tab-strip vertical inset (1px gap from header top, height = h - TAB_HEIGHT_INSET).
const TAB_TOP_INSET: f32 = 1.0;
const TAB_HEIGHT_INSET: f32 = 2.0;
/// Active-tab underline thickness — mirrors `dt_f32!(tab.underline_thickness, 2.0)`.
const TAB_UNDERLINE_THICKNESS: f32 = 2.0;
/// Vertical icon position inside the icon-label buttons (fraction of rect height).
const ICON_BTN_ICON_Y_FRAC: f32 = 0.42;
/// Distance from button bottom to the small label baseline.
const ICON_BTN_LABEL_BOTTOM_OFFSET: f32 = 3.5;
/// Inset from header top/bottom for small icon buttons.
const ICON_BTN_INSET_V: f32 = 6.0;

// ─── Painter-mode helpers ────────────────────────────────────────────────────

/// Foreground color for solid-fill badges (option C/P pill, DTE chip).
/// Theme-derived so it stays readable under both light and dark presets.
#[inline]
fn badge_fg(theme: &Theme) -> Color32 {
    contrast_fg(if theme.is_light() { Color32::WHITE } else { theme.bg })
}

/// Tri-state color tuple used by every painter-mode button in the header.
/// Returns `(bg, fg, border)`. Inactive / hovered / active resolve to the same
/// alpha tiers across +Tab, ORDER, DOM, etc. — so a style-knob change propagates.
fn painter_btn_colors(t: &Theme, hovered: bool, active: bool) -> (Color32, Color32, Color32) {
    if active {
        (color_alpha(t.accent, alpha_tint()),
         t.accent,
         color_alpha(t.accent, alpha_active()))
    } else if hovered {
        (color_alpha(t.toolbar_border, alpha_subtle()),
         t.text,
         color_alpha(t.accent, alpha_line()))
    } else {
        (color_alpha(t.toolbar_border, alpha_ghost()),
         t.dim.gamma_multiply(0.8),
         color_alpha(t.toolbar_border, alpha_muted()))
    }
}

/// Paint the close `×` glyph inside `rect`, with the standard hover treatment
/// (bear color + tinted background). Used by the per-tab close, the indicator
/// chip remove-X, and the pane-level close button.
fn paint_close_glyph(painter: &egui::Painter, rect: Rect, hovered: bool, theme: &Theme, font_size_offset: f32) {
    let col = if hovered { theme.bear } else { theme.dim.gamma_multiply(0.7) };
    if hovered {
        painter.rect_filled(rect, radius_sm(), color_alpha(theme.bear, alpha_tint()));
    }
    painter.text(
        rect.center(), Align2::CENTER_CENTER,
        "\u{00D7}", FontId::proportional(font_md() + font_size_offset), col,
    );
}

/// Paint a C/P pill + DTE countdown badge starting at `cx` on the given vertical
/// center. Returns the consumed horizontal width (so callers can advance their
/// cursor). `h_avail` is the available header height used to size the badge.
///
/// Paints nothing if both side and expiry are empty/unrecognised.
fn paint_option_badges(
    painter: &egui::Painter,
    cx: f32,
    center_y: f32,
    h_avail: f32,
    side: &str,
    expiry: &str,
    theme: &Theme,
) -> f32 {
    let bh = (h_avail - BADGE_INSET_V).min(BADGE_HEIGHT_MAX);
    let by = center_y - bh / 2.0;
    let badge_font = FontId::monospace(BADGE_FONT_SIZE);
    let dark_fg = badge_fg(theme);
    let mut consumed = 0.0_f32;

    if side == "C" || side == "P" {
        let g = painter.layout_no_wrap(side.to_string(), badge_font.clone(), dark_fg);
        let bw = g.size().x + 8.0;
        let r = Rect::from_min_size(pos2(cx + consumed, by), Vec2::new(bw, bh));
        let accent_color = if side == "C" { theme.bull } else { theme.bear };
        painter.rect_filled(r, radius_sm(), color_alpha(accent_color, alpha_solid()));
        painter.text(r.center(), Align2::CENTER_CENTER, side, badge_font.clone(), dark_fg);
        consumed += bw + 4.0;
    }
    if !expiry.is_empty() {
        use chrono::NaiveDate;
        let today = chrono::Utc::now().date_naive();
        let dte = NaiveDate::parse_from_str(expiry, "%Y-%m-%d")
            .ok().map(|d| (d - today).num_days()).unwrap_or(0);
        let lbl = if dte <= 0 { "0D".to_string() } else { format!("{}D", dte) };
        let g = painter.layout_no_wrap(lbl.clone(), badge_font.clone(), dark_fg);
        let bw = g.size().x + 6.0;
        let r = Rect::from_min_size(pos2(cx + consumed, by), Vec2::new(bw, bh));
        painter.rect_filled(r, radius_sm(), color_alpha(theme.accent, alpha_solid()));
        painter.text(r.center(), Align2::CENTER_CENTER, &lbl, badge_font, dark_fg);
        consumed += bw + 6.0;
    }
    consumed
}

/// Paint an icon-label button (ORDER / DOM / +Tab style). The icon is drawn in
/// the upper portion, the label (may be empty) along the bottom edge.
fn paint_icon_label_btn(
    painter: &egui::Painter,
    rect: Rect,
    icon: &str,
    label: &str,
    bg: Color32,
    fg: Color32,
    border: Color32,
    icon_font: FontId,
) {
    painter.rect_filled(rect, radius_md(), bg);
    painter.rect_stroke(rect, radius_md(), Stroke::new(stroke_thin(), border), StrokeKind::Outside);
    if label.is_empty() {
        painter.text(rect.center(), Align2::CENTER_CENTER, icon, icon_font, fg);
    } else {
        let icon_y = rect.top() + rect.height() * ICON_BTN_ICON_Y_FRAC;
        let label_y = rect.bottom() - ICON_BTN_LABEL_BOTTOM_OFFSET;
        painter.text(pos2(rect.center().x, icon_y), Align2::CENTER_CENTER, icon, icon_font, fg);
        painter.text(
            pos2(rect.center().x, label_y), Align2::CENTER_CENTER, label,
            FontId::monospace(ICON_BTN_LABEL_SIZE), fg,
        );
    }
}

// ─── Border / chrome system ──────────────────────────────────────────────────
//
// All pane-header chrome (background fill, outer hairline border, inter-section
// dividers, active underline) is painted through the helpers below. They route
// every alpha / stroke / luminance decision through `StyleSettings`, so a
// single token change re-skins the entire header.

/// Paint the pane-header background fill. Active panes get the brighter
/// `active_header_fill_multiply` tint; inactive panes (when `visible_count > 1`
/// AND `inactive_header_fill` is on) get the recessed `inactive_header_fill_multiply`.
fn header_fill(painter: &egui::Painter, rect: Rect, theme: &Theme, is_active: bool, visible_count: usize) {
    let st = current();
    if visible_count <= 1 {
        return;
    }
    let mul = if is_active {
        st.active_header_fill_multiply
    } else if st.inactive_header_fill {
        st.inactive_header_fill_multiply
    } else {
        return;
    };
    painter.rect_filled(rect, 0.0, theme.bg.gamma_multiply(mul));
}

/// Paint a hairline outer border around inactive pane headers. Color is
/// derived from theme luminance via `contrast_fg`-style logic — light themes
/// get a dark border, dark themes get a light border. Width and alpha come
/// from `header_outer_border_width` / `header_outer_border_alpha`.
fn header_outer_border(painter: &egui::Painter, rect: Rect, theme: &Theme, _is_active: bool) {
    // Paint the hairline border for every pane (active included). The active
    // pane is distinguished by header fill differential, not by losing its
    // border — losing the border made the active pane's chrome "disappear",
    // which read as a regression vs the rest of the layout grid.
    let st = current();
    let contrast = contrast_fg(theme.bg);
    let border_col = color_alpha(contrast, st.header_outer_border_alpha);
    painter.rect_stroke(
        rect, 0.0,
        Stroke::new(st.header_outer_border_width, border_col),
        StrokeKind::Inside,
    );
}

/// Paint a vertical hairline divider at `cx` inside the header rect. Used
/// between section clusters (nav cluster, tab/symbol area, indicator chips,
/// right-side icon buttons). Single source of truth for all in-header dividers
/// — alpha + stroke width route through tokens so visual density is one knob.
fn header_divider(painter: &egui::Painter, cx: f32, rect: Rect, theme: &Theme) {
    if !current().vertical_group_dividers { return; }
    let alpha = current().header_divider_alpha;
    let col = color_alpha(theme.toolbar_border, alpha);
    painter.line_segment(
        [pos2(cx, rect.top() + 4.0), pos2(cx, rect.bottom() - 4.0)],
        Stroke::new(stroke_hair(), col),
    );
}

/// Variant of `header_divider` that ignores the `vertical_group_dividers`
/// toggle — used between adjacent buttons that visually need a separator
/// regardless of style preset (e.g. ORDER ↔ DOM).
fn header_divider_inline(painter: &egui::Painter, cx: f32, rect: Rect, theme: &Theme) {
    let col = color_alpha(theme.toolbar_border, current().header_divider_alpha);
    painter.line_segment(
        [pos2(cx, rect.top() + 3.0), pos2(cx, rect.bottom() - 3.0)],
        Stroke::new(stroke_hair(), col),
    );
}

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
    /// Show Order-entry toggle button (top-right cluster).
    show_order_btn: bool,
    /// Whether the order entry panel is currently open (button lit).
    order_btn_active: bool,
    /// Show DOM sidebar toggle button (top-right cluster).
    show_dom_btn: bool,
    /// Whether the DOM sidebar is currently open (button lit).
    dom_btn_active: bool,
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
            show_order_btn: false,
            order_btn_active: false,
            show_dom_btn: false,
            dom_btn_active: false,
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
    /// Show order-entry toggle button. `active` = order entry is currently open.
    pub fn show_order_btn(mut self, active: bool) -> Self {
        self.show_order_btn = true; self.order_btn_active = active; self
    }
    /// Show DOM sidebar toggle button. `active` = DOM sidebar is currently open.
    pub fn show_dom_btn(mut self, active: bool) -> Self {
        self.show_dom_btn = true; self.dom_btn_active = active; self
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

        // ── Header chrome ────────────────────────────────────────────────────
        // Active pane gets a darker fill differential. Outer hairline border
        // paints on every pane (active included) so the layout grid stays
        // consistent. NO accent underline — the active state is communicated
        // through fill alone, matching the Zed reference.
        header_fill(&painter, rect, t, self.is_active, self.visible_count);
        header_outer_border(&painter, rect, t, self.is_active);

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
            clicked_order: false,
            clicked_dom: false,
            tab_rects: Vec::new(),
            plus_tab_rect: None,
        };
        out.hover_pos = ui.ctx().pointer_hover_pos().filter(|p| rect.contains(*p));

        // ── Cursor x walks left → right ────────────────────────────────────
        let mut cx = rect.left() + gap_sm();

        // Link dot
        if self.show_link_dot {
            let center = pos2(cx + LINK_DOT_SIZE / 2.0, rect.center().y);
            let hit = Rect::from_center_size(center, Vec2::new(LINK_DOT_SIZE + gap_sm(), h));
            let resp = ui.allocate_rect(hit, Sense::click());
            if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            let link_colors: [Color32; 4] = drawing_palette();
            if self.link_group > 0 && self.link_group <= 4 {
                painter.circle_filled(center, LINK_DOT_SIZE / 2.0, link_colors[(self.link_group - 1) as usize]);
            } else {
                painter.circle_stroke(center, LINK_DOT_SIZE / 2.0 - 0.5,
                    Stroke::new(stroke_thin() * 2.0, t.dim.gamma_multiply(0.4)));
            }
            if resp.clicked() { out.clicked_link = true; }
            cx += LINK_DOT_SIZE + gap_md();
        }

        // Back / Fwd
        if self.show_back_fwd {
            // Back
            {
                let r = Rect::from_center_size(pos2(cx + NAV_BTN_SIZE / 2.0, rect.center().y), Vec2::splat(NAV_BTN_SIZE));
                let resp = ui.allocate_rect(r, Sense::click());
                let (bg, fg) = nav_colors(self.can_go_back, resp.hovered(), t, ui);
                painter.rect_filled(r, radius_sm(), bg);
                painter.text(r.center(), Align2::CENTER_CENTER, Icon::CARET_LEFT,
                    FontId::proportional(font_md() + 1.0), fg);
                if resp.clicked() && self.can_go_back { out.clicked_back = true; }
                cx += NAV_BTN_SIZE + gap_xs();
            }
            // Fwd
            {
                let r = Rect::from_center_size(pos2(cx + NAV_BTN_SIZE / 2.0, rect.center().y), Vec2::splat(NAV_BTN_SIZE));
                let resp = ui.allocate_rect(r, Sense::click());
                let (bg, fg) = nav_colors(self.can_go_fwd, resp.hovered(), t, ui);
                painter.rect_filled(r, radius_sm(), bg);
                painter.text(r.center(), Align2::CENTER_CENTER, Icon::CARET_RIGHT,
                    FontId::proportional(font_md() + 1.0), fg);
                if resp.clicked() && self.can_go_fwd { out.clicked_fwd = true; }
                cx += NAV_BTN_SIZE + gap_sm();
            }
            // Post-nav divider — only in simple-symbol mode (tabs delimit themselves).
            if self.tabs.is_empty() {
                header_divider(&painter, cx, rect, t);
            }
        }

        // ── Tab strip OR simple symbol label ──
        let title_font = FontId::monospace(self.title_font_size);

        if !self.tabs.is_empty() {
            // Tab bar
            let tab_h = h - TAB_HEIGHT_INSET;
            let tab_y = rect.top() + TAB_TOP_INSET;
            let tab_pad = gap_md() + 4.0;
            let gap_between = gap_md();

            for (ti, (sym, price_text, _chg)) in self.tabs.iter().enumerate() {
                let is_active_tab = ti == self.active_tab;
                let sym_galley = painter.layout_no_wrap(sym.to_string(), title_font.clone(), t.dim);
                let price_font = FontId::monospace((self.title_font_size - 1.0).max(font_sm()));
                let price_galley = painter.layout_no_wrap(
                    price_text.to_string(), price_font.clone(), t.dim);
                let tab_w = tab_pad + sym_galley.size().x + gap_between
                    + price_galley.size().x + gap_between + TAB_CLOSE_SIZE + tab_pad;

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

                // Bg — tab_hover_bg_alpha and tab_inactive_alpha knobs from StyleSettings.
                // Animated transitions: active fades over MED, inactive hover fades over FAST.
                let style_st = current();
                use crate::chart::renderer::ui::components::motion;
                let active_id = egui::Id::new(("painter_pane_tab_active", self.pane_index, ti));
                let hover_id  = egui::Id::new(("painter_pane_tab_hover",  self.pane_index, ti));
                let active_t = motion::ease_bool(ui.ctx(), active_id, is_active_tab, motion::MED);
                let hover_t  = motion::ease_bool(ui.ctx(), hover_id,  tab_resp.hovered() && !is_active_tab, motion::FAST);
                let idle_bg   = Color32::TRANSPARENT;
                let hover_bg  = color_alpha(t.toolbar_border, style_st.tab_hover_bg_alpha);
                // Active tab: noticeably darker than the (now lighter) inactive
                // pane header so the contrast reads clearly.
                let active_bg = t.bg.gamma_multiply(0.4);
                let mut tab_bg = motion::lerp_color(idle_bg, hover_bg, hover_t);
                tab_bg = motion::lerp_color(tab_bg, active_bg, active_t);
                let r_md = radius_md() as u8;
                painter.rect_filled(
                    tab_rect,
                    egui::CornerRadius { nw: r_md, ne: r_md, sw: 0, se: 0 },
                    tab_bg,
                );
                if active_t > 0.001 {
                    // Active tab: 2px top accent + 1px hairline borders on
                    // top / left / right. NO bottom border — the tab merges
                    // with the pane content below (TabTreatment::Card look).
                    let accent = motion::fade_in(t.accent, active_t);
                    let border = motion::fade_in(
                        color_alpha(t.toolbar_border, alpha_solid()),
                        active_t,
                    );
                    let bs = Stroke::new(stroke_std(), border);
                    // 2px top accent indicator (sits below the top hairline)
                    painter.line_segment(
                        [pos2(tab_rect.left(), tab_rect.top() + 1.5),
                         pos2(tab_rect.right(), tab_rect.top() + 1.5)],
                        Stroke::new(TAB_UNDERLINE_THICKNESS, accent),
                    );
                    // Top hairline (full 1px stroke for visibility)
                    painter.line_segment(
                        [pos2(tab_rect.left(), tab_rect.top()),
                         pos2(tab_rect.right(), tab_rect.top())],
                        bs,
                    );
                    // Left hairline — inset by 0.5 so it sits inside the rect
                    let lx = tab_rect.left() + 0.5;
                    painter.line_segment(
                        [pos2(lx, tab_rect.top()), pos2(lx, tab_rect.bottom())],
                        bs,
                    );
                    // Right hairline — inset by 0.5
                    let rx = tab_rect.right() - 0.5;
                    painter.line_segment(
                        [pos2(rx, tab_rect.top()), pos2(rx, tab_rect.bottom())],
                        bs,
                    );
                }

                // Vertical divider between this tab and the next — paints for
                // every adjacent pair (active included). Drawn at the right
                // edge of the current tab inside the inter-tab gap.
                if ti + 1 < self.tabs.len() {
                    let div_col = color_alpha(t.toolbar_border, alpha_muted());
                    painter.line_segment(
                        [pos2(tab_rect.right() + 0.5, tab_rect.top() + 4.0),
                         pos2(tab_rect.right() + 0.5, tab_rect.bottom() - 4.0)],
                        Stroke::new(stroke_thin(), div_col),
                    );
                }

                // tab_inactive_alpha dims inactive tab text
                let sym_col = if is_active_tab {
                    if self.is_active && self.visible_count > 1 { t.accent } else { t.text }
                } else {
                    t.dim.gamma_multiply(style_st.tab_inactive_alpha)
                };
                painter.text(
                    pos2(tab_rect.left() + tab_pad, tab_rect.center().y),
                    Align2::LEFT_CENTER, sym, title_font.clone(), sym_col,
                );
                let mut price_x = tab_rect.left() + tab_pad + sym_galley.size().x + gap_between;
                // Option badges in tab strip (C/P pill + DTE)
                if let Some((side, expiry)) = self.option_badges {
                    price_x += paint_option_badges(
                        &painter, price_x, tab_rect.center().y, tab_rect.height(),
                        side, expiry, t,
                    );
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
                        pos2(tab_rect.right() - tab_pad - TAB_CLOSE_SIZE / 2.0, tab_rect.center().y),
                        Vec2::splat(TAB_CLOSE_SIZE),
                    );
                    let resp = ui.allocate_rect(close_rect, Sense::click());
                    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    paint_close_glyph(&painter, close_rect, resp.hovered(), t, 2.0);
                    if resp.clicked() {
                        out.clicked_close = true;
                        out.clicked_tab = Some(ti);
                    }
                }

                if tab_resp.clicked() && !out.clicked_close {
                    out.clicked_tab = Some(ti);
                }
                out.tab_rects.push(tab_rect);
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
            cx += sym_galley.size().x + gap_md() + 4.0;

            // Option badges: C/P pill + DTE countdown (shared helper).
            if let Some((side, expiry)) = self.option_badges {
                cx += paint_option_badges(&painter, cx, rect.center().y, h, side, expiry, t);
            }

            if let Some(tf) = self.timeframe {
                let tf_font = FontId::monospace(font_sm());
                let g = painter.layout_no_wrap(tf.to_string(), tf_font.clone(), t.dim);
                painter.text(pos2(cx, rect.center().y), Align2::LEFT_CENTER, tf,
                    tf_font, t.dim);
                cx += g.size().x + gap_md();
            }

            if let (Some(price_text), price_color) = (self.price_text, self.price_color.unwrap_or(t.dim)) {
                let price_font = FontId::monospace(self.title_font_size - 1.0);
                let g = painter.layout_no_wrap(price_text.to_string(), price_font.clone(), price_color);
                painter.text(pos2(cx, rect.center().y), Align2::LEFT_CENTER,
                    price_text, price_font, price_color);
                cx += g.size().x + gap_md() + 4.0;
            }

            // Surface symbol click to caller
            if sym_clicked { out.clicked_symbol = true; }
        }

        // Divider after symbol/tabs section (before indicator chips). Routes
        // through the unified `header_divider` helper.
        header_divider(&painter, cx, rect, t);

        // ── Indicator chips with painted ✕ ──
        const CHIP_X_W: f32 = 12.0;
        for (i, ind) in self.indicators.iter().enumerate() {
            let chip_font = FontId::monospace(font_sm());
            let g = painter.layout_no_wrap(ind.to_string(), chip_font.clone(), t.dim);
            let chip_pad = gap_md();
            let chip_w = chip_pad + g.size().x + gap_sm() + CHIP_X_W + chip_pad;
            let chip_h = (h - BADGE_INSET_V).min(CHIP_HEIGHT_MAX);
            let chip_rect = Rect::from_min_size(
                pos2(cx, rect.center().y - chip_h / 2.0),
                Vec2::new(chip_w, chip_h),
            );
            painter.rect_stroke(
                chip_rect, radius_sm(),
                Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, alpha_muted())),
                StrokeKind::Inside,
            );
            painter.text(
                pos2(chip_rect.left() + chip_pad, chip_rect.center().y),
                Align2::LEFT_CENTER, ind, chip_font, t.dim,
            );
            let x_rect = Rect::from_center_size(
                pos2(chip_rect.right() - chip_pad - CHIP_X_W / 2.0, chip_rect.center().y),
                Vec2::splat(CHIP_X_W),
            );
            let x_resp = ui.allocate_rect(x_rect, Sense::click());
            if x_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            paint_close_glyph(&painter, x_rect, x_resp.hovered(), t, 0.0);
            if x_resp.clicked() {
                out.clicked_indicator_remove = Some(i);
            }
            cx += chip_w + gap_sm();
        }

        // ── (Star/template button removed — template selection lives in the
        //    unified pane picker triggered from the symbol/title click.) ──

        // ── +Tab: left-aligned immediately after the last tab ──
        if self.show_plus_tab {
            let plus_h = h - ICON_BTN_INSET_V;
            let plus_rect = Rect::from_min_size(
                pos2(cx, rect.center().y - plus_h / 2.0),
                Vec2::new(PLUS_TAB_W, plus_h),
            );
            let resp = ui.allocate_rect(plus_rect, Sense::click());
            if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            let (bg, fg, border) = painter_btn_colors(t, resp.hovered(), false);
            // +Tab uses a single-line label (no icon glyph above) — paint inline
            // rather than via paint_icon_label_btn since the layout differs.
            painter.rect_filled(plus_rect, radius_md(), bg);
            painter.rect_stroke(plus_rect, radius_md(),
                Stroke::new(stroke_thin(), border), StrokeKind::Outside);
            painter.text(plus_rect.center(), Align2::CENTER_CENTER,
                "+ Tab",
                FontId::monospace((self.title_font_size - 2.0).max(font_sm())), fg);
            if resp.clicked() { out.clicked_plus = true; }
            out.plus_tab_rect = Some(plus_rect);
            cx += PLUS_TAB_W + gap_sm();
        }

        // ── Right cluster: Order | DOM | Close (right-anchored) ──────────────
        let close_total = if self.show_close { gap_md() + CLOSE_BTN_SIZE + gap_md() } else { gap_sm() };
        let order_dom_total = {
            let mut w = 0.0f32;
            if self.show_order_btn { w += ICON_BTN_W; }
            if self.show_dom_btn   { w += ICON_BTN_W; }
            w
        };

        // Divider before right icon cluster
        header_divider(&painter, rect.right() - close_total - order_dom_total, rect, t);

        // ── Order + DOM icon buttons ──────────────────────────────────────────
        {
            let icon_h = h - ICON_BTN_INSET_V;
            // Icons here render bigger than the title text — these are the
            // primary affordances on the right cluster.
            let icon_font = FontId::proportional(self.title_font_size + 4.0);
            let mut rx = rect.right() - close_total - order_dom_total;
            let mut paint_btn = |ui: &mut Ui, rx: f32, icon: &str, label: &str, is_active: bool| -> bool {
                let r = Rect::from_min_size(
                    pos2(rx, rect.center().y - icon_h / 2.0),
                    Vec2::new(ICON_BTN_W, icon_h),
                );
                let resp = ui.allocate_rect(r, Sense::click());
                if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                let (bg, fg, border) = painter_btn_colors(t, resp.hovered(), is_active);
                paint_icon_label_btn(&painter, r, icon, label, bg, fg, border, icon_font.clone());
                resp.clicked()
            };

            if self.show_order_btn {
                if paint_btn(ui, rx, Icon::CURRENCY_DOLLAR, "ORDER", self.order_btn_active) {
                    out.clicked_order = true;
                }
                rx += ICON_BTN_W;
                // Divider between ORDER and DOM buttons (always shown — these
                // buttons need a separator regardless of style preset).
                if self.show_dom_btn {
                    header_divider_inline(&painter, rx, rect, t);
                }
            }
            if self.show_dom_btn {
                if paint_btn(ui, rx, Icon::LADDER, "DOM", self.dom_btn_active) {
                    out.clicked_dom = true;
                }
            }
        }

        // ── Close button (right-anchored) ──
        if self.show_close {
            let close_rect = Rect::from_center_size(
                pos2(rect.right() - gap_md() - CLOSE_BTN_SIZE / 2.0, rect.center().y),
                Vec2::splat(CLOSE_BTN_SIZE),
            );
            let resp = ui.allocate_rect(close_rect, Sense::click());
            if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            paint_close_glyph(&painter, close_rect, resp.hovered(), t, 2.0);
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
    /// Order-entry toggle button was clicked.
    pub clicked_order: bool,
    /// DOM sidebar toggle button was clicked.
    pub clicked_dom: bool,
    /// Per-tab screen rects (in tab-strip mode). Empty in simple-symbol mode.
    /// Use these to anchor popups (the pane picker, etc.) to a specific tab.
    pub tab_rects: Vec<Rect>,
    /// Screen rect of the +Tab button when shown — for anchoring pickers
    /// triggered by the plus-tab click.
    pub plus_tab_rect: Option<Rect>,
}

// ─── Local helpers ──────────────────────────────────────────────────────────

fn nav_colors(enabled: bool, hovered: bool, t: &Theme, ui: &mut Ui) -> (Color32, Color32) {
    use super::super::style::alpha_dim;
    if enabled {
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            (color_alpha(t.toolbar_border, alpha_dim()), t.text)
        } else {
            (Color32::TRANSPARENT, t.dim.gamma_multiply(0.8))
        }
    } else {
        (Color32::TRANSPARENT, t.dim.gamma_multiply(0.25))
    }
}
