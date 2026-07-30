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
use crate::chart_renderer::ui::style::tint;
use crate::ui_kit::sx::Tone;

use super::super::style::{
    alpha_active, alpha_ghost, alpha_line, alpha_muted, alpha_solid, alpha_subtle, alpha_tint,
    color_alpha, color_subtle, color_muted, color_half, color_dim, color_very_dim, contrast_fg, current, drawing_palette, font_md, font_md_plus, font_sm, gap_md, gap_sm, gap_xs,
    paint_bevel, paint_gradient_highlight, current_style_bevel_hi,
    radius_sm, radius_md, stroke_hair, stroke_std, stroke_thin,
};
use crate::ui_kit::icons::Icon;

type Theme = super::super::super::gpu::Theme;

// ─── Button-state primitives ─────────────────────────────────────────────────

/// Collapsed show+active pair for each toggleable pane-header button.
/// Replaces the previous 10× `show_X: bool` + `X_active: bool` flat fields.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ButtonState {
    pub(crate) show:   bool,
    pub(crate) active: bool,
}

/// Index into the `PainterPaneHeader::buttons` array. Also the discriminant of
/// `PainterPaneHeaderResponse::clicked_button`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum PaneBtn {
    Template  = 0,
    Order     = 1,
    Dom       = 2,
    Options   = 3,
    Overlay   = 4,
    Drawing   = 5, // kept for compatibility; never shown by default
    Layers    = 6,
    Expand    = 7,
    Split     = 8,
    ClosePane = 9,
}
const PANE_BTN_COUNT: usize = 10;

// ─── Sizing constants ────────────────────────────────────────────────────────
// Promoted from inline magic numbers. Grouped here so visual tuning lands in one place.

/// Width reserved for the link-group Select trigger at the left edge of the header.
/// Compact dot-only trigger — just wide enough for the icon button.
const LINK_SELECT_W: f32 = 26.0;
/// Square edge of the back / forward navigation buttons.
const NAV_BTN_SIZE: f32 = 18.0;
/// Square edge of the per-tab close `×` hit zone.
const TAB_CLOSE_SIZE: f32 = 14.0;
/// Width of the "+ Tab" affordance.
const PLUS_TAB_W: f32 = 44.0;
/// Square edge of the pane-level close button (right-anchored).
const CLOSE_BTN_SIZE: f32 = 18.0;
/// Width of the ORDER / DOM icon-label buttons in the right cluster.
/// Sized for a horizontal `[icon] LABEL` pill (icon-left-of-text), not
/// the previous vertical icon-above-sublabel stack.
const ICON_BTN_W: f32 = 60.0;
/// Narrower width for the DOM button (shorter label).
const ICON_BTN_W_DOM: f32 = 52.0;
/// Width for the OPTIONS button (longer label — 7 chars).
const ICON_BTN_W_OPTIONS: f32 = 68.0;
const ICON_BTN_W_OV: f32 = 80.0;
const ICON_BTN_W_DRAWING: f32 = 60.0;
/// Width for the LAYERS (object-tree) button.
const ICON_BTN_W_LAYERS: f32 = 60.0;
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
///
/// `pub(crate)` so other chrome modules can reuse the same button-color logic.
pub(crate) fn painter_btn_colors(t: &Theme, hovered: bool, active: bool) -> (Color32, Color32, Color32) {
    if active {
        (tint(t, Tone::Accent, alpha_tint()),
         t.accent,
         tint(t, Tone::Accent, alpha_active()))
    } else if hovered {
        (tint(t, Tone::Border, alpha_subtle()),
         t.text,
         tint(t, Tone::Accent, alpha_line()))
    } else {
        (tint(t, Tone::Border, alpha_ghost()),
         color_subtle(t.dim),
         tint(t, Tone::Border, alpha_muted()))
    }
}

/// Paint the close `×` glyph inside `rect`, with the standard hover treatment
/// (bear color + tinted background). Used by the per-tab close, the indicator
/// chip remove-X, and the pane-level close button.
///
/// `pub(crate)` so other chrome modules can reuse the same close-glyph style.
pub(crate) fn paint_close_glyph(painter: &egui::Painter, rect: Rect, hovered: bool, theme: &Theme, font_size_offset: f32) {
    let col = if hovered { theme.bear } else { color_subtle(theme.dim) };
    if hovered {
        painter.rect_filled(rect, radius_sm(), tint(theme, Tone::Bear, alpha_tint()));
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
///
/// `pub(crate)` for reuse by other pane-chrome renderers.
pub(crate) fn paint_option_badges(
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
        painter.rect_filled(r, radius_sm(), tint(theme, Tone::Accent, alpha_solid()));
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
    // Background fill (multi-pane only): active panes brighter, inactive recessed.
    if visible_count > 1 {
        let mul = if is_active {
            Some(st.active_header_fill_multiply)
        } else if st.inactive_header_fill {
            Some(st.inactive_header_fill_multiply)
        } else {
            None
        };
        if let Some(mul) = mul {
            painter.rect_filled(rect, 0.0, theme.bg.gamma_multiply(mul));
        }
    }
    // Gradient highlight + crisp bevel lines — both no-ops when bevel is None.
    // Gradient: soft white-to-transparent fade matching React's linear-gradient.
    // Bevel: 1px highlight top + 1px shadow bottom.
    paint_gradient_highlight(painter, rect, current_style_bevel_hi());
    paint_bevel(painter, rect, egui::CornerRadius::ZERO);
}

/// Paint a vertical hairline divider at `cx` inside the header rect. Used
/// between section clusters (nav cluster, tab/symbol area, indicator chips,
/// right-side icon buttons). Single source of truth for all in-header dividers
/// — alpha + stroke width route through tokens so visual density is one knob.
fn header_divider(painter: &egui::Painter, cx: f32, rect: Rect, theme: &Theme) {
    if !current().vertical_group_dividers { return; }
    let alpha = current().header_divider_alpha;
    let col = tint(theme, Tone::Border, alpha);
    painter.line_segment(
        [pos2(cx, rect.top() + 4.0), pos2(cx, rect.bottom() - 4.0)],
        Stroke::new(stroke_hair(), col),
    );
}

/// Variant of `header_divider` that ignores the `vertical_group_dividers`
/// toggle — used between adjacent buttons that visually need a separator
/// regardless of style preset (e.g. ORDER ↔ DOM).
fn header_divider_inline(painter: &egui::Painter, cx: f32, rect: Rect, theme: &Theme) {
    let col = tint(theme, Tone::Border, current().header_divider_alpha);
    painter.line_segment(
        [pos2(cx, rect.top() + 3.0), pos2(cx, rect.bottom() - 3.0)],
        Stroke::new(stroke_hair(), col),
    );
}

/// Strong inter-button divider routed through the `border_variant` token —
/// used between buttons in the right cluster (Order/DOM/+Tab) and between
/// the tab strip and the right cluster. Higher visibility than
/// `header_divider`/`header_divider_inline` per trading-terminal density needs.
fn header_divider_strong(painter: &egui::Painter, cx: f32, rect: Rect, theme: &Theme) {
    let col = color_alpha(theme.border_variant, 200);
    painter.line_segment(
        [pos2(cx, rect.top() + 3.0), pos2(cx, rect.bottom() - 3.0)],
        Stroke::new(stroke_thin(), col),
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
    /// Resolved color for the current link group (None = unlinked/hollow).
    link_group_color: Option<Color32>,
    /// Human-readable name for the current link group, shown as a tooltip on the dot.
    link_group_name: Option<String>,
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
    /// All toggleable icon buttons keyed by [`PaneBtn`].
    /// Replaces 10 × (show_X, X_active) bool pairs.
    buttons: [ButtonState; PANE_BTN_COUNT],
    /// Sense for tab strip interactions — use `Sense::click_and_drag()` for cross-pane drag.
    tab_sense: Option<Sense>,
    /// Pane index — used to build unique egui Ids for tab interactions.
    pane_index: usize,
    /// Whether the pane is currently maximized (drives Expand button colour).
    is_maximized: bool,
    /// Whether the split popup is currently open (drives Split button colour).
    split_btn_active: bool,
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
            link_group_color: None,
            link_group_name: None,
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
            buttons: [ButtonState::default(); PANE_BTN_COUNT],
            tab_sense: None,
            pane_index: 0,
            is_maximized: false,
            split_btn_active: false,
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
    pub fn link_group_color(mut self, c: Option<Color32>) -> Self { self.link_group_color = c; self }
    /// Set the human-readable label for the link group dot tooltip (e.g. "Group 1").
    pub fn link_group_name(mut self, name: impl Into<String>) -> Self {
        self.link_group_name = Some(name.into()); self
    }
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
        self.buttons[PaneBtn::Template as usize] = ButtonState { show: true, active }; self
    }
    /// Show order-entry toggle button. `active` = order entry is currently open.
    pub fn show_order_btn(mut self, active: bool) -> Self {
        self.buttons[PaneBtn::Order as usize] = ButtonState { show: true, active }; self
    }
    /// Show DOM sidebar toggle button. `active` = DOM sidebar is currently open.
    pub fn show_dom_btn(mut self, active: bool) -> Self {
        self.buttons[PaneBtn::Dom as usize] = ButtonState { show: true, active }; self
    }
    pub fn show_options_btn(mut self, active: bool) -> Self {
        self.buttons[PaneBtn::Options as usize] = ButtonState { show: true, active }; self
    }
    /// Show symbol-overlay toggle button. `active` = overlay editing is on or overlays present.
    pub fn show_overlay_btn(mut self, active: bool) -> Self {
        self.buttons[PaneBtn::Overlay as usize] = ButtonState { show: true, active }; self
    }
    /// Show drawing-palette toggle button (kept for compatibility; not shown by default).
    pub fn show_drawing_btn(mut self, active: bool) -> Self {
        self.buttons[PaneBtn::Drawing as usize] = ButtonState { show: true, active }; self
    }
    /// Show layers (object-tree) button. `active` = object tree panel is open.
    pub fn show_layers_btn(mut self, active: bool) -> Self {
        self.buttons[PaneBtn::Layers as usize] = ButtonState { show: true, active }; self
    }
    /// Override tab `Sense` — use `Sense::click_and_drag()` for cross-pane drag support.
    pub fn tab_sense(mut self, s: Sense) -> Self { self.tab_sense = Some(s); self }
    /// Pane index — used to form unique egui Ids for tabs and drag state.
    pub fn pane_index(mut self, i: usize) -> Self { self.pane_index = i; self }
    /// Show expand button. `maximized` = pane is currently maximized (button lit).
    pub fn show_expand_btn(mut self, maximized: bool) -> Self {
        self.buttons[PaneBtn::Expand as usize] = ButtonState { show: true, active: false };
        self.is_maximized = maximized; self
    }
    /// Show the split-pane button. `popup_open` = H/V dropdown is showing (button lit).
    pub fn show_split_btn(mut self, popup_open: bool) -> Self {
        self.buttons[PaneBtn::Split as usize] = ButtonState { show: true, active: false };
        self.split_btn_active = popup_open; self
    }
    /// Show the close-pane button (distinct from the per-tab close).
    pub fn show_close_pane_btn(mut self) -> Self {
        self.buttons[PaneBtn::ClosePane as usize] = ButtonState { show: true, active: false }; self
    }

    pub fn show(self, ui: &mut Ui) -> PainterPaneHeaderResponse {
        let t = self.theme;
        let rect = self.rect;
        let h = rect.height();

        // Reserve the whole header rect for hover only. Sense::click() here
        // would consume clicks before they reach Order/DOM/OPTIONS buttons —
        // allocate_rect registers a widget that out-priorities later
        // ui.interact() calls at overlapping rects. Active-pane focus on
        // click is now detected via raw input pointer test in the caller.
        let bg_resp = ui.allocate_rect(rect, Sense::hover());

        let painter = ui.painter_at(rect);

        // ── Header chrome ────────────────────────────────────────────────────
        // Restored (round 2): trading-terminal density needs:
        //   1. Active-pane bg darken when multiple panes are visible.
        //   2. A subtle perimeter hairline around the header rect.
        //   3. Stronger inter-tab + inter-button dividers (handled inline below
        //      via `border_variant`-routed colors).
        //
        // The standalone bottom hairline previously painted here is removed —
        // the outer border's bottom edge now does that job.

        // 1. Active-pane header treatment (only meaningful with >1 pane).
        //
        // Three modes driven by StyleSettings:
        //   pane_active_fill_accent=true (Aperture) → fill with accent color (orange bar).
        //   pane_active_indicator & 2 (header fill, default) → direction-aware bg fill:
        //     dark themes: slight darken (content-blending); light themes: slight lift.
        //   pane_active_indicator & 1 (top stripe) → accent-colored 2px top edge.
        {
            let st = current();
            if self.visible_count > 1 {
                // ── Background fill ──────────────────────────────────────────
                if self.is_active {
                    if st.pane_active_fill_accent {
                        // A solid accent fill across the whole header reads as a
                        // loud, alarming orange bar. Use a restrained active
                        // indicator instead — a faint accent wash + a crisp 2px
                        // accent top stripe — which still clearly marks the active
                        // pane without shouting (mockup parity).
                        painter.rect_filled(rect, 0.0, tint(t, Tone::Accent, alpha_subtle()));
                        let stripe = Rect::from_min_max(
                            rect.min,
                            pos2(rect.max.x, rect.min.y + 2.0),
                        );
                        painter.rect_filled(stripe, 0.0, t.accent);
                    } else if st.pane_active_indicator & 2 != 0 {
                        // Standard fill: direction-aware so light themes lift
                        // instead of darken. Mirror `color_layer_up` logic.
                        let is_dark = (t.bg.r() as i16 + t.bg.g() as i16 + t.bg.b() as i16) < 384;
                        let active_bg = if is_dark {
                            // Dark theme: slightly darker = focused/immersive.
                            color_subtle(t.bg)
                        } else {
                            // Light theme: slightly lighter = lifted/elevated.
                            let ch = |c: i16| -> u8 { c.clamp(0, 255) as u8 };
                            Color32::from_rgb(
                                ch(t.bg.r() as i16 + 12),
                                ch(t.bg.g() as i16 + 10),
                                ch(t.bg.b() as i16 +  8),
                            )
                        };
                        painter.rect_filled(rect, 0.0, active_bg);
                    }
                } else if st.inactive_header_fill {
                    let is_dark = (t.bg.r() as i16 + t.bg.g() as i16 + t.bg.b() as i16) < 384;
                    let inactive_bg = if is_dark {
                        t.bg.gamma_multiply(st.inactive_header_fill_multiply)
                    } else {
                        // Light theme: slightly darker for inactive = recessed.
                        let ch = |c: i16| -> u8 { c.clamp(0, 255) as u8 };
                        Color32::from_rgb(
                            ch(t.bg.r() as i16 - 10),
                            ch(t.bg.g() as i16 - 10),
                            ch(t.bg.b() as i16 - 10),
                        )
                    };
                    painter.rect_filled(rect, 0.0, inactive_bg);
                }
            }
            // ── Top accent stripe (Mariner "instrument needle", Meridien) ───
            // pane_active_indicator bit 1: 2px accent-colored top edge on the
            // active pane header. Signals focus without filling the whole bar.
            if self.is_active && st.pane_active_indicator & 1 != 0 {
                painter.line_segment(
                    [pos2(rect.left(), rect.top() + 1.0), pos2(rect.right(), rect.top() + 1.0)],
                    Stroke::new(2.0, t.accent),
                );
            }
        }

        // 2. Outer perimeter hairline — text-derived color so it stays visible
        //    across both light and dark themes (bg-derived `border_variant`
        //    became invisible on darker themes). Width and alpha come from
        //    the active style's `header_outer_border_width` and
        //    `header_outer_border_alpha` so per-style design tokens drive the
        //    look rather than a hardcoded value here.
        let st = current();
        // Pre-compute accent-header flag and derived text colors. Must be before
        // the outer border paint (which reads accent_header) and all color sites below.
        let accent_header = current().pane_active_fill_accent
            && self.is_active
            && self.visible_count > 1;
        let h_text   = if accent_header { contrast_fg(t.accent) } else { t.text };
        let h_dim    = if accent_header { color_subtle(contrast_fg(t.accent)) } else { t.dim };
        let h_accent = if accent_header { contrast_fg(t.accent) } else { t.accent };

        painter.rect_stroke(
            rect, 0.0,
            // On accent-filled header (Aperture), suppress the outer border —
            // the vibrant fill is the focus indicator; a border would double it.
            Stroke::new(
                if accent_header { 0.0 } else { st.header_outer_border_width },
                tint(t, Tone::Text, st.header_outer_border_alpha),
            ),
            StrokeKind::Inside,
        );

        let mut out = PainterPaneHeaderResponse {
            response: bg_resp,
            clicked_close: false,
            clicked_plus: false,
            clicked_back: false,
            clicked_fwd: false,
            clicked_indicator_remove: None,
            clicked_tab: None,
            hover_pos: None,
            clicked_button: None,
            tab_drag_started: None,
            tab_drag_pos: None,
            tab_drag_released: None,
            symbol_rect: None,
            clicked_symbol: false,
            tab_rects: Vec::new(),
            plus_tab_rect: None,
            split_btn_rect: None,
            link_dot_rect: None,
        };
        out.hover_pos = ui.ctx().pointer_hover_pos().filter(|p| rect.contains(*p));

        // ── Cursor x walks left → right ────────────────────────────────────
        let mut cx = rect.left() + gap_sm();

        // Link group — passive region; caller places a Select widget here via link_dot_rect.
        // We also emit a tooltip when a group name is supplied so users can discover
        // what the colored dot means without opening the group picker.
        if self.show_link_dot {
            let hit = Rect::from_min_size(pos2(cx, rect.top()), Vec2::new(LINK_SELECT_W, h));
            let dot_resp = ui.allocate_rect(hit, Sense::hover());
            // Tooltip: "Group 1 (link group 1)" or "Not linked — click to join a link group"
            let tip = if self.link_group == 0 {
                "Not linked — click to join a link group".to_string()
            } else if let Some(ref name) = self.link_group_name {
                format!("{name} (link group {})", self.link_group)
            } else {
                format!("Link group {}", self.link_group)
            };
            dot_resp.on_hover_text(tip);
            out.link_dot_rect = Some(hit);
            cx += LINK_SELECT_W + gap_sm();
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
                    FontId::proportional(font_md_plus()), fg);
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
                    FontId::proportional(font_md_plus()), fg);
                if resp.clicked() && self.can_go_fwd { out.clicked_fwd = true; }
                cx += NAV_BTN_SIZE + gap_sm();
            }
            // Divider separating the nav/group cluster from the ticker/tab area — always visible.
            header_divider_inline(&painter, cx, rect, t);
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
                let sym_galley = painter.layout_no_wrap(sym.to_string(), title_font.clone(), h_dim);
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
                let hover_bg  = tint(t, Tone::Border, style_st.tab_hover_bg_alpha);
                // Aperture: active tab on orange header = dark pill ("cream pill, orange text")
                // Normal: active tab = darkened bg.
                let active_bg = if accent_header {
                    color_alpha(h_text, 200) // ~dark semi-opaque fill
                } else {
                    color_dim(t.bg)
                };
                let mut tab_bg = motion::lerp_color(idle_bg, hover_bg, hover_t);
                tab_bg = motion::lerp_color(tab_bg, active_bg, active_t);
                // Tab corner radius: pill for accent_header (Aperture), top-rounded otherwise.
                let r_md = radius_md() as u8;
                let tab_cr = if accent_header {
                    egui::CornerRadius::same(style_st.r_pill.min(r_md + 4))
                } else {
                    egui::CornerRadius { nw: r_md, ne: r_md, sw: 0, se: 0 }
                };
                painter.rect_filled(tab_rect, tab_cr, tab_bg);
                // Per Zed pattern: no accent top stripe, no top/left/right
                // hairline borders on the active tab. Active signal lives in
                // the active-tab-dot + bottom-edge accent line, both owned by
                // the Tabs widget (separate agent).
                let _ = active_t;

                // Vertical hairline divider between this tab and the next —
                // paints for every adjacent pair (active included). Drawn at
                // the right edge of the current tab inside the inter-tab gap.
                if ti + 1 < self.tabs.len() {
                    let div_col = color_alpha(t.border_variant, 200);
                    painter.line_segment(
                        [pos2(tab_rect.right() + 0.5, tab_rect.top() + 4.0),
                         pos2(tab_rect.right() + 0.5, tab_rect.bottom() - 4.0)],
                        Stroke::new(stroke_thin(), div_col),
                    );
                }

                // tab_inactive_alpha dims inactive tab text
                let sym_col = if is_active_tab {
                    // On accent-filled header: active tab = dark contrasting text.
                    // On normal header: active tab = accent color (usual).
                    if self.is_active && self.visible_count > 1 { h_accent } else { t.text }
                } else {
                    if accent_header { color_subtle(h_text) } else { t.dim.gamma_multiply(style_st.tab_inactive_alpha) }
                };
                painter.text(
                    pos2(tab_rect.left() + tab_pad, tab_rect.center().y),
                    Align2::LEFT_CENTER, sym, title_font.clone(), sym_col,
                );
                let mut price_x = tab_rect.left() + tab_pad + sym_galley.size().x + gap_between;
                // Option badges in tab strip (C/P pill + DTE). `option_badges` is a
                // single header-level value derived from the pane's ACTIVE
                // instrument (core.rs), so it only describes the active tab —
                // painting it on every tab bled a C/P+DTE badge onto unrelated
                // stock tabs sharing the pane. Gate on the active tab.
                if is_active_tab {
                    if let Some((side, expiry)) = self.option_badges {
                        price_x += paint_option_badges(
                            &painter, price_x, tab_rect.center().y, tab_rect.height(),
                            side, expiry, t,
                        );
                    }
                }
                // On accent header (Aperture orange bar), use h_dim for price/change text.
                let price_color = if accent_header && is_active_tab {
                    h_dim
                } else {
                    self.price_color.unwrap_or(t.dim)
                };
                painter.text(
                    pos2(price_x, tab_rect.center().y),
                    Align2::LEFT_CENTER, price_text, price_font, price_color,
                );

                // Close × on hover or active — use h_dim on accent header.
                let show_close_x = self.tabs.len() > 1
                    && (self.hovered_tab == Some(ti) || is_active_tab);
                if show_close_x {
                    let close_rect = Rect::from_center_size(
                        pos2(tab_rect.right() - tab_pad - TAB_CLOSE_SIZE / 2.0, tab_rect.center().y),
                        Vec2::splat(TAB_CLOSE_SIZE),
                    );
                    let resp = ui.allocate_rect(close_rect, Sense::click());
                    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    // On orange header the theme has t.text = cream which would show wrong;
                    // temporarily swap the close glyph color using a custom paint.
                    if accent_header {
                        let col = if resp.hovered() { h_text } else { h_dim };
                        painter.text(close_rect.center(), egui::Align2::CENTER_CENTER,
                            "×", FontId::proportional(10.0), col);
                    } else {
                        paint_close_glyph(&painter, close_rect, resp.hovered(), t, 2.0);
                    }
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
            // Simple label. Bump the symbol one tier above the title baseline
            // so the pane-title reads as the primary header (vs axis labels /
            // chip text). Keep the tab-strip path on title_font so dense tab
            // rows don't grow.
            let label_color = if accent_header { h_text } else if self.is_active { t.bull } else { t.text };
            let sym_font = FontId::monospace(crate::chart_renderer::ui::style::font_lg());
            let sym_galley = painter.layout_no_wrap(sym.to_string(), sym_font.clone(), label_color);
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
            // Dev Inspector — record the symbol header widget.
            #[cfg(debug_assertions)]
            crate::dev_inspector::record(crate::dev_inspector::WidgetRecord {
                id: format!("pane_header.symbol"),
                role: "label".into(),
                label: sym.to_string(),
                value: Some(sym.to_string()),
                rect: sym_label_rect.into(),
                clip_rect: ui.clip_rect().into(),
                layer: 0,
                focused: false,
                hovered: sym_resp.hovered(),
                enabled: true,
                is_clipped: false,
                style_class: None,
            });
            let p0 = pos2(cx + 2.0, rect.center().y);
            painter.text(pos2(p0.x + 0.5, p0.y), Align2::LEFT_CENTER, sym,
                sym_font.clone(), label_color);
            painter.text(p0, Align2::LEFT_CENTER, sym, sym_font.clone(), label_color);
            cx += sym_galley.size().x + gap_md() + 4.0;

            // Option badges: C/P pill + DTE countdown (shared helper).
            if let Some((side, expiry)) = self.option_badges {
                cx += paint_option_badges(&painter, cx, rect.center().y, h, side, expiry, t);
            }

            if let Some(tf) = self.timeframe {
                let tf_font = FontId::monospace(font_sm());
                let g = painter.layout_no_wrap(tf.to_string(), tf_font.clone(), h_dim);
                painter.text(pos2(cx, rect.center().y), Align2::LEFT_CENTER, tf,
                    tf_font, h_dim);
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
            let g = painter.layout_no_wrap(ind.to_string(), chip_font.clone(), h_dim);
            let chip_pad = gap_md();
            let chip_w = chip_pad + g.size().x + gap_sm() + CHIP_X_W + chip_pad;
            let chip_h = (h - BADGE_INSET_V).min(CHIP_HEIGHT_MAX);
            let chip_rect = Rect::from_min_size(
                pos2(cx, rect.center().y - chip_h / 2.0),
                Vec2::new(chip_w, chip_h),
            );
            painter.rect_stroke(
                chip_rect, radius_sm(),
                Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_muted())),
                StrokeKind::Inside,
            );
            painter.text(
                pos2(chip_rect.left() + chip_pad, chip_rect.center().y),
                Align2::LEFT_CENTER, ind, chip_font, h_dim,
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
            // Strong divider between the tab strip and the +Tab affordance —
            // mirrors the IDE convention of separating editor tabs from the
            // per-pane controls cluster.
            if !self.tabs.is_empty() {
                header_divider_strong(&painter, cx + gap_xs(), rect, t);
                cx += gap_sm();
            }
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

        // ── Right cluster: [OVERLAY] [LAYERS] [DOM] [OPTIONS] | [ClosePane] [Split] [Expand] | [×] ──
        // Pane-action controls (close/split/expand) sit at far-right, just left of
        // the close-tab button. Icon buttons (OVERLAY, LAYERS, etc.) sit to their left.
        const EXPAND_BTN_W: f32 = 28.0;
        const SPLIT_BTN_W:  f32 = 28.0;
        const CLOSE_PANE_BTN_W: f32 = 28.0;
        let expand_total      = if self.buttons[PaneBtn::Expand as usize].show     { EXPAND_BTN_W     } else { 0.0 };
        let split_total       = if self.buttons[PaneBtn::Split as usize].show      { SPLIT_BTN_W      } else { 0.0 };
        let close_pane_total  = if self.buttons[PaneBtn::ClosePane as usize].show { CLOSE_PANE_BTN_W } else { 0.0 };
        let close_total       = if self.show_close          { gap_md() + CLOSE_BTN_SIZE + gap_md() } else { gap_sm() };
        // Pane-action controls are at the far right (left of close-tab button).
        let pane_ctrls_total  = close_pane_total + split_total + expand_total;
        let order_dom_total = {
            let mut w = 0.0f32;
            if self.buttons[PaneBtn::Overlay as usize].show { w += ICON_BTN_W_OV; }
            if self.buttons[PaneBtn::Layers as usize].show  { w += ICON_BTN_W_LAYERS; }
            if self.buttons[PaneBtn::Order as usize].show   { w += ICON_BTN_W; }
            if self.buttons[PaneBtn::Dom as usize].show     { w += ICON_BTN_W_DOM; }
            if self.buttons[PaneBtn::Options as usize].show { w += ICON_BTN_W_OPTIONS; }
            w
        };

        // Strong divider at the left edge of the entire right cluster.
        if order_dom_total > 0.0 || pane_ctrls_total > 0.0 || self.show_close {
            header_divider_strong(
                &painter,
                rect.right() - close_total - pane_ctrls_total - order_dom_total,
                rect, t,
            );
        }

        // ── Icon button cluster (OVERLAY / LAYERS / ORDER / DOM / OPTIONS) ────
        // Table-driven: one entry per button; each renders identically.
        // Only visible buttons are rendered; dividers appear between them.
        {
            use crate::ui_kit::widgets::Button;

            // (btn_enum, label, icon, width)
            const BTN_TABLE: &[(PaneBtn, &str, &str, f32)] = &[
                (PaneBtn::Overlay, "OVERLAY", Icon::EYE,            ICON_BTN_W_OV),
                (PaneBtn::Layers,  "LAYERS",  Icon::STACK,           ICON_BTN_W_LAYERS),
                (PaneBtn::Order,   "ORDER",   Icon::CURRENCY_DOLLAR, ICON_BTN_W),
                (PaneBtn::Dom,     "DOM",     Icon::LADDER,          ICON_BTN_W_DOM),
                (PaneBtn::Options, "OPTIONS", Icon::CIRCLE,          ICON_BTN_W_OPTIONS),
            ];

            let visible: Vec<(PaneBtn, &str, &str, f32)> = BTN_TABLE.iter()
                .filter(|(btn, ..)| self.buttons[*btn as usize].show)
                .copied()
                .collect();

            let icon_h = h - ICON_BTN_INSET_V;
            let mut rx  = rect.right() - close_total - pane_ctrls_total - order_dom_total;
            let mut clicked_btn: Option<PaneBtn> = None;

            for (pos, &(btn, label, icon, width)) in visible.iter().enumerate() {
                let r = Rect::from_min_size(
                    pos2(rx, rect.center().y - icon_h / 2.0),
                    Vec2::new(width, icon_h),
                );
                let resp = Button::new(label)
                    .leading_icon(icon)
                    .status(true)
                    .active(self.buttons[btn as usize].active)
                    .show_at(ui, &painter, r, t);
                if resp.clicked() { clicked_btn = Some(btn); }
                rx += width;
                if pos + 1 < visible.len() {
                    header_divider_strong(&painter, rx, rect, t);
                }
            }

            // Record which icon-cluster button was clicked (if any).
            if clicked_btn.is_some() { out.clicked_button = clicked_btn; }

            // Divider between icon-button cluster and pane-action controls.
            if pane_ctrls_total > 0.0 && order_dom_total > 0.0 {
                header_divider_strong(&painter, rx, rect, t);
            }
        }

        // ── Pane-action controls: [ClosePane] [Split] [Expand] (far-right) ────
        {
            let icon_h  = h - ICON_BTN_INSET_V;
            let pc_left = rect.right() - close_total - pane_ctrls_total;
            let mut px  = pc_left;

            if self.buttons[PaneBtn::ClosePane as usize].show {
                let cp_rect = Rect::from_min_size(
                    pos2(px, rect.center().y - icon_h / 2.0),
                    Vec2::new(CLOSE_PANE_BTN_W, icon_h),
                );
                let resp = ui.allocate_rect(cp_rect, Sense::click());
                if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                let col = if resp.hovered() { t.bear } else { color_subtle(t.dim) };
                if resp.hovered() {
                    painter.rect_filled(cp_rect, radius_sm(), tint(t, Tone::Bear, alpha_subtle()));
                }
                painter.text(
                    cp_rect.center(), Align2::CENTER_CENTER,
                    Icon::X, FontId::proportional(font_md_plus()), col,
                );
                if resp.clicked() { out.clicked_button = Some(PaneBtn::ClosePane); }
                px += CLOSE_PANE_BTN_W;
            }

            if self.buttons[PaneBtn::Split as usize].show {
                let split_rect = Rect::from_min_size(
                    pos2(px, rect.center().y - icon_h / 2.0),
                    Vec2::new(SPLIT_BTN_W, icon_h),
                );
                let resp = ui.allocate_rect(split_rect, Sense::click());
                if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                let col = if self.split_btn_active { t.accent }
                    else if resp.hovered() { t.text }
                    else { color_subtle(t.dim) };
                if resp.hovered() || self.split_btn_active {
                    painter.rect_filled(split_rect, radius_sm(), color_alpha(
                        if self.split_btn_active { t.accent } else { t.toolbar_border },
                        if self.split_btn_active { alpha_tint() } else { alpha_subtle() },
                    ));
                }
                painter.text(
                    split_rect.center(), Align2::CENTER_CENTER,
                    Icon::BROWSERS, FontId::proportional(font_md_plus()), col,
                );
                if resp.clicked() { out.clicked_button = Some(PaneBtn::Split); }
                out.split_btn_rect = Some(split_rect);
                px += SPLIT_BTN_W;
            }

            if self.buttons[PaneBtn::Expand as usize].show {
                let expand_rect = Rect::from_min_size(
                    pos2(px, rect.center().y - icon_h / 2.0),
                    Vec2::new(EXPAND_BTN_W, icon_h),
                );
                let resp = ui.allocate_rect(expand_rect, Sense::click());
                if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                let col = if self.is_maximized { t.accent }
                    else if resp.hovered() { t.text }
                    else { color_subtle(t.dim) };
                if resp.hovered() || self.is_maximized {
                    painter.rect_filled(expand_rect, radius_sm(), color_alpha(
                        if self.is_maximized { t.accent } else { t.toolbar_border },
                        if self.is_maximized { alpha_tint() } else { alpha_subtle() },
                    ));
                }
                painter.text(
                    expand_rect.center(), Align2::CENTER_CENTER,
                    Icon::ARROWS_OUT_SIMPLE, FontId::proportional(font_md_plus()), col,
                );
                if resp.clicked() { out.clicked_button = Some(PaneBtn::Expand); }
            }
            let _ = px;
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
    pub clicked_back: bool,
    pub clicked_fwd: bool,
    /// Index of an indicator chip whose ✕ was clicked.
    pub clicked_indicator_remove: Option<usize>,
    /// Index of a tab strip entry that was clicked.
    pub clicked_tab: Option<usize>,
    /// Pointer position relative to viewport, if hovering inside `rect`.
    pub hover_pos: Option<egui::Pos2>,

    // ── New response fields ────────────────────────────────────────────────
    /// Which toggle/action button in the right cluster was clicked this frame
    /// (Template / Order / Dom / Options / Overlay / Drawing / Layers / Expand /
    /// Split / ClosePane). At most one per frame. Replaces the previous explosion
    /// of `clicked_X: bool` fields — match on the [`PaneBtn`] variant instead.
    pub clicked_button: Option<PaneBtn>,
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
    /// Per-tab screen rects (in tab-strip mode). Empty in simple-symbol mode.
    /// Use these to anchor popups (the pane picker, etc.) to a specific tab.
    pub tab_rects: Vec<Rect>,
    /// Screen rect of the +Tab button when shown — for anchoring pickers
    /// triggered by the plus-tab click.
    pub plus_tab_rect: Option<Rect>,
    /// Phase 1c — screen rect of the split button, for anchoring the popup.
    pub split_btn_rect: Option<Rect>,
    /// Screen rect of the link-group dot — for anchoring the group picker popup.
    pub link_dot_rect: Option<Rect>,
}

impl PainterPaneHeaderResponse {
    /// Convenience: was the given right-cluster button clicked this frame?
    #[inline]
    pub fn clicked(&self, btn: PaneBtn) -> bool {
        self.clicked_button == Some(btn)
    }
}

// ─── Local helpers ──────────────────────────────────────────────────────────

fn nav_colors(enabled: bool, hovered: bool, t: &Theme, ui: &mut Ui) -> (Color32, Color32) {
    use super::super::style::alpha_dim;
    if enabled {
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            (tint(t, Tone::Border, alpha_dim()), t.text)
        } else {
            (Color32::TRANSPARENT, color_subtle(t.dim))
        }
    } else {
        (Color32::TRANSPARENT, color_very_dim(t.dim))
    }
}
