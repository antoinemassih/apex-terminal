//! Native GPU chart renderer — winit (any_thread) + egui for all rendering.
//! egui handles UI + chart painting. winit handles window on non-main thread.

use std::sync::{mpsc, Arc, Mutex};
use std::fmt::Write as FmtWrite;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes},
    dpi::PhysicalSize,
};

use super::{Bar, ChartCommand, Drawing, DrawingKind, DrawingGroup, LineStyle, PatternLabel};

// ── Replay overlay hook (sibling-branch ReplayScrubber feature) ────────────
//
// A single, scoped hook for rendering "replay" OHLCV bars on top of the live
// chart in a distinct color. Designed to be driven by the `ReplayScrubber`
// pane (see branch `sota-terminal-replay`, `replay_pane.rs`) — that pane
// installs/clears the overlay via the public methods on `Chart` below.
//
// This branch (`replay-overlay-hook`) is INDEPENDENT of `sota-terminal-replay`
// and is based on `main`. The overlay field stays `None` and the second
// render pass is a no-op until the scrubber pane lands and wires it up.
//
// Render contract (see render/pane.rs):
//   - Overlay bars share the same time axis as live bars: alignment is done
//     by matching `overlay.timestamps[i]` against `chart.timestamps` and
//     drawing at the resulting live-bar x-position. Overlay bars whose
//     timestamps fall outside the live `timestamps` window are skipped.
//   - Overlay candles are drawn semi-transparent in `overlay.color`
//     (alpha ~160), AFTER live candles but BEFORE drawings/annotations.
//   - When the overlay is active, a "REPLAY MODE: <label>" badge is shown
//     in the top-left of the chart pane.
//   - The live-bar render path is intentionally untouched.
#[derive(Clone, Debug)]
pub struct ReplayOverlay {
    /// OHLCV bars to render as the replay overlay.
    pub bars: Vec<Bar>,
    /// Timestamps (ms since epoch) parallel to `bars`. Used to align the
    /// overlay to the live chart's time axis.
    pub timestamps: Vec<i64>,
    /// Color used for the overlay candles (alpha is composed in the renderer).
    /// Default is a distinct orange so the overlay is clearly differentiated
    /// from live bull/bear coloring.
    pub color: egui::Color32,
    /// Label rendered in the top-left "REPLAY MODE" badge, e.g.
    /// "Replay: 2026-04-15 10:30:00".
    pub label: String,
}

impl ReplayOverlay {
    /// Distinct orange used as the default overlay color.
    pub const DEFAULT_COLOR: egui::Color32 = egui::Color32::from_rgb(0xff, 0xa5, 0x00);

    /// Construct an empty overlay with the given label and the default
    /// orange color.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            bars: Vec::new(),
            timestamps: Vec::new(),
            color: Self::DEFAULT_COLOR,
            label: label.into(),
        }
    }

    /// Append a single replay bar (used by the streaming/WS path).
    pub fn push(&mut self, bar: Bar, t_ms: i64) {
        self.bars.push(bar);
        self.timestamps.push(t_ms);
    }
}

/// Per-alert hit rects stashed each frame so the priority-0 click handler
/// can route clicks to PLACE/X buttons rendered via painter.
#[derive(Clone)]
pub(crate) struct AlertBadgeHit {
    pub(crate) alert_id: u32,
    pub(crate) is_draft: bool,
    pub(crate) place_rect: egui::Rect, // only valid for drafts
    pub(crate) x_rect: egui::Rect,
    pub(crate) drag_line_y: f32,
}

// Thread-local to pass window ref into draw_chart (which doesn't have access to ChartWindow)
std::thread_local! {
    pub(crate) static CURRENT_WINDOW: std::cell::RefCell<Option<Arc<Window>>> = const { std::cell::RefCell::new(None) };
    pub(crate) static CLOSE_REQUESTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PENDING_ALERT: std::cell::RefCell<Option<(String, f32, bool)>> = const { std::cell::RefCell::new(None) };
    pub(crate) static TB_BTN_CLICKED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static CONN_PANEL_OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(crate) static CROSSHAIR_SYNC_TIME: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
    pub(crate) static PENDING_WL_TOOLTIP: std::cell::RefCell<Option<WlTooltipData>> = const { std::cell::RefCell::new(None) };
    pub(crate) static ALERT_BADGE_HITS: std::cell::RefCell<Vec<AlertBadgeHit>> = const { std::cell::RefCell::new(Vec::new()) };
    #[cfg(feature = "design-mode")]
    pub(crate) static DESIGN_INSPECTOR: std::cell::RefCell<Option<crate::design_inspector::Inspector>> = const { std::cell::RefCell::new(None) };
}

#[derive(Clone)]
pub(crate) struct WlTooltipData {
    pub(crate) sym: String, pub(crate) price: f32, pub(crate) prev_close: f32,
    pub(crate) day_high: f32, pub(crate) day_low: f32, pub(crate) high_52wk: f32, pub(crate) low_52wk: f32,
    pub(crate) atr: f32, pub(crate) rvol: f32, pub(crate) avg_range: f32, pub(crate) earnings_days: i32,
    pub(crate) tags: Vec<String>, pub(crate) alert_triggered: bool,
    pub(crate) anchor_y: f32, pub(crate) sidebar_left: f32,
}

pub(crate) fn set_pending_wl_tooltip(data: Option<WlTooltipData>) {
    PENDING_WL_TOOLTIP.with(|t| *t.borrow_mut() = data);
}

use crate::ui_kit::{self};

use super::trading::*;
pub(crate) use super::trading::APEXIB_URL;

// ─── Split-pane sidebar sections ──────────────────────────────────────────────

/// One subdivision of a sidebar — has its own tab selection and height fraction.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct SplitSection<T: Clone> {
    pub tab: T,
    pub frac: f32, // fraction of available space (0.0–1.0)
}

impl<T: Clone> SplitSection<T> {
    pub fn new(tab: T, frac: f32) -> Self { Self { tab, frac } }
}

// ─── Themes ───────────────────────────────────────────────────────────────────

// ─── 6-color contract (Zed-style palette discipline) ─────────────────────────
//
// Each theme exposes exactly 7 semantic foreground colors:
//
//   accent  — single primary action color (1 hue per theme)
//   bull    — gains, positive deltas, buy
//   bear    — losses, negative deltas, sell
//   text    — primary foreground
//   dim     — secondary foreground (use color_alpha or t.dim itself)
//   border  — separators, panel edges, faint structure
//   warn    — single warning color (alerts, fat-finger, freeze)
//
// Hierarchy comes from color_alpha() opacity stops (alpha_ghost() through
// alpha_heavy()), NOT from new hues. If you find yourself adding a 7th color,
// the answer is almost always "use accent at a different opacity instead."
//
// Background tokens (`bg`, `toolbar_bg`) are surface fills, not palette
// colors — they intentionally sit outside the 6-color contract.
//
// Legacy fields (gold, notification_red, pinned_row_tint, text_muted, hud_bg,
// hud_border, overlay_text, shadow_color, rrg_*) are kept as #[deprecated]
// derived getters so call sites compile-warn but do not break. Migrate them
// incrementally to color_alpha(t.<core>, ALPHA_*) instead.
//
// `cmd_palette` is the only documented exception: an 11-slot category badge
// palette where each slot needs a distinct hue (symbol/widget/overlay/etc).
// It is theme-invariant (CMD_PALETTE_DEFAULT) and shared across all themes.
#[derive(Clone)]
pub(crate) struct Theme {
    pub(crate) name: &'static str,
    // ── Backgrounds (surface fills, not palette) ────────────────────────────
    pub(crate) bg: egui::Color32,
    pub(crate) toolbar_bg: egui::Color32,
    // ── 6-color core foreground palette ─────────────────────────────────────
    pub(crate) accent: egui::Color32,
    pub(crate) bull:   egui::Color32,
    pub(crate) bear:   egui::Color32,
    pub(crate) text:   egui::Color32,
    pub(crate) dim:    egui::Color32,
    /// Separators, panel edges, faint structure. Used as `border` in the new
    /// contract; legacy alias `toolbar_border` is kept (heavy usage: 371 sites)
    /// and intentionally non-deprecated to avoid warning storm.
    pub(crate) toolbar_border: egui::Color32,
    /// Slightly more visible border for in-content dividers (Zed `border_variant`).
    /// Computed as a ~10% luminance shift from `bg` (vs ~5% for `toolbar_border`).
    /// In dark mode this is LIGHTER than `toolbar_border`; in light mode, darker.
    pub(crate) border_variant: egui::Color32,
    pub(crate) warn:   egui::Color32,
    /// Shared command-palette category badges. Theme-invariant. Documented
    /// exception to the 6-color rule — each slot is a distinct hue by design.
    pub(crate) cmd_palette: [egui::Color32; 11],
    // ── Legacy fields (kept for back-compat; prefer derived getters) ────────
    // These remain as fields so existing call-sites compile. New code should
    // use the deprecated getter forms below (e.g. `t.gold()`) which warn and
    // route through `color_alpha`. Eventually these fields will be removed.
    /// LEGACY: use `color_alpha(t.accent, alpha_heavy())`.
    pub(crate) gold: egui::Color32,
    /// LEGACY: use `t.bear` (or `t.warn` for non-loss alerts).
    pub(crate) notification_red: egui::Color32,
    /// LEGACY: use `color_alpha(t.bg, alpha_heavy())` — pure-black baseline.
    pub(crate) shadow_color: egui::Color32,
    /// LEGACY: use `t.text` (overlay text is just the primary foreground).
    pub(crate) overlay_text: egui::Color32,
    /// LEGACY: use `t.bull`.
    pub(crate) rrg_leading: egui::Color32,
    /// LEGACY: use `t.accent`.
    pub(crate) rrg_improving: egui::Color32,
    /// LEGACY: use `t.warn`.
    pub(crate) rrg_weakening: egui::Color32,
    /// LEGACY: use `t.bear`.
    pub(crate) rrg_lagging: egui::Color32,
    /// LEGACY: use `color_alpha(t.accent, alpha_ghost())`.
    pub(crate) pinned_row_tint: egui::Color32,
    /// LEGACY: use `color_alpha(t.dim, alpha_heavy())`.
    pub(crate) text_muted: egui::Color32,
    /// LEGACY: use `color_alpha(t.bg, alpha_solid())` for HUD overlays.
    pub(crate) hud_bg: egui::Color32,
    /// LEGACY: use `t.toolbar_border`.
    pub(crate) hud_border: egui::Color32,
    // P12 (2026-05-25): The 10 Zed-style overlay fields (element_*, ghost_*,
    // icon, icon_muted, icon_disabled, icon_accent) previously lived here as
    // pre-computed Color32 values, copied through 15 theme initializers.
    // Removed in favour of deriving in theme_impl.rs at the ComponentTheme
    // trait boundary from the core 6-color palette (text, accent, dim).
    // Single source of truth — palette overrides automatically affect overlays.
}

impl Theme {
    /// Single border color in the 6-color contract. Aliases `toolbar_border`
    /// (which is kept as a field for call-site compatibility).
    #[inline]
    pub(crate) fn border(&self) -> egui::Color32 { self.toolbar_border }
    /// Slightly more visible divider color for in-content separators
    /// (Zed `border_variant` token). See field doc.
    #[inline]
    pub(crate) fn border_variant(&self) -> egui::Color32 { self.border_variant }
}

/// Computes a hairline border color from a base background.
/// Shifts luminance by ~5% (13/255) — toward white if dark, toward black if light.
/// This is Zed's "barely there" border treatment.
pub(crate) const fn hairline_border(bg: egui::Color32) -> egui::Color32 {
    let r = bg.r() as i16;
    let g = bg.g() as i16;
    let b = bg.b() as i16;
    let is_dark = (r + g + b) < 384;
    let shift: i16 = if is_dark { 13 } else { -13 };
    let cr = r + shift; let cg = g + shift; let cb = b + shift;
    let cr = if cr < 0 { 0 } else if cr > 255 { 255 } else { cr };
    let cg = if cg < 0 { 0 } else if cg > 255 { 255 } else { cg };
    let cb = if cb < 0 { 0 } else if cb > 255 { 255 } else { cb };
    egui::Color32::from_rgb(cr as u8, cg as u8, cb as u8)
}

/// Slightly more visible companion to `hairline_border`. Shifts luminance
/// by ~10% (25/255) — used for in-content dividers (`border_variant`).
pub(crate) const fn hairline_border_variant(bg: egui::Color32) -> egui::Color32 {
    let r = bg.r() as i16;
    let g = bg.g() as i16;
    let b = bg.b() as i16;
    let is_dark = (r + g + b) < 384;
    let shift: i16 = if is_dark { 25 } else { -25 };
    let cr = r + shift; let cg = g + shift; let cb = b + shift;
    let cr = if cr < 0 { 0 } else if cr > 255 { 255 } else { cr };
    let cg = if cg < 0 { 0 } else if cg > 255 { 255 } else { cg };
    let cb = if cb < 0 { 0 } else if cb > 255 { 255 } else { cb };
    egui::Color32::from_rgb(cr as u8, cg as u8, cb as u8)
}
pub(crate) const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 { egui::Color32::from_rgb(r, g, b) }
/// Premultiplied RGBA — all callers must pass already-premultiplied RGB components.
pub(crate) const fn rgba_pre(r: u8, g: u8, b: u8, a: u8) -> egui::Color32 { egui::Color32::from_rgba_premultiplied(r, g, b, a) }

/// UI style presets — placeholder names for now. Selected style is shown
/// alongside the theme as e.g. "GruvBox/Meridien". Actual visual differences
/// will be wired later.
pub(crate) const STYLE_NAMES: &[&str] = &[
    "Meridien", "Aperture", "Octave", "Cadence", "Alto",
    "Mariner",  "Lucid",    "Relay",  "Glass",   "Contour",
];

/// Per-theme preferred **proportional** font, ported from the React mockup's
/// `--ds-font-ui` per `[data-ds]` block. Returns an index into the font table
/// used by `init_fonts` (0=JetBrains, 1=Inter, 2=PlusJakarta, 3=SpaceGrotesk,
/// 4=DM Sans, 5=Geist). `None` = no per-theme preference (use the user picker).
///
/// NOTE: monospace stays pinned to JetBrains Mono regardless (tabular-digit
/// policy in `init_fonts`). Alto/Mariner specify IBM Plex Sans in React, which
/// is NOT bundled here — they fall back to Geist (closest neutral technical
/// face). Bundle IBM Plex Sans/Mono TTFs for true Alto/Mariner fidelity.
pub(crate) fn style_preferred_font(style_id: u8) -> Option<usize> {
    match style_id {
        1 => Some(1), // Aperture → Inter (React: Inter Tight)
        3 => Some(1), // Cadence  → Inter
        4 => Some(6), // Alto     → IBM Plex Sans (React: --ds-font-ui: 'IBM Plex Sans')
        5 => Some(6), // Mariner  → IBM Plex Sans (same family — instrument panel)
        6 => Some(4), // Lucid    → DM Sans
        _ => None,    // Meridien / Octave / unnamed → user font picker
    }
}

/// Returns the style id for a watchlist's selected style.
/// Any valid index within the live preset list is returned as-is.
/// Out-of-range falls back to 0 (Meridien).
pub(crate) fn style_id(wl: &Watchlist) -> u8 {
    let presets = crate::chart_renderer::ui::style::list_style_presets();
    let idx = wl.style_idx as u8;
    if presets.iter().any(|(id, _)| *id == idx) { idx } else { 0 }
}

/// Style-aware non-tabs pane header height. Mirrors `PaneHeaderSize::header_h`
/// but lets specific styles tweak vertical density.
pub(crate) fn pane_header_h(wl: &Watchlist) -> f32 {
    use crate::chart_renderer::PaneHeaderSize;
    let base = wl.pane_header_size.header_h();
    let style_adj = match (style_id(wl), wl.pane_header_size) {
        (1, PaneHeaderSize::Compact) => base + 2.0,
        (2, PaneHeaderSize::Compact) => (base - 2.0).max(16.0),
        _ => base,
    };
    // Multiply by current().header_height_scale so the design-mode slider has effect.
    (style_adj * super::ui::style::current().header_height_scale).max(12.0)
}

/// Borderless-window edge resize. We draw our own chrome (`with_decorations(false)`),
/// which on Windows strips the OS sizing border — so without this, window edges
/// aren't grabbable and resize cursors never appear. This adds INVISIBLE grab bands
/// at the left/right/bottom edges + bottom corners that show the resize cursor and
/// start a native OS resize via `drag_resize_window`, WITHOUT adding any Windows
/// frame (the window stays fully custom). The TOP edge is intentionally left to the
/// titlebar (window move) to avoid fighting the toolbar drag handler. No-op while
/// maximized. Runs inside the egui frame so the cursor is applied via egui output.
fn window_resize_borders(ctx: &egui::Context, window: &Window) {
    use winit::window::ResizeDirection as RD;
    use egui::CursorIcon as CI;
    if window.is_maximized() { return; }
    let s = ctx.screen_rect();
    let b = 6.0_f32;   // edge band thickness
    let c = 14.0_f32;  // corner square (takes priority over the straight edges)
    // The L/R bands sit at Order::Foreground (above everything), so where they
    // overlap the toolbar they'd eat clicks on the top-nav window controls /
    // panel toggles. Start the side bands BELOW the toolbar so the whole top nav
    // stays clickable (you still resize from the lower part of the side edges).
    let side_top = (s.top() + c)
        .max(crate::chart_renderer::ui::style::toolbar_rect().bottom() + 2.0);
    let zones: [(egui::Rect, RD, CI, &str); 5] = [
        // bottom corners first (drawn last → on top → win the hit test)
        (egui::Rect::from_min_max(egui::pos2(s.left(), s.bottom()-c), egui::pos2(s.left()+c, s.bottom())), RD::SouthWest, CI::ResizeNeSw, "sw"),
        (egui::Rect::from_min_max(egui::pos2(s.right()-c, s.bottom()-c), s.right_bottom()), RD::SouthEast, CI::ResizeNwSe, "se"),
        // straight edges (inset by the corner size so corners stay distinct)
        (egui::Rect::from_min_max(egui::pos2(s.left()+c, s.bottom()-b), egui::pos2(s.right()-c, s.bottom())), RD::South, CI::ResizeVertical, "so"),
        (egui::Rect::from_min_max(egui::pos2(s.left(), side_top), egui::pos2(s.left()+b, s.bottom()-c)), RD::West, CI::ResizeHorizontal, "we"),
        (egui::Rect::from_min_max(egui::pos2(s.right()-b, side_top), egui::pos2(s.right(), s.bottom()-c)), RD::East, CI::ResizeHorizontal, "ea"),
    ];
    // Edges first, then corners last so corners win where they overlap.
    for (rect, dir, cursor, sfx) in [zones[2], zones[3], zones[4], zones[0], zones[1]] {
        let resp = egui::Area::new(egui::Id::new(("winrsz", sfx)))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .interactable(true)
            .show(ctx, |ui| {
                ui.set_min_size(rect.size());
                ui.allocate_rect(rect, egui::Sense::drag())
            })
            .inner;
        if resp.hovered() || resp.dragged() { ctx.set_cursor_icon(cursor); }
        if resp.drag_started() { let _ = window.drag_resize_window(dir); }
    }
}

/// Paint rounded corner + border frames over each chart pane on the Foreground layer.
/// Called after `draw_chart` inside the egui run closure so the frames sit on top of
/// chart content. Only fires for styles with `r_md > 0` and `pane_gap > 0` (tiled
/// card styles like Aperture and Glass).
fn paint_pane_card_frames(ctx: &egui::Context, panes: &[Chart], layout: Layout, wl: &Watchlist) {
    use crate::chart_renderer::ui::style::{current as style_current, color_alpha};
    let st = style_current();
    // Only paint when the style requests visible tiled cards (Aperture/Glass).
    if st.pane_gap <= 0.0 || st.r_md == 0 { return; }
    let gap = st.pane_gap;
    let cr = egui::CornerRadius::same(st.r_md);
    let border_alpha = 40u8; // subtle border matching React's --ds-border-dim (~rgba(255,255,255,0.06))

    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("pane_card_frames"));
    let painter = ctx.layer_painter(layer);
    let t = crate::chart_renderer::theme_impl::active_theme(ctx);
    let border_col = color_alpha(t.toolbar_border, border_alpha + 20);

    // Compute pane rects (mirror of `pane_rects_for_layout`).
    let screen = ctx.screen_rect();
    // Approximate the chart area (below toolbar, above bottom bar).
    // We use the full screen minus the toolbar height as a best-effort rect.
    let toolbar_h = (if wl.compact_mode { 30.0 } else { 38.0 }) * st.toolbar_height_scale;
    let chart_area = egui::Rect::from_min_max(
        egui::pos2(screen.left(), screen.top() + toolbar_h),
        screen.max,
    );

    let visible = panes.len().min(layout.max_panes());
    if visible <= 1 { return; }

    let rects = compute_pane_rects_for_frame(wl, layout, chart_area, visible, gap);
    let bg_col = t.bg;
    let r = st.r_md as f32;
    for rect in rects.iter().take(visible) {
        if !rect.is_finite() || rect.width() < 8.0 || rect.height() < 8.0 { continue; }

        // ── Corner masks: paint bg-colored L-shaped patches over each corner ──
        // This hides the chart content's square corners, approximating clip-to-radius.
        // Each corner needs two rect fills forming an L at radius r.
        let corners = [
            (rect.left(), rect.top()),                     // top-left
            (rect.right() - r, rect.top()),                // top-right
            (rect.left(), rect.bottom() - r),              // bottom-left
            (rect.right() - r, rect.bottom() - r),        // bottom-right
        ];
        let is_tl_or_tr = |i: usize| i < 2;
        let is_left = |i: usize| i % 2 == 0;
        for (i, (cx, cy)) in corners.iter().enumerate() {
            // Horizontal strip (full width r, partial height r/2)
            let h_rect = egui::Rect::from_min_size(
                egui::pos2(*cx, *cy),
                egui::vec2(r, r * 0.45),
            );
            // Vertical strip (partial width r/2, full height r)
            let v_rect = egui::Rect::from_min_size(
                egui::pos2(*cx, *cy),
                egui::vec2(r * 0.45, r),
            );
            let _ = (is_tl_or_tr(i), is_left(i)); // suppress unused warnings
            painter.rect_filled(h_rect, egui::CornerRadius::ZERO, bg_col);
            painter.rect_filled(v_rect, egui::CornerRadius::ZERO, bg_col);
        }

        // ── Rounded border: defines the card edge on top of corner masks ──
        painter.rect_stroke(
            *rect, cr,
            egui::Stroke::new(st.pane_border_width.max(1.0), border_col),
            egui::StrokeKind::Inside,
        );
    }

    // ── Toolbar card border (Aperture/Glass tiled look) ──────────────────────
    // The toolbar is also a floating tile in Aperture. Paint its card border
    // using the stored toolbar rect (set each frame by set_toolbar_rect).
    let tb = super::ui::style::toolbar_rect();
    if tb.is_finite() && tb.width() > 8.0 {
        painter.rect_stroke(tb, cr,
            egui::Stroke::new(st.pane_border_width.max(1.0), border_col),
            egui::StrokeKind::Inside);
    }
}

/// Style-aware tabs pane header height. Mirrors `PaneHeaderSize::tabs_header_h`.
pub(crate) fn pane_tabs_header_h(wl: &Watchlist) -> f32 {
    use crate::chart_renderer::PaneHeaderSize;
    let base = wl.pane_header_size.tabs_header_h();
    let style_adj = match (style_id(wl), wl.pane_header_size) {
        (1, PaneHeaderSize::Compact) => base + 2.0,
        (2, PaneHeaderSize::Compact) => (base - 2.0).max(20.0),
        _ => base,
    };
    (style_adj * super::ui::style::current().header_height_scale).max(16.0)
}

// ┌─ THEMES_BEGIN ──────────────────────────────────────────────────────────────
/// Shared command-palette category badge palette (theme-invariant hardcoded colors).
/// Slots: [symbol, widget, overlay, theme_cat, timeframe, layout, play, alert, ai, dynamic, calc]
pub(crate) const CMD_PALETTE_DEFAULT: [egui::Color32; 11] = [
    rgb(120,180,255), // symbol
    rgb(180,140,240), // widget
    rgb(160,200,140), // overlay
    rgb(240,180,140), // theme
    rgb(140,220,200), // timeframe
    rgb(220,200,120), // layout
    rgb(240,140,180), // play
    rgb(240,120,120), // alert
    rgb(255,120,200), // ai
    rgb(255,180, 80), // dynamic
    rgb(140,240,200), // calc
];

/// Const-eval companion of `color_alpha`: re-stamps a Color32's alpha channel.
/// Used inside the `THEMES` const array to derive element-state overlays and
/// icon ramps from existing tokens. Mirrors `ui::style::color_alpha` semantics
/// (unmultiplied RGB + alpha) but produced as a premultiplied Color32 because
/// only `from_rgba_premultiplied` is const-evaluable in egui. Result is
/// visually identical to `color_alpha(c, a)` — premultiplication is the
/// physical encoding egui uses internally.
pub(crate) const fn alpha(c: egui::Color32, a: u8) -> egui::Color32 {
    let pr = ((c.r() as u16 * a as u16) / 255) as u8;
    let pg = ((c.g() as u16 * a as u16) / 255) as u8;
    let pb = ((c.b() as u16 * a as u16) / 255) as u8;
    egui::Color32::from_rgba_premultiplied(pr, pg, pb, a)
}

/// Computes a hover overlay color appropriate for the bg's luminance.
/// On dark bgs: tints toward white (lighten on hover).
/// On light bgs: tints toward black (darken on hover).
/// Mirrors Zed's approach: dark themes use a light wash, light themes use a
/// dark wash. Returned premultiplied (matching `alpha()` semantics) so it
/// composites identically over the theme bg.
pub(crate) const fn element_overlay(bg: egui::Color32, a: u8) -> egui::Color32 {
    let sum = bg.r() as u16 + bg.g() as u16 + bg.b() as u16;
    if sum < 384 {
        // Dark bg → tint toward white. Premultiplied white at alpha `a`.
        egui::Color32::from_rgba_premultiplied(a, a, a, a)
    } else {
        // Light bg → tint toward black. Premultiplied black at alpha `a`.
        egui::Color32::from_rgba_premultiplied(0, 0, 0, a)
    }
}

/// Legacy compile-time theme catalogue. **Test-only as of P1.3.**
///
/// The runtime no longer reads from this array — `LIVE_THEMES` is populated
/// from `design_system::builtin_color_schemes()` via the adapter at startup
/// (see `live_themes()`). This const is retained exclusively as the reference
/// ground truth for `design_system::equivalence_tests`, which verifies that
/// the adapter's output is byte-identical to these entries for all 16 schemes.
///
/// Do NOT add new runtime call sites against this. Use `get_theme(idx)` or
/// `get_all_themes()` instead, which read the live store.
///
/// Also compiled under `design-mode`: the design inspector's "Reset all to
/// defaults" and save-to-source features treat this const as the default-theme
/// ground truth (it edits the `THEMES_BEGIN/END` markers below). Kept out of
/// normal release builds.
#[cfg(any(test, feature = "design-mode"))]
pub(crate) const THEMES: &[Theme] = &[
    Theme { name: "Midnight",    bg: rgb(14,16,21),   bull: rgb(62,120,180),  bear: rgb(180,65,58),   dim: rgb(100,105,115), toolbar_bg: rgb(10,12,17),  toolbar_border: hairline_border(rgb(14,16,21)), border_variant: hairline_border_variant(rgb(14,16,21)),  accent: rgb(62,120,180),  text: rgb(220,220,230),  warn: rgb(255,191,  0), notification_red: rgb(231, 76, 60), gold: rgb(255,193, 37), shadow_color: rgb(0,0,0),       overlay_text: rgb(240,240,250), rrg_leading: rgb(56,203,137), rrg_improving: rgb(74,158,255), rrg_weakening: rgb(230,200,50), rrg_lagging: rgb(224,82,82), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(3,5,9,12), text_muted: rgb(180,180,195), hud_bg: rgba_pre(12,12,18,230), hud_border: rgb(50,52,64), },
    Theme { name: "Nord",        bg: rgb(38,44,56),   bull: rgb(163,190,140), bear: rgb(191,97,106),  dim: rgb(129,161,193), toolbar_bg: rgb(32,38,50),  toolbar_border: hairline_border(rgb(38,44,56)), border_variant: hairline_border_variant(rgb(38,44,56)),  accent: rgb(136,192,208), text: rgb(220,220,230),  warn: rgb(235,203,139), notification_red: rgb(191, 97,106), gold: rgb(235,203,139), shadow_color: rgb(0,0,0),       overlay_text: rgb(236,239,244), rrg_leading: rgb(163,190,140), rrg_improving: rgb(136,192,208), rrg_weakening: rgb(235,203,139), rrg_lagging: rgb(191,97,106), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(5,7,9,14), text_muted: rgb(175,180,190), hud_bg: rgba_pre(30,34,46,230), hud_border: rgb(60,66,80), },
    Theme { name: "Monokai",     bg: rgb(39,40,34),   bull: rgb(166,226,46),  bear: rgb(249,38,114),  dim: rgb(165,159,133), toolbar_bg: rgb(33,34,28),  toolbar_border: hairline_border(rgb(39,40,34)), border_variant: hairline_border_variant(rgb(39,40,34)),  accent: rgb(230,219,116), text: rgb(220,220,230),  warn: rgb(230,219,116), notification_red: rgb(249, 38,114), gold: rgb(255,193, 37), shadow_color: rgb(0,0,0),       overlay_text: rgb(248,248,240), rrg_leading: rgb(166,226, 46), rrg_improving: rgb(102,217,239), rrg_weakening: rgb(230,219,116), rrg_lagging: rgb(249,38,114), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(4,10,11,12), text_muted: rgb(180,178,160), hud_bg: rgba_pre(30,30,24,230), hud_border: rgb(55,54,44), },
    Theme { name: "Solarized",   bg: rgb(0,43,54),    bull: rgb(133,153,0),   bear: rgb(220,50,47),   dim: rgb(131,148,150), toolbar_bg: rgb(0,37,48),   toolbar_border: hairline_border(rgb(0,43,54)), border_variant: hairline_border_variant(rgb(0,43,54)),   accent: rgb(42,161,152),  text: rgb(220,220,230),  warn: rgb(181,137,  0), notification_red: rgb(220, 50, 47), gold: rgb(181,137,  0), shadow_color: rgb(0,0,0),       overlay_text: rgb(253,246,227), rrg_leading: rgb(133,153,  0), rrg_improving: rgb( 38,139,210), rrg_weakening: rgb(181,137,  0), rrg_lagging: rgb(220,50, 47), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(1,6,9,12), text_muted: rgb(156,172,175), hud_bg: rgba_pre(0,28,36,230), hud_border: rgb(7,54,66), },
    Theme { name: "Dracula",     bg: rgb(40,42,54),   bull: rgb(80,250,123),  bear: rgb(255,85,85),   dim: rgb(189,147,249), toolbar_bg: rgb(34,36,48),  toolbar_border: hairline_border(rgb(40,42,54)), border_variant: hairline_border_variant(rgb(40,42,54)),  accent: rgb(255,121,198), text: rgb(220,220,230),  warn: rgb(241,250,140), notification_red: rgb(255, 85, 85), gold: rgb(241,250,140), shadow_color: rgb(0,0,0),       overlay_text: rgb(248,248,242), rrg_leading: rgb( 80,250,123), rrg_improving: rgb(139,233,253), rrg_weakening: rgb(241,250,140), rrg_lagging: rgb(255,85, 85), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(6,10,11,12), text_muted: rgb(190,185,215), hud_bg: rgba_pre(30,32,44,230), hud_border: rgb(55,58,75), },
    Theme { name: "Gruvbox",     bg: rgb(40,40,40),   bull: rgb(184,187,38),  bear: rgb(251,73,52),   dim: rgb(213,196,161), toolbar_bg: rgb(34,34,34),  toolbar_border: hairline_border(rgb(40,40,40)), border_variant: hairline_border_variant(rgb(40,40,40)),  accent: rgb(254,128,25),  text: rgb(220,220,230),  warn: rgb(250,189, 47), notification_red: rgb(251, 73, 52), gold: rgb(250,189, 47), shadow_color: rgb(0,0,0),       overlay_text: rgb(235,219,178), rrg_leading: rgb(184,187, 38), rrg_improving: rgb(131,165,152), rrg_weakening: rgb(250,189, 47), rrg_lagging: rgb(251,73, 52), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(6,8,7,13), text_muted: rgb(185,178,160), hud_bg: rgba_pre(28,28,28,230), hud_border: rgb(60,56,50), },
    Theme { name: "Catppuccin",  bg: rgb(30,30,46),   bull: rgb(166,227,161), bear: rgb(243,139,168), dim: rgb(180,190,254), toolbar_bg: rgb(24,24,38),  toolbar_border: hairline_border(rgb(30,30,46)), border_variant: hairline_border_variant(rgb(30,30,46)),  accent: rgb(203,166,247), text: rgb(220,220,230),  warn: rgb(249,226,175), notification_red: rgb(243,139,168), gold: rgb(249,226,175), shadow_color: rgb(0,0,0),       overlay_text: rgb(205,214,244), rrg_leading: rgb(166,227,161), rrg_improving: rgb(137,220,235), rrg_weakening: rgb(249,226,175), rrg_lagging: rgb(243,139,168), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(6,8,11,12), text_muted: rgb(182,186,220), hud_bg: rgba_pre(20,20,36,230), hud_border: rgb(49,50,68), },
    Theme { name: "Tokyo Night", bg: rgb(26,27,38),   bull: rgb(158,206,106), bear: rgb(247,118,142), dim: rgb(122,162,247), toolbar_bg: rgb(21,22,32),  toolbar_border: hairline_border(rgb(26,27,38)), border_variant: hairline_border_variant(rgb(26,27,38)),  accent: rgb(125,207,255), text: rgb(220,220,230),  warn: rgb(224,175,104), notification_red: rgb(247,118,142), gold: rgb(224,175,104), shadow_color: rgb(0,0,0),       overlay_text: rgb(192,202,245), rrg_leading: rgb(158,206,106), rrg_improving: rgb(125,207,255), rrg_weakening: rgb(224,175,104), rrg_lagging: rgb(247,118,142), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(5,9,12,12), text_muted: rgb(172,178,220), hud_bg: rgba_pre(18,18,28,230), hud_border: rgb(40,44,62), },
    // ── Additional themes ──
    Theme { name: "Kanagawa",    bg: rgb(22,22,29),   bull: rgb(118,169,130), bear: rgb(195,64,67),   dim: rgb(84,88,104),   toolbar_bg: rgb(18,18,24),  toolbar_border: hairline_border(rgb(22,22,29)), border_variant: hairline_border_variant(rgb(22,22,29)),  accent: rgb(127,180,202), text: rgb(220,220,230),  warn: rgb(228,175, 69), notification_red: rgb(195, 64, 67), gold: rgb(228,175, 69), shadow_color: rgb(0,0,0),       overlay_text: rgb(220,215,186), rrg_leading: rgb(118,169,130), rrg_improving: rgb(127,180,202), rrg_weakening: rgb(228,175, 69), rrg_lagging: rgb(195,64, 67), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(5,8,9,12), text_muted: rgb(155,158,175), hud_bg: rgba_pre(14,14,20,230), hud_border: rgb(36,36,50), },
    Theme { name: "Everforest",  bg: rgb(39,46,38),   bull: rgb(167,192,128), bear: rgb(230,126,128), dim: rgb(157,169,140), toolbar_bg: rgb(33,40,32),  toolbar_border: hairline_border(rgb(39,46,38)), border_variant: hairline_border_variant(rgb(39,46,38)),  accent: rgb(131,165,152), text: rgb(220,220,230),  warn: rgb(223,199,118), notification_red: rgb(230,126,128), gold: rgb(223,199,118), shadow_color: rgb(0,0,0),       overlay_text: rgb(211,198,170), rrg_leading: rgb(167,192,128), rrg_improving: rgb(131,165,152), rrg_weakening: rgb(223,199,118), rrg_lagging: rgb(230,126,128), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(6,8,7,13), text_muted: rgb(175,178,162), hud_bg: rgba_pre(28,34,28,230), hud_border: rgb(52,60,50), },
    Theme { name: "Vesper",      bg: rgb(16,16,16),   bull: rgb(166,218,149), bear: rgb(238,130,98),  dim: rgb(120,120,120), toolbar_bg: rgb(11,11,11),  toolbar_border: hairline_border(rgb(16,16,16)), border_variant: hairline_border_variant(rgb(16,16,16)),  accent: rgb(255,199,119), text: rgb(220,220,230),  warn: rgb(255,199,119), notification_red: rgb(238,130, 98), gold: rgb(255,193, 37), shadow_color: rgb(0,0,0),       overlay_text: rgb(230,230,230), rrg_leading: rgb(166,218,149), rrg_improving: rgb( 74,158,255), rrg_weakening: rgb(255,199,119), rrg_lagging: rgb(238,130, 98), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(3,6,11,11), text_muted: rgb(170,170,180), hud_bg: rgba_pre(10,10,10,230), hud_border: rgb(42,42,42), },
    Theme { name: "Rosé Pine",   bg: rgb(25,23,36),   bull: rgb(156,207,216), bear: rgb(235,111,146), dim: rgb(110,106,134), toolbar_bg: rgb(20,18,30),  toolbar_border: hairline_border(rgb(25,23,36)), border_variant: hairline_border_variant(rgb(25,23,36)),  accent: rgb(196,167,231), text: rgb(220,220,230),  warn: rgb(246,193,119), notification_red: rgb(235,111,146), gold: rgb(246,193,119), shadow_color: rgb(0,0,0),       overlay_text: rgb(224,222,244), rrg_leading: rgb(156,207,216), rrg_improving: rgb(196,167,231), rrg_weakening: rgb(246,193,119), rrg_lagging: rgb(235,111,146), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(7,9,10,12), text_muted: rgb(167,162,187), hud_bg: rgba_pre(18,16,28,230), hud_border: rgb(44,40,58), },
    // ── Light themes ──
    Theme { name: "Bauhaus",     bg: rgb(242,242,238), bull: rgb(20,120,60),   bear: rgb(200,55,45),   dim: rgb(120,125,130), toolbar_bg: rgb(248,248,245), toolbar_border: hairline_border(rgb(242,242,238)), border_variant: hairline_border_variant(rgb(242,242,238)), accent: rgb(232,93,38),   text: rgb(22,22,24),   warn: rgb(204,120,  0), notification_red: rgb(200, 55, 45), gold: rgb(204,153,  0), shadow_color: rgb(40,40,40),    overlay_text: rgb( 20, 20, 22), rrg_leading: rgb( 20,120, 60), rrg_improving: rgb( 30,100,180), rrg_weakening: rgb(180,140,  0), rrg_lagging: rgb(200,55, 45), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(1,5,9,14), text_muted: rgb(100,102,110), hud_bg: rgba_pre(20,20,20,220), hud_border: rgb(80,82,88), },
    Theme { name: "Peach",       bg: rgb(243,241,238), bull: rgb(22,130,70),   bear: rgb(195,50,55),   dim: rgb(115,120,125), toolbar_bg: rgb(250,248,246), toolbar_border: hairline_border(rgb(243,241,238)), border_variant: hairline_border_variant(rgb(243,241,238)), accent: rgb(210,95,70),   text: rgb(20,20,22),   warn: rgb(200,130,  0), notification_red: rgb(195, 50, 55), gold: rgb(200,150,  0), shadow_color: rgb(40,40,40),    overlay_text: rgb( 20, 20, 22), rrg_leading: rgb( 22,130, 70), rrg_improving: rgb( 30,100,180), rrg_weakening: rgb(180,140,  0), rrg_lagging: rgb(195,50, 55), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(1,5,9,14), text_muted: rgb(98,100,108), hud_bg: rgba_pre(20,20,20,220), hud_border: rgb(82,80,78), },
    Theme { name: "Ivory",       bg: rgb(240,242,238), bull: rgb(80,160,50),   bear: rgb(210,60,50),   dim: rgb(118,122,128), toolbar_bg: rgb(248,250,246), toolbar_border: hairline_border(rgb(240,242,238)), border_variant: hairline_border_variant(rgb(240,242,238)), accent: rgb(160,190,40),  text: rgb(18,20,22),   warn: rgb(190,140,  0), notification_red: rgb(210, 60, 50), gold: rgb(190,150,  0), shadow_color: rgb(40,40,40),    overlay_text: rgb( 18, 20, 22), rrg_leading: rgb( 80,160, 50), rrg_improving: rgb( 30,100,180), rrg_weakening: rgb(180,140,  0), rrg_lagging: rgb(210,60, 50), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(1,5,9,14), text_muted: rgb(100,102,108), hud_bg: rgba_pre(18,20,18,220), hud_border: rgb(80,82,80), },
    Theme { name: "Newsprint",   bg: rgb(238,232,220), bull: rgb(34,94,56),    bear: rgb(168,52,52),   dim: rgb(120,116,104), toolbar_bg: rgb(238,232,220), toolbar_border: hairline_border(rgb(238,232,220)), border_variant: hairline_border_variant(rgb(238,232,220)), accent: rgb(34,94,56),    text: rgb(28,28,28),   warn: rgb(168,120,  0), notification_red: rgb(168, 52, 52), gold: rgb(168,130,  0), shadow_color: rgb(60,50,40),    overlay_text: rgb( 28, 28, 28), rrg_leading: rgb( 34, 94, 56), rrg_improving: rgb( 30, 90,160), rrg_weakening: rgb(160,120,  0), rrg_lagging: rgb(168,52, 52), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(1,4,8,13), text_muted: rgb(105,100,90), hud_bg: rgba_pre(28,24,18,220), hud_border: rgb(90,82,68), },
];
// └─ THEMES_END ────────────────────────────────────────────────────────────────

impl Theme {
    pub(crate) const fn is_light(&self) -> bool {
        // A theme is "light" if the background luminance is above ~50%
        (self.bg.r() as u16 + self.bg.g() as u16 + self.bg.b() as u16) > 400
    }
}

// ─── Live theme registry (extracted to `theme_registry`, WS-E E2) ─────────────
// Re-exported so `gpu::get_theme()` / `gpu::append_installed_themes()` / ... plus
// the internal live_themes() call sites are all unchanged.
pub(crate) use super::theme_registry::{live_themes, get_theme, set_theme, get_all_themes, live_theme_count};
pub use super::theme_registry::{append_installed_themes, upsert_installed_themes};

// ─── Phase 1c — pending pane-close queue ─────────────────────────────────────
//
// Set by a pane's close-pane button click (per-pane loop), drained after the
// loop completes so Vec<Chart> isn't mutated mid-iteration. Indices may
// shift after a remove; the drain sorts descending so each remove is safe
// against the indices that follow.
thread_local! {
    pub(crate) static PENDING_PANE_CLOSE: std::cell::RefCell<Vec<usize>> =
        std::cell::RefCell::new(Vec::new());
}

// ─── Pane layout & lifecycle (extracted to `pane_layout`, WS-E E2) ──────────
// Re-exported so `gpu::compute_pane_rects_for_frame()` / `gpu::alloc_pane_id()`
// / ... and gpu.rs's own bare compute_pane_rects_for_frame() call are unchanged.
pub(crate) use super::pane_ops::*;

// ─── Simulation constants ────────────────────────────────────────────────────
const SIM_TICK_FRAMES: u64 = 5;           // Update price every N frames (~12 ticks/sec at 60fps)
const SIM_CANDLE_MS: u128 = 3000;         // New simulated candle every 3s
const SIM_VOLATILITY: f32 = 0.0005;       // Per-tick price change magnitude (~0.05%)
const SIM_REVERSION: f32 = 0.003;         // Mean-reversion strength toward candle open
const SIM_VOL_BASE: f32 = 1000.0;         // Minimum volume per tick
const SIM_VOL_RANGE: f32 = 8000.0;        // Random volume range above base
const SIM_DEFAULT_INTERVAL: i64 = 300;    // Default bar interval (5 min) when no timestamps
const AUTO_SCROLL_RESUME_SECS: u64 = 5;   // Resume auto-scroll after N seconds of inactivity
pub(crate) const CHART_RIGHT_PAD: u32 = 20;           // Empty bars of space to the right of latest bar
pub(crate) const MAX_RECENT_SYMBOLS: usize = 20;     // Max entries in recent symbols list
pub(crate) const MAX_SEARCH_RESULTS: usize = 15;     // Max Yahoo/static search results

// Shared helpers (only the symbols actually used in gpu.rs — the rest of
// the historical import list was untouched by cargo and is unused; cleaned
// up 2026-05 to drop 32 dead names that produced "unused imports" warnings).
use super::ui::style::{
    draw_line_rgba, color_alpha, color_dim, color_half, color_very_dim,
    alpha_muted, alpha_dim, alpha_strong, alpha_active,
};
use super::ui::style as style;
use super::compute::{compute_sma, compute_ema, compute_rsi, compute_macd, compute_stochastic, compute_vwap, detect_divergences, compute_atr, compute_bollinger, compute_ichimoku, compute_psar, compute_supertrend, compute_keltner, compute_adx, compute_cci, compute_williams_r, compute_obv};

// compute_sma, compute_ema — now in compute.rs

// ─── Layout ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Layout {
    One, Two, TwoH,
    Three,      // 1 top + 2 bottom
    ThreeL,     // 1 big left + 2 stacked right
    ThreeR,     // 2 stacked left + 1 big right
    Four,       // 2×2 grid
    FourL,      // 1 big left + 3 stacked right
    FiveC,      // 2 left + 1 big center + 2 right
    FiveL,      // 2 stacked left + 3 stacked right
    FiveW,      // 1 wide top + 2×2 bottom (all horizontal)
    FiveR,      // 2 top + 1 middle + 2 bottom (all horizontal rows)
    Six, SixH,
    SixL,       // 2 big stacked left + 4 stacked right
    Seven,      // 1 big top + 6 bottom (3 cols × 2 rows)
    EightH,     // 4 horizontal stacked left + 4 horizontal stacked right
    Nine,
}

impl Layout {
    pub(crate) fn max_panes(self) -> usize { match self {
        Layout::One=>1, Layout::Two|Layout::TwoH=>2,
        Layout::Three|Layout::ThreeL|Layout::ThreeR=>3, Layout::Four|Layout::FourL=>4,
        Layout::FiveC|Layout::FiveL|Layout::FiveW|Layout::FiveR=>5,
        Layout::Six|Layout::SixH|Layout::SixL=>6,
        Layout::Seven=>7, Layout::EightH=>8, Layout::Nine=>9,
    }}
    pub(crate) fn label(self) -> &'static str { match self {
        Layout::One=>"1", Layout::Two=>"2", Layout::TwoH=>"2H",
        Layout::Three=>"3", Layout::ThreeL=>"3L", Layout::ThreeR=>"3R",
        Layout::Four=>"4", Layout::FourL=>"4L",
        Layout::FiveC=>"5C", Layout::FiveL=>"5L", Layout::FiveW=>"5W", Layout::FiveR=>"5R",
        Layout::Six=>"6", Layout::SixH=>"6H", Layout::SixL=>"6L",
        Layout::Seven=>"7", Layout::EightH=>"8H", Layout::Nine=>"9",
    }}
    pub(crate) fn description(self) -> &'static str { match self {
        Layout::One=>"Single pane", Layout::Two=>"2 side-by-side", Layout::TwoH=>"2 stacked",
        Layout::Three=>"1 top + 2 bottom", Layout::ThreeL=>"1 left + 2 right", Layout::ThreeR=>"2 left + 1 right",
        Layout::Four=>"2\u{00d7}2 grid", Layout::FourL=>"1 left + 3 right",
        Layout::FiveC=>"2L + 1 center + 2R", Layout::FiveL=>"2 left + 3 right",
        Layout::FiveW=>"1 wide top + 2\u{00d7}2", Layout::FiveR=>"2 + 1 + 2 rows",
        Layout::Six=>"2\u{00d7}3 grid", Layout::SixH=>"3 + 3 stacked",
        Layout::SixL=>"2 left + 4 right",
        Layout::Seven=>"1 top + 6 bottom", Layout::EightH=>"4 + 4 columns",
        Layout::Nine=>"3\u{00d7}3 grid",
    }}
    /// Section header for the layout dropdown
    pub(crate) fn section(self) -> &'static str { match self {
        Layout::One => "1 Pane",
        Layout::Two | Layout::TwoH => "2 Panes",
        Layout::Three | Layout::ThreeL | Layout::ThreeR => "3 Panes",
        Layout::Four | Layout::FourL => "4 Panes",
        Layout::FiveC | Layout::FiveL | Layout::FiveW | Layout::FiveR => "5 Panes",
        Layout::Six | Layout::SixH | Layout::SixL => "6 Panes",
        Layout::Seven | Layout::EightH | Layout::Nine => "7+ Panes",
    }}
    /// Returns (col, row) grid dimensions for each pane in the layout, given the total rect.
    /// For Layout::Three, returns a custom arrangement: 1 full-width top (60%) + 2 bottom (40%).
    pub(crate) fn pane_rects(self, rect: egui::Rect, count: usize, split_h: f32, split_v: f32, split_h2: f32, split_v2: f32, split_v3: f32, split_v4: f32, split_v5: f32, split_v6: f32) -> Vec<egui::Rect> {
        if count == 0 { return vec![]; }
        // pane_gap from StyleSettings lets the user control inter-pane spacing.
        let gap = super::ui::style::current().pane_gap;
        match self {
            Layout::Two if count >= 2 => {
                // Two side-by-side panes with adjustable horizontal split
                let left_w = (rect.width() - gap) * split_h.clamp(0.15, 0.85);
                let right_w = rect.width() - gap - left_w;
                vec![
                    egui::Rect::from_min_size(rect.min, egui::vec2(left_w, rect.height())),
                    egui::Rect::from_min_size(egui::pos2(rect.left() + left_w + gap, rect.top()), egui::vec2(right_w, rect.height())),
                ]
            }
            Layout::TwoH if count >= 2 => {
                // Two stacked panes with adjustable vertical split
                let top_h = (rect.height() - gap) * split_v.clamp(0.15, 0.85);
                let bot_h = rect.height() - gap - top_h;
                vec![
                    egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), top_h)),
                    egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top() + top_h + gap), egui::vec2(rect.width(), bot_h)),
                ]
            }
            Layout::Three if count >= 2 => {
                let top_h = (rect.height() - gap) * split_v.clamp(0.15, 0.85);
                let bot_h = rect.height() - top_h - gap;
                let top = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), top_h));
                let bot_count = (count - 1).min(2);
                let mut rects = vec![top];
                if bot_count == 2 {
                    let left_w = (rect.width() - gap) * split_h.clamp(0.15, 0.85);
                    let right_w = rect.width() - gap - left_w;
                    rects.push(egui::Rect::from_min_size(
                        egui::pos2(rect.left(), rect.top() + top_h + gap), egui::vec2(left_w, bot_h)));
                    rects.push(egui::Rect::from_min_size(
                        egui::pos2(rect.left() + left_w + gap, rect.top() + top_h + gap), egui::vec2(right_w, bot_h)));
                } else {
                    rects.push(egui::Rect::from_min_size(
                        egui::pos2(rect.left(), rect.top() + top_h + gap), egui::vec2(rect.width(), bot_h)));
                }
                rects
            }
            Layout::ThreeL if count >= 2 => {
                // 1 big left + 2 stacked right (split_v controls right side)
                let left_w = (rect.width() - gap) * split_h.clamp(0.2, 0.8);
                let right_w = rect.width() - gap - left_w;
                let rx = rect.left() + left_w + gap;
                let mut rects = vec![egui::Rect::from_min_size(rect.min, egui::vec2(left_w, rect.height()))];
                if count >= 3 {
                    let h0 = (rect.height() - gap) * split_v.clamp(0.15, 0.85);
                    let h1 = rect.height() - gap - h0;
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top()), egui::vec2(right_w, h0)));
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + h0 + gap), egui::vec2(right_w, h1)));
                } else {
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top()), egui::vec2(right_w, rect.height())));
                }
                rects
            }
            Layout::ThreeR if count >= 2 => {
                // 2 stacked left + 1 big right (split_v controls left side)
                let left_w = (rect.width() - gap) * split_h.clamp(0.2, 0.8);
                let right_w = rect.width() - gap - left_w;
                let rx = rect.left() + left_w + gap;
                let mut rects = Vec::new();
                if count >= 3 {
                    let h0 = (rect.height() - gap) * split_v.clamp(0.15, 0.85);
                    let h1 = rect.height() - gap - h0;
                    rects.push(egui::Rect::from_min_size(rect.min, egui::vec2(left_w, h0)));
                    rects.push(egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top() + h0 + gap), egui::vec2(left_w, h1)));
                } else {
                    rects.push(egui::Rect::from_min_size(rect.min, egui::vec2(left_w, rect.height())));
                }
                rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top()), egui::vec2(right_w, rect.height())));
                rects
            }
            Layout::FourL if count >= 2 => {
                // 1 big left + 3 stacked right (split_v + split_v2 control right side)
                let left_w = (rect.width() - gap) * split_h.clamp(0.2, 0.8);
                let right_w = rect.width() - gap - left_w;
                let rx = rect.left() + left_w + gap;
                let mut rects = vec![egui::Rect::from_min_size(rect.min, egui::vec2(left_w, rect.height()))];
                let n_right = (count - 1).min(3);
                if n_right == 3 {
                    let total_rh = rect.height() - gap * 2.0;
                    let h0 = total_rh * (split_v * 0.9).clamp(0.1, 0.5);
                    let rest = total_rh - h0;
                    let h1 = rest * split_v2.clamp(0.2, 0.8);
                    let h2 = rest - h1;
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top()), egui::vec2(right_w, h0)));
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + h0 + gap), egui::vec2(right_w, h1)));
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + h0 + gap + h1 + gap), egui::vec2(right_w, h2)));
                } else {
                    let rh = (rect.height() - gap * (n_right as f32 - 1.0).max(0.0)) / n_right as f32;
                    for i in 0..n_right {
                        rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + i as f32 * (rh + gap)), egui::vec2(right_w, rh)));
                    }
                }
                rects
            }
            Layout::FiveC if count >= 3 => {
                // 2 stacked left + 1 big center + 2 stacked right
                let side_w = (rect.width() - gap * 2.0) * 0.2;
                let center_w = rect.width() - gap * 2.0 - side_w * 2.0;
                let cx = rect.left() + side_w + gap;
                let rx = cx + center_w + gap;
                let half_h = (rect.height() - gap) / 2.0;
                let mut rects = Vec::new();
                // Left 2
                rects.push(egui::Rect::from_min_size(rect.min, egui::vec2(side_w, half_h)));
                rects.push(egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top() + half_h + gap), egui::vec2(side_w, half_h)));
                // Center
                rects.push(egui::Rect::from_min_size(egui::pos2(cx, rect.top()), egui::vec2(center_w, rect.height())));
                // Right 2
                let n_right = (count - 3).min(2);
                for i in 0..n_right {
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + i as f32 * (half_h + gap)), egui::vec2(side_w, half_h)));
                }
                rects
            }
            Layout::FiveL if count >= 2 => {
                // 2 stacked left + 3 stacked right
                let left_w = (rect.width() - gap) * split_h.clamp(0.2, 0.8);
                let right_w = rect.width() - gap - left_w;
                let rx = rect.left() + left_w + gap;
                let lh0 = (rect.height() - gap) * split_v.clamp(0.15, 0.85);
                let lh1 = rect.height() - gap - lh0;
                let mut rects = Vec::new();
                rects.push(egui::Rect::from_min_size(rect.min, egui::vec2(left_w, lh0)));
                rects.push(egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top() + lh0 + gap), egui::vec2(left_w, lh1)));
                let n_right = (count - 2).min(3);
                if n_right == 3 {
                    let total_rh = rect.height() - gap * 2.0;
                    let rh0 = total_rh * (split_v2 * 0.9).clamp(0.1, 0.5);
                    let rest = total_rh - rh0;
                    let rh1 = rest * split_v3.clamp(0.2, 0.8);
                    let rh2 = rest - rh1;
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top()), egui::vec2(right_w, rh0)));
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + rh0 + gap), egui::vec2(right_w, rh1)));
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + rh0 + gap + rh1 + gap), egui::vec2(right_w, rh2)));
                } else {
                    let rh = (rect.height() - gap * (n_right as f32 - 1.0).max(0.0)) / n_right.max(1) as f32;
                    for i in 0..n_right {
                        rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + i as f32 * (rh + gap)), egui::vec2(right_w, rh)));
                    }
                }
                rects
            }
            Layout::SixL if count >= 2 => {
                // 2 big stacked left + 4 stacked right
                let left_w = (rect.width() - gap) * split_h.clamp(0.2, 0.8);
                let right_w = rect.width() - gap - left_w;
                let rx = rect.left() + left_w + gap;
                let lh0 = (rect.height() - gap) * split_v.clamp(0.15, 0.85);
                let lh1 = rect.height() - gap - lh0;
                let mut rects = Vec::new();
                rects.push(egui::Rect::from_min_size(rect.min, egui::vec2(left_w, lh0)));
                rects.push(egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top() + lh0 + gap), egui::vec2(left_w, lh1)));
                let n_right = (count - 2).min(4);
                if n_right == 4 {
                    let total_rh = rect.height() - gap * 3.0;
                    let rh0 = total_rh * (split_v2 * 0.9).clamp(0.08, 0.4);
                    let rest0 = total_rh - rh0;
                    let rh1 = rest0 * (split_v3 * 0.9).clamp(0.1, 0.5);
                    let rest1 = rest0 - rh1;
                    let rh2 = rest1 * split_v4.clamp(0.2, 0.8);
                    let rh3 = rest1 - rh2;
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top()), egui::vec2(right_w, rh0)));
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + rh0 + gap), egui::vec2(right_w, rh1)));
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + rh0 + gap + rh1 + gap), egui::vec2(right_w, rh2)));
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + rh0 + gap + rh1 + gap + rh2 + gap), egui::vec2(right_w, rh3)));
                } else {
                    let rh = (rect.height() - gap * (n_right as f32 - 1.0).max(0.0)) / n_right.max(1) as f32;
                    for i in 0..n_right {
                        rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + i as f32 * (rh + gap)), egui::vec2(right_w, rh)));
                    }
                }
                rects
            }
            Layout::EightH if count >= 2 => {
                // 4 horizontal stacked left + 4 horizontal stacked right
                let left_w = (rect.width() - gap) * split_h.clamp(0.2, 0.8);
                let right_w = rect.width() - gap - left_w;
                let rx = rect.left() + left_w + gap;
                let n_left = count.min(4);
                let n_right = count.saturating_sub(4).min(4);
                let mut rects = Vec::new();
                // Left column: use split_v, split_v2, split_v3 for 4 panes
                if n_left == 4 {
                    let total_lh = rect.height() - gap * 3.0;
                    let lh0 = total_lh * (split_v * 0.9).clamp(0.08, 0.4);
                    let rest0 = total_lh - lh0;
                    let lh1 = rest0 * (split_v2 * 0.9).clamp(0.1, 0.5);
                    let rest1 = rest0 - lh1;
                    let lh2 = rest1 * split_v3.clamp(0.2, 0.8);
                    let lh3 = rest1 - lh2;
                    let ys = [0.0, lh0 + gap, lh0 + gap + lh1 + gap, lh0 + gap + lh1 + gap + lh2 + gap];
                    let hs = [lh0, lh1, lh2, lh3];
                    for i in 0..4 {
                        rects.push(egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top() + ys[i]), egui::vec2(left_w, hs[i])));
                    }
                } else {
                    let lh = (rect.height() - gap * (n_left as f32 - 1.0).max(0.0)) / n_left.max(1) as f32;
                    for i in 0..n_left {
                        rects.push(egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top() + i as f32 * (lh + gap)), egui::vec2(left_w, lh)));
                    }
                }
                // Right column: use split_v4, split_v5, split_v6 for 4 panes
                if n_right == 4 {
                    let total_rh = rect.height() - gap * 3.0;
                    let rh0 = total_rh * (split_v4 * 0.9).clamp(0.08, 0.4);
                    let rest0 = total_rh - rh0;
                    let rh1 = rest0 * (split_v5 * 0.9).clamp(0.1, 0.5);
                    let rest1 = rest0 - rh1;
                    let rh2 = rest1 * split_v6.clamp(0.2, 0.8);
                    let rh3 = rest1 - rh2;
                    let ys = [0.0, rh0 + gap, rh0 + gap + rh1 + gap, rh0 + gap + rh1 + gap + rh2 + gap];
                    let hs = [rh0, rh1, rh2, rh3];
                    for i in 0..4 {
                        rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + ys[i]), egui::vec2(right_w, hs[i])));
                    }
                } else {
                    let rh = (rect.height() - gap * (n_right as f32 - 1.0).max(0.0)) / n_right.max(1) as f32;
                    for i in 0..n_right {
                        rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + i as f32 * (rh + gap)), egui::vec2(right_w, rh)));
                    }
                }
                rects
            }
            Layout::FiveW if count >= 2 => {
                // 1 wide top + 2 cols × 2 rows bottom (all horizontal)
                let top_h = (rect.height() - gap) * split_v.clamp(0.15, 0.85);
                let bot_h = rect.height() - gap - top_h;
                let mut rects = vec![egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), top_h))];
                let by = rect.top() + top_h + gap;
                let half_w = (rect.width() - gap) / 2.0;
                let half_bh = (bot_h - gap) / 2.0;
                let n_bot = (count - 1).min(4);
                // 2 cols × 2 rows: [TL, TR, BL, BR]
                let positions = [(0.0, 0.0), (half_w + gap, 0.0), (0.0, half_bh + gap), (half_w + gap, half_bh + gap)];
                for i in 0..n_bot {
                    let (dx, dy) = positions[i];
                    rects.push(egui::Rect::from_min_size(egui::pos2(rect.left() + dx, by + dy), egui::vec2(half_w, half_bh)));
                }
                rects
            }
            Layout::FiveR if count >= 2 => {
                // 2 top + 1 middle + 2 bottom (all horizontal rows)
                let total_h = rect.height() - gap * 2.0;
                let row_h = total_h / 3.0;
                let half_w = (rect.width() - gap) / 2.0;
                let mut rects = Vec::new();
                // Top row: 2 panes
                let n_top = (count).min(2);
                for i in 0..n_top {
                    rects.push(egui::Rect::from_min_size(egui::pos2(rect.left() + i as f32 * (half_w + gap), rect.top()), egui::vec2(half_w, row_h)));
                }
                // Middle row: 1 full-width pane
                if count > 2 {
                    rects.push(egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top() + row_h + gap), egui::vec2(rect.width(), row_h)));
                }
                // Bottom row: 2 panes
                let n_bot = count.saturating_sub(3).min(2);
                for i in 0..n_bot {
                    rects.push(egui::Rect::from_min_size(egui::pos2(rect.left() + i as f32 * (half_w + gap), rect.top() + (row_h + gap) * 2.0), egui::vec2(half_w, row_h)));
                }
                rects
            }
            Layout::Seven if count >= 2 => {
                // 1 big top + 6 bottom (3 cols × 2 rows)
                let top_h = (rect.height() - gap) * split_v.clamp(0.15, 0.85);
                let bot_h = rect.height() - gap - top_h;
                let mut rects = vec![egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), top_h))];
                let by = rect.top() + top_h + gap;
                let col_w = (rect.width() - gap * 2.0) / 3.0;
                let half_bh = (bot_h - gap) / 2.0;
                let n_bot = (count - 1).min(6);
                // 3 cols × 2 rows: col0r0, col1r0, col2r0, col0r1, col1r1, col2r1
                for i in 0..n_bot {
                    let col = i % 3;
                    let row = i / 3;
                    rects.push(egui::Rect::from_min_size(
                        egui::pos2(rect.left() + col as f32 * (col_w + gap), by + row as f32 * (half_bh + gap)),
                        egui::vec2(col_w, half_bh)));
                }
                rects
            }
            _ => {
                let (cols, rows) = match self {
                    Layout::One => (1, 1),
                    Layout::Two => (2, 1),
                    Layout::TwoH => (1, 2),
                    Layout::Three | Layout::ThreeL | Layout::ThreeR => (2, 2),
                    Layout::Four | Layout::FourL => (2, 2),
                    Layout::FiveC | Layout::FiveL | Layout::FiveW | Layout::FiveR => (3, 2),
                    Layout::Six | Layout::SixL => (3, 2),
                    Layout::SixH => (2, 3),
                    Layout::Seven => (3, 3),
                    Layout::EightH => (4, 2),
                    Layout::Nine => (3, 3),
                };
                // Use split ratios for column/row positions
                let total_w = rect.width() - gap * (cols as f32 - 1.0).max(0.0);
                let total_h = rect.height() - gap * (rows as f32 - 1.0).max(0.0);
                // Column widths: for 2 cols use split_h, for 3 cols use split_h for first divider
                let col_widths: Vec<f32> = if cols == 2 {
                    let w0 = total_w * split_h.clamp(0.15, 0.85);
                    vec![w0, total_w - w0]
                } else if cols == 3 {
                    let w0 = total_w * (split_h * 0.9).clamp(0.15, 0.5);
                    let rest = total_w - w0;
                    let w1 = rest * split_h2.clamp(0.2, 0.8);
                    vec![w0, w1, rest - w1]
                } else { vec![total_w] };
                // Row heights: for 2 rows use split_v, for 3 rows equal
                let row_heights: Vec<f32> = if rows == 2 {
                    let h0 = total_h * split_v.clamp(0.15, 0.85);
                    vec![h0, total_h - h0]
                } else if rows == 3 {
                    let h0 = total_h * (split_v * 0.9).clamp(0.15, 0.5);
                    let rest = total_h - h0;
                    let h1 = rest * split_v2.clamp(0.2, 0.8);
                    vec![h0, h1, rest - h1]
                } else { vec![total_h] };

                let mut rects = Vec::new();
                let mut y = rect.top();
                for r in 0..rows {
                    let mut x = rect.left();
                    let rh = row_heights[r];
                    for c in 0..cols {
                        if rects.len() >= count { break; }
                        let cw = col_widths[c];
                        rects.push(egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, rh)));
                        x += cw + gap;
                    }
                    y += rh + gap;
                }
                rects
            }
        }
    }
}

pub(crate) const ALL_LAYOUTS: &[Layout] = &[
    Layout::One, Layout::Two, Layout::TwoH,
    Layout::Three, Layout::ThreeL, Layout::ThreeR,
    Layout::Four, Layout::FourL,
    Layout::FiveC, Layout::FiveL, Layout::FiveW, Layout::FiveR,
    Layout::Six, Layout::SixH, Layout::SixL,
    Layout::Seven, Layout::EightH, Layout::Nine,
];

// ─── Indicators ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum IndicatorType { SMA, EMA, WMA, DEMA, TEMA, VWAP, BollingerBands, Ichimoku, ParabolicSAR, Supertrend, KeltnerChannels, RSI, MACD, Stochastic, ADX, CCI, WilliamsR, ATR, OBV }

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum IndicatorCategory { Overlay, Oscillator }

impl IndicatorType {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::SMA => "SMA", Self::EMA => "EMA", Self::WMA => "WMA",
            Self::DEMA => "DEMA", Self::TEMA => "TEMA", Self::VWAP => "VWAP",
            Self::BollingerBands => "BB", Self::Ichimoku => "ICHI",
            Self::ParabolicSAR => "PSAR", Self::Supertrend => "ST",
            Self::KeltnerChannels => "KC",
            Self::RSI => "RSI", Self::MACD => "MACD", Self::Stochastic => "STOCH",
            Self::ADX => "ADX", Self::CCI => "CCI", Self::WilliamsR => "%R",
            Self::ATR => "ATR", Self::OBV => "OBV",
        }
    }
    pub(crate) fn all() -> &'static [Self] { &[Self::SMA, Self::EMA, Self::WMA, Self::DEMA, Self::TEMA, Self::VWAP, Self::BollingerBands, Self::Ichimoku, Self::ParabolicSAR, Self::Supertrend, Self::KeltnerChannels, Self::RSI, Self::MACD, Self::Stochastic, Self::ADX, Self::CCI, Self::WilliamsR, Self::ATR, Self::OBV] }
    pub(crate) fn default_period(self) -> usize {
        match self {
            Self::SMA | Self::EMA | Self::WMA | Self::DEMA | Self::TEMA => 20,
            Self::RSI | Self::Stochastic | Self::ADX | Self::CCI | Self::WilliamsR | Self::ATR => 14,
            Self::MACD => 12, Self::VWAP => 1,
            Self::BollingerBands | Self::KeltnerChannels => 20,
            Self::Ichimoku => 9, Self::ParabolicSAR => 1, Self::Supertrend => 10,
            Self::OBV => 1, // cumulative — no period
        }
    }
    pub(crate) fn category(self) -> IndicatorCategory {
        match self { Self::RSI | Self::MACD | Self::Stochastic | Self::ADX | Self::CCI | Self::WilliamsR | Self::ATR | Self::OBV => IndicatorCategory::Oscillator, _ => IndicatorCategory::Overlay }
    }

    fn compute(self, closes: &[f32], period: usize) -> Vec<f32> {
        match self {
            Self::SMA => compute_sma(closes, period),
            Self::EMA => compute_ema(closes, period),
            Self::WMA => super::compute::compute_wma(closes, period),
            Self::DEMA => super::compute::compute_dema(closes, period),
            Self::TEMA => super::compute::compute_tema(closes, period),
            Self::VWAP | Self::BollingerBands | Self::Ichimoku | Self::ParabolicSAR
            | Self::Supertrend | Self::KeltnerChannels => vec![f32::NAN; closes.len()], // computed separately
            Self::RSI => compute_rsi(closes, period),
            Self::MACD => compute_ema(closes, period),
            Self::Stochastic => vec![f32::NAN; closes.len()],
            Self::ADX | Self::CCI | Self::WilliamsR | Self::ATR | Self::OBV => vec![f32::NAN; closes.len()], // need OHLCV — computed in recompute_indicators
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Indicator {
    pub(crate) id: u32,
    pub(crate) kind: IndicatorType,
    pub(crate) period: usize,
    pub(crate) source_tf: String,
    pub(crate) color: String,
    pub(crate) thickness: f32,
    pub(crate) line_style: LineStyle,
    pub(crate) visible: bool,
    pub(crate) values: Vec<f32>, // primary line (same length as chart bars)
    pub(crate) values2: Vec<f32>, // secondary line: MACD signal, Stochastic %D, BB upper, KC upper, Ichi kijun
    pub(crate) values3: Vec<f32>, // BB lower, KC lower, Ichi senkou_a
    pub(crate) values4: Vec<f32>, // Ichi senkou_b
    pub(crate) values5: Vec<f32>, // Ichi chikou
    pub(crate) supertrend_dir: Vec<bool>, // Supertrend: true=bullish
    pub(crate) histogram: Vec<f32>, // MACD histogram
    pub(crate) divergences: Vec<i8>, // 1=bullish divergence, -1=bearish, 0=none
    // Cross-timeframe state
    pub(crate) source_bars: Vec<Bar>,
    pub(crate) source_timestamps: Vec<i64>,
    pub(crate) source_loaded: bool,
    // Extended parameters (0.0 = use default)
    pub(crate) param2: f32, // BB stddev, KC mult, ST mult, MACD slow, Stoch D, Ichi kijun, SAR step
    pub(crate) param3: f32, // MACD signal, Ichi senkou_b, SAR max
    pub(crate) param4: f32, // SAR start, Ichi displacement
    pub(crate) source: u8, // 0=Close, 1=Open, 2=High, 3=Low, 4=HL2, 5=OHLC4
    pub(crate) offset: i16, // shift line forward/backward N bars
    pub(crate) ob_level: f32, // overbought level (RSI 70, Stoch 80, CCI 100, WR -20)
    pub(crate) os_level: f32, // oversold level (RSI 30, Stoch 20, CCI -100, WR -80)
    // BB/KC band styling (empty = inherit from main color)
    pub(crate) upper_color: String,
    pub(crate) lower_color: String,
    pub(crate) fill_color_hex: String,
    pub(crate) upper_thickness: f32,
    pub(crate) lower_thickness: f32,
}

pub(crate) const INDICATOR_TIMEFRAMES: &[&str] = &["", "1m", "5m", "15m", "30m", "1h", "4h", "1d", "1wk"];

#[allow(dead_code)]
impl Indicator {
    pub(crate) fn new(id: u32, kind: IndicatorType, period: usize, color: &str) -> Self {
        Self { id, kind, period, source_tf: String::new(), color: color.into(), thickness: 1.2,
               line_style: LineStyle::Solid, visible: true,
               values: vec![], values2: vec![], values3: vec![], values4: vec![], values5: vec![],
               supertrend_dir: vec![],
               histogram: vec![], divergences: vec![],
               source_bars: vec![], source_timestamps: vec![], source_loaded: false,
               param2: 0.0, param3: 0.0, param4: 0.0, source: 0, offset: 0, ob_level: 0.0, os_level: 0.0,
               upper_color: String::new(), lower_color: String::new(), fill_color_hex: String::new(),
               upper_thickness: 0.0, lower_thickness: 0.0 }
    }
    pub(crate) fn display_name(&self) -> String {
        let tf = if self.source_tf.is_empty() { "Chart" } else { &self.source_tf };
        match self.kind {
            IndicatorType::MACD => {
                let fast = self.period;
                let slow = if self.param2 > 0.0 { self.param2 as usize } else { 26 };
                let sig = if self.param3 > 0.0 { self.param3 as usize } else { 9 };
                format!("MACD {}/{}/{} ({})", fast, slow, sig, tf)
            }
            IndicatorType::BollingerBands => {
                let std = if self.param2 > 0.0 { self.param2 } else { 2.0 };
                format!("BB {} {:.1}σ ({})", self.period, std, tf)
            }
            IndicatorType::Ichimoku => {
                let kijun = if self.param2 > 0.0 { self.param2 as usize } else { 26 };
                format!("Ichimoku {}/{} ({})", self.period, kijun, tf)
            }
            _ => format!("{} {} ({})", self.kind.label(), self.period, tf)
        }
    }
    fn source_label(&self) -> &str {
        if self.source_tf.is_empty() { "Chart" } else { &self.source_tf }
    }
}

pub(crate) static INDICATOR_COLORS: &[&str] = &["#00bef0", "#f0961a", "#f0d732", "#b266e6", "#1abc9c", "#e74c3c", "#3498db", "#e67e22"];

/// Return a theme-palette-derived default colour for a new indicator at position `slot`.
///
/// Cycles through 8 distinct, theme-coherent stops derived entirely from the active
/// `Theme`'s semantic palette — no raw RGB beyond the arithmetic below:
///
/// | slot % 8 | source |
/// |---|---|
/// | 0 | `t.accent` |
/// | 1 | `t.bull` |
/// | 2 | `t.bear` |
/// | 3 | `t.warn` |
/// | 4 | `t.dim` brightened (×1.4, clamped) |
/// | 5 | `t.accent` darkened (×0.65) |
/// | 6 | `t.bull` darkened (×0.65) |
/// | 7 | `t.bear` darkened (×0.65) |
///
/// The caller may still override the colour afterwards — this only sets the initial value.
pub(crate) fn indicator_default_color(slot: usize, t: &Theme) -> String {
    #[inline]
    fn brighten(c: egui::Color32, factor: f32) -> egui::Color32 {
        egui::Color32::from_rgb(
            ((c.r() as f32 * factor).round() as u32).min(255) as u8,
            ((c.g() as f32 * factor).round() as u32).min(255) as u8,
            ((c.b() as f32 * factor).round() as u32).min(255) as u8,
        )
    }
    let c = match slot % 8 {
        0 => t.accent,
        1 => t.bull,
        2 => t.bear,
        3 => t.warn,
        4 => brighten(t.dim,    1.40),
        5 => brighten(t.accent, 0.65),
        6 => brighten(t.bull,   0.65),
        _ => brighten(t.bear,   0.65),
    };
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

// compute_rsi, compute_macd, compute_stochastic, compute_vwap, detect_divergences — now in compute.rs

// ─── Signal drawings (auto-generated trendlines from analysis server) ────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct SignalDrawing {
    pub(crate) id: String,
    pub(crate) symbol: String,
    pub(crate) drawing_type: String, // "trendline", "hline", "hzone"
    pub(crate) points: Vec<(i64, f32)>, // (unix_timestamp, price)
    pub(crate) color: String,
    pub(crate) opacity: f32,
    pub(crate) thickness: f32,
    pub(crate) line_style: LineStyle,
    pub(crate) strength: f32, // 0.0-1.0, how confident the analysis is
    pub(crate) timeframe: String,
    pub(crate) detection_method: String, // wick/ransac/kalman/hough/kde/… — for by-method filtering
    pub(crate) source: String, // producer: "trendlines" / "chart_patterns" / "signal" — scopes replacement
    pub(crate) extend_left: bool,  // project line to the left chart edge
    pub(crate) extend_right: bool, // project line to the right chart edge
}

impl SignalDrawing {
    /// Convert timestamp to fractional bar index using the chart's timestamp array.
    pub(crate) fn time_to_bar(ts: i64, timestamps: &[i64]) -> f32 {
        if timestamps.is_empty() { return 0.0; }
        // Binary search for the closest bar
        let pos = timestamps.partition_point(|&t| t < ts);
        if pos == 0 { return 0.0; }
        // Extrapolate into future if timestamp is beyond last bar
        if pos >= timestamps.len() {
            let candle_sec = if timestamps.len() > 1 { timestamps[1] - timestamps[0] } else { 300 };
            let last_ts = *timestamps.last().unwrap_or(&0);
            let beyond = ts - last_ts;
            return (timestamps.len() - 1) as f32 + beyond as f32 / candle_sec as f32;
        }
        // Interpolate between bars
        let t0 = timestamps[pos - 1];
        let t1 = timestamps[pos];
        if t1 == t0 { return pos as f32; }
        let frac = (ts - t0) as f32 / (t1 - t0) as f32;
        (pos - 1) as f32 + frac
    }
}

/// Event marker for chart overlay (earnings, dividends, splits, economic events)
pub(crate) struct EventMarker {
    pub(crate) time: i64,
    pub(crate) event_type: u8,   // 0=earnings, 1=dividend, 2=split, 3=economic
    pub(crate) label: String,
    pub(crate) details: String,
    pub(crate) impact: i8,       // -1=bearish, 0=neutral, 1=bullish
}

/// Fundamental data for a symbol.
#[derive(Clone, Default)]
pub(crate) struct FundamentalData {
    pub pe_ratio: f32,
    pub forward_pe: f32,
    pub eps_ttm: f32,
    pub market_cap: f64,        // in billions
    pub dividend_yield: f32,
    pub revenue_growth: f32,    // YoY %
    pub profit_margin: f32,     // %
    pub debt_to_equity: f32,
    pub short_interest: f32,    // %
    pub institutional_pct: f32, // %
    pub insider_pct: f32,       // %
    pub beta: f32,
    pub avg_volume: f64,
    pub shares_outstanding: f64,
    // Analyst consensus
    pub analyst_target_mean: f32,
    pub analyst_target_high: f32,
    pub analyst_target_low: f32,
    pub analyst_buy: u8,
    pub analyst_hold: u8,
    pub analyst_sell: u8,
    // Earnings history (last 4 quarters)
    pub earnings: Vec<EarningsQuarter>,
}

#[derive(Clone)]
pub(crate) struct EarningsQuarter {
    pub quarter: String,       // "Q1 2026"
    pub eps_actual: f32,
    pub eps_estimate: f32,
    pub revenue_actual: f64,   // in millions
    pub revenue_estimate: f64,
    pub date: i64,
}

/// Economic calendar event.
#[derive(Clone)]
pub(crate) struct EconEvent {
    pub time: i64,
    pub name: String,
    pub importance: u8,        // 0=low, 1=medium, 2=high, 3=critical
    pub actual: Option<f64>,
    pub forecast: f64,
    pub previous: f64,
    pub country: String,
}

/// SEC filing / insider transaction.
#[derive(Clone)]
pub(crate) struct InsiderTrade {
    pub name: String,
    pub title: String,
    pub transaction: String, // "Buy", "Sell", "Grant"
    pub shares: i64,
    pub price: f32,
    pub date: i64,
    pub value: f64,
}

/// A corporate action (dividend ex-date or stock split) for the chart's symbol,
/// from ApexData `/api/stocks/dividends` + `/splits`. Rendered as a marker on
/// the bottom time axis at the matching bar.
#[derive(Clone)]
pub(crate) struct CorpAction {
    pub date: i64,        // epoch seconds (ex-date / execution date, midnight UTC)
    pub is_split: bool,   // false = dividend, true = split
    pub amount: f32,      // dividend cash amount (0 for splits)
    pub label: String,    // "$0.25" (dividend) or "10:1" (split)
}

/// A completed trade for the journal.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct JournalEntry {
    pub id: String,
    pub symbol: String,
    pub side: String,          // "Long" or "Short"
    pub qty: i32,
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub entry_time: i64,
    pub exit_time: i64,
    pub duration_mins: i64,
    pub setup_type: String,    // "breakout", "scalp", "swing", etc.
    pub notes: String,
    pub tags: Vec<String>,
    pub timeframe: String,
    pub r_multiple: f64,       // P&L in terms of risk units
}

/// Convert a fractional bar index to a timestamp using interpolation.
/// Convert DTE (trading days) to calendar date, skipping weekends
pub(crate) fn trading_date(dte: i32) -> (u32, u32, u32) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let mut days_added = 0i32;
    let mut offset_days = 0i64;
    while days_added < dte {
        offset_days += 1;
        let ts = now as i64 + offset_days * 86400;
        let dow = ((ts / 86400 + 4) % 7) as u32;
        if dow != 0 && dow != 6 { days_added += 1; }
    }
    let total_secs = now as i64 + offset_days * 86400;
    let days_since_epoch = total_secs / 86400;
    let mut y = 1970i32; let mut remaining = days_since_epoch;
    loop {
        let diy = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        if remaining < diy { break; }
        remaining -= diy; y += 1;
    }
    let md = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
        [31,29,31,30,31,30,31,31,30,31,30,31]
    } else { [31,28,31,30,31,30,31,31,30,31,30,31] };
    let mut m = 0u32;
    for d in &md { if remaining < *d as i64 { break; } remaining -= *d as i64; m += 1; }
    (y as u32, m + 1, remaining as u32 + 1)
}

pub(crate) fn trading_month_name(m: u32) -> &'static str {
    match m { 1=>"Jan",2=>"Feb",3=>"Mar",4=>"Apr",5=>"May",6=>"Jun",7=>"Jul",8=>"Aug",9=>"Sep",10=>"Oct",11=>"Nov",12=>"Dec",_=>"" }
}

pub(crate) fn dte_label(dte: i32) -> String {
    if dte == 0 { return "0DTE Today".into(); }
    let (_, m, d) = trading_date(dte);
    format!("{}DTE {} {}", dte, trading_month_name(m), d)
}

pub(crate) fn bar_to_time(bar: f32, timestamps: &[i64]) -> i64 {
    let idx = bar as usize;
    if timestamps.is_empty() { return 0; }
    // Extrapolate into the future if bar index is beyond available data
    if idx >= timestamps.len() {
        let candle_sec = if timestamps.len() > 1 { timestamps[1] - timestamps[0] } else { 300 };
        let last_ts = *timestamps.last().unwrap_or(&0);
        let bars_beyond = bar - (timestamps.len() - 1) as f32;
        return last_ts + (bars_beyond * candle_sec as f32) as i64;
    }
    let frac = bar - idx as f32;
    if frac < 0.01 || idx + 1 >= timestamps.len() { return timestamps[idx]; }
    // Interpolate
    let t0 = timestamps[idx];
    let t1 = timestamps[idx + 1];
    t0 + ((t1 - t0) as f32 * frac) as i64
}

/// Global, persisted config for the auto-drawing control panel. Drives what the
/// engine computes (sent as query params) and which layers render.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct AutoDrawConfig {
    pub enabled: bool,    // master on/off
    pub trendlines: bool,
    pub levels: bool,
    pub channels: bool,
    pub patterns: bool,
    pub candles: bool,
    pub pivot_mode: String, // "hybrid" | "atr" | "percent"
    pub atr_k: f64,
    pub pct: f64,
    pub min_touches: u32,
    pub touch_pct: f64,
    pub max_lines: usize,
    /// Legacy methods to also run for comparison (wick, body, hough, ransac, …).
    pub methods: Vec<String>,
    /// Line extension: "none" | "right" | "both" | "left".
    pub extend: String,
    /// Legacy tuning knobs.
    pub sensitivity: f64,
    pub lookback: usize,
    pub swing_window: usize,
    /// How many bars back the engine loads/scans (the "window of operation").
    /// Larger = older, more valuable lines reach further back.
    pub window: usize,
    /// Drop lines whose endpoints don't sit on an actual candle (kills
    /// "middle of nowhere" floating starts from fitted methods).
    pub anchored_only: bool,
    /// Drawing IDs the user has explicitly rejected — filtered from render and
    /// POSTed to /significance/feedback for learning.
    #[serde(default)]
    pub rejected_drawings: std::collections::HashSet<String>,
    /// Use the apex-data unified drawing FEED (computed for the whole universe,
    /// updated live via /ws/drawings) instead of the tuned per-chart ApexSignals
    /// fetch. Off by default — the tuned path honours the panel knobs; the feed
    /// uses fixed server-side config but updates live and exists for symbols with
    /// no chart open.
    #[serde(default)]
    pub live_feed: bool,
}
impl Default for AutoDrawConfig {
    fn default() -> Self {
        Self {
            enabled: true, trendlines: true, levels: true, channels: true,
            patterns: true, candles: false, pivot_mode: "hybrid".into(),
            atr_k: 2.0, pct: 0.015, min_touches: 3, touch_pct: 0.004, max_lines: 12,
            methods: vec![], extend: "none".into(),
            sensitivity: 0.003, lookback: 200, swing_window: 5,
            window: 500, anchored_only: true,
            rejected_drawings: Default::default(),
            live_feed: false,
        }
    }
}
impl AutoDrawConfig {
    fn types_csv(&self) -> String {
        let mut v = vec![];
        if self.trendlines { v.push("trendlines"); }
        if self.levels { v.push("levels"); }
        if self.channels { v.push("channels"); }
        if self.patterns { v.push("patterns"); }
        if self.candles { v.push("candles"); }
        v.join(",")
    }
    fn query(&self) -> String {
        format!(
            "&types={}&pivot_mode={}&atr_k={}&pct={}&min_touches={}&touch_pct={}&max_lines={}&methods={}&extend={}&sensitivity={}&lookback={}&swing_window={}&window={}&anchored_only={}",
            self.types_csv(), self.pivot_mode, self.atr_k, self.pct,
            self.min_touches, self.touch_pct, self.max_lines,
            self.methods.join(","), self.extend, self.sensitivity, self.lookback, self.swing_window,
            self.window, self.anchored_only,
        )
    }
}

impl crate::state::persistence::Persistable for AutoDrawConfig {
    const KEY: &'static str = "auto_draw_config";
    const VERSION: u32 = 1;
}

fn auto_draw_path() -> std::path::PathBuf {
    let mut p = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("apex-terminal");
    let _ = std::fs::create_dir_all(&p);
    p.push("auto-draw-config.json");
    p
}

static AUTO_DRAW: std::sync::OnceLock<std::sync::Mutex<AutoDrawConfig>> = std::sync::OnceLock::new();

/// Current auto-draw config (loads from disk on first access).
pub(crate) fn auto_draw_config() -> AutoDrawConfig {
    AUTO_DRAW
        .get_or_init(|| {
            let cfg = crate::state::persistence::load::<AutoDrawConfig>(&auto_draw_path())
                .unwrap_or_default();
            std::sync::Mutex::new(cfg)
        })
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

/// Update + persist the auto-draw config (global, applies to all charts).
pub(crate) fn set_auto_draw_config(cfg: AutoDrawConfig) {
    let cell = AUTO_DRAW.get_or_init(|| std::sync::Mutex::new(AutoDrawConfig::default()));
    if let Ok(mut g) = cell.lock() { *g = cfg.clone(); }
    let _ = crate::state::persistence::save(&auto_draw_path(), &cfg);
}

// ── Plays persistence / sharing / level helpers ────────────────────────────
// EXTRACTED to `playbook_store` (WS-E E2) to shrink this god-file. Re-exported
// so every `gpu::plays_path()` / `gpu::author_handle()` / ... caller is unchanged.
pub(crate) use super::playbook_store::*;

/// Classify an apex-data unified-feed drawing into the terminal's source bucket
/// ("trendlines" | "chart_patterns" | "candles") so the existing replace-by-source
/// render path applies. Patterns carry `detection_method = "pattern:*"`; candle
/// markers are `type = "marker_*"`; everything else (trendlines, lines, zones,
/// channels, fib) is a "trendline" overlay.
fn classify_drawing_source(d: &serde_json::Value) -> &'static str {
    let method = d.get("detection_method").and_then(|v| v.as_str()).unwrap_or("");
    let dtype = d.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if method.starts_with("pattern") {
        "chart_patterns"
    } else if dtype.starts_with("marker") {
        "candles"
    } else {
        "trendlines"
    }
}

/// Fetch auto-chart drawings from ApexSignals using the current panel config.
/// Sends an AutoTrendlines command for *every* known source (empty if the engine
/// returned none) so unchecked layers and master-off actually clear. The engine
/// computes per the config query params. Base via `APEX_SIGNALS_HTTP` (:8100).
pub(crate) fn fetch_apexsignals_drawings(symbol: String, timeframe: String) {
    let txs: Vec<std::sync::mpsc::Sender<super::ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { return; }
    let cfg = auto_draw_config();
    std::thread::spawn(move || {
        const SOURCES: [&str; 3] = ["trendlines", "chart_patterns", "candles"];
        let mut by_source: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let client = reqwest::blocking::Client::builder().user_agent("apex-native").build().unwrap_or_else(|_| reqwest::blocking::Client::new());
        if cfg.enabled && cfg.live_feed {
            // Unified apex-data drawing feed: one flat list (all sources merged),
            // regrouped client-side into the terminal's source buckets so the
            // replace-by-source render path is unchanged.
            let base = crate::data::feeds::apex_data::config::apex_url();
            let class = if symbol.starts_with("F:") { "futures" } else { "stocks" };
            let sym = symbol.strip_prefix("F:").unwrap_or(&symbol);
            let url = format!("{base}/api/drawings/{class}/{sym}/{timeframe}");
            let mut buckets: std::collections::HashMap<&str, Vec<serde_json::Value>> = std::collections::HashMap::new();
            if let Ok(resp) = client.get(&url).timeout(std::time::Duration::from_secs(4)).send() {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    if let Some(arr) = json.get("drawings").and_then(|d| d.as_array()) {
                        for d in arr {
                            buckets.entry(classify_drawing_source(d)).or_default().push(d.clone());
                        }
                    }
                }
            }
            for (src, items) in buckets {
                by_source.insert(src.to_string(), serde_json::Value::Array(items).to_string());
            }
        } else if cfg.enabled {
            let base = std::env::var("APEX_SIGNALS_HTTP").unwrap_or_else(|_| "http://localhost:8100".to_string());
            let url = format!("{base}/signals/drawings/{symbol}?timeframe={timeframe}{}", cfg.query());
            if let Ok(resp) = client.get(&url).timeout(std::time::Duration::from_secs(4)).send() {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    if let Some(frames) = json.get("frames").and_then(|f| f.as_array()) {
                        for frame in frames {
                            if let Some(src) = frame.get("source").and_then(|s| s.as_str()) {
                                let dj = frame.get("drawings").map(|d| d.to_string()).unwrap_or_else(|| "[]".to_string());
                                by_source.insert(src.to_string(), dj);
                            }
                        }
                    }
                }
            }
        }
        // Replace every known source (empty clears) — handles toggles + master-off.
        for src in SOURCES {
            let drawings_json = by_source.get(src).cloned().unwrap_or_else(|| "[]".to_string());
            let cmd = super::ChartCommand::AutoTrendlines { symbol: symbol.clone(), drawings_json, source: src.to_string() };
            for tx in &txs { let _ = tx.send(cmd.clone()); }
        }
        crate::wake_native_ui();
    });
}

/// POST user feedback on a drawn line to /significance/feedback.
/// Fire-and-forget — spawns a thread, ignores errors.
pub(crate) fn post_drawing_feedback(
    drawing_id: String,
    action: String, // "accept" | "reject" | "adjust"
    symbol: String,
    timeframe: String,
) {
    std::thread::spawn(move || {
        let base = std::env::var("APEX_SIGNALS_HTTP")
            .unwrap_or_else(|_| "http://localhost:8100".to_string());
        let url = format!("{base}/significance/feedback");
        let body = serde_json::json!({
            "drawing_id": drawing_id,
            "action": action,
            "symbol": symbol,
            "timeframe": timeframe,
        });
        let client = reqwest::blocking::Client::builder()
            .user_agent("apex-native")
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        let _ = client.post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(3))
            .send();
    });
}

/// Fetch signal annotations from OCOCO API for a symbol.
pub(crate) fn fetch_signal_drawings(symbol: String) {
    let txs: Vec<std::sync::mpsc::Sender<super::ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { return; }
    std::thread::spawn(move || {
        let url = format!("{}/api/annotations?symbol={}&source=signal", crate::data::endpoints::ococo_http(), symbol);
        let client = reqwest::blocking::Client::builder().user_agent("apex-native").build().unwrap_or_else(|_| reqwest::blocking::Client::new());
        if let Ok(resp) = client.get(&url).timeout(std::time::Duration::from_secs(3)).send() {
            if let Ok(json) = resp.json::<Vec<serde_json::Value>>() {
                let drawings: Vec<SignalDrawing> = json.iter().filter_map(|a| {
                    let id = a.get("id")?.as_str()?.to_string();
                    let sym = a.get("symbol")?.as_str()?.to_string();
                    let dtype = a.get("type")?.as_str().unwrap_or("trendline").to_string();
                    let points: Vec<(i64, f32)> = a.get("points")?.as_array()?.iter().filter_map(|p| {
                        Some((p.get("time")?.as_i64()?, p.get("price")?.as_f64()? as f32))
                    }).collect();
                    let style = a.get("style");
                    let color = style.and_then(|s| s.get("color")).and_then(|c| c.as_str()).unwrap_or("#4a9eff").to_string();
                    let opacity = style.and_then(|s| s.get("opacity")).and_then(|o| o.as_f64()).unwrap_or(0.7) as f32;
                    let thickness = style.and_then(|s| s.get("thickness")).and_then(|t| t.as_f64()).unwrap_or(1.0) as f32;
                    let ls_str = style.and_then(|s| s.get("lineStyle")).and_then(|l| l.as_str()).unwrap_or("dashed");
                    let line_style = match ls_str { "solid" => LineStyle::Solid, "dotted" => LineStyle::Dotted, _ => LineStyle::Dashed };
                    let strength = a.get("strength").and_then(|s| s.as_f64()).unwrap_or(0.5) as f32;
                    let timeframe = a.get("timeframe").and_then(|t| t.as_str()).unwrap_or("5m").to_string();
                    let detection_method = a.get("detection_method").and_then(|m| m.as_str()).unwrap_or("").to_string();
                    Some(SignalDrawing { id, symbol: sym, drawing_type: dtype, points, color, opacity, thickness, line_style, strength, timeframe, detection_method, source: "signal".to_string(), extend_left: a.get("extendLeft").and_then(|v| v.as_bool()).unwrap_or(false), extend_right: a.get("extendRight").and_then(|v| v.as_bool()).unwrap_or(false) })
                }).collect();

                if !drawings.is_empty() {
                    eprintln!("[signal] Fetched {} signal drawings for {}", drawings.len(), symbol);
                }
                // Send via command channel
                let cmd = super::ChartCommand::SignalDrawings { symbol, drawings_json: serde_json::to_string(&json).unwrap_or_default() };
                for tx in &txs { let _ = tx.send(cmd.clone()); } crate::wake_native_ui();
            }
        }
    });
}

// ─── Orders, Account, Alerts, Triggers ─── (moved to trading.rs)

/// ApexIB endpoint — runtime-configurable via APEXIB_HTTP env var.
#[inline]
pub(crate) fn apexib_url() -> String {
    std::env::var("APEXIB_HTTP").unwrap_or_else(|_| APEXIB_URL.to_string())
}

// ─── Volume Profile ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum VolumeProfileMode { Off, Classic, Heatmap, Strip, Clean }

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum CandleMode { Standard, Violin, Gradient, ViolinGradient, HeikinAshi, Line, Area, Renko, RangeBar, TickBar }

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum StrikeMode {
    Count,      // N strikes above/below center point
    Pct(u8),    // strikes within X% of underlying (index into PCT_OPTIONS)
    StdDev,     // strikes within N std deviations
}
pub(crate) const PCT_OPTIONS: [f32; 5] = [0.6, 1.0, 1.25, 1.5, 2.0];
// NearMidFar: 0=Near (ATM), 1=Mid (1σ away), 2=Far (2σ away) — sets center point, orthogonal to mode

pub(crate) struct VolumeLevel {
    pub(crate) price: f32,
    pub(crate) total_vol: f32,
    pub(crate) buy_vol: f32,
    pub(crate) sell_vol: f32,
    /// Off-exchange (FINRA TRF / dark-pool) volume at this price, from VAP.
    /// `0` for the bar-derived profile (no off-exchange breakdown).
    pub(crate) off_exchange: f32,
}

pub(crate) struct VolumeProfileData {
    pub(crate) levels: Vec<VolumeLevel>,
    pub(crate) poc_price: f32,
    pub(crate) vah: f32,
    pub(crate) val: f32,
    pub(crate) max_vol: f32,
    pub(crate) price_step: f32,
}

/// Shared order entry body — renders qty controls, price fields, and BUY/SELL buttons.
/// Called from both the main order panel and floating strike-order panes.
/// `id_salt`: unique value to differentiate egui widget IDs between instances.
pub(crate) fn render_order_entry_body(
    ui: &mut egui::Ui,
    chart: &mut Chart,
    t: &Theme,
    _id_salt: u64,
    panel_w: f32,
) {
    // ── Meridien path: fully-redesigned editorial order ticket (#13) ─────────
    if super::ui::style::current().hairline_borders {
        let last_price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
        let spread = (last_price * 0.0001).max(0.01);
        let oe_qty_snapshot = chart.order_panel.qty;
        let mut oe_state = super::ui::inputs::form::OrderTicketState {
            symbol:         &chart.symbol,
            is_buy:         &mut chart.order_panel.is_buy,
            order_type_idx: &mut chart.order_panel.type_idx,
            order_tif_idx:  &mut chart.order_panel.tif_idx,
            order_qty:      &mut chart.order_panel.qty,
            order_market:   &mut chart.order_panel.market,
            limit_price:    &mut chart.order_panel.limit_price,
            stop_price:     &mut chart.order_panel.stop_price,
            tp_price:       &mut chart.order_panel.tp_price,
            sl_price:       &mut chart.order_panel.sl_price,
            bracket:        &mut chart.order_panel.bracket,
            bid:            (last_price - spread).max(0.0),
            last:           last_price,
            ask:            last_price + spread,
            notional:       last_price * oe_qty_snapshot as f32,
            buying_power:   crate::chart_renderer::trading::read_account_data()
                                .map(|(s, _, _)| s.buying_power as f32)
                                .unwrap_or(0.0),
            slippage_bps:   0.0,
        };
        let outcome = super::ui::inputs::form::MeridienOrderTicket::new()
            .theme(t)
            .show(ui, &mut oe_state);
        if outcome.review_clicked {
            // Translate REVIEW click into a submit — same path as the existing BUY/SELL buttons.
            // Side is determined by order_is_buy that the widget just toggled.
            let side = if chart.order_panel.is_buy { "BUY" } else { "SELL" };
            let sym  = chart.symbol.clone();
            let qty  = chart.order_panel.qty;
            let ot   = chart.order_panel.type_idx;
            let tif  = chart.order_panel.tif_idx;
            let price = if chart.order_panel.market { last_price } else {
                chart.order_panel.limit_price.parse::<f32>().unwrap_or(last_price)
            };
            let bracket = chart.order_panel.bracket;
            let tp = chart.order_panel.tp_price.parse::<f32>().ok();
            let sl = chart.order_panel.sl_price.parse::<f32>().ok();
            std::thread::spawn(move || {
                submit_ib_order(&sym, side, qty, ot, tif, price, bracket, tp, sl);
            });
        }
        return;
    }
    // ── Aperture / Octave path: delegated to ApertureOrderTicket widget ──────
    use super::ui::inputs::form::{ApertureOrderTicket, ApertureOrderState, ApertureAction, ApertureVariant};

    let last_price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
    let spread = (last_price * 0.0001).max(0.01);

    let mut oe_state = ApertureOrderState {
        last_price,
        spread,
        order_advanced:        chart.order_panel.advanced,
        order_market:          &mut chart.order_panel.market,
        order_type_idx:        &mut chart.order_panel.type_idx,
        order_tif_idx:         &mut chart.order_panel.tif_idx,
        order_qty:             &mut chart.order_panel.qty,
        order_notional_mode:   &mut chart.order_panel.notional_mode,
        order_notional_amount: &mut chart.order_panel.notional_amount,
        order_limit_price:     &mut chart.order_panel.limit_price,
        order_stop_price:      &mut chart.order_panel.stop_price,
        order_trail_amt:       &mut chart.order_panel.trail_amt,
        order_bracket:         &mut chart.order_panel.bracket,
        order_tp_price:        &mut chart.order_panel.tp_price,
        order_sl_price:        &mut chart.order_panel.sl_price,
        order_outside_rth:     &mut chart.order_panel.outside_rth,
        is_option:             chart.is_option,
        option_type:           &chart.option_type,
        armed:                 chart.armed,
    };

    ui.add_space(crate::chart_renderer::ui::style::gap_xs());
    let outcome = ApertureOrderTicket::new()
        .variant(ApertureVariant::Aperture)
        .theme(t)
        .panel_width(panel_w)
        .show(ui, &mut oe_state);

    // Handle the action returned by the widget — submission lives here because
    // submit_ib_order / submit_order are in this module.
    let adv = chart.order_panel.advanced;
    match outcome.action {
        ApertureAction::TriggerBuy  => { chart.pending_und_order = Some(OrderSide::TriggerBuy); }
        ApertureAction::TriggerSell => { chart.pending_und_order = Some(OrderSide::TriggerSell); }
        ApertureAction::Buy { price } => {
            if chart.armed && adv {
                let sym = chart.symbol.clone();
                let qty = chart.order_panel.qty;
                let ot_idx = chart.order_panel.type_idx;
                let tif_idx = chart.order_panel.tif_idx;
                let bracket = chart.order_panel.bracket;
                let tp = chart.order_panel.tp_price.parse::<f32>().ok();
                let sl = chart.order_panel.sl_price.parse::<f32>().ok();
                std::thread::spawn(move || {
                    submit_ib_order(&sym, "BUY", qty, ot_idx, tif_idx, price, bracket, tp, sl);
                });
            } else {
                use super::trading::order_manager::*;
                let intent = OrderIntent {
                    symbol: chart.symbol.clone(), side: OrderSide::Buy,
                    order_type: ManagedOrderType::Limit, price, qty: chart.order_panel.qty,
                    source: OrderSource::OrderPanel, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: chart.order_panel.tif_idx as u8, outside_rth: chart.order_panel.outside_rth,
                    strategy_id: None, override_warnings: false,
                };
                let result = submit_order(intent.clone());
                match result {
                    OrderResult::Accepted(id) => {
                        chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Buy, price, qty: chart.order_panel.qty, status: OrderStatus::Placed, state: OrderState::Working, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
                    }
                    OrderResult::NeedsConfirmation(id) => {
                        chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Buy, price, qty: chart.order_panel.qty, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
                        chart.pending_confirms.push((id as u32, std::time::Instant::now()));
                    }
                    OrderResult::NeedsApproval { reason, .. } => {
                        // Stash original intent + reason for the per-frame
                        // approval modal; on confirm, the modal flips
                        // override_warnings=true and resubmits.
                        enqueue_approval(reason, intent);
                    }
                    OrderResult::Rejected(reason) => {
                        eprintln!("[aperture] rejected (buy @ {:.2}): {}", price, reason);
                    }
                    OrderResult::Duplicate => { /* silently blocked */ }
                }
            }
        }
        ApertureAction::Sell { price } => {
            if chart.armed && adv {
                let sym = chart.symbol.clone();
                let qty = chart.order_panel.qty;
                let ot_idx = chart.order_panel.type_idx;
                let tif_idx = chart.order_panel.tif_idx;
                let bracket = chart.order_panel.bracket;
                let tp = chart.order_panel.tp_price.parse::<f32>().ok();
                let sl = chart.order_panel.sl_price.parse::<f32>().ok();
                std::thread::spawn(move || {
                    submit_ib_order(&sym, "SELL", qty, ot_idx, tif_idx, price, bracket, tp, sl);
                });
            } else {
                use super::trading::order_manager::*;
                let intent = OrderIntent {
                    symbol: chart.symbol.clone(), side: OrderSide::Sell,
                    order_type: ManagedOrderType::Limit, price, qty: chart.order_panel.qty,
                    source: OrderSource::OrderPanel, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: chart.order_panel.tif_idx as u8, outside_rth: chart.order_panel.outside_rth,
                    strategy_id: None, override_warnings: false,
                };
                let result = submit_order(intent.clone());
                match result {
                    OrderResult::Accepted(id) => {
                        chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Sell, price, qty: chart.order_panel.qty, status: OrderStatus::Placed, state: OrderState::Working, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
                    }
                    OrderResult::NeedsConfirmation(id) => {
                        chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Sell, price, qty: chart.order_panel.qty, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
                        chart.pending_confirms.push((id as u32, std::time::Instant::now()));
                    }
                    OrderResult::NeedsApproval { reason, .. } => {
                        // Stash original intent + reason for the per-frame
                        // approval modal; on confirm, the modal flips
                        // override_warnings=true and resubmits.
                        enqueue_approval(reason, intent);
                    }
                    OrderResult::Rejected(reason) => {
                        eprintln!("[aperture] rejected (sell @ {:.2}): {}", price, reason);
                    }
                    OrderResult::Duplicate => { /* silently blocked */ }
                }
            }
        }
        ApertureAction::None => {}
    }
}

// ─── Overlay colors for multi-symbol overlays ───────────────────────────────
pub(crate) const OVERLAY_COLORS: &[&str] = &["#ff8c3c", "#00e5ff", "#ff00ff", "#76ff03", "#ff4081"];

#[derive(Clone)]
pub(crate) struct SymbolOverlay {
    pub(crate) symbol: String,
    pub(crate) color: String, // hex color
    pub(crate) bars: Vec<Bar>,
    pub(crate) timestamps: Vec<i64>,
    pub(crate) loading: bool,
    pub(crate) show_candles: bool, // false = line, true = candle bodies (future use)
    pub(crate) visible: bool,
}

// ─── Chart state ──────────────────────────────────────────────────────────────

pub(crate) struct DarkPoolPrint {
    pub(crate) price: f32,
    pub(crate) size: u64,
    pub(crate) time: i64,
    pub(crate) side: i8, // 1=buy, -1=sell, 0=unknown
}

/// What type of content a pane displays.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum PaneType {
    Chart,          // standard candlestick/line chart (default)
    Portfolio,      // portfolio positions table + risk analytics
    Dashboard,      // masonry grid of widgets (no chart)
    Heatmap,        // market/sector heatmap treemap
    Spreadsheet,    // editable string-cell grid
}

impl Default for PaneType { fn default() -> Self { Self::Chart } }

// ─── Named value types (replace anonymous tuples) ────────────────────────────

/// State for the alternative (non-time) chart bar types: Renko, Range, Tick.
/// The computed bars live here and are rebuilt when `dirty` or the source
/// bar count changes.
#[derive(Clone)]
pub(crate) struct AltBarsState {
    /// Renko brick size; 0.0 = auto (ATR-based). (was `renko_brick_size`)
    pub(crate) renko_brick: f32,
    /// Range-bar size; 0.0 = auto. (was `range_bar_size`)
    pub(crate) range_size:  f32,
    /// Tick-bar count; default 500. (was `tick_bar_count`)
    pub(crate) tick_count:  u32,
    /// Recomputed non-time bars. (was `alt_bars`)
    pub(crate) bars:        Vec<Bar>,
    /// Timestamps for the alt bars. (was `alt_timestamps`)
    pub(crate) timestamps:  Vec<i64>,
    /// True when the alt bars need recomputation. (was `alt_bars_dirty`)
    pub(crate) dirty:       bool,
    /// Source bar count when `bars` was last computed. (was `alt_bars_source_len`)
    pub(crate) source_len:  usize,
}

impl Default for AltBarsState {
    fn default() -> Self {
        Self {
            renko_brick: 0.0, range_size: 0.0, tick_count: 500,
            bars: vec![], timestamps: vec![], dirty: true, source_len: 0,
        }
    }
}

/// Symbol-search picker popup state (the `/`-triggered quick symbol switcher).
/// Field names drop the old `picker_` prefix.
#[derive(Default)]
pub(crate) struct SymbolPickerState {
    /// Popup is open. (was `picker_open`)
    pub(crate) open:       bool,
    /// Current query text. (was `picker_query`)
    pub(crate) query:      String,
    /// Results: `(symbol, name, exchange/type)`. (was `picker_results`)
    pub(crate) results:    Vec<(String, String, String)>,
    /// Last query searched — debounce guard. (was `picker_last_query`)
    pub(crate) last_query: String,
    /// Background search in flight. (was `picker_searching`)
    pub(crate) searching:  bool,
    /// Receiver for background search results. (was `picker_rx`)
    pub(crate) rx:         Option<mpsc::Receiver<Vec<(String, String, String)>>>,
    /// Anchor position for the popup. (was `picker_pos`)
    pub(crate) pos:        egui::Pos2,
}

/// Volume-profile display state: the active mode plus the cached computed data
/// and the view keys it was computed for (cache-invalidation).
pub(crate) struct VolumeProfileState {
    /// Display mode (Off / Classic / Heatmap / Strip / Clean). (was `vp_mode`)
    pub(crate) mode:    VolumeProfileMode,
    /// Cached computed profile. (was `vp_data`)
    pub(crate) data:    Option<VolumeProfileData>,
    /// View-start the cache was computed for; -1 = stale. (was `vp_last_vs`)
    pub(crate) last_vs: f32,
    /// View-count the cache was computed for. (was `vp_last_vc`)
    pub(crate) last_vc: u32,
}

impl Default for VolumeProfileState {
    fn default() -> Self {
        Self { mode: VolumeProfileMode::Off, data: None, last_vs: -1.0, last_vc: 0 }
    }
}

/// Drawing-tool picker popup state (2nd middle-click radial picker).
#[derive(Default)]
pub(crate) struct DrawPickerState {
    pub(crate) open:        bool,            // was draw_picker_open
    pub(crate) pos:         egui::Pos2,      // was draw_picker_pos
    pub(crate) hover_cat:   Option<String>,  // was draw_picker_hover_cat
    pub(crate) hover_cat_y: f32,             // was draw_picker_hover_cat_y
}

/// Template popup state (pane-header T button).
#[derive(Default)]
pub(crate) struct TemplatePopup {
    pub(crate) open:      bool,        // was template_popup_open
    pub(crate) pos:       egui::Pos2,  // was template_popup_pos
    pub(crate) save_name: String,      // was template_save_name
}

/// Option quick-picker popup state (options-tab click).
#[derive(Default)]
pub(crate) struct OptionQuickPicker {
    pub(crate) open:    bool,        // was option_quick_open
    pub(crate) pos:     egui::Pos2,  // was option_quick_pos
    pub(crate) dte_idx: usize,       // was option_quick_dte_idx
}

/// The per-pane order-ticket panel state (MeridienOrderTicket / ApertureOrderTicket).
/// Field names drop the old `order_`/`order_panel_` prefix. NOTE: distinct from the
/// `form::OrderTicketState`/`ApertureOrderState` adapters, which borrow these fields.
pub(crate) struct OrderPanelState {
    pub(crate) qty:             u32,        // was order_qty
    pub(crate) is_buy:          bool,       // was order_is_buy
    pub(crate) market:          bool,       // was order_market (true=market, false=limit)
    pub(crate) limit_price:     String,     // was order_limit_price
    pub(crate) type_idx:        usize,      // was order_type_idx (0=MKT,1=LMT,2=STP,3=STP-LMT,4=TRAIL)
    pub(crate) tif_idx:         usize,      // was order_tif_idx (0=DAY,1=GTC,2=IOC)
    pub(crate) outside_rth:     bool,       // was order_outside_rth
    pub(crate) advanced:        bool,       // was order_advanced (expanded mode)
    pub(crate) bracket:         bool,       // was order_bracket
    pub(crate) stop_price:      String,     // was order_stop_price
    pub(crate) trail_amt:       String,     // was order_trail_amt
    pub(crate) tp_price:        String,     // was order_tp_price
    pub(crate) sl_price:        String,     // was order_sl_price
    pub(crate) pos:             egui::Pos2, // was order_panel_pos
    pub(crate) dragging:        bool,       // was order_panel_dragging
    pub(crate) collapsed:       bool,       // was order_collapsed
    pub(crate) notional_mode:   bool,       // was order_notional_mode
    pub(crate) notional_amount: String,     // was order_notional_amount
}

impl Default for OrderPanelState {
    fn default() -> Self {
        Self {
            qty: 100, is_buy: true, market: true, limit_price: String::new(),
            type_idx: 0, tif_idx: 0, outside_rth: false, advanced: false, bracket: false,
            stop_price: String::new(), trail_amt: String::new(),
            tp_price: String::new(), sl_price: String::new(),
            pos: egui::pos2(8.0, -80.0), dragging: false, collapsed: false,
            notional_mode: false, notional_amount: String::new(),
        }
    }
}

/// All state for the DOM (Depth-of-Market / Price Ladder) panel.
#[derive(Clone)]
pub(crate) struct DomPanelState {
    /// Floating DOM window open.
    pub(crate) open:           bool,
    /// DOM built into the right side-rail of the pane (sidebar mode).
    pub(crate) sidebar_open:   bool,
    pub(crate) levels:         Vec<super::ui::panels::dom_panel::DomLevel>,
    pub(crate) tick_size:      f32,
    pub(crate) center_price:   f32,
    pub(crate) width:          f32,
    pub(crate) selected_price: Option<f32>,
    pub(crate) order_type:     super::ui::panels::dom_panel::DomOrderType,
    pub(crate) armed:          bool,
    /// 0 = single-column, 1 = split bid/ask columns.
    pub(crate) col_mode:       u8,
    pub(crate) dragging:       Option<(u32, f32)>,
    /// 0 = left edge of pane, 1 = right edge.
    pub(crate) position:       u8,
    /// DOM panel takes the full pane area; chart regions hidden.
    pub(crate) fullscreen:     bool,
    /// Epoch ms of the last live `DomLevels` frame. `0` = never. Used by the
    /// renderer to suppress the mock generator while a live feed is flowing.
    pub(crate) last_live_ms:   i64,
}

impl Default for DomPanelState {
    fn default() -> Self {
        Self {
            open: false, sidebar_open: false,
            levels: vec![], tick_size: 0.01, center_price: 0.0,
            width: super::ui::panels::dom_panel::DOM_SIDEBAR_W,
            selected_price: None,
            order_type: super::ui::panels::dom_panel::DomOrderType::Market,
            armed: false, col_mode: 1, dragging: None, position: 0, fullscreen: false,
            last_live_ms: 0,
        }
    }
}

/// `(bar_index, price)` coordinate used during in-progress drawing operations.
/// A transparent alias over `(f32, f32)` — existing destructuring patterns
/// (`if let Some((bar, price)) = …`) continue to work unchanged.
pub(crate) type DrawCoord = (f32, f32);

/// A detected change-point on the price series (from the change-point detection engine).
#[derive(Debug, Clone)]
pub(crate) struct ChangePoint {
    pub(crate) time:       i64,
    pub(crate) kind:       String,
    pub(crate) confidence: f32,
}

/// An active trade plan anchored to a chart pane.
#[derive(Debug, Clone)]
pub(crate) struct TradePlan {
    pub(crate) direction:  i8,
    pub(crate) entry:      f32,
    pub(crate) target:     f32,
    pub(crate) stop:       f32,
    pub(crate) contract:   String,
    pub(crate) rr:         f32,
    pub(crate) conviction: f32,
}

/// Measure-tool state (shift+drag distance measurement).
#[derive(Debug, Clone, Default)]
pub(crate) struct MeasureState {
    /// User is shift-dragging to measure (was `measuring`).
    pub(crate) active: bool,
    /// Start point in `(bar, price)` canvas coords (was `measure_start`).
    pub(crate) start:  Option<(f32, f32)>,
    /// Measure mode was activated via context menu (was `measure_active`).
    pub(crate) mode:   bool,
}

/// A gamma-exposure level for the options gamma overlay.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GammaLevel {
    /// Strike price.
    pub(crate) price:     f32,
    /// Net gamma exposure (positive = stabilising, negative = accelerating).
    pub(crate) exposure:  f32,
}

/// A continuation-gauge call at a down-thrust stall, from the signal feed
/// (`/signals/continuation/...`). Rendered as a HOLD (green) / EXIT (red) marker
/// on the chart's time axis. `p` is the pin-adjusted P(continuation); `strong`
/// = STRONG HOLD/EXIT band.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ContinuationMarker {
    /// Stall time, epoch ms (UTC) — aligns to bar timestamps.
    pub(crate) time:   i64,
    /// true = HOLD/continuation lean, false = EXIT/reversal lean.
    pub(crate) hold:   bool,
    /// STRONG band (|P-0.5| large) → brighter marker.
    pub(crate) strong: bool,
    /// Pin-adjusted P(continuation), 0..1.
    pub(crate) p:      f32,
}

/// Watchlist item drag state.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WatchlistDragState {
    pub(crate) section_idx: usize,
    pub(crate) item_idx:    usize,
}

/// A pending option-chart open request (deferred until next frame).
#[derive(Debug, Clone)]
pub(crate) struct PendingOptionChart {
    pub(crate) symbol:  String,
    pub(crate) strike:  f32,
    pub(crate) is_call: bool,
    pub(crate) expiry:  String,
}

/// A watchlist custom price-change filter.
#[derive(Debug, Clone)]
pub(crate) struct CustomFilter {
    pub(crate) name:       String,
    pub(crate) min_change: f32,
    pub(crate) max_change: f32,
}

/// A multi-selected order reference (pane + broker order id).
#[derive(Debug, Clone, Copy)]
pub(crate) struct SelectedOrder {
    pub(crate) pane_idx: usize,
    pub(crate) order_id: u32,
}

/// A fetched options chain (calls + puts).
#[derive(Debug, Clone, Default)]
pub(crate) struct OptionChain {
    pub(crate) calls: Vec<OptionRow>,
    pub(crate) puts:  Vec<OptionRow>,
}

// ─────────────────────────────────────────────────────────────────────────────

pub(crate) struct Chart {
    pub(crate) pane_type: PaneType,
    pub(crate) symbol: String, pub(crate) timeframe: String,
    /// Typed metadata for `symbol`. Wave 9c: populated/refreshed whenever
    /// `symbol` changes via `crate::foundation::types::symbol_or_guess`.
    /// Reading this gives `asset_class` + vendor aliases without re-parsing
    /// the canonical String — the String stays canonical for wire/display.
    /// Heads-up: hot-path code in `render/pane/core.rs` reads `symbol`
    /// directly; do NOT change the String to a Symbol.
    pub(crate) symbol_meta: crate::foundation::types::Symbol,
    // Option chart metadata
    pub(crate) is_option: bool,
    pub(crate) underlying: String, // e.g. "SPY" when this chart shows an option
    pub(crate) option_type: String, // "C" or "P"
    pub(crate) option_strike: f32,
    pub(crate) option_expiry: String, // "20260402"
    pub(crate) option_con_id: i64,
    /// OCC ticker for the option contract (e.g. "O:SPY251219C00450000").
    /// Non-empty when this chart shows a specific contract; used as the ApexData
    /// fetch key while `symbol` carries the human-readable display label.
    pub(crate) option_contract: String,
    /// MARK_BARS_PROTOCOL: bar source for this (option) pane.
    /// `false` = "last" (trade prints, default), `true` = "mark" (NBBO mid).
    /// Stock panes ignore this — they always fetch/sub Last.
    /// Persisted with the chart.
    pub(crate) bar_source_mark: bool,
    pub(crate) bars: Vec<Bar>, pub(crate) timestamps: Vec<i64>, pub(crate) drawings: Vec<Drawing>,
    /// Per-(symbol, timeframe) cache of bars/timestamps. Tab switches stash the
    /// current data here and restore from here when re-entering, so a tab swap
    /// shows the previous chart instantly while a fresh fetch runs in the bg.
    pub(crate) tab_cache: std::collections::HashMap<(String, String), (Vec<Bar>, Vec<i64>, std::time::Instant)>,
    pub(crate) indicators: Vec<Indicator>,
    pub(crate) indicator_bar_count: usize, // bar count when indicators were last computed
    pub(crate) next_indicator_id: u32,
    pub(crate) editing_indicator: Option<u32>, // id of indicator being edited
    /// Monotonic request generation, bumped on every symbol/timeframe change. A
    /// LoadBars result carries the gen it was fetched under; if it arrives stale
    /// (gen < this) it's dropped so a superseded async load can't clobber the pane.
    pub(crate) request_gen: u64,
    pub(crate) vs: f32, pub(crate) vc: u32, pub(crate) price_lock: Option<(f32,f32)>,
    pub(crate) log_scale: bool,
    pub(crate) drag_zoom_active: bool,
    pub(crate) drag_zoom_start: Option<egui::Pos2>,
    pub(crate) auto_scroll: bool, pub(crate) last_input: std::time::Instant,
    pub(crate) draw_price_freeze: Option<(f32, f32)>, // locks y-range while drawing so new bars can't rescale
    // Template popup (opened from pane header T button)
    pub(crate) template_popup: TemplatePopup,
    // Option quick-picker popup (opened by clicking an options tab)
    pub(crate) option_quick: OptionQuickPicker,
    pub(crate) history_loading: bool, // true while fetching older bars
    pub(crate) history_exhausted: bool, // true if no more history available
    /// Set when the bar fetch returned nothing / every source failed, so the
    /// pane shows an honest "no data" message instead of an endless spinner.
    /// Cleared on a successful LoadBars or a new symbol/timeframe request.
    pub(crate) load_error: Option<String>,
    pub(crate) tick_counter: u64, pub(crate) last_candle_time: std::time::Instant, pub(crate) sim_price: f32, pub(crate) sim_seed: u64,
    pub(crate) theme_idx: usize,
    pub(crate) draw_tool: String, // "", "hline", "trendline", "hzone", "barmarker", "fibonacci", "channel"
    /// Drawing-tool picker (opened by 2nd middle-click while a tool is active).
    pub(crate) draw_picker: DrawPickerState,
    pub(crate) pending_pt:  Option<DrawCoord>, // first click (bar, price)
    pub(crate) pending_pt2: Option<DrawCoord>, // second click for channel (bar, price)
    pub(crate) pending_pts: Vec<DrawCoord>,    // multi-point: pitchfork(3), xabcd(5), elliott(3/5)
    pub(crate) magnet: bool, // snap to OHLC when placing drawings
    pub(crate) selected_id: Option<String>,
    pub(crate) selected_ids: Vec<String>, // multi-select with shift
    pub(crate) dragging_drawing: Option<(String, i32)>,
    pub(crate) drag_start_price: f32, pub(crate) drag_start_bar: f32,
    pub(crate) groups: Vec<DrawingGroup>,
    pub(crate) hidden_groups: Vec<String>,
    pub(crate) signal_drawings: Vec<SignalDrawing>, // auto-generated trendlines from server
    pub(crate) hide_signal_drawings: bool,
    pub(crate) hidden_signal_methods: Vec<String>, // detection_methods toggled off in the filter
    pub(crate) pattern_labels: Vec<PatternLabel>,   // candlestick pattern labels from ApexSignals
    pub(crate) show_pattern_labels: bool,
    // ── Signal engine state ──────────────────────────────────────────────────
    pub(crate) trend_health_score: f32,
    pub(crate) trend_health_direction: i8,
    pub(crate) trend_health_regime: String,
    pub(crate) exit_gauge_score: f32,
    pub(crate) exit_gauge_urgency: String,
    pub(crate) signal_zones: Vec<super::SignalZone>,
    pub(crate) precursor_active: bool,
    pub(crate) precursor_score: f32,
    pub(crate) precursor_direction: i8,
    pub(crate) precursor_description: String,
    pub(crate) change_points: Vec<ChangePoint>,
    pub(crate) trade_plan: Option<TradePlan>,
    pub(crate) divergence_markers: Vec<super::DivergenceMarker>,
    pub(crate) show_divergences: bool,
    pub(crate) signal_demo_toggle: bool, // set to true to toggle demo on/off
    // Per-signal visibility toggles (controlled from Signals panel)
    pub(crate) show_trend_health: bool,
    pub(crate) show_exit_gauge: bool,
    pub(crate) show_precursor: bool,
    pub(crate) show_signal_zones: bool,
    pub(crate) show_trade_plan: bool,
    pub(crate) show_change_points: bool,
    pub(crate) show_vix_alert: bool,
    pub(crate) show_auto_trendlines: bool, // mirrors !hide_signal_drawings, for UI consistency
    // VIX Expiry alert
    pub(crate) vix_expiry_active: bool,
    pub(crate) vix_expiry_days: u32,
    pub(crate) vix_expiry_date: String,
    pub(crate) vix_spot: f32,
    pub(crate) vix_expiring_future: f32,
    pub(crate) vix_realized_vol: f32,
    pub(crate) vix_gap_pct: f32,
    pub(crate) vix_convergence_score: f32,
    pub(crate) drawings_requested: bool, // prevents duplicate fetch_drawings_background calls
    pub(crate) last_signal_fetch: std::time::Instant,
    pub(crate) hide_all_drawings: bool,
    pub(crate) hide_all_indicators: bool,
    pub(crate) ohlc_tooltip: bool, // show OHLC values at crosshair
    pub(crate) measure_tooltip: bool, // show big distance-only measurement at crosshair
    pub(crate) show_volume: bool,
    pub(crate) show_oscillators: bool, // toggle oscillator sub-panel
    pub(crate) draw_color: String, // current drawing color
    pub(crate) zoom_selecting: bool, pub(crate) zoom_start: egui::Pos2,
    pub(crate) axis_drag_mode: u8, // 0=none, 1=xaxis, 2=yaxis
    // Symbol picker
    pub(crate) picker: SymbolPickerState,
    pub(crate) recent_symbols: Vec<(String, String)>, // (symbol, name) — most recent first, max 20
    // Group management
    pub(crate) group_manager_open: bool,
    pub(crate) new_group_name: String,
    // Orders
    pub(crate) orders: Vec<OrderLevel>,
    pub(crate) next_order_id: u32,
    pub(crate) order_panel: OrderPanelState,
    pub(crate) dragging_order: Option<u32>, // order id being dragged
    pub(crate) dragging_alert: Option<u32>, // alert id being dragged (includes drafts)
    pub(crate) editing_order: Option<u32>,
    pub(crate) edit_order_qty: String,
    pub(crate) edit_order_price: String,
    pub(crate) armed: bool, // skip confirmation, fire orders immediately
    pub(crate) pending_confirms: Vec<(u32, std::time::Instant)>, // order ids awaiting user confirm from panel
    // ── Trigger orders (options on underlying price) ──
    pub(crate) trigger_setup: TriggerSetup,
    pub(crate) trigger_levels: Vec<TriggerLevel>,
    pub(crate) pending_und_order: Option<OrderSide>, // deferred: activate underlying crosshair
    pub(crate) next_trigger_id: u32,
    pub(crate) dragging_trigger: Option<u32>,
    pub(crate) editing_trigger: Option<u32>,
    // Widget data cache (avoid recomputing every frame)
    pub(crate) widget_cache_bar_count: usize,
    pub(crate) widget_cache: Option<super::ui::chart_widgets::WidgetDataCache>,
    // ── Play lines (chart companion for play editor) ──
    pub(crate) play_lines: Vec<super::PlayLine>,
    pub(crate) next_play_line_id: u32,
    pub(crate) dragging_play_line: Option<u32>,
    pub(crate) play_click_to_set: Option<super::PlayLineKind>, // click-on-chart fills price
    /// Label of the level the dragged play line is currently snapped to (HUD).
    pub(crate) play_snap_label: Option<String>,
    // Measure tool (shift+drag)
    pub(crate) measure: MeasureState,
    pub(crate) dom: DomPanelState,
    // Symbol/timeframe change request — signals the App to reload data
    pub(crate) pending_symbol_change: Option<String>,
    pub(crate) pending_timeframe_change: Option<String>,
    // Undo/redo
    pub(crate) undo_stack: Vec<DrawingAction>,
    pub(crate) redo_stack: Vec<DrawingAction>,
    pub(crate) drag_drawing_snapshot: Option<Drawing>,
    // Text annotation editing
    pub(crate) text_edit_id: Option<String>,
    pub(crate) text_edit_buf: String,
    // Reusable buffers to avoid per-frame allocations
    pub(crate) indicator_pts_buf: Vec<egui::Pos2>,
    pub(crate) fmt_buf: String, // reusable format buffer
    pub(crate) vp: VolumeProfileState,
    pub(crate) candle_mode: CandleMode,
    // Alternative chart types (Renko, Range, Tick)
    pub(crate) alt: AltBarsState,
    pub(crate) show_footprint: bool, // hover-activated volume footprint on individual bars
    // Volume analytics
    pub(crate) show_vwap_bands: bool,
    pub(crate) show_cvd: bool,
    pub(crate) show_delta_volume: bool,
    pub(crate) show_rvol: bool,
    pub(crate) show_ma_ribbon: bool,
    pub(crate) show_prev_close: bool,
    pub(crate) show_auto_sr: bool,
    pub(crate) show_auto_fib: bool,
    pub(crate) swing_leg_mode: u8, // 0=off, 1=vertical, 2=diagonal
    pub(crate) symbol_overlays: Vec<SymbolOverlay>,
    pub(crate) overlay_editing: bool,
    pub(crate) overlay_editing_idx: Option<usize>, // Some(i) = editing existing overlay, None = adding new
    pub(crate) overlay_input: String,
    pub(crate) show_gamma: bool,
    // Hit-test highlighting — flash indicators/drawings when price touches them
    pub(crate) hit_highlight: bool,
    pub(crate) hit_highlights: Vec<(u32, std::time::Instant)>, // (key, when hit detected)
    pub(crate) hit_cooldowns: Vec<(u32, usize)>, // (key, bar_index when last triggered) — cooldown for 5 bars
    pub(crate) show_events: bool,
    pub(crate) event_markers: Vec<EventMarker>,
    pub(crate) show_strikes_overlay: bool, // show option strikes on the chart
    pub(crate) overlay_calls: Vec<OptionRow>, // independent chain data for strikes overlay
    pub(crate) overlay_puts: Vec<OptionRow>,
    pub(crate) overlay_chain_symbol: String, // symbol for which overlay data is loaded
    pub(crate) overlay_chain_loading: bool,
    /// True when the overlay chain was synthesized locally (real upstream
    /// unavailable). Renderer paints a "PLACEHOLDER" tag on the axis.
    pub(crate) overlay_chain_placeholder: bool,
    pub(crate) floating_order_panes: Vec<FloatingOrderPane>, // floating order entry windows
    pub(crate) gamma_levels: Vec<GammaLevel>,
    pub(crate) gamma_call_wall: f32,
    pub(crate) gamma_put_wall: f32,
    pub(crate) gamma_zero: f32,
    pub(crate) gamma_hvl: f32,
    /// Flow layer (PPE) from the gamma feed — populated only during market
    /// hours (feed reports `flow.active=false` off-hours). `None` PPE = no
    /// live flow reading right now.
    pub(crate) gamma_ppe: Option<f32>,
    pub(crate) gamma_iv_rising: Option<bool>,
    pub(crate) gamma_flow_active: bool,
    /// `short_posture.posture` string (e.g. `hold_press`). Empty when absent.
    pub(crate) gamma_posture: String,
    /// F3 (audit): true when the current gamma_levels are SYNTHETIC placeholders
    /// (the :8412 feed was unavailable), false when they came from the real feed.
    /// Drives the "SYNTHETIC" chart badge so a trader is never misled into
    /// treating fabricated GEX walls as real. Not persisted (runtime-derived).
    pub(crate) gamma_synthetic: bool,
    /// Continuation-gauge HOLD/EXIT markers (signal feed). Shown with the gamma
    /// overlay; refreshed for the date currently loaded on the chart.
    pub(crate) continuation_signals: Vec<ContinuationMarker>,
    // Analytics overlays
    pub(crate) show_vol_shelves: bool,
    pub(crate) show_confluence: bool,
    pub(crate) show_momentum_heat: bool,
    pub(crate) show_trend_strip: bool,
    pub(crate) show_breadth_tint: bool,
    pub(crate) show_vol_cone: bool,
    pub(crate) show_price_memory: bool,
    pub(crate) show_liquidity_voids: bool,
    pub(crate) show_corr_ribbon: bool,
    // Dark Pool overlay
    // Fundamental data + research
    pub(crate) fundamentals: FundamentalData,
    pub(crate) show_analyst_targets: bool,
    pub(crate) show_pe_band: bool,
    pub(crate) show_insider_trades: bool,
    pub(crate) insider_trades: Vec<InsiderTrade>,
    pub(crate) show_corp_actions: bool,
    pub(crate) corp_actions: Vec<CorpAction>,
    pub(crate) econ_calendar: Vec<EconEvent>,
    pub(crate) show_darkpool: bool,
    pub(crate) darkpool_prints: Vec<DarkPoolPrint>,
    pub(crate) vwap_data: Vec<f32>,
    pub(crate) vwap_upper1: Vec<f32>,
    pub(crate) vwap_lower1: Vec<f32>,
    pub(crate) vwap_upper2: Vec<f32>,
    pub(crate) vwap_lower2: Vec<f32>,
    pub(crate) cvd_data: Vec<f32>,
    pub(crate) delta_data: Vec<f32>,
    pub(crate) rvol_data: Vec<f32>,
    pub(crate) vol_analytics_computed: usize,
    pub(crate) replay_mode: bool,
    pub(crate) replay_bar_count: usize,
    pub(crate) replay_playing: bool,
    pub(crate) replay_speed: f32,      // 1.0 = normal, 2.0 = 2x, etc.
    pub(crate) replay_last_step: Option<std::time::Instant>,
    /// Replay overlay installed by the `ReplayScrubber` pane (sibling branch
    /// `sota-terminal-replay`). When `Some`, the chart render loop will draw
    /// these bars in a distinct color on top of the live bars. See the
    /// `ReplayOverlay` doc comment for the render contract.
    pub replay_overlay: Option<ReplayOverlay>,
    // Notional-based order entry
    // (order_notional_mode / order_notional_amount moved into `order_panel`.)
    // Bracket order templates
    pub(crate) bracket_templates: Vec<BracketTemplate>,
    pub(crate) new_bracket_name: String,
    pub(crate) new_bracket_target: String,
    pub(crate) new_bracket_stop: String,
    // ── Linked pane groups ──
    pub(crate) link_group: u8, // 0=unlinked, 1-4 = link group (blue, green, orange, purple)
    // ── Per-pane price alerts (rendered on chart) ──
    pub(crate) price_alerts: Vec<PriceAlert>,
    pub(crate) next_alert_id: u32,
    pub(crate) alert_input_price: String,
    // ── P&L equity curve ──
    pub(crate) show_pnl_curve: bool,
    // Floating chart widgets (info cards on the canvas)
    pub(crate) chart_widgets: Vec<super::ChartWidget>,
    pub(crate) dragging_widget: Option<usize>, // index of widget being dragged
    // ── Symbol history breadcrumb (back/forward navigation) ──
    pub(crate) symbol_history: Vec<String>,
    pub(crate) symbol_history_idx: usize,
    pub(crate) symbol_nav_in_progress: bool, // true when navigating via back/forward (skip history push)
    // ── Smooth zoom animation ──
    pub(crate) vc_target: u32,
    // ── Auto-fit price animation ──
    pub(crate) price_range_animated: Option<(f32, f32)>,
    // ── Tabs (multiple symbols per pane) ──
    pub(crate) tab_symbols: Vec<String>, // symbol per tab
    pub(crate) tab_timeframes: Vec<String>, // timeframe per tab
    pub(crate) tab_changes: Vec<f32>, // cached daily change % per tab
    pub(crate) tab_prices: Vec<f32>,  // cached last-known price per tab (0.0 = unknown)
    pub(crate) tab_active: usize, // index of active tab (0-based)
    pub(crate) tab_hovered: Option<usize>, // which tab the mouse is over (for close button)
    // -- Session shading (pre/post market) --
    pub(crate) session_shading: bool,          // master toggle for ETH dimming
    pub(crate) rth_start_minutes: u16,         // 570 = 9:30 AM ET
    pub(crate) rth_end_minutes: u16,           // 960 = 4:00 PM ET
    pub(crate) eth_bar_opacity: f32,           // 0.35 default (0.0-1.0)
    pub(crate) session_bg_tint: bool,          // shade background behind ETH bars
    pub(crate) session_bg_color: String,       // "#1a1a2e" default
    pub(crate) session_bg_opacity: f32,        // 0.15 default (0.0-1.0)
    pub(crate) session_break_lines: bool,      // vertical dashed lines at session boundaries
    // -- Spreadsheet pane state --
    pub(crate) spreadsheet_cells: Vec<Vec<String>>,
    pub(crate) spreadsheet_cols: usize,
    pub(crate) spreadsheet_rows: usize,
    pub(crate) spreadsheet_selected: Option<(usize, usize)>,
    pub(crate) spreadsheet_editing: Option<(usize, usize, String)>,
    // -- Pane content picker popup --
    pub(crate) pane_template_name: Option<String>, // currently selected template name for active mode
    pub(crate) pane_picker_open: bool,
    pub(crate) pane_picker_pos: egui::Pos2,
    pub(crate) pane_picker_query: String,          // symbol search query inside picker
    pub(crate) pane_picker_save_name: String,      // template name input in pane picker
    pub(crate) pane_picker_option_mode: bool,      // Chart-mode picker: false=ticker, true=option chain
    /// GPU candle render params — populated each frame by render_chart_pane,
    /// consumed by ChartPipeline::upload before the chart render pass.
    #[cfg(feature = "gpu_chart_v2")]
    pub(crate) gpu_render_params: crate::chart::renderer_gpu::ChartRenderParams,

    // ── Phase 5 migration shim ────────────────────────────────────────────────
    // Canonical per-chart state. Populated after symbol/timeframe are committed;
    // `None` only during the very first `Chart::new()` before the first load.
    //
    // Migration contract (see docs/STATE_MIGRATION_PHASE5.md):
    //   - Persistence (XOL/DB codec) reads/writes from this field.
    //   - Renderer fields (vs, vc, drawings, indicators, …) remain the live
    //     source of truth until their migration tier is executed.
    //   - Never read `chart_state` inside `render/pane/core.rs` until
    //     Tier 3.2 is benchmarked and reviewed.
    pub(crate) chart_state: Option<crate::chart::state::ChartState>,
}

/// Hard cap on Chart::tab_cache entries.
///
/// Each entry holds bar data (~120 KB at 5000 bars), so an unbounded cache
/// leaks memory across long sessions. 64 is enough to keep recently-visited
/// (symbol, timeframe) tab swaps warm without blowing past a few MB per pane.
pub(crate) const TAB_CACHE_MAX: usize = 64;

/// LRU-evict the oldest tab_cache entry (by stored `Instant`) when the cache
/// is at or above `TAB_CACHE_MAX`. Call this immediately before every insert
/// to keep the post-insert size bounded by `TAB_CACHE_MAX`.
pub(crate) fn evict_oldest_if_full(
    cache: &mut std::collections::HashMap<(String, String), (Vec<Bar>, Vec<i64>, std::time::Instant)>,
) {
    if cache.len() < TAB_CACHE_MAX {
        return;
    }
    if let Some(oldest_key) = cache
        .iter()
        .min_by_key(|(_, (_, _, t))| *t)
        .map(|(k, _)| k.clone())
    {
        cache.remove(&oldest_key);
    }
}

impl Chart {
    pub(crate) fn new_with(symbol: &str, timeframe: &str) -> Self {
        let mut c = Self::new();
        c.symbol = symbol.into();
        c.symbol_meta = crate::foundation::types::symbol_or_guess(symbol);
        c.timeframe = timeframe.into();
        c
    }

    /// Populate the GEX/gamma overlay levels for this pane: prefer the real
    /// gamma feed (gamma_feed_service / ApexSignals via :8412), else synthesize
    /// placeholder levels so the overlay renders immediately. Single source of
    /// truth shared by the toolbar Overlays menu and the dev-inspector
    /// `SynthGamma` command — the command path previously only flipped the
    /// `show_gamma` bool (via `SetChartFlag`) and never populated levels, so
    /// gamma scenarios stayed empty whenever the :8412 feed was absent.
    pub(crate) fn populate_gamma(&mut self, force_synth: bool) {
        if !self.gamma_levels.is_empty() {
            return;
        }
        // Prefer the real feed unless the caller forces synthesis (the harness
        // SynthGamma command does, so gamma is deterministic without :8412).
        // Treat an empty feed snapshot as "no data" and fall through to synth —
        // for QQQ/SPY the feed is *queried* but returns nothing when :8412 is down.
        if !force_synth {
            if let Some(snap) = crate::chart_renderer::gpu::fetch_gamma_from_feed(&self.symbol) {
                if !snap.levels.is_empty() {
                    self.gamma_levels = snap.levels;
                    self.gamma_zero = snap.flip;
                    self.gamma_call_wall = snap.call_wall;
                    self.gamma_put_wall = snap.put_wall;
                    self.gamma_ppe = snap.ppe;
                    self.gamma_iv_rising = snap.iv_rising;
                    self.gamma_flow_active = snap.flow_active;
                    self.gamma_posture = snap.posture;
                    if let Some(last_bar) = self.bars.last() {
                        self.gamma_hvl = last_bar.close;
                    }
                    self.gamma_synthetic = false; // real feed data
                    return;
                }
            }
        }
        // Feed unavailable — synthesize placeholder levels. Works even when bars
        // are empty (falls back to a sensible default price). F3: mark synthetic
        // so the overlay shows a "SYNTHETIC" badge.
        self.gamma_synthetic = true;
        let price = self.bars.last().map(|b| b.close).filter(|&p| p > 0.0).unwrap_or(500.0);
        let step = if price > 200.0 { 5.0 } else if price > 50.0 { 2.5 } else { 1.0 };
        let mut levels: Vec<GammaLevel> = vec![];
        for i in -15..=15_i32 {
            let level_price = (price / step).round() * step + i as f32 * step;
            let dist = i.abs() as f32;
            let gex = if dist < 5.0 {
                (500.0 - dist * 80.0) * (1.0 + 0.3 * (level_price * 7.3).sin())
            } else {
                (-100.0 - (dist - 5.0) * 50.0) * (1.0 + 0.2 * (level_price * 3.1).sin())
            };
            levels.push(GammaLevel { price: level_price, exposure: gex });
        }
        // Walls must sit on the correct side of spot: the call wall (largest
        // gamma above price) above, the put wall (largest gamma below price)
        // below — mirroring how a real GEX profile brackets spot. Picking global
        // max/min exposure ignored side and could invert them.
        let center = (price / step).round() * step;
        let by_mag = |a: &&GammaLevel, b: &&GammaLevel|
            a.exposure.abs().partial_cmp(&b.exposure.abs()).unwrap_or(std::cmp::Ordering::Equal);
        let call = levels.iter().filter(|l| l.price > center).max_by(by_mag);
        let put  = levels.iter().filter(|l| l.price < center).max_by(by_mag);
        self.gamma_call_wall = call.map_or(center + 10.0 * step, |l| l.price);
        self.gamma_put_wall  = put.map_or(center - 10.0 * step, |l| l.price);
        // Flip (zero-gamma) sits at spot for the placeholder profile.
        self.gamma_zero = center;
        self.gamma_hvl  = levels.iter().filter(|l| l.exposure > 0.0).max_by(|a, b|
            a.exposure.partial_cmp(&b.exposure).unwrap_or(std::cmp::Ordering::Equal))
            .map_or(center, |l| l.price);
        self.gamma_levels = levels;
    }
    pub(crate) fn new() -> Self {
        Self { pane_type: PaneType::Chart,
            symbol: "AAPL".into(),
            symbol_meta: crate::foundation::types::symbol_or_guess("AAPL"),
            timeframe: "5m".into(),
            is_option: false, underlying: String::new(), option_type: String::new(),
            option_strike: 0.0, option_expiry: String::new(), option_con_id: 0, option_contract: String::new(),
            bar_source_mark: false,
            bars: vec![], timestamps: vec![], drawings: vec![], tab_cache: std::collections::HashMap::new(), indicator_bar_count: 0,
            next_indicator_id: 5, editing_indicator: None, request_gen: 0,
            indicators: vec![
                Indicator::new(1, IndicatorType::SMA, 20, "#00bef0"),
                Indicator::new(2, IndicatorType::SMA, 50, "#f0961a"),
                Indicator::new(3, IndicatorType::EMA, 12, "#f0d732"),
                Indicator::new(4, IndicatorType::EMA, 26, "#b266e6"),
            ],
            vs: 0.0, vc: 200, price_lock: None, log_scale: false, drag_zoom_active: false, drag_zoom_start: None,
            auto_scroll: true, draw_price_freeze: None,
            template_popup: TemplatePopup::default(),
            option_quick: OptionQuickPicker::default(),
            history_loading: false, history_exhausted: false, load_error: None,
            last_input: std::time::Instant::now(), tick_counter: 0,
            last_candle_time: std::time::Instant::now(), sim_price: 0.0,
            sim_seed: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(42),
            theme_idx: 5, // Gruvbox
            draw_tool: String::new(), draw_picker: DrawPickerState::default(), pending_pt: None, pending_pt2: None, pending_pts: vec![], magnet: true,
            selected_id: None, selected_ids: vec![], dragging_drawing: None,
            drag_start_price: 0.0, drag_start_bar: 0.0,
            groups: vec![DrawingGroup { id: "default".into(), name: "Temp".into(), color: None }],
            hidden_groups: vec![], hide_all_drawings: false, hide_all_indicators: false, show_volume: true, show_oscillators: true, ohlc_tooltip: true, measure_tooltip: false,
            signal_drawings: vec![], hide_signal_drawings: false, hidden_signal_methods: vec![],
            pattern_labels: vec![], show_pattern_labels: true,
            trend_health_score: 0.0, trend_health_direction: 0, trend_health_regime: String::new(),
            exit_gauge_score: 0.0, exit_gauge_urgency: String::new(),
            signal_zones: vec![], precursor_active: false, precursor_score: 0.0,
            precursor_direction: 0, precursor_description: String::new(),
            change_points: vec![], trade_plan: None,
            divergence_markers: vec![], show_divergences: true,
            signal_demo_toggle: false,
            show_trend_health: true, show_exit_gauge: true, show_precursor: true,
            show_signal_zones: true, show_trade_plan: true, show_change_points: true,
            show_vix_alert: true, show_auto_trendlines: true,
            vix_expiry_active: false, vix_expiry_days: 0, vix_expiry_date: String::new(),
            vix_spot: 0.0, vix_expiring_future: 0.0, vix_realized_vol: 0.0,
            vix_gap_pct: 0.0, vix_convergence_score: 0.0,
            last_signal_fetch: std::time::Instant::now(), drawings_requested: false,
            draw_color: indicator_default_color(0, &get_theme(5)), group_manager_open: false, new_group_name: String::new(),
            zoom_selecting: false, zoom_start: egui::Pos2::ZERO, axis_drag_mode: 0,
            picker: SymbolPickerState::default(),
            recent_symbols: vec![("AAPL".into(), "Apple".into()), ("SPY".into(), "S&P 500 ETF".into()), ("TSLA".into(), "Tesla".into()), ("NVDA".into(), "Nvidia".into()), ("MSFT".into(), "Microsoft".into())],
            orders: vec![], next_order_id: 1, order_panel: OrderPanelState::default(),
            dragging_order: None, dragging_alert: None, editing_order: None, edit_order_qty: String::new(), edit_order_price: String::new(),
            armed: false, pending_confirms: vec![],
            trigger_setup: TriggerSetup::default(), trigger_levels: vec![], next_trigger_id: 1, dragging_trigger: None, editing_trigger: None, pending_und_order: None,
            widget_cache_bar_count: 0, widget_cache: None,
            play_lines: vec![], next_play_line_id: 1, dragging_play_line: None, play_click_to_set: None,
            play_snap_label: None,
            measure: MeasureState::default(), dom: DomPanelState::default(),
            pending_symbol_change: None, pending_timeframe_change: None,
            undo_stack: vec![], redo_stack: vec![], drag_drawing_snapshot: None,
            text_edit_id: None, text_edit_buf: String::new(),
            indicator_pts_buf: Vec::with_capacity(512), fmt_buf: String::with_capacity(256),
            vp: VolumeProfileState::default(), candle_mode: CandleMode::Standard,
            alt: AltBarsState::default(),
            show_footprint: false,
            show_vwap_bands: false, show_cvd: false, show_delta_volume: false, show_rvol: true,
            show_ma_ribbon: false, show_prev_close: true, show_auto_sr: false, show_auto_fib: false, swing_leg_mode: 0,
            symbol_overlays: vec![], overlay_editing: false, overlay_editing_idx: None, overlay_input: String::new(),
            show_gamma: false, hit_highlight: false, hit_highlights: vec![], hit_cooldowns: vec![],
            show_events: false, event_markers: vec![],
            show_strikes_overlay: false, overlay_calls: vec![], overlay_puts: vec![], overlay_chain_symbol: String::new(), overlay_chain_loading: false, overlay_chain_placeholder: false, floating_order_panes: vec![], gamma_levels: vec![], gamma_call_wall: 0.0, gamma_put_wall: 0.0, gamma_zero: 0.0, gamma_hvl: 0.0, gamma_ppe: None, gamma_iv_rising: None, gamma_flow_active: false, gamma_posture: String::new(), gamma_synthetic: false, continuation_signals: vec![],
            fundamentals: FundamentalData::default(), show_analyst_targets: false,
            show_pe_band: false, show_insider_trades: false, insider_trades: vec![],
            show_corp_actions: false, corp_actions: vec![],
            econ_calendar: vec![],
            show_vol_shelves: false, show_confluence: false,
            show_momentum_heat: false, show_trend_strip: false, show_breadth_tint: false,
            show_vol_cone: false, show_price_memory: false, show_liquidity_voids: false, show_corr_ribbon: false,
            show_darkpool: false, darkpool_prints: vec![],
            vwap_data: vec![], vwap_upper1: vec![], vwap_lower1: vec![], vwap_upper2: vec![], vwap_lower2: vec![],
            cvd_data: vec![], delta_data: vec![], rvol_data: vec![], vol_analytics_computed: 0,
            replay_mode: false, replay_bar_count: 0, replay_playing: false, replay_speed: 1.0, replay_last_step: None,
            replay_overlay: None,
            bracket_templates: vec![
                BracketTemplate { name: "Tight".into(),  target_pct: 1.0, stop_pct: 0.5 },
                BracketTemplate { name: "Normal".into(), target_pct: 2.0, stop_pct: 1.0 },
                BracketTemplate { name: "Wide".into(),   target_pct: 5.0, stop_pct: 2.0 },
                BracketTemplate { name: "Scalp".into(),  target_pct: 0.3, stop_pct: 0.15 },
            ],
            new_bracket_name: String::new(), new_bracket_target: String::new(), new_bracket_stop: String::new(),
            link_group: 0,
            price_alerts: vec![], next_alert_id: 1, alert_input_price: String::new(),
            show_pnl_curve: false, chart_widgets: vec![], dragging_widget: None,
            symbol_history: vec![], symbol_history_idx: 0, symbol_nav_in_progress: false,
            vc_target: 200,
            price_range_animated: None,
            tab_symbols: vec![], tab_timeframes: vec![], tab_changes: vec![], tab_prices: vec![], tab_active: 0, tab_hovered: None,
            session_shading: false, rth_start_minutes: 570, rth_end_minutes: 960,
            eth_bar_opacity: 0.35, session_bg_tint: false, session_bg_color: "#1a1a2e".into(),
            session_bg_opacity: 0.15, session_break_lines: true,
            spreadsheet_cells: vec![vec![String::new(); 4]; 8],
            spreadsheet_cols: 4,
            spreadsheet_rows: 8,
            spreadsheet_selected: None,
            spreadsheet_editing: None,
            pane_template_name: None,
            pane_picker_open: false,
            pane_picker_pos: egui::Pos2::ZERO,
            pane_picker_query: String::new(),
            pane_picker_save_name: String::new(),
            pane_picker_option_mode: false,
            #[cfg(feature = "gpu_chart_v2")]
            gpu_render_params: crate::chart::renderer_gpu::ChartRenderParams::default(),
            chart_state: None,
        }
    }

    // ── Replay overlay hook (public API) ─────────────────────────────────
    // Driven by the `ReplayScrubber` pane on branch `sota-terminal-replay`.
    // See the `ReplayOverlay` doc comment near the top of this file for the
    // render contract.

    /// Install a replay overlay on this chart. Replaces any existing overlay.
    pub fn set_replay_overlay(&mut self, overlay: ReplayOverlay) {
        self.replay_overlay = Some(overlay);
    }

    /// Append a single bar to the active replay overlay. Creates an empty
    /// overlay (with the default color and an empty label) if none is
    /// installed yet — useful for the streaming/WS path where bars arrive
    /// incrementally before any explicit `set_replay_overlay` call.
    pub fn append_replay_bar(&mut self, bar: Bar, t_ms: i64) {
        let overlay = self
            .replay_overlay
            .get_or_insert_with(|| ReplayOverlay::new(String::new()));
        overlay.push(bar, t_ms);
    }

    /// Remove the replay overlay (if any). The next frame will render only
    /// the live bars.
    pub fn clear_replay_overlay(&mut self) {
        self.replay_overlay = None;
    }

    fn process(&mut self, cmd: ChartCommand) {
        match cmd {
            ChartCommand::LoadBars { bars, timestamps, symbol, timeframe, gen, .. } => {
                // Skip if this pane is an option chart and the LoadBars is for the underlying
                if self.is_option && symbol != self.symbol { return; }
                // Stale-generation drop (the authoritative guard): this result was
                // fetched under an older request generation than the pane now holds —
                // i.e. the user has since switched symbol/timeframe (each bumps the
                // generation), so this load is superseded. gen == 0 is unversioned
                // (initial load / reset) and always applies. This catches even the
                // same-(symbol,timeframe) races the symbol/tf guards below cannot.
                if gen != 0 && gen < self.request_gen { return; }
                // Stale-timeframe / -symbol drops (belt-and-suspenders, and they also
                // cover gen==0 / unversioned loads): don't let a late result overwrite
                // the pane's current symbol or timeframe.
                if symbol == self.symbol && !self.timeframe.is_empty() && timeframe != self.timeframe {
                    return;
                }
                let is_new_symbol = self.symbol != symbol;
                self.symbol = symbol;
                if is_new_symbol {
                    self.symbol_meta = crate::foundation::types::symbol_or_guess(&self.symbol);
                }
                self.timeframe = timeframe;
                // Futures live-bar feed follows the chart's symbol + timeframe
                // (no-op / deactivates for non-futures symbols).
                crate::data::futures_feed::set_target(&self.symbol, &self.timeframe);
                self.bars = bars; self.timestamps = timestamps;
                self.load_error = None; // bars arrived — clear any prior "no data" state
                // Allow negative vs: fewer bars than vc → bars right-align instead
                // of clustering at the left edge with empty space on the right.
                self.vs = self.bars.len() as f32 - self.vc as f32 + CHART_RIGHT_PAD as f32;
                self.sim_price = 0.0;
                self.last_candle_time = std::time::Instant::now();
                self.indicator_bar_count = 0; // force recompute
                self.vol_analytics_computed = 0; // force vol analytics recompute
                self.price_range_animated = None; // reset — no slide animation on symbol/tf change
                // Drawings: fetch asynchronously via single worker thread
                if is_new_symbol {
                    // Re-point the DOM (L2 depth) feed at the new symbol.
                    crate::data::dom_feed::set_symbol(&self.symbol);
                    self.dom.last_live_ms = 0; // drop stale live flag until first frame
                    self.dom.selected_price = None; // reset stale selected price level
                    self.dom.center_price = 0.0;    // reset scroll position for new symbol
                    self.option_quick.dte_idx = 0;  // reset to 0DTE on symbol change
                    self.drawings_requested = false; self.drawings.clear();
                    // No synthetic fundamentals / econ-calendar / insider trades —
                    // these have no live feed yet (see FRONTEND_REQUEST_DATA_GAPS),
                    // so leave them empty and let the panels show honest "no data"
                    // states instead of fabricated numbers. The COMPANY section
                    // (name/market-cap/sector) is real via /api/ticker.
                    self.fundamentals = FundamentalData::default();
                    self.econ_calendar = Vec::new();
                    self.insider_trades = Vec::new();
                    // If an overlay-chain fetch was in-flight for the old symbol,
                    // the OverlayChainData handler checks chart.symbol == *symbol,
                    // so it will silently drop the result. Clear the loading flag
                    // now so the auto-fetch condition can trigger for the new symbol.
                    if self.overlay_chain_loading {
                        self.overlay_chain_loading = false;
                    }
                    self.overlay_calls.clear();
                    self.overlay_puts.clear();
                }
                // Futures live-bars feed (/ws/futures): re-target on any symbol
                // OR timeframe load (futures bars are tf-specific, and tf changes
                // don't trip is_new_symbol). Idempotent — no-op if unchanged.
                if self.symbol.starts_with("F:") {
                    crate::data::futures_feed::set_target(&self.symbol, &self.timeframe);
                }
                if !self.drawings_requested {
                    self.drawings_requested = true;
                    fetch_drawings_background(drawing_persist_key(self));
                }

                // Fetch signal drawings for new symbol
                self.signal_drawings.clear();
                self.last_signal_fetch = std::time::Instant::now();
                fetch_signal_drawings(self.symbol.clone());
                fetch_apexsignals_drawings(self.symbol.clone(), self.timeframe.clone()); // initial auto-chart paint
                crate::data::drawings_feed::set_target(&self.symbol, &self.timeframe); // live-feed re-pull target

                // Reload cross-timeframe indicator sources for new symbol
                for ind in &mut self.indicators {
                    if !ind.source_tf.is_empty() {
                        ind.source_loaded = false;
                        ind.source_bars.clear();
                        ind.source_timestamps.clear();
                        fetch_indicator_source(self.symbol.clone(), ind.source_tf.clone(), ind.id);
                    }
                }
            }
            ChartCommand::BarsUnavailable { symbol, timeframe, reason } => {
                // Only act if this is still the pane's current selection and we
                // have nothing to show — don't blank a chart that already has bars
                // just because a background refresh failed.
                if self.symbol == symbol && self.timeframe == timeframe {
                    self.history_loading = false;
                    if self.bars.is_empty() {
                        self.load_error = Some(reason);
                    }
                }
            }
            ChartCommand::PrependBars { symbol, timeframe, bars, timestamps } => {
                self.history_loading = false;
                if symbol == self.symbol && timeframe == self.timeframe {
                    if bars.is_empty() {
                        // No data returned — no more history available
                        self.history_exhausted = true;
                        eprintln!("[history] exhausted for {} {}", symbol, timeframe);
                    } else {
                        // Deduplicate: only keep bars older than our earliest
                        let earliest_existing = self.timestamps.first().copied().unwrap_or(i64::MAX);
                        let new_count = timestamps.iter().take_while(|&&t| t < earliest_existing).count();
                        if new_count == 0 {
                            self.history_exhausted = true;
                            eprintln!("[history] no new unique bars for {} {} — exhausted", symbol, timeframe);
                        } else {
                            let mut new_bars: Vec<Bar> = bars[..new_count].to_vec();
                            let mut new_ts: Vec<i64> = timestamps[..new_count].to_vec();
                            new_bars.append(&mut self.bars);
                            new_ts.append(&mut self.timestamps);
                            self.bars = new_bars;
                            self.timestamps = new_ts;
                            self.vs += new_count as f32;
                            self.indicator_bar_count = 0;
                            self.vol_analytics_computed = 0;
                            eprintln!("[history] prepended {} bars for {} {} (total: {})", new_count, symbol, timeframe, self.bars.len());
                        }
                    }
                }
            }
            ChartCommand::CacheBars { symbol, timeframe, bars, timestamps } => {
                if !bars.is_empty() {
                    let key = (symbol, timeframe);
                    if !self.tab_cache.contains_key(&key) {
                        evict_oldest_if_full(&mut self.tab_cache);
                        self.tab_cache.insert(key, (bars, timestamps, std::time::Instant::now()));
                    }
                }
            }
            ChartCommand::AppendBar { symbol, timeframe, bar, timestamp, mark } => {
                // MARK_BARS_PROTOCOL: drop frames whose source doesn't match the pane's
                // current selection (race window between toggle and server stop).
                // Only meaningful for option panes; stock panes always run in Last mode.
                if self.is_option && mark != self.bar_source_mark { return; }
                // Only append if both symbol AND timeframe match this pane
                if symbol == self.symbol && timeframe == self.timeframe {
                    // Dedupe: a cumulative feed (ApexData) may already have
                    // created this minute's bar via UpdateLastBar. If the last
                    // bar is the same minute, finalize it in place instead of
                    // pushing a duplicate.
                    if self.timestamps.last() == Some(&timestamp) {
                        if let Some(l) = self.bars.last_mut() { *l = bar; }
                        self.sim_price = bar.close;
                    } else {
                        self.bars.push(bar); self.timestamps.push(timestamp);
                        // Cap live bar growth to avoid unbounded memory accumulation.
                        const MAX_LIVE_BARS: usize = 50_000;
                        if self.bars.len() > MAX_LIVE_BARS {
                            let excess = self.bars.len() - MAX_LIVE_BARS;
                            self.bars.drain(..excess);
                            self.timestamps.drain(..excess);
                        }
                        // Smooth advance: increment vs by 1 instead of snapping, so if auto_scroll
                        // re-engages from a slight offset, the view continues from that position
                        if self.auto_scroll { self.vs += 1.0; }
                    }
                }
            }
            ChartCommand::UpdateLastBar { symbol, timeframe, bar, timestamp, mark, cumulative } => {
                if self.is_option && mark != self.bar_source_mark { return; }
                if symbol != self.symbol || !(timeframe.is_empty() || timeframe == self.timeframe) {
                    // not for this pane
                } else if cumulative {
                    // ApexData: `bar` is the full current-minute aggregate. Upsert
                    // by minute timestamp so the building candle is its OWN bar
                    // with the server's true high/low and cumulative volume —
                    // not folded into the previous (closed) bar.
                    let new_minute = self.timestamps.last().map_or(true, |&lt| timestamp > lt);
                    if new_minute {
                        self.bars.push(bar); self.timestamps.push(timestamp);
                        const MAX_LIVE_BARS: usize = 50_000;
                        if self.bars.len() > MAX_LIVE_BARS {
                            let excess = self.bars.len() - MAX_LIVE_BARS;
                            self.bars.drain(..excess);
                            self.timestamps.drain(..excess);
                        }
                        if self.auto_scroll { self.vs += 1.0; }
                    } else if let Some(l) = self.bars.last_mut() {
                        // ApexData sends the FULL current-minute aggregate every
                        // frame, so TRUST its high/low (replace) rather than
                        // min/max-accumulating. Min/max made any single transient
                        // bad frame (e.g. a spurious low) permanent — the bar then
                        // rendered an extreme wick that never recovered even after
                        // good frames arrived (the long-wick "comb" bug). Guard
                        // against non-positive prices (invalid) by keeping the
                        // prior value.
                        if bar.high > 0.0 { l.high = bar.high; }
                        if bar.low  > 0.0 { l.low  = bar.low;  }
                        l.close = bar.close;
                        l.volume = bar.volume; // cumulative — replace, don't add
                        // Keep OHLC self-consistent: the wick must bracket the body.
                        let body_hi = l.open.max(l.close);
                        let body_lo = l.open.min(l.close);
                        if l.high < body_hi { l.high = body_hi; }
                        if l.low  > body_lo { l.low  = body_lo; }
                    }
                    self.sim_price = bar.close;
                } else if let Some(l) = self.bars.last_mut() {
                    // IB / crypto: incremental tick — fold into the last bar.
                    l.close = bar.close;
                    l.high = l.high.max(bar.close);
                    l.low = l.low.min(bar.close);
                    l.volume += bar.volume;
                    self.sim_price = bar.close;
                }
            }
            ChartCommand::SetDrawing(d) => { self.drawings.retain(|x| x.id != d.id); self.drawings.push(d); }
            ChartCommand::RemoveDrawing { id } => { self.drawings.retain(|x| x.id != id); }
            ChartCommand::ClearDrawings => { self.drawings.clear(); }
            ChartCommand::LoadDrawings { symbol, drawings, groups } => {
                if symbol == self.symbol {
                    // Merge: keep locally-created drawings not yet in DB result
                    let db_ids: std::collections::HashSet<String> = drawings.iter().map(|d| d.id.clone()).collect();
                    let local_extras: Vec<Drawing> = self.drawings.iter()
                        .filter(|d| !db_ids.contains(&d.id))
                        .cloned().collect();
                    self.drawings = drawings;
                    self.drawings.extend(local_extras);
                    self.groups = groups.into_iter().map(|g| super::DrawingGroup { id: g.id, name: g.name, color: g.color }).collect();
                }
            }
            ChartCommand::SignalDrawings { symbol, drawings_json } => {
                if symbol == self.symbol {
                    // Parse signal drawings from JSON
                    if let Ok(annotations) = serde_json::from_str::<Vec<serde_json::Value>>(&drawings_json) {
                        let source = "signal".to_string();
                        self.signal_drawings.retain(|d| d.source != source);
                        for a in &annotations {
                            let id = a.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let dtype = a.get("type").and_then(|v| v.as_str()).unwrap_or("trendline").to_string();
                            let points: Vec<(i64, f32)> = a.get("points").and_then(|v| v.as_array()).map(|arr| {
                                arr.iter().filter_map(|p| Some((p.get("time")?.as_i64()?, p.get("price")?.as_f64()? as f32))).collect()
                            }).unwrap_or_default();
                            let style = a.get("style");
                            let color = style.and_then(|s| s.get("color")).and_then(|c| c.as_str()).unwrap_or("#4a9eff").to_string();
                            let opacity = style.and_then(|s| s.get("opacity")).and_then(|o| o.as_f64()).unwrap_or(0.7) as f32;
                            let thickness = style.and_then(|s| s.get("thickness")).and_then(|t| t.as_f64()).unwrap_or(1.0) as f32;
                            let ls = match style.and_then(|s| s.get("lineStyle")).and_then(|l| l.as_str()).unwrap_or("dashed") {
                                "solid" => LineStyle::Solid, "dotted" => LineStyle::Dotted, _ => LineStyle::Dashed,
                            };
                            let strength = a.get("strength").and_then(|s| s.as_f64()).unwrap_or(0.5) as f32;
                            let tf = a.get("timeframe").and_then(|t| t.as_str()).unwrap_or("5m").to_string();
                            let detection_method = a.get("detection_method").and_then(|m| m.as_str()).unwrap_or("").to_string();
                            self.signal_drawings.push(SignalDrawing { id, symbol: symbol.clone(), drawing_type: dtype, points, color, opacity, thickness, line_style: ls, strength, timeframe: tf, detection_method, source: source.clone(), extend_left: a.get("extendLeft").and_then(|v| v.as_bool()).unwrap_or(false), extend_right: a.get("extendRight").and_then(|v| v.as_bool()).unwrap_or(false) });
                        }
                    }
                }
            }
            ChartCommand::IndicatorSourceBars { indicator_id, timeframe, bars, timestamps } => {
                if let Some(ind) = self.indicators.iter_mut().find(|i| i.id == indicator_id && i.source_tf == timeframe) {
                    ind.source_bars = bars;
                    ind.source_timestamps = timestamps;
                    ind.source_loaded = true;
                    self.indicator_bar_count = 0; // force recompute
                }
            }
            ChartCommand::OverlayBars { symbol, bars, timestamps } => {
                eprintln!("[overlay] Received {} bars for '{}', overlays: {:?}", bars.len(), symbol,
                    self.symbol_overlays.iter().map(|o| o.symbol.as_str()).collect::<Vec<_>>());
                if let Some(ov) = self.symbol_overlays.iter_mut().find(|o| o.symbol == symbol) {
                    ov.bars = bars;
                    ov.timestamps = timestamps;
                    ov.loading = false;
                    eprintln!("[overlay] Loaded {} bars for {}", ov.bars.len(), ov.symbol);
                }
            }
            ChartCommand::EventData { symbol, events } => {
                if symbol == self.symbol {
                    self.event_markers = events.into_iter().map(|(ts, etype, label, details, impact)| {
                        let event_type = match etype.as_str() {
                            "earnings" => 0, "dividend" => 1, "split" => 2, "economic" => 3, _ => 0,
                        };
                        EventMarker { time: ts, event_type, label, details, impact }
                    }).collect();
                }
            }
            ChartCommand::DarkPoolData { symbol, prints } => {
                if symbol == self.symbol {
                    self.darkpool_prints = prints.into_iter().map(|(price, size, time, side)| {
                        DarkPoolPrint { price, size, time, side }
                    }).collect();
                }
            }
            ChartCommand::PatternLabels { symbol, labels } => {
                if symbol == self.symbol {
                    self.pattern_labels = labels;
                }
            }
            ChartCommand::DomLevels { symbol, levels } => {
                if symbol == self.symbol {
                    self.dom.levels = levels;
                    self.dom.last_live_ms = crate::data::dom_feed::now_ms();
                }
            }
            ChartCommand::AlertTriggered { symbol: _, alert_id: _, price, message } => {
                // Push a toast notification regardless of active symbol — alerts are always relevant
                crate::chart_renderer::ui::tools::notification::push_pending(
                    crate::chart_renderer::ui::tools::notification::Notification::new(message, crate::chart_renderer::ui::tools::notification::NotificationSeverity::Warning).with_value(price).with_source("alerts")
                );
            }
            ChartCommand::AutoTrendlines { symbol, drawings_json, source } => {
                // Replaces only this source's drawings, so trendlines and chart
                // patterns (separate producers) coexist instead of clobbering.
                if symbol == self.symbol {
                    if let Ok(annotations) = serde_json::from_str::<Vec<serde_json::Value>>(&drawings_json) {
                        self.signal_drawings.retain(|d| d.source != source);
                        for a in &annotations {
                            let id = a.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let dtype = a.get("type").and_then(|v| v.as_str()).unwrap_or("trendline").to_string();
                            let points: Vec<(i64, f32)> = a.get("points").and_then(|v| v.as_array()).map(|arr| {
                                arr.iter().filter_map(|p| Some((p.get("time")?.as_i64()?, p.get("price")?.as_f64()? as f32))).collect()
                            }).unwrap_or_default();
                            let style = a.get("style");
                            let color = style.and_then(|s| s.get("color")).and_then(|c| c.as_str()).unwrap_or("#4a9eff").to_string();
                            let opacity = style.and_then(|s| s.get("opacity")).and_then(|o| o.as_f64()).unwrap_or(0.7) as f32;
                            let thickness = style.and_then(|s| s.get("thickness")).and_then(|t| t.as_f64()).unwrap_or(1.0) as f32;
                            let ls = match style.and_then(|s| s.get("lineStyle")).and_then(|l| l.as_str()).unwrap_or("dashed") {
                                "solid" => LineStyle::Solid, "dotted" => LineStyle::Dotted, _ => LineStyle::Dashed,
                            };
                            let strength = a.get("strength").and_then(|s| s.as_f64()).unwrap_or(0.5) as f32;
                            let tf = a.get("timeframe").and_then(|t| t.as_str()).unwrap_or("5m").to_string();
                            let detection_method = a.get("detection_method").and_then(|m| m.as_str()).unwrap_or("").to_string();
                            self.signal_drawings.push(SignalDrawing { id, symbol: symbol.clone(), drawing_type: dtype, points, color, opacity, thickness, line_style: ls, strength, timeframe: tf, detection_method, source: source.clone(), extend_left: a.get("extendLeft").and_then(|v| v.as_bool()).unwrap_or(false), extend_right: a.get("extendRight").and_then(|v| v.as_bool()).unwrap_or(false) });
                        }
                        // Reset the HTTP polling timer so it doesn't immediately overwrite push data
                        self.last_signal_fetch = std::time::Instant::now();
                    }
                }
            }
            ChartCommand::SignificanceUpdate { symbol, drawing_id, score, touches, strength } => {
                if symbol == self.symbol {
                    for d in &mut self.drawings {
                        if d.id == drawing_id {
                            d.significance = Some(super::DrawingSignificance {
                                score, touches,
                                timeframe: String::new(),
                                age_days: 0,
                                volume_index: 1.0,
                                last_tested_bars: 0,
                                strength: strength.clone(),
                            });
                        }
                    }
                }
            }
            ChartCommand::TrendHealthUpdate { symbol, timeframe: _, score, direction, exhaustion_count: _, regime } => {
                if symbol == self.symbol {
                    self.trend_health_score = score;
                    self.trend_health_direction = direction;
                    self.trend_health_regime = regime;
                }
            }
            ChartCommand::ExitGaugeUpdate { symbol, score, urgency, components: _ } => {
                if symbol == self.symbol {
                    self.exit_gauge_score = score;
                    self.exit_gauge_urgency = urgency;
                }
            }
            ChartCommand::SupplyDemandZones { symbol, timeframe: _, zones } => {
                if symbol == self.symbol {
                    self.signal_zones = zones;
                }
            }
            ChartCommand::PrecursorAlert { symbol, score, direction, surge_ratio: _, lead_minutes, description } => {
                if symbol == self.symbol {
                    self.precursor_active = true;
                    self.precursor_score = score;
                    self.precursor_direction = direction;
                    self.precursor_description = description;
                    // Auto-toast
                    crate::chart_renderer::ui::tools::notification::push_pending(
                        crate::chart_renderer::ui::tools::notification::Notification::new(format!("PRECURSOR: {}", self.precursor_description), crate::chart_renderer::ui::tools::notification::NotificationSeverity::Warning).with_value(lead_minutes).with_source("precursor")
                    );
                }
            }
            ChartCommand::ChangePointMarker { symbol, time, change_type, confidence } => {
                if symbol == self.symbol {
                    self.change_points.push(ChangePoint { time, kind: change_type, confidence });
                    // Keep only last 20
                    if self.change_points.len() > 20 {
                        self.change_points.remove(0);
                    }
                }
            }
            ChartCommand::TradePlanUpdate { symbol, direction, entry_price, target_price, stop_price, contract_name, contract_entry: _, contract_target: _, risk_reward, conviction, summary } => {
                if symbol == self.symbol {
                    self.trade_plan = Some(TradePlan {
                        direction, entry: entry_price, target: target_price, stop: stop_price,
                        contract: contract_name, rr: risk_reward, conviction,
                    });
                    crate::chart_renderer::ui::tools::notification::push_pending(
                        crate::chart_renderer::ui::tools::notification::Notification::new(summary, crate::chart_renderer::ui::tools::notification::NotificationSeverity::Info).with_value(conviction).with_source("trade_plan")
                    );
                }
            }
            ChartCommand::DivergenceOverlay { symbol, timeframe, divergences } => {
                if symbol == self.symbol && timeframe == self.timeframe {
                    self.divergence_markers = divergences;
                }
            }
            _ => {}
        }
    }
    /// Recompute alternative bars (Renko, Range, Tick) from source OHLC data.
    fn recompute_alt_bars(&mut self) {
        if !matches!(self.candle_mode, CandleMode::Renko | CandleMode::RangeBar | CandleMode::TickBar) {
            return;
        }
        let (bars, ts) = match self.candle_mode {
            CandleMode::Renko => {
                let brick = if self.alt.renko_brick > 0.0 {
                    self.alt.renko_brick
                } else {
                    Self::auto_brick_size(&self.bars, 0.5)
                };
                Self::compute_renko_bars(&self.bars, &self.timestamps, brick)
            }
            CandleMode::RangeBar => {
                let range = if self.alt.range_size > 0.0 {
                    self.alt.range_size
                } else {
                    Self::auto_brick_size(&self.bars, 1.0)
                };
                Self::compute_range_bars(&self.bars, &self.timestamps, range)
            }
            CandleMode::TickBar => {
                Self::compute_tick_bars(&self.bars, &self.timestamps, self.alt.tick_count)
            }
            _ => return,
        };
        self.alt.bars = bars;
        self.alt.timestamps = ts;
        self.alt.dirty = false;
        self.alt.source_len = self.bars.len();
    }

    /// Auto-calculate brick/range size from ATR(14) * multiplier
    pub(crate) fn auto_brick_size(bars: &[Bar], multiplier: f32) -> f32 {
        if bars.len() < 16 { return 1.0; }
        let highs: Vec<f32> = bars.iter().map(|b| b.high).collect();
        let lows: Vec<f32> = bars.iter().map(|b| b.low).collect();
        let closes: Vec<f32> = bars.iter().map(|b| b.close).collect();
        let atr = compute_atr(&highs, &lows, &closes, 14);
        // Use the last valid ATR value
        let val = atr.iter().rev().find(|v| !v.is_nan()).copied().unwrap_or(1.0);
        (val * multiplier).max(0.01)
    }

    /// Build Renko bars from source OHLC data.
    fn compute_renko_bars(bars: &[Bar], timestamps: &[i64], brick_size: f32) -> (Vec<Bar>, Vec<i64>) {
        if bars.is_empty() || brick_size <= 0.0 { return (vec![], vec![]); }
        let mut out_bars: Vec<Bar> = Vec::new();
        let mut out_ts: Vec<i64> = Vec::new();
        let mut current_top = bars[0].close;
        let mut current_bot = bars[0].close;
        // Round to nearest brick boundary
        current_top = (current_top / brick_size).ceil() * brick_size;
        current_bot = current_top - brick_size;
        for (i, b) in bars.iter().enumerate() {
            let ts = timestamps.get(i).copied().unwrap_or(0);
            let price = b.close;
            // Up bricks
            while price >= current_top + brick_size {
                let new_bot = current_top;
                let new_top = new_bot + brick_size;
                out_bars.push(Bar {
                    open: new_bot, close: new_top, low: new_bot, high: new_top,
                    volume: b.volume, _pad: 0.0,
                });
                out_ts.push(ts);
                current_top = new_top;
                current_bot = new_bot;
            }
            // Down bricks
            while price <= current_bot - brick_size {
                let new_top = current_bot;
                let new_bot = new_top - brick_size;
                out_bars.push(Bar {
                    open: new_top, close: new_bot, low: new_bot, high: new_top,
                    volume: b.volume, _pad: 0.0,
                });
                out_ts.push(ts);
                current_top = new_top;
                current_bot = new_bot;
            }
        }
        (out_bars, out_ts)
    }

    /// Build Range bars from source OHLC data.
    fn compute_range_bars(bars: &[Bar], timestamps: &[i64], range_size: f32) -> (Vec<Bar>, Vec<i64>) {
        if bars.is_empty() || range_size <= 0.0 { return (vec![], vec![]); }
        let mut out_bars: Vec<Bar> = Vec::new();
        let mut out_ts: Vec<i64> = Vec::new();
        let mut cur_open = bars[0].open;
        let mut cur_high = bars[0].high;
        let mut cur_low = bars[0].low;
        let mut cur_close = bars[0].close;
        let mut cur_vol = 0.0_f32;
        let mut cur_ts = timestamps.first().copied().unwrap_or(0);
        for (i, b) in bars.iter().enumerate() {
            let ts = timestamps.get(i).copied().unwrap_or(0);
            // Simulate tick-by-tick using OHLC: process open, high, low, close in order
            let ticks = if b.close >= b.open {
                [b.open, b.low, b.high, b.close]
            } else {
                [b.open, b.high, b.low, b.close]
            };
            let tick_vol = b.volume / 4.0;
            for &tick in &ticks {
                cur_high = cur_high.max(tick);
                cur_low = cur_low.min(tick);
                cur_close = tick;
                cur_vol += tick_vol;
                // Check if range reached
                if cur_high - cur_low >= range_size {
                    out_bars.push(Bar {
                        open: cur_open, high: cur_high, low: cur_low, close: cur_close,
                        volume: cur_vol, _pad: 0.0,
                    });
                    out_ts.push(cur_ts);
                    // Start new bar
                    cur_open = cur_close;
                    cur_high = cur_close;
                    cur_low = cur_close;
                    cur_vol = 0.0;
                    cur_ts = ts;
                }
            }
            if i == 0 { cur_ts = ts; }
        }
        // Emit final partial bar if it has data
        if cur_vol > 0.0 || out_bars.is_empty() {
            out_bars.push(Bar {
                open: cur_open, high: cur_high, low: cur_low, close: cur_close,
                volume: cur_vol, _pad: 0.0,
            });
            out_ts.push(cur_ts);
        }
        (out_bars, out_ts)
    }

    /// Build Tick bars by splitting source OHLC bars based on volume proportions.
    fn compute_tick_bars(bars: &[Bar], timestamps: &[i64], tick_count: u32) -> (Vec<Bar>, Vec<i64>) {
        if bars.is_empty() || tick_count == 0 { return (vec![], vec![]); }
        let tick_count = tick_count.max(1) as f32;
        let mut out_bars: Vec<Bar> = Vec::new();
        let mut out_ts: Vec<i64> = Vec::new();
        let mut cur_open = bars[0].open;
        let mut cur_high = bars[0].high;
        let mut cur_low = bars[0].low;
        let mut cur_close = bars[0].close;
        let mut cur_vol = 0.0_f32;
        let mut cur_ts = timestamps.first().copied().unwrap_or(0);
        for (i, b) in bars.iter().enumerate() {
            let ts = timestamps.get(i).copied().unwrap_or(0);
            // Accumulate
            cur_high = cur_high.max(b.high);
            cur_low = cur_low.min(b.low);
            cur_close = b.close;
            cur_vol += b.volume;
            // Emit when accumulated volume >= tick_count
            while cur_vol >= tick_count {
                out_bars.push(Bar {
                    open: cur_open, high: cur_high, low: cur_low, close: cur_close,
                    volume: tick_count, _pad: 0.0,
                });
                out_ts.push(cur_ts);
                cur_vol -= tick_count;
                cur_open = cur_close;
                cur_high = cur_close;
                cur_low = cur_close;
                cur_ts = ts;
            }
            if i == 0 { cur_ts = ts; }
        }
        // Final partial bar
        if cur_vol > 0.0 || out_bars.is_empty() {
            out_bars.push(Bar {
                open: cur_open, high: cur_high, low: cur_low, close: cur_close,
                volume: cur_vol, _pad: 0.0,
            });
            out_ts.push(cur_ts);
        }
        (out_bars, out_ts)
    }

    /// Recompute all indicator values from bar data.
    fn recompute_indicators(&mut self) {
        let chart_closes: Vec<f32> = self.bars.iter().map(|b| b.close).collect();
        let chart_opens: Vec<f32> = self.bars.iter().map(|b| b.open).collect();
        let chart_highs: Vec<f32> = self.bars.iter().map(|b| b.high).collect();
        let chart_lows: Vec<f32> = self.bars.iter().map(|b| b.low).collect();
        let chart_volumes: Vec<f32> = self.bars.iter().map(|b| b.volume).collect();
        let chart_hl2: Vec<f32> = chart_highs.iter().zip(chart_lows.iter()).map(|(h, l)| (h + l) / 2.0).collect();
        let chart_ohlc4: Vec<f32> = self.bars.iter().map(|b| (b.open + b.high + b.low + b.close) / 4.0).collect();

        for ind in &mut self.indicators {
            // D1 fix: derive closes from the fetched source-timeframe bars when a
            // multi-timeframe source is configured and its bars have been loaded.
            // Materialise an owned Vec so later &mut ind borrows have no conflict.
            let source_closes_owned: Option<Vec<f32>> =
                if !ind.source_tf.is_empty() && ind.source_loaded && !ind.source_bars.is_empty() {
                    Some(ind.source_bars.iter().map(|b| b.close).collect())
                } else {
                    None
                };

            let skip = !ind.source_tf.is_empty() && !(ind.source_loaded && !ind.source_bars.is_empty());
            if skip {
                ind.values = vec![f32::NAN; self.bars.len()];
                ind.values2 = vec![]; ind.values3 = vec![]; ind.values4 = vec![]; ind.values5 = vec![];
                ind.histogram = vec![];
                continue;
            }

            let base_source: &Vec<f32> = source_closes_owned.as_ref().unwrap_or(&chart_closes);

            // Select source based on ind.source
            let closes = match ind.source {
                1 => &chart_opens,
                2 => &chart_highs,
                3 => &chart_lows,
                4 => &chart_hl2,
                5 => &chart_ohlc4,
                _ => base_source,
            };

            match ind.kind {
                IndicatorType::VWAP => {
                    ind.values = compute_vwap(closes, &chart_volumes, &chart_highs, &chart_lows, &self.timestamps);
                }
                IndicatorType::RSI => {
                    ind.values = compute_rsi(closes, ind.period);
                    ind.divergences = detect_divergences(closes, &ind.values, 5);
                }
                IndicatorType::MACD => {
                    let fast = ind.period;
                    let slow = if ind.param2 > 0.0 { ind.param2 as usize } else { 26 };
                    let signal = if ind.param3 > 0.0 { ind.param3 as usize } else { 9 };
                    let (macd, sig, hist) = compute_macd(closes, fast, slow, signal);
                    ind.values = macd;
                    ind.values2 = sig;
                    ind.histogram = hist;
                    ind.divergences = detect_divergences(closes, &ind.values, 5);
                }
                IndicatorType::Stochastic => {
                    let d_period = if ind.param2 > 0.0 { ind.param2 as usize } else { 3 };
                    let (k, d) = compute_stochastic(&chart_highs, &chart_lows, closes, ind.period.max(2), d_period);
                    ind.values = k;
                    ind.values2 = d;
                    ind.divergences = detect_divergences(closes, &ind.values, 5);
                }
                IndicatorType::ADX => {
                    let (adx, plus_di, minus_di) = compute_adx(&chart_highs, &chart_lows, &closes, ind.period);
                    ind.values = adx;
                    ind.values2 = plus_di;   // +DI line
                    ind.values3 = minus_di;  // -DI line
                    ind.histogram = vec![];
                }
                IndicatorType::CCI => {
                    ind.values = compute_cci(&chart_highs, &chart_lows, &closes, ind.period);
                    ind.values2 = vec![]; ind.histogram = vec![];
                }
                IndicatorType::WilliamsR => {
                    ind.values = compute_williams_r(&chart_highs, &chart_lows, &closes, ind.period);
                    ind.values2 = vec![]; ind.histogram = vec![];
                }
                IndicatorType::BollingerBands => {
                    let std_dev = if ind.param2 > 0.0 { ind.param2 } else { 2.0 };
                    let (mid, upper, lower) = compute_bollinger(closes, ind.period, std_dev);
                    ind.values = mid;
                    ind.values2 = upper;
                    ind.values3 = lower;
                    ind.values4 = vec![]; ind.values5 = vec![];
                    ind.histogram = vec![];
                }
                IndicatorType::Ichimoku => {
                    let tenkan = ind.period;
                    let kijun = if ind.param2 > 0.0 { ind.param2 as usize } else { 26 };
                    let senkou_b = if ind.param3 > 0.0 { ind.param3 as usize } else { 52 };
                    let (tenkan_v, kijun_v, sa, sb, chikou) = compute_ichimoku(&chart_highs, &chart_lows, closes, tenkan, kijun, senkou_b);
                    ind.values = tenkan_v;
                    ind.values2 = kijun_v;
                    ind.values3 = sa;
                    ind.values4 = sb;
                    ind.values5 = chikou;
                    ind.histogram = vec![];
                }
                IndicatorType::ParabolicSAR => {
                    let af_start = if ind.param4 > 0.0 { ind.param4 } else { 0.02 };
                    let af_step = if ind.param2 > 0.0 { ind.param2 } else { 0.02 };
                    let af_max = if ind.param3 > 0.0 { ind.param3 } else { 0.2 };
                    ind.values = compute_psar(&chart_highs, &chart_lows, af_start, af_step, af_max);
                    ind.values2 = vec![]; ind.values3 = vec![]; ind.values4 = vec![]; ind.values5 = vec![];
                    ind.histogram = vec![];
                }
                IndicatorType::Supertrend => {
                    let mult = if ind.param2 > 0.0 { ind.param2 } else { 3.0 };
                    let (st, dir) = compute_supertrend(&chart_highs, &chart_lows, closes, ind.period, mult);
                    ind.values = st;
                    ind.supertrend_dir = dir;
                    ind.values2 = vec![]; ind.values3 = vec![]; ind.values4 = vec![]; ind.values5 = vec![];
                    ind.histogram = vec![];
                }
                IndicatorType::KeltnerChannels => {
                    let mult = if ind.param2 > 0.0 { ind.param2 } else { 2.0 };
                    let (mid, upper, lower) = compute_keltner(&chart_highs, &chart_lows, closes, ind.period, mult);
                    ind.values = mid;
                    ind.values2 = upper;
                    ind.values3 = lower;
                    ind.values4 = vec![]; ind.values5 = vec![];
                    ind.histogram = vec![];
                }
                IndicatorType::ATR => {
                    ind.values = compute_atr(&chart_highs, &chart_lows, closes, ind.period);
                    ind.values2 = vec![]; ind.values3 = vec![]; ind.values4 = vec![]; ind.values5 = vec![];
                    ind.histogram = vec![];
                }
                IndicatorType::OBV => {
                    // OBV needs aligned close+volume — always from the chart bars
                    // (cross-TF volume isn't materialised here), so use chart_closes.
                    ind.values = compute_obv(&chart_closes, &chart_volumes);
                    ind.values2 = vec![]; ind.values3 = vec![]; ind.values4 = vec![]; ind.values5 = vec![];
                    ind.histogram = vec![];
                }
                _ => {
                    ind.values = ind.kind.compute(closes, ind.period);
                    ind.values2 = vec![];
                    ind.values3 = vec![]; ind.values4 = vec![]; ind.values5 = vec![];
                    ind.histogram = vec![];
                }
            }
        }
        self.indicator_bar_count = self.bars.len();
    }

    /// Update indicators — full recompute on data load or config change,
    /// incremental for single-bar appends (simulation).
    fn update_indicators(&mut self) {
        let n = self.bars.len();
        if n == self.indicator_bar_count { return; }

        // Full recompute needed
        if self.indicator_bar_count == 0 || n < self.indicator_bar_count || (n - self.indicator_bar_count) > 5 {
            self.recompute_indicators();
            return;
        }

        // Incremental: extend each indicator for newly added bars
        let old = self.indicator_bar_count;
        self.indicator_bar_count = n;
        for idx in old..n {
            let close = self.bars[idx].close;
            for ind in &mut self.indicators {
                match ind.kind {
                    IndicatorType::SMA | IndicatorType::WMA => {
                        if idx >= ind.period {
                            if ind.kind == IndicatorType::SMA {
                                let sum: f32 = self.bars[idx+1-ind.period..=idx].iter().map(|b| b.close).sum();
                                ind.values.push(sum / ind.period as f32);
                            } else {
                                let denom = (ind.period * (ind.period + 1)) / 2;
                                let mut s = 0.0;
                                for j in 0..ind.period { s += self.bars[idx + 1 - ind.period + j].close * (j + 1) as f32; }
                                ind.values.push(s / denom as f32);
                            }
                        } else { ind.values.push(f32::NAN); }
                    }
                    IndicatorType::EMA => {
                        let k = 2.0 / (ind.period as f32 + 1.0);
                        let prev = ind.values.last().copied().unwrap_or(f32::NAN);
                        let v = if prev.is_nan() {
                            if idx >= ind.period - 1 {
                                self.bars[idx+1-ind.period..=idx].iter().map(|b| b.close).sum::<f32>() / ind.period as f32
                            } else { f32::NAN }
                        } else { close * k + prev * (1.0 - k) };
                        ind.values.push(v);
                    }
                    _ => {
                        // DEMA, TEMA, VWAP, RSI, MACD, Stochastic — need full recompute
                        ind.values.push(f32::NAN);
                    }
                }
            }
        }
    }
    pub(crate) fn price_range(&self) -> (f32,f32) {
        if let Some(r) = self.price_lock { return r; }
        // Freeze range while actively drawing so new bars don't rescale the Y-axis mid-draw
        if let Some(r) = self.draw_price_freeze { return r; }
        // Use alt_bars for alternative chart types
        let bars_ref = if matches!(self.candle_mode, CandleMode::Renko | CandleMode::RangeBar | CandleMode::TickBar) && !self.alt.bars.is_empty() {
            &self.alt.bars
        } else {
            &self.bars
        };
        let s = self.vs as u32; let e = (s+self.vc).min(bars_ref.len() as u32);
        let (mut lo,mut hi) = (f32::MAX,f32::MIN);
        for i in s..e { if let Some(b) = bars_ref.get(i as usize) { lo=lo.min(b.low); hi=hi.max(b.high); } }
        // No bars in the visible window (empty chart or all snap requests 404'd) —
        // return a safe flat range so downstream .clamp() never sees NaN.
        if lo == f32::MAX { return (0.0, 1.0); }
        if lo>=hi { lo-=0.5; hi+=0.5; }
        let p=(hi-lo)*0.05; (lo-p,hi+p)
    }
}

// ─── egui rendering ──────────────────────────────────────────────────────────

/// Run one tick of price simulation for a single pane.
pub(crate) fn new_uuid() -> String { uuid::Uuid::new_v4().to_string() }

/// Promote the currently-visible auto signal-drawings into persistent, editable
/// drawings — saved to the drawings DB and undoable, exactly like hand-drawn
/// lines. Respects the BY-METHOD / signals visibility filters so only what you
/// see gets pinned. Pinned drawings land in the `auto-chart` group, labelled by
/// detection method. Returns how many were pinned.
pub(crate) fn pin_signal_drawings(chart: &mut Chart) -> usize {
    if chart.hide_signal_drawings {
        return 0;
    }
    // Collect geometry first so we don't hold an immutable borrow while mutating.
    let prepared: Vec<(DrawingKind, String, String)> = chart
        .signal_drawings
        .iter()
        .filter(|sd| !chart.hidden_signal_methods.iter().any(|m| m == &sd.detection_method))
        .filter_map(|sd| {
            let kind = match sd.drawing_type.as_str() {
                "trendline" if sd.points.len() >= 2 => DrawingKind::TrendLine {
                    price0: sd.points[0].1,
                    time0: sd.points[0].0,
                    price1: sd.points[1].1,
                    time1: sd.points[1].0,
                },
                "hline" if !sd.points.is_empty() => DrawingKind::HLine { price: sd.points[0].1 },
                _ => return None,
            };
            Some((kind, sd.color.clone(), sd.detection_method.clone()))
        })
        .collect();

    let sym = chart.symbol.clone();
    let tf = chart.timeframe.clone();
    let mut pinned = 0;
    for (kind, color, method) in prepared {
        let mut d = Drawing::new(new_uuid(), kind);
        d.color = color;
        d.group_id = "auto-chart".into();
        if !method.is_empty() {
            d.label = Some(method);
        }
        crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
        if chart.undo_stack.len() >= 50 {
            chart.undo_stack.remove(0);
        }
        chart.undo_stack.push(DrawingAction::Add(d.clone()));
        chart.drawings.push(d);
        pinned += 1;
    }
    if pinned > 0 {
        chart.redo_stack.clear();
    }
    pinned
}

/// Undo/redo action for drawing operations.
#[derive(Clone)]
pub(crate) enum DrawingAction {
    Add(Drawing),
    Remove(Drawing),
    Modify(String, Drawing), // (id, old_state)
}

/// Shift all timestamp fields in a DrawingKind by dt seconds.
pub(crate) fn shift_drawing_time(kind: &mut DrawingKind, dt: i64) {
    match kind {
        DrawingKind::TrendLine { time0, time1, .. } | DrawingKind::Ray { time0, time1, .. }
        | DrawingKind::Fibonacci { time0, time1, .. } | DrawingKind::Channel { time0, time1, .. }
        | DrawingKind::FibChannel { time0, time1, .. } | DrawingKind::GannFan { time0, time1, .. }
        | DrawingKind::FibArc { time0, time1, .. } | DrawingKind::GannBox { time0, time1, .. }
        | DrawingKind::PriceRange { time0, time1, .. } => { *time0 += dt; *time1 += dt; }
        DrawingKind::Pitchfork { time0, time1, time2, .. }
        | DrawingKind::FibExtension { time0, time1, time2, .. } => { *time0 += dt; *time1 += dt; *time2 += dt; }
        DrawingKind::RegressionChannel { time0, time1 } => { *time0 += dt; *time1 += dt; }
        DrawingKind::XABCD { points } | DrawingKind::ElliottWave { points, .. } => {
            for (t, _) in points.iter_mut() { *t += dt; }
        }
        DrawingKind::AnchoredVWAP { time } | DrawingKind::VerticalLine { time }
        | DrawingKind::FibTimeZone { time } => { *time += dt; }
        DrawingKind::RiskReward { entry_time, .. } => { *entry_time += dt; }
        DrawingKind::BarMarker { time, .. } => { *time += dt; }
        DrawingKind::TextNote { time, .. } => { *time += dt; }
        DrawingKind::HLine { .. } | DrawingKind::HZone { .. } => {}
    }
}

/// Short human-readable name for a DrawingKind (used in undo/redo toasts).
pub(crate) fn drawing_kind_short(kind: &DrawingKind) -> &'static str {
    match kind {
        DrawingKind::HLine{..} => "HLine", DrawingKind::TrendLine{..} => "TrendLine",
        DrawingKind::Ray{..} => "Ray", DrawingKind::HZone{..} => "Zone",
        DrawingKind::Fibonacci{..} => "Fibonacci", DrawingKind::Channel{..} => "Channel",
        DrawingKind::FibChannel{..} => "FibChannel", DrawingKind::Pitchfork{..} => "Pitchfork",
        DrawingKind::GannFan{..} => "GannFan", DrawingKind::GannBox{..} => "GannBox",
        DrawingKind::RegressionChannel{..} => "Regression", DrawingKind::XABCD{..} => "XABCD",
        DrawingKind::ElliottWave{..} => "Elliott", DrawingKind::AnchoredVWAP{..} => "AVWAP",
        DrawingKind::PriceRange{..} => "PriceRange", DrawingKind::RiskReward{..} => "RiskReward",
        DrawingKind::BarMarker{..} => "Marker", DrawingKind::VerticalLine{..} => "VLine",
        DrawingKind::FibExtension{..} => "FibExt", DrawingKind::FibTimeZone{..} => "FibTime",
        DrawingKind::FibArc{..} => "FibArc", DrawingKind::TextNote{..} => "TextNote",
    }
}

fn draw_circle_rgba(buf: &mut Vec<u8>, size: u32, cx: f32, cy: f32, r: f32, sw: f32, color: [u8; 4]) {
    let inner_sq = (r - sw * 0.5).max(0.0).powi(2);
    let outer_sq = (r + sw * 0.5).powi(2);
    let x0 = ((cx - r - sw) as i32).max(0) as u32;
    let x1 = ((cx + r + sw) as i32 + 1).min(size as i32) as u32;
    let y0 = ((cy - r - sw) as i32).max(0) as u32;
    let y1 = ((cy + r + sw) as i32 + 1).min(size as i32) as u32;
    for y in y0..y1 {
        for x in x0..x1 {
            let dist_sq = (x as f32 - cx).powi(2) + (y as f32 - cy).powi(2);
            if dist_sq >= inner_sq && dist_sq <= outer_sq {
                let idx = ((y * size + x) * 4) as usize;
                if idx + 4 <= buf.len() {
                    buf[idx..idx + 4].copy_from_slice(&color);
                }
            }
        }
    }
}

fn draw_logo_icon(buf: &mut Vec<u8>, size: u32, color: [u8; 4]) {
    let sc = size as f32 / 24.0;
    let sw = (size as f32 * 0.055).max(1.0);

    draw_circle_rgba(buf, size, 6.15852 * sc, 6.28967 * sc, (3.5638 * sc - sw * 0.5).max(1.0), sw, color);
    draw_circle_rgba(buf, size, 15.577  * sc, 15.7082 * sc, (5.85481 * sc - sw * 0.5).max(1.0), sw, color);

    let n = 8usize;
    let cb = |p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2]| -> Vec<(f32, f32)> {
        (0..=n).map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            (
                (u*u*u*p0[0]+3.0*u*u*t*p1[0]+3.0*u*t*t*p2[0]+t*t*t*p3[0]) * sc,
                (u*u*u*p0[1]+3.0*u*u*t*p1[1]+3.0*u*t*t*p2[1]+t*t*t*p3[1]) * sc,
            )
        }).collect()
    };
    let mut bpts: Vec<(f32, f32)> = Vec::with_capacity(80);
    bpts.extend(cb([13.6456,3.38161],[15.5131,1.51417],[18.5651,1.53812],[20.4625,3.43547]));
    bpts.extend(cb([20.4625,3.43547],[22.3595,5.33285],[22.3837,8.385],[20.5163,10.2525]));
    bpts.push((20.3209*sc, 10.4355*sc));
    bpts.push((20.4293*sc, 10.5439*sc));
    bpts.push((10.4567*sc, 20.5166*sc));
    bpts.extend(cb([10.4353,20.538],[8.52338,22.45],[5.42336,22.4505],[3.51134,20.5387]));
    bpts.extend(cb([3.51134,20.5387],[1.59935,18.6267],[1.59935,15.526],[3.51134,13.614]));
    bpts.push((13.1263*sc, 3.99895*sc));
    bpts.extend(cb([13.1263,3.99895],[13.2793,3.78238],[13.4519,3.57531],[13.6456,3.38161]));
    for w in bpts.windows(2) {
        draw_line_rgba(buf, size, w[0].0, w[0].1, w[1].0, w[1].1, sw, color);
    }
}

/// Generate a 32x32 RGBA window icon — Xolio logo in orange on transparent bg.
fn make_window_icon() -> Option<winit::window::Icon> {
    let s: u32 = 32;
    let mut rgba = vec![0u8; (s * s * 4) as usize];
    draw_logo_icon(&mut rgba, s, [254u8, 128, 25, 255]);
    winit::window::Icon::from_rgba(rgba, s, s).ok()
}

/// Create HICON in memory using CreateIconIndirect — no file needed.
#[cfg(target_os = "windows")]
fn make_window_icon_hicon() -> Option<isize> {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    // Bake the Apex triangle .ico into the binary at compile time and parse it
    // with CreateIconFromResourceEx. Avoids the .rc / tauri-build collision.
    const APEX_ICO: &[u8] = include_bytes!("../../../icons/apex-native.ico");
    unsafe {
        // Find the best 32x32 32-bit image inside the .ico directory
        let dir_id = LookupIconIdFromDirectoryEx(
            APEX_ICO.as_ptr(),
            1,                       // fIcon
            32, 32,                  // desired size
            LR_DEFAULTCOLOR,
        );
        if dir_id > 0 {
            let offset = dir_id as usize;
            if offset < APEX_ICO.len() {
                let hicon = CreateIconFromResourceEx(
                    APEX_ICO[offset..].as_ptr(),
                    (APEX_ICO.len() - offset) as u32,
                    1,               // fIcon
                    0x00030000,      // version
                    32, 32,
                    LR_DEFAULTCOLOR,
                );
                if !hicon.is_null() {
                    eprintln!("[native-chart] Loaded Apex .ico via CreateIconFromResourceEx");
                    return Some(hicon as isize);
                }
            }
        }
        eprintln!("[native-chart] .ico parse failed (dir_id={}) — falling back to procedural", dir_id);
    }
    use windows_sys::Win32::Graphics::Gdi::*;

    let s: i32 = 32;
    // Build BGRA pixel data (pre-multiplied alpha)
    let mut bgra = vec![0u8; (s * s * 4) as usize];
    draw_logo_icon(&mut bgra, s as u32, [25u8, 128, 254, 255]); // BGRA for orange #FE8019

    unsafe {
        // Create a DIB section for the color bitmap
        let hdc = GetDC(std::ptr::null_mut());
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = s;
        bmi.bmiHeader.biHeight = -(s); // top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = 0; // BI_RGB

        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let hbm_color = CreateDIBSection(hdc, &bmi, 0, &mut bits, std::ptr::null_mut(), 0);
        if !hbm_color.is_null() && !bits.is_null() {
            std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, bgra.len());
        }

        // Create monochrome mask (all zeros = fully opaque where color has alpha)
        let hbm_mask = CreateBitmap(s, s, 1, 1, std::ptr::null());

        let mut ii: ICONINFO = std::mem::zeroed();
        ii.fIcon = 1; // TRUE = icon
        ii.hbmMask = hbm_mask;
        ii.hbmColor = hbm_color;

        let hicon = CreateIconIndirect(&ii);

        // Cleanup bitmaps (icon keeps its own copy)
        if !hbm_color.is_null() { DeleteObject(hbm_color as _); }
        if !hbm_mask.is_null() { DeleteObject(hbm_mask as _); }
        ReleaseDC(std::ptr::null_mut(), hdc);

        if !hicon.is_null() {
            eprintln!("[native-chart] Icon created via CreateIconIndirect");
            Some(hicon as isize)
        } else {
            eprintln!("[native-chart] Warning: CreateIconIndirect failed");
            None
        }
    }
}

/// Stable persistence key for a chart pane. Equities and indexes use the
/// symbol as-is; option panes use a synthesized OCC contract id (which
/// doesn't change when the display label is re-formatted).
///
/// The display `symbol` for an option pane is a human-readable label like
/// "AAPL 287.5C 2026-04-30" that varies with strike formatting and expiry
/// rendering. The OCC ticker is built from the underlying, expiry
/// (YYYYMMDD), C/P flag, and strike*1000 zero-padded to 8 digits, prefixed
/// with "O:" — e.g. `O:AAPL260430C00287500` — which is invariant.
///
/// Note: pre-existing rows in the `drawings` table that were keyed by the
/// human-readable label will appear orphaned after this change. Migration
/// is intentionally skipped — option drawings are typically short-lived
/// (0DTE, weekly), so the orphan cost is low and a regex-based re-key
/// migration isn't worth the complexity. Equity/index drawings are
/// unaffected.
pub(crate) fn drawing_persist_key(chart: &Chart) -> String {
    if chart.is_option && !chart.underlying.is_empty() && !chart.option_expiry.is_empty() {
        // Expiry is stored as "YYYYMMDD"; OCC uses YYMMDD.
        let exp = &chart.option_expiry;
        let yymmdd = if exp.len() == 8 { &exp[2..] } else { exp.as_str() };
        let cp = if chart.option_type.eq_ignore_ascii_case("C") { 'C' } else { 'P' };
        let strike_milli = (chart.option_strike as f64 * 1000.0).round() as i64;
        format!("O:{}{}{}{:08}", chart.underlying, yymmdd, cp, strike_milli)
    } else {
        chart.symbol.clone()
    }
}

/// Convert a native Drawing to DbDrawing for persistence.
pub(crate) fn drawing_to_db(d: &Drawing, symbol: &str, timeframe: &str) -> crate::drawing_db::DbDrawing {
    let (drawing_type, points) = match &d.kind {
        DrawingKind::HLine { price } => ("hline".into(), vec![(0.0, *price as f64)]),
        DrawingKind::TrendLine { price0, time0, price1, time1 } => ("trendline".into(), vec![(*time0 as f64, *price0 as f64), (*time1 as f64, *price1 as f64)]),
        DrawingKind::HZone { price0, price1 } => ("hzone".into(), vec![(0.0, *price0 as f64), (0.0, *price1 as f64)]),
        DrawingKind::BarMarker { time, price, up } => ("barmarker".into(), vec![(*time as f64, *price as f64), (if *up { 1.0 } else { 0.0 }, 0.0)]),
        DrawingKind::Fibonacci { price0, time0, price1, time1 } => ("fibonacci".into(), vec![(*time0 as f64, *price0 as f64), (*time1 as f64, *price1 as f64)]),
        DrawingKind::Channel { price0, time0, price1, time1, offset } => ("channel".into(), vec![(*time0 as f64, *price0 as f64), (*time1 as f64, *price1 as f64), (*offset as f64, 0.0)]),
        DrawingKind::FibChannel { price0, time0, price1, time1, offset } => ("fibchannel".into(), vec![(*time0 as f64, *price0 as f64), (*time1 as f64, *price1 as f64), (*offset as f64, 0.0)]),
        DrawingKind::Pitchfork { price0, time0, price1, time1, price2, time2 } => ("pitchfork".into(), vec![(*time0 as f64, *price0 as f64), (*time1 as f64, *price1 as f64), (*time2 as f64, *price2 as f64)]),
        DrawingKind::GannFan { price0, time0, price1, time1 } => ("gannfan".into(), vec![(*time0 as f64, *price0 as f64), (*time1 as f64, *price1 as f64)]),
        DrawingKind::RegressionChannel { time0, time1 } => ("regression".into(), vec![(*time0 as f64, 0.0), (*time1 as f64, 0.0)]),
        DrawingKind::XABCD { points } => ("xabcd".into(), points.iter().map(|&(t, p)| (t as f64, p as f64)).collect()),
        DrawingKind::ElliottWave { points, wave_type } => {
            let mut pts: Vec<(f64, f64)> = points.iter().map(|&(t, p)| (t as f64, p as f64)).collect();
            pts.push((*wave_type as f64, 0.0));
            ("elliott".into(), pts)
        }
        DrawingKind::AnchoredVWAP { time } => ("avwap".into(), vec![(*time as f64, 0.0)]),
        DrawingKind::PriceRange { price0, time0, price1, time1 } => ("pricerange".into(), vec![(*time0 as f64, *price0 as f64), (*time1 as f64, *price1 as f64)]),
        DrawingKind::RiskReward { entry_price, entry_time, stop_price, target_price } => ("riskreward".into(), vec![(*entry_time as f64, *entry_price as f64), (0.0, *stop_price as f64), (0.0, *target_price as f64)]),
        DrawingKind::VerticalLine { time } => ("vline".into(), vec![(*time as f64, 0.0)]),
        DrawingKind::Ray { price0, time0, price1, time1 } => ("ray".into(), vec![(*time0 as f64, *price0 as f64), (*time1 as f64, *price1 as f64)]),
        DrawingKind::FibExtension { price0, time0, price1, time1, price2, time2 } => ("fibext".into(), vec![(*time0 as f64, *price0 as f64), (*time1 as f64, *price1 as f64), (*time2 as f64, *price2 as f64)]),
        DrawingKind::FibTimeZone { time } => ("fibtimezone".into(), vec![(*time as f64, 0.0)]),
        DrawingKind::FibArc { price0, time0, price1, time1 } => ("fibarc".into(), vec![(*time0 as f64, *price0 as f64), (*time1 as f64, *price1 as f64)]),
        DrawingKind::GannBox { price0, time0, price1, time1 } => ("gannbox".into(), vec![(*time0 as f64, *price0 as f64), (*time1 as f64, *price1 as f64)]),
        DrawingKind::TextNote { price, time, text, font_size } => {
            let mut pts = vec![(*time as f64, *price as f64), (*font_size as f64, text.len() as f64)];
            for ch in text.chars() { pts.push((ch as u32 as f64, 0.0)); }
            ("textnote".into(), pts)
        }
    };
    let ls = match d.line_style { LineStyle::Solid => "solid", LineStyle::Dashed => "dashed", LineStyle::Dotted => "dotted" };
    crate::drawing_db::DbDrawing {
        id: d.id.clone(), symbol: symbol.into(), timeframe: timeframe.into(),
        drawing_type, points, color: d.color.clone(), opacity: d.opacity,
        line_style: ls.into(), thickness: d.thickness, group_id: d.group_id.clone(),
    }
}

/// Convert a DbDrawing to native Drawing.
pub(crate) fn db_to_drawing(d: &crate::drawing_db::DbDrawing) -> Option<Drawing> {
    let kind = match d.drawing_type.as_str() {
        "hline" => DrawingKind::HLine { price: d.points.first()?.1 as f32 },
        "trendline" => {
            let p0 = d.points.get(0)?;
            let p1 = d.points.get(1)?;
            DrawingKind::TrendLine { time0: p0.0 as i64, price0: p0.1 as f32, time1: p1.0 as i64, price1: p1.1 as f32 }
        }
        "hzone" => DrawingKind::HZone { price0: d.points.get(0)?.1 as f32, price1: d.points.get(1)?.1 as f32 },
        "barmarker" => DrawingKind::BarMarker { time: d.points.get(0)?.0 as i64, price: d.points.get(0)?.1 as f32, up: d.points.get(1).map(|p| p.0 > 0.5).unwrap_or(true) },
        "fibonacci" => {
            let p0 = d.points.get(0)?; let p1 = d.points.get(1)?;
            DrawingKind::Fibonacci { time0: p0.0 as i64, price0: p0.1 as f32, time1: p1.0 as i64, price1: p1.1 as f32 }
        }
        "channel" => {
            let p0 = d.points.get(0)?; let p1 = d.points.get(1)?;
            let offset = d.points.get(2).map(|p| p.0 as f32).unwrap_or(0.0);
            DrawingKind::Channel { time0: p0.0 as i64, price0: p0.1 as f32, time1: p1.0 as i64, price1: p1.1 as f32, offset }
        }
        "fibchannel" => {
            let p0 = d.points.get(0)?; let p1 = d.points.get(1)?;
            let offset = d.points.get(2).map(|p| p.0 as f32).unwrap_or(0.0);
            DrawingKind::FibChannel { time0: p0.0 as i64, price0: p0.1 as f32, time1: p1.0 as i64, price1: p1.1 as f32, offset }
        }
        "pitchfork" => {
            let p0 = d.points.get(0)?; let p1 = d.points.get(1)?; let p2 = d.points.get(2)?;
            DrawingKind::Pitchfork { time0: p0.0 as i64, price0: p0.1 as f32, time1: p1.0 as i64, price1: p1.1 as f32, time2: p2.0 as i64, price2: p2.1 as f32 }
        }
        "gannfan" => {
            let p0 = d.points.get(0)?; let p1 = d.points.get(1)?;
            DrawingKind::GannFan { time0: p0.0 as i64, price0: p0.1 as f32, time1: p1.0 as i64, price1: p1.1 as f32 }
        }
        "regression" => {
            let p0 = d.points.get(0)?; let p1 = d.points.get(1)?;
            DrawingKind::RegressionChannel { time0: p0.0 as i64, time1: p1.0 as i64 }
        }
        "xabcd" => {
            if d.points.len() < 5 { return None; }
            DrawingKind::XABCD { points: d.points.iter().map(|&(t, p)| (t as i64, p as f32)).collect() }
        }
        "elliott" => {
            let wave_type = d.points.last().map(|p| p.0 as u8).unwrap_or(0);
            let pts_len = d.points.len().saturating_sub(1);
            DrawingKind::ElliottWave { points: d.points[..pts_len].iter().map(|&(t, p)| (t as i64, p as f32)).collect(), wave_type }
        }
        "avwap" => { let p0 = d.points.get(0)?; DrawingKind::AnchoredVWAP { time: p0.0 as i64 } }
        "pricerange" => {
            let p0 = d.points.get(0)?; let p1 = d.points.get(1)?;
            DrawingKind::PriceRange { time0: p0.0 as i64, price0: p0.1 as f32, time1: p1.0 as i64, price1: p1.1 as f32 }
        }
        "riskreward" => {
            let p0 = d.points.get(0)?; let p1 = d.points.get(1)?; let p2 = d.points.get(2)?;
            DrawingKind::RiskReward { entry_time: p0.0 as i64, entry_price: p0.1 as f32, stop_price: p1.1 as f32, target_price: p2.1 as f32 }
        }
        "vline" => { let p0 = d.points.get(0)?; DrawingKind::VerticalLine { time: p0.0 as i64 } }
        "ray" => {
            let p0 = d.points.get(0)?; let p1 = d.points.get(1)?;
            DrawingKind::Ray { time0: p0.0 as i64, price0: p0.1 as f32, time1: p1.0 as i64, price1: p1.1 as f32 }
        }
        "fibext" => {
            let p0 = d.points.get(0)?; let p1 = d.points.get(1)?; let p2 = d.points.get(2)?;
            DrawingKind::FibExtension { time0: p0.0 as i64, price0: p0.1 as f32, time1: p1.0 as i64, price1: p1.1 as f32, time2: p2.0 as i64, price2: p2.1 as f32 }
        }
        "fibtimezone" => { let p0 = d.points.get(0)?; DrawingKind::FibTimeZone { time: p0.0 as i64 } }
        "fibarc" => {
            let p0 = d.points.get(0)?; let p1 = d.points.get(1)?;
            DrawingKind::FibArc { time0: p0.0 as i64, price0: p0.1 as f32, time1: p1.0 as i64, price1: p1.1 as f32 }
        }
        "gannbox" => {
            let p0 = d.points.get(0)?; let p1 = d.points.get(1)?;
            DrawingKind::GannBox { time0: p0.0 as i64, price0: p0.1 as f32, time1: p1.0 as i64, price1: p1.1 as f32 }
        }
        "textnote" => {
            let p0 = d.points.get(0)?;
            let p1 = d.points.get(1)?;
            let font_size = p1.0 as f32;
            let text_len = p1.1 as usize;
            let text: String = d.points.iter().skip(2).take(text_len)
                .map(|p| char::from_u32(p.0 as u32).unwrap_or('?')).collect();
            DrawingKind::TextNote { time: p0.0 as i64, price: p0.1 as f32, text, font_size }
        }
        _ => return None,
    };
    let ls = match d.line_style.as_str() { "dashed" => LineStyle::Dashed, "dotted" => LineStyle::Dotted, _ => LineStyle::Solid };
    let mut drawing = Drawing::new(d.id.clone(), kind);
    drawing.color = d.color.clone();
    drawing.opacity = d.opacity;
    drawing.line_style = ls;
    drawing.thickness = d.thickness;
    drawing.group_id = d.group_id.clone();
    Some(drawing)
}

fn tick_simulation(chart: &mut Chart) {
    // Skip simulation for crypto — real data comes from ApexCrypto.
    // Wave 9c: registry-backed asset class via `symbol_meta` (avoids the
    // `is_crypto(&str)` suffix heuristic mis-flagging XUSDT-style equities).
    if chart.symbol_meta.is_crypto() { return; }
    // Skip simulation when ApexData is the active feed (Polygon-backed).
    // Off-hours we just want the chart to sit still; ticks/bars come from
    // WS Trade/Bar frames or not at all.
    if crate::apex_data::is_enabled() { return; }
    if !chart.bars.is_empty() {
        // Init sim_price from last bar's close — and immediately create a new
        // candle so the simulation never overwrites historical data.
        if chart.sim_price == 0.0 {
            chart.sim_price = chart.bars.last().map(|b| b.close).unwrap_or(100.0);
            chart.last_candle_time = std::time::Instant::now();
            // Create first sim candle so ticks don't touch real bars
            let last_ts = chart.timestamps.last().copied().unwrap_or(0);
            let interval = if chart.timestamps.len() > 1 {
                chart.timestamps[chart.timestamps.len()-1] - chart.timestamps[chart.timestamps.len()-2]
            } else { SIM_DEFAULT_INTERVAL };
            chart.bars.push(Bar {
                open: chart.sim_price, high: chart.sim_price, low: chart.sim_price,
                close: chart.sim_price, volume: 0.0, _pad: 0.0,
            });
            chart.timestamps.push(last_ts + interval);
        }

        chart.tick_counter += 1;

        let rng = |seed: &mut u64| -> f32 {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (*seed >> 33) as f32 / u32::MAX as f32
        };
        let r1 = rng(&mut chart.sim_seed);
        let r2 = rng(&mut chart.sim_seed);

        // Tick every ~5 frames (~12x/sec) — update last (simulated) bar
        if chart.tick_counter % SIM_TICK_FRAMES == 0 {
            let normal = (-2.0 * r1.max(0.0001).ln()).sqrt() * (2.0 * std::f32::consts::PI * r2).cos();
            let base_open = chart.bars.last().map(|b| b.open).unwrap_or(chart.sim_price);
            let reversion = (base_open - chart.sim_price) * SIM_REVERSION;
            let change = normal * chart.sim_price * SIM_VOLATILITY + reversion;
            chart.sim_price += change;
            let volume_tick = (r1 * SIM_VOL_RANGE + SIM_VOL_BASE) * (1.0 + normal.abs());

            if let Some(last) = chart.bars.last_mut() {
                last.close = chart.sim_price;
                last.high = last.high.max(chart.sim_price);
                last.low = last.low.min(chart.sim_price);
                last.volume += volume_tick;
            }
        }

        // New candle every ~3 seconds (cap at 10K bars to prevent unbounded growth)
        if chart.last_candle_time.elapsed().as_millis() >= SIM_CANDLE_MS && chart.bars.len() < 10_000 {
            chart.last_candle_time = std::time::Instant::now();
            let last_ts = chart.timestamps.last().copied().unwrap_or(0);
            let interval = if chart.timestamps.len() > 1 {
                chart.timestamps[chart.timestamps.len()-1] - chart.timestamps[chart.timestamps.len()-2]
            } else { SIM_DEFAULT_INTERVAL };
            chart.bars.push(Bar {
                open: chart.sim_price, high: chart.sim_price, low: chart.sim_price,
                close: chart.sim_price, volume: 0.0, _pad: 0.0,
            });
            chart.timestamps.push(last_ts + interval);
        }

        if chart.auto_scroll {
            chart.vs = chart.bars.len() as f32 - chart.vc as f32 + CHART_RIGHT_PAD as f32;
        }

    }

    // ── Draw-mode price freeze: lock Y-range while user is mid-stroke ──
    // Only freeze when actually placing points — NOT when a tool is merely selected.
    // Having a tool selected in the toolbar shouldn't block Y-axis auto-fit.
    let mid_stroke = chart.dragging_drawing.is_some()
        || chart.pending_pt.is_some()
        || chart.pending_pt2.is_some()
        || !chart.pending_pts.is_empty();
    if mid_stroke {
        if chart.draw_price_freeze.is_none() && chart.price_lock.is_none() {
            chart.draw_price_freeze = Some(chart.price_range());
        }
    } else if chart.draw_price_freeze.is_some() {
        chart.draw_price_freeze = None;
    }

    // ── Auto-scroll re-engagement rules ──
    // - User panned backward: when latest bar is within 20 bars of the visible right edge,
    //   smoothly re-engage auto_scroll (vs stays put, AppendBar advances it)
    // - User panned forward past latest (empty future in view): snap back after 5 seconds
    // - User zoomed in so latest went off-screen right: snap back after 5 seconds
    if !chart.auto_scroll && !chart.bars.is_empty() {
        let latest = chart.bars.len() as f32 - 1.0;
        let right_edge = chart.vs + chart.vc as f32;
        if latest < chart.vs || latest >= right_edge {
            // Latest bar not visible (panned forward past it OR zoomed in past it).
            // Snap back after inactivity.
            if chart.last_input.elapsed().as_secs() >= AUTO_SCROLL_RESUME_SECS {
                chart.auto_scroll = true;
                chart.price_lock = None;
                chart.vs = chart.bars.len() as f32 - chart.vc as f32 + CHART_RIGHT_PAD as f32;
            }
        } else if right_edge - latest <= 20.0 {
            // Latest is within 20 bars of the right edge — re-engage smoothly without snapping
            chart.auto_scroll = true;
        }
    }

    // ── Per-pane price alert checking ──
    if let Some(last_bar) = chart.bars.last() {
        let price = last_bar.close;
        for alert in &mut chart.price_alerts {
            if alert.triggered || alert.draft || alert.symbol != chart.symbol { continue; }
            if (alert.above && price >= alert.price) || (!alert.above && price <= alert.price) {
                alert.triggered = true;
                let dir = if alert.above { "above" } else { "below" };
                let msg = format!("{} alert: price {} {:.2}", chart.symbol, dir, alert.price);
                eprintln!("[ALERT TRIGGERED] {} -- sound notification placeholder", msg);
                crate::chart_renderer::ui::tools::notification::push_pending(
                    crate::chart_renderer::ui::tools::notification::Notification::new(msg, crate::chart_renderer::ui::tools::notification::NotificationSeverity::Warning).with_value(alert.price).with_source("price_alert")
                );
            }
        }
    }
}


/// Build a `VolumeProfileData` from real volume-at-price (ApexData VAP) instead
/// of the bar-spread approximation. Levels carry true per-price volume; POC/VA
/// are computed the same way. `buy_vol`/`sell_vol` come straight from VAP (0
/// until the backend adds the per-level split). `off_exchange` per level is
/// summed into the profile total for an (optional) dark-pool readout.
pub(crate) fn volume_profile_from_vap(v: &crate::apex_data::rest::VapResponse) -> Option<VolumeProfileData> {
    if v.levels.len() < 2 { return None; }
    let mut lv: Vec<&crate::apex_data::rest::VapLevel> = v.levels.iter().collect();
    lv.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal));
    let price_step = if lv.len() >= 2 { (lv[1].price - lv[0].price) as f32 } else { 0.01 };
    let levels: Vec<VolumeLevel> = lv.iter().map(|l| VolumeLevel {
        price: l.price as f32,
        total_vol: l.volume as f32,
        buy_vol: l.buy_volume as f32,
        sell_vol: l.sell_volume as f32,
        off_exchange: l.off_exchange_volume as f32,
    }).collect();
    let max_vol = levels.iter().map(|l| l.total_vol).fold(0.0_f32, f32::max);
    if max_vol <= 0.0 { return None; }
    let poc_idx = levels.iter().enumerate()
        .max_by(|a, b| a.1.total_vol.partial_cmp(&b.1.total_vol).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i).unwrap_or(0);
    let poc_price = levels[poc_idx].price;
    let total_vol: f32 = levels.iter().map(|l| l.total_vol).sum();
    let va_target = total_vol * 0.70;
    let mut va_vol = levels[poc_idx].total_vol;
    let (mut va_lo, mut va_hi) = (poc_idx, poc_idx);
    while va_vol < va_target && (va_lo > 0 || va_hi < levels.len() - 1) {
        let lo_vol = if va_lo > 0 { levels[va_lo - 1].total_vol } else { 0.0 };
        let hi_vol = if va_hi < levels.len() - 1 { levels[va_hi + 1].total_vol } else { 0.0 };
        if lo_vol >= hi_vol && va_lo > 0 { va_lo -= 1; va_vol += levels[va_lo].total_vol; }
        else if va_hi < levels.len() - 1 { va_hi += 1; va_vol += levels[va_hi].total_vol; }
        else { break; }
    }
    let val = levels[va_lo].price - price_step / 2.0;
    let vah = levels[va_hi].price + price_step / 2.0;
    Some(VolumeProfileData { levels, poc_price, vah, val, max_vol, price_step })
}

pub(crate) fn compute_volume_profile(bars: &[Bar], start: usize, end: usize, num_levels: usize) -> Option<VolumeProfileData> {
    if start >= end || end > bars.len() || num_levels < 2 { return None; }
    let mut min_price = f32::MAX;
    let mut max_price = f32::MIN;
    for b in &bars[start..end] { min_price = min_price.min(b.low); max_price = max_price.max(b.high); }
    if max_price <= min_price { return None; }
    let price_step = (max_price - min_price) / num_levels as f32;
    let mut levels: Vec<VolumeLevel> = (0..num_levels).map(|i| VolumeLevel {
        price: min_price + (i as f32 + 0.5) * price_step, total_vol: 0.0, buy_vol: 0.0, sell_vol: 0.0,
        off_exchange: 0.0, // bar-derived profile has no off-exchange breakdown
    }).collect();
    for b in &bars[start..end] {
        let bar_range = b.high - b.low;
        if bar_range <= 0.0 { continue; }
        let buy_ratio = (b.close - b.low) / bar_range;
        let sell_ratio = 1.0 - buy_ratio;
        let lo_idx = ((b.low - min_price) / price_step) as usize;
        let hi_idx = ((b.high - min_price) / price_step).ceil() as usize;
        let lo_idx = lo_idx.min(num_levels - 1);
        let hi_idx = hi_idx.min(num_levels);
        let span = (hi_idx - lo_idx).max(1) as f32;
        let vol_per_level = b.volume / span;
        for i in lo_idx..hi_idx {
            levels[i].total_vol += vol_per_level;
            levels[i].buy_vol += vol_per_level * buy_ratio;
            levels[i].sell_vol += vol_per_level * sell_ratio;
        }
    }
    let max_vol = levels.iter().map(|l| l.total_vol).fold(0.0_f32, f32::max);
    let poc_price = levels.iter().max_by(|a, b| a.total_vol.partial_cmp(&b.total_vol).unwrap_or(std::cmp::Ordering::Equal))
        .map(|l| l.price).unwrap_or(min_price);
    let total_vol: f32 = levels.iter().map(|l| l.total_vol).sum();
    let va_target = total_vol * 0.70;
    let poc_idx = levels.iter().position(|l| l.price == poc_price).unwrap_or(0);
    let mut va_vol = levels[poc_idx].total_vol;
    let mut va_lo = poc_idx;
    let mut va_hi = poc_idx;
    while va_vol < va_target && (va_lo > 0 || va_hi < levels.len() - 1) {
        let lo_vol = if va_lo > 0 { levels[va_lo - 1].total_vol } else { 0.0 };
        let hi_vol = if va_hi < levels.len() - 1 { levels[va_hi + 1].total_vol } else { 0.0 };
        if lo_vol >= hi_vol && va_lo > 0 { va_lo -= 1; va_vol += levels[va_lo].total_vol; }
        else if va_hi < levels.len() - 1 { va_hi += 1; va_vol += levels[va_hi].total_vol; }
        else { break; }
    }
    let val = levels[va_lo].price - price_step / 2.0;
    let vah = levels[va_hi].price + price_step / 2.0;
    Some(VolumeProfileData { levels, poc_price, vah, val, max_vol, price_step })
}

/// Compute micro volume profile for a single bar (levels within the bar's range).
/// Returns: Vec of (price, width_fraction, buy_ratio) for each level.
pub(crate) fn bar_micro_profile(bar: &Bar, levels: usize) -> Vec<(f32, f32, f32)> {
    let range = bar.high - bar.low;
    if range <= 0.0 || levels == 0 { return vec![(bar.close, 1.0, 0.5)]; }

    let step = range / levels as f32;
    let mut result = Vec::with_capacity(levels);

    // Heuristic: volume concentrates near the close price
    // Use a gaussian-like distribution centered on the close
    let close_pos = (bar.close - bar.low) / range; // 0-1 position of close
    let open_pos = (bar.open - bar.low) / range;   // 0-1 position of open

    let mut total_weight = 0.0_f32;
    let mut weights = Vec::with_capacity(levels);

    for i in 0..levels {
        let level_price = bar.low + (i as f32 + 0.5) * step;
        let level_pos = (level_price - bar.low) / range; // 0-1

        // Volume weight: gaussian centered on close, with wider spread near open
        let dist_to_close = (level_pos - close_pos).abs();
        let dist_to_open = (level_pos - open_pos).abs();
        let weight = (-dist_to_close * dist_to_close * 4.0).exp() * 0.7
            + (-dist_to_open * dist_to_open * 4.0).exp() * 0.3;
        weights.push(weight);
        total_weight += weight;
    }

    // Normalize weights and compute buy ratio per level
    for i in 0..levels {
        let level_price = bar.low + (i as f32 + 0.5) * step;
        let level_pos = (level_price - bar.low) / range;
        let width_frac = if total_weight > 0.0 { weights[i] / total_weight * levels as f32 } else { 1.0 };
        let width_frac = width_frac.clamp(0.2, 2.5);

        // Buy ratio varies within the bar:
        // Bullish: buying pressure increases toward the top
        // Bearish: selling pressure increases toward the top
        let is_bull = bar.close >= bar.open;
        let buy_ratio = if is_bull {
            0.3 + 0.5 * level_pos
        } else {
            0.7 - 0.5 * level_pos
        };

        result.push((level_price, width_frac, buy_ratio));
    }

    result
}

pub(crate) fn compute_volume_analytics(chart: &mut Chart) {
    let n = chart.bars.len();
    if n == 0 || chart.vol_analytics_computed == n { return; }

    chart.vwap_data.resize(n, f32::NAN);
    chart.vwap_upper1.resize(n, f32::NAN);
    chart.vwap_lower1.resize(n, f32::NAN);
    chart.vwap_upper2.resize(n, f32::NAN);
    chart.vwap_lower2.resize(n, f32::NAN);
    chart.cvd_data.resize(n, 0.0);
    chart.delta_data.resize(n, 0.0);
    chart.rvol_data.resize(n, 1.0);

    // Per-bar delta. Prefer REAL order-flow delta from the live trade stream
    // (server-side aggressor `side`, accrued per minute in live_state) for any
    // bar the session has streamed; fall back to the close-position heuristic
    // for historical bars (no per-trade history) and futures (no side feed).
    for i in 0..n {
        let b = &chart.bars[i];
        let bar_from = chart.timestamps.get(i).copied().unwrap_or(0);
        let bar_to = chart.timestamps.get(i + 1).copied().unwrap_or(bar_from + 60_000);
        let real = if bar_from > 0 {
            crate::apex_data::live_state::realized_delta_in(&chart.symbol, bar_from, bar_to)
        } else { None };
        chart.delta_data[i] = match real {
            Some(d) => d,
            None => {
                let range = b.high - b.low;
                if range > 0.0 {
                    let buy_ratio = (b.close - b.low) / range;
                    b.volume * buy_ratio - b.volume * (1.0 - buy_ratio)
                } else { 0.0 }
            }
        };
    }

    // CVD — cumulative sum of delta
    let mut cum = 0.0_f32;
    for i in 0..n {
        cum += chart.delta_data[i];
        chart.cvd_data[i] = cum;
    }

    // Session VWAP + σ bands (session boundary = gap > 4 hours between bars)
    let mut cum_tp_vol = 0.0_f64;
    let mut cum_vol = 0.0_f64;
    let mut cum_tp2_vol = 0.0_f64;
    for i in 0..n {
        let new_session = if i == 0 { true } else {
            let gap = chart.timestamps.get(i).unwrap_or(&0) - chart.timestamps.get(i-1).unwrap_or(&0);
            gap > 14400
        };
        if new_session {
            cum_tp_vol = 0.0;
            cum_vol = 0.0;
            cum_tp2_vol = 0.0;
        }
        let b = &chart.bars[i];
        let tp = ((b.high + b.low + b.close) / 3.0) as f64;
        let vol = b.volume as f64;
        cum_tp_vol += tp * vol;
        cum_vol += vol;
        cum_tp2_vol += tp * tp * vol;
        if cum_vol > 0.0 {
            let vwap = (cum_tp_vol / cum_vol) as f32;
            chart.vwap_data[i] = vwap;
            let mean_sq = cum_tp2_vol / cum_vol;
            let sq_mean = (cum_tp_vol / cum_vol).powi(2);
            let sigma = ((mean_sq - sq_mean).max(0.0)).sqrt() as f32;
            chart.vwap_upper1[i] = vwap + sigma;
            chart.vwap_lower1[i] = vwap - sigma;
            chart.vwap_upper2[i] = vwap + sigma * 2.0;
            chart.vwap_lower2[i] = vwap - sigma * 2.0;
        }
    }

    // RVOL — compare bar volume to 20-bar moving average
    let rvol_period = 20_usize;
    for i in 0..n {
        if i < rvol_period {
            chart.rvol_data[i] = 1.0;
        } else {
            let avg: f32 = chart.bars[i-rvol_period..i].iter().map(|b| b.volume).sum::<f32>() / rvol_period as f32;
            chart.rvol_data[i] = if avg > 0.0 { chart.bars[i].volume / avg } else { 1.0 };
        }
    }

    chart.vol_analytics_computed = n;
}

// ── draw_chart phase functions ──────────────────────────────────────────────

/// Phase 1: Route incoming commands to matching panes or watchlist.
pub(crate) fn route_commands(rx: &mpsc::Receiver<ChartCommand>, panes: &mut [Chart], active_pane: &mut usize, watchlist: &mut Watchlist) {
    use crate::monitoring::{span_begin, span_end};
    span_begin("cmd_routing");
    while let Ok(cmd) = rx.try_recv() {
        match &cmd {
            // Pane-targeted commands: route by symbol OR option_contract.
            // Option panes carry a display label in `symbol` ("SPY 450C 0DTE") and the
            // real OCC ticker in `option_contract` — live bar frames arrive keyed by
            // OCC, so match both.
            ChartCommand::UpdateLastBar { symbol, .. } | ChartCommand::AppendBar { symbol, .. } => {
                let s = symbol.clone();
                for p in panes.iter_mut() {
                    if p.symbol == s || (!p.option_contract.is_empty() && p.option_contract == s) {
                        p.process(cmd.clone());
                    }
                }
            }
            ChartCommand::LoadBars { symbol, .. } | ChartCommand::PrependBars { symbol, .. } | ChartCommand::LoadDrawings { symbol, .. } => {
                let s = symbol.clone();
                crate::apex_log!("route.load", "cmd symbol='{s}' panes=[{}]",
                    panes.iter().map(|p| format!("{}|{}", p.symbol, p.option_contract)).collect::<Vec<_>>().join(","));
                if let Some(p) = panes.iter_mut().find(|p|
                    p.symbol == s || (!p.option_contract.is_empty() && p.option_contract == s))
                {
                    crate::apex_log!("route.load", "matched pane symbol='{}' option_contract='{}'", p.symbol, p.option_contract);
                    p.process(cmd);
                } else if let Some(p) = panes.get_mut(*active_pane) {
                    // Only adopt an unmatched load if the active pane is actually
                    // showing this symbol. A load whose symbol matches no visible pane
                    // is stale — the user switched away (or reset) before it arrived —
                    // so dropping it prevents an in-flight load from clobbering the
                    // active pane back to a previous symbol (rapid-switch convergence).
                    if !p.is_option && p.symbol == s {
                        crate::apex_log!("route.load", "fallback to active_pane (stock)");
                        p.process(cmd);
                    } else {
                        crate::apex_log!("route.load",
                            "DROPPED stale load '{s}' (active pane shows '{}', is_option={})",
                            p.symbol, p.is_option);
                    }
                }
            }
            // Watchlist-targeted commands: handle directly
            ChartCommand::WatchlistPrice { symbol, price, prev_close, day_close, change_perc, stale } => {
                watchlist.set_price(symbol, *price);
                watchlist.set_prev_close(symbol, *prev_close);
                watchlist.set_day_close(symbol, *day_close);
                watchlist.set_change_perc(symbol, *change_perc);
                watchlist.set_stale(symbol, *stale);
            }
            ChartCommand::ScannerPrice { symbol, price, prev_close, volume } => {
                // Update or insert into scanner results pool
                if let Some(r) = watchlist.scanner.results.iter_mut().find(|r| r.symbol == *symbol) {
                    r.price = *price;
                    r.volume = *volume;
                    r.change_pct = if *prev_close > 0.0 { (price - prev_close) / prev_close * 100.0 } else { 0.0 };
                } else {
                    let change_pct = if *prev_close > 0.0 { (price - prev_close) / prev_close * 100.0 } else { 0.0 };
                    watchlist.scanner.results.push(ScanResult {
                        symbol: symbol.clone(), price: *price, change_pct, volume: *volume,
                    });
                }
            }
            ChartCommand::HeatmapBars { cells } => {
                watchlist.heatmap.cells = cells.clone();
            }
            ChartCommand::TapeEntry { symbol, price, qty, time, is_buy } => {
                watchlist.tape.entries.push(TapeRow {
                    symbol: symbol.clone(), price: *price, qty: *qty, time: *time, is_buy: *is_buy,
                });
                // Cap at 500 entries
                if watchlist.tape.entries.len() > 500 {
                    watchlist.tape.entries.drain(..watchlist.tape.entries.len() - 500);
                }
            }
            ChartCommand::ChainData { symbol, dte, underlying_price, calls, puts, placeholder } => {
                if *symbol == watchlist.chain.symbol {
                    let to_rows = |data: &[(f32,f32,f32,f32,i32,i32,f32,bool,String)]| -> Vec<OptionRow> {
                        data.iter().map(|(strike,last,bid,ask,vol,oi,iv,itm,contract)| OptionRow {
                            strike: *strike, last: *last, bid: *bid, ask: *ask,
                            volume: *vol, oi: *oi, iv: *iv, itm: *itm, contract: contract.clone(),
                        }).collect()
                    };
                    if *dte == 0 {
                        watchlist.chain.near = OptionChain { calls: to_rows(calls), puts: to_rows(puts) };
                        watchlist.chain.near_placeholder = *placeholder;
                    } else {
                        watchlist.chain.far = OptionChain { calls: to_rows(calls), puts: to_rows(puts) };
                        watchlist.chain.far_placeholder = *placeholder;
                    }
                    watchlist.chain.loading = false;
                    // Wave 5: mirror the legacy boolean into the InFlightRegistry
                    // by completing any matching outstanding chain request.
                    let kind = crate::state::InFlightKind::OptionsChain {
                        underlying: symbol.clone(),
                    };
                    if let Some(id) = watchlist.inflight.dedup_kind(&kind) {
                        watchlist.inflight.complete(id);
                    }
                    if *underlying_price > 0.0 { watchlist.chain.underlying_price = *underlying_price; }
                    eprintln!("[chain] Loaded {} calls + {} puts for {} dte={} price={:.2}",
                        if *dte == 0 { watchlist.chain.near.calls.len() } else { watchlist.chain.far.calls.len() },
                        if *dte == 0 { watchlist.chain.near.puts.len()  } else { watchlist.chain.far.puts.len()  },
                        symbol, dte, underlying_price);
                }
            }
            ChartCommand::OverlayChainData { symbol, calls, puts, placeholder } => {
                let to_rows = |data: &[(f32,f32,f32,f32,i32,i32,f32,bool,String)]| -> Vec<OptionRow> {
                    data.iter().map(|(strike,last,bid,ask,vol,oi,iv,itm,contract)| OptionRow {
                        strike: *strike, last: *last, bid: *bid, ask: *ask,
                        volume: *vol, oi: *oi, iv: *iv, itm: *itm, contract: contract.clone(),
                    }).collect()
                };
                for chart in panes.iter_mut() {
                    if chart.symbol == *symbol && chart.overlay_chain_loading {
                        chart.overlay_calls = to_rows(calls);
                        chart.overlay_puts = to_rows(puts);
                        chart.overlay_chain_symbol = symbol.clone();
                        chart.overlay_chain_loading = false;
                        chart.overlay_chain_placeholder = *placeholder;
                        eprintln!("[overlay-chain] Loaded {} calls + {} puts for {}{}",
                            chart.overlay_calls.len(), chart.overlay_puts.len(), symbol,
                            if *placeholder { " (placeholder)" } else { "" });
                    }
                }
            }
            ChartCommand::SearchResults { query, results, source } => {
                if source == "watchlist" && !query.is_empty()
                    && watchlist.search.query.to_lowercase().starts_with(&query.to_lowercase()) {
                    for (sym, name) in results {
                        if !watchlist.search.results.iter().any(|(s, _)| s == sym) {
                            watchlist.search.results.push((sym.clone(), name.clone()));
                        }
                    }
                } else if source == "chain" && !query.is_empty()
                    && watchlist.chain.sym_input.to_lowercase().starts_with(&query.to_lowercase()) {
                    for (sym, name) in results {
                        if !watchlist.search.results.iter().any(|(s, _)| s == sym) {
                            watchlist.search.results.push((sym.clone(), name.clone()));
                        }
                    }
                } else if let Some(idx_str) = source.strip_prefix("pane_picker_") {
                    // In-pane ticker picker (core.rs ~1044). Route results to that
                    // pane's picker, but only if the user is still typing the same
                    // query (avoids a stale async response clobbering newer input).
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if let Some(p) = panes.get_mut(idx) {
                            if !query.is_empty()
                                && p.picker.last_query.to_lowercase().starts_with(&query.to_lowercase())
                            {
                                p.picker.searching = false;
                                for (sym, name) in results {
                                    if !p.picker.results.iter().any(|(s, _, _)| s == sym) {
                                        p.picker.results.push((sym.clone(), name.clone(), String::new()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Overlay bars: route to all panes that have this overlay symbol
            ChartCommand::OverlayBars { symbol, .. } => {
                let s = symbol.clone();
                for p in panes.iter_mut() { if p.symbol_overlays.iter().any(|o| o.symbol == s) { p.process(cmd.clone()); } }
            }
            // Everything else goes to active pane
            _ => {
                if let Some(p) = panes.get_mut(*active_pane) { p.process(cmd); }
            }
        }
    }
    if *active_pane >= panes.len() { *active_pane = 0; }
    span_end();
}

/// Phase 2: Check if active pane needs history pagination (scroll-back).
pub(crate) fn check_history_pagination(panes: &mut [Chart], active_pane: usize) {
    if active_pane < panes.len() {
        let chart = &mut panes[active_pane];
        // Trigger when left edge of viewport is within 30 bars of start of data
        let threshold = 30.0;
        if !chart.auto_scroll && chart.vs < threshold && !chart.history_loading && !chart.history_exhausted
            && !chart.bars.is_empty() && chart.timestamps.len() > 1 {
            chart.history_loading = true;
            let display_sym = chart.symbol.clone();
            let tf = chart.timeframe.clone();
            let earliest_ts = chart.timestamps[0];
            eprintln!("[history] TRIGGERED for {} {} (vs={:.1}, bars={})", display_sym, tf, chart.vs, chart.bars.len());
            // Option panes paginate by OCC (the real feed key) but PrependBars
            // is matched against the pane's display symbol — pass both.
            if chart.is_option && !chart.option_contract.is_empty() {
                fetch_option_history_background(
                    chart.option_contract.clone(), display_sym, tf, earliest_ts, chart.bar_source_mark);
            } else {
                fetch_history_background(display_sym, tf, earliest_ts);
            }
        }
    }
}

/// Phase 3: Run simulation tick + indicator recompute for all panes.
pub(crate) fn update_simulation(panes: &mut [Chart]) {
    use crate::monitoring::{span_begin, span_end};
    span_begin("simulation_indicators");
    for chart in panes.iter_mut() {
        // Recompute alt bars if dirty or source bars changed
        if matches!(chart.candle_mode, CandleMode::Renko | CandleMode::RangeBar | CandleMode::TickBar) {
            if chart.alt.dirty || chart.alt.source_len != chart.bars.len() {
                chart.recompute_alt_bars();
            }
        }
        chart.update_indicators();
        tick_simulation(chart);
    }
    span_end();
}

/// Apply a batch of cross-pane `PaneEvent`s to the pane vector.
///
/// Pulled out of `App::about_to_wait` so the propagation contract is
/// testable in isolation. Each `(event, origin)` pair from the drained
/// SubscriptionBus is applied to every sibling pane whose `link_group`
/// matches (or to all panes for `BROADCAST_GROUP`), skipping the
/// originating pane by index.
///
/// `group_count` validates non-broadcast groups against the
/// `Watchlist::link_groups` vector length, mirroring the prior
/// imperative loop's guard against stale group ids. `apply_bars_fetch`
/// is `true` in production (kicks off background bar loads); tests
/// set it to `false` to avoid the network side-effect.
///
/// Behavior parity with the prior imperative detector:
/// - Sibling whose current symbol/timeframe already matches is skipped.
/// - Sibling-symbol-change preserves timeframe, indicators, drawings
///   (only the bars + meta swap).
/// - Sibling-timeframe-change mirrors the per-pane tab-cache stash +
///   cache-hit restore from `App::about_to_wait`.
pub(crate) fn apply_pane_events(
    panes: &mut [Chart],
    events: &[(crate::state::PaneEvent, Option<usize>)],
    group_count: u8,
    apply_bars_fetch: bool,
) {
    use crate::state::{PaneEvent, BROADCAST_GROUP};
    for (event, origin) in events {
        match event {
            PaneEvent::SymbolChanged { group, symbol } => {
                let is_broadcast = *group == BROADCAST_GROUP;
                if !is_broadcast && (*group == 0 || *group > group_count) {
                    continue;
                }
                for (pi, pane) in panes.iter_mut().enumerate() {
                    if Some(pi) == *origin { continue; }
                    let matches = is_broadcast || pane.link_group == *group;
                    if !matches { continue; }
                    if pane.symbol == *symbol { continue; }
                    let tf = pane.timeframe.clone();
                    pane.symbol = symbol.clone();
                    pane.symbol_meta = crate::foundation::types::symbol_or_guess(symbol);
                    pane.bars.clear();
                    pane.timestamps.clear();
                    pane.indicator_bar_count = 0;
                    pane.vol_analytics_computed = 0;
                    pane.history_loading = false;
                    pane.history_exhausted = false;
                    pane.drawings_requested = false;
                    pane.drawings.clear();
                    if apply_bars_fetch {
                        pane.request_gen = pane.request_gen.wrapping_add(1);
                        fetch_bars_background(symbol.clone(), tf, pane.request_gen);
                    }
                }
            }
            PaneEvent::TimeframeChanged { group, timeframe } => {
                let is_broadcast = *group == BROADCAST_GROUP;
                if !is_broadcast && (*group == 0 || *group > group_count) {
                    continue;
                }
                for (pi, pane) in panes.iter_mut().enumerate() {
                    if Some(pi) == *origin { continue; }
                    let matches = is_broadcast || pane.link_group == *group;
                    if !matches { continue; }
                    if pane.timeframe == *timeframe { continue; }
                    if !pane.symbol.is_empty() && !pane.bars.is_empty() {
                        evict_oldest_if_full(&mut pane.tab_cache);
                        pane.tab_cache.insert(
                            (pane.symbol.clone(), pane.timeframe.clone()),
                            (pane.bars.clone(), pane.timestamps.clone(), std::time::Instant::now()),
                        );
                    }
                    pane.timeframe = timeframe.clone();
                    let sym = pane.symbol.clone();
                    let tf = pane.timeframe.clone();
                    let cache_hit = pane.tab_cache.get(&(sym.clone(), tf.clone())).cloned();
                    if let Some((cb, cts, _)) = cache_hit {
                        pane.bars = cb;
                        pane.timestamps = cts;
                        pane.indicator_bar_count = 0;
                    } else {
                        pane.bars.clear();
                        pane.timestamps.clear();
                    }
                    for ind in &mut pane.indicators {
                        ind.values.clear(); ind.values2.clear(); ind.values3.clear();
                        ind.values4.clear(); ind.values5.clear();
                        ind.supertrend_dir.clear(); ind.histogram.clear(); ind.divergences.clear();
                        ind.source_bars.clear(); ind.source_timestamps.clear();
                        ind.source_loaded = false;
                    }
                    pane.drawings.clear();
                    pane.drawings_requested = false;
                    pane.history_loading = false;
                    pane.history_exhausted = false;
                    pane.replay_mode = false;
                    pane.replay_playing = false;
                    if apply_bars_fetch {
                        pane.request_gen = pane.request_gen.wrapping_add(1);
                        fetch_bars_background(sym.clone(), tf.clone(), pane.request_gen);
                    }
                    if !pane.symbol_overlays.is_empty() {
                        for ov in &mut pane.symbol_overlays {
                            ov.bars.clear();
                            ov.timestamps.clear();
                            ov.loading = true;
                            fetch_overlay_bars_background(ov.symbol.clone(), tf.clone());
                        }
                    }
                }
            }
            PaneEvent::ToggleChanged { group, kind, value } => {
                use crate::state::PaneToggle;
                let is_broadcast = *group == BROADCAST_GROUP;
                if !is_broadcast && (*group == 0 || *group > group_count) {
                    continue;
                }
                for (pi, pane) in panes.iter_mut().enumerate() {
                    if Some(pi) == *origin { continue; }
                    let matches = is_broadcast || pane.link_group == *group;
                    if !matches { continue; }
                    match kind {
                        PaneToggle::LogScale          => pane.log_scale = *value,
                        PaneToggle::OhlcTooltip       => pane.ohlc_tooltip = *value,
                        PaneToggle::MeasureTooltip    => pane.measure_tooltip = *value,
                        PaneToggle::ShowVolume        => pane.show_volume = *value,
                        PaneToggle::ShowDeltaVolume   => pane.show_delta_volume = *value,
                        PaneToggle::ShowRvol          => pane.show_rvol = *value,
                        PaneToggle::ShowMaRibbon      => pane.show_ma_ribbon = *value,
                        PaneToggle::ShowCvd           => pane.show_cvd = *value,
                        PaneToggle::ShowPrevClose     => pane.show_prev_close = *value,
                        PaneToggle::ShowPatternLabels => pane.show_pattern_labels = *value,
                        PaneToggle::ShowFootprint     => pane.show_footprint = *value,
                        PaneToggle::ShowAutoFib       => pane.show_auto_fib = *value,
                        PaneToggle::HitHighlight      => pane.hit_highlight = *value,
                    }
                }
            }
            PaneEvent::SwingLegModeChanged { group, value } => {
                let is_broadcast = *group == BROADCAST_GROUP;
                if !is_broadcast && (*group == 0 || *group > group_count) {
                    continue;
                }
                for (pi, pane) in panes.iter_mut().enumerate() {
                    if Some(pi) == *origin { continue; }
                    let matches = is_broadcast || pane.link_group == *group;
                    if !matches { continue; }
                    pane.swing_leg_mode = *value;
                }
            }
            PaneEvent::IndicatorVisibilityChanged { group, kind, visible } => {
                let is_broadcast = *group == BROADCAST_GROUP;
                if !is_broadcast && (*group == 0 || *group > group_count) {
                    continue;
                }
                for (pi, pane) in panes.iter_mut().enumerate() {
                    if Some(pi) == *origin { continue; }
                    let matches = is_broadcast || pane.link_group == *group;
                    if !matches { continue; }
                    for ind in pane.indicators.iter_mut() {
                        if ind.kind == *kind { ind.visible = *visible; }
                    }
                }
            }
            PaneEvent::IndicatorsRemoved { group, kind, period } => {
                let is_broadcast = *group == BROADCAST_GROUP;
                if !is_broadcast && (*group == 0 || *group > group_count) {
                    continue;
                }
                for (pi, pane) in panes.iter_mut().enumerate() {
                    if Some(pi) == *origin { continue; }
                    let matches = is_broadcast || pane.link_group == *group;
                    if !matches { continue; }
                    let before = pane.indicators.len();
                    pane.indicators.retain(|ind| {
                        !(ind.kind == *kind && period.is_none_or(|p| ind.period == p))
                    });
                    if pane.indicators.len() != before {
                        pane.indicator_bar_count = 0;
                    }
                }
            }
            PaneEvent::IndicatorAdded { group, indicator } => {
                let is_broadcast = *group == BROADCAST_GROUP;
                if !is_broadcast && (*group == 0 || *group > group_count) {
                    continue;
                }
                for (pi, pane) in panes.iter_mut().enumerate() {
                    if Some(pi) == *origin { continue; }
                    let matches = is_broadcast || pane.link_group == *group;
                    if !matches { continue; }
                    let mut copy = indicator.clone();
                    copy.id = pane.next_indicator_id;
                    pane.next_indicator_id += 1;
                    pane.indicators.push(copy);
                    pane.indicator_bar_count = 0;
                }
            }
            PaneEvent::LayoutChanged | PaneEvent::BroadcastEnabled { .. } => {
                // No subscribers today; queue drain still removes them.
            }
        }
    }
}

/// Phase 4: Apply theme, font scale, cache account data, get window ref.
pub(crate) fn setup_theme(ctx: &egui::Context, panes: &[Chart], active_pane: usize, watchlist: &Watchlist) -> (usize, Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>, Option<Arc<Window>>) {
    if panes.is_empty() {
        // No panes yet (early launch frame) — skip theme application and
        // return a sensible zero-state so the caller can continue safely.
        return (0, None, None);
    }
    // Clamp active_pane in case the index is stale relative to the current
    // pane list length (e.g. a pane was just removed).
    let active_pane = active_pane.min(panes.len() - 1);
    let theme_idx = panes[active_pane].theme_idx;
    let _t_owned = get_theme(theme_idx);
    let t = &_t_owned;
    // P5b extraction Step 3: stash a portable copy of the chart Theme so
    // ui_kit widgets reading `theme::active_theme(ctx)` get a PortableTheme
    // carrying the active palette's colors (bull/bear/accent/text/etc.).
    // Without this, ui_kit's portable active_theme returns the default
    // PortableTheme and widgets show the wrong palette.
    crate::ui_kit::widgets::theme::set_ambient_theme(
        ctx,
        crate::chart_renderer::theme_impl::theme_to_portable(t),
    );
    // Stream S5 — ADOPTION: stash the active RecipeSet so ui_kit widgets built
    // via `StyleCtx::from_ctx` pick up theme-pack overrides automatically.
    // S8 update: only fall back to the empty set when no ThemePack has stashed
    // a real RecipeSet — if a pack was activated, its recipes are already in
    // egui memory and we must not overwrite them with the empty placeholder.
    {
        let has_pack_recipes = ctx.data(|d| {
            d.get_temp::<std::sync::Arc<crate::design_system::recipes::RecipeSet>>(
                egui::Id::new("apex_ambient_recipes"),
            ).is_some()
        });
        if !has_pack_recipes {
            crate::ui_kit::widgets::theme::set_ambient_recipes(
                ctx,
                crate::ui_kit::widgets::theme::empty_recipe_arc(),
            );
        }
    }

    // S8 — apply the persisted ThemePack on the very first frame (once).
    crate::chart_renderer::theme_pack_bridge::apply_startup_active_pack(ctx);
    {
        let mut style = (*ctx.style()).clone();
        style.visuals.panel_fill = t.toolbar_bg;
        style.visuals.extreme_bg_color = t.bg;
        // ── Rich visual system — editorial design language ──
        let is_light = t.is_light();
        style.visuals.dark_mode = !is_light;
        style.visuals.override_text_color = Some(t.text);
        style.interaction.tooltip_delay = 0.12;

        // Popup/dropdown shadows — subtle, matches GPU shadow visual intent.
        // egui's built-in feathered shadow is heavy by default; tune down so
        // ComboBox / menu_button / response.context_menu read as "barely there
        // but present" rather than 2010 Win32 chrome. Apex-specific dropdowns
        // paint paint_shadow_gpu themselves on top of this.
        style.visuals.popup_shadow = egui::epaint::Shadow {
            offset: [0, 4],
            blur: 12,
            spread: 0,
            color: color_alpha(t.shadow_color, if is_light { 56 } else { 80 }),
        };
        // Window shadows (dialogs, egui::Window) — slightly stronger than popups.
        style.visuals.window_shadow = egui::epaint::Shadow {
            offset: [0, 6],
            blur: 16,
            spread: 0,
            color: color_alpha(t.shadow_color, if is_light { 64 } else { 96 }),
        };

        // Corner radii — reduced for dropdowns, moderate for buttons
        let r = egui::CornerRadius::same(style::radius_sm() as u8);
        let popup_r = egui::CornerRadius::same(style::radius_md() as u8); // halved from 12

        // ── Widget styling ──

        // Inactive — subtle fill, visible border
        style.visuals.widgets.inactive.bg_fill       = color_alpha(t.toolbar_border, if is_light { 12 } else { 18 });
        style.visuals.widgets.inactive.weak_bg_fill  = egui::Color32::TRANSPARENT;
        style.visuals.widgets.inactive.bg_stroke     = egui::Stroke::new(style::stroke_medium(), color_alpha(t.toolbar_border, if is_light { 50 } else { 35 }));
        style.visuals.widgets.inactive.corner_radius = r;
        style.visuals.widgets.inactive.fg_stroke     = egui::Stroke::new(style::stroke_std(), t.dim);

        // Hovered — clear feedback, beveled feel
        style.visuals.widgets.hovered.bg_fill        = color_alpha(t.toolbar_border, if is_light { 35 } else { 45 });
        style.visuals.widgets.hovered.bg_stroke      = egui::Stroke::new(style::stroke_std(), color_alpha(t.accent, if is_light { 90 } else { 70 }));
        style.visuals.widgets.hovered.corner_radius  = r;
        style.visuals.widgets.hovered.fg_stroke      = egui::Stroke::new(style::stroke_std(), t.text);

        // Active/pressed
        style.visuals.widgets.active.bg_fill         = color_alpha(t.accent, if is_light { 30 } else { 40 });
        style.visuals.widgets.active.bg_stroke       = egui::Stroke::new(style::stroke_std(), color_alpha(t.accent, alpha_strong()));
        style.visuals.widgets.active.corner_radius   = r;
        style.visuals.widgets.active.fg_stroke       = egui::Stroke::new(style::stroke_std(), t.accent);

        // Open (menu/combo open state)
        style.visuals.widgets.open.bg_fill           = color_alpha(t.accent, if is_light { 25 } else { 35 });
        style.visuals.widgets.open.bg_stroke         = egui::Stroke::new(style::stroke_std(), color_alpha(t.accent, alpha_active()));
        style.visuals.widgets.open.corner_radius     = r;
        style.visuals.widgets.open.fg_stroke         = egui::Stroke::new(style::stroke_std(), t.accent);

        // Selection
        style.visuals.selection.bg_fill              = color_alpha(t.accent, if is_light { 25 } else { 35 });
        style.visuals.selection.stroke               = egui::Stroke::new(style::stroke_std(), t.accent);

        // Popup/menu window — more visible border, reduced rounding.
        // Border width is a fixed 1.2 px (the v0.9.7 value) — deliberately
        // between stroke_std (1.0) and stroke_bold (1.5); the colour stays
        // theme-wired off toolbar_border.
        style.visuals.window_fill                    = t.toolbar_bg;
        style.visuals.window_stroke                  = egui::Stroke::new(1.2, color_alpha(t.toolbar_border, if is_light { 80 } else { 60 }));
        style.visuals.window_corner_radius           = popup_r;
        style.visuals.menu_corner_radius             = popup_r;

        // Spacing — more padding, balanced sides, taller items
        style.spacing.button_padding                 = egui::vec2(12.0, 6.0);
        style.spacing.menu_margin                    = egui::Margin { left: 10, right: 10, top: 8, bottom: 8 };
        style.spacing.interact_size.y                = 26.0;
        style.spacing.item_spacing                   = egui::vec2(6.0, 4.0);

        // Crisp text rendering
        style.visuals.text_cursor.on_duration = 0.5;

        // Per-style egui overrides (Meridien/density/shadows/scrollbar) merged
        // into the SAME style object — ONE clone + ONE set_style per frame instead
        // of two (halves the per-frame Style allocation). Must run AFTER the rich
        // visual block so per-style tweaks win (#3).
        let st = super::ui::style::current();
        super::ui::style::apply_ui_style(&mut style, &st, t.toolbar_border, t.toolbar_bg, t.accent);
        ctx.set_style(style);
    }
    // native_dpi_scale is the floor (never render below display resolution).
    // font_scale is the user zoom on top; on a 1x display it wins if > 1.0,
    // on Retina (2x) the display floor takes over unless the user zooms past it.
    ctx.set_pixels_per_point(watchlist.font_scale.max(watchlist.native_dpi_scale));
    let account_data_cached = read_account_data();
    // NOTE: reconcile_with_ib is NOT called from the render loop — the
    // background hot-orders poller in `trading/mod.rs` calls it every 1s.
    // Calling it per-frame (60Hz) here was the source of duplicate FILLED
    // toasts: the same broker payload was reconciled both by the poller
    // and the frame loop, and any transient state mismatch could re-fire
    // the toast. The poller is the single canonical caller now.
    // Drain order manager toasts (fills, rejections, cancellations) into pending.
    {
        use crate::chart_renderer::ui::tools::notification::{Notification, severity_for_order_msg, push_pending};
        let order_toasts = super::trading::order_manager::drain_order_toasts();
        for msg in order_toasts {
            let sev = severity_for_order_msg(&msg);
            push_pending(Notification::new(msg, sev).with_source("orders"));
        }
    }
    // Drain ApexData toasts (sub_rejected, feed errors) into pending.
    // Messages may carry a leading control-byte severity prefix — decoded into
    // NotificationSeverity by decode_apex_message.
    {
        use crate::chart_renderer::ui::tools::notification::{Notification, decode_apex_message, push_pending};
        let apex_toasts = crate::apex_data::live_state::drain_toasts();
        for msg in apex_toasts {
            let (text, sev) = decode_apex_message(&msg);
            push_pending(Notification::new(text, sev).with_source("feed"));
        }
    }
    // T4: Detect trading mode from APEX_TRADING_MODE env var, read once at
    // startup. Accepted values: "live" (real money) or "paper" (simulated).
    // Defaults to "paper" if the variable is absent or has any other value
    // (fail-safe: unknown config must never accidentally enable live trading).
    {
        static PAPER_DETECTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !PAPER_DETECTED.load(std::sync::atomic::Ordering::Relaxed) {
            PAPER_DETECTED.store(true, std::sync::atomic::Ordering::Relaxed);
            let is_paper = match std::env::var("APEX_TRADING_MODE").as_deref() {
                Ok("live") => false,
                Ok("paper") | _ => true, // unset, invalid, or "paper" → paper (fail-safe)
            };
            // The guard in set_paper_mode only blocks switching TO paper when live
            // orders exist.  At startup there are none, so this is always Ok.
            let _ = super::trading::order_manager::set_paper_mode(is_paper);
        }
    }
    super::trading::order_manager::gc_orders(); // periodic cleanup
    // Stash the active theme index AND the resolved Theme in egui memory so
    // ui_kit widgets can read either form. The ambient Theme stash is the
    // portable path; the idx is kept for legacy code that does its own
    // registry lookup.
    let resolved = get_theme(theme_idx);
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new("apex_active_theme_idx"), theme_idx);
        d.insert_temp(egui::Id::new("apex_ambient_theme"), resolved);
    });
    let win_ref: Option<Arc<Window>> = {
        CURRENT_WINDOW.with(|w| w.borrow().clone())
    };
    (theme_idx, account_data_cached, win_ref)
}

/// Phase 5: Render the top toolbar (symbol picker, layout controls, settings, account strip).
fn generate_placeholder_fundamentals(symbol: &str, bars: &[super::types::Bar]) -> FundamentalData {
    let price = bars.last().map(|b| b.close).unwrap_or(150.0);
    // Seed from symbol name for consistency
    let seed: u32 = symbol.bytes().map(|b| b as u32).sum::<u32>();
    let r = |base: f32, range: f32| -> f32 { base + ((seed as f32 * 7.3 + base * 3.1).sin() * 0.5 + 0.5) * range };
    FundamentalData {
        pe_ratio: r(18.0, 20.0),
        forward_pe: r(16.0, 18.0),
        eps_ttm: price / r(18.0, 20.0),
        market_cap: r(50.0, 2500.0) as f64,
        dividend_yield: r(0.0, 3.0),
        revenue_growth: r(-5.0, 30.0),
        profit_margin: r(5.0, 30.0),
        debt_to_equity: r(0.2, 2.0),
        short_interest: r(1.0, 8.0),
        institutional_pct: r(50.0, 40.0),
        insider_pct: r(1.0, 15.0),
        beta: r(0.6, 1.2),
        avg_volume: r(5.0, 50.0) as f64 * 1_000_000.0,
        shares_outstanding: r(500.0, 3000.0) as f64 * 1_000_000.0,
        analyst_target_mean: price * r(1.02, 0.15),
        analyst_target_high: price * r(1.15, 0.20),
        analyst_target_low: price * r(0.80, 0.15),
        analyst_buy: (r(5.0, 20.0)) as u8,
        analyst_hold: (r(3.0, 10.0)) as u8,
        analyst_sell: (r(0.0, 5.0)) as u8,
        earnings: vec![
            EarningsQuarter { quarter: "Q1 2026".into(), eps_actual: r(1.2, 1.5), eps_estimate: r(1.1, 1.3), revenue_actual: r(20.0, 60.0) as f64 * 1000.0, revenue_estimate: r(19.0, 58.0) as f64 * 1000.0, date: 0 },
            EarningsQuarter { quarter: "Q4 2025".into(), eps_actual: r(1.0, 1.4), eps_estimate: r(1.0, 1.2), revenue_actual: r(18.0, 55.0) as f64 * 1000.0, revenue_estimate: r(17.0, 53.0) as f64 * 1000.0, date: 0 },
            EarningsQuarter { quarter: "Q3 2025".into(), eps_actual: r(0.9, 1.3), eps_estimate: r(0.95, 1.1), revenue_actual: r(17.0, 50.0) as f64 * 1000.0, revenue_estimate: r(16.5, 48.0) as f64 * 1000.0, date: 0 },
            EarningsQuarter { quarter: "Q2 2025".into(), eps_actual: r(0.85, 1.2), eps_estimate: r(0.9, 1.0), revenue_actual: r(16.0, 48.0) as f64 * 1000.0, revenue_estimate: r(15.5, 46.0) as f64 * 1000.0, date: 0 },
        ],
    }
}

fn generate_placeholder_econ() -> Vec<EconEvent> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    vec![
        EconEvent { time: now + 86400 * 2, name: "FOMC Rate Decision".into(), importance: 3, actual: None, forecast: 4.5, previous: 4.5, country: "US".into() },
        EconEvent { time: now + 86400 * 5, name: "CPI (MoM)".into(), importance: 2, actual: None, forecast: 0.3, previous: 0.4, country: "US".into() },
        EconEvent { time: now + 86400 * 8, name: "Non-Farm Payrolls".into(), importance: 3, actual: None, forecast: 180.0, previous: 195.0, country: "US".into() },
        EconEvent { time: now + 86400 * 12, name: "PPI (YoY)".into(), importance: 1, actual: None, forecast: 2.2, previous: 2.4, country: "US".into() },
        EconEvent { time: now + 86400 * 15, name: "Retail Sales".into(), importance: 2, actual: None, forecast: 0.5, previous: 0.7, country: "US".into() },
        EconEvent { time: now + 86400 * 20, name: "GDP (QoQ)".into(), importance: 3, actual: None, forecast: 2.1, previous: 2.3, country: "US".into() },
    ]
}

fn generate_placeholder_insiders(symbol: &str) -> Vec<InsiderTrade> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let seed: u32 = symbol.bytes().map(|b| b as u32).sum();
    let names = ["John Smith (CEO)", "Jane Doe (CFO)", "Robert Lee (CTO)", "Sarah Chen (VP Sales)", "Michael Park (Director)"];
    let mut trades = Vec::new();
    for i in 0..6u32 {
        let s = seed.wrapping_mul(i + 1).wrapping_add(7919);
        let is_buy = s % 3 == 0;
        let shares = ((s % 50 + 5) * 1000) as i64 * if is_buy { 1 } else { -1 };
        let price = 100.0 + (s % 200) as f32;
        trades.push(InsiderTrade {
            name: names[(s as usize) % names.len()].into(),
            title: "".into(),
            transaction: if is_buy { "Buy" } else { "Sell" }.into(),
            shares, price,
            date: now - (i as i64 + 1) * 86400 * ((s % 15 + 3) as i64),
            value: (shares.abs() as f64) * price as f64,
        });
    }
    trades
}

fn generate_placeholder_journal() -> Vec<JournalEntry> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    vec![
        JournalEntry { id: "j1".into(), symbol: "AAPL".into(), side: "Long".into(), qty: 100, entry_price: 188.50, exit_price: 192.30, pnl: 380.0, pnl_pct: 2.02, entry_time: now - 86400 * 2, exit_time: now - 86400 * 2 + 3600 * 4, duration_mins: 240, setup_type: "breakout".into(), notes: "Clean break above 188 resistance with volume".into(), tags: vec!["momentum".into()], timeframe: "5m".into(), r_multiple: 1.8 },
        JournalEntry { id: "j2".into(), symbol: "NVDA".into(), side: "Long".into(), qty: 50, entry_price: 118.20, exit_price: 115.80, pnl: -120.0, pnl_pct: -2.03, entry_time: now - 86400 * 3, exit_time: now - 86400 * 3 + 3600 * 2, duration_mins: 120, setup_type: "scalp".into(), notes: "Stopped out on reversal".into(), tags: vec!["scalp".into()], timeframe: "1m".into(), r_multiple: -1.0 },
        JournalEntry { id: "j3".into(), symbol: "TSLA".into(), side: "Short".into(), qty: 30, entry_price: 248.00, exit_price: 238.50, pnl: 285.0, pnl_pct: 3.83, entry_time: now - 86400 * 4, exit_time: now - 86400 * 3, duration_mins: 1440, setup_type: "swing".into(), notes: "Bearish divergence on daily RSI".into(), tags: vec!["swing".into(), "divergence".into()], timeframe: "1D".into(), r_multiple: 2.4 },
        JournalEntry { id: "j4".into(), symbol: "SPY".into(), side: "Long".into(), qty: 200, entry_price: 562.00, exit_price: 565.80, pnl: 760.0, pnl_pct: 0.68, entry_time: now - 86400 * 5, exit_time: now - 86400 * 5 + 3600 * 6, duration_mins: 360, setup_type: "breakout".into(), notes: "Gap and go above PDH".into(), tags: vec!["momentum".into(), "gap".into()], timeframe: "5m".into(), r_multiple: 1.5 },
        JournalEntry { id: "j5".into(), symbol: "MSFT".into(), side: "Long".into(), qty: 75, entry_price: 420.00, exit_price: 418.50, pnl: -112.5, pnl_pct: -0.36, entry_time: now - 86400 * 6, exit_time: now - 86400 * 6 + 3600, duration_mins: 60, setup_type: "scalp".into(), notes: "Weak follow-through".into(), tags: vec!["scalp".into()], timeframe: "1m".into(), r_multiple: -0.5 },
        JournalEntry { id: "j6".into(), symbol: "AMZN".into(), side: "Long".into(), qty: 40, entry_price: 186.00, exit_price: 191.20, pnl: 208.0, pnl_pct: 2.80, entry_time: now - 86400 * 7, exit_time: now - 86400 * 6, duration_mins: 1440, setup_type: "swing".into(), notes: "Bounce off 50 SMA with ApexSignals precursor".into(), tags: vec!["swing".into(), "signals".into()], timeframe: "1D".into(), r_multiple: 2.1 },
        JournalEntry { id: "j7".into(), symbol: "META".into(), side: "Short".into(), qty: 25, entry_price: 502.00, exit_price: 508.00, pnl: -150.0, pnl_pct: -1.20, entry_time: now - 86400 * 8, exit_time: now - 86400 * 8 + 3600 * 3, duration_mins: 180, setup_type: "mean-rev".into(), notes: "Failed breakdown, squeezed out".into(), tags: vec!["mean-rev".into()], timeframe: "15m".into(), r_multiple: -1.2 },
        JournalEntry { id: "j8".into(), symbol: "GOOG".into(), side: "Long".into(), qty: 60, entry_price: 170.00, exit_price: 174.50, pnl: 270.0, pnl_pct: 2.65, entry_time: now - 86400 * 10, exit_time: now - 86400 * 8, duration_mins: 2880, setup_type: "swing".into(), notes: "Earnings drift play".into(), tags: vec!["earnings".into(), "swing".into()], timeframe: "1D".into(), r_multiple: 1.9 },
    ]
}

pub(crate) fn widget_description(kind: super::ChartWidgetKind) -> &'static str {
    use super::ChartWidgetKind::*;
    match kind {
        TrendStrength  => "Trend health gauge with needle",
        Momentum       => "RSI gauge with overbought/oversold",
        Volatility     => "ATR with % of price bar",
        VolumeProfile  => "Mini volume-at-price bars",
        SessionTimer   => "Countdown ring to market close",
        KeyLevels      => "Pivot points with distance %",
        OptionGreeks   => "Delta/Gamma/Theta/Vega display",
        RiskReward     => "Position risk-reward bar",
        MarketBreadth  => "Advance/decline, new highs/lows",
        Correlation    => "Correlation gauge vs SPY",
        DarkPool       => "Unusual volume / dark pool prints",
        PositionPnl    => "Live unrealized P&L for position",
        EarningsBadge  => "Earnings countdown with expected move",
        NewsTicker     => "Scrolling headline strip",
        ExitGauge      => "Position exit urgency meter",
        PrecursorAlert => "Smart money / unusual options",
        TradePlan      => "Entry/target/stop suggestion",
        ChangePoints   => "Regime shift detection timeline",
        ZoneStrength   => "Supply/demand zone health",
        PatternScanner => "Latest candlestick patterns",
        VixMonitor     => "VIX spot, gap, convergence",
        SignalDashboard=> "All signals in one compact view",
        DivergenceMonitor => "Active indicator divergences",
        ConvictionMeter=> "Aggregate signal conviction score",
        RsiMulti       => "Concentric RSI across 7 timeframes",
        TrendAlign     => "Multi-TF trend alignment grid",
        VolumeShelf    => "Volume shelf S/R levels",
        Confluence     => "S/R confluence meter",
        FlowCompass    => "Institutional flow compass",
        VolRegime      => "Volatility regime detector",
        MomentumHeat   => "Momentum across lookbacks",
        BreadthThermo  => "Market breadth dot matrix",
        SectorRotation => "Sector rotation quadrant",
        OptionsSentiment => "Options sentiment composite",
        RelStrength    => "Relative strength vs market",
        RiskDash       => "Position risk calculator",
        EarningsMom    => "Earnings momentum trends",
        LiquidityScore => "Liquidity health gauge",
        SignalRadar    => "Radial map of all active signals",
        CrossAssetPulse => "Multi-asset market dashboard",
        TapeSpeed      => "Trade velocity speedometer",
        Fundamentals   => "PE, EPS, margins, ownership",
        EconCalendar   => "Upcoming economic events",
        Latency        => "Frame time + data feed latency",
        PayoffChart    => "Options payoff curve diagram",
        OptionsFlow    => "Unusual options activity",
        PositionsPanel => "All positions with P&L + close",
        DailyPnl       => "Hero daily P&L with close all",
        Custom         => "User-defined widget",
    }
}

/// Paint a tiny preview icon for a widget in the picker dropdown.
pub(crate) fn paint_widget_preview(p: &egui::Painter, r: egui::Rect, kind: super::ChartWidgetKind, t: &Theme, active: bool) {
    use super::ChartWidgetKind as W;
    let cx = r.center().x;
    let cy = r.center().y;
    let accent = if active { t.accent } else { color_half(t.dim) };
    let bull = if active { t.bull } else { color_dim(t.dim) };
    let bear = if active { t.bear } else { color_very_dim(t.dim) };

    match kind {
        // Donut gauges — small ring
        W::TrendStrength | W::Momentum | W::ConvictionMeter | W::LiquidityScore
        | W::OptionsSentiment | W::Volatility => {
            let r_sz = 9.0;
            for i in 0..16 {
                let a = (i as f32 / 16.0) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                let a2 = ((i + 1) as f32 / 16.0) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                let col = if i < 11 { accent } else { color_alpha(t.toolbar_border, alpha_muted()) };
                p.line_segment([
                    egui::pos2(cx + r_sz * a.cos(), cy + r_sz * a.sin()),
                    egui::pos2(cx + r_sz * a2.cos(), cy + r_sz * a2.sin())],
                    egui::Stroke::new(style::stroke_heavy(), col));
            }
        }
        // Concentric rings
        W::RsiMulti | W::VolRegime | W::RelStrength => {
            for i in 0..3 {
                let r_sz = 10.0 - i as f32 * 3.0;
                let frac = [0.7, 0.5, 0.85][i];
                for j in 0..12 {
                    let a = (j as f32 / 12.0) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                    let a2 = ((j + 1) as f32 / 12.0) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                    let col = if (j as f32 / 12.0) < frac { accent } else { color_alpha(t.toolbar_border, alpha_muted()) };
                    p.line_segment([
                        egui::pos2(cx + r_sz * a.cos(), cy + r_sz * a.sin()),
                        egui::pos2(cx + r_sz * a2.cos(), cy + r_sz * a2.sin())],
                        egui::Stroke::new(style::stroke_thick(), col));
                }
            }
        }
        // Dot grid
        W::TrendAlign | W::BreadthThermo => {
            for row in 0..4 {
                for col in 0..4 {
                    let dx = r.left() + 5.0 + col as f32 * 5.5;
                    let dy = r.top() + 5.0 + row as f32 * 5.5;
                    let on = (row + col) % 3 != 0;
                    p.circle_filled(egui::pos2(dx, dy), 1.8, if on { bull } else { color_alpha(t.toolbar_border, alpha_muted()) });
                }
            }
        }
        // Horizontal bars
        W::VolumeShelf | W::Confluence | W::VolumeProfile => {
            for i in 0..4 {
                let y = r.top() + 4.0 + i as f32 * 6.0;
                let w = [18.0, 12.0, 22.0, 8.0][i];
                let col = if i % 2 == 0 { bull } else { bear };
                p.rect_filled(egui::Rect::from_min_size(egui::pos2(r.left() + 3.0, y), egui::vec2(w, 4.0)), 1.0, col);
            }
        }
        // Heat strip
        W::MomentumHeat => {
            for i in 0..7 {
                let x = r.left() + 2.0 + i as f32 * 3.5;
                let col = if i < 4 { bull } else { bear };
                let alpha = [180, 120, 200, 80, 100, 160, 60][i] as u8;
                p.rect_filled(egui::Rect::from_min_size(egui::pos2(x, r.top() + 4.0), egui::vec2(3.0, 20.0)),
                    1.0, egui::Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), alpha));
            }
        }
        // Compass
        W::FlowCompass => {
            p.circle_stroke(egui::pos2(cx, cy), 10.0, egui::Stroke::new(style::stroke_std(), accent));
            p.line_segment([egui::pos2(cx, cy), egui::pos2(cx + 4.0, cy - 8.0)], egui::Stroke::new(style::stroke_bold(), bull));
            p.circle_filled(egui::pos2(cx, cy), 2.0, accent);
        }
        // 2x2 quadrant
        W::SectorRotation | W::EarningsMom => {
            p.line_segment([egui::pos2(cx, r.top() + 3.0), egui::pos2(cx, r.bottom() - 3.0)],
                egui::Stroke::new(style::stroke_thin(), color_alpha(t.dim, alpha_muted())));
            p.line_segment([egui::pos2(r.left() + 3.0, cy), egui::pos2(r.right() - 3.0, cy)],
                egui::Stroke::new(style::stroke_thin(), color_alpha(t.dim, alpha_muted())));
            for (dx, dy, col) in [(5.0, -5.0, bull), (-4.0, 3.0, bear), (3.0, 4.0, accent)] {
                p.circle_filled(egui::pos2(cx + dx, cy + dy), 2.5, col);
            }
        }
        // Radar dots
        W::SignalRadar => {
            p.circle_stroke(egui::pos2(cx, cy), 10.0, egui::Stroke::new(style::stroke_thin(), color_alpha(t.dim, alpha_muted())));
            for i in 0..8 {
                let a = (i as f32 / 8.0) * std::f32::consts::TAU;
                let on = i % 3 != 0;
                let rr = if on { 10.0 } else { 6.0 };
                p.circle_filled(egui::pos2(cx + rr * a.cos(), cy + rr * a.sin()), 1.5,
                    if on { accent } else { color_alpha(t.dim, alpha_muted()) });
            }
        }
        // Grid cells
        W::CrossAssetPulse => {
            for row in 0..2 {
                for col in 0..4 {
                    let x = r.left() + 2.0 + col as f32 * 6.5;
                    let y = r.top() + 4.0 + row as f32 * 12.0;
                    let col_c = [bull, bear, bull, accent][col];
                    p.rect_filled(egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(5.5, 10.0)), 1.0, color_alpha(col_c, alpha_dim()));
                }
            }
        }
        // Speedometer
        W::TapeSpeed | W::SessionTimer => {
            let segs = 10;
            for i in 0..segs {
                let a = std::f32::consts::PI + (i as f32 / segs as f32) * std::f32::consts::PI;
                let a2 = std::f32::consts::PI + ((i + 1) as f32 / segs as f32) * std::f32::consts::PI;
                let col = if i < 6 { accent } else { color_alpha(t.toolbar_border, alpha_muted()) };
                p.line_segment([
                    egui::pos2(cx + 10.0 * a.cos(), cy + 4.0 + 10.0 * a.sin()),
                    egui::pos2(cx + 10.0 * a2.cos(), cy + 4.0 + 10.0 * a2.sin())],
                    egui::Stroke::new(style::stroke_extra_thick(), col));
            }
        }
        // Hero number fallback
        _ => {
            p.text(egui::pos2(cx, cy), egui::Align2::CENTER_CENTER, kind.icon(),
                egui::FontId::proportional(style::font_md_plus()), accent);
        }
    }
}


// ─── Render functions (moved to render/pane.rs) ──────────────────────────────
pub(crate) use super::render::pane::draw_chart;


// ─── winit + egui integration ─────────────────────────────────────────────────

/// A single native chart window with its own GPU context, panes, and layout.
// ─── Watchlist ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub(crate) struct HotKey {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) category: String,
    pub(crate) action: String,
    pub(crate) key_name: String,
    pub(crate) key: egui::Key,
    pub(crate) ctrl: bool,
    pub(crate) shift: bool,
    pub(crate) alt: bool,
}

// ─── Discord Chat ────────────────────────────────────────────────────────────
// TODO: Connect to Discord bot via WebSocket — needs bot token in K8s secrets

#[derive(Clone)]
pub(crate) struct DiscordMessage {
    pub(crate) author: String,
    pub(crate) content: String,
    pub(crate) timestamp: String, // "2m ago", "12:34"
    pub(crate) is_own: bool, // true if sent by the user
    #[allow(dead_code)]
    pub(crate) has_chart: bool, // true if message contains a chart screenshot
}

// ─── News Feed ───────────────────────────────────────────────────────────────
// TODO: Connect to stock wire API / news feed — poll every 60s

#[derive(Clone)]
pub(crate) struct NewsItem {
    pub(crate) headline: String,
    pub(crate) source: String, // "Reuters", "Bloomberg", "Benzinga"
    pub(crate) timestamp: String, // "10m ago", "1h ago"
    pub(crate) symbol: String, // related symbol
    pub(crate) sentiment: i8, // -1 bearish, 0 neutral, 1 bullish
    pub(crate) url: String, // link to full article
}

#[derive(Clone)]
pub(crate) struct TapeRow {
    pub(crate) symbol: String,
    pub(crate) price: f32,
    pub(crate) qty: f32,
    pub(crate) time: i64, // epoch ms
    pub(crate) is_buy: bool,
}

#[derive(Clone)]
pub(crate) struct WatchlistItem {
    pub(crate) symbol: String,
    pub(crate) price: f32,
    pub(crate) prev_close: f32,
    /// Today's regular-session close (bulk snap `day.c`); 0 while live/pre-open,
    /// set after close + weekends. Drives last-close-to-close + ext-hours change.
    pub(crate) day_close: f32,
    /// Server-computed session/DST-aware % change (apex-data-v2). `Some` →
    /// render directly; `None` → fall back to the client-side computation.
    pub(crate) change_perc: Option<f32>,
    /// True when the latest snapshot was served from the backend's last-good
    /// cache (upstream blip) rather than fresh. Value is still real; the row
    /// marks it so the trader knows it isn't live.
    pub(crate) stale: bool,
    pub(crate) loaded: bool,
    // Option fields (defaults for stocks)
    pub(crate) is_option: bool,
    pub(crate) underlying: String, // e.g. "SPY"
    pub(crate) option_type: String, // "C" or "P"
    pub(crate) strike: f32,
    pub(crate) expiry: String, // "0DTE", "5DTE" etc.
    pub(crate) bid: f32,
    pub(crate) ask: f32,
    // Watchlist enhancement fields
    pub(crate) pinned: bool,
    pub(crate) tags: Vec<String>,
    pub(crate) rvol: f32, // relative volume (1.0 = average)
    pub(crate) atr: f32, // average true range
    pub(crate) high_52wk: f32,
    pub(crate) low_52wk: f32,
    pub(crate) day_high: f32,
    pub(crate) day_low: f32,
    pub(crate) avg_daily_range: f32, // average daily move % for extreme detection
    pub(crate) earnings_days: i32, // days until earnings (-1 = unknown)
    pub(crate) alert_triggered: bool,
    pub(crate) price_history: Vec<f32>, // last ~30 price snapshots for sparkline
    // Flash animation fields — transient, not persisted.
    pub(crate) prev_price: f32,                              // price value from the previous quote update
    pub(crate) price_change_at: Option<std::time::Instant>, // when price last changed (None = never changed)
}

#[derive(Clone)]
pub(crate) struct WatchlistSection {
    pub(crate) id: u32,
    pub(crate) title: String, // optional label, empty = no header shown
    pub(crate) color: Option<String>, // hex bg tint, None = default
    pub(crate) collapsed: bool,
    pub(crate) items: Vec<WatchlistItem>,
}

#[derive(Clone)]
pub(crate) struct SavedWatchlist {
    pub(crate) name: String,
    pub(crate) sections: Vec<WatchlistSection>,
    pub(crate) next_section_id: u32,
}

// ─── Options chain ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct OptionRow {
    pub(crate) strike: f32,
    pub(crate) last: f32,
    pub(crate) bid: f32,
    pub(crate) ask: f32,
    pub(crate) volume: i32,
    pub(crate) oi: i32,
    pub(crate) iv: f32,
    pub(crate) itm: bool,
    pub(crate) contract: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct SavedOption {
    pub(crate) contract: String,
    pub(crate) symbol: String,
    pub(crate) strike: f32,
    pub(crate) is_call: bool,
    pub(crate) expiry: String,
    pub(crate) last: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum WatchlistTab { Stocks, Chain, Heat, Scan }

// ─── Scanner types ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub(crate) struct ScanResult {
    pub(crate) symbol: String,
    pub(crate) price: f32,
    pub(crate) change_pct: f32,
    pub(crate) volume: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ScanSort {
    ChangeDesc,
    ChangeAsc,
    VolumeDesc,
}

#[derive(Clone, Debug)]
pub(crate) struct ScannerDef {
    pub(crate) name: String,
    pub(crate) preset: Option<String>, // "gainers", "losers", "most_active"; None = custom
    pub(crate) min_change: f32,
    pub(crate) max_change: f32,
    pub(crate) min_volume: u64,
    pub(crate) sort_by: ScanSort,
    pub(crate) limit: usize,
    pub(crate) collapsed: bool,
}

impl ScannerDef {
    fn preset_gainers() -> Self {
        Self { name: "Top Gainers".into(), preset: Some("gainers".into()), min_change: 0.0, max_change: 999.0, min_volume: 0, sort_by: ScanSort::ChangeDesc, limit: 20, collapsed: false }
    }
    fn preset_losers() -> Self {
        Self { name: "Top Losers".into(), preset: Some("losers".into()), min_change: -999.0, max_change: 0.0, min_volume: 0, sort_by: ScanSort::ChangeAsc, limit: 20, collapsed: false }
    }
    fn preset_most_active() -> Self {
        Self { name: "Most Active".into(), preset: Some("most_active".into()), min_change: -999.0, max_change: 999.0, min_volume: 0, sort_by: ScanSort::VolumeDesc, limit: 20, collapsed: false }
    }
}

/// Cross-pane tab drag state — populated when user starts dragging a tab header,
/// cleared when drag ends. Handled in draw_chart after all panes are rendered.
#[derive(Clone)]
pub(crate) struct TabDragState {
    pub source_pane: usize,
    pub tab_idx: usize,
    pub symbol: String,
    pub timeframe: String,
    pub price: f32,
    pub change: f32,
    pub current_pos: egui::Pos2,
}

/// A named, colored pane link group. Groups live on the Watchlist so all panes
/// in the same window share the same group definitions.
pub(crate) struct LinkGroup {
    pub(crate) name: String,
    pub(crate) color: egui::Color32,
}

/// Play-editor form state (WS-E E3, Watchlist-split strangler slice 1).
/// Grouped out of the Watchlist god-struct: 19 transient input-buffer fields
/// for the play-entry form. Not persisted (ephemeral UI). `kind` was
/// `play_editor_type` (renamed — `type` is a reserved word). The explicit
/// `Default` mirrors the original per-field seeds exactly (qty=1, pct seeds).
pub(crate) struct PlayEditorState {
    pub(crate) open: bool,
    pub(crate) symbol: String,
    pub(crate) entry: String,
    pub(crate) target: String,
    pub(crate) stop: String,
    pub(crate) notes: String,
    pub(crate) direction: super::PlayDirection,
    pub(crate) kind: super::PlayType,
    pub(crate) qty: u32,
    pub(crate) qty_str: String,
    pub(crate) tags: Vec<String>,
    pub(crate) t2: String,
    pub(crate) t2_pct: String,
    pub(crate) t3: String,
    pub(crate) t3_pct: String,
    pub(crate) has_t2: bool,
    pub(crate) has_t3: bool,
    pub(crate) custom_tag: String,
    pub(crate) target_pct: String,
}

impl Default for PlayEditorState {
    fn default() -> Self {
        Self {
            open: false,
            symbol: String::new(),
            entry: String::new(),
            target: String::new(),
            stop: String::new(),
            notes: String::new(),
            direction: super::PlayDirection::Long,
            kind: super::PlayType::Directional,
            qty: 1,
            qty_str: "1".into(),
            tags: vec![],
            t2: String::new(),
            t2_pct: "25".into(),
            t3: String::new(),
            t3_pct: "25".into(),
            has_t2: false,
            has_t3: false,
            custom_tag: String::new(),
            target_pct: "100".into(),
        }
    }
}

/// Scripting/backtesting panel state (WS-E E3, Watchlist-split slice 2).
/// Grouped out of the Watchlist god-struct: 6 fields for the script editor +
/// backtest output. Not persisted. Explicit Default mirrors the seeds
/// (result_tab = Output).
pub(crate) struct ScriptState {
    pub(crate) open: bool,
    pub(crate) source: String,
    pub(crate) output: String,
    pub(crate) ai_prompt: String,
    pub(crate) result_tab: super::ui::panels::script_panel::ScriptResultTab,
    pub(crate) backtest: Option<super::ui::panels::script_panel::BacktestResult>,
}

impl Default for ScriptState {
    fn default() -> Self {
        Self {
            open: false,
            source: String::new(),
            output: String::new(),
            ai_prompt: String::new(),
            result_tab: super::ui::panels::script_panel::ScriptResultTab::Output,
            backtest: None,
        }
    }
}

/// Scanner + custom-scanner-builder state (WS-E E3, Watchlist-split slice 3).
/// Grouped out of the Watchlist god-struct: 12 scanner_* fields (results pool,
/// fetch state, builder form, movers tab). Explicit Default mirrors the seeds
/// (3 preset defs; min/max change -999/999). `open`/`builder_open` also mirror
/// the persisted SidebarState flags (which stay flat there).
pub(crate) struct ScannerState {
    pub(crate) open: bool,
    pub(crate) defs: Vec<ScannerDef>,
    pub(crate) results: Vec<ScanResult>,
    pub(crate) last_fetch: Option<std::time::Instant>,
    pub(crate) fetching: bool,
    pub(crate) new_name: String,
    pub(crate) new_min_change: f32,
    pub(crate) new_max_change: f32,
    pub(crate) new_min_volume: String,
    pub(crate) builder_open: bool,
    pub(crate) mover_tab: usize,
    pub(crate) filter_popup_open: bool,
}

impl Default for ScannerState {
    fn default() -> Self {
        Self {
            open: false,
            defs: vec![ScannerDef::preset_gainers(), ScannerDef::preset_losers(), ScannerDef::preset_most_active()],
            results: vec![],
            last_fetch: None,
            fetching: false,
            new_name: String::new(),
            new_min_change: -999.0,
            new_max_change: 999.0,
            new_min_volume: String::new(),
            builder_open: false,
            mover_tab: 0,
            filter_popup_open: false,
        }
    }
}

/// RRG (Relative Rotation Graph) panel state (WS-E E3, Watchlist-split slice 4).
/// Grouped out of the Watchlist god-struct: 5 rrg_* panel fields. Distinct from
/// the Theme's rrg_leading/improving/weakening/lagging QUADRANT COLORS (those
/// stay on Theme). `open` mirrors the persisted SidebarState flag (kept flat).
pub(crate) struct RrgState {
    pub(crate) open: bool,
    pub(crate) sectors: Vec<super::ui::panels::rrg_panel::RRGSector>,
    pub(crate) cycle_phase: String,
    pub(crate) time_offset: f32,
    pub(crate) tail_length: usize,
}

impl Default for RrgState {
    fn default() -> Self {
        Self {
            open: false,
            sectors: vec![],
            cycle_phase: String::new(),
            time_offset: 0.0,
            tail_length: 5,
        }
    }
}

/// Discord chat-panel state (WS-E E3, Watchlist-split slice 5). Grouped out of
/// the Watchlist god-struct: 17 discord_* fields. The 9 persisted subset
/// (open/input/channel/authenticated/username/user_id/selected_guild/
/// selected_channel/last_msg_id) mirror the ChatState store (kept flat there);
/// the rest are runtime cache. All field types impl Default → derive it.
#[derive(Default)]
pub(crate) struct DiscordState {
    pub(crate) open: bool,
    pub(crate) messages: Vec<DiscordMessage>,
    pub(crate) input: String,
    pub(crate) channel: String,
    pub(crate) authenticated: bool,
    pub(crate) username: String,
    pub(crate) user_id: String,
    pub(crate) guilds: Vec<crate::discord::DiscordGuild>,
    pub(crate) selected_guild: Option<String>,
    pub(crate) channels: Vec<crate::discord::DiscordChannel>,
    pub(crate) selected_channel: Option<String>,
    pub(crate) connecting: bool,
    pub(crate) guild_icons: std::collections::HashMap<String, egui::TextureHandle>,
    pub(crate) last_msg_id: Option<String>,
    pub(crate) poll_timer: Option<std::time::Instant>,
    pub(crate) channels_loading: bool,
    pub(crate) messages_loading: bool,
}

/// Command-palette state (WS-E E3, Watchlist-split slice 6). Grouped out of the
/// Watchlist god-struct: 8 cmd_palette_* fields. `recent`+`freq` are persisted
/// separately (cmd_palette_state.json via workspace_persist — field reads only).
/// Explicit Default (sel = -1 = no selection).
pub(crate) struct CmdPaletteState {
    pub(crate) open: bool,
    pub(crate) query: String,
    pub(crate) results: Vec<(String, String, String)>,
    pub(crate) sel: i32,
    pub(crate) recent: Vec<String>,
    pub(crate) freq: std::collections::HashMap<String, u32>,
    pub(crate) ai_mode: bool,
    pub(crate) ai_input: String,
}

impl Default for CmdPaletteState {
    fn default() -> Self {
        Self {
            open: false,
            query: String::new(),
            results: vec![],
            sel: -1,
            recent: vec![],
            freq: std::collections::HashMap::new(),
            ai_mode: false,
            ai_input: String::new(),
        }
    }
}

/// Options-chain state (WS-E E3, Watchlist-split slice 7). Grouped out of the
/// Watchlist god-struct: 24 chain_* fields. SEMANTIC RENAME (the flat names
/// couldn't strip cleanly — `chain_0dte`/`chain_0_*` start with a digit): the
/// near/0-DTE chain is `near` + `near_*`; the far-dated chain is `far` + `far_*`;
/// shared/legacy fields keep their names. Not synced/persisted (runtime + fetch).
pub(crate) struct ChainState {
    pub(crate) symbol: String,
    pub(crate) sym_input: String,
    pub(crate) num_strikes: usize, // legacy fallback
    pub(crate) far_dte: i32,
    pub(crate) near: OptionChain,  // was chain_0dte
    pub(crate) far: OptionChain,
    /// True when `near` / `far` rows are locally-synthesized Black-Scholes
    /// placeholder data (real upstream unavailable).
    pub(crate) near_placeholder: bool, // was chain_0dte_placeholder
    pub(crate) far_placeholder: bool,
    pub(crate) select_mode: bool,
    pub(crate) loading: bool, // true while fetching chain from ApexIB
    pub(crate) underlying_price: f32, // real-time underlying price from IB chain response
    pub(crate) frozen: bool, // legacy fallback
    pub(crate) center_offset: i32, // legacy fallback
    // Per-chain independent controls
    pub(crate) near_num_strikes: usize, // was chain_0_num_strikes
    pub(crate) near_frozen: bool,
    pub(crate) near_offset: i32,
    pub(crate) near_strike_mode: StrikeMode,
    pub(crate) near_nmf: u8, // 0=near, 1=mid, 2=far
    pub(crate) far_num_strikes: usize,
    pub(crate) far_frozen: bool,
    pub(crate) far_offset: i32,
    pub(crate) far_strike_mode: StrikeMode,
    pub(crate) far_nmf: u8,
    pub(crate) last_fetch: Option<std::time::Instant>, // debounce chain refetches
}

impl Default for ChainState {
    fn default() -> Self {
        Self {
            symbol: "SPY".into(), sym_input: String::new(), num_strikes: 10, far_dte: 1,
            near: OptionChain::default(), far: OptionChain::default(),
            near_placeholder: false, far_placeholder: false,
            select_mode: false, loading: false, underlying_price: 0.0,
            frozen: false, center_offset: 0,
            near_num_strikes: 10, near_frozen: false, near_offset: 0, near_strike_mode: StrikeMode::Count, near_nmf: 0,
            far_num_strikes: 10, far_frozen: false, far_offset: 0, far_strike_mode: StrikeMode::Count, far_nmf: 0,
            last_fetch: None,
        }
    }
}

/// Order-ledger panel state (WS-E E3, Watchlist-split slice 8). Grouped out of
/// the Watchlist god-struct: 6 order_ledger_* fields. `open`/`view`/`filter`
/// mirror the persisted SidebarState flags (kept flat there). All Default.
#[derive(Default)]
pub(crate) struct OrderLedgerState {
    pub(crate) open: bool,
    pub(crate) view: u8,    // 0=Active, 1=Journal, 2=All
    pub(crate) filter: u8,  // index into LedgerFilter
    pub(crate) search: String,
    /// P2: per-symbol sub-section expanded state — keys are symbol strings. Ephemeral.
    pub(crate) sym_expanded: std::collections::HashMap<String, bool>,
    /// P2: pending bulk-cancel confirmation (Some(symbol) = inline confirm row). Ephemeral.
    pub(crate) pending_bulk_cancel: Option<String>,
}

/// Heatmap display config (WS-E E3, Watchlist-split slice 9). 4 heat_* fields.
/// Not synced/persisted. Explicit Default (index "Watchlist", cols 2).
pub(crate) struct HeatState {
    pub(crate) index: String,
    pub(crate) collapsed: std::collections::HashSet<String>,
    pub(crate) cols: u8,
    pub(crate) sort: i8,
}

impl Default for HeatState {
    fn default() -> Self {
        Self { index: "Watchlist".into(), collapsed: std::collections::HashSet::new(), cols: 2, sort: 0 }
    }
}

/// ProvenancePane state (WS-E E3, Watchlist-split slice 9). 2 provenance_*
/// fields. `open` mirrors the persisted SidebarState flag (kept flat there).
#[derive(Default)]
pub(crate) struct ProvenanceState {
    pub(crate) open: bool,
    pub(crate) active_lineage: Option<String>,
}

/// Analysis sidebar state (WS-E E3, Watchlist-split slice 9). 3 analysis_*
/// fields (auto_chart_open stays a separate Watchlist field). `open` mirrors
/// the persisted SidebarState flag. Explicit Default (tab = Rrg, one split).
pub(crate) struct AnalysisState {
    pub(crate) open: bool,
    pub(crate) tab: crate::chart_renderer::AnalysisTab,
    pub(crate) splits: Vec<SplitSection<crate::chart_renderer::AnalysisTab>>,
}

impl Default for AnalysisState {
    fn default() -> Self {
        Self {
            open: false,
            tab: crate::chart_renderer::AnalysisTab::Rrg,
            splits: vec![SplitSection::new(crate::chart_renderer::AnalysisTab::Rrg, 1.0)],
        }
    }
}

/// Feed sidebar-panel state (WS-E E3, Watchlist-split slice 10). 3 fields for
/// the Feed sidebar (News/Discord/Screenshots). Named `feed_panel` (not `feed`)
/// because Watchlist already has `feed: Vec<Play>` (the plays feed — different
/// feature). `open` mirrors the persisted SidebarState flag. Default: tab News.
pub(crate) struct FeedPanelState {
    pub(crate) open: bool,
    pub(crate) tab: crate::chart_renderer::FeedTab,
    pub(crate) splits: Vec<SplitSection<crate::chart_renderer::FeedTab>>,
}

impl Default for FeedPanelState {
    fn default() -> Self {
        Self {
            open: false,
            tab: crate::chart_renderer::FeedTab::News,
            splits: vec![SplitSection::new(crate::chart_renderer::FeedTab::News, 1.0)],
        }
    }
}

/// Signals sidebar-panel state (WS-E E3, Watchlist-split slice 11). 3 fields
/// (signals_panel_open/tab/splits). `open` mirrors both the persisted
/// SidebarState flag AND the workspace JSON key "signals_panel_open" (string
/// key unchanged). Explicit Default (tab Alerts).
pub(crate) struct SignalsPanelState {
    pub(crate) open: bool,
    pub(crate) tab: crate::chart_renderer::SignalsTab,
    pub(crate) splits: Vec<SplitSection<crate::chart_renderer::SignalsTab>>,
}

impl Default for SignalsPanelState {
    fn default() -> Self {
        Self {
            open: false,
            tab: crate::chart_renderer::SignalsTab::Alerts,
            splits: vec![SplitSection::new(crate::chart_renderer::SignalsTab::Alerts, 1.0)],
        }
    }
}

/// Timeframe favorites/dropdown state (WS-E E3, Watchlist-split slice 12). 3
/// timeframe_* fields. `favorites` mirrors the persisted SidebarState list
/// (kept flat there). Explicit Default (8 preset timeframes).
pub(crate) struct TimeframeState {
    pub(crate) favorites: Vec<String>,
    pub(crate) dropdown_open: bool,
    pub(crate) dropdown_pos: egui::Pos2,
}

impl Default for TimeframeState {
    fn default() -> Self {
        Self {
            favorites: vec!["1m".into(), "5m".into(), "15m".into(), "30m".into(), "1h".into(), "4h".into(), "1d".into(), "1wk".into()],
            dropdown_open: false,
            dropdown_pos: egui::Pos2::ZERO,
        }
    }
}

/// Indicators sidebar-panel state (WS-E E3, Watchlist-split slice 13). 4
/// indicators_* fields. `panel_open` + `section_fracs` mirror the persisted
/// SidebarState (kept flat there). Explicit Default (section_fracs 0.18/0.25/0.57).
pub(crate) struct IndicatorsState {
    pub(crate) panel_open: bool,
    pub(crate) panel_search: String,
    pub(crate) lib_collapsed: std::collections::HashSet<String>,
    pub(crate) section_fracs: [f32; 3],
}

impl Default for IndicatorsState {
    fn default() -> Self {
        Self {
            panel_open: false,
            panel_search: String::new(),
            lib_collapsed: std::collections::HashSet::new(),
            section_fracs: [0.18, 0.25, 0.57],
        }
    }
}

/// Trade-journal PANEL state (WS-E E3, Watchlist-split slice 14). 3 fields
/// (journal_panel_open/entries/page). Distinct from `journal_open` (the
/// Book-tab journal toggle, which stays a flat Watchlist field). `open` mirrors
/// the persisted SidebarState flag. Default seeds placeholder journal entries.
pub(crate) struct JournalPanelState {
    pub(crate) open: bool,
    pub(crate) entries: Vec<JournalEntry>,
    pub(crate) page: usize,
}

impl Default for JournalPanelState {
    fn default() -> Self {
        Self { open: false, entries: generate_placeholder_journal(), page: 0 }
    }
}

/// News-feed panel state (WS-E E3, Watchlist-split slice 15). 4 news_* fields.
/// `open` mirrors the persisted SidebarState flag. Explicit Default
/// (sentiment_filter -2 = All).
pub(crate) struct NewsState {
    pub(crate) open: bool,
    pub(crate) items: Vec<NewsItem>,
    pub(crate) filter_symbol: bool,
    pub(crate) sentiment_filter: i8,
}

impl Default for NewsState {
    fn default() -> Self {
        Self { open: false, items: vec![], filter_symbol: false, sentiment_filter: -2 }
    }
}

/// Heatmap-pane data (WS-E E3, Watchlist-split slice 16). cells + last_fetch,
/// cold-started from /api/stocks/grouped. `heatmap_templates` stays flat with
/// the other *_templates lists. Not synced/persisted. All Default.
#[derive(Default)]
pub(crate) struct HeatmapState {
    pub(crate) cells: Vec<(String, f32, f64)>,
    pub(crate) last_fetch: Option<std::time::Instant>,
}

/// Time & Sales (tape) panel state (WS-E E3, Watchlist-split slice 17). 2
/// tape_* fields. `open` mirrors the persisted SidebarState flag. All Default.
#[derive(Default)]
pub(crate) struct TapeState {
    pub(crate) open: bool,
    pub(crate) entries: Vec<TapeRow>,
}

/// Pane split-ratio + divider-drag state (WS-E E3, Watchlist-split slice 18).
/// 8 split ratios (persisted to workspace + global-settings JSON under the
/// UNCHANGED "pane_split_*" / "splits" keys) + the transient `dragging` flag
/// (was `pane_divider_dragging`; not persisted/synced). The persisted subset
/// also mirrors the flat pane_split_* fields on both `SidebarState` and the
/// `LoadedSettings` apply-step — those stay flat there.
pub(crate) struct PaneSplitState {
    pub(crate) h: f32,
    pub(crate) v: f32,
    pub(crate) h2: f32,
    pub(crate) v2: f32,
    pub(crate) v3: f32,
    pub(crate) v4: f32,
    pub(crate) v5: f32,
    pub(crate) v6: f32,
    pub(crate) dragging: bool,
}

impl Default for PaneSplitState {
    fn default() -> Self {
        Self { h: 0.5, v: 0.5, h2: 0.5, v2: 0.5, v3: 0.5, v4: 0.5, v5: 0.5, v6: 0.5, dragging: false }
    }
}

/// Workspace-management state (WS-E E3, Watchlist-split slice 19). 7 fields
/// (active / save-name / pending-load / nav-expanded / pending-new-blank /
/// rename target+buf). `active` + `save_name` mirror SidebarState (flat there);
/// `nav_expanded` persists to workspace JSON under the UNCHANGED "rail_expanded"
/// key. Mixed original prefixes -> semantic field names.
pub(crate) struct WorkspaceState {
    pub(crate) active: String,          // was active_workspace
    pub(crate) save_name: String,       // was workspace_save_name
    pub(crate) pending_load: Option<String>, // was pending_workspace_load
    pub(crate) nav_expanded: bool,      // was workspace_nav_expanded
    pub(crate) pending_new_blank: bool,
    pub(crate) rename_target: Option<String>, // was workspace_rename_target
    pub(crate) rename_buf: String,      // was workspace_rename_buf
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            active: "Default".into(),
            save_name: String::new(),
            pending_load: None,
            nav_expanded: false,
            pending_new_blank: false,
            rename_target: None,
            rename_buf: String::new(),
        }
    }
}

/// Watchlist symbol-search state (WS-E E3, Watchlist-split slice 20). 4 fields
/// (query/results/sel/refocus) for the add-symbol search box. Not synced/
/// persisted; single consumer (watchlist_panel). Explicit Default (sel -1 = none).
pub(crate) struct SearchState {
    pub(crate) query: String,
    pub(crate) results: Vec<(String, String)>,
    pub(crate) sel: i32,
    pub(crate) refocus: bool,
}

impl Default for SearchState {
    fn default() -> Self {
        Self { query: String::new(), results: vec![], sel: -1, refocus: false }
    }
}

pub(crate) struct Watchlist {
    pub(crate) open: bool,
    /// User-defined link groups. Index 0 = group-id 1, index 1 = group-id 2, etc.
    pub(crate) link_groups: Vec<LinkGroup>,
    pub(crate) tab: WatchlistTab,
    pub(crate) sections: Vec<WatchlistSection>,
    pub(crate) next_section_id: u32,
    // Multi-watchlist
    pub(crate) saved_watchlists: Vec<SavedWatchlist>,
    pub(crate) active_watchlist_idx: usize,
    pub(crate) watchlist_name_editing: bool,
    pub(crate) watchlist_name_buf: String,
    /// Symbol-search box state (WS-E E3 slice 20) — was search_query/results/sel/refocus.
    pub(crate) search: SearchState,
    pub(crate) options_visible: bool, // toggle options section below stocks
    pub(crate) options_split: f32, // fraction of height for stocks (0.3..0.9), rest for options
    pub(crate) divider_dragging: bool, // true while dragging the stocks/options divider
    pub(crate) divider_y: f32, // screen Y of divider (set during render)
    pub(crate) divider_total_h: f32, // total available height for split calculation
    // Drag-and-drop state
    pub(crate) dragging:       Option<WatchlistDragState>,
    pub(crate) drag_start_pos: Option<egui::Pos2>,
    pub(crate) drop_target:    Option<WatchlistDragState>,
    pub(crate) drag_confirmed: bool,
    // Section editing
    pub(crate) renaming_section: Option<u32>, // section id being renamed
    pub(crate) rename_buf: String,
    // Toolbar
    pub(crate) hotkey_editor_open: bool,
    pub(crate) hotkey_editing_id: Option<u32>,
    pub(crate) settings_open: bool,
    pub(crate) font_scale: f32,
    pub(crate) native_dpi_scale: f32, // window.scale_factor() — 2.0 on Retina, 1.0 on 1x displays
    pub(crate) font_idx: usize, // 0=JetBrains, 1=Roboto, 2=SourceCode, 3=IBMPlex
    // Order defaults (global)
    pub(crate) default_stock_qty: u32,
    pub(crate) default_options_qty: u32,
    pub(crate) default_order_type: usize,   // 0=MKT, 1=LMT, 2=STP
    pub(crate) default_tif: usize,          // 0=DAY, 1=GTC, 2=IOC
    pub(crate) default_outside_rth: bool,
    pub(crate) compact_mode: bool,
    pub(crate) pane_header_size: crate::chart_renderer::PaneHeaderSize,
    pub(crate) toolbar_auto_hide: bool,
    pub(crate) toolbar_hover_time: Option<std::time::Instant>,
    pub(crate) show_x_axis: bool,
    pub(crate) show_y_axis: bool,
    pub(crate) shared_x_axis: bool,
    pub(crate) shared_y_axis: bool,
    pub(crate) hotkeys: Vec<HotKey>,
    // Order ledger panel state (wave 3b; WS-E E3 slice 8) — was 6 flat
    // `order_ledger_*` fields, now grouped into OrderLedgerState.
    pub(crate) order_ledger: OrderLedgerState,
    /// Order System Health panel — operator observability for the order
    /// subsystem. Toggled via Ctrl+Shift+O. See
    /// `chart::renderer::ui::panels::order_health_panel`.
    pub(crate) order_health_open: bool,
    /// Active bottom-dock tab: 0=Orders, 1=Positions, 2=Account, 3=Notifications.
    /// (Footer *visibility* is a style default + session override — `footer_visible()`.)
    pub(crate) bottom_dock_tab: u8,
    /// Persisted right-rail column width (px). Driven by the rail's resize grip.
    pub(crate) rail_col_width: f32,
    /// Persisted bottom-dock (footer) height (px). Driven by the dock's resize grip.
    pub(crate) bottom_dock_height: f32,
    pub(crate) trendline_filter_open: bool, // trendline filter dropdown
    pub(crate) account_strip_open: bool, // account summary bar below toolbar
    pub(crate) object_tree_open: bool, // object tree panel (drawings, indicators, overlays)
    pub(crate) broadcast_mode: bool, // when true, toolbar actions apply to all panes
    /// Drawing-tool favorites shown in the middle-click picker. Persisted.
    pub(crate) draw_favorites: Vec<String>,
    /// Boss key: when true, a full-viewport fake Excel TPS-report overlay
    /// is rendered over the entire app. Toggled by Cmd+Shift+H (configurable
    /// under action "tps_toggle") or the "TPS" toolbar button.
    pub(crate) boss_key_active: bool,
    /// UI style preset index (0..STYLE_NAMES.len()). Combines with `theme_idx`
    /// to form the full visual identity (e.g. "GruvBox/Meridien").
    pub(crate) style_idx: usize,
    /// User density override (Compact/Standard/Spacious). Scales row,
    /// button, tab heights. `None` = inherit from active style preset.
    /// Wired in P4.3; persisted in state.json.
    pub(crate) density_override: Option<crate::ui_kit::style::DensityMode>,
    /// User border-weight override (Hairline/Standard/Bold). Multiplier on
    /// every `stroke_*()` token. `None` = no override. P5.
    pub(crate) border_weight_override: Option<crate::ui_kit::style::BorderWeight>,
    /// User corner-scale override (Sharp/Subtle/Standard/Round). Multiplier on
    /// every `radius_*()` token. `None` = no override. P5.
    pub(crate) corner_scale_override: Option<crate::ui_kit::style::CornerScale>,
    /// User spacing-scale override (Tight/Standard/Loose). Multiplier on
    /// every `gap_*()` token. `None` = no override. P5.
    pub(crate) spacing_scale_override: Option<crate::ui_kit::style::SpacingScale>,
    /// User motion-speed override (Off/Fast/Standard/Slow). Multiplier on
    /// every `motion_*()` duration token. Off disables animation entirely
    /// — useful for accessibility, RDP, or distraction-free trading. P5.
    pub(crate) motion_speed_override: Option<crate::ui_kit::style::MotionSpeed>,
    pub(crate) pending_opt_chart: Option<PendingOptionChart>, // deferred option chart open
    /// Optional OCC contract ticker for the pending open. When present, used as the
    /// fetch key so real bars come from ApexData; pane.symbol stays the display label.
    pub(crate) pending_opt_chart_contract: Option<String>,
    pub(crate) apex_diag_open: bool,
    /// SOTA §4.2 — historical replay scrubber. Hidden by default; opened via
    /// the command palette (`setting:replay`) or future hotkey.
    pub(crate) replay_pane_open: bool,
    /// Developer-only widget gallery panel (Ctrl+Shift+G). See
    /// `chart::renderer::ui::panels::widget_gallery`.
    pub widget_gallery_open: bool,
    // Watchlist filter
    pub(crate) filter_open: bool,
    // Watchlist column config — ordered list of visible columns.
    pub(crate) wl_columns: Vec<crate::chart::renderer::ui::lists::rows::watchlist_columns::WatchlistColumnId>,
    pub(crate) wl_columns_open: bool, // settings popup
    pub(crate) filter_text: String,
    pub(crate) filter_preset: String,
    pub(crate) custom_filters: Vec<CustomFilter>,
    pub(crate) filter_min_change: f32,
    pub(crate) filter_max_change: f32,
    // Heatmap (WS-E E3 slice 9) — was 4 flat heat_* fields.
    pub(crate) heat: HeatState,
    // Orders
    pub(crate) orders_panel_open: bool,
    pub(crate) order_entry_open: bool,
    pub(crate) selected_order_ids: Vec<SelectedOrder>,
    // Positions
    pub(crate) positions: Vec<Position>,
    // Alerts
    pub(crate) alerts: Vec<Alert>,
    pub(crate) next_alert_id: u32,
    #[allow(dead_code)]
    pub(crate) alert_query: String,
    pub(crate) alerts_panel_open: bool,
    // Options chain (WS-E E3 slice 7) — was 24 flat `chain_*` fields.
    pub(crate) chain: ChainState,
    // Saved options
    pub(crate) saved_options: Vec<SavedOption>,
    pub(crate) dte_filter: i32,
    // Workspaces (WS-E E3 slice 19) — was 7 flat workspace-mgmt fields,
    // grouped into WorkspaceState.
    pub(crate) workspace: WorkspaceState,
    /// Active (focused) pane index, mirrored from the render loop each frame so
    /// `workspace_to_json` can persist it without threading `active_pane`
    /// through every `save_workspace` call site.
    pub(crate) active_pane_idx: usize,
    // Pane split ratios + divider drag (WS-E E3 slice 18) — was 8 pane_split_*
    // ratio fields + pane_divider_dragging, grouped into PaneSplitState.
    pub(crate) pane_split: PaneSplitState,
    /// Phase-1 PaneGrid topology. `None` = legacy 19-template + 8-split path
    /// (older workspaces). `Some` = recursive split tree drives layout.
    /// Watchlist is persisted via a hand-rolled JSON path (not serde-derived);
    /// the layout will be saved/restored alongside the other state fields.
    pub(crate) pane_layout: Option<crate::chart_renderer::pane_layout::PaneLayout>,
    /// Phase 1c — when the user clicks a pane header's Split button, store
    /// the pane_idx here so the H/V picker popup renders anchored to it.
    /// `None` = popup is closed. Cleared when an axis is picked, Escape is
    /// pressed, or the user clicks outside.
    pub(crate) pane_split_popup_for: Option<usize>,
    /// P17 #3 — undo stack for destructive pane operations. Each entry is
    /// a snapshot of `pane_layout` taken IMMEDIATELY BEFORE a split/close.
    /// Cap at 32 entries; oldest evicted FIFO.
    pub(crate) pane_layout_undo: Vec<Option<crate::chart_renderer::pane_layout::PaneLayout>>,
    /// Companion redo stack — populated when undo pops + pushes onto here.
    /// Cleared whenever a NEW destructive op happens (standard editor model).
    pub(crate) pane_layout_redo: Vec<Option<crate::chart_renderer::pane_layout::PaneLayout>>,
    /// P17 #1 — stable IDs for each Chart in `panes`. Parallel array; the
    /// id at position `i` belongs to `panes[i]`. When a chart is removed,
    /// this Vec is shortened in lockstep so id ordering stays consistent
    /// with chart ordering. `PaneLayout::PaneSlot` carries these IDs rather
    /// than raw indices, so closing pane N never shifts other panes' slot
    /// references — the previous index-decrement fixup is no longer needed.
    pub(crate) pane_ids: Vec<u64>,
    /// Monotonic id counter. Never decremented; new charts always get a
    /// fresh id. `0` is reserved (used as a sentinel for "uninitialized").
    pub(crate) next_pane_id: u64,
    // Command palette
    /// Command-palette state (WS-E E3 slice 6) — was 8 flat `cmd_palette_*` fields.
    pub(crate) cmd_palette: CmdPaletteState,
    // Layout favorites (shown as buttons in toolbar; rest in dropdown)
    pub(crate) layout_favorites: Vec<String>,
    pub(crate) layout_dropdown_open: bool,
    pub(crate) pending_overlay_add: bool,
    pub(crate) layout_dropdown_pos: egui::Pos2,
    // Timeframe favorites (shown as segmented control; full list in dropdown)
    /// Timeframe favorites/dropdown state (WS-E E3 slice 12) — was 3 flat timeframe_* fields.
    pub(crate) timeframe: TimeframeState,
    // Cross-pane tab drag state
    pub(crate) dragging_tab: Option<TabDragState>,
    // Pane templates (save/load indicator + toggle configs)
    pub(crate) pane_templates: Vec<(String, serde_json::Value)>,  // (name, serialized pane config)
    pub(crate) portfolio_templates: Vec<String>,
    pub(crate) dashboard_templates: Vec<String>,
    pub(crate) heatmap_templates: Vec<String>,
    pub(crate) spreadsheet_templates: Vec<String>,
    // Plays / Playbook system
    pub(crate) plays: Vec<super::Play>,
    /// Community/received feed of published plays (F1). Populated by Publish +
    /// import + (future) a backend feed source.
    pub(crate) feed: Vec<super::Play>,
    /// Feed filter (F2): symbol substring ("" = all).
    pub(crate) feed_filter_symbol: String,
    /// Risk sizing (P-power): account equity + risk-per-trade fraction, used to
    /// size plays from their stop distance and aggregate portfolio risk.
    pub(crate) account_size: f32,
    pub(crate) risk_pct: f32,
    /// Play-editor form state (WS-E E3 strangler slice 1) — was 19 flat
    /// `play_editor_*` fields, now grouped into PlayEditorState.
    pub(crate) play_editor: PlayEditorState,
    pub(crate) play_templates: Vec<super::PlayTemplate>,
    pub(crate) widget_presets: Vec<super::WidgetPreset>,
    pub(crate) widget_preset_name: String, // input buffer for naming a new preset
    pub(crate) pane_template_name: String, // input buffer for naming a new template
    // Discord chat panel
    /// Discord chat-panel state (WS-E E3 slice 5) — was 17 flat `discord_*` fields.
    pub(crate) discord: DiscordState,
    // Time & Sales
    /// Time & Sales (tape) panel state (WS-E E3 slice 17) — was tape_open + tape_entries.
    pub(crate) tape: TapeState,
    // News feed panel (WS-E E3 slice 15) — 4 news_* fields grouped into NewsState.
    pub(crate) news: NewsState,
    // Trade Journal panel
    pub(crate) journal_open: bool,
    // Scanner (WS-E E3 slice 3) — 12 scanner_* fields grouped into ScannerState.
    pub(crate) scanner: ScannerState,
    // Heatmap pane data (WS-E E3 slice 16) — was heatmap_cells + heatmap_last_fetch.
    // Each cell: (symbol, change_pct, dollar_volume). Cold-started from
    // /api/stocks/grouped/:date.
    pub(crate) heatmap: HeatmapState,
    // Spread Builder panel
    pub(crate) spread_open: bool,
    pub(crate) maximized_pane: Option<usize>, // Some(idx) = pane shown fullscreen
    pub(crate) spread_state: super::ui::panels::spread_panel::SpreadState,
    // Scripting / Backtesting panel
    /// Scripting/backtesting panel state (WS-E E3 slice 2) — was 6 flat
    /// `script_*` fields, now grouped into ScriptState.
    pub(crate) script: ScriptState,
    // Screenshot library
    pub(crate) screenshot_open: bool,
    pub(crate) screenshot_entries: Vec<super::ui::panels::screenshot_panel::ScreenshotEntry>,
    /// Chart Library sidepane open state (transient — not persisted).
    pub(crate) charts_library_open: bool,
    // RRG (Relative Rotation Graph)
    /// RRG panel state (WS-E E3 slice 4) — was 5 flat `rrg_*` fields.
    pub(crate) rrg: RrgState,
    // Analysis sidebar — subdivided sections (each has its own tab)
    /// Analysis sidebar state (WS-E E3 slice 9) — was analysis_open/tab/splits.
    pub(crate) analysis: AnalysisState,
    pub(crate) auto_chart_open: bool, // Auto-Charting side panel
    // Signals sidebar — subdivided sections
    /// Signals sidebar-panel state (WS-E E3 slice 11) — was signals_panel_open/tab/splits.
    pub(crate) signals_panel: SignalsPanelState,
    // Indicators panel — unified active list / library / tool toggles.
    /// Indicators sidebar-panel state (WS-E E3 slice 13) — was 4 flat indicators_* fields.
    pub(crate) indicators: IndicatorsState,
    // Feed sidebar — subdivided sections
    /// Feed sidebar-panel state (WS-E E3 slice 10) — was feed_panel_open/tab/splits.
    pub(crate) feed_panel: FeedPanelState,
    // Playbook sidebar
    pub(crate) playbook_panel_open: bool,
    // Trade Journal panel (WS-E E3 slice 14) — was journal_panel_open/entries/page.
    pub(crate) journal_panel: JournalPanelState,
    // Book pane tab (Positions/Orders + Journal)
    pub(crate) book_tab: crate::chart_renderer::BookTab,
    // ── SOTA UX (Agent A) — ProvenancePane state ────────────────────────────
    /// Whether the ProvenancePane (right side panel, evidence DAG) is open.
    /// ProvenancePane state (WS-E E3 slice 9) — was provenance_open +
    /// provenance_active_lineage, now grouped into ProvenanceState.
    pub(crate) provenance: ProvenanceState,
    // Wave 5: cross-pane event bus. Replaces ad-hoc pull-based pane
    // iteration for link-group + broadcast propagation. Listeners are
    // registered at construction time. See `state::subscriptions`.
    pub(crate) subscriptions: crate::state::SubscriptionBus,
    // Wave 5: centralized in-flight request tracker. Replaces scattered
    // `*_loading: bool` flags. Wave 5 wires the registry alongside the
    // legacy `chain_loading` boolean as proof-of-concept; subsequent
    // waves migrate the remaining flags one at a time. See
    // `state::inflight`.
    pub(crate) inflight: crate::state::InFlightRegistry,
    /// Wave 14c: typed aggregate for UI display preferences. Mirrors
    /// the legacy `font_scale` / `font_idx` / `compact_mode` /
    /// `pane_header_size` / `toolbar_auto_hide` / `show_x_axis` /
    /// `show_y_axis` / `shared_x_axis` / `shared_y_axis` / `style_idx`
    /// fields. The legacy fields remain the read source of truth
    /// (notably `core.rs` reads them directly); `push_to_ui_settings`
    /// copies legacy → aggregate before serialization and
    /// `pull_from_ui_settings` copies aggregate → legacy after load.
    pub(crate) ui_settings: crate::state::UiSettings,
    /// Wave 2 (state): `Store<UiSettings>` wraps the aggregate so mutations
    /// are debounce-persisted by the background supervisor.  The plain
    /// `ui_settings` field above stays in place as a mirror for the sacred
    /// `core.rs` paint pipeline and the legacy load/save path; it is kept in
    /// sync via `update_ui_settings()` / `pull_from_ui_settings()`.
    pub(crate) ui_settings_store: std::sync::Arc<crate::state::Store<crate::state::UiSettings>>,
    /// Wave 2 (state): `Store<TradingDefaults>` wraps the trading defaults so
    /// mutations are debounce-persisted by the background supervisor.
    ///
    /// The flat legacy fields (`default_stock_qty`, `default_options_qty`,
    /// `default_order_type`, `default_tif`, `default_outside_rth`) stay in
    /// place as the read source of truth for existing callers; they are kept
    /// in sync via `update_trading_defaults()` / `sync_trading_defaults_from_store()`.
    /// `daily_loss_cap` and `max_position_pct` are new fields that live ONLY in
    /// the store (no corresponding legacy flat field existed).
    pub(crate) trading_defaults_store: std::sync::Arc<crate::state::Store<crate::state::TradingDefaults>>,
    /// Wave 3 (state): `Store<AlertsState>` wraps the alerts aggregate so
    /// mutations are debounce-persisted by the background supervisor.
    ///
    /// The flat fields (`alerts`, `next_alert_id`, `alert_query`,
    /// `alerts_panel_open`) stay in place as the read source of truth for
    /// existing callers; they are kept in sync via
    /// `update_alerts_state()` / `sync_from_alerts_store()`.
    pub(crate) alerts_store: std::sync::Arc<crate::state::Store<crate::state::AlertsState>>,
    /// Wave 3 (state): `Store<SidebarState>` wraps the sidebar open-flags
    /// aggregate so mutations are debounce-persisted by the background supervisor.
    ///
    /// The flat boolean fields (`open`, `settings_open`, `tape_open`, etc.)
    /// stay in place as the read source of truth for existing callers and the
    /// sacred `core.rs` paint pipeline; they are kept in sync via
    /// `update_sidebar_state()` / `sync_from_sidebar_store()`.
    pub(crate) sidebar_state_store: std::sync::Arc<crate::state::Store<crate::state::SidebarState>>,
    /// Wave 3 (state): `Store<LayoutState>` wraps the layout aggregate so
    /// mutations are debounce-persisted by the background supervisor.
    ///
    /// The flat fields (`link_groups`, `broadcast_mode`, `pane_split_*`,
    /// `layout_favorites`, `timeframe_favorites`, `maximized_pane`,
    /// `pane_templates`, `portfolio_templates`, `dashboard_templates`,
    /// `heatmap_templates`, `spreadsheet_templates`, `active_workspace`,
    /// `workspace_save_name`) stay in place as the read source of truth for
    /// existing callers; they are kept in sync via
    /// `push_to_layout_store()` / `sync_from_layout_store()`.
    pub(crate) layout_state_store: std::sync::Arc<crate::state::Store<crate::state::LayoutState>>,
    /// Wave 3 (state): `Store<ChatState>` wraps the Discord chat aggregate so
    /// mutations are debounce-persisted by the background supervisor.
    ///
    /// The flat fields (`discord_open`, `discord_input`, `discord_channel`,
    /// `discord_authenticated`, `discord_username`, `discord_user_id`,
    /// `discord_selected_guild`, `discord_selected_channel`,
    /// `discord_last_msg_id`) stay in place as the read source of truth for
    /// existing callers; they are kept in sync via
    /// `push_to_chat_store()` / `sync_from_chat_store()`.
    pub(crate) chat_state_store: std::sync::Arc<crate::state::Store<crate::state::ChatState>>,
    // ── Top-nav symbol input (UX-1 Fix 1) ──────────────────────────────────
    /// Buffer for the editable symbol input in the top toolbar.
    pub(crate) top_nav_sym_input: String,
    /// True while the symbol input has focus (used for autocomplete dropdown
    /// visibility).
    pub(crate) top_nav_sym_focused: bool,
    // ── Welcome wizard (P2) ─────────────────────────────────────────────────
    /// First-launch wizard. `None` after completion or before first check.
    /// Populated in `new()` when `ui_settings.has_seen_welcome == false`.
    pub(crate) welcome_wizard: Option<crate::chart_renderer::ui::welcome::WelcomeWizard>,
}

const DEFAULT_WATCHLIST: &[&str] = &["SPY","QQQ","IWM","DIA","AAPL","MSFT","NVDA","TSLA","AMZN","META","GOOGL","GLD"];
const DEFAULT_CRYPTO: &[&str] = &["BTCUSDT","ETHUSDT","SOLUSDT","XRPUSDT","BNBUSDT","DOGEUSDT","ADAUSDT","AVAXUSDT","LINKUSDT","DOTUSDT","SUIUSDT","NEARUSDT","ARBUSDT","OPUSDT","APTUSDT","AAVEUSDT","UNIUSDT","ATOMUSDT","LTCUSDT","MATICUSDT"];

impl Watchlist {
    fn new() -> Self {
        let (saved_watchlists, active_idx) = load_watchlists();
        let active = &saved_watchlists[active_idx];
        let sections = active.sections.clone();
        let next_section_id = active.next_section_id;
        // Four predefined link groups so users have something to pick from
        // immediately. Panes default to link_group=0 ("None" / unlinked) and
        // `link_group_propagation` validates that a pane's group index is
        // within `watchlist.link_groups.len()` before linking, so untouched
        // panes never auto-link to each other through a default group.
        let link_groups = vec![
            LinkGroup { name: "Group 1".into(), color: egui::Color32::from_rgb(70, 130, 255) },
            LinkGroup { name: "Group 2".into(), color: egui::Color32::from_rgb(80, 200, 120) },
            LinkGroup { name: "Group 3".into(), color: egui::Color32::from_rgb(255, 160, 60) },
            LinkGroup { name: "Group 4".into(), color: egui::Color32::from_rgb(180, 100, 255) },
        ];
        Self { open: false, tab: WatchlistTab::Stocks, sections, next_section_id,
               link_groups,
               saved_watchlists, active_watchlist_idx: active_idx,
               watchlist_name_editing: false, watchlist_name_buf: String::new(),
               search: SearchState::default(),
               options_visible: true, options_split: 0.6, divider_dragging: false, divider_y: 0.0, divider_total_h: 0.0,
               dragging: None, drag_start_pos: None, drop_target: None, drag_confirmed: false,
               renaming_section: None, rename_buf: String::new(),
               hotkey_editor_open: false, hotkey_editing_id: None, hotkeys: default_hotkeys(),
               order_ledger: OrderLedgerState::default(),
               order_health_open: false,
               bottom_dock_tab: 0,
               rail_col_width: 400.0, bottom_dock_height: 240.0,
               settings_open: false, font_scale: 1.6, native_dpi_scale: 1.0, font_idx: 0,
               default_stock_qty: 100, default_options_qty: 1, default_order_type: 0, default_tif: 0, default_outside_rth: false,
               compact_mode: false,
               pane_header_size: crate::chart_renderer::PaneHeaderSize::Compact,
               show_x_axis: true, show_y_axis: true,
               toolbar_auto_hide: false, toolbar_hover_time: None, shared_x_axis: false, shared_y_axis: false,
               trendline_filter_open: false, account_strip_open: false, object_tree_open: false, broadcast_mode: false,
               draw_favorites: vec!["trendline".into(), "magnifier".into(), "measure".into(), "hline".into(), "channel".into(), "fibonacci".into()],
               boss_key_active: false,
               style_idx: 0,
               density_override: None,
               border_weight_override: None,
               corner_scale_override: None,
               spacing_scale_override: None,
               motion_speed_override: None,
               pending_opt_chart: None, pending_opt_chart_contract: None, apex_diag_open: false, replay_pane_open: false, widget_gallery_open: false,
               wl_columns: crate::chart::renderer::ui::lists::rows::watchlist_columns::default_columns(),
               wl_columns_open: false,
               filter_open: false, filter_text: String::new(), filter_preset: "All".into(), filter_min_change: -999.0, filter_max_change: 999.0, custom_filters: vec![],
               orders_panel_open: false, order_entry_open: false, selected_order_ids: vec![], positions: vec![], alerts: vec![], next_alert_id: 1, alert_query: String::new(), alerts_panel_open: false,
               chain: ChainState::default(),
               saved_options: vec![], dte_filter: -1,
               heat: HeatState::default(),
               workspace: WorkspaceState::default(),
               active_pane_idx: 0,
               pane_split: PaneSplitState::default(),
               // Phase 1 PaneGrid topology — None means "use legacy 8-fraction path".
               // Migration happens on first user action that depends on the tree.
               pane_layout: None,
               pane_split_popup_for: None,
               pane_layout_undo: Vec::new(),
               pane_layout_redo: Vec::new(),
               pane_ids: Vec::new(),
               next_pane_id: 1, // 0 reserved as sentinel

               cmd_palette: CmdPaletteState::default(),
               layout_favorites: vec!["1".into(), "2".into(), "2H".into(), "3".into(), "4".into()],
               layout_dropdown_open: false, layout_dropdown_pos: egui::Pos2::ZERO, dragging_tab: None,
               timeframe: TimeframeState::default(),
               pending_overlay_add: false,
               pane_templates: vec![], pane_template_name: String::new(),
               portfolio_templates: vec!["Default".into()],
               dashboard_templates: vec!["Default".into()],
               heatmap_templates: vec!["Default".into()],
               spreadsheet_templates: vec!["Default".into()],
               plays: vec![], feed: vec![], feed_filter_symbol: String::new(),
               account_size: 100000.0, risk_pct: 0.01,
               play_editor: PlayEditorState::default(),
               play_templates: vec![],
               widget_presets: vec![], widget_preset_name: String::new(),
               discord: DiscordState::default(),
               tape: TapeState::default(),
               news: NewsState::default(),
               journal_open: false,
               scanner: ScannerState::default(),
               heatmap: HeatmapState::default(),
               spread_open: false, maximized_pane: None,
               spread_state: super::ui::panels::spread_panel::SpreadState::default(),
               script: ScriptState::default(),
               screenshot_open: false,
               screenshot_entries: super::ui::panels::screenshot_panel::load_screenshots(),
               charts_library_open: false,
               rrg: RrgState::default(),
               analysis: AnalysisState::default(),
               auto_chart_open: false,
               signals_panel: SignalsPanelState::default(),
               indicators: IndicatorsState::default(),
               feed_panel: FeedPanelState::default(),
               playbook_panel_open: false,
               journal_panel: JournalPanelState::default(),
               book_tab: crate::chart_renderer::BookTab::Book,
               provenance: ProvenanceState::default(),
               // Wave 12c: queue-backed bus. Publishers push events; the
               // render loop (`App::about_to_wait`) drains and applies them
               // to sibling panes once per frame. See `state::subscriptions`
               // for the model description and group sentinel.
               subscriptions: crate::state::SubscriptionBus::new(),
               inflight: crate::state::InFlightRegistry::new(),
               ui_settings: crate::state::UiSettings::default(),
               ui_settings_store: crate::state::Store::new(
                   "ui_settings",
                   crate::state::UiSettings::default(),
                   Some(ui_settings_path()),
               ),
               trading_defaults_store: crate::state::Store::new(
                   "trading_defaults",
                   crate::state::TradingDefaults::default(),
                   Some(trading_defaults_path()),
               ),
               alerts_store: crate::state::Store::new(
                   "alerts_state",
                   crate::state::AlertsState::default(),
                   Some(alerts_state_path()),
               ),
               sidebar_state_store: crate::state::Store::new(
                   "sidebar_state",
                   crate::state::SidebarState::default(),
                   Some(sidebar_state_path()),
               ),
               layout_state_store: crate::state::Store::new(
                   "layout_state",
                   crate::state::LayoutState::default(),
                   Some(layout_state_path()),
               ),
               chat_state_store: crate::state::Store::new(
                   "chat_state",
                   crate::state::ChatState::default(),
                   Some(chat_state_path()),
               ),
               top_nav_sym_input: String::new(),
               top_nav_sym_focused: false,
               // Welcome wizard is initialized after load (when ui_settings is populated).
               // See `init_welcome_wizard()` called after `pull_from_ui_settings()`.
               welcome_wizard: None,
        }
    }

    /// Wave 14c: copy legacy display-pref fields into the
    /// `ui_settings` aggregate. Call this immediately before persisting
    /// so the serialized aggregate matches what reads see in the live
    /// `Watchlist`. The legacy fields stay the authoritative read source
    /// for now (sacred `core.rs` reads `pane_header_size`,
    /// `shared_x_axis`, etc. directly).
    ///
    /// Wave 2 addendum: also writes the merged value into
    /// `ui_settings_store` so the persist supervisor sees the latest state
    /// in addition to the one-shot save in `save_state`.
    pub(crate) fn push_to_ui_settings(&mut self) {
        self.ui_settings.font_scale = self.font_scale;
        self.ui_settings.font_idx = self.font_idx;
        self.ui_settings.compact_mode = self.compact_mode;
        self.ui_settings.pane_header_size = self.pane_header_size;
        self.ui_settings.toolbar_auto_hide = self.toolbar_auto_hide;
        self.ui_settings.show_x_axis = self.show_x_axis;
        self.ui_settings.show_y_axis = self.show_y_axis;
        self.ui_settings.shared_x_axis = self.shared_x_axis;
        self.ui_settings.shared_y_axis = self.shared_y_axis;
        self.ui_settings.style_idx = self.style_idx;
        // Propagate into the store so the supervisor can persist it as well.
        let snapshot = self.ui_settings.clone();
        self.ui_settings_store.update(|s| *s = snapshot);
    }

    /// P2: Initialise the welcome wizard from the loaded `ui_settings`.
    /// Call this immediately after `pull_from_ui_settings()` at startup.
    /// If `has_seen_welcome` is false, a new wizard is created at the
    /// persisted resume step; otherwise `welcome_wizard` stays `None`.
    pub(crate) fn init_welcome_wizard(&mut self) {
        if !self.ui_settings.has_seen_welcome {
            self.welcome_wizard = Some(
                crate::chart_renderer::ui::welcome::WelcomeWizard::from_settings(
                    false,
                    self.ui_settings.welcome_step_resume,
                )
            );
        }
    }

    /// Wave 14c: copy the loaded `ui_settings` aggregate back onto the
    /// legacy `Watchlist` fields. Call this after a successful
    /// `Persistable::load` so existing readers (UI panels and the
    /// sacred `core.rs` paint pipeline) observe the restored values
    /// through their familiar field names.
    pub(crate) fn pull_from_ui_settings(&mut self) {
        self.font_scale = self.ui_settings.font_scale;
        self.font_idx = self.ui_settings.font_idx;
        self.compact_mode = self.ui_settings.compact_mode;
        self.pane_header_size = self.ui_settings.pane_header_size;
        self.toolbar_auto_hide = self.ui_settings.toolbar_auto_hide;
        self.show_x_axis = self.ui_settings.show_x_axis;
        self.show_y_axis = self.ui_settings.show_y_axis;
        self.shared_x_axis = self.ui_settings.shared_x_axis;
        self.shared_y_axis = self.ui_settings.shared_y_axis;
        self.style_idx = self.ui_settings.style_idx;
        // Keep the store in sync with the freshly-loaded values so the
        // persist supervisor starts from the restored state, not the Default.
        self.ui_settings_store.update(|s| *s = self.ui_settings.clone());
    }

    // ── Wave 2 (state): Store<UiSettings> accessor / mutator ─────────────────

    /// Read the current `UiSettings` from the store.
    /// The returned guard holds a read lock — release it before calling
    /// `update_ui_settings` to avoid a deadlock.
    pub(crate) fn ui_settings_snapshot(
        &self,
    ) -> parking_lot::RwLockReadGuard<crate::state::UiSettings> {
        self.ui_settings_store.read()
    }

    /// Mutate `UiSettings` through the store.
    ///
    /// The store bumps its version counter and starts the debounce clock;
    /// the background persist supervisor will write to disk within
    /// `state::DEBOUNCE_MS` + `state::PERSIST_TICK_MS` (~250ms).
    ///
    /// The plain `ui_settings` mirror is updated in-place so the legacy
    /// serialization path (`push_to_ui_settings` / `save_state`) and the
    /// `init_welcome_wizard` logic continue to see the latest values.
    pub(crate) fn update_ui_settings(&mut self, f: impl FnOnce(&mut crate::state::UiSettings)) {
        self.ui_settings_store.update(|s| f(s));
        // Mirror the store's new value into the plain field used by the
        // legacy load/save path and `init_welcome_wizard`.
        self.ui_settings = self.ui_settings_store.read().clone();
    }

    // ── Wave 2 (state): Store<TradingDefaults> accessor / mutator ────────────

    /// Read the current `TradingDefaults` from the store.
    pub(crate) fn trading_defaults_snapshot(
        &self,
    ) -> parking_lot::RwLockReadGuard<crate::state::TradingDefaults> {
        self.trading_defaults_store.read()
    }

    /// Mutate `TradingDefaults` through the store.
    ///
    /// The store bumps its version counter and starts the debounce clock;
    /// the background persist supervisor writes to disk within ~250ms.
    ///
    /// The flat legacy fields (`default_stock_qty`, etc.) on Watchlist are
    /// kept in sync so existing callers that read them directly continue to
    /// see the latest values. `daily_loss_cap` and `max_position_pct` have
    /// no legacy counterpart — they live only in the store.
    pub(crate) fn update_trading_defaults(&mut self, f: impl FnOnce(&mut crate::state::TradingDefaults)) {
        self.trading_defaults_store.update(|s| f(s));
        self.sync_trading_defaults_from_store();
    }

    /// Push flat legacy fields → `trading_defaults_store`.
    ///
    /// Called from `settings_panel` after any mutation to the flat fields so
    /// the store stays in sync and the persist supervisor can write to disk.
    pub(crate) fn push_to_trading_defaults_store(&mut self) {
        let order_type = match self.default_order_type {
            0 => crate::state::DefaultOrderType::Market,
            1 => crate::state::DefaultOrderType::Limit,
            2 => crate::state::DefaultOrderType::Stop,
            _ => crate::state::DefaultOrderType::StopLimit,
        };
        let tif = match self.default_tif {
            0 => crate::state::DefaultTimeInForce::Day,
            1 => crate::state::DefaultTimeInForce::Gtc,
            2 => crate::state::DefaultTimeInForce::Ioc,
            _ => crate::state::DefaultTimeInForce::Fok,
        };
        let qty = self.default_stock_qty;
        let opts_qty = self.default_options_qty;
        let rth = self.default_outside_rth;
        self.trading_defaults_store.update(|s| {
            s.default_stock_qty   = qty;
            s.default_options_qty = opts_qty;
            s.default_order_type  = order_type;
            s.default_tif         = tif;
            s.default_outside_rth = rth;
        });
    }

    /// Copy the store's current `TradingDefaults` into the flat legacy fields.
    /// Called after `update_trading_defaults()` and at load time.
    pub(crate) fn sync_trading_defaults_from_store(&mut self) {
        let snap = self.trading_defaults_store.read().clone();
        self.default_stock_qty   = snap.default_stock_qty;
        self.default_options_qty = snap.default_options_qty;
        // Map typed enum → legacy usize index (0=MKT,1=LMT,2=STP,3=STPLMT)
        self.default_order_type  = match snap.default_order_type {
            crate::state::DefaultOrderType::Market    => 0,
            crate::state::DefaultOrderType::Limit     => 1,
            crate::state::DefaultOrderType::Stop      => 2,
            crate::state::DefaultOrderType::StopLimit => 3,
        };
        // Map typed enum → legacy usize index (0=DAY,1=GTC,2=IOC,3=FOK)
        self.default_tif         = match snap.default_tif {
            crate::state::DefaultTimeInForce::Day => 0,
            crate::state::DefaultTimeInForce::Gtc => 1,
            crate::state::DefaultTimeInForce::Ioc => 2,
            crate::state::DefaultTimeInForce::Fok => 3,
        };
        self.default_outside_rth = snap.default_outside_rth;
    }

    // ── Wave 3 (state): Store<AlertsState> accessor / mutator ────────────────

    /// Read the current `AlertsState` from the store.
    /// The returned guard holds a read lock — release it before calling
    /// `update_alerts_state` to avoid a deadlock.
    pub(crate) fn alerts_state_snapshot(
        &self,
    ) -> parking_lot::RwLockReadGuard<crate::state::AlertsState> {
        self.alerts_store.read()
    }

    /// Mutate `AlertsState` through the store.
    ///
    /// The store bumps its version counter and starts the debounce clock;
    /// the background persist supervisor will write to disk within ~250ms.
    ///
    /// The flat legacy fields (`alerts`, `next_alert_id`, `alert_query`,
    /// `alerts_panel_open`) on Watchlist are kept in sync so existing
    /// callers that read them directly continue to see the latest values.
    pub(crate) fn update_alerts_state(&mut self, f: impl FnOnce(&mut crate::state::AlertsState)) {
        self.alerts_store.update(|s| f(s));
        self.sync_from_alerts_store();
    }

    /// Push flat legacy fields → `alerts_store`.
    ///
    /// Call this after any batch mutation to the flat alert fields so
    /// the store stays in sync and the persist supervisor can write to disk.
    pub(crate) fn push_to_alerts_store(&mut self) {
        let persisted: Vec<crate::state::PersistedAlert> = self.alerts.iter().map(|a| {
            crate::state::PersistedAlert {
                id: a.id,
                symbol: a.symbol.clone(),
                price: a.price,
                above: a.above,
                triggered: a.triggered,
                message: a.message.clone(),
            }
        }).collect();
        let next_id = self.next_alert_id;
        let query = self.alert_query.clone();
        let panel_open = self.alerts_panel_open;
        self.alerts_store.update(|s| {
            s.alerts = persisted;
            s.next_alert_id = next_id;
            s.alert_query = query;
            s.alerts_panel_open = panel_open;
        });
    }

    /// Copy the store's current `AlertsState` into the flat legacy fields.
    /// Called after `update_alerts_state()` and at load time.
    pub(crate) fn sync_from_alerts_store(&mut self) {
        let snap = self.alerts_store.read().clone();
        self.alerts = snap.alerts.iter().map(|pa| {
            crate::chart_renderer::trading::Alert {
                id: pa.id,
                symbol: pa.symbol.clone(),
                price: pa.price,
                above: pa.above,
                triggered: pa.triggered,
                message: pa.message.clone(),
            }
        }).collect();
        self.next_alert_id = snap.next_alert_id;
        self.alert_query = snap.alert_query;
        self.alerts_panel_open = snap.alerts_panel_open;
    }

    // ── Wave 3 (state): Store<SidebarState> accessor / mutator ──────────────

    /// Read the current `SidebarState` from the store.
    /// The returned guard holds a read lock — release it before calling
    /// `update_sidebar_state` to avoid a deadlock.
    pub(crate) fn sidebar_state_snapshot(
        &self,
    ) -> parking_lot::RwLockReadGuard<crate::state::SidebarState> {
        self.sidebar_state_store.read()
    }

    /// Mutate `SidebarState` through the store.
    ///
    /// The store bumps its version counter and starts the debounce clock;
    /// the background persist supervisor will write to disk within ~250ms.
    ///
    /// The flat legacy fields on Watchlist are kept in sync so existing
    /// callers that read them directly continue to see the latest values.
    pub(crate) fn update_sidebar_state(&mut self, f: impl FnOnce(&mut crate::state::SidebarState)) {
        self.sidebar_state_store.update(|s| f(s));
        self.sync_from_sidebar_store();
    }

    /// Push flat legacy fields → `sidebar_state_store`.
    ///
    /// Call this after any batch mutation to the flat sidebar fields so
    /// the store stays in sync and the persist supervisor can write to disk.
    pub(crate) fn push_to_sidebar_store(&mut self) {
        let open = self.open;
        let settings_open = self.settings_open;
        let orders_panel_open = self.orders_panel_open;
        let order_entry_open = self.order_entry_open;
        let order_ledger_open = self.order_ledger.open;
        let order_ledger_view = self.order_ledger.view;
        let order_ledger_filter = self.order_ledger.filter;
        let order_health_open = self.order_health_open;
        let bottom_dock_tab = self.bottom_dock_tab;
        let rail_col_width = self.rail_col_width;
        let bottom_dock_height = self.bottom_dock_height;
        let account_strip_open = self.account_strip_open;
        let object_tree_open = self.object_tree_open;
        let trendline_filter_open = self.trendline_filter_open;
        let apex_diag_open = self.apex_diag_open;
        let widget_gallery_open = self.widget_gallery_open;
        let filter_open = self.filter_open;
        let wl_columns_open = self.wl_columns_open;
        let tape_open = self.tape.open;
        let news_open = self.news.open;
        let journal_open = self.journal_open;
        let scanner_open = self.scanner.open;
        let scanner_builder_open = self.scanner.builder_open;
        let spread_open = self.spread_open;
        let script_open = self.script.open;
        let screenshot_open = self.screenshot_open;
        let rrg_open = self.rrg.open;
        let analysis_open = self.analysis.open;
        let auto_chart_open = self.auto_chart_open;
        let signals_panel_open = self.signals_panel.open;
        let indicators_panel_open = self.indicators.panel_open;
        let indicators_section_fracs = self.indicators.section_fracs;
        let feed_panel_open = self.feed_panel.open;
        let playbook_panel_open = self.playbook_panel_open;
        let journal_panel_open = self.journal_panel.open;
        let provenance_open = self.provenance.open;
        let replay_pane_open = self.replay_pane_open;
        let hotkey_editor_open = self.hotkey_editor_open;
        self.sidebar_state_store.update(|s| {
            s.watchlist_open = open;
            s.settings_open = settings_open;
            s.orders_panel_open = orders_panel_open;
            s.order_entry_open = order_entry_open;
            s.order_ledger_open = order_ledger_open;
            s.order_ledger_view = order_ledger_view;
            s.order_ledger_filter = order_ledger_filter;
            s.order_health_open = order_health_open;
            s.bottom_dock_tab = bottom_dock_tab;
            s.rail_col_width = rail_col_width;
            s.bottom_dock_height = bottom_dock_height;
            s.account_strip_open = account_strip_open;
            s.object_tree_open = object_tree_open;
            s.trendline_filter_open = trendline_filter_open;
            s.apex_diag_open = apex_diag_open;
            s.widget_gallery_open = widget_gallery_open;
            s.filter_open = filter_open;
            s.wl_columns_open = wl_columns_open;
            s.tape_open = tape_open;
            s.news_open = news_open;
            s.journal_open = journal_open;
            s.scanner_open = scanner_open;
            s.scanner_builder_open = scanner_builder_open;
            s.spread_open = spread_open;
            s.script_open = script_open;
            s.screenshot_open = screenshot_open;
            s.rrg_open = rrg_open;
            s.analysis_open = analysis_open;
            s.auto_chart_open = auto_chart_open;
            s.signals_panel_open = signals_panel_open;
            s.indicators_panel_open = indicators_panel_open;
            s.indicators_section_fracs = indicators_section_fracs;
            s.feed_panel_open = feed_panel_open;
            s.playbook_panel_open = playbook_panel_open;
            s.journal_panel_open = journal_panel_open;
            s.provenance_open = provenance_open;
            s.replay_pane_open = replay_pane_open;
            s.hotkey_editor_open = hotkey_editor_open;
        });
    }

    /// Copy the store's current `SidebarState` into the flat legacy fields.
    /// Called after `update_sidebar_state()` and at load time.
    pub(crate) fn sync_from_sidebar_store(&mut self) {
        let snap = self.sidebar_state_store.read().clone();
        self.open = snap.watchlist_open;
        self.settings_open = snap.settings_open;
        self.orders_panel_open = snap.orders_panel_open;
        self.order_entry_open = snap.order_entry_open;
        self.order_ledger.open = snap.order_ledger_open;
        self.order_ledger.view = snap.order_ledger_view;
        self.order_ledger.filter = snap.order_ledger_filter;
        self.order_health_open = snap.order_health_open;
        self.bottom_dock_tab = snap.bottom_dock_tab;
        self.rail_col_width = snap.rail_col_width;
        self.bottom_dock_height = snap.bottom_dock_height;
        self.account_strip_open = snap.account_strip_open;
        self.object_tree_open = snap.object_tree_open;
        self.trendline_filter_open = snap.trendline_filter_open;
        self.apex_diag_open = snap.apex_diag_open;
        self.widget_gallery_open = snap.widget_gallery_open;
        self.filter_open = snap.filter_open;
        self.wl_columns_open = snap.wl_columns_open;
        self.tape.open = snap.tape_open;
        self.news.open = snap.news_open;
        self.journal_open = snap.journal_open;
        self.scanner.open = snap.scanner_open;
        self.scanner.builder_open = snap.scanner_builder_open;
        self.spread_open = snap.spread_open;
        self.script.open = snap.script_open;
        self.screenshot_open = snap.screenshot_open;
        self.rrg.open = snap.rrg_open;
        self.analysis.open = snap.analysis_open;
        self.auto_chart_open = snap.auto_chart_open;
        self.signals_panel.open = snap.signals_panel_open;
        self.indicators.panel_open = snap.indicators_panel_open;
        self.indicators.section_fracs = snap.indicators_section_fracs;
        self.feed_panel.open = snap.feed_panel_open;
        self.playbook_panel_open = snap.playbook_panel_open;
        self.journal_panel.open = snap.journal_panel_open;
        self.provenance.open = snap.provenance_open;
        self.replay_pane_open = snap.replay_pane_open;
        self.hotkey_editor_open = snap.hotkey_editor_open;
    }

    // ── Wave 3 (state): Store<LayoutState> accessor / mutator ───────────────

    /// Push flat legacy fields → `layout_state_store`.
    ///
    /// Call this after any batch mutation to the flat layout fields so
    /// the store stays in sync and the persist supervisor can write to disk.
    pub(crate) fn push_to_layout_store(&mut self) {
        let link_groups: Vec<crate::state::PersistedLinkGroup> = self.link_groups.iter().map(|g| {
            let [r, g2, b, a] = g.color.to_array();
            crate::state::PersistedLinkGroup { name: g.name.clone(), color_rgba: [r, g2, b, a] }
        }).collect();
        let broadcast_mode = self.broadcast_mode;
        let pane_split_h = self.pane_split.h;
        let pane_split_v = self.pane_split.v;
        let pane_split_h2 = self.pane_split.h2;
        let pane_split_v2 = self.pane_split.v2;
        let pane_split_v3 = self.pane_split.v3;
        let pane_split_v4 = self.pane_split.v4;
        let pane_split_v5 = self.pane_split.v5;
        let pane_split_v6 = self.pane_split.v6;
        // P16 fix #1 — persist the PaneGrid tree if present.
        let pane_layout_clone = self.pane_layout.clone();
        let layout_favorites = self.layout_favorites.clone();
        let timeframe_favorites = self.timeframe.favorites.clone();
        let maximized_pane = self.maximized_pane;
        let pane_template_names: Vec<String> = self.pane_templates.iter().map(|(n, _)| n.clone()).collect();
        let portfolio_templates = self.portfolio_templates.clone();
        let dashboard_templates = self.dashboard_templates.clone();
        let heatmap_templates = self.heatmap_templates.clone();
        let spreadsheet_templates = self.spreadsheet_templates.clone();
        let active_workspace = self.workspace.active.clone();
        let workspace_save_name = self.workspace.save_name.clone();
        self.layout_state_store.update(|s| {
            s.link_groups = link_groups;
            s.broadcast_mode = broadcast_mode;
            s.pane_split_h = pane_split_h;
            s.pane_split_v = pane_split_v;
            s.pane_split_h2 = pane_split_h2;
            s.pane_split_v2 = pane_split_v2;
            s.pane_split_v3 = pane_split_v3;
            s.pane_split_v4 = pane_split_v4;
            s.pane_split_v5 = pane_split_v5;
            s.pane_split_v6 = pane_split_v6;
            s.pane_layout = pane_layout_clone;
            s.layout_favorites = layout_favorites;
            s.timeframe_favorites = timeframe_favorites;
            s.maximized_pane = maximized_pane;
            s.pane_template_names = pane_template_names;
            s.portfolio_templates = portfolio_templates;
            s.dashboard_templates = dashboard_templates;
            s.heatmap_templates = heatmap_templates;
            s.spreadsheet_templates = spreadsheet_templates;
            s.active_workspace = active_workspace;
            s.workspace_save_name = workspace_save_name;
        });
    }

    /// Copy the store's current `LayoutState` into the flat legacy fields.
    /// Called at load time after `layout_state_store` is seeded from disk.
    pub(crate) fn sync_from_layout_store(&mut self) {
        let snap = self.layout_state_store.read().clone();
        self.link_groups = snap.link_groups.iter().map(|g| LinkGroup {
            name: g.name.clone(),
            color: egui::Color32::from_rgba_unmultiplied(
                g.color_rgba[0], g.color_rgba[1], g.color_rgba[2], g.color_rgba[3],
            ),
        }).collect();
        self.broadcast_mode = snap.broadcast_mode;
        self.pane_split.h = snap.pane_split_h;
        self.pane_split.v = snap.pane_split_v;
        self.pane_split.h2 = snap.pane_split_h2;
        self.pane_split.v2 = snap.pane_split_v2;
        self.pane_split.v3 = snap.pane_split_v3;
        self.pane_split.v4 = snap.pane_split_v4;
        self.pane_split.v5 = snap.pane_split_v5;
        self.pane_split.v6 = snap.pane_split_v6;
        // P16 fix #1 — restore the saved tree if present. Older workspaces
        // omit pane_layout (#[serde(default)]) so loading them yields None,
        // and ensure_pane_layout will materialize from the legacy template.
        self.pane_layout = snap.pane_layout.clone();
        self.layout_favorites = snap.layout_favorites;
        self.timeframe.favorites = snap.timeframe_favorites;
        self.maximized_pane = snap.maximized_pane;
        // pane_template_names → pane_templates: names are restored; payloads
        // are loaded separately by load_templates(). We only restore names that
        // already exist in the loaded templates; orphaned names are dropped.
        // (pane_templates is already populated by load_templates() before this
        // call, so we do not overwrite it — template names are authoritative
        // from load_templates().)
        self.portfolio_templates = snap.portfolio_templates;
        self.dashboard_templates = snap.dashboard_templates;
        self.heatmap_templates = snap.heatmap_templates;
        self.spreadsheet_templates = snap.spreadsheet_templates;
        self.workspace.active = snap.active_workspace;
        self.workspace.save_name = snap.workspace_save_name;
    }

    // ── Wave 3 (state): Store<ChatState> accessor / mutator ─────────────────

    /// Push flat legacy fields → `chat_state_store`.
    ///
    /// Call this after any batch mutation to the flat discord/chat fields so
    /// the store stays in sync and the persist supervisor can write to disk.
    pub(crate) fn push_to_chat_store(&mut self) {
        let discord_open = self.discord.open;
        let discord_input = self.discord.input.clone();
        let discord_channel = self.discord.channel.clone();
        let discord_authenticated = self.discord.authenticated;
        let discord_username = self.discord.username.clone();
        let discord_user_id = self.discord.user_id.clone();
        let discord_selected_guild = self.discord.selected_guild.clone();
        let discord_selected_channel = self.discord.selected_channel.clone();
        let discord_last_msg_id = self.discord.last_msg_id.clone();
        self.chat_state_store.update(|s| {
            s.discord_open = discord_open;
            s.discord_input = discord_input;
            s.discord_channel = discord_channel;
            s.discord_authenticated = discord_authenticated;
            s.discord_username = discord_username;
            s.discord_user_id = discord_user_id;
            s.discord_selected_guild = discord_selected_guild;
            s.discord_selected_channel = discord_selected_channel;
            s.discord_last_msg_id = discord_last_msg_id;
        });
    }

    /// Copy the store's current `ChatState` into the flat legacy fields.
    /// Called at load time after `chat_state_store` is seeded from disk.
    pub(crate) fn sync_from_chat_store(&mut self) {
        let snap = self.chat_state_store.read().clone();
        self.discord.open = snap.discord_open;
        self.discord.input = snap.discord_input;
        self.discord.channel = snap.discord_channel;
        self.discord.authenticated = snap.discord_authenticated;
        self.discord.username = snap.discord_username;
        self.discord.user_id = snap.discord_user_id;
        self.discord.selected_guild = snap.discord_selected_guild;
        self.discord.selected_channel = snap.discord_selected_channel;
        self.discord.last_msg_id = snap.discord_last_msg_id;
    }

    // ── Phase 3 (state): single-chokepoint flush helper ──────────────────────

    /// Push ALL live `Watchlist` legacy fields into their typed stores.
    ///
    /// Call this immediately before any persist point (manual save or
    /// `flush_all`) so every store is guaranteed to hold the freshest
    /// values.  Adding a new aggregate in the future means adding ONE
    /// line here — no other persist site needs to change.
    pub(crate) fn push_all_stores(&mut self) {
        self.push_to_ui_settings();
        self.push_to_trading_defaults_store();
        self.push_to_alerts_store();
        self.push_to_sidebar_store();
        self.push_to_layout_store();
        self.push_to_chat_store();
    }

    /// Add symbol to the last section (creates one if none exist).
    pub(crate) fn add_symbol(&mut self, sym: &str) {
        let s = sym.to_uppercase();
        // Check all sections for duplicates
        if self.sections.iter().any(|sec| sec.items.iter().any(|i| i.symbol == s)) { return; }
        // Find the last non-option section, or create one
        let target = self.sections.iter().rposition(|sec| !sec.title.contains("Options"));
        let target_idx = if let Some(idx) = target {
            idx
        } else {
            let id = self.next_section_id; self.next_section_id += 1;
            self.sections.insert(0, WatchlistSection { id, title: String::new(), color: None, collapsed: false, items: vec![] });
            0
        };
        // Use symbol hash for a pseudo-random rvol so rows look varied in dev
        let sym_hash = s.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
        let rvol_seed = 1.0; // neutral until real RVOL is wired (was: hash-seeded random masquerading as data)
        self.sections[target_idx].items.push(WatchlistItem {
            symbol: s, price: 0.0, prev_close: 0.0, day_close: 0.0, change_perc: None, stale: false, loaded: false,
            is_option: false, underlying: String::new(), option_type: String::new(), strike: 0.0, expiry: String::new(), bid: 0.0, ask: 0.0,
            pinned: false, tags: vec![], rvol: rvol_seed, atr: 0.0,
            high_52wk: 0.0, low_52wk: 0.0, day_high: 0.0, day_low: 0.0,
            avg_daily_range: 2.0, earnings_days: -1, alert_triggered: false, price_history: vec![],
            prev_price: 0.0, price_change_at: None,
        });
    }

    /// Remove symbol from all sections.
    pub(crate) fn remove_symbol(&mut self, sym: &str) {
        for sec in &mut self.sections {
            sec.items.retain(|i| i.symbol != sym);
        }
    }

    pub(crate) fn set_price(&mut self, sym: &str, price: f32) {
        for sec in &mut self.sections {
            if let Some(item) = sec.items.iter_mut().find(|i| i.symbol == sym) {
                // Flash animation: capture prev price and timestamp on actual change,
                // but only if the item had a prior non-zero price (not initial load).
                if (item.price - price).abs() > f32::EPSILON && item.price > 0.0 {
                    item.prev_price = item.price;
                    item.price_change_at = Some(std::time::Instant::now());
                }
                item.price = price;
                item.price_history.push(price);
                if item.price_history.len() > 30 { item.price_history.remove(0); }
            }
        }
    }

    /// Set the real intraday high/low from a live snapshot. Zero-guarded so an
    /// off-hours/empty snapshot doesn't clobber a previously-good range.
    pub(crate) fn set_day_range(&mut self, sym: &str, high: f32, low: f32) {
        for sec in &mut self.sections {
            if let Some(item) = sec.items.iter_mut().find(|i| i.symbol == sym) {
                if high > 0.0 { item.day_high = high; }
                if low > 0.0 { item.day_low = low; }
            }
        }
    }

    pub(crate) fn set_prev_close(&mut self, sym: &str, prev_close: f32) {
        for sec in &mut self.sections {
            if let Some(item) = sec.items.iter_mut().find(|i| i.symbol == sym) {
                item.prev_close = prev_close;
                item.loaded = true;
            }
        }
    }

    /// Today's regular-session close from the bulk snapshot. Zero-guarded so a
    /// live/pre-open snapshot (day.c == 0) doesn't wipe a good close.
    pub(crate) fn set_day_close(&mut self, sym: &str, day_close: f32) {
        if day_close <= 0.0 { return; }
        for sec in &mut self.sections {
            if let Some(item) = sec.items.iter_mut().find(|i| i.symbol == sym) {
                item.day_close = day_close;
            }
        }
    }

    /// Set the server-computed session/DST-aware % change (apex-data-v2). When
    /// `Some`, the watchlist renders this directly instead of recomputing.
    /// `None` (older backend) leaves any prior value in place so a transient
    /// missing field doesn't blank the row.
    pub(crate) fn set_change_perc(&mut self, sym: &str, change_perc: Option<f32>) {
        let Some(p) = change_perc else { return };
        for sec in &mut self.sections {
            if let Some(item) = sec.items.iter_mut().find(|i| i.symbol == sym) {
                item.change_perc = Some(p);
            }
        }
    }

    /// Mark whether `sym`'s latest snapshot was last-good cache (stale) vs fresh.
    /// Set unconditionally so a recovered row clears its stale mark.
    pub(crate) fn set_stale(&mut self, sym: &str, stale: bool) {
        for sec in &mut self.sections {
            if let Some(item) = sec.items.iter_mut().find(|i| i.symbol == sym) {
                item.stale = stale;
            }
        }
    }

    /// Look up live change% for `sym` from the watchlist's loaded items
    /// (returns None if the symbol isn't in the watchlist or has no prev_close).
    pub(crate) fn get_change_pct(&self, sym: &str) -> Option<f32> {
        for sec in &self.sections {
            if let Some(item) = sec.items.iter().find(|i| i.symbol == sym) {
                if item.loaded && item.prev_close > 0.0 {
                    return Some((item.price - item.prev_close) / item.prev_close * 100.0);
                }
            }
        }
        None
    }

    /// Current live price for a symbol from any watchlist row (0/unloaded → None).
    pub(crate) fn get_price(&self, sym: &str) -> Option<f32> {
        for sec in &self.sections {
            if let Some(item) = sec.items.iter().find(|i| i.symbol.eq_ignore_ascii_case(sym)) {
                if item.loaded && item.price > 0.0 { return Some(item.price); }
            }
        }
        None
    }

    /// Collect all symbols across all sections.
    fn all_symbols(&self) -> Vec<String> {
        self.sections.iter().flat_map(|s| s.items.iter().map(|i| i.symbol.clone())).collect()
    }

    /// Find an item by symbol across all sections.
    pub(crate) fn find_item(&self, sym: &str) -> Option<&WatchlistItem> {
        self.sections.iter().flat_map(|s| s.items.iter()).find(|i| i.symbol == sym)
    }

    /// Add a new empty section (stocks area — inserted before any options sections).
    pub(crate) fn add_section(&mut self, title: &str) {
        let id = self.next_section_id; self.next_section_id += 1;
        let new_sec = WatchlistSection { id, title: title.to_string(), color: None, collapsed: false, items: vec![] };
        // Insert before the first options section (so new sections go in the stocks area)
        let first_opt = self.sections.iter().position(|s| s.title.contains("Options"));
        if let Some(pos) = first_opt {
            self.sections.insert(pos, new_sec);
        } else {
            self.sections.push(new_sec);
        }
    }

    /// Add a new empty section in the options area (title contains "Options").
    pub(crate) fn add_option_section(&mut self, title: &str) {
        let id = self.next_section_id; self.next_section_id += 1;
        let full_title = if title.contains("Options") { title.to_string() } else { format!("{} Options", title) };
        let new_sec = WatchlistSection { id, title: full_title, color: None, collapsed: false, items: vec![] };
        self.sections.push(new_sec);
    }

    /// Add an option contract to the "Options" section (auto-creates if needed).
    /// Returns false if already present (duplicate check by symbol string).
    pub(crate) fn add_option_to_watchlist(&mut self, underlying: &str, strike: f32, is_call: bool, expiry: &str, bid: f32, ask: f32) -> bool {
        let type_str = if is_call { "C" } else { "P" };
        let strike_str = if (strike - strike.round()).abs() < 0.005 { format!("{:.0}", strike) } else { format!("{:.1}", strike) };
        let opt_sym = format!("{} {}{} {}", underlying, strike_str, type_str, expiry);
        // Duplicate check across all sections
        if self.sections.iter().any(|sec| sec.items.iter().any(|i| i.symbol == opt_sym)) {
            return false;
        }
        // Find or create section named after underlying (e.g. "SPY Options")
        let section_title = format!("{} Options", underlying);
        let sec_idx = if let Some(idx) = self.sections.iter().position(|s| s.title == section_title) {
            idx
        } else {
            let id = self.next_section_id; self.next_section_id += 1;
            self.sections.push(WatchlistSection {
                id, title: section_title, color: None, collapsed: false, items: vec![],
            });
            self.sections.len() - 1
        };
        self.sections[sec_idx].items.push(WatchlistItem {
            symbol: opt_sym, price: 0.0, prev_close: 0.0, day_close: 0.0, change_perc: None, stale: false, loaded: false,
            is_option: true, underlying: underlying.to_string(), option_type: type_str.to_string(), strike, expiry: expiry.to_string(), bid, ask,
            pinned: false, tags: vec![], rvol: 1.0, atr: 0.0,
            high_52wk: 0.0, low_52wk: 0.0, day_high: 0.0, day_low: 0.0,
            avg_daily_range: 2.0, earnings_days: -1, alert_triggered: false, price_history: vec![],
            prev_price: 0.0, price_change_at: None,
        });
        true
    }

    /// Move an item from (src_sec, src_idx) to (dst_sec, dst_idx).
    pub(crate) fn move_item(&mut self, src_sec: usize, src_idx: usize, dst_sec: usize, dst_idx: usize) {
        if src_sec >= self.sections.len() { return; }
        if src_idx >= self.sections[src_sec].items.len() { return; }
        let item = self.sections[src_sec].items.remove(src_idx);
        let dst_sec = dst_sec.min(self.sections.len() - 1);
        let clamped = dst_idx.min(self.sections[dst_sec].items.len());
        self.sections[dst_sec].items.insert(clamped, item);
    }

    /// Sync current live sections back into saved_watchlists at active index.
    fn sync_to_saved(&mut self) {
        if self.active_watchlist_idx < self.saved_watchlists.len() {
            self.saved_watchlists[self.active_watchlist_idx].sections = self.sections.clone();
            self.saved_watchlists[self.active_watchlist_idx].next_section_id = self.next_section_id;
        }
    }

    /// Save current state and persist to disk.
    pub(crate) fn persist(&mut self) {
        self.sync_to_saved();
        save_watchlists(self);
    }

    /// Persist the playbook to disk. Call after any real (user-driven) play
    /// mutation — create / edit / delete / activate.
    pub(crate) fn persist_plays(&self) {
        save_plays(&self.plays);
    }

    /// Switch to a different watchlist by index. Returns symbols needing price fetch.
    pub(crate) fn switch_to(&mut self, idx: usize) -> Vec<String> {
        if idx >= self.saved_watchlists.len() || idx == self.active_watchlist_idx { return vec![]; }
        // Save current
        self.sync_to_saved();
        // Load new
        self.active_watchlist_idx = idx;
        let wl = &self.saved_watchlists[idx];
        self.sections = wl.sections.clone();
        self.next_section_id = wl.next_section_id;
        // Clear prices
        for sec in &mut self.sections {
            for item in &mut sec.items {
                item.price = 0.0;
                item.prev_close = 0.0;
                item.loaded = false;
            }
        }
        save_watchlists(self);
        self.all_symbols()
    }

    /// Create a new watchlist and switch to it. Returns symbols needing price fetch.
    pub(crate) fn create_watchlist(&mut self, name: &str) -> Vec<String> {
        self.sync_to_saved();
        let new_wl = SavedWatchlist {
            name: name.to_string(),
            sections: vec![WatchlistSection { id: 1, title: String::new(), color: None, collapsed: false, items: vec![] }],
            next_section_id: 2,
        };
        self.saved_watchlists.push(new_wl);
        let new_idx = self.saved_watchlists.len() - 1;
        self.switch_to(new_idx)
    }

    /// Duplicate watchlist at given index. Returns symbols needing price fetch.
    pub(crate) fn duplicate_watchlist(&mut self, idx: usize) -> Vec<String> {
        if idx >= self.saved_watchlists.len() { return vec![]; }
        self.sync_to_saved();
        let mut dup = self.saved_watchlists[idx].clone();
        dup.name = format!("{} (copy)", dup.name);
        self.saved_watchlists.push(dup);
        let new_idx = self.saved_watchlists.len() - 1;
        self.switch_to(new_idx)
    }

    /// Delete watchlist at given index (only if more than 1 exists). Returns symbols needing price fetch if active changed.
    pub(crate) fn delete_watchlist(&mut self, idx: usize) -> Vec<String> {
        if self.saved_watchlists.len() <= 1 || idx >= self.saved_watchlists.len() { return vec![]; }
        self.saved_watchlists.remove(idx);
        // Adjust active index
        if self.active_watchlist_idx == idx {
            let new_idx = if idx > 0 { idx - 1 } else { 0 };
            self.active_watchlist_idx = new_idx;
            let wl = &self.saved_watchlists[new_idx];
            self.sections = wl.sections.clone();
            self.next_section_id = wl.next_section_id;
            for sec in &mut self.sections {
                for item in &mut sec.items {
                    item.price = 0.0; item.prev_close = 0.0; item.loaded = false;
                }
            }
            save_watchlists(self);
            return self.all_symbols();
        } else if self.active_watchlist_idx > idx {
            self.active_watchlist_idx -= 1;
        }
        save_watchlists(self);
        vec![]
    }

}

// Black-Scholes, strike_interval, atm_strike, get_iv, sim_oi — now in compute.rs

pub(crate) fn default_hotkeys() -> Vec<HotKey> {
    let mut id = 1u32;
    let mut hk = |name: &str, cat: &str, action: &str, key: egui::Key, ctrl: bool, shift: bool, key_name: &str| -> HotKey {
        let h = HotKey { id, name: name.into(), category: cat.into(), action: action.into(), key_name: key_name.into(), key, ctrl, shift, alt: false };
        id += 1; h
    };
    vec![
        hk("Buy Market",         "Trading", "buy_market",     egui::Key::B,      true,  false, "Ctrl+B"),
        hk("Sell Market",        "Trading", "sell_market",    egui::Key::B,      true,  true,  "Ctrl+Shift+B"),
        hk("Cancel All Orders",  "Trading", "cancel_all",     egui::Key::Q,      true,  true,  "Ctrl+Shift+Q"),
        hk("Flatten Position",   "Trading", "flatten",        egui::Key::F,      true,  true,  "Ctrl+Shift+F"),
        hk("Kill Switch",        "Trading", "kill_switch",    egui::Key::K,      true,  true,  "Ctrl+Shift+K"),
        hk("Halt Trading",       "Trading", "halt_trading",   egui::Key::K,      true,  true,  "Ctrl+Shift+K"),
        hk("Resume Trading",     "Trading", "resume_trading", egui::Key::R,      true,  true,  "Ctrl+Shift+R"),
        hk("Trendline",          "Drawing", "tool_trendline", egui::Key::T,      false, false, "T"),
        hk("H-Line",             "Drawing", "tool_hline",     egui::Key::H,      false, false, "H"),
        hk("Fibonacci",          "Drawing", "tool_fibonacci", egui::Key::F,      false, false, "F"),
        hk("Channel",            "Drawing", "tool_channel",   egui::Key::C,      false, false, "C"),
        hk("Vertical Line",      "Drawing", "tool_vline",     egui::Key::V,      false, false, "V"),
        hk("Ray",                "Drawing", "tool_ray",       egui::Key::R,      false, false, "R"),
        hk("Zone",               "Drawing", "tool_hzone",     egui::Key::Z,      false, false, "Z"),
        hk("Pitchfork",          "Drawing", "tool_pitchfork", egui::Key::P,      false, false, "P"),
        hk("Gann Fan",           "Drawing", "tool_gannfan",   egui::Key::G,      false, false, "G"),
        hk("Fib Extension",      "Drawing", "tool_fibext",    egui::Key::X,      false, false, "X"),
        hk("Text Note",          "Drawing", "tool_textnote",  egui::Key::N,      false, false, "N"),
        hk("Toggle Magnet",      "Drawing", "toggle_magnet",  egui::Key::M,      false, false, "M"),
        hk("Undo",               "General", "undo",           egui::Key::Z,      true,  false, "Ctrl+Z"),
        hk("Redo",               "General", "redo",           egui::Key::Y,      true,  false, "Ctrl+Y"),
        hk("Duplicate",          "General", "duplicate",      egui::Key::D,      true,  false, "Ctrl+D"),
        hk("Screenshot",         "General", "screenshot",     egui::Key::S,      true,  true,  "Ctrl+Shift+S"),
        hk("Delete",             "General", "delete",         egui::Key::Delete, false, false, "Delete"),
        hk("Cancel / Deselect",  "General", "escape",         egui::Key::Escape, false, false, "Escape"),
        hk("Command Palette",    "General", "cmd_palette",    egui::Key::Space,  true,  false, "Ctrl+Space"),
        hk("TPS Reports",        "General", "tps_toggle",     egui::Key::H,      true,  true,  "⌘⇧H"),
    ]
}


// ─── Fetch / IO helpers (moved to io/fetch.rs) ────────────────────────────────
pub use super::io::fetch::fetch_bars_background_pub;
pub(crate) use super::io::fetch::{
    fetch_chain_background, refresh_chain_rest, fetch_overlay_chain_background,
    fetch_search_background, fetch_watchlist_prices, fetch_scanner_prices,
    SCANNER_UNIVERSE, active_zero_dte_date, apex_data_chain_to_tuples,
    fetch_indicator_source, submit_ib_order, fetch_option_history_background,
    fetch_history_background, fetch_drawings_background,
    synthesize_occ, fetch_option_bars_background, fetch_bars_background,
    fetch_overlay_bars_background, fetch_gamma_from_feed, refresh_gamma_feeds, fetch_corp_actions, ticker_detail_cached,
    options_analytics_cached, prev_session_change_cached, rvol_cached, futures_price_cached, vap_cached,
};




struct ChartWindow {
    id: winit::window::WindowId,
    win: Arc<Window>,
    gpu: GpuCtx,
    rx: mpsc::Receiver<ChartCommand>,
    panes: Vec<Chart>,
    active_pane: usize,
    layout: Layout,
    maximized_pane: Option<usize>, // Some(idx) = this pane is shown fullscreen
    close_requested: bool,
    watchlist: Watchlist,
    // Order execution toasts
    toasts: Vec<crate::chart_renderer::ui::tools::notification::Notification>,
    // Connection panel
    conn_panel_open: bool,
    // Auto-save timer
    last_save: Option<std::time::Instant>,
}

/// Request to spawn a new window (sent from Tauri command thread).
struct SpawnRequest {
    rx: mpsc::Receiver<ChartCommand>,
    initial_cmd: ChartCommand,
}

/// Top-level app managing multiple chart windows on a single EventLoop.
struct App {
    iw: u32, ih: u32,
    windows: Vec<ChartWindow>,
    /// Design-mode F12 inspector as a separate OS window (multi-monitor
    /// popout). Created on-demand when the user clicks POP. None when
    /// docked or not yet popped.
    #[cfg(feature = "design-mode")]
    inspector_window: Option<crate::chart::renderer::inspector_window::InspectorWindow>,
    spawn_rx: mpsc::Receiver<SpawnRequest>,
    /// Wave 2 (state): registry shared with the persist supervisor thread.
    /// All `Store<T>` instances created for the process lifetime are
    /// registered here so the supervisor can walk them every ~50ms.
    store_registry: std::sync::Arc<crate::state::StoreRegistry>,
    /// Wave 2 (state): handle that keeps the persist supervisor thread alive
    /// for the duration of the process. The thread loops until the OS
    /// cleans it up at exit; there is no shutdown channel by design.
    #[allow(dead_code)]
    persist_supervisor: std::thread::JoinHandle<()>,
}

struct GpuCtx {
    device: wgpu::Device, queue: wgpu::Queue,
    surface: wgpu::Surface<'static>, config: wgpu::SurfaceConfiguration,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    // Kept around so the inspector window (design-mode) can create its
    // own wgpu::Surface from the same Instance / Adapter / Device /
    // Queue — avoids spinning up a second adapter (wasteful + double-VRAM).
    // wgpu::Instance and wgpu::Adapter are Clone (cheap Arc internally
    // in wgpu 24).
    #[cfg(feature = "design-mode")]
    instance: wgpu::Instance,
    #[cfg(feature = "design-mode")]
    adapter: wgpu::Adapter,
    // Set to true when window loses focus — causes a PointerGone event to be injected
    // into the next frame so egui never stays stuck in drag state.
    pointer_gone_needed: bool,
    // GPU chart pipeline (SPEC_GPU_CHART_REFACTOR.md). Built at startup so the
    // WGSL is validated on every release; rendering is gated behind the
    // `gpu_chart_v2` feature flag.
    #[cfg_attr(not(feature = "gpu_chart_v2"), allow(dead_code))]
    chart_pipeline: crate::chart::renderer_gpu::ChartPipeline,
}

impl GpuCtx {
    fn new(window: Arc<Window>) -> Option<Self> {
        let size = window.inner_size();
        #[cfg(target_os = "windows")]
        let backends = wgpu::Backends::DX12;
        #[cfg(target_os = "macos")]
        let backends = wgpu::Backends::METAL;
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let backends = wgpu::Backends::VULKAN;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends, ..Default::default() });
        let surface = instance.create_surface(Arc::clone(&window)).ok()?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: Some(&surface), force_fallback_adapter: false,
        }))?;
        let mut required_features = wgpu::Features::empty();
        if adapter.features().contains(wgpu::Features::DUAL_SOURCE_BLENDING) {
            required_features |= wgpu::Features::DUAL_SOURCE_BLENDING;
            eprintln!("[gpu] DUAL_SOURCE_BLENDING enabled — subpixel-AA text path available");
        } else {
            eprintln!("[gpu] DUAL_SOURCE_BLENDING not supported — text will use grayscale AA");
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("chart"), memory_hints: wgpu::MemoryHints::Performance,
            required_features,
            ..Default::default()
        }, None)).ok()?;
        let caps = surface.get_capabilities(&adapter);
        let fmt = caps.formats.iter().find(|f| f.is_srgb()).copied().unwrap_or(caps.formats[0]);
        // Present mode + frame_latency tradeoff for pan/zoom feel:
        //   Mailbox        — vsync, no queue. Lowest input lag, but wgpu on
        //                    macOS Metal doesn't always advertise it; we try
        //                    it first and gracefully fall back.
        //   Fifo + lat=1   — vsync with single-frame queue. ~16ms max input
        //                    lag. May exhibit brief acquire stalls if GPU work
        //                    spikes, but per-frame work is now cheap enough
        //                    (post spinner-repaint-storm fix) for this to be
        //                    viable.
        //   Fifo + lat=2   — last-resort fallback. Smooth pacing but up to
        //                    33ms input lag.
        //
        // User-reported symptom that motivated the switch: "movement feels
        // behind my drag" + "micro-stuttering during pan".
        eprintln!("[native-chart] available present modes: {:?}", caps.present_modes);
        // Reverted to the original baseline after diagnostic showed:
        //   1. macOS Metal only advertises [Fifo, Immediate] (no Mailbox).
        //   2. Immediate (no vsync) still stutters AND tears — confirms the
        //      paint pipeline has variable frame times, not a vsync issue.
        //   3. Fifo+lat=1 is responsive but stuttery (variance exposed).
        //   4. Fifo+lat=2 buffers the variance behind a 2-frame queue (~33ms
        //      input lag) — produces the "buttery" feel users remember.
        //
        // The real fix for the underlying variance requires profiling +
        // targeted work in the chart paint hot path (sacred core.rs, single-
        // owner pass). Until then, keep the original config.
        // User-reported drag lag (2026-05-26): the Fifo+lat=2 baseline added
        // ~33ms of perceptible delay when panning the chart. Prefer Mailbox
        // where available — same vsync pacing (no tearing) but drops stale
        // frames instead of queuing them, so the GPU never falls behind
        // user input. macOS Metal historically only advertised [Fifo,
        // Immediate] so the Fifo fallback is preserved for that path.
        let (present_mode, frame_latency) =
            if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
                (wgpu::PresentMode::Mailbox, 1u32)
            } else if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
                (wgpu::PresentMode::Fifo, 2u32)
            } else {
                (wgpu::PresentMode::AutoVsync, 2u32)
            };
        eprintln!("[native-chart] PresentMode::{:?}, frame latency {}", present_mode, frame_latency);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, format: fmt,
            width: size.width.max(1), height: size.height.max(1),
            present_mode, alpha_mode: caps.alpha_modes[0],
            view_formats: vec![], desired_maximum_frame_latency: frame_latency,
        };
        surface.configure(&device, &config);

        let egui_ctx = egui::Context::default();
        // Publish the Context so background threads (data feeds, fetch jobs)
        // can call `crate::wake_native_ui()` to wake the UI from sleep when
        // they have new data to show. Without this, removing the catch-all
        // repaint would leave async data invisible until user input.
        let _ = crate::NATIVE_EGUI_CTX.set(egui_ctx.clone());
        let mut visuals = egui::Visuals::dark();
        // Subtle rounded corners on all widgets
        let r3 = egui::CornerRadius::same(style::radius_sm() as u8);
        let r6 = egui::CornerRadius::same(style::radius_md() as u8);
        visuals.window_corner_radius = r6;
        visuals.menu_corner_radius = egui::CornerRadius::same(style::radius_sm() as u8);
        visuals.widgets.noninteractive.corner_radius = r3;
        visuals.widgets.inactive.corner_radius = r3;
        visuals.widgets.hovered.corner_radius = r3;
        visuals.widgets.active.corner_radius = r3;
        visuals.widgets.open.corner_radius = r3;
        egui_ctx.set_visuals(visuals);
        ui_kit::icons::init_icons(&egui_ctx);
        start_account_poller();
        let egui_state = egui_winit::State::new(egui_ctx.clone(), egui::ViewportId::ROOT, &*window, Some(window.scale_factor() as f32), None, None);
        let egui_renderer = egui_wgpu::Renderer::new(&device, fmt, None, 1, false);

        // Publish the surface format so `paint_shadow_gpu` can build its
        // pipeline lazily on first use.
        crate::ui_kit::widgets::shadow_pipeline::set_surface_format(fmt);
        crate::ui_kit::widgets::text_subpixel_pipeline::set_surface_format(fmt);

        // Phase 1.5: eagerly build the subpixel text pipeline so naga validates
        // the WGSL at startup rather than on first use. Pushes shader-syntax
        // failures up to launch time instead of runtime regressions.
        let _ = crate::ui_kit::widgets::text_subpixel_pipeline::TextSubpixelPipeline::get(&device, fmt);
        eprintln!("[gpu] text_subpixel_pipeline: WGSL validated OK");

        // GPU chart pipeline (SPEC_GPU_CHART_REFACTOR.md, Phase 1). Build the
        // pipeline unconditionally so the WGSL is validated at every launch,
        // even with the feature off. Activation is reported to monitoring so
        // the Prometheus surface advertises which path is live.
        let chart_pipeline = crate::chart::renderer_gpu::ChartPipeline::new(&device, fmt);
        let chart_active = cfg!(feature = "gpu_chart_v2");
        crate::monitoring::set_chart_pipeline_active(chart_active);
        eprintln!("[gpu] chart_pipeline: WGSL validated OK (active={chart_active})");

        Some(Self {
            device, queue, surface, config,
            egui_ctx, egui_state, egui_renderer,
            #[cfg(feature = "design-mode")]
            instance,
            #[cfg(feature = "design-mode")]
            adapter,
            pointer_gone_needed: false,
            chart_pipeline,
        })
    }

    fn render(&mut self, window: &Window, panes: &mut Vec<Chart>, active_pane: &mut usize, layout: &mut Layout, watchlist: &mut Watchlist, toasts: &[crate::chart_renderer::ui::tools::notification::Notification], conn_panel_open: &mut bool, rx: &mpsc::Receiver<ChartCommand>) {
        crate::monitoring::frame_begin();
        crate::foundation::frame_profiler::frame_begin();
        // Bump shadow pipeline frame counter for texture pool recycling.
        crate::ui_kit::widgets::shadow_pipeline::next_frame();

        // Effective proportional font = per-theme preference (ported from the
        // React `--ds-font-ui` blocks) falling back to the user's picker. The
        // active style's font wins on themed presets (Aperture/Cadence/Alto/
        // Mariner/Lucid); Meridien/Octave defer to the user picker. Monospace
        // stays JetBrains regardless (tabular-digit policy in init_fonts).
        let effective_font = style_preferred_font(style_id(watchlist))
            .unwrap_or(watchlist.font_idx);
        // Mirror into TextEngine so PolishedLabel (Family::SansSerif sentinel)
        // shapes with the matching primary font.
        crate::ui_kit::widgets::text_engine::set_active_font_idx(effective_font);
        // Re-install egui fonts only when the effective font actually changes —
        // `set_fonts` is a global, allocation-heavy call, never per-frame.
        {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static LAST_FONT: AtomicUsize = AtomicUsize::new(usize::MAX);
            if LAST_FONT.swap(effective_font, Ordering::Relaxed) != effective_font {
                // S7: route through FontRegistry path so active StyleSystem's
                // Typography families drive font loading.
                crate::ui_kit::icons::init_fonts_for_idx(&self.egui_ctx, effective_font);
            }
        }

        // Mirror the active pane's background luminance into TextEngine so
        // theme-aware gamma can adapt glyph rendering to light/dark themes.
        {
            fn linearize(channel: u8) -> f32 {
                let x = channel as f32 / 255.0;
                if x <= 0.03928 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
            }
            fn compute_relative_luminance(c: egui::Color32) -> f32 {
                0.2126 * linearize(c.r()) + 0.7152 * linearize(c.g()) + 0.0722 * linearize(c.b())
            }
            let theme_idx = panes.get(*active_pane).map(|p| p.theme_idx).unwrap_or(0);
            let bg = get_theme(theme_idx).bg;
            let lum = compute_relative_luminance(bg);
            crate::ui_kit::widgets::text_engine::set_active_bg_luminance(lum);
        }

        // Phase 1: Acquire surface texture
        let t0 = std::time::Instant::now();
        let output = match self.surface.get_current_texture() {
            Ok(t) => t, Err(_) => { self.surface.configure(&self.device, &self.config); window.request_redraw(); return; }
        };
        let view = output.texture.create_view(&Default::default());
        let acquire_us = t0.elapsed().as_micros() as u64;

        // Phase 2: egui layout + draw_chart logic
        let t1 = std::time::Instant::now();
        let mut raw_input = self.egui_state.take_egui_input(window);
        // Inject synthetic PointerGone when focus was lost so egui never stays
        // stuck in a drag state because mouseUp was never delivered.
        if std::mem::take(&mut self.pointer_gone_needed) {
            raw_input.events.push(egui::Event::PointerGone);
        }
        // Feed the profiler the input-event count so is_idle() can detect
        // genuinely quiet frames (no clicks, drags, key presses, scrolls).
        crate::foundation::frame_profiler::note_input_events(raw_input.events.len() as u32);
        // Dev Inspector — inject queued input events into raw_input before the frame.
        #[cfg(debug_assertions)]
        {
            use crate::dev_inspector::input_queue::{drain_inputs_raw};
            drain_inputs_raw(&mut raw_input.events);
        }
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            // NOTE: the inter-region gaps show the GPU chart pipeline's surface
            // clear (already the theme bg), so no egui canvas paint is needed.
            // An opaque egui Background-layer rect here would composite OVER the
            // GPU chart and blank the screen — do NOT add one.
            draw_chart(ctx, panes, active_pane, layout, watchlist, toasts, conn_panel_open, rx);
            // Aperture/Glass tiled-card overlay: paint rounded corners + border on each pane
            // AFTER draw_chart so it sits on top of the chart content. Uses Foreground layer.
            paint_pane_card_frames(ctx, panes, *layout, watchlist);
            // Boss key: paint the TPS overlay on top of everything when active.
            // render_tps_overlay returns true when the user dismisses it
            // (Esc or the fake-Excel ✕ close button).
            if watchlist.boss_key_active
                && crate::chart_renderer::ui::tps_overlay::render_tps_overlay(ctx) {
                watchlist.boss_key_active = false;
            }
            // Borderless-window edge-resize grab bands (custom chrome, no OS frame).
            // Last so the resize cursor overrides the chart crosshair at the edges.
            window_resize_borders(ctx, window);

            // Bug-report Inspect overlay (Ctrl+Shift+I) — resolved LAST so every
            // panel + pane anchor registered this frame is hit-testable. Pure egui
            // overlay; never touches the GPU chart pipeline.
            {
                use crate::chart_renderer::bug_anchor as ba;
                if ba::inspect() {
                    ba::resolve_frame(ctx, ba::draft_is_open());
                    if !ba::draft_is_open() {
                        if let Some(hit) = ba::take_pending() { ba::open_draft(hit); }
                    }
                    ba::prompt(ctx);
                }
            }
        });
        self.egui_state.handle_platform_output(window, full_output.platform_output);

        // Fulfill any region-screenshot requests queued by the bug-report prompt
        // this frame (the window HWND + scale live here, not inside draw_chart).
        #[cfg(target_os = "windows")]
        {
            let reqs  = crate::chart_renderer::bug_anchor::take_capture_reqs();
            // Harness screenshot requests (full-window) share this render-thread site.
            // `dev_inspector` is `#[cfg(debug_assertions)]` (a dev-only tool), so the
            // screenshot harness is inert in release — keeps the release build buildable.
            #[cfg(debug_assertions)]
            let shots = crate::dev_inspector::take_screenshot_reqs();
            #[cfg(not(debug_assertions))]
            let shots: Vec<String> = Vec::new();
            if !reqs.is_empty() || !shots.is_empty() {
                use winit::raw_window_handle::HasWindowHandle;
                if let Ok(handle) = window.window_handle() {
                    if let winit::raw_window_handle::RawWindowHandle::Win32(h) = handle.as_raw() {
                        let hwnd = h.hwnd.get() as *mut std::ffi::c_void;
                        let scale = full_output.pixels_per_point;
                        for req in reqs {
                            if let Err(e) = crate::chart_renderer::bug_anchor::capture_window_region(
                                hwnd, scale, req.rect, &req.out,
                            ) {
                                eprintln!("[bug-anchor] region capture failed: {e}");
                            }
                        }
                        // Full-window: pass an oversized rect; capture clamps to the client area.
                        let full = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1.0e5, 1.0e5));
                        for name in shots {
                            let out = std::path::Path::new("dev/screenshots").join(format!("{name}.png"));
                            if let Err(e) = crate::chart_renderer::bug_anchor::capture_window_region(
                                hwnd, scale, full, &out,
                            ) {
                                eprintln!("[dev-inspector] screenshot '{name}' failed: {e}");
                            }
                        }
                    }
                }
            }
        }
        let layout_us = t1.elapsed().as_micros() as u64;

        // The per-pane upload + render now happens inside the chart pass loop
        // below (Phase 5a). Each visible pane writes its own ChartRenderParams
        // and runs its own draw, so we can't upload a single pane up here.

        // Phase 3: Tessellation — optimize for crisp text
        let t2 = std::time::Instant::now();
        self.egui_ctx.tessellation_options_mut(|opts| {
            opts.round_text_to_pixels = true;      // snap glyphs to whole pixels — eliminates subpixel blur
            opts.feathering_size_in_pixels = 1.0;  // standard AA (lower = crisper but more aliased)
        });
        let paint_jobs = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        let tessellate_us = t2.elapsed().as_micros() as u64;

        // Collect render stats
        let num_paint_jobs = paint_jobs.len() as u32;
        let mut total_vertices = 0u32;
        let mut total_indices = 0u32;
        for job in &paint_jobs {
            if let egui::epaint::Primitive::Mesh(mesh) = &job.primitive {
                total_vertices += mesh.vertices.len() as u32;
                total_indices += mesh.indices.len() as u32;
            }
        }
        let texture_uploads = full_output.textures_delta.set.len() as u32;
        let texture_frees = full_output.textures_delta.free.len() as u32;

        let sd = egui_wgpu::ScreenDescriptor { size_in_pixels: [self.config.width, self.config.height], pixels_per_point: full_output.pixels_per_point };

        // Phase 4: GPU upload (textures + buffers)
        let t3 = std::time::Instant::now();
        for (id, delta) in &full_output.textures_delta.set { self.egui_renderer.update_texture(&self.device, &self.queue, *id, delta); }
        let mut enc = self.device.create_command_encoder(&Default::default());
        self.egui_renderer.update_buffers(&self.device, &self.queue, &mut enc, &paint_jobs, &sd);
        self.queue.submit(std::iter::once(enc.finish()));
        let upload_us = t3.elapsed().as_micros() as u64;

        // Phase 5a: GPU chart pass (SPEC_GPU_CHART_REFACTOR.md, per-pane).
        // Runs BEFORE egui so candles are under egui chrome (grid, axes,
        // crosshair). The first pane in the loop uses LoadOp::Clear(bg);
        // subsequent panes use LoadOp::Load to composite onto the same
        // surface. Each pane has its own scissor, so panes can't overdraw
        // each other.
        //
        // The bg colour for the clear comes from the active pane (multiple
        // panes with different themes is rare; the active pane's bg covers
        // the gaps between panes).
        #[cfg(feature = "gpu_chart_v2")]
        {
            let active_bg = panes.get(*active_pane)
                .map(|c| c.gpu_render_params.bg)
                .unwrap_or([0.0, 0.0, 0.0, 1.0]);
            let clear_color = wgpu::Color {
                r: active_bg[0] as f64,
                g: active_bg[1] as f64,
                b: active_bg[2] as f64,
                a: active_bg[3] as f64,
            };

            let surf_w = self.config.width as f32;
            let surf_h = self.config.height as f32;
            let ppp = full_output.pixels_per_point;
            let mut chart_us_total: u64 = 0;
            let mut total_visible_bars: u32 = 0;
            let mut first_pane_done = false;
            // When a pane is maximized, render only that pane on the GPU.
            // The egui pane-render loop already skips non-maximized panes,
            // and the central panel's pre-emptive clear zeroes their GPU
            // state — but if a frame slipped past either guard we'd ghost
            // the previous layout. This is the final belt-and-braces gate.
            let max_idx_opt = watchlist.maximized_pane;
            for (i, chart) in panes.iter().enumerate() {
                if let Some(mi) = max_idx_opt { if i != mi { continue; } }
                // Skip panes that didn't populate a real chart_rect this
                // frame (alt-mode bars, non-Chart pane types, n==0 loading,
                // hidden panes). The render_chart_pane prologue resets the
                // rect to [0;4]; only the candle / volume blocks set it.
                let cr = chart.gpu_render_params.chart_rect;
                let has_chart = cr[2] > cr[0] && cr[3] > cr[1];
                let has_data = !chart.gpu_render_params.instances.is_empty()
                    || !chart.gpu_render_params.line_segments.is_empty()
                    || !chart.gpu_render_params.fill_quads.is_empty();
                if !has_chart || !has_data { continue; }

                self.chart_pipeline.upload(&self.queue, &chart.gpu_render_params, surf_w, surf_h, ppp);
                let load_op = if !first_pane_done {
                    wgpu::LoadOp::Clear(clear_color)
                } else {
                    wgpu::LoadOp::Load
                };
                chart_us_total += self.chart_pipeline.render(&self.device, &self.queue, &view, load_op);
                total_visible_bars += chart.gpu_render_params.instances.len() as u32;
                first_pane_done = true;
            }

            // No pane had GPU content (loading, all alt-mode, etc.) — still
            // need to initialise the surface or egui's LoadOp::Load reads
            // garbage / stale frame.
            if !first_pane_done {
                self.chart_pipeline.upload(&self.queue, &Default::default(), surf_w, surf_h, ppp);
                chart_us_total += self.chart_pipeline.render(&self.device, &self.queue, &view, wgpu::LoadOp::Clear(clear_color));
            }

            crate::monitoring::set_chart_pass_us(chart_us_total);
            crate::monitoring::set_chart_visible_bars(total_visible_bars);
            crate::monitoring::set_chart_pipeline_active(true);
        }

        // Phase 5b: egui render pass
        let t4 = std::time::Instant::now();
        let mut enc2 = self.device.create_command_encoder(&Default::default());
        // When gpu_chart_v2 is active the chart pass already cleared the surface,
        // so egui uses LoadOp::Load to composite on top. Otherwise egui clears itself.
        #[cfg(feature = "gpu_chart_v2")]
        let egui_load_op = wgpu::LoadOp::Load;
        #[cfg(not(feature = "gpu_chart_v2"))]
        let egui_load_op = wgpu::LoadOp::Clear(wgpu::Color::BLACK);
        let mut pass = enc2.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view, resolve_target: None,
                ops: wgpu::Operations { load: egui_load_op, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None,
        }).forget_lifetime();
        self.egui_renderer.render(&mut pass, &paint_jobs, &sd);
        drop(pass);
        self.queue.submit(std::iter::once(enc2.finish()));
        let render_us = t4.elapsed().as_micros() as u64;

        // Phase 6: Present
        let t5 = std::time::Instant::now();
        for id in &full_output.textures_delta.free { self.egui_renderer.free_texture(id); }
        output.present();
        let present_us = t5.elapsed().as_micros() as u64;

        // Report all phase timings + render stats
        crate::monitoring::frame_end_detailed(crate::monitoring::FramePhases {
            acquire_us, layout_us, tessellate_us, upload_us, render_us, present_us,
            paint_jobs: num_paint_jobs, vertices: total_vertices, indices: total_indices,
            texture_uploads, texture_frees,
        });
        let _frame_profile = crate::foundation::frame_profiler::frame_end();
    }
}

impl App {
    fn spawn_window(&mut self, el: &ActiveEventLoop, rx: mpsc::Receiver<ChartCommand>, initial_cmd: Option<ChartCommand>) {
        // On Windows: borderless window (custom chrome drawn by egui).
        // On macOS: DO NOT use with_decorations(false) — NSWindowStyleMask::borderless
        //   breaks the key-window / mouse-tracking session so mouseUp is never delivered,
        //   leaving egui permanently stuck in drag state.
        //   Instead: keep decorations=true (NSWindowStyleMask::titled) for correct event
        //   routing, then hide the titlebar visually with macOS platform APIs.
        //   The result is visually identical but the window is a proper key window.
        #[cfg(not(target_os = "macos"))]
        let attrs = {
            #[allow(unused_mut)]
            let mut a = WindowAttributes::default()
                .with_title("Apex Terminal")
                .with_inner_size(PhysicalSize::new(self.iw, self.ih))
                .with_min_inner_size(PhysicalSize::new(960, 540))
                .with_decorations(false)
                .with_window_icon(make_window_icon())
                .with_active(true)
                .with_maximized(true);
            #[cfg(debug_assertions)]
            if crate::dev_inspector::is_headless() {
                // In headless mode: small window, fully on-screen, so DWM delivers
                // WM_PAINT at full speed. The ShowWindow block below sends it to the
                // bottom of the Z-order so it hides behind every other open window.
                // Must also un-maximize — a maximized window ignores position.
                a = a
                    .with_maximized(false)
                    .with_inner_size(winit::dpi::PhysicalSize::new(640_u32, 480_u32))
                    .with_position(winit::dpi::PhysicalPosition::new(0_i32, 0_i32));
            }
            a
        };

        #[cfg(target_os = "macos")]
        let attrs = {
            use winit::platform::macos::WindowAttributesExtMacOS;
            WindowAttributes::default()
                .with_title("Apex Terminal")
                .with_inner_size(PhysicalSize::new(self.iw, self.ih))
                .with_min_inner_size(PhysicalSize::new(960, 540))
                .with_active(true)
                .with_maximized(true)
                // IMPORTANT: do NOT use with_titlebar_hidden(true) — winit maps that to
                // NSWindowStyleMask::Borderless which prevents AppKit from ever calling
                // makeFirstResponder(contentView). Without that call, mouseUp events are
                // not routed to the WinitView, so egui never sees releases and every click
                // leaves the pointer permanently stuck in "pressed" state.
                //
                // with_titlebar_transparent(true) + fullsize_content_view achieves the same
                // visual result (invisible titlebar, content fills the whole window) while
                // keeping NSWindowStyleMask::Titled — AppKit then correctly sets first
                // responder on makeKeyAndOrderFront, so all mouse events work.
                .with_titlebar_transparent(true)
                .with_fullsize_content_view(true)
                .with_titlebar_buttons_hidden(true)
                .with_title_hidden(true)
                .with_has_shadow(true)
                .with_accepts_first_mouse(true)
                .with_movable_by_window_background(false)
        };

        let w = match el.create_window(attrs)
        {
            Ok(w) => {
                // Enable rounded corners on Windows 11 (DWM)
                #[cfg(target_os = "windows")]
                {
                    use winit::raw_window_handle::HasWindowHandle;
                    if let Ok(handle) = w.window_handle() {
                        if let winit::raw_window_handle::RawWindowHandle::Win32(h) = handle.as_raw() {
                            unsafe {
                                let hwnd = h.hwnd.get() as *mut std::ffi::c_void;

                                // Ensure WS_EX_APPWINDOW (0x40000) so taskbar shows our icon,
                                // and clear WS_EX_TOOLWINDOW (0x80) which winit sets when
                                // `with_decorations(false)` is used and which suppresses
                                // the taskbar entry. Windows latches taskbar membership at
                                // window-creation time, so we must hide → restyle → show
                                // for the new ex-style to actually register the window
                                // with the shell.
                                use windows_sys::Win32::UI::WindowsAndMessaging::{
                                    GetWindowLongW, SetWindowLongW, SetWindowPos, ShowWindow,
                                    SendMessageW, SetClassLongPtrW,
                                };
                                let ex_style = GetWindowLongW(hwnd, -20);
                                let new_ex = (ex_style | 0x00040000) & !0x00000080;
                                ShowWindow(hwnd, 0);                       // SW_HIDE
                                SetWindowLongW(hwnd, -20, new_ex);
                                SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 0, 0,
                                    0x0001 | 0x0002 | 0x0004 | 0x0010 | 0x0020); // NOSIZE|NOMOVE|NOZORDER|NOACTIVATE|FRAMECHANGED
                                // Set the icon WHILE hidden so the shell sees a valid
                                // icon at the moment the window first appears as a
                                // taskbar-eligible app window. Some Win11 builds suppress
                                // the entry if the icon isn't set at first paint.
                                if let Some(hicon) = make_window_icon_hicon() {
                                    SendMessageW(hwnd, 0x0080, 1, hicon); // ICON_BIG
                                    SendMessageW(hwnd, 0x0080, 0, hicon); // ICON_SMALL
                                    SetClassLongPtrW(hwnd, -14, hicon as _);
                                    SetClassLongPtrW(hwnd, -34, hicon as _);
                                }
                                // SW_SHOWMAXIMIZED preserves the with_maximized(true)
                                // state and re-registers the taskbar entry. SW_RESTORE
                                // would un-maximize. In headless mode, SW_SHOWNORMAL
                                // keeps the off-screen position instead of overriding it.
                                #[cfg(debug_assertions)]
                                // SW_SHOWNOACTIVATE(8): show at position without stealing focus.
                                let show_cmd = if crate::dev_inspector::is_headless() { 8 } else { 3 };
                                #[cfg(not(debug_assertions))]
                                let show_cmd = 3_i32;
                                ShowWindow(hwnd, show_cmd);

                                // In headless mode, push window to bottom of Z-order so
                                // it renders at full speed (on-screen) but hides behind
                                // every other window. HWND_BOTTOM = 1 as a sentinel.
                                #[cfg(debug_assertions)]
                                if crate::dev_inspector::is_headless() {
                                    SetWindowPos(
                                        hwnd,
                                        1 as *mut _,  // HWND_BOTTOM
                                        0, 0, 0, 0,
                                        0x0001 | 0x0002 | 0x0010, // NOSIZE|NOMOVE|NOACTIVATE
                                    );
                                }

                                // DWMWA_WINDOW_CORNER_PREFERENCE = 33, DWMWCP_ROUND = 2
                                let preference: u32 = 2;
                                let _ = windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                                    hwnd,
                                    33,
                                    &preference as *const u32 as *const _,
                                    std::mem::size_of::<u32>() as u32,
                                );
                            }
                        }
                    }
                }
                // Set window icon (taskbar + alt-tab)
                if let Some(icon) = make_window_icon() {
                    w.set_window_icon(Some(icon));
                }
                // Also set via Win32 WM_SETICON for reliable taskbar display
                #[cfg(target_os = "windows")]
                {
                    use winit::raw_window_handle::HasWindowHandle;
                    if let Ok(handle) = w.window_handle() {
                        if let winit::raw_window_handle::RawWindowHandle::Win32(h) = handle.as_raw() {
                            if let Some(hicon) = make_window_icon_hicon() {
                                unsafe {
                                    let hwnd_msg = h.hwnd.get() as *mut std::ffi::c_void;
                                    // WM_SETICON: ICON_BIG=1, ICON_SMALL=0
                                    windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(hwnd_msg, 0x0080, 1, hicon);
                                    windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(hwnd_msg, 0x0080, 0, hicon);
                                    // Set on window CLASS — this is what Win11 taskbar reads
                                    // GCLP_HICON = -14, GCLP_HICONSM = -34
                                    windows_sys::Win32::UI::WindowsAndMessaging::SetClassLongPtrW(hwnd_msg, -14, hicon as _);
                                    windows_sys::Win32::UI::WindowsAndMessaging::SetClassLongPtrW(hwnd_msg, -34, hicon as _);
                                }
                            }
                        }
                    }
                }

                Arc::new(w)
            }
            Err(e) => { eprintln!("[native-chart] Window creation failed: {e}"); return; }
        };
        let gpu = match GpuCtx::new(Arc::clone(&w)) {
            Some(g) => g,
            None => { eprintln!("[native-chart] GPU init failed"); return; }
        };
        let id = w.id();
        let (panes, layout, loaded_settings) = load_state();
        let mut wl = Watchlist::new();
        // Restore the saved playbook (plays are persisted to plays.json).
        wl.plays = load_plays();
        // Apply persisted global settings
        wl.font_scale = loaded_settings.font_scale;
        wl.font_idx = loaded_settings.font_idx;
        // Re-init fonts if the loaded font differs from default.
        // S7: route through FontRegistry path so Typography families drive loading.
        if wl.font_idx != 0 { crate::ui_kit::icons::init_fonts_for_idx(&gpu.egui_ctx, wl.font_idx); }
        wl.compact_mode = loaded_settings.compact_mode;
        wl.pane_header_size = loaded_settings.pane_header_size;
        wl.toolbar_auto_hide = loaded_settings.toolbar_auto_hide;
        wl.show_x_axis = loaded_settings.show_x_axis;
        wl.show_y_axis = loaded_settings.show_y_axis;
        wl.shared_x_axis = loaded_settings.shared_x_axis;
        wl.shared_y_axis = loaded_settings.shared_y_axis;
        if let Some(favs) = loaded_settings.draw_favorites.clone() { wl.draw_favorites = favs; }
        wl.style_idx = loaded_settings.style_idx;
        // P4.3 + P5 — restore token-scale overrides and immediately push them
        // to the global atomic slots so the first frame uses the user's saved
        // choices instead of the style preset's defaults.
        wl.density_override        = loaded_settings.density_override;
        wl.border_weight_override  = loaded_settings.border_weight_override;
        wl.corner_scale_override   = loaded_settings.corner_scale_override;
        wl.spacing_scale_override  = loaded_settings.spacing_scale_override;
        wl.motion_speed_override   = loaded_settings.motion_speed_override;
        crate::chart_renderer::ui::style::set_density_override(wl.density_override);
        crate::ui_kit::style::set_border_weight_override(wl.border_weight_override);
        crate::ui_kit::style::set_corner_scale_override(wl.corner_scale_override);
        crate::ui_kit::style::set_spacing_scale_override(wl.spacing_scale_override);
        crate::ui_kit::style::set_motion_speed_override(wl.motion_speed_override);
        wl.pane_split.h = loaded_settings.pane_split_h;
        wl.pane_split.v = loaded_settings.pane_split_v;
        wl.pane_split.h2 = loaded_settings.pane_split_h2;
        wl.pane_split.v2 = loaded_settings.pane_split_v2;
        wl.pane_split.v3 = loaded_settings.pane_split_v3;
        wl.pane_split.v4 = loaded_settings.pane_split_v4;
        wl.pane_split.v5 = loaded_settings.pane_split_v5;
        wl.pane_split.v6 = loaded_settings.pane_split_v6;
        // Wave 14c: overlay the typed UiSettings aggregate if present,
        // overriding the legacy `settings` blob values. Cold-start (no
        // file yet) keeps the legacy-derived values in place.
        if let Some(loaded_ui) =
            crate::state::load::<crate::state::UiSettings>(&ui_settings_path())
        {
            wl.ui_settings = loaded_ui;
            wl.pull_from_ui_settings();
        }
        // P2: Initialize the welcome wizard from loaded ui_settings.
        wl.init_welcome_wizard();
        // Wave 2 (state): load TradingDefaults from disk, seed the store and
        // mirror into the flat legacy fields.
        if let Some(loaded_td) =
            crate::state::load::<crate::state::TradingDefaults>(&trading_defaults_path())
        {
            wl.trading_defaults_store.update(|s| *s = loaded_td);
            wl.sync_trading_defaults_from_store();
        }
        // P2 (command-palette-frecency): restore frecency data if present.
        if let Some(ps) =
            crate::state::load::<crate::state::CmdPaletteState>(&cmd_palette_state_path())
        {
            wl.cmd_palette.recent = ps.recent;
            wl.cmd_palette.freq = ps.freq;
        }
        // Load persisted hotkeys (override defaults)
        load_hotkeys(&mut wl.hotkeys);
        // Load persisted templates
        wl.pane_templates = load_templates();
        // Load persisted alerts (always needed for pane-level price alerts).
        let (wl_alerts_legacy, pane_alerts_map) = load_alerts();
        // Wave 3 (state): load AlertsState from the new store format if present.
        // Falls back to the legacy load_alerts() data so existing alerts are migrated
        // on first launch after the upgrade.
        if let Some(loaded_as) =
            crate::state::load::<crate::state::AlertsState>(&alerts_state_path())
        {
            wl.alerts_store.update(|s| *s = loaded_as);
            wl.sync_from_alerts_store();
        } else {
            // Legacy path: seed from the custom JSON format used before Wave 3.
            wl.alerts = wl_alerts_legacy;
            if !wl.alerts.is_empty() {
                wl.next_alert_id = wl.alerts.iter().map(|a| a.id).max().unwrap_or(0) + 1;
            }
            // Seed the store so future saves use the new format.
            wl.push_to_alerts_store();
        }
        // Wave 3 (state): load SidebarState from disk if present.
        // Cold-start (no file yet) keeps the Watchlist::new() defaults.
        if let Some(loaded_ss) =
            crate::state::load::<crate::state::SidebarState>(&sidebar_state_path())
        {
            wl.sidebar_state_store.update(|s| *s = loaded_ss);
            wl.sync_from_sidebar_store();
        } else {
            // Seed the store from the current defaults so it's ready to persist.
            wl.push_to_sidebar_store();
        }
        // Wave 3 (state): load LayoutState from disk if present.
        // Cold-start (no file yet) keeps the Watchlist::new() defaults.
        // Note: pane_templates payloads are loaded by load_templates() above;
        // sync_from_layout_store() does not overwrite them — only the names
        // list and all other flat fields are restored.
        if let Some(loaded_ls) =
            crate::state::load::<crate::state::LayoutState>(&layout_state_path())
        {
            wl.layout_state_store.update(|s| *s = loaded_ls);
            wl.sync_from_layout_store();
        } else {
            // Seed the store from the current defaults so it's ready to persist.
            wl.push_to_layout_store();
        }
        // Wave 3 (state): load ChatState from disk if present.
        // Cold-start (no file yet) keeps the Watchlist::new() defaults.
        if let Some(loaded_cs) =
            crate::state::load::<crate::state::ChatState>(&chat_state_path())
        {
            wl.chat_state_store.update(|s| *s = loaded_cs);
            wl.sync_from_chat_store();
        } else {
            // Seed the store from the current defaults so it's ready to persist.
            wl.push_to_chat_store();
        }
        let wl_syms: Vec<String> = wl.all_symbols();
        let mut cw = ChartWindow { id, win: Arc::clone(&w), gpu, rx, panes, active_pane: 0, layout, maximized_pane: None, close_requested: false, watchlist: wl, toasts: vec![], conn_panel_open: false, last_save: None };
        cw.watchlist.native_dpi_scale = w.scale_factor() as f32;
        // Apply persisted per-symbol alerts to panes
        for p in &mut cw.panes {
            if let Some(alerts) = pane_alerts_map.get(&p.symbol) {
                p.price_alerts = alerts.clone();
                if let Some(max_id) = p.price_alerts.iter().map(|a| a.id).max() {
                    p.next_alert_id = max_id + 1;
                }
            }
        }
        // Fetch prices for default watchlist symbols
        fetch_watchlist_prices(wl_syms);
        if let Some(cmd) = initial_cmd {
            // Route initial LoadBars to first pane
            if let Some(p) = cw.panes.first_mut() { p.process(cmd); }
        }
        // Wave 2 (state): register both stores with the persist supervisor
        // registry so they are walked every ~50ms from this point on.
        self.store_registry.register(
            cw.watchlist.ui_settings_store.clone() as std::sync::Arc<dyn crate::state::PersistableStore>
        );
        self.store_registry.register(
            cw.watchlist.trading_defaults_store.clone() as std::sync::Arc<dyn crate::state::PersistableStore>
        );
        // Wave 3 (state): register the alerts store.
        self.store_registry.register(
            cw.watchlist.alerts_store.clone() as std::sync::Arc<dyn crate::state::PersistableStore>
        );
        // Wave 3 (state): register the sidebar_state store.
        self.store_registry.register(
            cw.watchlist.sidebar_state_store.clone() as std::sync::Arc<dyn crate::state::PersistableStore>
        );
        // Wave 3 (state): register the layout_state store.
        self.store_registry.register(
            cw.watchlist.layout_state_store.clone() as std::sync::Arc<dyn crate::state::PersistableStore>
        );
        // Wave 3 (state): register the chat_state store.
        self.store_registry.register(
            cw.watchlist.chat_state_store.clone() as std::sync::Arc<dyn crate::state::PersistableStore>
        );
        self.windows.push(cw);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        // On first resume, check for pending spawn request
        if self.windows.is_empty() {
            if let Ok(req) = self.spawn_rx.try_recv() {
                self.spawn_window(el, req.rx, Some(req.initial_cmd));
            }
        }
    }
    fn window_event(&mut self, _el: &ActiveEventLoop, wid: winit::window::WindowId, ev: WindowEvent) {
        // ── Inspector window dispatch (design-mode only) ──────────────────
        // Route events to the popped-out inspector OS window if its id matches.
        // Done BEFORE the chart-window lookup so the inspector handles its own
        // close/redraw without falling through.
        #[cfg(feature = "design-mode")]
        {
            if let Some(insp_win) = self.inspector_window.as_mut() {
                if insp_win.id == wid {
                    let _ = insp_win.on_window_event(&ev);
                    match ev {
                        WindowEvent::CloseRequested => {
                            insp_win.close_requested = true;
                            // The about_to_wait tick will drop the struct and
                            // clear Inspector::is_popout.
                        }
                        WindowEvent::Resized(s) => {
                            insp_win.resize(s.width, s.height);
                            insp_win.win.request_redraw();
                        }
                        WindowEvent::RedrawRequested => {
                            insp_win.render();
                        }
                        WindowEvent::ScaleFactorChanged { .. } => {
                            insp_win.win.request_redraw();
                        }
                        _ => {}
                    }
                    return;
                }
            }
        }

        let cw = match self.windows.iter_mut().find(|w| w.id == wid) { Some(w) => w, None => return };

        // Trace mouse events in debug builds — helps diagnose macOS event delivery.
        // If you see Pressed but never Released, the OS is swallowing mouseUp.
        #[cfg(debug_assertions)]
        match &ev {
            WindowEvent::MouseInput { state, button, .. } =>
                eprintln!("[input] {:?} {:?}", button, state),
            WindowEvent::Focused(f) => eprintln!("[input] Focused({})", f),
            _ => {}
        }

        let egui_response = cw.gpu.egui_state.on_window_event(&cw.win, &ev);
        if egui_response.repaint {
            // Egui-driven redraw: hover, drag, animation in flight, etc.
            // Mostly user/animation-driven — keep it immediate.
            crate::foundation::frame_profiler::note_repaint(
                concat!(file!(), ":", line!(), " egui_response"),
            );
            cw.win.request_redraw();
        }
        match ev {
            WindowEvent::CloseRequested => {
                // Phase 3 (state): push all legacy fields into their stores
                // BEFORE save_state and flush_all so both paths see fresh data.
                // save_state calls push_all_stores internally, but the explicit
                // call here guards the flush_all path in case save_state is
                // bypassed or reordered in a future refactor.
                save_state(&cw.panes, cw.layout, &mut cw.watchlist);
                cw.watchlist.persist();
                // Phase 3 (state): push_all_stores was already called inside
                // save_state above, so stores are fresh; flush_all persists any
                // that haven't been written by the debounce supervisor yet.
                // Stop the supervisor first so it cannot race a stale snapshot
                // in after the final flush_all (audit: Wave 6).
                crate::state::shutdown_persist_supervisor();
                let flush_failures = self.store_registry.flush_all();
                for (key, e) in &flush_failures {
                    eprintln!("[state] final flush failed for '{key}': {e}");
                }
                self.windows.retain(|w| w.id != wid);
                // When the LAST chart window closes, clear the global command-sender
                // registry that native_main's poll loop watches. Without this the
                // closed window's Sender lingers in NATIVE_CHART_TXS, the poll loop
                // never sees "no senders", main never returns, and the process keeps
                // running headless — a zombie that holds the GPU + :9091 and stacks
                // up on every relaunch (root cause of the acquire-stall / fps decay).
                if self.windows.is_empty() {
                    if let Some(m) = crate::NATIVE_CHART_TXS.get() {
                        if let Ok(mut v) = m.lock() { v.clear(); }
                    }
                }
            }
            WindowEvent::Resized(s) => {
                if s.width>0&&s.height>0 {
                    cw.gpu.config.width=s.width; cw.gpu.config.height=s.height;
                    cw.gpu.surface.configure(&cw.gpu.device, &cw.gpu.config);
                    crate::foundation::frame_profiler::note_repaint(
                        concat!(file!(), ":", line!(), " resize"),
                    );
                    // Paint SYNCHRONOUSLY here rather than only request_redraw():
                    // the Windows modal resize loop owns the thread and does NOT
                    // service RedrawRequested, so a deferred redraw leaves the old
                    // frame stretched by DWM until the drag ends (the rubber-band
                    // jank). Rendering inline tracks the new size live on every
                    // step. Same render path as RedrawRequested — this changes only
                    // WHEN the existing render runs, never the chart GPU pipeline.
                    cw.gpu.render(&cw.win, &mut cw.panes, &mut cw.active_pane, &mut cw.layout, &mut cw.watchlist, &cw.toasts, &mut cw.conn_panel_open, &cw.rx);
                }
            }
            WindowEvent::RedrawRequested => {
                // Drain watchlist price updates before render
                // (these come via the broadcast channel from fetch_watchlist_prices)
                let mut cmds_to_requeue = Vec::new();
                while let Ok(cmd) = cw.rx.try_recv() {
                    match cmd {
                        ChartCommand::WatchlistPrice { ref symbol, price, prev_close, day_close, change_perc, stale } => {
                            cw.watchlist.set_price(symbol, price);
                            cw.watchlist.set_prev_close(symbol, prev_close);
                            cw.watchlist.set_day_close(symbol, day_close);
                            cw.watchlist.set_change_perc(symbol, change_perc);
                            cw.watchlist.set_stale(symbol, stale);
                        }
                        ChartCommand::ScannerPrice { ref symbol, price, prev_close, volume } => {
                            if let Some(r) = cw.watchlist.scanner.results.iter_mut().find(|r| r.symbol == *symbol) {
                                r.price = price;
                                r.volume = volume;
                                r.change_pct = if prev_close > 0.0 { (price - prev_close) / prev_close * 100.0 } else { 0.0 };
                            } else {
                                let change_pct = if prev_close > 0.0 { (price - prev_close) / prev_close * 100.0 } else { 0.0 };
                                cw.watchlist.scanner.results.push(ScanResult {
                                    symbol: symbol.clone(), price, change_pct, volume,
                                });
                            }
                        }
                        ChartCommand::HeatmapBars { ref cells } => {
                            cw.watchlist.heatmap.cells = cells.clone();
                        }
                        ChartCommand::TapeEntry { ref symbol, price, qty, time, is_buy } => {
                            cw.watchlist.tape.entries.push(TapeRow {
                                symbol: symbol.clone(), price, qty, time, is_buy,
                            });
                            if cw.watchlist.tape.entries.len() > 500 {
                                cw.watchlist.tape.entries.drain(..cw.watchlist.tape.entries.len() - 500);
                            }
                        }
                        ChartCommand::ChainData { ref symbol, dte, underlying_price, ref calls, ref puts, placeholder } => {
                            let _ = underlying_price;
                            if *symbol == cw.watchlist.chain.symbol {
                                let to_rows = |data: &[(f32,f32,f32,f32,i32,i32,f32,bool,String)]| -> Vec<OptionRow> {
                                    data.iter().map(|(strike,last,bid,ask,vol,oi,iv,itm,contract)| OptionRow {
                                        strike: *strike, last: *last, bid: *bid, ask: *ask,
                                        volume: *vol, oi: *oi, iv: *iv, itm: *itm, contract: contract.clone(),
                                    }).collect()
                                };
                                if dte == 0 {
                                    cw.watchlist.chain.near = OptionChain { calls: to_rows(calls), puts: to_rows(puts) };
                                    cw.watchlist.chain.near_placeholder = placeholder;
                                } else {
                                    cw.watchlist.chain.far = OptionChain { calls: to_rows(calls), puts: to_rows(puts) };
                                    cw.watchlist.chain.far_placeholder = placeholder;
                                }
                                cw.watchlist.chain.loading = false;
                            }
                        }
                        ChartCommand::SearchResults { ref query, ref results, ref source } => {
                            // Only apply if query still matches current search
                            if source == "watchlist" && !query.is_empty()
                                && cw.watchlist.search.query.to_lowercase().starts_with(&query.to_lowercase()) {
                                // Merge: keep static results and append API results that aren't already present
                                for (sym, name) in results {
                                    if !cw.watchlist.search.results.iter().any(|(s, _)| s == sym) {
                                        cw.watchlist.search.results.push((sym.clone(), name.clone()));
                                    }
                                }
                            } else if source == "chain" && !query.is_empty()
                                && cw.watchlist.chain.sym_input.to_lowercase().starts_with(&query.to_lowercase()) {
                                for (sym, name) in results {
                                    if !cw.watchlist.search.results.iter().any(|(s, _)| s == sym) {
                                        cw.watchlist.search.results.push((sym.clone(), name.clone()));
                                    }
                                }
                            }
                        }
                        other => cmds_to_requeue.push(other),
                    }
                }
                // Re-inject non-watchlist commands (they'll be picked up by draw_chart)
                // Can't re-send to rx since we own the receiver. Use a temp buffer approach:
                // Actually, draw_chart also drains rx. So we need to pass these through.
                // Simpler: just process ALL commands here and pass pane commands to the right pane.
                for cmd in cmds_to_requeue {
                    // Tick updates: broadcast to all matching panes (each checks timeframe)
                    match &cmd {
                        ChartCommand::UpdateLastBar { symbol, .. } | ChartCommand::AppendBar { symbol, .. } => {
                            let s = symbol.clone();
                            for p in cw.panes.iter_mut() { if p.symbol == s { p.process(cmd.clone()); } }
                            continue;
                        }
                        _ => {}
                    }
                    let sym = match &cmd {
                        ChartCommand::LoadBars { symbol, .. } | ChartCommand::PrependBars { symbol, .. } | ChartCommand::LoadDrawings { symbol, .. } => Some(symbol.clone()),
                        ChartCommand::IndicatorSourceBars { .. } => None,
                        ChartCommand::OverlayBars { ref symbol, .. } => {
                            eprintln!("[about_to_wait] OverlayBars for '{}' arrived", symbol);
                            let s = symbol.clone(); for p in cw.panes.iter_mut() { if p.symbol_overlays.iter().any(|o| o.symbol == s) { p.process(cmd.clone()); } } continue;
                        }
                        // OverlayChainData is a WINDOW-level command (applied across panes),
                        // not a per-pane one — routing it to p.process() drops it silently.
                        // Apply it here exactly like the draw_chart dispatch (gpu.rs:4655),
                        // otherwise whether the strikes overlay populates depends on which
                        // drain consumes the result (it was racily lost about half the time).
                        ChartCommand::OverlayChainData { symbol, calls, puts, placeholder } => {
                            let to_rows = |data: &[(f32,f32,f32,f32,i32,i32,f32,bool,String)]| -> Vec<OptionRow> {
                                data.iter().map(|(strike,last,bid,ask,vol,oi,iv,itm,contract)| OptionRow {
                                    strike: *strike, last: *last, bid: *bid, ask: *ask,
                                    volume: *vol, oi: *oi, iv: *iv, itm: *itm, contract: contract.clone(),
                                }).collect()
                            };
                            for p in cw.panes.iter_mut() {
                                if p.symbol == *symbol && p.overlay_chain_loading {
                                    p.overlay_calls = to_rows(calls);
                                    p.overlay_puts = to_rows(puts);
                                    p.overlay_chain_symbol = symbol.clone();
                                    p.overlay_chain_loading = false;
                                    p.overlay_chain_placeholder = *placeholder;
                                }
                            }
                            continue;
                        }
                        _ => None,
                    };
                    if let Some(s) = sym {
                        if let Some(p) = cw.panes.iter_mut().find(|p| p.symbol == s) { p.process(cmd); }
                        else if let Some(p) = cw.panes.get_mut(cw.active_pane) { p.process(cmd); }
                    } else if let Some(p) = cw.panes.get_mut(cw.active_pane) { p.process(cmd); }
                }

                // Also update watchlist from tick data (UpdateLastBar contains current price)
                for sec in &mut cw.watchlist.sections {
                    for item in &mut sec.items {
                        // Check if any pane has this symbol and get its latest price
                        if let Some(pane) = cw.panes.iter().find(|p| p.symbol == item.symbol) {
                            if let Some(bar) = pane.bars.last() {
                                item.price = bar.close;
                            }
                        }
                    }
                }

                CURRENT_WINDOW.with(|w| *w.borrow_mut() = Some(Arc::clone(&cw.win)));
                CLOSE_REQUESTED.with(|f| f.set(false));
                cw.gpu.render(&cw.win, &mut cw.panes, &mut cw.active_pane, &mut cw.layout, &mut cw.watchlist, &cw.toasts, &mut cw.conn_panel_open, &cw.rx);
                CURRENT_WINDOW.with(|w| *w.borrow_mut() = None);
                if CLOSE_REQUESTED.with(|f| f.get()) {
                    cw.close_requested = true;
                }
                // Auto-save state every 30 seconds
                {
                    let now = std::time::Instant::now();
                    let should_save = cw.last_save.map_or(true, |t| now.duration_since(t).as_secs() >= 30);
                    if should_save {
                        save_state(&cw.panes, cw.layout, &mut cw.watchlist);
                        cw.last_save = Some(now);
                    }
                }
                // Process pending "new blank workspace" — reset the live panes to
                // a single default pane and switch to a fresh untitled name
                // (unsaved until the user saves it from the workspace rail).
                if cw.watchlist.workspace.pending_new_blank {
                    cw.watchlist.workspace.pending_new_blank = false;
                    let mut chart = Chart::new();
                    // Trigger the initial bar fetch + drawing load for the fresh
                    // pane (mirrors the per-pane load path), so the blank
                    // workspace shows a populated default chart instead of empty.
                    chart.pending_symbol_change = Some(chart.symbol.clone());
                    cw.panes = vec![chart];
                    cw.layout = Layout::One;
                    cw.active_pane = 0;
                    cw.watchlist.pane_layout = None; // re-materialize from Layout::One
                    cw.watchlist.workspace.active = next_untitled_workspace_name();
                }
                // Process pending workspace load
                if let Some(ws_name) = cw.watchlist.workspace.pending_load.take() {
                    let path = workspace_dir().join(format!("{}.json", ws_name));
                    if let Ok(data) = std::fs::read_to_string(&path) {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                            let (new_panes, new_layout) = {
                                let layout = match json.get("layout").and_then(|v| v.as_str()).unwrap_or("1") {
                                    "2" => Layout::Two, "2H" => Layout::TwoH, "3" => Layout::Three, "3L" => Layout::ThreeL, "3R" => Layout::ThreeR,
                                    "4" => Layout::Four, "4L" => Layout::FourL,
                                    "5C" => Layout::FiveC, "5L" => Layout::FiveL, "5W" => Layout::FiveW, "5R" => Layout::FiveR,
                                    "6" => Layout::Six, "6H" => Layout::SixH, "6L" => Layout::SixL,
                                    "7" => Layout::Seven, "8H" => Layout::EightH, "9" => Layout::Nine, _ => Layout::One,
                                };
                                let theme_idx = {
                                    let raw = json.get("theme_idx").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
                                    raw.min(live_themes().read().unwrap_or_else(|e| e.into_inner()).len().saturating_sub(1))
                                };
                                let recents: Vec<(String, String)> = json.get("recent_symbols").and_then(|v| v.as_array()).map(|arr| {
                                    arr.iter().filter_map(|v| {
                                        let a = v.as_array()?;
                                        Some((a.first()?.as_str()?.to_string(), a.get(1)?.as_str()?.to_string()))
                                    }).collect()
                                }).unwrap_or_default();
                                let mut panes: Vec<Chart> = Vec::new();
                                if let Some(arr) = json.get("panes").and_then(|v| v.as_array()) {
                                    for p in arr {
                                        let sym = p.get("symbol").and_then(|v| v.as_str()).unwrap_or("AAPL");
                                        let tf = p.get("timeframe").and_then(|v| v.as_str()).unwrap_or("5m");
                                        let mut chart = Chart::new_with(sym, tf);
                                        chart.theme_idx = theme_idx;
                                        chart.recent_symbols = recents.clone();
                                        chart.pending_symbol_change = Some(sym.to_string());
                                        let gb = |key: &str, def: bool| -> bool { p.get(key).and_then(|v| v.as_bool()).unwrap_or(def) };
                                        chart.show_volume = gb("show_volume", true);
                                        chart.show_oscillators = gb("show_oscillators", true);
                                        chart.ohlc_tooltip = gb("ohlc_tooltip", true);
                                        chart.magnet = gb("magnet", true);
                                        chart.log_scale = gb("log_scale", false);
                                        chart.show_vwap_bands = gb("show_vwap_bands", false);
                                        chart.show_cvd = gb("show_cvd", false);
                                        chart.show_delta_volume = gb("show_delta_volume", false);
                                        chart.show_rvol = gb("show_rvol", true);
                                        chart.show_ma_ribbon = gb("show_ma_ribbon", false);
                                        chart.show_prev_close = gb("show_prev_close", true);
                                        chart.show_auto_sr = gb("show_auto_sr", false);
                                        chart.show_auto_fib = gb("show_auto_fib", false);
                                        chart.swing_leg_mode = p.get("swing_leg_mode").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                                        chart.show_footprint = gb("show_footprint", false);
                                        chart.show_gamma = gb("show_gamma", false); chart.hit_highlight = gb("hit_highlight", false);
                                        chart.show_darkpool = gb("show_darkpool", false);
                                        chart.show_events = gb("show_events", false);
                                        chart.show_pnl_curve = gb("show_pnl_curve", false);
                                        chart.show_pattern_labels = gb("show_pattern_labels", true);
                                        chart.link_group = p.get("link_group").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                                        // v5: overlay toggles
                                        chart.show_vol_shelves     = gb("show_vol_shelves", false);
                                        chart.show_confluence      = gb("show_confluence", false);
                                        chart.show_momentum_heat   = gb("show_momentum_heat", false);
                                        chart.show_trend_strip     = gb("show_trend_strip", false);
                                        chart.show_breadth_tint    = gb("show_breadth_tint", false);
                                        chart.show_vol_cone        = gb("show_vol_cone", false);
                                        chart.show_price_memory    = gb("show_price_memory", false);
                                        chart.show_liquidity_voids = gb("show_liquidity_voids", false);
                                        chart.show_corr_ribbon     = gb("show_corr_ribbon", false);
                                        chart.show_analyst_targets = gb("show_analyst_targets", false);
                                        chart.show_pe_band         = gb("show_pe_band", false);
                                        chart.show_insider_trades  = gb("show_insider_trades", false);
                                        // Session shading
                                        chart.session_shading = gb("session_shading", false);
                                        chart.rth_start_minutes = p.get("rth_start_minutes").and_then(|v| v.as_u64()).unwrap_or(570) as u16;
                                        chart.rth_end_minutes = p.get("rth_end_minutes").and_then(|v| v.as_u64()).unwrap_or(960) as u16;
                                        chart.eth_bar_opacity = p.get("eth_bar_opacity").and_then(|v| v.as_f64()).unwrap_or(0.35) as f32;
                                        chart.session_bg_tint = gb("session_bg_tint", false);
                                        chart.session_bg_color = p.get("session_bg_color").and_then(|v| v.as_str()).unwrap_or("#1a1a2e").to_string();
                                        chart.session_bg_opacity = p.get("session_bg_opacity").and_then(|v| v.as_f64()).unwrap_or(0.15) as f32;
                                        chart.session_break_lines = gb("session_break_lines", true);
                                        chart.candle_mode = match p.get("candle_mode").and_then(|v| v.as_str()).unwrap_or("std") {
                                            "vln" => CandleMode::Violin, "grd" => CandleMode::Gradient, "vg" => CandleMode::ViolinGradient,
                                            "ha" => CandleMode::HeikinAshi, "line" => CandleMode::Line, "area" => CandleMode::Area,
                    "rnk" => CandleMode::Renko, "rng" => CandleMode::RangeBar, "tck" => CandleMode::TickBar,
                                            _ => CandleMode::Standard,
                                        };
                                        chart.alt.renko_brick = p.get("renko_brick_size").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                        chart.alt.range_size = p.get("range_bar_size").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                        chart.alt.tick_count = p.get("tick_bar_count").and_then(|v| v.as_u64()).unwrap_or(500) as u32;
                                        chart.alt.dirty = true;
                                        chart.vp.mode = match p.get("vp_mode").and_then(|v| v.as_str()).unwrap_or("off") {
                                            "classic" => VolumeProfileMode::Classic, "heatmap" => VolumeProfileMode::Heatmap,
                                            "strip" => VolumeProfileMode::Strip, "clean" => VolumeProfileMode::Clean,
                                            _ => VolumeProfileMode::Off,
                                        };
                                        if let Some(inds) = p.get("indicators").and_then(|v| v.as_array()) {
                                            chart.indicators.clear();
                                            for (idx, ind_json) in inds.iter().enumerate() {
                                                let kind_label = ind_json.get("kind").and_then(|v| v.as_str()).unwrap_or("SMA");
                                                let kind = IndicatorType::all().iter().find(|t| t.label() == kind_label).copied().unwrap_or(IndicatorType::SMA);
                                                let period = ind_json.get("period").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                                                let color = ind_json.get("color").and_then(|v| v.as_str()).unwrap_or(INDICATOR_COLORS[idx % INDICATOR_COLORS.len()]);
                                                let visible = ind_json.get("visible").and_then(|v| v.as_bool()).unwrap_or(true);
                                                let thickness = ind_json.get("thickness").and_then(|v| v.as_f64()).unwrap_or(1.5) as f32;
                                                let id = chart.next_indicator_id; chart.next_indicator_id += 1;
                                                let mut ind = Indicator::new(id, kind, period, color);
                                                ind.visible = visible; ind.thickness = thickness;
                                                ind.param2 = ind_json.get("param2").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                                ind.param3 = ind_json.get("param3").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                                ind.param4 = ind_json.get("param4").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                                ind.source = ind_json.get("source").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                                                ind.offset = ind_json.get("offset").and_then(|v| v.as_i64()).unwrap_or(0) as i16;
                                                ind.ob_level = ind_json.get("ob_level").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                                ind.os_level = ind_json.get("os_level").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                                ind.source_tf = ind_json.get("source_tf").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                ind.line_style = match ind_json.get("line_style").and_then(|v| v.as_str()).unwrap_or("solid") {
                                                    "dashed" => LineStyle::Dashed, "dotted" => LineStyle::Dotted, _ => LineStyle::Solid,
                                                };
                                                // Band styling (BB, Keltner, etc.) — v3 parity; absent in v2 files, defaults to empty/0
                                                ind.upper_color = ind_json.get("upper_color").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                ind.lower_color = ind_json.get("lower_color").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                ind.fill_color_hex = ind_json.get("fill_color_hex").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                ind.upper_thickness = ind_json.get("upper_thickness").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                                ind.lower_thickness = ind_json.get("lower_thickness").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                                chart.indicators.push(ind);
                                            }
                                        }
                                        // v3 parity: chart widgets — absent in v2 files, defaults to empty vec
                                        if let Some(wv) = p.get("chart_widgets") {
                                            if let Ok(widgets) = serde_json::from_value::<Vec<super::ChartWidget>>(wv.clone()) {
                                                chart.chart_widgets = widgets;
                                                for w in &mut chart.chart_widgets { w.anim_init = false; }
                                            }
                                        }
                                        // v3 parity: option-pane state — absent in v2 files, defaults to false/empty
                                        chart.is_option = p.get("is_option").and_then(|v| v.as_bool()).unwrap_or(false);
                                        chart.option_contract = p.get("option_contract").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        chart.option_strike   = p.get("option_strike").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                        chart.option_type     = p.get("option_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        chart.option_expiry   = p.get("option_expiry").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        chart.underlying      = p.get("underlying").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        // v3 parity: bar source — absent in v2 files, defaults to "last"
                                        chart.bar_source_mark = p.get("bar_source").and_then(|v| v.as_str()).unwrap_or("last") == "mark";
                                        // v4: per-pane DOM panel state.
                                        chart.dom.open = gb("dom_open", false);
                                        chart.dom.sidebar_open = gb("dom_sidebar_open", false);
                                        // v6: tab history
                                        if let Some(ts) = p.get("tab_symbols").and_then(|v| v.as_array()) {
                                            chart.tab_symbols = ts.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                                        }
                                        if let Some(ts) = p.get("tab_timeframes").and_then(|v| v.as_array()) {
                                            chart.tab_timeframes = ts.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                                        }
                                        chart.tab_active = p.get("tab_active").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                        // v6: symbol overlays — bars re-fetched when pending_symbol_change fires
                                        if let Some(ovs) = p.get("symbol_overlays").and_then(|v| v.as_array()) {
                                            for ov in ovs {
                                                let sym = ov.get("symbol").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                if sym.is_empty() { continue; }
                                                chart.symbol_overlays.push(SymbolOverlay {
                                                    symbol: sym,
                                                    color: ov.get("color").and_then(|v| v.as_str()).unwrap_or("#FF5733").to_string(),
                                                    show_candles: ov.get("show_candles").and_then(|v| v.as_bool()).unwrap_or(false),
                                                    visible: ov.get("visible").and_then(|v| v.as_bool()).unwrap_or(true),
                                                    bars: vec![], timestamps: vec![], loading: true,
                                                });
                                            }
                                        }
                                        panes.push(chart);
                                    }
                                }
                                if panes.is_empty() { panes.push(Chart::new()); }
                                panes.truncate(layout.max_panes());
                                (panes, layout)
                            };
                            cw.panes = new_panes;
                            cw.layout = new_layout;
                            cw.active_pane = 0;
                            // v4: restore pane geometry. v3 (and older) files omit
                            // these keys, so loading them leaves the current
                            // geometry untouched (back-compat). Drawings are NOT
                            // restored here — they reload from Postgres via the
                            // per-pane symbol-load triggered by pending_symbol_change.
                            if let Some(splits) = json.get("splits") {
                                let gf = |k: &str, def: f32| splits.get(k).and_then(|v| v.as_f64()).map(|f| f as f32).unwrap_or(def);
                                cw.watchlist.pane_split.h  = gf("h",  0.5);
                                cw.watchlist.pane_split.v  = gf("v",  0.5);
                                cw.watchlist.pane_split.h2 = gf("h2", 0.5);
                                cw.watchlist.pane_split.v2 = gf("v2", 0.5);
                                cw.watchlist.pane_split.v3 = gf("v3", 0.5);
                                cw.watchlist.pane_split.v4 = gf("v4", 0.5);
                                cw.watchlist.pane_split.v5 = gf("v5", 0.5);
                                cw.watchlist.pane_split.v6 = gf("v6", 0.5);
                            }
                            // Restore the saved pane-geometry tree ONLY if it is
                            // consistent with the restored pane count. A stale tree
                            // (e.g. a single leaf saved for a multi-pane workspace
                            // by older builds) is discarded so `ensure_pane_layout`
                            // rebuilds the correct geometry from the `layout` enum;
                            // a matching tree is kept so custom split ratios survive
                            // the round-trip.
                            cw.watchlist.pane_layout = json.get("pane_layout")
                                .filter(|pl| !pl.is_null())
                                .and_then(|pl| serde_json::from_value::<Option<crate::chart_renderer::pane_layout::PaneLayout>>(pl.clone()).ok())
                                .flatten()
                                .filter(|tree| tree.pane_count() == cw.panes.len());

                            // Restore per-workspace UI state (side panels, focused
                            // pane, rail expand). Absent in pre-v4 files → panels
                            // keep their current state.
                            if let Some(ui) = json.get("ui") {
                                let gb = |k: &str, def: bool| ui.get(k).and_then(|v| v.as_bool()).unwrap_or(def);
                                cw.watchlist.workspace.nav_expanded = gb("rail_expanded", cw.watchlist.workspace.nav_expanded);
                                cw.watchlist.object_tree_open   = gb("object_tree_open", false);
                                cw.watchlist.open               = gb("watchlist_open", false);
                                cw.watchlist.signals_panel.open = gb("signals_panel_open", false);
                                cw.watchlist.account_strip_open = gb("account_strip_open", false);
                                let ap = ui.get("active_pane").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                cw.active_pane = ap.min(cw.panes.len().saturating_sub(1));
                                cw.watchlist.active_pane_idx = cw.active_pane;
                            }
                        }
                    }
                }
                // Process pending alerts from context menu
                if let Some((sym, price, above)) = PENDING_ALERT.with(|a| a.borrow_mut().take()) {
                    cw.watchlist.update_alerts_state(|s| {
                        let id = s.next_alert_id;
                        s.next_alert_id += 1;
                        s.alerts.push(crate::state::PersistedAlert { id, symbol: sym, price, above, triggered: false, message: String::new() });
                    });
                }
                // Drain and dedup pending notifications (collapse identical messages in the same frame).
                {
                    use crate::chart_renderer::ui::tools::notification::Notification;
                    let new_toasts: Vec<Notification> = crate::chart_renderer::ui::tools::notification::drain_pending();
                    if !new_toasts.is_empty() {
                        // Tee into the persistent history log (bottom dock's Notifications tab).
                        crate::chart_renderer::ui::tools::notification::record_history(&new_toasts);
                        // Collect existing messages as owned Strings to release the borrow before pushing.
                        let existing: std::collections::HashSet<String> = cw.toasts.iter().map(|n: &Notification| n.message.clone()).collect();
                        let mut seen = std::collections::HashSet::<String>::new();
                        for n in new_toasts {
                            if !existing.contains(&n.message) && seen.insert(n.message.clone()) {
                                cw.toasts.push(n);
                            }
                        }
                    }
                }
                // Remove expired toasts (>5 seconds)
                cw.toasts.retain(|n: &crate::chart_renderer::ui::tools::notification::Notification| n.created.elapsed().as_secs() < 5);
            }
            WindowEvent::Focused(false) => {
                // When focus is lost the OS may swallow the pending mouseUp, leaving egui
                // permanently stuck in drag state. Inject PointerGone into the next frame.
                cw.gpu.pointer_gone_needed = true;
                // User-driven (focus loss) — inject PointerGone next frame.
                crate::foundation::frame_profiler::note_repaint(
                    concat!(file!(), ":", line!(), " focus_lost"),
                );
                cw.win.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                cw.watchlist.native_dpi_scale = scale_factor as f32;
                // Reconfigure the surface to the new physical size so the frame
                // isn't mis-scaled/stretched for a beat when crossing monitors of
                // different DPI (a Resized may not follow, especially mid titlebar
                // move-loop). Then paint inline (the move-loop won't service a
                // deferred redraw).
                let new = cw.win.inner_size();
                if new.width>0 && new.height>0 {
                    cw.gpu.config.width=new.width; cw.gpu.config.height=new.height;
                    cw.gpu.surface.configure(&cw.gpu.device, &cw.gpu.config);
                }
                crate::foundation::frame_profiler::note_repaint(
                    concat!(file!(), ":", line!(), " dpi_change"),
                );
                cw.gpu.render(&cw.win, &mut cw.panes, &mut cw.active_pane, &mut cw.layout, &mut cw.watchlist, &cw.toasts, &mut cw.conn_panel_open, &cw.rx);
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        // Check for new window spawn requests
        while let Ok(req) = self.spawn_rx.try_recv() {
            self.spawn_window(el, req.rx, Some(req.initial_cmd));
        }

        // ── Inspector window lifecycle (design-mode only) ─────────────────
        // Polled from atomics flipped by the inspector code (POP/DOCK click
        // happens deep in the render loop, far from `&ActiveEventLoop`).
        #[cfg(feature = "design-mode")]
        {
            use crate::chart::renderer::inspector_window::{
                self, InspectorWindow,
            };
            // Open request → create the OS window if not already open.
            if inspector_window::take_open_request() && self.inspector_window.is_none() {
                if let Some(cw) = self.windows.first() {
                    let device = cw.gpu.device.clone();
                    let queue = cw.gpu.queue.clone();
                    let instance = cw.gpu.instance.clone();
                    let adapter = cw.gpu.adapter.clone();
                    if let Some(insp_win) =
                        InspectorWindow::create(el, device, queue, &instance, &adapter)
                    {
                        eprintln!("[inspector_window] created (id={:?})", insp_win.id);
                        self.inspector_window = Some(insp_win);
                    }
                } else {
                    eprintln!(
                        "[inspector_window] open requested but no chart window yet; ignoring"
                    );
                }
            }
            // Close request OR the OS window's X button → drop the struct
            // and clear Inspector::is_popout so the inspector docks again.
            let close_via_request = inspector_window::take_close_request();
            let close_via_x = self
                .inspector_window
                .as_ref()
                .map_or(false, |w| w.close_requested);
            if close_via_request || close_via_x {
                if self.inspector_window.is_some() {
                    eprintln!("[inspector_window] closing");
                    self.inspector_window = None;
                    DESIGN_INSPECTOR.with(|cell| {
                        if let Some(insp) = cell.borrow_mut().as_mut() {
                            insp.is_popout = false;
                        }
                    });
                }
            }
            // Continuously repaint the inspector window so slider drags +
            // hover state stay responsive. Egui's repaint hint also
            // requests it but a per-tick redraw guarantees liveness.
            if let Some(insp_win) = self.inspector_window.as_ref() {
                insp_win.win.request_redraw();
            }
        }

        // Remove windows that requested close
        self.windows.retain(|w| !w.close_requested);

        // Handle symbol/timeframe changes + frame rate for ALL windows
        for cw in &mut self.windows {
            // Track per-pane changes for cross-pane propagation. We collect
            // these inside the per-pane loop (which holds &mut cw.panes) and
            // publish/apply them AFTER the loop, when we can also borrow
            // &mut cw.watchlist.subscriptions and re-borrow &mut cw.panes
            // for sibling-pane apply. Each entry: (originating pane index,
            // pane.link_group, new_symbol, new_timeframe).
            let mut pane_changes: Vec<(usize, u8, Option<String>, Option<String>)> = Vec::new();
            for (pane_idx, pane) in cw.panes.iter_mut().enumerate() {
                let sym_change = pane.pending_symbol_change.take();
                let tf_change = pane.pending_timeframe_change.take();
                let sym_changed = sym_change.is_some();
                let tf_changed = tf_change.is_some();
                if sym_changed || tf_changed {
                    // Stash the OUTGOING (sym, tf)'s bars/ts in the tab cache
                    // before swapping, so re-entry restores instantly.
                    if !pane.symbol.is_empty() && !pane.bars.is_empty() {
                        // LRU-evict BEFORE insert so post-insert size <= TAB_CACHE_MAX.
                        evict_oldest_if_full(&mut pane.tab_cache);
                        pane.tab_cache.insert(
                            (pane.symbol.clone(), pane.timeframe.clone()),
                            (pane.bars.clone(), pane.timestamps.clone(), std::time::Instant::now()),
                        );
                    }

                    if let Some(ref sym) = sym_change {
                        // Push old symbol to history for back/forward navigation
                        // (skip if this change was triggered by back/forward nav buttons)
                        if !pane.symbol_nav_in_progress {
                            let old_sym = pane.symbol.clone();
                            if !old_sym.is_empty() && old_sym != *sym {
                                // Truncate forward history if we navigated back
                                if pane.symbol_history_idx < pane.symbol_history.len() {
                                    pane.symbol_history.truncate(pane.symbol_history_idx);
                                }
                                pane.symbol_history.push(old_sym);
                                // Wave 3 fix: cap history at 50 entries, keeping idx valid.
                                const MAX_SYMBOL_HISTORY: usize = 50;
                                if pane.symbol_history.len() > MAX_SYMBOL_HISTORY {
                                    let excess = pane.symbol_history.len() - MAX_SYMBOL_HISTORY;
                                    pane.symbol_history.drain(..excess);
                                    pane.symbol_history_idx = pane.symbol_history_idx.saturating_sub(excess);
                                }
                                pane.symbol_history_idx = pane.symbol_history.len();
                            }
                        }
                        pane.symbol_nav_in_progress = false;
                        pane.symbol = sym.clone();
                        pane.symbol_meta = crate::foundation::types::symbol_or_guess(sym);
                        // Switching to a new symbol via the picker means we're
                        // leaving the current option contract behind. Clear the
                        // option-pane state so the fetch dispatch below routes
                        // through fetch_bars_background, not fetch_option_bars
                        // with a stale OCC. (Option clicks set is_option=true
                        // separately, AFTER bypassing pending_symbol_change.)
                        pane.is_option = false;
                        pane.option_contract.clear();
                        pane.option_type.clear();
                        pane.option_expiry.clear();
                        pane.option_strike = 0.0;
                        pane.underlying.clear();
                        pane.bar_source_mark = false;
                    }
                    if let Some(tf) = tf_change { pane.timeframe = tf; }

                    let sym = pane.symbol.clone();
                    let tf = pane.timeframe.clone();
                    eprintln!("[native-chart] Loading {} {}", sym, tf);

                    // Try cache first — if we recently had this (sym, tf), restore
                    // instantly so the user doesn't see a blank chart while the
                    // background fetch runs to refresh it.
                    let cache_hit = pane.tab_cache.get(&(sym.clone(), tf.clone())).cloned();
                    if let Some((cb, cts, _)) = cache_hit {
                        pane.bars = cb;
                        pane.timestamps = cts;
                        pane.indicator_bar_count = 0; // recompute against restored bars
                    } else {
                        pane.bars.clear();
                        pane.timestamps.clear();
                    }
                    // Preserve indicator configuration across symbol/tf switches.
                    // Only wipe computed values so they're rebuilt against the new bars
                    // when LoadBars arrives and update_indicators() runs. Clearing the
                    // whole Vec meant users silently lost all their RSI/MACD settings on
                    // every symbol change — and the cross-TF source reload in LoadBars
                    // silently iterated an empty Vec.
                    for ind in &mut pane.indicators {
                        ind.values.clear();
                        ind.values2.clear();
                        ind.values3.clear();
                        ind.values4.clear();
                        ind.values5.clear();
                        ind.supertrend_dir.clear();
                        ind.histogram.clear();
                        ind.divergences.clear();
                        ind.source_bars.clear();
                        ind.source_timestamps.clear();
                        ind.source_loaded = false;
                    }
                    pane.drawings.clear(); // cleared here, reloaded when LoadBars arrives
                    pane.drawings_requested = false; // allow re-fetch for new timeframe
                    pane.history_loading = false;
                    pane.history_exhausted = false;
                    pane.load_error = None; // new symbol/tf — show spinner, not a stale error
                    pane.sim_price = 0.0;
                    pane.last_candle_time = std::time::Instant::now();
                    // Replay position is meaningless after a symbol/tf change — the bar
                    // count refers to the OLD symbol's bars. Stop replay so the user
                    // doesn't land in replay mode showing an arbitrary slice of the new
                    // symbol's bars. The user can re-enable replay manually after switching.
                    pane.replay_mode = false;
                    pane.replay_playing = false;

                    if pane.is_option && !pane.option_contract.is_empty() {
                        fetch_option_bars_background(pane.option_contract.clone(), sym.clone(), tf.clone(), pane.bar_source_mark);
                    } else {
                        // Origin pane fetch (its request_gen was bumped in commands.rs
                        // when the symbol/timeframe change was queued).
                        fetch_bars_background(sym.clone(), tf.clone(), pane.request_gen);
                    }

                    // Overlay bars are rendered by index, so they must be on the same
                    // timeframe as the main chart. Refetch on TF change (symbol change
                    // is fine — the TF is unchanged and index alignment still holds).
                    if tf_changed && !pane.symbol_overlays.is_empty() {
                        for ov in &mut pane.symbol_overlays {
                            ov.bars.clear();
                            ov.timestamps.clear();
                            ov.loading = true;
                            fetch_overlay_bars_background(ov.symbol.clone(), tf.clone());
                        }
                    } else {
                        // Overlays restored from workspace have loading=true and empty bars.
                        // Trigger their fetch now that the main chart's symbol/tf is known.
                        for ov in &pane.symbol_overlays {
                            if ov.loading && ov.bars.is_empty() {
                                fetch_overlay_bars_background(ov.symbol.clone(), tf.clone());
                            }
                        }
                    }

                    // Wave 12c: record this pane's change for cross-pane
                    // propagation via the SubscriptionBus, applied after the
                    // per-pane loop exits (where we can borrow watchlist +
                    // panes together). Only record when the pane is in a
                    // user-defined link group; group==0 means unlinked and
                    // nothing should propagate.
                    if pane.link_group > 0 {
                        pane_changes.push((
                            pane_idx,
                            pane.link_group,
                            if sym_changed { Some(sym) } else { None },
                            if tf_changed { Some(tf) } else { None },
                        ));
                    }
                }
            }

            // ── Wave 12c: cross-pane propagation via SubscriptionBus ──
            // Publish the changes recorded above as PaneEvents, then drain
            // the bus and apply each event to sibling panes. This replaces
            // the prior `link_changes` detector loop that inferred which
            // panes had changed by spotting empty-bars + link_group>0 —
            // we now know exactly which pane originated each change
            // (`origin_pane_idx`) and skip it during apply.
            //
            // The `pane_origins` vec parallels the queue order so the
            // drain step can pair each event with its originator. Events
            // published from outside the renderer (e.g. command palette,
            // see `ui::command_palette::execute`) carry no origin index
            // — they fall through to "apply to every matching pane",
            // which is fine because the publishing call site already
            // applied the mutation to its own pane before publishing.
            // Wave 13a: origin is now stored on the bus itself
            // (`publish_from`), so events published from anywhere
            // (here, top_nav toggles, command palette, …) carry their
            // own originator without a parallel index Vec. Drain
            // returns `(event, origin)` pairs ready for the dispatcher.
            for (origin, group, sym_opt, tf_opt) in pane_changes.drain(..) {
                if let Some(sym) = sym_opt {
                    cw.watchlist.subscriptions.publish_from(
                        crate::state::PaneEvent::SymbolChanged { group, symbol: sym },
                        origin,
                    );
                }
                if let Some(tf) = tf_opt {
                    cw.watchlist.subscriptions.publish_from(
                        crate::state::PaneEvent::TimeframeChanged { group, timeframe: tf },
                        origin,
                    );
                }
            }

            // Only treat a pane as linked when its `link_group` indexes
            // into an existing watchlist group — otherwise stale group
            // IDs from prior sessions (or the old click-cycle UI) would
            // silently link panes the user never explicitly grouped.
            let group_count = cw.watchlist.link_groups.len() as u8;
            let paired = cw.watchlist.subscriptions.drain();
            apply_pane_events(&mut cw.panes, &paired, group_count, true);

            // ── Unconditional 60 fps redraw (2026-05-26 — perf rev) ──────────
            //
            // Apex is a real-time trading chart. Reactive-only repaint
            // (gated on `egui_ctx.has_requested_repaint()`) was added earlier
            // as a battery optimisation, but it has two problems for this
            // workload:
            //
            //  1. Live ticks arrive ~100 Hz on active symbols. If a tick
            //     lands between vsync intervals and the reactive path hasn't
            //     yet called request_repaint for it, the candle update can
            //     be delayed by up to a frame. For a scalper that's
            //     unacceptable.
            //  2. Even small per-frame work spikes (motion animations on
            //     toolbar widgets, theme rebuild) can push a frame over the
            //     16 ms vsync budget. With reactive repaint + Fifo+lat=2,
            //     the slipped frame stacks input into the queue — exactly
            //     the "drag feels delayed" symptom the user reported.
            //
            // Unconditional request_redraw + vsync (Mailbox preferred,
            // Fifo+lat=2 fallback) pins the app at the display refresh and
            // keeps the swapchain perpetually fresh. The GPU work is
            // throttled by vsync so we don't actually burn the CPU at
            // 1000 fps — but every vsync we hand the compositor the latest
            // chart state.
            //
            // Battery cost: ~5-10% CPU vs idle reactive mode. For a trading
            // app the app is never actually idle (constant ticks) so this
            // delta is small in practice. If a future use-case wants the
            // battery-saver mode back, gate this behind a runtime flag.
            crate::foundation::frame_profiler::note_repaint(
                concat!(file!(), ":", line!(), " about_to_wait_tick"),
            );
            cw.win.request_redraw();
        }

        // Frame-pacing: Poll so the loop wakes every iteration to keep the
        // unconditional request_redraw above firing at vsync cadence.
        // Vsync (Mailbox / Fifo) handles the actual 60 fps gating downstream,
        // so Poll doesn't burn the CPU at 1000 fps — it just guarantees we
        // never sit waiting for an external wake when a frame is due.
        el.set_control_flow(winit::event_loop::ControlFlow::Poll);
    }
}

// ─── State persistence ───────────────────────────────────────────────────────

pub(crate) fn state_path() -> std::path::PathBuf {
    let mut p = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("apex-terminal");
    let _ = std::fs::create_dir_all(&p);
    p.push("native-chart-state.json");
    p
}

/// Wave 14c: companion to `state_path()` for the `UiSettings`
/// aggregate. Lives alongside `native-chart-state.json`; the legacy
/// `settings` blob inside that file stays the authoritative load path
/// until a follow-up wave can drop it, so this file is purely additive.
pub(crate) fn ui_settings_path() -> std::path::PathBuf {
    let mut p = state_path();
    p.pop();
    p.push("ui_settings.json");
    p
}

/// Wave 2 (state): persist path for the `TradingDefaults` aggregate.
/// Lives alongside `native-chart-state.json` in the same directory.
fn trading_defaults_path() -> std::path::PathBuf {
    let mut p = state_path();
    p.pop();
    p.push("trading_defaults.json");
    p
}

/// P2 (command-palette-frecency): persist path for the `CmdPaletteState`
/// aggregate.  Lives alongside `native-chart-state.json` in the same directory.
pub(crate) fn cmd_palette_state_path() -> std::path::PathBuf {
    let mut p = state_path();
    p.pop();
    p.push("cmd_palette_state.json");
    p
}

/// Wave 3 (state): persist path for the `AlertsState` aggregate.
/// Lives alongside `native-chart-state.json` in the same directory.
fn alerts_state_path() -> std::path::PathBuf {
    let mut p = state_path();
    p.pop();
    p.push("alerts_state.json");
    p
}

/// Wave 3 (state): persist path for the `SidebarState` aggregate.
/// Lives alongside `native-chart-state.json` in the same directory.
fn sidebar_state_path() -> std::path::PathBuf {
    let mut p = state_path();
    p.pop();
    p.push("sidebar_state.json");
    p
}

/// Wave 3 (state): persist path for the `LayoutState` aggregate.
/// Lives alongside `native-chart-state.json` in the same directory.
pub(crate) fn layout_state_path() -> std::path::PathBuf {
    let mut p = state_path();
    p.pop();
    p.push("layout_state.json");
    p
}

/// Wave 3 (state): persist path for the `ChatState` aggregate.
/// Lives alongside `native-chart-state.json` in the same directory.
fn chat_state_path() -> std::path::PathBuf {
    let mut p = state_path();
    p.pop();
    p.push("chat_state.json");
    p
}

pub(crate) fn workspace_dir() -> std::path::PathBuf {
    let mut p = state_path(); p.pop(); p.push("workspaces"); let _ = std::fs::create_dir_all(&p); p
}

// ─── Workspace persistence (extracted to `workspace_persist`, WS-E E2) ───────
// Re-exported so `gpu::save_state()` / `gpu::load_state()` / `gpu::save_workspace()`
// / ... and gpu.rs's own bare save_state()/load_state() calls are unchanged.
pub(crate) use super::workspace_persist::*;

// ─── Alerts persistence ──────────────────────────────────────────────────────

fn alerts_path() -> std::path::PathBuf {
    let mut p = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("apex-terminal"); let _ = std::fs::create_dir_all(&p);
    p.push("alerts.json"); p
}

pub(crate) fn save_alerts(watchlist: &Watchlist, panes: &[Chart]) {
    
    // Watchlist-level alerts
    let wl_alerts: Vec<serde_json::Value> = watchlist.alerts.iter().map(|a| serde_json::json!({
        "id": a.id, "symbol": a.symbol, "price": a.price, "above": a.above,
        "triggered": a.triggered, "message": a.message,
    })).collect();
    // Per-pane alerts keyed by symbol
    let mut pane_alerts = serde_json::Map::new();
    for p in panes {
        if p.price_alerts.is_empty() { continue; }
        let arr: Vec<serde_json::Value> = p.price_alerts.iter().map(|a| serde_json::json!({
            "id": a.id, "price": a.price, "above": a.above,
            "triggered": a.triggered, "draft": a.draft, "symbol": a.symbol,
        })).collect();
        pane_alerts.insert(p.symbol.clone(), serde_json::Value::Array(arr));
    }
    let json = serde_json::json!({ "watchlist_alerts": wl_alerts, "pane_alerts": pane_alerts });
    let _ = crate::state::persistence::atomic_write(
        &alerts_path(),
        serde_json::to_string_pretty(&json).unwrap_or_default().as_bytes(),
    );
}

fn load_alerts() -> (Vec<crate::chart_renderer::trading::Alert>, std::collections::HashMap<String, Vec<crate::chart_renderer::trading::PriceAlert>>) {
    let path = alerts_path();
    let data = std::fs::read_to_string(&path).unwrap_or_default();
    let json: serde_json::Value = serde_json::from_str(&data).unwrap_or(serde_json::Value::Null);
    // Watchlist alerts
    let wl: Vec<crate::chart_renderer::trading::Alert> = json.get("watchlist_alerts")
        .and_then(|v| v.as_array()).map(|arr| arr.iter().filter_map(|a| {
            Some(crate::chart_renderer::trading::Alert {
                id: a.get("id")?.as_u64()? as u32,
                symbol: a.get("symbol")?.as_str()?.to_string(),
                price: a.get("price")?.as_f64()? as f32,
                above: a.get("above")?.as_bool()?,
                triggered: a.get("triggered").and_then(|v| v.as_bool()).unwrap_or(false),
                message: a.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            })
        }).collect()).unwrap_or_default();
    // Pane alerts by symbol
    let mut pa = std::collections::HashMap::new();
    if let Some(obj) = json.get("pane_alerts").and_then(|v| v.as_object()) {
        for (sym, arr) in obj {
            if let Some(alerts) = arr.as_array() {
                let v: Vec<crate::chart_renderer::trading::PriceAlert> = alerts.iter().filter_map(|a| {
                    Some(crate::chart_renderer::trading::PriceAlert {
                        id: a.get("id")?.as_u64()? as u32,
                        price: a.get("price")?.as_f64()? as f32,
                        above: a.get("above")?.as_bool()?,
                        triggered: a.get("triggered").and_then(|v| v.as_bool()).unwrap_or(false),
                        draft: a.get("draft").and_then(|v| v.as_bool()).unwrap_or(false),
                        symbol: a.get("symbol").and_then(|v| v.as_str()).unwrap_or(sym).to_string(),
                    })
                }).collect();
                if !v.is_empty() { pa.insert(sym.clone(), v); }
            }
        }
    }
    (wl, pa)
}

// ─── Hotkeys persistence ─────────────────────────────────────────────────────

fn hotkeys_path() -> std::path::PathBuf {
    let mut p = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("apex-terminal"); let _ = std::fs::create_dir_all(&p);
    p.push("hotkeys.json"); p
}

pub(crate) fn save_hotkeys(watchlist: &Watchlist) {
    let arr: Vec<serde_json::Value> = watchlist.hotkeys.iter().map(|hk| serde_json::json!({
        "action": hk.action, "key_name": hk.key_name,
        "ctrl": hk.ctrl, "shift": hk.shift, "alt": hk.alt,
    })).collect();
    let _ = crate::state::persistence::atomic_write(
        &hotkeys_path(),
        serde_json::to_string_pretty(&serde_json::Value::Array(arr)).unwrap_or_default().as_bytes(),
    );
}

fn load_hotkeys(defaults: &mut Vec<HotKey>) {
    let path = hotkeys_path();
    let data = match std::fs::read_to_string(&path) { Ok(d) => d, Err(_) => return };
    let arr: Vec<serde_json::Value> = match serde_json::from_str(&data) { Ok(v) => v, Err(_) => return };
    // Override default bindings from saved file (match by action)
    for saved in &arr {
        let action = match saved.get("action").and_then(|v| v.as_str()) { Some(a) => a, None => continue };
        if let Some(hk) = defaults.iter_mut().find(|h| h.action == action) {
            hk.key_name = saved.get("key_name").and_then(|v| v.as_str()).unwrap_or(&hk.key_name).to_string();
            hk.ctrl = saved.get("ctrl").and_then(|v| v.as_bool()).unwrap_or(hk.ctrl);
            hk.shift = saved.get("shift").and_then(|v| v.as_bool()).unwrap_or(hk.shift);
            hk.alt = saved.get("alt").and_then(|v| v.as_bool()).unwrap_or(hk.alt);
            // Remap key enum from key_name
            let keys = [
                ("A", egui::Key::A), ("B", egui::Key::B), ("C", egui::Key::C), ("D", egui::Key::D),
                ("E", egui::Key::E), ("F", egui::Key::F), ("G", egui::Key::G), ("H", egui::Key::H),
                ("I", egui::Key::I), ("J", egui::Key::J), ("K", egui::Key::K), ("L", egui::Key::L),
                ("M", egui::Key::M), ("N", egui::Key::N), ("O", egui::Key::O), ("P", egui::Key::P),
                ("Q", egui::Key::Q), ("R", egui::Key::R), ("S", egui::Key::S), ("T", egui::Key::T),
                ("U", egui::Key::U), ("V", egui::Key::V), ("W", egui::Key::W), ("X", egui::Key::X),
                ("Y", egui::Key::Y), ("Z", egui::Key::Z),
                ("F1", egui::Key::F1), ("F2", egui::Key::F2), ("F3", egui::Key::F3), ("F4", egui::Key::F4),
                ("Del", egui::Key::Delete), ("Bksp", egui::Key::Backspace),
            ];
            // Extract the last segment of key_name (after any "Ctrl+Shift+" prefix)
            let raw = hk.key_name.split('+').last().unwrap_or("");
            for (name, key) in keys { if raw == name { hk.key = key; break; } }
        }
    }
}

// ─── Templates persistence ───────────────────────────────────────────────────

fn templates_dir() -> std::path::PathBuf {
    let mut p = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("apex-terminal"); p.push("templates"); let _ = std::fs::create_dir_all(&p); p
}

/// Look up an IndicatorType by its label string (used by template_popup).
pub(crate) fn indicator_type_from_label(label: &str) -> IndicatorType {
    IndicatorType::all().iter().find(|t| t.label() == label).copied().unwrap_or(IndicatorType::SMA)
}

pub(crate) fn save_templates(templates: &[(String, serde_json::Value)]) {
    let dir = templates_dir();
    // Wave 6 fix: write into a temp sibling dir first, then rename over the real
    // dir atomically so a crash between writes cannot lose all templates.
    let tmp_dir = {
        let mut p = dir.clone();
        p.set_file_name("templates.tmp");
        p
    };
    let _ = std::fs::remove_dir_all(&tmp_dir);
    if std::fs::create_dir_all(&tmp_dir).is_err() {
        // Fallback: write in-place (original behaviour) on mkdir failure.
        for (name, data) in templates {
            let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' }).collect();
            let path = dir.join(format!("{}.json", safe));
            let _ = crate::state::persistence::atomic_write(
                &path,
                serde_json::to_string_pretty(data).unwrap_or_default().as_bytes(),
            );
        }
        return;
    }
    for (name, data) in templates {
        let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' }).collect();
        let path = tmp_dir.join(format!("{}.json", safe));
        let _ = crate::state::persistence::atomic_write(
            &path,
            serde_json::to_string_pretty(data).unwrap_or_default().as_bytes(),
        );
    }
    // Atomic swap: discard old dir, rename tmp into place.
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::rename(&tmp_dir, &dir);
}

fn load_templates() -> Vec<(String, serde_json::Value)> {
    let dir = templates_dir();
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().map_or(false, |x| x == "json") {
                let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if let Ok(data) = std::fs::read_to_string(&path) {
                    if let Ok(val) = serde_json::from_str(&data) {
                        out.push((name, val));
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn watchlists_path() -> std::path::PathBuf {
    let mut p = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("apex-terminal");
    let _ = std::fs::create_dir_all(&p);
    p.push("watchlists.json");
    p
}

fn save_watchlists(watchlist: &Watchlist) {
    // DB-first: fire-and-forget through the watchlist_db worker. The worker
    // is a no-op until init() has been called, so this is safe in tests too.
    crate::persistence::watchlist_db::save_all(
        &watchlist.saved_watchlists,
        watchlist.active_watchlist_idx,
    );

    // Write-through cache to disk so offline users keep working.
    let wls: Vec<serde_json::Value> = watchlist.saved_watchlists.iter().map(|wl| {
        let sections: Vec<serde_json::Value> = wl.sections.iter().map(|sec| {
            let items: Vec<serde_json::Value> = sec.items.iter().map(|item| {
                if item.is_option {
                    serde_json::json!({ "symbol": item.symbol, "is_option": true, "underlying": item.underlying, "option_type": item.option_type, "strike": item.strike, "expiry": item.expiry, "bid": item.bid, "ask": item.ask })
                } else {
                    serde_json::json!({ "symbol": item.symbol })
                }
            }).collect();
            serde_json::json!({
                "id": sec.id,
                "title": sec.title,
                "color": sec.color,
                "collapsed": sec.collapsed,
                "items": items,
            })
        }).collect();
        serde_json::json!({
            "name": wl.name,
            "sections": sections,
            "next_section_id": wl.next_section_id,
        })
    }).collect();
    let state = serde_json::json!({
        "watchlists": wls,
        "active_idx": watchlist.active_watchlist_idx,
    });
    let _ = crate::state::persistence::atomic_write(
        &watchlists_path(),
        serde_json::to_string_pretty(&state).unwrap_or_default().as_bytes(),
    );
}

fn load_watchlists() -> (Vec<SavedWatchlist>, usize) {
    // JSON-first on the render thread — it's local file I/O (microseconds).
    // The DB load can take 1-3s on the first cold sqlx connection (TCP+TLS
    // handshake to Postgres) which would white-screen the window during
    // spawn_window. Save path writes both DB and JSON, so the JSON is a
    // valid source of truth on the same machine. Cross-machine sync (read
    // from DB when JSON is missing) happens via the fallback below — that
    // path still blocks but only when there's literally no local cache,
    // which is a one-time event per machine.
    let path = watchlists_path();
    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return default_watchlists(),
    };
    let json: serde_json::Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(_) => return default_watchlists(),
    };
    let active_idx = json.get("active_idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let wl_arr = match json.get("watchlists").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return default_watchlists(),
    };
    let mut watchlists: Vec<SavedWatchlist> = Vec::new();
    for wl_val in wl_arr {
        let name = wl_val.get("name").and_then(|v| v.as_str()).unwrap_or("Untitled").to_string();
        let next_section_id = wl_val.get("next_section_id").and_then(|v| v.as_u64()).unwrap_or(2) as u32;
        let mut sections: Vec<WatchlistSection> = Vec::new();
        if let Some(sec_arr) = wl_val.get("sections").and_then(|v| v.as_array()) {
            for sec_val in sec_arr {
                let id = sec_val.get("id").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                let title = sec_val.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let color = sec_val.get("color").and_then(|v| v.as_str()).map(|s| s.to_string());
                let collapsed = sec_val.get("collapsed").and_then(|v| v.as_bool()).unwrap_or(false);
                let mut items: Vec<WatchlistItem> = Vec::new();
                if let Some(item_arr) = sec_val.get("items").and_then(|v| v.as_array()) {
                    for item_val in item_arr {
                        let symbol = item_val.get("symbol").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if !symbol.is_empty() {
                            let is_option = item_val.get("is_option").and_then(|v| v.as_bool()).unwrap_or(false);
                            let underlying = item_val.get("underlying").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let option_type = item_val.get("option_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let strike = item_val.get("strike").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                            let expiry = item_val.get("expiry").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let bid = item_val.get("bid").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                            let ask = item_val.get("ask").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                            let sym_hash = symbol.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
                            let rvol_seed = 1.0; // neutral until real RVOL feed
                            items.push(WatchlistItem {
                                symbol, price: 0.0, prev_close: 0.0, day_close: 0.0, change_perc: None, stale: false, loaded: false,
                                is_option, underlying, option_type, strike, expiry, bid, ask,
                                pinned: false, tags: vec![], rvol: rvol_seed, atr: 0.0,
                                high_52wk: 0.0, low_52wk: 0.0, day_high: 0.0, day_low: 0.0,
                                avg_daily_range: 2.0, earnings_days: -1, alert_triggered: false, price_history: vec![],
                                prev_price: 0.0, price_change_at: None,
                            });
                        }
                    }
                }
                sections.push(WatchlistSection { id, title, color, collapsed, items });
            }
        }
        watchlists.push(SavedWatchlist { name, sections, next_section_id });
    }
    if watchlists.is_empty() { return default_watchlists(); }
    let idx = active_idx.min(watchlists.len() - 1);
    (watchlists, idx)
}

fn default_watchlists() -> (Vec<SavedWatchlist>, usize) {
    let make_items = |syms: &[&str]| -> Vec<WatchlistItem> {
        syms.iter().map(|&s| {
            let sym_hash = s.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
            let rvol_seed = 1.0; // neutral until real RVOL feed
            WatchlistItem {
                symbol: s.into(), price: 0.0, prev_close: 0.0, day_close: 0.0, change_perc: None, stale: false, loaded: false,
                is_option: false, underlying: String::new(), option_type: String::new(), strike: 0.0, expiry: String::new(), bid: 0.0, ask: 0.0,
                pinned: false, tags: vec![], rvol: rvol_seed, atr: 0.0,
                high_52wk: 0.0, low_52wk: 0.0, day_high: 0.0, day_low: 0.0,
                avg_daily_range: 2.0, earnings_days: -1, alert_triggered: false, price_history: vec![],
                prev_price: 0.0, price_change_at: None,
            }
        }).collect()
    };

    let stocks_section = WatchlistSection {
        id: 1, title: String::new(), color: None, collapsed: false, items: make_items(DEFAULT_WATCHLIST),
    };
    let stocks = SavedWatchlist { name: "Stocks".into(), sections: vec![stocks_section], next_section_id: 2 };

    let crypto_section = WatchlistSection {
        id: 1, title: String::new(), color: None, collapsed: false, items: make_items(DEFAULT_CRYPTO),
    };
    let crypto = SavedWatchlist { name: "Crypto".into(), sections: vec![crypto_section], next_section_id: 2 };

    (vec![stocks, crypto], 0)
}

/// Global sender for spawning new windows on the persistent render thread.
static SPAWN_TX: std::sync::OnceLock<Mutex<Option<mpsc::Sender<SpawnRequest>>>> = std::sync::OnceLock::new();

/// Open a new native chart window.
/// First call starts the render thread; subsequent calls send spawn requests.
pub fn open_window(rx: mpsc::Receiver<ChartCommand>, initial_cmd: ChartCommand) {
    let spawn_tx_lock = SPAWN_TX.get_or_init(|| Mutex::new(None));
    let mut guard = spawn_tx_lock.lock().unwrap();

    // Try sending to existing render thread
    let req = SpawnRequest { rx, initial_cmd };
    let req = if let Some(tx) = guard.as_ref() {
        match tx.send(req) {
            Ok(()) => return, // success — render thread got it
            Err(mpsc::SendError(r)) => r, // thread died — get req back, restart below
        }
    } else { req };

    // First call or render thread died — start the render thread
    let (spawn_tx, spawn_rx) = mpsc::channel();
    let _ = spawn_tx.send(req);
    *guard = Some(spawn_tx);

    std::thread::spawn(move || {
        #[cfg(target_os = "windows")]
        let el = {
            use winit::platform::windows::EventLoopBuilderExtWindows;
            EventLoop::builder().with_any_thread(true).build().unwrap()
        };
        #[cfg(not(target_os = "windows"))]
        let el = EventLoop::builder().build().unwrap();
        // Wave 2 (state): create the registry and spawn the persist supervisor
        // before the first window opens so every Store<T> registered during
        // spawn_window() is already being walked on the next tick.
        let store_registry = crate::state::StoreRegistry::new();
        let persist_supervisor = crate::state::spawn_persist_supervisor(store_registry.clone());
        let mut app = App {
            iw: 1920, ih: 1080,
            windows: Vec::new(),
            #[cfg(feature = "design-mode")]
            inspector_window: None,
            spawn_rx,
            store_registry, persist_supervisor,
        };
        let _ = el.run_app(&mut app);
        // All windows closed — clear the spawn sender so next call restarts
        if let Some(lock) = SPAWN_TX.get() {
            *lock.lock().unwrap() = None;
        }
    });
}

/// macOS requires the winit event loop on the main thread.
/// Call this from `main()` instead of `open_window`; it blocks until all windows close.
#[cfg(target_os = "macos")]
pub fn open_window_blocking(rx: mpsc::Receiver<ChartCommand>, initial_cmd: ChartCommand) {
    use winit::platform::macos::EventLoopBuilderExtMacOS;

    let spawn_tx_lock = SPAWN_TX.get_or_init(|| Mutex::new(None));
    let (spawn_tx, spawn_rx) = mpsc::channel::<SpawnRequest>();
    let _ = spawn_tx.send(SpawnRequest { rx, initial_cmd });
    *spawn_tx_lock.lock().unwrap() = Some(spawn_tx);

    let el = EventLoop::builder()
        .with_activate_ignoring_other_apps(true)
        .build()
        .unwrap();
    // Wave 2 (state): create the registry and spawn the persist supervisor.
    let store_registry = crate::state::StoreRegistry::new();
    let persist_supervisor = crate::state::spawn_persist_supervisor(store_registry.clone());
    let mut app = App {
        iw: 1920, ih: 1080,
        windows: Vec::new(),
        #[cfg(feature = "design-mode")]
        inspector_window: None,
        spawn_rx, store_registry, persist_supervisor,
    };
    let _ = el.run_app(&mut app);
    *spawn_tx_lock.lock().unwrap() = None;
}

#[cfg(test)]
mod synthesize_occ_tests {
    use super::synthesize_occ;

    #[test]
    fn integer_strike() {
        // SPY 450C on 2026-05-07 → O:SPY260507C00450000
        let occ = synthesize_occ("SPY", 450.0, true, "2026-05-07");
        assert_eq!(occ, "O:SPY260507C00450000");
    }

    #[test]
    fn decimal_strike() {
        // 287.5 must become 00287500 (the bug we shipped a fix for).
        let occ = synthesize_occ("AAPL", 287.5, true, "2026-04-30");
        assert!(occ.ends_with("C00287500"), "got: {occ}");
    }

    #[test]
    fn sub_dollar_strike() {
        // 75¢ option → 00000750
        let occ = synthesize_occ("XYZ", 0.75, true, "2026-05-04");
        assert!(occ.ends_with("C00000750"), "got: {occ}");
    }

    #[test]
    fn iso_date_round_trip() {
        let occ = synthesize_occ("AAPL", 100.0, true, "2026-05-04");
        // YYMMDD = 260504
        assert!(occ.contains("260504"), "got: {occ}");
    }

    #[test]
    fn put_vs_call() {
        let c = synthesize_occ("SPY", 450.0, true,  "2026-05-07");
        let p = synthesize_occ("SPY", 450.0, false, "2026-05-07");
        assert!(c.contains('C'), "call missing C: {c}");
        assert!(p.contains('P'), "put missing P: {p}");
        assert_ne!(c, p);
    }

    #[test]
    fn spx_maps_to_spxw() {
        // Polygon stores SPX index options under SPXW root.
        let occ = synthesize_occ("SPX", 5000.0, true, "2026-05-07");
        assert!(occ.starts_with("O:SPXW"), "got: {occ}");
    }

    #[test]
    fn ndx_maps_to_ndxp() {
        let occ = synthesize_occ("NDX", 18000.0, true, "2026-05-07");
        assert!(occ.starts_with("O:NDXP"), "got: {occ}");
    }

    #[test]
    fn spxw_passes_through() {
        // If caller already used SPXW, don't double-map.
        let occ = synthesize_occ("SPXW", 5000.0, true, "2026-05-07");
        assert!(occ.starts_with("O:SPXW"));
    }

    #[test]
    fn aapl_passes_through() {
        let occ = synthesize_occ("AAPL", 200.0, true, "2026-05-07");
        assert!(occ.starts_with("O:AAPL"), "got: {occ}");
    }
}

#[cfg(test)]
mod replay_overlay_tests {
    use super::{Bar, Chart, ReplayOverlay};

    fn bar(o: f32, h: f32, l: f32, c: f32) -> Bar {
        Bar { open: o, high: h, low: l, close: c, volume: 100.0, _pad: 0.0 }
    }

    #[test]
    fn default_overlay_is_none() {
        let c = Chart::new();
        assert!(c.replay_overlay.is_none());
    }

    #[test]
    fn set_installs_overlay() {
        let mut c = Chart::new();
        let mut o = ReplayOverlay::new("Replay: 2026-04-15 10:30:00");
        o.push(bar(100.0, 101.0, 99.5, 100.5), 1_700_000_000_000);
        c.set_replay_overlay(o);
        let installed = c.replay_overlay.as_ref().expect("overlay installed");
        assert_eq!(installed.bars.len(), 1);
        assert_eq!(installed.timestamps.len(), 1);
        assert_eq!(installed.label, "Replay: 2026-04-15 10:30:00");
        assert_eq!(installed.color, ReplayOverlay::DEFAULT_COLOR);
    }

    #[test]
    fn append_creates_when_none_and_grows() {
        let mut c = Chart::new();
        assert!(c.replay_overlay.is_none());
        c.append_replay_bar(bar(1.0, 2.0, 0.5, 1.5), 1);
        c.append_replay_bar(bar(1.5, 2.5, 1.0, 2.0), 2);
        let o = c.replay_overlay.as_ref().unwrap();
        assert_eq!(o.bars.len(), 2);
        assert_eq!(o.timestamps, vec![1, 2]);
    }

    #[test]
    fn append_respects_existing_overlay() {
        let mut c = Chart::new();
        c.set_replay_overlay(ReplayOverlay::new("session-A"));
        c.append_replay_bar(bar(10.0, 11.0, 9.0, 10.5), 42);
        let o = c.replay_overlay.as_ref().unwrap();
        assert_eq!(o.label, "session-A");
        assert_eq!(o.bars.len(), 1);
        assert_eq!(o.timestamps, vec![42]);
    }

    #[test]
    fn clear_removes_overlay() {
        let mut c = Chart::new();
        c.set_replay_overlay(ReplayOverlay::new("x"));
        assert!(c.replay_overlay.is_some());
        c.clear_replay_overlay();
        assert!(c.replay_overlay.is_none());
    }

    #[test]
    fn custom_color_is_preserved() {
        let mut o = ReplayOverlay::new("");
        o.color = egui::Color32::from_rgb(0x00, 0xff, 0xff);
        let mut c = Chart::new();
        c.set_replay_overlay(o);
        assert_eq!(c.replay_overlay.unwrap().color, egui::Color32::from_rgb(0x00, 0xff, 0xff));
    }
}

mod tab_cache_lru_tests {
    use super::{evict_oldest_if_full, Bar, TAB_CACHE_MAX};
    use std::collections::HashMap;
    use std::time::{Duration, Instant};


    #[test]
    fn evict_drops_oldest_when_full() {
        let mut cache: HashMap<(String, String), (Vec<Bar>, Vec<i64>, Instant)> = HashMap::new();
        let base = Instant::now();
        // Insert TAB_CACHE_MAX + 1 entries with monotonically increasing Instants,
        // calling evict before each insert (matching real call-site contract).
        for i in 0..(TAB_CACHE_MAX + 1) {
            evict_oldest_if_full(&mut cache);
            let ts = base + Duration::from_millis(i as u64);
            cache.insert((format!("S{i}"), "1m".into()), (vec![], vec![], ts));
        }
        assert_eq!(cache.len(), TAB_CACHE_MAX,
            "post-insert size must be capped at TAB_CACHE_MAX");
        // S0 had the oldest Instant — it should be gone.
        assert!(!cache.contains_key(&("S0".to_string(), "1m".to_string())),
            "oldest entry should have been evicted");
        // The most recent (S_{MAX}) should still be present.
        let newest = format!("S{}", TAB_CACHE_MAX);
        assert!(cache.contains_key(&(newest.clone(), "1m".to_string())),
            "newest entry should remain");
    }

    #[test]
    fn evict_is_noop_when_under_cap() {
        let mut cache: HashMap<(String, String), (Vec<Bar>, Vec<i64>, Instant)> = HashMap::new();
        let base = Instant::now();
        for i in 0..(TAB_CACHE_MAX - 1) {
            cache.insert((format!("S{i}"), "1m".into()),
                (vec![], vec![], base + Duration::from_millis(i as u64)));
        }
        let before = cache.len();
        evict_oldest_if_full(&mut cache);
        assert_eq!(cache.len(), before, "evict should not touch a sub-cap cache");
    }
}


#[cfg(test)]
mod pane_event_apply_tests {
    //! Wave 12c: contract tests for `apply_pane_events`. The render
    //! loop in `App::about_to_wait` drains the SubscriptionBus once per
    //! frame and delegates to this helper; these tests exercise it in
    //! isolation (no winit / wgpu / network) so the propagation
    //! contract is locked in.

    use super::*;
    use crate::state::{PaneEvent, BROADCAST_GROUP};

    fn chart(symbol: &str, tf: &str, link_group: u8) -> Chart {
        let mut c = Chart::new_with(symbol, tf);
        c.link_group = link_group;
        // Give the pane non-empty bars so timeframe-change's tab-cache
        // stash branch is exercised; symbol-change tests don't care.
        c.bars.push(Bar { open: 1.0, high: 1.0, low: 1.0, close: 1.0, volume: 1.0, _pad: 0.0 });
        c.timestamps.push(0);
        c
    }

    #[test]
    fn symbol_change_propagates_to_link_group_siblings_only() {
        // 4 panes: pane 0 in group 1, panes 1+2 in group 1, pane 3 unlinked.
        let mut panes = vec![
            chart("AAPL", "5m", 1), // originator
            chart("MSFT", "5m", 1),
            chart("NVDA", "5m", 1),
            chart("TSLA", "5m", 0), // unlinked
        ];
        let events = vec![(
            PaneEvent::SymbolChanged { group: 1, symbol: "AAPL".into() },
            Some(0usize),
        )];
        // group_count=2 (groups 1..=2 are valid). apply_bars_fetch=false
        // so we don't kick a background HTTP request from tests.
        apply_pane_events(&mut panes, &events, 2, false);

        assert_eq!(panes[0].symbol, "AAPL", "originator unchanged");
        assert_eq!(panes[1].symbol, "AAPL", "sibling in group 1 updated");
        assert_eq!(panes[2].symbol, "AAPL", "sibling in group 1 updated");
        assert_eq!(panes[3].symbol, "TSLA", "unlinked pane untouched");

        // Sibling panes had bars cleared + indicator counters reset
        // (the contract the imperative loop also enforced).
        assert!(panes[1].bars.is_empty());
        assert!(panes[2].bars.is_empty());
        assert_eq!(panes[1].indicator_bar_count, 0);
        assert_eq!(panes[2].indicator_bar_count, 0);

        // Originator's own bars are untouched by apply (the per-pane
        // loop in about_to_wait handles the originator separately).
        assert!(!panes[0].bars.is_empty());
        assert!(!panes[3].bars.is_empty());
    }

    #[test]
    fn broadcast_group_applies_to_every_pane_except_origin() {
        let mut panes = vec![
            chart("AAPL", "5m", 0),
            chart("MSFT", "5m", 1),
            chart("NVDA", "5m", 2),
            chart("TSLA", "5m", 0),
        ];
        let events = vec![(
            PaneEvent::SymbolChanged { group: BROADCAST_GROUP, symbol: "SPY".into() },
            Some(0usize),
        )];
        // group_count=0: real groups would be rejected, but BROADCAST_GROUP
        // bypasses validation by design.
        apply_pane_events(&mut panes, &events, 0, false);

        assert_eq!(panes[0].symbol, "AAPL", "originator skipped");
        assert_eq!(panes[1].symbol, "SPY");
        assert_eq!(panes[2].symbol, "SPY");
        assert_eq!(panes[3].symbol, "SPY");
    }

    #[test]
    fn invalid_group_id_is_dropped() {
        // group=5 but only 2 link groups exist → don't propagate to anyone.
        let mut panes = vec![
            chart("AAPL", "5m", 5),
            chart("MSFT", "5m", 5),
        ];
        let events = vec![(
            PaneEvent::SymbolChanged { group: 5, symbol: "ZZZ".into() },
            Some(0usize),
        )];
        apply_pane_events(&mut panes, &events, 2, false);
        assert_eq!(panes[0].symbol, "AAPL");
        assert_eq!(panes[1].symbol, "MSFT", "stale group id must not propagate");
    }

    #[test]
    fn zero_group_id_is_dropped() {
        // group=0 means "unlinked" — should never propagate via apply.
        let mut panes = vec![
            chart("AAPL", "5m", 0),
            chart("MSFT", "5m", 0),
        ];
        let events = vec![(
            PaneEvent::SymbolChanged { group: 0, symbol: "ZZZ".into() },
            Some(0usize),
        )];
        apply_pane_events(&mut panes, &events, 2, false);
        assert_eq!(panes[0].symbol, "AAPL");
        assert_eq!(panes[1].symbol, "MSFT", "group=0 must not propagate");
    }

    #[test]
    fn matching_symbol_sibling_is_skipped() {
        // Sibling already has the target symbol — apply should be a no-op
        // for it (preserves the prior loop's `pane.symbol != sym` guard).
        let mut panes = vec![
            chart("AAPL", "5m", 1),
            chart("AAPL", "5m", 1),
        ];
        let events = vec![(
            PaneEvent::SymbolChanged { group: 1, symbol: "AAPL".into() },
            Some(0usize),
        )];
        apply_pane_events(&mut panes, &events, 2, false);
        assert_eq!(panes[1].symbol, "AAPL");
        assert!(!panes[1].bars.is_empty(), "matching-symbol sibling: bars preserved");
    }

    #[test]
    fn timeframe_change_propagates_to_link_group() {
        let mut panes = vec![
            chart("AAPL", "5m", 1),
            chart("MSFT", "5m", 1),
            chart("TSLA", "5m", 0),
        ];
        let events = vec![(
            PaneEvent::TimeframeChanged { group: 1, timeframe: "1h".into() },
            Some(0usize),
        )];
        apply_pane_events(&mut panes, &events, 2, false);
        assert_eq!(panes[0].timeframe, "5m", "originator unchanged");
        assert_eq!(panes[1].timeframe, "1h", "sibling group 1 updated");
        assert_eq!(panes[2].timeframe, "5m", "unlinked pane untouched");
    }

    #[test]
    fn layout_event_drains_without_effect() {
        let mut panes = vec![chart("AAPL", "5m", 1)];
        let events = vec![(PaneEvent::LayoutChanged, None)];
        apply_pane_events(&mut panes, &events, 2, false);
        // No fields changed — but the drain still consumed the event.
        assert_eq!(panes[0].symbol, "AAPL");
        assert_eq!(panes[0].timeframe, "5m");
    }

    // ── Wave 13a: ToggleChanged / SwingLegModeChanged contract ──

    use crate::state::PaneToggle;

    #[test]
    fn toggle_propagates_to_link_group_siblings_only() {
        // 4 panes: pane 0 originator (group 1), pane 1+2 also group 1,
        // pane 3 unlinked. log_scale toggle should reach 1+2 only.
        let mut panes = vec![
            chart("AAPL", "5m", 1),
            chart("MSFT", "5m", 1),
            chart("NVDA", "5m", 1),
            chart("TSLA", "5m", 0),
        ];
        // simulate originator already flipped to true
        panes[0].log_scale = true;
        let events = vec![(
            PaneEvent::ToggleChanged { group: 1, kind: PaneToggle::LogScale, value: true },
            Some(0usize),
        )];
        apply_pane_events(&mut panes, &events, 2, false);
        assert!(panes[0].log_scale, "originator unchanged (caller pre-set)");
        assert!(panes[1].log_scale, "sibling group 1 updated");
        assert!(panes[2].log_scale, "sibling group 1 updated");
        assert!(!panes[3].log_scale, "unlinked pane untouched");
    }

    #[test]
    fn toggle_broadcast_applies_to_every_pane_except_origin() {
        let mut panes = vec![
            chart("AAPL", "5m", 0),
            chart("MSFT", "5m", 1),
            chart("NVDA", "5m", 2),
            chart("TSLA", "5m", 0),
        ];
        // pre-set originator's show_volume to false to verify it stays put
        panes[0].show_volume = true;
        for p in &mut panes[1..] { p.show_volume = true; }
        // originator just flipped to false
        panes[0].show_volume = false;
        let events = vec![(
            PaneEvent::ToggleChanged {
                group: BROADCAST_GROUP, kind: PaneToggle::ShowVolume, value: false,
            },
            Some(0usize),
        )];
        apply_pane_events(&mut panes, &events, 0, false);
        assert!(!panes[0].show_volume, "originator (already pre-flipped)");
        assert!(!panes[1].show_volume);
        assert!(!panes[2].show_volume);
        assert!(!panes[3].show_volume);
    }

    #[test]
    fn toggle_invalid_group_id_is_dropped() {
        let mut panes = vec![
            chart("AAPL", "5m", 5),
            chart("MSFT", "5m", 5),
        ];
        let events = vec![(
            PaneEvent::ToggleChanged { group: 5, kind: PaneToggle::ShowCvd, value: true },
            Some(0usize),
        )];
        apply_pane_events(&mut panes, &events, 2, false);
        assert!(!panes[0].show_cvd);
        assert!(!panes[1].show_cvd, "stale group id must not propagate");
    }

    #[test]
    fn toggle_zero_group_is_dropped() {
        let mut panes = vec![
            chart("AAPL", "5m", 0),
            chart("MSFT", "5m", 0),
        ];
        let events = vec![(
            PaneEvent::ToggleChanged { group: 0, kind: PaneToggle::OhlcTooltip, value: false },
            Some(0usize),
        )];
        apply_pane_events(&mut panes, &events, 2, false);
        assert!(panes[0].ohlc_tooltip, "default true preserved");
        assert!(panes[1].ohlc_tooltip, "group=0 must not propagate");
    }

    #[test]
    fn toggle_with_no_origin_applies_to_every_match() {
        // origin=None: dispatcher fans to all matching panes incl
        // would-be-originator. Used when the caller already wrote
        // their own field separately (top_nav pattern uses Some(ap)
        // explicitly, but command-palette-style callers may not).
        let mut panes = vec![
            chart("AAPL", "5m", 1),
            chart("MSFT", "5m", 1),
        ];
        let events = vec![(
            PaneEvent::ToggleChanged { group: 1, kind: PaneToggle::ShowMaRibbon, value: true },
            None,
        )];
        apply_pane_events(&mut panes, &events, 2, false);
        assert!(panes[0].show_ma_ribbon);
        assert!(panes[1].show_ma_ribbon);
    }

    #[test]
    fn toggle_dispatcher_covers_every_pane_toggle_variant() {
        // Sanity: every PaneToggle variant must dispatch to a Chart
        // field. If a future variant is added without an apply arm,
        // this test will fail to compile (exhaustive `match`) — see
        // apply_pane_events::ToggleChanged.
        let kinds = [
            PaneToggle::LogScale, PaneToggle::OhlcTooltip, PaneToggle::MeasureTooltip,
            PaneToggle::ShowVolume, PaneToggle::ShowDeltaVolume, PaneToggle::ShowRvol,
            PaneToggle::ShowMaRibbon, PaneToggle::ShowCvd, PaneToggle::ShowPrevClose,
            PaneToggle::ShowPatternLabels, PaneToggle::ShowFootprint,
            PaneToggle::ShowAutoFib, PaneToggle::HitHighlight,
        ];
        let mut panes = vec![chart("AAPL", "5m", 1), chart("MSFT", "5m", 1)];
        for kind in kinds {
            let events = vec![(
                PaneEvent::ToggleChanged { group: 1, kind, value: true },
                Some(0usize),
            )];
            apply_pane_events(&mut panes, &events, 2, false);
        }
        // After flipping every variant true on the sibling, spot-check
        // a handful of fields landed correctly.
        assert!(panes[1].log_scale);
        assert!(panes[1].show_cvd);
        assert!(panes[1].show_footprint);
        assert!(panes[1].hit_highlight);
    }

    #[test]
    fn swing_leg_mode_propagates_to_link_group() {
        let mut panes = vec![
            chart("AAPL", "5m", 1),
            chart("MSFT", "5m", 1),
            chart("TSLA", "5m", 0),
        ];
        panes[0].swing_leg_mode = 2;
        let events = vec![(
            PaneEvent::SwingLegModeChanged { group: 1, value: 2 },
            Some(0usize),
        )];
        apply_pane_events(&mut panes, &events, 2, false);
        assert_eq!(panes[0].swing_leg_mode, 2, "originator untouched");
        assert_eq!(panes[1].swing_leg_mode, 2, "sibling group 1 cycled");
        assert_eq!(panes[2].swing_leg_mode, 0, "unlinked pane untouched");
    }

    #[test]
    fn swing_leg_mode_broadcast_applies_to_every_pane_except_origin() {
        let mut panes = vec![
            chart("AAPL", "5m", 0),
            chart("MSFT", "5m", 1),
            chart("NVDA", "5m", 2),
        ];
        panes[0].swing_leg_mode = 1;
        let events = vec![(
            PaneEvent::SwingLegModeChanged { group: BROADCAST_GROUP, value: 1 },
            Some(0usize),
        )];
        apply_pane_events(&mut panes, &events, 0, false);
        assert_eq!(panes[0].swing_leg_mode, 1);
        assert_eq!(panes[1].swing_leg_mode, 1);
        assert_eq!(panes[2].swing_leg_mode, 1);
    }

    // ── Wave 14a: indicator mass-mutation event contract ──

    #[test]
    fn indicator_visibility_propagates_to_siblings_only() {
        let mut panes = vec![
            chart("AAPL", "5m", 1),
            chart("MSFT", "5m", 1),
            chart("NVDA", "5m", 1),
            chart("TSLA", "5m", 0),
        ];
        // Clear Chart::new's default indicator set so each test owns
        // its `indicators` Vec, then give every pane one SMA(20).
        for (i, p) in panes.iter_mut().enumerate() {
            p.indicators.clear();
            p.indicators.push(Indicator::new(100 + i as u32, IndicatorType::SMA, 20, "#00bef0"));
        }
        // Originator just flipped its SMA off.
        panes[0].indicators[0].visible = false;
        let events = vec![(
            PaneEvent::IndicatorVisibilityChanged { group: 1, kind: IndicatorType::SMA, visible: false },
            Some(0usize),
        )];
        apply_pane_events(&mut panes, &events, 2, false);
        assert!(!panes[0].indicators[0].visible, "originator pre-set");
        assert!(!panes[1].indicators[0].visible, "sibling group 1");
        assert!(!panes[2].indicators[0].visible, "sibling group 1");
        assert!(panes[3].indicators[0].visible, "unlinked pane untouched");
    }

    #[test]
    fn indicators_removed_by_kind_period_predicate_matches_originator_intent() {
        let mut panes = vec![
            chart("AAPL", "5m", 1),
            chart("MSFT", "5m", 1),
            chart("TSLA", "5m", 0),
        ];
        // Sibling has SMA(20), SMA(50), and EMA(20). After remove
        // event for (SMA, Some(20)), only SMA(20) should drop.
        for p in panes.iter_mut() {
            p.indicators.clear();
            p.indicators.push(Indicator::new(1, IndicatorType::SMA, 20, "#aaa"));
            p.indicators.push(Indicator::new(2, IndicatorType::SMA, 50, "#bbb"));
            p.indicators.push(Indicator::new(3, IndicatorType::EMA, 20, "#ccc"));
            // Simulate already-warm sibling counter that should reset.
            p.indicator_bar_count = 42;
        }
        let events = vec![(
            PaneEvent::IndicatorsRemoved { group: 1, kind: IndicatorType::SMA, period: Some(20) },
            Some(0usize),
        )];
        apply_pane_events(&mut panes, &events, 2, false);
        // Originator untouched by dispatcher (it pre-removed locally).
        assert_eq!(panes[0].indicators.len(), 3);
        assert_eq!(panes[0].indicator_bar_count, 42);
        // Sibling: SMA(20) dropped, SMA(50) and EMA(20) preserved.
        assert_eq!(panes[1].indicators.len(), 2);
        assert!(panes[1].indicators.iter().any(|i| i.kind == IndicatorType::SMA && i.period == 50));
        assert!(panes[1].indicators.iter().any(|i| i.kind == IndicatorType::EMA && i.period == 20));
        assert_eq!(panes[1].indicator_bar_count, 0, "sibling counter reset on actual removal");
        // Unlinked pane: untouched.
        assert_eq!(panes[2].indicators.len(), 3);
        assert_eq!(panes[2].indicator_bar_count, 42);

        // period: None form removes ALL of the kind.
        let events = vec![(
            PaneEvent::IndicatorsRemoved { group: 1, kind: IndicatorType::SMA, period: None },
            Some(0usize),
        )];
        apply_pane_events(&mut panes, &events, 2, false);
        assert_eq!(panes[1].indicators.len(), 1, "all SMA gone, EMA remains");
        assert!(panes[1].indicators.iter().all(|i| i.kind == IndicatorType::EMA));
    }

    #[test]
    fn indicator_added_clones_into_each_sibling_with_fresh_id() {
        let mut panes = vec![
            chart("AAPL", "5m", 1),
            chart("MSFT", "5m", 1),
            chart("NVDA", "5m", 1),
            chart("TSLA", "5m", 0),
        ];
        // Clear default indicator set; pre-set distinct
        // next_indicator_id per sibling so we can verify each pane
        // allocates from its OWN counter, not the originator's.
        for p in panes.iter_mut() { p.indicators.clear(); }
        panes[1].next_indicator_id = 77;
        panes[2].next_indicator_id = 200;
        let mut original = Indicator::new(42, IndicatorType::EMA, 12, "#f0d732");
        original.visible = true;
        // Originator already pushed its own copy at id=42.
        panes[0].indicators.push(original.clone());
        let events = vec![(
            PaneEvent::IndicatorAdded { group: 1, indicator: original },
            Some(0usize),
        )];
        apply_pane_events(&mut panes, &events, 2, false);
        // Originator unchanged (skipped by Some(0)).
        assert_eq!(panes[0].indicators.len(), 1);
        assert_eq!(panes[0].indicators[0].id, 42);
        // Siblings got the indicator with their own next id.
        assert_eq!(panes[1].indicators.len(), 1);
        assert_eq!(panes[1].indicators[0].id, 77);
        assert_eq!(panes[1].indicators[0].kind, IndicatorType::EMA);
        assert_eq!(panes[1].indicators[0].period, 12);
        assert_eq!(panes[1].next_indicator_id, 78, "sibling counter advanced");
        assert_eq!(panes[1].indicator_bar_count, 0);
        assert_eq!(panes[2].indicators.len(), 1);
        assert_eq!(panes[2].indicators[0].id, 200);
        assert_eq!(panes[2].next_indicator_id, 201);
        // Unlinked pane untouched.
        assert_eq!(panes[3].indicators.len(), 0);
    }
}
