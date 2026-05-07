//! Native GPU chart renderer — winit (any_thread) + egui for all rendering.
//! egui handles UI + chart painting. winit handles window on non-main thread.

use std::sync::{mpsc, Arc, Mutex};
use std::fmt::Write as FmtWrite;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes},
    dpi::PhysicalSize,
};

use super::{Bar, ChartCommand, Drawing, DrawingKind, DrawingGroup, DrawingSignificance, LineStyle, PatternLabel};

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
    pub(crate) static PENDING_TOASTS: std::cell::RefCell<Vec<(String, f32, bool)>> = const { std::cell::RefCell::new(Vec::new()) };
    pub(crate) static TB_BTN_CLICKED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static CONN_PANEL_OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(crate) static CROSSHAIR_SYNC_TIME: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
    pub(crate) static PENDING_WL_TOOLTIP: std::cell::RefCell<Option<WlTooltipData>> = const { std::cell::RefCell::new(None) };
    pub(crate) static ALERT_BADGE_HITS: std::cell::RefCell<Vec<AlertBadgeHit>> = const { std::cell::RefCell::new(Vec::new()) };
    #[cfg(feature = "design-mode")]
    static DESIGN_INSPECTOR: std::cell::RefCell<Option<crate::design_inspector::Inspector>> = const { std::cell::RefCell::new(None) };
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

use crate::ui_kit::{self, icons::Icon};

use super::trading::*;

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
// Hierarchy comes from color_alpha() opacity stops (ALPHA_GHOST through
// ALPHA_HEAVY), NOT from new hues. If you find yourself adding a 7th color,
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
    pub(crate) warn:   egui::Color32,
    /// Shared command-palette category badges. Theme-invariant. Documented
    /// exception to the 6-color rule — each slot is a distinct hue by design.
    pub(crate) cmd_palette: [egui::Color32; 11],
    // ── Legacy fields (kept for back-compat; prefer derived getters) ────────
    // These remain as fields so existing call-sites compile. New code should
    // use the deprecated getter forms below (e.g. `t.gold()`) which warn and
    // route through `color_alpha`. Eventually these fields will be removed.
    /// LEGACY: use `color_alpha(t.accent, ALPHA_HEAVY)`.
    pub(crate) gold: egui::Color32,
    /// LEGACY: use `t.bear` (or `t.warn` for non-loss alerts).
    pub(crate) notification_red: egui::Color32,
    /// LEGACY: use `color_alpha(t.bg, ALPHA_HEAVY)` — pure-black baseline.
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
    /// LEGACY: use `color_alpha(t.accent, ALPHA_GHOST)`.
    pub(crate) pinned_row_tint: egui::Color32,
    /// LEGACY: use `color_alpha(t.dim, ALPHA_HEAVY)`.
    pub(crate) text_muted: egui::Color32,
    /// LEGACY: use `color_alpha(t.bg, ALPHA_SOLID)` for HUD overlays.
    pub(crate) hud_bg: egui::Color32,
    /// LEGACY: use `t.toolbar_border`.
    pub(crate) hud_border: egui::Color32,
}

impl Theme {
    /// Single border color in the 6-color contract. Aliases `toolbar_border`
    /// (which is kept as a field for call-site compatibility).
    #[inline]
    pub(crate) fn border(&self) -> egui::Color32 { self.toolbar_border }
}
pub(crate) const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 { egui::Color32::from_rgb(r, g, b) }
/// Premultiplied RGBA — all callers must pass already-premultiplied RGB components.
pub(crate) const fn rgba_pre(r: u8, g: u8, b: u8, a: u8) -> egui::Color32 { egui::Color32::from_rgba_premultiplied(r, g, b, a) }

/// UI style presets — placeholder names for now. Selected style is shown
/// alongside the theme as e.g. "GruvBox/Meridien". Actual visual differences
/// will be wired later.
pub(crate) const STYLE_NAMES: &[&str] = &[
    "Meridien", "Aperture", "Octave", "Cadence", "Chord",
    "Lattice",  "Tangent",  "Tempo",  "Contour", "Relay",
];

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

pub(crate) const THEMES: &[Theme] = &[
    Theme { name: "Midnight",    bg: rgb(14,16,21),   bull: rgb(62,120,180),  bear: rgb(180,65,58),   dim: rgb(100,105,115), toolbar_bg: rgb(10,12,17),  toolbar_border: rgb(28,32,40),  accent: rgb(62,120,180),  text: rgb(220,220,230),  warn: rgb(255,191,  0), notification_red: rgb(231, 76, 60), gold: rgb(255,193, 37), shadow_color: rgb(0,0,0),       overlay_text: rgb(240,240,250), rrg_leading: rgb(56,203,137), rrg_improving: rgb(74,158,255), rrg_weakening: rgb(230,200,50), rrg_lagging: rgb(224,82,82), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(3,5,9,12), text_muted: rgb(180,180,195), hud_bg: rgba_pre(12,12,18,230), hud_border: rgb(50,52,64) },
    Theme { name: "Nord",        bg: rgb(38,44,56),   bull: rgb(163,190,140), bear: rgb(191,97,106),  dim: rgb(129,161,193), toolbar_bg: rgb(32,38,50),  toolbar_border: rgb(50,56,70),  accent: rgb(136,192,208), text: rgb(220,220,230),  warn: rgb(235,203,139), notification_red: rgb(191, 97,106), gold: rgb(235,203,139), shadow_color: rgb(0,0,0),       overlay_text: rgb(236,239,244), rrg_leading: rgb(163,190,140), rrg_improving: rgb(136,192,208), rrg_weakening: rgb(235,203,139), rrg_lagging: rgb(191,97,106), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(5,7,9,14), text_muted: rgb(175,180,190), hud_bg: rgba_pre(30,34,46,230), hud_border: rgb(60,66,80) },
    Theme { name: "Monokai",     bg: rgb(39,40,34),   bull: rgb(166,226,46),  bear: rgb(249,38,114),  dim: rgb(165,159,133), toolbar_bg: rgb(33,34,28),  toolbar_border: rgb(55,54,44),  accent: rgb(230,219,116), text: rgb(220,220,230),  warn: rgb(230,219,116), notification_red: rgb(249, 38,114), gold: rgb(255,193, 37), shadow_color: rgb(0,0,0),       overlay_text: rgb(248,248,240), rrg_leading: rgb(166,226, 46), rrg_improving: rgb(102,217,239), rrg_weakening: rgb(230,219,116), rrg_lagging: rgb(249,38,114), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(4,10,11,12), text_muted: rgb(180,178,160), hud_bg: rgba_pre(30,30,24,230), hud_border: rgb(55,54,44) },
    Theme { name: "Solarized",   bg: rgb(0,43,54),    bull: rgb(133,153,0),   bear: rgb(220,50,47),   dim: rgb(131,148,150), toolbar_bg: rgb(0,37,48),   toolbar_border: rgb(7,54,66),   accent: rgb(42,161,152),  text: rgb(220,220,230),  warn: rgb(181,137,  0), notification_red: rgb(220, 50, 47), gold: rgb(181,137,  0), shadow_color: rgb(0,0,0),       overlay_text: rgb(253,246,227), rrg_leading: rgb(133,153,  0), rrg_improving: rgb( 38,139,210), rrg_weakening: rgb(181,137,  0), rrg_lagging: rgb(220,50, 47), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(1,6,9,12), text_muted: rgb(156,172,175), hud_bg: rgba_pre(0,28,36,230), hud_border: rgb(7,54,66) },
    Theme { name: "Dracula",     bg: rgb(40,42,54),   bull: rgb(80,250,123),  bear: rgb(255,85,85),   dim: rgb(189,147,249), toolbar_bg: rgb(34,36,48),  toolbar_border: rgb(52,55,70),  accent: rgb(255,121,198), text: rgb(220,220,230),  warn: rgb(241,250,140), notification_red: rgb(255, 85, 85), gold: rgb(241,250,140), shadow_color: rgb(0,0,0),       overlay_text: rgb(248,248,242), rrg_leading: rgb( 80,250,123), rrg_improving: rgb(139,233,253), rrg_weakening: rgb(241,250,140), rrg_lagging: rgb(255,85, 85), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(6,10,11,12), text_muted: rgb(190,185,215), hud_bg: rgba_pre(30,32,44,230), hud_border: rgb(55,58,75) },
    Theme { name: "Gruvbox",     bg: rgb(40,40,40),   bull: rgb(184,187,38),  bear: rgb(251,73,52),   dim: rgb(213,196,161), toolbar_bg: rgb(34,34,34),  toolbar_border: rgb(55,52,50),  accent: rgb(254,128,25),  text: rgb(220,220,230),  warn: rgb(250,189, 47), notification_red: rgb(251, 73, 52), gold: rgb(250,189, 47), shadow_color: rgb(0,0,0),       overlay_text: rgb(235,219,178), rrg_leading: rgb(184,187, 38), rrg_improving: rgb(131,165,152), rrg_weakening: rgb(250,189, 47), rrg_lagging: rgb(251,73, 52), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(6,8,7,13), text_muted: rgb(185,178,160), hud_bg: rgba_pre(28,28,28,230), hud_border: rgb(60,56,50) },
    Theme { name: "Catppuccin",  bg: rgb(30,30,46),   bull: rgb(166,227,161), bear: rgb(243,139,168), dim: rgb(180,190,254), toolbar_bg: rgb(24,24,38),  toolbar_border: rgb(49,50,68),  accent: rgb(203,166,247), text: rgb(220,220,230),  warn: rgb(249,226,175), notification_red: rgb(243,139,168), gold: rgb(249,226,175), shadow_color: rgb(0,0,0),       overlay_text: rgb(205,214,244), rrg_leading: rgb(166,227,161), rrg_improving: rgb(137,220,235), rrg_weakening: rgb(249,226,175), rrg_lagging: rgb(243,139,168), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(6,8,11,12), text_muted: rgb(182,186,220), hud_bg: rgba_pre(20,20,36,230), hud_border: rgb(49,50,68) },
    Theme { name: "Tokyo Night", bg: rgb(26,27,38),   bull: rgb(158,206,106), bear: rgb(247,118,142), dim: rgb(122,162,247), toolbar_bg: rgb(21,22,32),  toolbar_border: rgb(36,40,59),  accent: rgb(125,207,255), text: rgb(220,220,230),  warn: rgb(224,175,104), notification_red: rgb(247,118,142), gold: rgb(224,175,104), shadow_color: rgb(0,0,0),       overlay_text: rgb(192,202,245), rrg_leading: rgb(158,206,106), rrg_improving: rgb(125,207,255), rrg_weakening: rgb(224,175,104), rrg_lagging: rgb(247,118,142), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(5,9,12,12), text_muted: rgb(172,178,220), hud_bg: rgba_pre(18,18,28,230), hud_border: rgb(40,44,62) },
    // ── Additional themes ──
    Theme { name: "Kanagawa",    bg: rgb(22,22,29),   bull: rgb(118,169,130), bear: rgb(195,64,67),   dim: rgb(84,88,104),   toolbar_bg: rgb(18,18,24),  toolbar_border: rgb(34,34,46),  accent: rgb(127,180,202), text: rgb(220,220,230),  warn: rgb(228,175, 69), notification_red: rgb(195, 64, 67), gold: rgb(228,175, 69), shadow_color: rgb(0,0,0),       overlay_text: rgb(220,215,186), rrg_leading: rgb(118,169,130), rrg_improving: rgb(127,180,202), rrg_weakening: rgb(228,175, 69), rrg_lagging: rgb(195,64, 67), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(5,8,9,12), text_muted: rgb(155,158,175), hud_bg: rgba_pre(14,14,20,230), hud_border: rgb(36,36,50) },
    Theme { name: "Everforest",  bg: rgb(39,46,38),   bull: rgb(167,192,128), bear: rgb(230,126,128), dim: rgb(157,169,140), toolbar_bg: rgb(33,40,32),  toolbar_border: rgb(52,60,50),  accent: rgb(131,165,152), text: rgb(220,220,230),  warn: rgb(223,199,118), notification_red: rgb(230,126,128), gold: rgb(223,199,118), shadow_color: rgb(0,0,0),       overlay_text: rgb(211,198,170), rrg_leading: rgb(167,192,128), rrg_improving: rgb(131,165,152), rrg_weakening: rgb(223,199,118), rrg_lagging: rgb(230,126,128), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(6,8,7,13), text_muted: rgb(175,178,162), hud_bg: rgba_pre(28,34,28,230), hud_border: rgb(52,60,50) },
    Theme { name: "Vesper",      bg: rgb(16,16,16),   bull: rgb(166,218,149), bear: rgb(238,130,98),  dim: rgb(120,120,120), toolbar_bg: rgb(11,11,11),  toolbar_border: rgb(36,36,36),  accent: rgb(255,199,119), text: rgb(220,220,230),  warn: rgb(255,199,119), notification_red: rgb(238,130, 98), gold: rgb(255,193, 37), shadow_color: rgb(0,0,0),       overlay_text: rgb(230,230,230), rrg_leading: rgb(166,218,149), rrg_improving: rgb( 74,158,255), rrg_weakening: rgb(255,199,119), rrg_lagging: rgb(238,130, 98), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(3,6,11,11), text_muted: rgb(170,170,180), hud_bg: rgba_pre(10,10,10,230), hud_border: rgb(42,42,42) },
    Theme { name: "Rosé Pine",   bg: rgb(25,23,36),   bull: rgb(156,207,216), bear: rgb(235,111,146), dim: rgb(110,106,134), toolbar_bg: rgb(20,18,30),  toolbar_border: rgb(38,35,53),  accent: rgb(196,167,231), text: rgb(220,220,230),  warn: rgb(246,193,119), notification_red: rgb(235,111,146), gold: rgb(246,193,119), shadow_color: rgb(0,0,0),       overlay_text: rgb(224,222,244), rrg_leading: rgb(156,207,216), rrg_improving: rgb(196,167,231), rrg_weakening: rgb(246,193,119), rrg_lagging: rgb(235,111,146), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(7,9,10,12), text_muted: rgb(167,162,187), hud_bg: rgba_pre(18,16,28,230), hud_border: rgb(44,40,58) },
    // ── Light themes ──
    Theme { name: "Bauhaus",     bg: rgb(242,242,238), bull: rgb(20,120,60),   bear: rgb(200,55,45),   dim: rgb(120,125,130), toolbar_bg: rgb(248,248,245), toolbar_border: rgb(225,225,220), accent: rgb(232,93,38),   text: rgb(22,22,24),   warn: rgb(204,120,  0), notification_red: rgb(200, 55, 45), gold: rgb(204,153,  0), shadow_color: rgb(40,40,40),    overlay_text: rgb( 20, 20, 22), rrg_leading: rgb( 20,120, 60), rrg_improving: rgb( 30,100,180), rrg_weakening: rgb(180,140,  0), rrg_lagging: rgb(200,55, 45), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(1,5,9,14), text_muted: rgb(100,102,110), hud_bg: rgba_pre(20,20,20,220), hud_border: rgb(80,82,88) },
    Theme { name: "Peach",       bg: rgb(243,241,238), bull: rgb(22,130,70),   bear: rgb(195,50,55),   dim: rgb(115,120,125), toolbar_bg: rgb(250,248,246), toolbar_border: rgb(228,225,220), accent: rgb(210,95,70),   text: rgb(20,20,22),   warn: rgb(200,130,  0), notification_red: rgb(195, 50, 55), gold: rgb(200,150,  0), shadow_color: rgb(40,40,40),    overlay_text: rgb( 20, 20, 22), rrg_leading: rgb( 22,130, 70), rrg_improving: rgb( 30,100,180), rrg_weakening: rgb(180,140,  0), rrg_lagging: rgb(195,50, 55), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(1,5,9,14), text_muted: rgb(98,100,108), hud_bg: rgba_pre(20,20,20,220), hud_border: rgb(82,80,78) },
    Theme { name: "Ivory",       bg: rgb(240,242,238), bull: rgb(80,160,50),   bear: rgb(210,60,50),   dim: rgb(118,122,128), toolbar_bg: rgb(248,250,246), toolbar_border: rgb(222,226,218), accent: rgb(160,190,40),  text: rgb(18,20,22),   warn: rgb(190,140,  0), notification_red: rgb(210, 60, 50), gold: rgb(190,150,  0), shadow_color: rgb(40,40,40),    overlay_text: rgb( 18, 20, 22), rrg_leading: rgb( 80,160, 50), rrg_improving: rgb( 30,100,180), rrg_weakening: rgb(180,140,  0), rrg_lagging: rgb(210,60, 50), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(1,5,9,14), text_muted: rgb(100,102,108), hud_bg: rgba_pre(18,20,18,220), hud_border: rgb(80,82,80) },
    Theme { name: "Newsprint",   bg: rgb(238,232,220), bull: rgb(34,94,56),    bear: rgb(168,52,52),   dim: rgb(120,116,104), toolbar_bg: rgb(238,232,220), toolbar_border: rgb(180,170,150), accent: rgb(34,94,56),    text: rgb(28,28,28),   warn: rgb(168,120,  0), notification_red: rgb(168, 52, 52), gold: rgb(168,130,  0), shadow_color: rgb(60,50,40),    overlay_text: rgb( 28, 28, 28), rrg_leading: rgb( 34, 94, 56), rrg_improving: rgb( 30, 90,160), rrg_weakening: rgb(160,120,  0), rrg_lagging: rgb(168,52, 52), cmd_palette: CMD_PALETTE_DEFAULT, pinned_row_tint: rgba_pre(1,4,8,13), text_muted: rgb(105,100,90), hud_bg: rgba_pre(28,24,18,220), hud_border: rgb(90,82,68) },
];
// └─ THEMES_END ────────────────────────────────────────────────────────────────

impl Theme {
    pub(crate) const fn is_light(&self) -> bool {
        // A theme is "light" if the background luminance is above ~50%
        (self.bg.r() as u16 + self.bg.g() as u16 + self.bg.b() as u16) > 400
    }
}

// ─── Live theme store ─────────────────────────────────────────────────────────
use std::sync::{OnceLock, RwLock};

static LIVE_THEMES: OnceLock<RwLock<Vec<Theme>>> = OnceLock::new();

fn live_themes() -> &'static RwLock<Vec<Theme>> {
    LIVE_THEMES.get_or_init(|| RwLock::new(THEMES.to_vec()))
}

pub(crate) fn get_theme(idx: usize) -> Theme {
    live_themes().read().unwrap()[idx].clone()
}

pub(crate) fn set_theme(idx: usize, theme: Theme) {
    live_themes().write().unwrap()[idx] = theme;
}

pub(crate) fn get_all_themes() -> Vec<Theme> {
    live_themes().read().unwrap().clone()
}

const PRESET_COLORS: &[&str] = &["#4a9eff","#e74c3c","#2ecc71","#f39c12","#9b59b6","#1abc9c","#e67e22","#3498db","#e91e63","#00bcd4","#8bc34a","#ff5722","#607d8b","#795548","#cddc39","#ff9800"];

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

// Shared helpers
use super::ui::style::{hex_to_color, dashed_line, draw_line_rgba, section_label, dim_label, color_alpha, separator, status_badge, order_card, action_btn, trade_btn, close_button, dialog_window_themed, dialog_header, dialog_separator_shadow, dialog_section, paint_tooltip_shadow, tooltip_frame, stat_row, segmented_control, paint_chrome_tile_button, ChromeTileState, chrome_tile_fg, FONT_LG, FONT_MD, FONT_SM, STROKE_THIN, STROKE_STD, ALPHA_FAINT, ALPHA_GHOST, ALPHA_SUBTLE, ALPHA_TINT, ALPHA_MUTED, ALPHA_LINE, ALPHA_DIM, ALPHA_STRONG, ALPHA_ACTIVE, ALPHA_HEAVY, TEXT_PRIMARY, COLOR_AMBER};
use super::ui::style as style;
use super::ui::widgets::foundation::text_style::TextStyle;
use super::compute::{compute_sma, compute_ema, compute_rsi, compute_macd, compute_stochastic, compute_vwap, detect_divergences, bs_price, strike_interval, atm_strike, get_iv, sim_oi, compute_atr, compute_bollinger, compute_ichimoku, compute_psar, compute_supertrend, compute_keltner, compute_adx, compute_cci, compute_williams_r};

// compute_sma, compute_ema — now in compute.rs

// ─── Layout ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Layout {
    One, Two, TwoH,
    Three,      // 1 top + 2 bottom
    ThreeL,     // 1 big left + 2 stacked right
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
        Layout::Three|Layout::ThreeL=>3, Layout::Four|Layout::FourL=>4,
        Layout::FiveC|Layout::FiveL|Layout::FiveW|Layout::FiveR=>5,
        Layout::Six|Layout::SixH|Layout::SixL=>6,
        Layout::Seven=>7, Layout::EightH=>8, Layout::Nine=>9,
    }}
    pub(crate) fn label(self) -> &'static str { match self {
        Layout::One=>"1", Layout::Two=>"2", Layout::TwoH=>"2H",
        Layout::Three=>"3", Layout::ThreeL=>"3L",
        Layout::Four=>"4", Layout::FourL=>"4L",
        Layout::FiveC=>"5C", Layout::FiveL=>"5L", Layout::FiveW=>"5W", Layout::FiveR=>"5R",
        Layout::Six=>"6", Layout::SixH=>"6H", Layout::SixL=>"6L",
        Layout::Seven=>"7", Layout::EightH=>"8H", Layout::Nine=>"9",
    }}
    pub(crate) fn description(self) -> &'static str { match self {
        Layout::One=>"Single pane", Layout::Two=>"2 side-by-side", Layout::TwoH=>"2 stacked",
        Layout::Three=>"1 top + 2 bottom", Layout::ThreeL=>"1 left + 2 right",
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
        Layout::Three | Layout::ThreeL => "3 Panes",
        Layout::Four | Layout::FourL => "4 Panes",
        Layout::FiveC | Layout::FiveL | Layout::FiveW | Layout::FiveR => "5 Panes",
        Layout::Six | Layout::SixH | Layout::SixL => "6 Panes",
        Layout::Seven | Layout::EightH | Layout::Nine => "7+ Panes",
    }}
    /// Returns (col, row) grid dimensions for each pane in the layout, given the total rect.
    /// For Layout::Three, returns a custom arrangement: 1 full-width top (60%) + 2 bottom (40%).
    pub(crate) fn pane_rects(self, rect: egui::Rect, count: usize, split_h: f32, split_v: f32, split_h2: f32, split_v2: f32) -> Vec<egui::Rect> {
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
                // 2 stacked left + 3 stacked right (split_v controls left, split_v2 controls right)
                let left_w = (rect.width() - gap) * split_h.clamp(0.2, 0.8);
                let right_w = rect.width() - gap - left_w;
                let rx = rect.left() + left_w + gap;
                let lh0 = (rect.height() - gap) * split_v.clamp(0.15, 0.85);
                let lh1 = rect.height() - gap - lh0;
                let mut rects = Vec::new();
                rects.push(egui::Rect::from_min_size(rect.min, egui::vec2(left_w, lh0)));
                rects.push(egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top() + lh0 + gap), egui::vec2(left_w, lh1)));
                let n_right = (count - 2).min(3);
                let rh = (rect.height() - gap * (n_right as f32 - 1.0).max(0.0)) / n_right as f32;
                for i in 0..n_right {
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + i as f32 * (rh + gap)), egui::vec2(right_w, rh)));
                }
                rects
            }
            Layout::SixL if count >= 2 => {
                // 2 big stacked left + 4 stacked right (split_v controls left)
                let left_w = (rect.width() - gap) * split_h.clamp(0.2, 0.8);
                let right_w = rect.width() - gap - left_w;
                let rx = rect.left() + left_w + gap;
                let lh0 = (rect.height() - gap) * split_v.clamp(0.15, 0.85);
                let lh1 = rect.height() - gap - lh0;
                let mut rects = Vec::new();
                rects.push(egui::Rect::from_min_size(rect.min, egui::vec2(left_w, lh0)));
                rects.push(egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top() + lh0 + gap), egui::vec2(left_w, lh1)));
                let n_right = (count - 2).min(4);
                let rh = (rect.height() - gap * (n_right as f32 - 1.0).max(0.0)) / n_right as f32;
                for i in 0..n_right {
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + i as f32 * (rh + gap)), egui::vec2(right_w, rh)));
                }
                rects
            }
            Layout::EightH if count >= 2 => {
                // 4 horizontal stacked left + 4 horizontal stacked right
                let left_w = (rect.width() - gap) * split_h.clamp(0.2, 0.8);
                let right_w = rect.width() - gap - left_w;
                let rx = rect.left() + left_w + gap;
                let n_left = (count).min(4);
                let n_right = count.saturating_sub(4).min(4);
                let mut rects = Vec::new();
                let lh = (rect.height() - gap * (n_left as f32 - 1.0).max(0.0)) / n_left as f32;
                for i in 0..n_left {
                    rects.push(egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top() + i as f32 * (lh + gap)), egui::vec2(left_w, lh)));
                }
                let rh = (rect.height() - gap * (n_right as f32 - 1.0).max(0.0)) / n_right.max(1) as f32;
                for i in 0..n_right {
                    rects.push(egui::Rect::from_min_size(egui::pos2(rx, rect.top() + i as f32 * (rh + gap)), egui::vec2(right_w, rh)));
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
                    Layout::Three | Layout::ThreeL => (2, 2),
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
    Layout::Three, Layout::ThreeL,
    Layout::Four, Layout::FourL,
    Layout::FiveC, Layout::FiveL, Layout::FiveW, Layout::FiveR,
    Layout::Six, Layout::SixH, Layout::SixL,
    Layout::Seven, Layout::EightH, Layout::Nine,
];

// ─── Indicators ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum IndicatorType { SMA, EMA, WMA, DEMA, TEMA, VWAP, BollingerBands, Ichimoku, ParabolicSAR, Supertrend, KeltnerChannels, RSI, MACD, Stochastic, ADX, CCI, WilliamsR, ATR }

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
            Self::ATR => "ATR",
        }
    }
    pub(crate) fn all() -> &'static [Self] { &[Self::SMA, Self::EMA, Self::WMA, Self::DEMA, Self::TEMA, Self::VWAP, Self::BollingerBands, Self::Ichimoku, Self::ParabolicSAR, Self::Supertrend, Self::KeltnerChannels, Self::RSI, Self::MACD, Self::Stochastic, Self::ADX, Self::CCI, Self::WilliamsR, Self::ATR] }
    #[allow(dead_code)]
    fn overlays() -> &'static [Self] { &[Self::SMA, Self::EMA, Self::WMA, Self::DEMA, Self::TEMA, Self::VWAP, Self::BollingerBands, Self::Ichimoku, Self::ParabolicSAR, Self::Supertrend, Self::KeltnerChannels] }
    #[allow(dead_code)]
    fn oscillators() -> &'static [Self] { &[Self::RSI, Self::MACD, Self::Stochastic, Self::ADX, Self::CCI, Self::WilliamsR, Self::ATR] }
    pub(crate) fn default_period(self) -> usize {
        match self {
            Self::SMA | Self::EMA | Self::WMA | Self::DEMA | Self::TEMA => 20,
            Self::RSI | Self::Stochastic | Self::ADX | Self::CCI | Self::WilliamsR | Self::ATR => 14,
            Self::MACD => 12, Self::VWAP => 1,
            Self::BollingerBands | Self::KeltnerChannels => 20,
            Self::Ichimoku => 9, Self::ParabolicSAR => 1, Self::Supertrend => 10,
        }
    }
    pub(crate) fn category(self) -> IndicatorCategory {
        match self { Self::RSI | Self::MACD | Self::Stochastic | Self::ADX | Self::CCI | Self::WilliamsR | Self::ATR => IndicatorCategory::Oscillator, _ => IndicatorCategory::Overlay }
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
            Self::ADX | Self::CCI | Self::WilliamsR | Self::ATR => vec![f32::NAN; closes.len()],
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

/// Fetch signal annotations from OCOCO API for a symbol.
pub(crate) fn fetch_signal_drawings(symbol: String) {
    let txs: Vec<std::sync::mpsc::Sender<super::ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { return; }
    std::thread::spawn(move || {
        let url = format!("http://192.168.1.60:30300/api/annotations?symbol={}&source=signal", symbol);
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
                    Some(SignalDrawing { id, symbol: sym, drawing_type: dtype, points, color, opacity, thickness, line_style, strength, timeframe })
                }).collect();

                if !drawings.is_empty() {
                    eprintln!("[signal] Fetched {} signal drawings for {}", drawings.len(), symbol);
                }
                // Send via command channel
                let cmd = super::ChartCommand::SignalDrawings { symbol, drawings_json: serde_json::to_string(&json).unwrap_or_default() };
                for tx in &txs { let _ = tx.send(cmd.clone()); }
            }
        }
    });
}

// ─── Orders, Account, Alerts, Triggers ─── (moved to trading.rs)

/// ApexIB endpoint configuration
pub(crate) const APEXIB_URL: &str = "https://apexib-dev.xllio.com";

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
        let oe_qty_snapshot = chart.order_qty;
        let mut oe_state = super::ui::widgets::form::OrderTicketState {
            symbol:         &chart.symbol,
            is_buy:         &mut chart.order_is_buy,
            order_type_idx: &mut chart.order_type_idx,
            order_tif_idx:  &mut chart.order_tif_idx,
            order_qty:      &mut chart.order_qty,
            order_market:   &mut chart.order_market,
            limit_price:    &mut chart.order_limit_price,
            stop_price:     &mut chart.order_stop_price,
            tp_price:       &mut chart.order_tp_price,
            sl_price:       &mut chart.order_sl_price,
            bracket:        &mut chart.order_bracket,
            bid:            (last_price - spread).max(0.0),
            last:           last_price,
            ask:            last_price + spread,
            notional:       last_price * oe_qty_snapshot as f32,
            buying_power:   0.0, // TODO: thread real buying_power from account data
            slippage_bps:   0.0,
        };
        let outcome = super::ui::widgets::form::MeridienOrderTicket::new()
            .theme(t)
            .show(ui, &mut oe_state);
        if outcome.review_clicked {
            // Translate REVIEW click into a submit — same path as the existing BUY/SELL buttons.
            // Side is determined by order_is_buy that the widget just toggled.
            let side = if chart.order_is_buy { "BUY" } else { "SELL" };
            let sym  = chart.symbol.clone();
            let qty  = chart.order_qty;
            let ot   = chart.order_type_idx;
            let tif  = chart.order_tif_idx;
            let price = if chart.order_market { last_price } else {
                chart.order_limit_price.parse::<f32>().unwrap_or(last_price)
            };
            let bracket = chart.order_bracket;
            let tp = chart.order_tp_price.parse::<f32>().ok();
            let sl = chart.order_sl_price.parse::<f32>().ok();
            std::thread::spawn(move || {
                submit_ib_order(&sym, side, qty, ot, tif, price, bracket, tp, sl);
            });
        }
        return;
    }
    // ── Aperture / Octave path: delegated to ApertureOrderTicket widget ──────
    use super::ui::widgets::form::{ApertureOrderTicket, ApertureOrderState, ApertureAction, ApertureVariant};

    let last_price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
    let spread = (last_price * 0.0001).max(0.01);

    let mut oe_state = ApertureOrderState {
        last_price,
        spread,
        order_advanced:        chart.order_advanced,
        order_market:          &mut chart.order_market,
        order_type_idx:        &mut chart.order_type_idx,
        order_tif_idx:         &mut chart.order_tif_idx,
        order_qty:             &mut chart.order_qty,
        order_notional_mode:   &mut chart.order_notional_mode,
        order_notional_amount: &mut chart.order_notional_amount,
        order_limit_price:     &mut chart.order_limit_price,
        order_stop_price:      &mut chart.order_stop_price,
        order_trail_amt:       &mut chart.order_trail_amt,
        order_bracket:         &mut chart.order_bracket,
        order_tp_price:        &mut chart.order_tp_price,
        order_sl_price:        &mut chart.order_sl_price,
        order_outside_rth:     &mut chart.order_outside_rth,
        is_option:             chart.is_option,
        option_type:           &chart.option_type,
        armed:                 chart.armed,
    };

    ui.add_space(4.0);
    let outcome = ApertureOrderTicket::new()
        .variant(ApertureVariant::Aperture)
        .theme(t)
        .panel_width(panel_w)
        .show(ui, &mut oe_state);

    // Handle the action returned by the widget — submission lives here because
    // submit_ib_order / submit_order are in this module.
    let adv = chart.order_advanced;
    match outcome.action {
        ApertureAction::TriggerBuy  => { chart.pending_und_order = Some(OrderSide::TriggerBuy); }
        ApertureAction::TriggerSell => { chart.pending_und_order = Some(OrderSide::TriggerSell); }
        ApertureAction::Buy { price } => {
            if chart.armed && adv {
                let sym = chart.symbol.clone();
                let qty = chart.order_qty;
                let ot_idx = chart.order_type_idx;
                let tif_idx = chart.order_tif_idx;
                let bracket = chart.order_bracket;
                let tp = chart.order_tp_price.parse::<f32>().ok();
                let sl = chart.order_sl_price.parse::<f32>().ok();
                std::thread::spawn(move || {
                    submit_ib_order(&sym, "BUY", qty, ot_idx, tif_idx, price, bracket, tp, sl);
                });
            } else {
                use super::trading::order_manager::*;
                let result = submit_order(OrderIntent {
                    symbol: chart.symbol.clone(), side: OrderSide::Buy,
                    order_type: ManagedOrderType::Limit, price, qty: chart.order_qty,
                    source: OrderSource::OrderPanel, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: chart.order_tif_idx as u8, outside_rth: chart.order_outside_rth,
                });
                match result {
                    OrderResult::Accepted(id) => {
                        chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Buy, price, qty: chart.order_qty, status: OrderStatus::Placed, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
                    }
                    OrderResult::NeedsConfirmation(id) => {
                        chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Buy, price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
                        chart.pending_confirms.push((id as u32, std::time::Instant::now()));
                    }
                    _ => {}
                }
            }
        }
        ApertureAction::Sell { price } => {
            if chart.armed && adv {
                let sym = chart.symbol.clone();
                let qty = chart.order_qty;
                let ot_idx = chart.order_type_idx;
                let tif_idx = chart.order_tif_idx;
                let bracket = chart.order_bracket;
                let tp = chart.order_tp_price.parse::<f32>().ok();
                let sl = chart.order_sl_price.parse::<f32>().ok();
                std::thread::spawn(move || {
                    submit_ib_order(&sym, "SELL", qty, ot_idx, tif_idx, price, bracket, tp, sl);
                });
            } else {
                use super::trading::order_manager::*;
                let result = submit_order(OrderIntent {
                    symbol: chart.symbol.clone(), side: OrderSide::Sell,
                    order_type: ManagedOrderType::Limit, price, qty: chart.order_qty,
                    source: OrderSource::OrderPanel, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: chart.order_tif_idx as u8, outside_rth: chart.order_outside_rth,
                });
                match result {
                    OrderResult::Accepted(id) => {
                        chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Sell, price, qty: chart.order_qty, status: OrderStatus::Placed, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
                    }
                    OrderResult::NeedsConfirmation(id) => {
                        chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Sell, price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
                        chart.pending_confirms.push((id as u32, std::time::Instant::now()));
                    }
                    _ => {}
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

pub(crate) struct Chart {
    pub(crate) pane_type: PaneType,
    pub(crate) symbol: String, pub(crate) timeframe: String,
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
    pub(crate) vs: f32, pub(crate) vc: u32, pub(crate) price_lock: Option<(f32,f32)>,
    pub(crate) log_scale: bool,
    pub(crate) drag_zoom_active: bool,
    pub(crate) drag_zoom_start: Option<egui::Pos2>,
    pub(crate) auto_scroll: bool, pub(crate) last_input: std::time::Instant,
    pub(crate) draw_price_freeze: Option<(f32, f32)>, // locks y-range while drawing so new bars can't rescale
    // Template popup (opened from pane header T button)
    pub(crate) template_popup_open: bool,
    pub(crate) template_popup_pos: egui::Pos2,
    pub(crate) template_save_name: String,
    // Option quick-picker popup (opened by clicking an options tab)
    pub(crate) option_quick_open: bool,
    pub(crate) option_quick_pos: egui::Pos2,
    pub(crate) option_quick_dte_idx: usize,
    pub(crate) history_loading: bool, // true while fetching older bars
    pub(crate) history_exhausted: bool, // true if no more history available
    pub(crate) tick_counter: u64, pub(crate) last_candle_time: std::time::Instant, pub(crate) sim_price: f32, pub(crate) sim_seed: u64,
    pub(crate) theme_idx: usize,
    pub(crate) draw_tool: String, // "", "hline", "trendline", "hzone", "barmarker", "fibonacci", "channel"
    /// Drawing-tool picker (opened by 2nd middle-click while a tool is active).
    pub(crate) draw_picker_open: bool,
    pub(crate) draw_picker_pos: egui::Pos2,
    /// Currently hovered category label in the picker (drives the flyout submenu).
    pub(crate) draw_picker_hover_cat: Option<String>,
    /// Top-Y of the hovered category row, in screen coords — used to align
    /// the flyout to the row that spawned it (not the top of the menu).
    pub(crate) draw_picker_hover_cat_y: f32,
    pub(crate) pending_pt: Option<(f32,f32)>,  // first click (bar, price)
    pub(crate) pending_pt2: Option<(f32,f32)>, // second click for channel (bar, price)
    pub(crate) pending_pts: Vec<(f32,f32)>,    // multi-point: pitchfork(3), xabcd(5), elliott(3/5)
    pub(crate) magnet: bool, // snap to OHLC when placing drawings
    pub(crate) selected_id: Option<String>,
    pub(crate) selected_ids: Vec<String>, // multi-select with shift
    pub(crate) dragging_drawing: Option<(String, i32)>,
    pub(crate) drag_start_price: f32, pub(crate) drag_start_bar: f32,
    pub(crate) groups: Vec<DrawingGroup>,
    pub(crate) hidden_groups: Vec<String>,
    pub(crate) signal_drawings: Vec<SignalDrawing>, // auto-generated trendlines from server
    pub(crate) hide_signal_drawings: bool,
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
    pub(crate) change_points: Vec<(i64, String, f32)>, // (time, type, confidence)
    pub(crate) trade_plan: Option<(i8, f32, f32, f32, String, f32, f32)>, // (dir, entry, target, stop, contract, rr, conviction)
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
    #[allow(dead_code)]
    pub(crate) drawing_list_open: bool, // DEPRECATED: consolidated into object_tree
    pub(crate) ohlc_tooltip: bool, // show OHLC values at crosshair
    pub(crate) measure_tooltip: bool, // show big distance-only measurement at crosshair
    pub(crate) show_volume: bool,
    pub(crate) show_oscillators: bool, // toggle oscillator sub-panel
    pub(crate) draw_color: String, // current drawing color
    pub(crate) zoom_selecting: bool, pub(crate) zoom_start: egui::Pos2,
    pub(crate) axis_drag_mode: u8, // 0=none, 1=xaxis, 2=yaxis
    // Symbol picker
    pub(crate) picker_open: bool, pub(crate) picker_query: String,
    pub(crate) picker_results: Vec<(String, String, String)>, // (symbol, name, exchange/type)
    pub(crate) picker_last_query: String, // debounce: only search when query changes
    pub(crate) picker_searching: bool, // true while background search is in flight
    pub(crate) picker_rx: Option<mpsc::Receiver<Vec<(String, String, String)>>>, // receives search results from bg thread
    pub(crate) picker_pos: egui::Pos2, // anchor position for the popup
    pub(crate) recent_symbols: Vec<(String, String)>, // (symbol, name) — most recent first, max 20
    // Group management
    pub(crate) group_manager_open: bool,
    pub(crate) new_group_name: String,
    // Orders
    pub(crate) orders: Vec<OrderLevel>,
    pub(crate) next_order_id: u32,
    pub(crate) order_qty: u32,
    pub(crate) order_is_buy: bool, // true=buy, false=sell (used by MeridienOrderTicket)
    pub(crate) order_market: bool, // true=market, false=limit
    pub(crate) order_limit_price: String, // limit price as editable text
    pub(crate) order_type_idx: usize, // 0=MKT, 1=LMT, 2=STP, 3=STP-LMT, 4=TRAIL
    pub(crate) order_tif_idx: usize, // 0=DAY, 1=GTC, 2=IOC
    pub(crate) order_outside_rth: bool, // allow trading outside regular trading hours
    pub(crate) order_advanced: bool, // expanded mode
    pub(crate) order_bracket: bool, // bracket mode: entry + TP + SL
    pub(crate) order_stop_price: String, // stop trigger price (for STP, STP-LMT)
    pub(crate) order_trail_amt: String, // trailing amount (for TRAIL)
    pub(crate) order_tp_price: String, // take profit price (bracket)
    pub(crate) order_sl_price: String, // stop loss price (bracket)
    pub(crate) order_panel_pos: egui::Pos2, // draggable position (relative to chart rect)
    pub(crate) order_panel_dragging: bool,
    pub(crate) order_collapsed: bool, // true = show as pill, double-click to expand
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
    // Measure tool (shift+drag)
    pub(crate) measuring: bool,
    pub(crate) measure_start: Option<(f32, f32)>, // (bar, price) start point
    pub(crate) measure_active: bool, // context menu activated measure mode
    pub(crate) dom_open: bool, // DOM / Price Ladder floating window
    // DOM full sidebar mode
    pub(crate) dom_sidebar_open: bool,
    pub(crate) dom_levels: Vec<super::ui::panels::dom_panel::DomLevel>,
    pub(crate) dom_tick_size: f32,
    pub(crate) dom_center_price: f32,
    pub(crate) dom_width: f32,
    pub(crate) dom_selected_price: Option<f32>,
    pub(crate) dom_order_type: super::ui::panels::dom_panel::DomOrderType,
    pub(crate) dom_armed: bool,
    pub(crate) dom_col_mode: u8,
    pub(crate) dom_dragging: Option<(u32, f32)>,
    // Symbol/timeframe change request — signals the App to reload data
    pub(crate) pending_symbol_change: Option<String>,
    pub(crate) pending_timeframe_change: Option<String>,
    // Cached formatted strings — updated only when data changes, not every frame
    #[allow(dead_code)] cached_ohlc: String,
    #[allow(dead_code)] cached_ohlc_bar_count: usize,
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
    pub(crate) vp_mode: VolumeProfileMode,
    pub(crate) candle_mode: CandleMode,
    // Alternative chart types (Renko, Range, Tick)
    pub(crate) renko_brick_size: f32,    // 0.0 = auto (ATR-based)
    pub(crate) range_bar_size: f32,      // 0.0 = auto
    pub(crate) tick_bar_count: u32,      // default 500
    pub(crate) alt_bars: Vec<Bar>,       // recomputed non-time bars
    pub(crate) alt_timestamps: Vec<i64>, // timestamps for alt bars
    pub(crate) alt_bars_dirty: bool,     // true when alt bars need recomputation
    pub(crate) alt_bars_source_len: usize, // source bar count when alt_bars was last computed
    pub(crate) show_footprint: bool, // hover-activated volume footprint on individual bars
    pub(crate) vp_data: Option<VolumeProfileData>,
    pub(crate) vp_last_vs: f32,
    pub(crate) vp_last_vc: u32,
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
    pub(crate) floating_order_panes: Vec<FloatingOrderPane>, // floating order entry windows
    pub(crate) gamma_levels: Vec<(f32, f32)>, // (price, gamma_exposure) — positive = stabilizing, negative = accelerating
    pub(crate) gamma_call_wall: f32,
    pub(crate) gamma_put_wall: f32,
    pub(crate) gamma_zero: f32,
    pub(crate) gamma_hvl: f32,
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
    // Notional-based order entry
    pub(crate) order_notional_mode: bool,
    pub(crate) order_notional_amount: String,
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
}

impl Chart {
    pub(crate) fn new_with(symbol: &str, timeframe: &str) -> Self {
        let mut c = Self::new();
        c.symbol = symbol.into();
        c.timeframe = timeframe.into();
        c
    }
    pub(crate) fn new() -> Self {
        Self { pane_type: PaneType::Chart,
            symbol: "AAPL".into(), timeframe: "5m".into(),
            is_option: false, underlying: String::new(), option_type: String::new(),
            option_strike: 0.0, option_expiry: String::new(), option_con_id: 0, option_contract: String::new(),
            bar_source_mark: false,
            bars: vec![], timestamps: vec![], drawings: vec![], tab_cache: std::collections::HashMap::new(), indicator_bar_count: 0,
            next_indicator_id: 5, editing_indicator: None,
            indicators: vec![
                Indicator::new(1, IndicatorType::SMA, 20, "#00bef0"),
                Indicator::new(2, IndicatorType::SMA, 50, "#f0961a"),
                Indicator::new(3, IndicatorType::EMA, 12, "#f0d732"),
                Indicator::new(4, IndicatorType::EMA, 26, "#b266e6"),
            ],
            vs: 0.0, vc: 200, price_lock: None, log_scale: false, drag_zoom_active: false, drag_zoom_start: None,
            auto_scroll: true, draw_price_freeze: None,
            template_popup_open: false, template_popup_pos: egui::Pos2::ZERO, template_save_name: String::new(),
            option_quick_open: false, option_quick_pos: egui::Pos2::ZERO, option_quick_dte_idx: 0,
            history_loading: false, history_exhausted: false,
            last_input: std::time::Instant::now(), tick_counter: 0,
            last_candle_time: std::time::Instant::now(), sim_price: 0.0,
            sim_seed: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(42),
            theme_idx: 5, // Gruvbox
            draw_tool: String::new(), draw_picker_open: false, draw_picker_pos: egui::Pos2::ZERO, draw_picker_hover_cat: None, draw_picker_hover_cat_y: 0.0, pending_pt: None, pending_pt2: None, pending_pts: vec![], magnet: true,
            selected_id: None, selected_ids: vec![], dragging_drawing: None,
            drag_start_price: 0.0, drag_start_bar: 0.0,
            groups: vec![DrawingGroup { id: "default".into(), name: "Temp".into(), color: None }],
            hidden_groups: vec![], hide_all_drawings: false, hide_all_indicators: false, show_volume: true, show_oscillators: true, drawing_list_open: false, ohlc_tooltip: true, measure_tooltip: false,
            signal_drawings: vec![], hide_signal_drawings: false,
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
            draw_color: "#4a9eff".into(), group_manager_open: false, new_group_name: String::new(),
            zoom_selecting: false, zoom_start: egui::Pos2::ZERO, axis_drag_mode: 0,
            picker_open: false, picker_query: String::new(), picker_results: vec![],
            picker_last_query: String::new(), picker_searching: false, picker_rx: None, picker_pos: egui::Pos2::ZERO,
            recent_symbols: vec![("AAPL".into(), "Apple".into()), ("SPY".into(), "S&P 500 ETF".into()), ("TSLA".into(), "Tesla".into()), ("NVDA".into(), "Nvidia".into()), ("MSFT".into(), "Microsoft".into())],
            orders: vec![], next_order_id: 1, order_qty: 100, order_is_buy: true, order_market: true, order_limit_price: String::new(),
            order_type_idx: 0, order_tif_idx: 0, order_outside_rth: false, order_advanced: false, order_bracket: false,
            order_stop_price: String::new(), order_trail_amt: String::new(),
            order_tp_price: String::new(), order_sl_price: String::new(),
            order_panel_pos: egui::pos2(8.0, -80.0), order_panel_dragging: false, order_collapsed: false,
            dragging_order: None, dragging_alert: None, editing_order: None, edit_order_qty: String::new(), edit_order_price: String::new(),
            armed: false, pending_confirms: vec![],
            trigger_setup: TriggerSetup::default(), trigger_levels: vec![], next_trigger_id: 1, dragging_trigger: None, editing_trigger: None, pending_und_order: None,
            widget_cache_bar_count: 0, widget_cache: None,
            play_lines: vec![], next_play_line_id: 1, dragging_play_line: None, play_click_to_set: None,
            measuring: false, measure_start: None, measure_active: false, dom_open: false,
            dom_sidebar_open: false, dom_levels: vec![], dom_tick_size: 0.01, dom_center_price: 0.0, dom_width: super::ui::panels::dom_panel::DOM_SIDEBAR_W,
            dom_selected_price: None, dom_order_type: super::ui::panels::dom_panel::DomOrderType::Market, dom_armed: false, dom_col_mode: 1, dom_dragging: None,
            pending_symbol_change: None, pending_timeframe_change: None,
            cached_ohlc: String::new(), cached_ohlc_bar_count: 0,
            undo_stack: vec![], redo_stack: vec![], drag_drawing_snapshot: None,
            text_edit_id: None, text_edit_buf: String::new(),
            indicator_pts_buf: Vec::with_capacity(512), fmt_buf: String::with_capacity(256),
            vp_mode: VolumeProfileMode::Off, candle_mode: CandleMode::Standard,
            renko_brick_size: 0.0, range_bar_size: 0.0, tick_bar_count: 500,
            alt_bars: vec![], alt_timestamps: vec![], alt_bars_dirty: true, alt_bars_source_len: 0,
            show_footprint: false, vp_data: None, vp_last_vs: -1.0, vp_last_vc: 0,
            show_vwap_bands: false, show_cvd: false, show_delta_volume: false, show_rvol: true,
            show_ma_ribbon: false, show_prev_close: true, show_auto_sr: false, show_auto_fib: false, swing_leg_mode: 0,
            symbol_overlays: vec![], overlay_editing: false, overlay_editing_idx: None, overlay_input: String::new(),
            show_gamma: false, hit_highlight: false, hit_highlights: vec![], hit_cooldowns: vec![],
            show_events: false, event_markers: vec![],
            show_strikes_overlay: false, overlay_calls: vec![], overlay_puts: vec![], overlay_chain_symbol: String::new(), overlay_chain_loading: false, floating_order_panes: vec![], gamma_levels: vec![], gamma_call_wall: 0.0, gamma_put_wall: 0.0, gamma_zero: 0.0, gamma_hvl: 0.0,
            fundamentals: FundamentalData::default(), show_analyst_targets: false,
            show_pe_band: false, show_insider_trades: false, insider_trades: vec![],
            econ_calendar: vec![],
            show_vol_shelves: false, show_confluence: false,
            show_momentum_heat: false, show_trend_strip: false, show_breadth_tint: false,
            show_vol_cone: false, show_price_memory: false, show_liquidity_voids: false, show_corr_ribbon: false,
            show_darkpool: false, darkpool_prints: vec![],
            vwap_data: vec![], vwap_upper1: vec![], vwap_lower1: vec![], vwap_upper2: vec![], vwap_lower2: vec![],
            cvd_data: vec![], delta_data: vec![], rvol_data: vec![], vol_analytics_computed: 0,
            replay_mode: false, replay_bar_count: 0, replay_playing: false, replay_speed: 1.0, replay_last_step: None,
            order_notional_mode: false, order_notional_amount: String::new(),
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
        }
    }
    fn process(&mut self, cmd: ChartCommand) {
        match cmd {
            ChartCommand::LoadBars { bars, timestamps, symbol, timeframe, .. } => {
                // Skip if this pane is an option chart and the LoadBars is for the underlying
                if self.is_option && symbol != self.symbol { return; }
                let is_new_symbol = self.symbol != symbol;
                self.symbol = symbol; self.timeframe = timeframe;
                self.bars = bars; self.timestamps = timestamps;
                self.vs = (self.bars.len() as f32 - self.vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
                self.sim_price = 0.0;
                self.last_candle_time = std::time::Instant::now();
                self.indicator_bar_count = 0; // force recompute
                self.vol_analytics_computed = 0; // force vol analytics recompute
                self.price_range_animated = None; // reset — no slide animation on symbol/tf change
                // Drawings: fetch asynchronously via single worker thread
                if is_new_symbol {
                    self.drawings_requested = false; self.drawings.clear();
                    self.fundamentals = generate_placeholder_fundamentals(&self.symbol, &self.bars);
                    self.econ_calendar = generate_placeholder_econ();
                    self.insider_trades = generate_placeholder_insiders(&self.symbol);
                }
                if !self.drawings_requested {
                    self.drawings_requested = true;
                    fetch_drawings_background(drawing_persist_key(self));
                }

                // Fetch signal drawings for new symbol
                self.signal_drawings.clear();
                self.last_signal_fetch = std::time::Instant::now();
                fetch_signal_drawings(self.symbol.clone());

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
            ChartCommand::AppendBar { symbol, timeframe, bar, timestamp, mark } => {
                // MARK_BARS_PROTOCOL: drop frames whose source doesn't match the pane's
                // current selection (race window between toggle and server stop).
                // Only meaningful for option panes; stock panes always run in Last mode.
                if self.is_option && mark != self.bar_source_mark { return; }
                // Only append if both symbol AND timeframe match this pane
                if symbol == self.symbol && timeframe == self.timeframe {
                    self.bars.push(bar); self.timestamps.push(timestamp);
                    // Smooth advance: increment vs by 1 instead of snapping, so if auto_scroll
                    // re-engages from a slight offset, the view continues from that position
                    if self.auto_scroll { self.vs += 1.0; }
                }
            }
            ChartCommand::UpdateLastBar { symbol, timeframe, bar, mark } => {
                if self.is_option && mark != self.bar_source_mark { return; }
                // Only update if both symbol AND timeframe match
                if symbol == self.symbol && (timeframe.is_empty() || timeframe == self.timeframe) {
                    if let Some(l) = self.bars.last_mut() {
                        // Properly update candle — don't replace open
                        l.close = bar.close;
                        l.high = l.high.max(bar.close);
                        l.low = l.low.min(bar.close);
                        l.volume += bar.volume;
                        // Keep sim in sync with real ticks
                        self.sim_price = bar.close;
                    }
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
                        self.signal_drawings.clear();
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
                            self.signal_drawings.push(SignalDrawing { id, symbol: symbol.clone(), drawing_type: dtype, points, color, opacity, thickness, line_style: ls, strength, timeframe: tf });
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
            ChartCommand::AlertTriggered { symbol: _, alert_id: _, price, message } => {
                // Push a toast notification regardless of active symbol — alerts are always relevant
                PENDING_TOASTS.with(|ts| ts.borrow_mut().push((message, price, true)));
            }
            ChartCommand::AutoTrendlines { symbol, drawings_json } => {
                // Same parsing as SignalDrawings — replaces signal_drawings for this symbol
                if symbol == self.symbol {
                    if let Ok(annotations) = serde_json::from_str::<Vec<serde_json::Value>>(&drawings_json) {
                        self.signal_drawings.clear();
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
                            self.signal_drawings.push(SignalDrawing { id, symbol: symbol.clone(), drawing_type: dtype, points, color, opacity, thickness, line_style: ls, strength, timeframe: tf });
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
                    PENDING_TOASTS.with(|ts| ts.borrow_mut().push((
                        format!("PRECURSOR: {}", self.precursor_description),
                        lead_minutes,
                        true,
                    )));
                }
            }
            ChartCommand::ChangePointMarker { symbol, time, change_type, confidence } => {
                if symbol == self.symbol {
                    self.change_points.push((time, change_type, confidence));
                    // Keep only last 20
                    if self.change_points.len() > 20 {
                        self.change_points.remove(0);
                    }
                }
            }
            ChartCommand::TradePlanUpdate { symbol, direction, entry_price, target_price, stop_price, contract_name, contract_entry: _, contract_target: _, risk_reward, conviction, summary } => {
                if symbol == self.symbol {
                    self.trade_plan = Some((direction, entry_price, target_price, stop_price, contract_name, risk_reward, conviction));
                    PENDING_TOASTS.with(|ts| ts.borrow_mut().push((summary, conviction, true)));
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
                let brick = if self.renko_brick_size > 0.0 {
                    self.renko_brick_size
                } else {
                    Self::auto_brick_size(&self.bars, 0.5)
                };
                Self::compute_renko_bars(&self.bars, &self.timestamps, brick)
            }
            CandleMode::RangeBar => {
                let range = if self.range_bar_size > 0.0 {
                    self.range_bar_size
                } else {
                    Self::auto_brick_size(&self.bars, 1.0)
                };
                Self::compute_range_bars(&self.bars, &self.timestamps, range)
            }
            CandleMode::TickBar => {
                Self::compute_tick_bars(&self.bars, &self.timestamps, self.tick_bar_count)
            }
            _ => return,
        };
        self.alt_bars = bars;
        self.alt_timestamps = ts;
        self.alt_bars_dirty = false;
        self.alt_bars_source_len = self.bars.len();
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
            let base_source = if ind.source_tf.is_empty() { &chart_closes } else if ind.source_loaded && !ind.source_bars.is_empty() {
                &chart_closes
            } else {
                ind.values = vec![f32::NAN; self.bars.len()];
                ind.values2 = vec![]; ind.values3 = vec![]; ind.values4 = vec![]; ind.values5 = vec![];
                ind.histogram = vec![];
                continue;
            };
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
                    ind.values = compute_vwap(closes, &chart_volumes, &chart_highs, &chart_lows);
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
        let bars_ref = if matches!(self.candle_mode, CandleMode::Renko | CandleMode::RangeBar | CandleMode::TickBar) && !self.alt_bars.is_empty() {
            &self.alt_bars
        } else {
            &self.bars
        };
        let s = self.vs as u32; let e = (s+self.vc).min(bars_ref.len() as u32);
        let (mut lo,mut hi) = (f32::MAX,f32::MIN);
        for i in s..e { if let Some(b) = bars_ref.get(i as usize) { lo=lo.min(b.low); hi=hi.max(b.high); } }
        if lo>=hi { lo-=0.5; hi+=0.5; }
        let p=(hi-lo)*0.05; (lo-p,hi+p)
    }
}

// ─── egui rendering ──────────────────────────────────────────────────────────

/// Run one tick of price simulation for a single pane.
pub(crate) fn new_uuid() -> String { uuid::Uuid::new_v4().to_string() }

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

/// Generate a 32x32 RGBA window icon — Apex triangle in orange on transparent bg.
fn make_window_icon() -> Option<winit::window::Icon> {
    let s: u32 = 32;
    let mut rgba = vec![0u8; (s * s * 4) as usize];
    let color = [254u8, 128, 25, 255]; // Gruvbox accent orange

    // Draw triangle outline: top-center to bottom-left to bottom-right
    let m = 3.0_f32; // margin
    let cx = s as f32 / 2.0;
    let top = (cx, m);
    let bl = (m, s as f32 - m);
    let br = (s as f32 - m, s as f32 - m);

    // Triangle sides
    draw_line_rgba(&mut rgba, s, top.0, top.1, bl.0, bl.1, 1.0, color);
    draw_line_rgba(&mut rgba, s, bl.0, bl.1, br.0, br.1, 1.0, color);
    draw_line_rgba(&mut rgba, s, br.0, br.1, top.0, top.1, 1.0, color);
    // Horizontal bar
    let bar_y = cx + 2.0;
    draw_line_rgba(&mut rgba, s, cx - 7.0, bar_y, cx + 7.0, bar_y, 1.0, color);

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
    let color_bgra = [25u8, 128, 254, 255]; // BGRA for orange #FE8019

    let m = 3.0_f32;
    let cx = s as f32 / 2.0;
    draw_line_rgba(&mut bgra, s as u32, cx, m, m, s as f32 - m, 1.0, color_bgra);
    draw_line_rgba(&mut bgra, s as u32, m, s as f32 - m, s as f32 - m, s as f32 - m, 1.0, color_bgra);
    draw_line_rgba(&mut bgra, s as u32, s as f32 - m, s as f32 - m, cx, m, 1.0, color_bgra);
    draw_line_rgba(&mut bgra, s as u32, cx - 7.0, cx + 2.0, cx + 7.0, cx + 2.0, 1.0, color_bgra);

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
    // Skip simulation for crypto — real data comes from ApexCrypto
    if crate::data::is_crypto(&chart.symbol) { return; }
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
            chart.vs = (chart.bars.len() as f32 - chart.vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
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
                chart.vs = (chart.bars.len() as f32 - chart.vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
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
                PENDING_TOASTS.with(|ts| ts.borrow_mut().push((msg, alert.price, alert.above)));
            }
        }
    }
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

    // Per-bar delta (buy - sell heuristic via close position in range)
    for i in 0..n {
        let b = &chart.bars[i];
        let range = b.high - b.low;
        if range > 0.0 {
            let buy_ratio = (b.close - b.low) / range;
            chart.delta_data[i] = b.volume * buy_ratio - b.volume * (1.0 - buy_ratio);
        } else {
            chart.delta_data[i] = 0.0;
        }
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
                    if !p.is_option {
                        crate::apex_log!("route.load", "fallback to active_pane (stock)");
                        p.process(cmd);
                    } else {
                        crate::apex_log!("route.load", "DROPPED — no matching pane (active is option)");
                    }
                }
            }
            // Watchlist-targeted commands: handle directly
            ChartCommand::WatchlistPrice { symbol, price, prev_close } => {
                watchlist.set_price(symbol, *price);
                watchlist.set_prev_close(symbol, *prev_close);
            }
            ChartCommand::ScannerPrice { symbol, price, prev_close, volume } => {
                // Update or insert into scanner results pool
                if let Some(r) = watchlist.scanner_results.iter_mut().find(|r| r.symbol == *symbol) {
                    r.price = *price;
                    r.volume = *volume;
                    r.change_pct = if *prev_close > 0.0 { (price - prev_close) / prev_close * 100.0 } else { 0.0 };
                } else {
                    let change_pct = if *prev_close > 0.0 { (price - prev_close) / prev_close * 100.0 } else { 0.0 };
                    watchlist.scanner_results.push(ScanResult {
                        symbol: symbol.clone(), price: *price, change_pct, volume: *volume,
                    });
                }
            }
            ChartCommand::TapeEntry { symbol, price, qty, time, is_buy } => {
                watchlist.tape_entries.push(TapeRow {
                    symbol: symbol.clone(), price: *price, qty: *qty, time: *time, is_buy: *is_buy,
                });
                // Cap at 500 entries
                if watchlist.tape_entries.len() > 500 {
                    watchlist.tape_entries.drain(..watchlist.tape_entries.len() - 500);
                }
            }
            ChartCommand::ChainData { symbol, dte, underlying_price, calls, puts } => {
                if *symbol == watchlist.chain_symbol {
                    let to_rows = |data: &[(f32,f32,f32,f32,i32,i32,f32,bool,String)]| -> Vec<OptionRow> {
                        data.iter().map(|(strike,last,bid,ask,vol,oi,iv,itm,contract)| OptionRow {
                            strike: *strike, last: *last, bid: *bid, ask: *ask,
                            volume: *vol, oi: *oi, iv: *iv, itm: *itm, contract: contract.clone(),
                        }).collect()
                    };
                    if *dte == 0 {
                        watchlist.chain_0dte = (to_rows(calls), to_rows(puts));
                    } else {
                        watchlist.chain_far = (to_rows(calls), to_rows(puts));
                    }
                    watchlist.chain_loading = false;
                    if *underlying_price > 0.0 { watchlist.chain_underlying_price = *underlying_price; }
                    eprintln!("[chain] Loaded {} calls + {} puts for {} dte={} price={:.2}",
                        if *dte == 0 { watchlist.chain_0dte.0.len() } else { watchlist.chain_far.0.len() },
                        if *dte == 0 { watchlist.chain_0dte.1.len() } else { watchlist.chain_far.1.len() },
                        symbol, dte, underlying_price);
                }
            }
            ChartCommand::OverlayChainData { symbol, calls, puts } => {
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
                        eprintln!("[overlay-chain] Loaded {} calls + {} puts for {}", chart.overlay_calls.len(), chart.overlay_puts.len(), symbol);
                    }
                }
            }
            ChartCommand::SearchResults { query, results, source } => {
                if source == "watchlist" && !query.is_empty()
                    && watchlist.search_query.to_lowercase().starts_with(&query.to_lowercase()) {
                    for (sym, name) in results {
                        if !watchlist.search_results.iter().any(|(s, _)| s == sym) {
                            watchlist.search_results.push((sym.clone(), name.clone()));
                        }
                    }
                } else if source == "chain" && !query.is_empty()
                    && watchlist.chain_sym_input.to_lowercase().starts_with(&query.to_lowercase()) {
                    for (sym, name) in results {
                        if !watchlist.search_results.iter().any(|(s, _)| s == sym) {
                            watchlist.search_results.push((sym.clone(), name.clone()));
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
            if chart.alt_bars_dirty || chart.alt_bars_source_len != chart.bars.len() {
                chart.recompute_alt_bars();
            }
        }
        chart.update_indicators();
        tick_simulation(chart);
    }
    span_end();
}

/// Phase 4: Apply theme, font scale, cache account data, get window ref.
pub(crate) fn setup_theme(ctx: &egui::Context, panes: &[Chart], active_pane: usize, watchlist: &Watchlist) -> (usize, Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>, Option<Arc<Window>>) {
    let theme_idx = panes[active_pane].theme_idx;
    let _t_owned = get_theme(theme_idx);
    let t = &_t_owned;
    {
        let mut style = (*ctx.style()).clone();
        style.visuals.panel_fill = t.toolbar_bg;
        style.visuals.extreme_bg_color = t.bg;
        // ── Rich visual system — editorial design language ──
        let is_light = t.is_light();
        style.visuals.dark_mode = !is_light;
        style.visuals.override_text_color = Some(t.text);
        style.interaction.tooltip_delay = 0.12;

        // Popup/dropdown shadows — rich depth with soft falloff
        style.visuals.popup_shadow = egui::epaint::Shadow {
            offset: [0, if is_light { 6 } else { 4 }],
            blur: if is_light { 24 } else { 18 },
            spread: if is_light { 2 } else { 1 },
            color: egui::Color32::from_black_alpha(if is_light { 35 } else { 90 }),
        };
        // Window shadows (dialogs, popups)
        style.visuals.window_shadow = egui::epaint::Shadow {
            offset: [0, 8],
            blur: 28,
            spread: 2,
            color: egui::Color32::from_black_alpha(if is_light { 40 } else { 100 }),
        };

        // Corner radii — reduced for dropdowns, moderate for buttons
        let r = egui::CornerRadius::same(4);
        let popup_r = egui::CornerRadius::same(6); // halved from 12

        // ── Widget styling ──

        // Inactive — subtle fill, visible border
        style.visuals.widgets.inactive.bg_fill       = color_alpha(t.toolbar_border, if is_light { 12 } else { 18 });
        style.visuals.widgets.inactive.weak_bg_fill  = egui::Color32::TRANSPARENT;
        style.visuals.widgets.inactive.bg_stroke     = egui::Stroke::new(0.8, color_alpha(t.toolbar_border, if is_light { 50 } else { 35 }));
        style.visuals.widgets.inactive.corner_radius = r;
        style.visuals.widgets.inactive.fg_stroke     = egui::Stroke::new(1.0, t.dim);

        // Hovered — clear feedback, beveled feel
        style.visuals.widgets.hovered.bg_fill        = color_alpha(t.toolbar_border, if is_light { 35 } else { 45 });
        style.visuals.widgets.hovered.bg_stroke      = egui::Stroke::new(1.0, color_alpha(t.accent, if is_light { 90 } else { 70 }));
        style.visuals.widgets.hovered.corner_radius  = r;
        style.visuals.widgets.hovered.fg_stroke      = egui::Stroke::new(1.0, t.text);

        // Active/pressed
        style.visuals.widgets.active.bg_fill         = color_alpha(t.accent, if is_light { 30 } else { 40 });
        style.visuals.widgets.active.bg_stroke       = egui::Stroke::new(1.0, color_alpha(t.accent, ALPHA_STRONG));
        style.visuals.widgets.active.corner_radius   = r;
        style.visuals.widgets.active.fg_stroke       = egui::Stroke::new(1.0, t.accent);

        // Open (menu/combo open state)
        style.visuals.widgets.open.bg_fill           = color_alpha(t.accent, if is_light { 25 } else { 35 });
        style.visuals.widgets.open.bg_stroke         = egui::Stroke::new(1.0, color_alpha(t.accent, ALPHA_ACTIVE));
        style.visuals.widgets.open.corner_radius     = r;
        style.visuals.widgets.open.fg_stroke         = egui::Stroke::new(1.0, t.accent);

        // Selection
        style.visuals.selection.bg_fill              = color_alpha(t.accent, if is_light { 25 } else { 35 });
        style.visuals.selection.stroke               = egui::Stroke::new(1.0, t.accent);

        // Popup/menu window — more visible border, reduced rounding
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

        ctx.set_style(style);
    }
    // Apply per-style egui visuals overrides (Meridien denser spacing, flat borders, no shadows).
    // Must run AFTER the rich visual block above so Meridien tweaks override where needed (#3).
    {
        let st = super::ui::style::current();
        super::ui::style::apply_ui_style(ctx, &st, t.toolbar_border, t.toolbar_bg);
    }
    // native_dpi_scale is the floor (never render below display resolution).
    // font_scale is the user zoom on top; on a 1x display it wins if > 1.0,
    // on Retina (2x) the display floor takes over unless the user zooms past it.
    ctx.set_pixels_per_point(watchlist.font_scale.max(watchlist.native_dpi_scale));
    let account_data_cached = read_account_data();
    // Reconcile OrderManager with IB backend state
    if let Some((_, _, ref ib_orders)) = account_data_cached {
        super::trading::order_manager::reconcile_with_ib(ib_orders);
    }
    // Drain order manager toasts (fills, rejections, cancellations) into PENDING_TOASTS
    {
        let order_toasts = super::trading::order_manager::drain_order_toasts();
        if !order_toasts.is_empty() {
            PENDING_TOASTS.with(|ts| {
                let mut v = ts.borrow_mut();
                for msg in order_toasts {
                    let is_fill = msg.starts_with("FILLED");
                    v.push((msg, 0.0, is_fill));
                }
            });
        }
    }
    // Drain ApexData toasts (sub_rejected, feed errors) into PENDING_TOASTS
    {
        let apex_toasts = crate::apex_data::live_state::drain_toasts();
        if !apex_toasts.is_empty() {
            PENDING_TOASTS.with(|ts| {
                let mut v = ts.borrow_mut();
                for msg in apex_toasts {
                    v.push((msg, 0.0, false)); // bearish/warn color
                }
            });
        }
    }
    // Detect paper mode from APEXIB URL (dev/paper endpoints indicate paper trading)
    if let Some((ref _summary, _, _)) = account_data_cached {
        // Detect paper mode once on first frame (don't override user toggle)
        static PAPER_DETECTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !PAPER_DETECTED.load(std::sync::atomic::Ordering::Relaxed) {
            PAPER_DETECTED.store(true, std::sync::atomic::Ordering::Relaxed);
            super::trading::order_manager::set_paper_mode(APEXIB_URL.contains("dev") || APEXIB_URL.contains("paper"));
        }
    }
    super::trading::order_manager::gc_orders(); // periodic cleanup
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
    let accent = if active { t.accent } else { t.dim.gamma_multiply(0.5) };
    let bull = if active { t.bull } else { t.dim.gamma_multiply(0.4) };
    let bear = if active { t.bear } else { t.dim.gamma_multiply(0.3) };

    match kind {
        // Donut gauges — small ring
        W::TrendStrength | W::Momentum | W::ConvictionMeter | W::LiquidityScore
        | W::OptionsSentiment | W::Volatility => {
            let r_sz = 9.0;
            for i in 0..16 {
                let a = (i as f32 / 16.0) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                let a2 = ((i + 1) as f32 / 16.0) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                let col = if i < 11 { accent } else { color_alpha(t.toolbar_border, ALPHA_MUTED) };
                p.line_segment([
                    egui::pos2(cx + r_sz * a.cos(), cy + r_sz * a.sin()),
                    egui::pos2(cx + r_sz * a2.cos(), cy + r_sz * a2.sin())],
                    egui::Stroke::new(3.0, col));
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
                    let col = if (j as f32 / 12.0) < frac { accent } else { color_alpha(t.toolbar_border, ALPHA_MUTED) };
                    p.line_segment([
                        egui::pos2(cx + r_sz * a.cos(), cy + r_sz * a.sin()),
                        egui::pos2(cx + r_sz * a2.cos(), cy + r_sz * a2.sin())],
                        egui::Stroke::new(2.0, col));
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
                    p.circle_filled(egui::pos2(dx, dy), 1.8, if on { bull } else { color_alpha(t.toolbar_border, ALPHA_MUTED) });
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
            p.circle_stroke(egui::pos2(cx, cy), 10.0, egui::Stroke::new(1.0, accent));
            p.line_segment([egui::pos2(cx, cy), egui::pos2(cx + 4.0, cy - 8.0)], egui::Stroke::new(1.5, bull));
            p.circle_filled(egui::pos2(cx, cy), 2.0, accent);
        }
        // 2x2 quadrant
        W::SectorRotation | W::EarningsMom => {
            p.line_segment([egui::pos2(cx, r.top() + 3.0), egui::pos2(cx, r.bottom() - 3.0)],
                egui::Stroke::new(0.5, color_alpha(t.dim, ALPHA_MUTED)));
            p.line_segment([egui::pos2(r.left() + 3.0, cy), egui::pos2(r.right() - 3.0, cy)],
                egui::Stroke::new(0.5, color_alpha(t.dim, ALPHA_MUTED)));
            for (dx, dy, col) in [(5.0, -5.0, bull), (-4.0, 3.0, bear), (3.0, 4.0, accent)] {
                p.circle_filled(egui::pos2(cx + dx, cy + dy), 2.5, col);
            }
        }
        // Radar dots
        W::SignalRadar => {
            p.circle_stroke(egui::pos2(cx, cy), 10.0, egui::Stroke::new(0.5, color_alpha(t.dim, ALPHA_MUTED)));
            for i in 0..8 {
                let a = (i as f32 / 8.0) * std::f32::consts::TAU;
                let on = i % 3 != 0;
                let rr = if on { 10.0 } else { 6.0 };
                p.circle_filled(egui::pos2(cx + rr * a.cos(), cy + rr * a.sin()), 1.5,
                    if on { accent } else { color_alpha(t.dim, ALPHA_MUTED) });
            }
        }
        // Grid cells
        W::CrossAssetPulse => {
            for row in 0..2 {
                for col in 0..4 {
                    let x = r.left() + 2.0 + col as f32 * 6.5;
                    let y = r.top() + 4.0 + row as f32 * 12.0;
                    let col_c = [bull, bear, bull, accent][col];
                    p.rect_filled(egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(5.5, 10.0)), 1.0, color_alpha(col_c, ALPHA_DIM));
                }
            }
        }
        // Speedometer
        W::TapeSpeed | W::SessionTimer => {
            let segs = 10;
            for i in 0..segs {
                let a = std::f32::consts::PI + (i as f32 / segs as f32) * std::f32::consts::PI;
                let a2 = std::f32::consts::PI + ((i + 1) as f32 / segs as f32) * std::f32::consts::PI;
                let col = if i < 6 { accent } else { color_alpha(t.toolbar_border, ALPHA_MUTED) };
                p.line_segment([
                    egui::pos2(cx + 10.0 * a.cos(), cy + 4.0 + 10.0 * a.sin()),
                    egui::pos2(cx + 10.0 * a2.cos(), cy + 4.0 + 10.0 * a2.sin())],
                    egui::Stroke::new(2.5, col));
            }
        }
        // Hero number fallback
        _ => {
            p.text(egui::pos2(cx, cy), egui::Align2::CENTER_CENTER, kind.icon(),
                egui::FontId::proportional(14.0), accent);
        }
    }
}


// ─── Render functions (moved to render/pane.rs) ──────────────────────────────
pub(crate) use super::render::pane::{render_toolbar, draw_chart};
use super::render::pane::*;


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
pub(crate) enum WatchlistTab { Stocks, Chain, Heat }

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

pub(crate) struct Watchlist {
    pub(crate) open: bool,
    pub(crate) tab: WatchlistTab,
    pub(crate) sections: Vec<WatchlistSection>,
    pub(crate) next_section_id: u32,
    // Multi-watchlist
    pub(crate) saved_watchlists: Vec<SavedWatchlist>,
    pub(crate) active_watchlist_idx: usize,
    pub(crate) watchlist_name_editing: bool,
    pub(crate) watchlist_name_buf: String,
    #[allow(dead_code)]
    pub(crate) watchlist_ctx_menu_idx: Option<usize>, // which watchlist index has context menu open
    pub(crate) search_query: String,
    pub(crate) search_results: Vec<(String, String)>,
    pub(crate) search_sel: i32, // -1 = none, 0+ = highlighted suggestion index
    pub(crate) search_refocus: bool, // request refocus on search bar after adding
    pub(crate) options_visible: bool, // toggle options section below stocks
    pub(crate) options_split: f32, // fraction of height for stocks (0.3..0.9), rest for options
    pub(crate) divider_dragging: bool, // true while dragging the stocks/options divider
    pub(crate) divider_y: f32, // screen Y of divider (set during render)
    pub(crate) divider_total_h: f32, // total available height for split calculation
    // Drag-and-drop state
    pub(crate) dragging: Option<(usize, usize)>,       // (section_idx, item_idx) being dragged
    pub(crate) drag_start_pos: Option<egui::Pos2>, // mouse position when drag started
    pub(crate) drop_target: Option<(usize, usize)>,     // (section_idx, insert_before_item_idx)
    pub(crate) drag_confirmed: bool, // true once mouse moved enough to confirm drag
    // Section editing
    pub(crate) renaming_section: Option<u32>, // section id being renamed
    pub(crate) rename_buf: String,
    #[allow(dead_code)]
    pub(crate) color_picking_section: Option<u32>, // section id picking color
    // Toolbar
    #[allow(dead_code)] toolbar_scroll: f32,
    #[allow(dead_code)] shortcuts_open: bool, // superseded by hotkey_editor_open
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
    pub(crate) trendline_filter_open: bool, // trendline filter dropdown
    pub(crate) account_strip_open: bool, // account summary bar below toolbar
    pub(crate) object_tree_open: bool, // object tree panel (drawings, indicators, overlays)
    pub(crate) broadcast_mode: bool, // when true, toolbar actions apply to all panes
    /// Drawing-tool favorites shown in the middle-click picker. Persisted.
    pub(crate) draw_favorites: Vec<String>,
    /// UI style preset index (0..STYLE_NAMES.len()). Combines with `theme_idx`
    /// to form the full visual identity (e.g. "GruvBox/Meridien").
    pub(crate) style_idx: usize,
    pub(crate) pending_opt_chart: Option<(String, f32, bool, String)>, // deferred option chart open
    /// Optional OCC contract ticker for the pending open. When present, used as the
    /// fetch key so real bars come from ApexData; pane.symbol stays the display label.
    pub(crate) pending_opt_chart_contract: Option<String>,
    pub(crate) apex_diag_open: bool,
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
    pub(crate) custom_filters: Vec<(String, f32, f32)>, // (name, min_change%, max_change%)
    pub(crate) filter_min_change: f32,
    pub(crate) filter_max_change: f32,
    #[allow(dead_code)] filter_min_rvol: f32,  // reserved for RVOL filter when data is available
    // Heatmap
    pub(crate) heat_index: String,
    pub(crate) heat_collapsed: std::collections::HashSet<String>,
    pub(crate) heat_cols: u8, // 1, 2, or 3 columns
    pub(crate) heat_sort: i8, // 0=default, 1=gainers first, -1=losers first
    // Orders
    pub(crate) orders_panel_open: bool,
    pub(crate) order_entry_open: bool,
    pub(crate) selected_order_ids: Vec<(usize, u32)>, // (pane_idx, order_id) for multi-select
    // Positions
    pub(crate) positions: Vec<Position>,
    // Alerts
    pub(crate) alerts: Vec<Alert>,
    pub(crate) next_alert_id: u32,
    #[allow(dead_code)]
    pub(crate) alert_query: String,
    pub(crate) alerts_panel_open: bool,
    // Options chain
    pub(crate) chain_symbol: String,
    pub(crate) chain_sym_input: String,
    pub(crate) chain_num_strikes: usize, // legacy fallback
    pub(crate) chain_far_dte: i32,
    pub(crate) chain_0dte: (Vec<OptionRow>, Vec<OptionRow>), // (calls, puts) for 0DTE
    pub(crate) chain_far: (Vec<OptionRow>, Vec<OptionRow>), // (calls, puts) for far DTE
    pub(crate) chain_select_mode: bool,
    pub(crate) chain_loading: bool, // true while fetching chain from ApexIB
    pub(crate) chain_underlying_price: f32, // real-time underlying price from IB chain response
    pub(crate) chain_frozen: bool, // legacy fallback
    pub(crate) chain_center_offset: i32, // legacy fallback
    // Per-chain independent controls
    pub(crate) chain_0_num_strikes: usize,
    pub(crate) chain_0_frozen: bool,
    pub(crate) chain_0_offset: i32,
    pub(crate) chain_0_strike_mode: StrikeMode,
    pub(crate) chain_0_nmf: u8, // 0=near, 1=mid, 2=far
    pub(crate) chain_far_num_strikes: usize,
    pub(crate) chain_far_frozen: bool,
    pub(crate) chain_far_offset: i32,
    pub(crate) chain_far_strike_mode: StrikeMode,
    pub(crate) chain_far_nmf: u8,
    pub(crate) chain_last_fetch: Option<std::time::Instant>, // debounce chain refetches
    // Saved options
    pub(crate) saved_options: Vec<SavedOption>,
    pub(crate) dte_filter: i32,
    // Workspaces
    pub(crate) active_workspace: String,
    pub(crate) workspace_save_name: String,
    pub(crate) pending_workspace_load: Option<String>,
    // Pane split ratios (for resizable panes)
    pub(crate) pane_split_h: f32, // primary vertical divider ratio
    pub(crate) pane_split_v: f32, // primary horizontal divider ratio
    pub(crate) pane_split_h2: f32, // secondary vertical divider ratio (for 3-column layouts)
    pub(crate) pane_split_v2: f32, // secondary horizontal divider ratio (for 3-row layouts)
    pub(crate) pane_divider_dragging: bool,
    // Command palette
    pub(crate) cmd_palette_open: bool,
    pub(crate) cmd_palette_query: String,
    pub(crate) cmd_palette_results: Vec<(String, String, String)>, // (id, name, category)
    pub(crate) cmd_palette_sel: i32, // selected result index (-1 = none)
    pub(crate) cmd_palette_recent: Vec<String>, // recent symbol/command ids (most-recent first)
    pub(crate) cmd_palette_freq: std::collections::HashMap<String, u32>, // frequency counter
    pub(crate) cmd_palette_ai_mode: bool, // AI chat overlay (Gemma 4 placeholder)
    pub(crate) cmd_palette_ai_input: String,
    // Layout favorites (shown as buttons in toolbar; rest in dropdown)
    pub(crate) layout_favorites: Vec<String>,
    pub(crate) layout_dropdown_open: bool,
    pub(crate) pending_overlay_add: bool,
    pub(crate) layout_dropdown_pos: egui::Pos2,
    // Timeframe favorites (shown as segmented control; full list in dropdown)
    pub(crate) timeframe_favorites: Vec<String>,
    pub(crate) timeframe_dropdown_open: bool,
    pub(crate) timeframe_dropdown_pos: egui::Pos2,
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
    pub(crate) play_editor_open: bool,
    pub(crate) play_editor_symbol: String,
    pub(crate) play_editor_entry: String,
    pub(crate) play_editor_target: String,
    pub(crate) play_editor_stop: String,
    pub(crate) play_editor_notes: String,
    pub(crate) play_editor_direction: super::PlayDirection,
    pub(crate) play_editor_type: super::PlayType,
    pub(crate) play_editor_qty: u32,
    pub(crate) play_editor_qty_str: String,
    pub(crate) play_editor_tags: Vec<String>,
    pub(crate) play_editor_t2: String,
    pub(crate) play_editor_t2_pct: String,
    pub(crate) play_editor_t3: String,
    pub(crate) play_editor_t3_pct: String,
    pub(crate) play_editor_has_t2: bool,
    pub(crate) play_editor_has_t3: bool,
    pub(crate) play_editor_custom_tag: String,
    pub(crate) play_editor_target_pct: String,  // T1 allocation %
    pub(crate) play_templates: Vec<super::PlayTemplate>,
    pub(crate) widget_presets: Vec<super::WidgetPreset>,
    pub(crate) widget_preset_name: String, // input buffer for naming a new preset
    pub(crate) pane_template_name: String, // input buffer for naming a new template
    // Discord chat panel
    pub(crate) discord_open: bool,
    pub(crate) discord_messages: Vec<DiscordMessage>,
    pub(crate) discord_input: String,
    pub(crate) discord_channel: String, // currently selected channel display name
    pub(crate) discord_authenticated: bool,
    pub(crate) discord_username: String,
    pub(crate) discord_user_id: String,
    pub(crate) discord_guilds: Vec<crate::discord::DiscordGuild>,
    pub(crate) discord_selected_guild: Option<String>,
    pub(crate) discord_channels: Vec<crate::discord::DiscordChannel>,
    pub(crate) discord_selected_channel: Option<String>,
    pub(crate) discord_connecting: bool,
    pub(crate) discord_guild_icons: std::collections::HashMap<String, egui::TextureHandle>,
    pub(crate) discord_last_msg_id: Option<String>,
    pub(crate) discord_poll_timer: Option<std::time::Instant>,
    pub(crate) discord_channels_loading: bool,
    pub(crate) discord_messages_loading: bool,
    // Time & Sales
    pub(crate) tape_open: bool,
    pub(crate) tape_entries: Vec<TapeRow>,
    // News feed panel
    pub(crate) news_open: bool,
    // Trade Journal panel
    pub(crate) journal_open: bool,
    pub(crate) news_items: Vec<NewsItem>,
    pub(crate) news_filter_symbol: bool, // true = filter to active chart symbol
    // Scanner
    pub(crate) scanner_open: bool,
    pub(crate) scanner_defs: Vec<ScannerDef>,
    pub(crate) scanner_results: Vec<ScanResult>, // raw bulk quote pool
    pub(crate) scanner_last_fetch: Option<std::time::Instant>,
    pub(crate) scanner_fetching: bool,
    // Custom scanner builder
    // Spread Builder panel
    pub(crate) spread_open: bool,
    pub(crate) maximized_pane: Option<usize>, // Some(idx) = pane shown fullscreen
    pub(crate) spread_state: super::ui::panels::spread_panel::SpreadState,
    // Scripting / Backtesting panel
    pub(crate) script_open: bool,
    pub(crate) script_source: String,
    pub(crate) script_output: String,
    pub(crate) script_ai_prompt: String,
    pub(crate) script_result_tab: super::ui::panels::script_panel::ScriptResultTab,
    pub(crate) script_backtest: Option<super::ui::panels::script_panel::BacktestResult>,
    pub(crate) scanner_new_name: String,
    pub(crate) scanner_new_min_change: f32,
    pub(crate) scanner_new_max_change: f32,
    pub(crate) scanner_new_min_volume: String, // string for text edit
    pub(crate) scanner_builder_open: bool,
    // Screenshot library
    pub(crate) screenshot_open: bool,
    pub(crate) screenshot_entries: Vec<super::ui::panels::screenshot_panel::ScreenshotEntry>,
    // RRG (Relative Rotation Graph)
    pub(crate) rrg_open: bool,
    pub(crate) rrg_sectors: Vec<super::ui::panels::rrg_panel::RRGSector>,
    pub(crate) rrg_cycle_phase: String,
    pub(crate) rrg_time_offset: f32, // 0.0 = current, 1.0 = oldest history point
    pub(crate) rrg_tail_length: usize, // how many tail points to show
    // Analysis sidebar — subdivided sections (each has its own tab)
    pub(crate) analysis_open: bool,
    pub(crate) analysis_tab: crate::chart_renderer::AnalysisTab, // default tab for new sections
    pub(crate) analysis_splits: Vec<SplitSection<crate::chart_renderer::AnalysisTab>>,
    // Signals sidebar — subdivided sections
    pub(crate) signals_panel_open: bool,
    pub(crate) signals_tab: crate::chart_renderer::SignalsTab,
    pub(crate) signals_splits: Vec<SplitSection<crate::chart_renderer::SignalsTab>>,
    // Feed sidebar — subdivided sections
    pub(crate) feed_panel_open: bool,
    pub(crate) feed_tab: crate::chart_renderer::FeedTab,
    pub(crate) feed_splits: Vec<SplitSection<crate::chart_renderer::FeedTab>>,
    // Playbook sidebar
    pub(crate) playbook_panel_open: bool,
    // Trade Journal
    pub(crate) journal_panel_open: bool,
    pub(crate) journal_entries: Vec<JournalEntry>,
    pub(crate) journal_page: usize,
    // Book pane tab (Positions/Orders + Journal)
    pub(crate) book_tab: crate::chart_renderer::BookTab,
}

const DEFAULT_WATCHLIST: &[&str] = &["SPY","QQQ","IWM","DIA","AAPL","MSFT","NVDA","TSLA","AMZN","META","GOOGL","GLD"];
const DEFAULT_CRYPTO: &[&str] = &["BTCUSDT","ETHUSDT","SOLUSDT","XRPUSDT","BNBUSDT","DOGEUSDT","ADAUSDT","AVAXUSDT","LINKUSDT","DOTUSDT","SUIUSDT","NEARUSDT","ARBUSDT","OPUSDT","APTUSDT","AAVEUSDT","UNIUSDT","ATOMUSDT","LTCUSDT","MATICUSDT"];

impl Watchlist {
    fn new() -> Self {
        let (saved_watchlists, active_idx) = load_watchlists();
        let active = &saved_watchlists[active_idx];
        let sections = active.sections.clone();
        let next_section_id = active.next_section_id;
        Self { open: false, tab: WatchlistTab::Stocks, sections, next_section_id,
               saved_watchlists, active_watchlist_idx: active_idx,
               watchlist_name_editing: false, watchlist_name_buf: String::new(), watchlist_ctx_menu_idx: None,
               search_query: String::new(), search_results: vec![], search_sel: -1, search_refocus: false,
               options_visible: true, options_split: 0.6, divider_dragging: false, divider_y: 0.0, divider_total_h: 0.0,
               dragging: None, drag_start_pos: None, drop_target: None, drag_confirmed: false,
               renaming_section: None, rename_buf: String::new(), color_picking_section: None,
               toolbar_scroll: 0.0, shortcuts_open: false,
               hotkey_editor_open: false, hotkey_editing_id: None, hotkeys: default_hotkeys(),
               settings_open: false, font_scale: 1.6, native_dpi_scale: 1.0, font_idx: 0,
               default_stock_qty: 100, default_options_qty: 1, default_order_type: 0, default_tif: 0, default_outside_rth: false,
               compact_mode: false,
               pane_header_size: crate::chart_renderer::PaneHeaderSize::Compact,
               show_x_axis: true, show_y_axis: true,
               toolbar_auto_hide: false, toolbar_hover_time: None, shared_x_axis: false, shared_y_axis: false,
               trendline_filter_open: false, account_strip_open: false, object_tree_open: false, broadcast_mode: false,
               draw_favorites: vec!["trendline".into(), "magnifier".into(), "measure".into(), "hline".into(), "channel".into(), "fibonacci".into()],
               style_idx: 0,
               pending_opt_chart: None, pending_opt_chart_contract: None, apex_diag_open: false, widget_gallery_open: false,
               wl_columns: crate::chart::renderer::ui::lists::rows::watchlist_columns::default_columns(),
               wl_columns_open: false,
               filter_open: false, filter_text: String::new(), filter_preset: "All".into(), filter_min_change: -999.0, filter_max_change: 999.0, filter_min_rvol: -1.0, custom_filters: vec![],
               orders_panel_open: false, order_entry_open: false, selected_order_ids: vec![], positions: vec![], alerts: vec![], next_alert_id: 1, alert_query: String::new(), alerts_panel_open: false,
               chain_symbol: "SPY".into(), chain_sym_input: String::new(), chain_num_strikes: 10, chain_far_dte: 1,
               chain_0dte: (vec![], vec![]), chain_far: (vec![], vec![]),
               chain_select_mode: false, chain_loading: false, chain_last_fetch: None, chain_frozen: false, chain_center_offset: 0, chain_underlying_price: 0.0,
               chain_0_num_strikes: 10, chain_0_frozen: false, chain_0_offset: 0, chain_0_strike_mode: StrikeMode::Count, chain_0_nmf: 0,
               chain_far_num_strikes: 10, chain_far_frozen: false, chain_far_offset: 0, chain_far_strike_mode: StrikeMode::Count, chain_far_nmf: 0,
               saved_options: vec![], dte_filter: -1,
               heat_index: "Watchlist".into(), heat_collapsed: std::collections::HashSet::new(), heat_cols: 2, heat_sort: 0,
               active_workspace: "Default".into(), pending_workspace_load: None, workspace_save_name: String::new(),
               pane_split_h: 0.5, pane_split_v: 0.5, pane_split_h2: 0.5, pane_split_v2: 0.5, pane_divider_dragging: false,
               cmd_palette_open: false, cmd_palette_query: String::new(), cmd_palette_results: vec![], cmd_palette_sel: -1,
               cmd_palette_recent: vec![], cmd_palette_freq: std::collections::HashMap::new(),
               cmd_palette_ai_mode: false, cmd_palette_ai_input: String::new(),
               layout_favorites: vec!["1".into(), "2".into(), "2H".into(), "3".into(), "4".into()],
               layout_dropdown_open: false, layout_dropdown_pos: egui::Pos2::ZERO, dragging_tab: None,
               timeframe_favorites: vec!["1m".into(), "5m".into(), "15m".into(), "30m".into(), "1h".into(), "4h".into(), "1d".into(), "1wk".into()],
               timeframe_dropdown_open: false, timeframe_dropdown_pos: egui::Pos2::ZERO,
               pending_overlay_add: false,
               pane_templates: vec![], pane_template_name: String::new(),
               portfolio_templates: vec!["Default".into()],
               dashboard_templates: vec!["Default".into()],
               heatmap_templates: vec!["Default".into()],
               spreadsheet_templates: vec!["Default".into()],
               plays: vec![], play_editor_open: false,
               play_editor_symbol: String::new(), play_editor_entry: String::new(),
               play_editor_target: String::new(), play_editor_stop: String::new(),
               play_editor_notes: String::new(), play_editor_direction: super::PlayDirection::Long,
               play_editor_type: super::PlayType::Directional, play_editor_qty: 1,
               play_editor_qty_str: "1".into(), play_editor_tags: vec![],
               play_editor_t2: String::new(), play_editor_t2_pct: "25".into(),
               play_editor_t3: String::new(), play_editor_t3_pct: "25".into(),
               play_editor_has_t2: false, play_editor_has_t3: false,
               play_editor_custom_tag: String::new(), play_editor_target_pct: "100".into(),
               play_templates: vec![],
               widget_presets: vec![], widget_preset_name: String::new(),
               discord_open: false,
               discord_messages: vec![],
               discord_input: String::new(),
               discord_channel: String::new(),
               discord_authenticated: false,
               discord_username: String::new(),
               discord_user_id: String::new(),
               discord_guilds: vec![],
               discord_selected_guild: None,
               discord_channels: vec![],
               discord_selected_channel: None,
               discord_connecting: false,
               discord_guild_icons: std::collections::HashMap::new(),
               discord_last_msg_id: None,
               discord_poll_timer: None,
               discord_channels_loading: false,
               discord_messages_loading: false,
               tape_open: false,
               tape_entries: vec![],
               news_open: false,
               journal_open: false,
               news_items: vec![
                   NewsItem { headline: "Fed Holds Rates Steady, Signals Cautious Approach".into(), source: "Reuters".into(), timestamp: "10m".into(), symbol: "SPY".into(), sentiment: 0, url: String::new() },
                   NewsItem { headline: "NVDA Beats Earnings Estimates, Guides Higher".into(), source: "Bloomberg".into(), timestamp: "25m".into(), symbol: "NVDA".into(), sentiment: 1, url: String::new() },
                   NewsItem { headline: "Apple Announces Stock Buyback Program".into(), source: "CNBC".into(), timestamp: "1h".into(), symbol: "AAPL".into(), sentiment: 1, url: String::new() },
                   NewsItem { headline: "Oil Prices Slide on Demand Concerns".into(), source: "Benzinga".into(), timestamp: "2h".into(), symbol: "USO".into(), sentiment: -1, url: String::new() },
                   NewsItem { headline: "Tesla Deliveries Miss Expectations".into(), source: "Reuters".into(), timestamp: "3h".into(), symbol: "TSLA".into(), sentiment: -1, url: String::new() },
               ],
               news_filter_symbol: false,
               scanner_open: false,
               scanner_defs: vec![ScannerDef::preset_gainers(), ScannerDef::preset_losers(), ScannerDef::preset_most_active()],
               scanner_results: vec![],
               scanner_last_fetch: None,
               scanner_fetching: false,
               scanner_new_name: String::new(),
               scanner_new_min_change: -999.0,
               scanner_new_max_change: 999.0,
               scanner_new_min_volume: String::new(),
               scanner_builder_open: false,
               spread_open: false, maximized_pane: None,
               spread_state: super::ui::panels::spread_panel::SpreadState::default(),
               script_open: false,
               script_source: String::new(),
               script_output: String::new(),
               script_ai_prompt: String::new(),
               script_result_tab: super::ui::panels::script_panel::ScriptResultTab::Output,
               script_backtest: None,
               screenshot_open: false,
               screenshot_entries: super::ui::panels::screenshot_panel::load_screenshots(),
               rrg_open: false, rrg_sectors: vec![], rrg_cycle_phase: String::new(),
               rrg_time_offset: 0.0, rrg_tail_length: 5,
               analysis_open: false,
               analysis_tab: crate::chart_renderer::AnalysisTab::Rrg,
               analysis_splits: vec![SplitSection::new(crate::chart_renderer::AnalysisTab::Rrg, 1.0)],
               signals_panel_open: false,
               signals_tab: crate::chart_renderer::SignalsTab::Alerts,
               signals_splits: vec![SplitSection::new(crate::chart_renderer::SignalsTab::Alerts, 1.0)],
               feed_panel_open: false,
               feed_tab: crate::chart_renderer::FeedTab::News,
               feed_splits: vec![SplitSection::new(crate::chart_renderer::FeedTab::News, 1.0)],
               playbook_panel_open: false,
               journal_panel_open: false,
               journal_entries: generate_placeholder_journal(),
               journal_page: 0,
               book_tab: crate::chart_renderer::BookTab::Book }
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
            symbol: s, price: 0.0, prev_close: 0.0, loaded: false,
            is_option: false, underlying: String::new(), option_type: String::new(), strike: 0.0, expiry: String::new(), bid: 0.0, ask: 0.0,
            pinned: false, tags: vec![], rvol: rvol_seed, atr: 0.0,
            high_52wk: 0.0, low_52wk: 0.0, day_high: 0.0, day_low: 0.0,
            avg_daily_range: 2.0, earnings_days: -1, alert_triggered: false, price_history: vec![],
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
                item.price = price;
                item.price_history.push(price);
                if item.price_history.len() > 30 { item.price_history.remove(0); }
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
            symbol: opt_sym, price: 0.0, prev_close: 0.0, loaded: false,
            is_option: true, underlying: underlying.to_string(), option_type: type_str.to_string(), strike, expiry: expiry.to_string(), bid, ask,
            pinned: false, tags: vec![], rvol: 1.0, atr: 0.0,
            high_52wk: 0.0, low_52wk: 0.0, day_high: 0.0, day_low: 0.0,
            avg_daily_range: 2.0, earnings_days: -1, alert_triggered: false, price_history: vec![],
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

    /// Get name of the active watchlist.
    #[allow(dead_code)]
    fn active_name(&self) -> &str {
        self.saved_watchlists.get(self.active_watchlist_idx).map(|w| w.name.as_str()).unwrap_or("Default")
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
        hk("Halt Trading",       "Trading", "halt_trading",   egui::Key::H,      true,  true,  "Ctrl+Shift+H"),
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
    ]
}


// ─── Fetch / IO helpers (moved to io/fetch.rs) ────────────────────────────────
pub use super::io::fetch::fetch_bars_background_pub;
pub(crate) use super::io::fetch::{
    fetch_chain_background, fetch_overlay_chain_background,
    fetch_search_background, fetch_watchlist_prices, fetch_scanner_prices,
    SCANNER_UNIVERSE, active_zero_dte_date, apex_data_chain_to_tuples,
    fetch_indicator_source, submit_ib_order, fetch_option_history_background,
    fetch_history_background, fetch_drawings_background,
    synthesize_occ, fetch_option_bars_background, fetch_bars_background,
    fetch_overlay_bars_background,
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
    toasts: Vec<(String, f32, std::time::Instant, bool)>, // (message, price, created, is_buy)
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
    app_handle: Option<tauri::AppHandle>,
    iw: u32, ih: u32,
    windows: Vec<ChartWindow>,
    spawn_rx: mpsc::Receiver<SpawnRequest>,
}

struct GpuCtx {
    device: wgpu::Device, queue: wgpu::Queue,
    surface: wgpu::Surface<'static>, config: wgpu::SurfaceConfiguration,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    // Set to true when window loses focus — causes a PointerGone event to be injected
    // into the next frame so egui never stays stuck in drag state.
    pointer_gone_needed: bool,
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
        // Fifo (vsync) + frame latency 2 = smooth consistent frame pacing.
        // Latency 2 lets us pipeline: CPU prepares frame N+1 while GPU presents frame N.
        // This eliminates the 10ms acquire stalls we had with latency 1.
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
            wgpu::PresentMode::Fifo
        } else {
            wgpu::PresentMode::AutoVsync
        };
        eprintln!("[native-chart] PresentMode::{:?}, frame latency 2", present_mode);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, format: fmt,
            width: size.width.max(1), height: size.height.max(1),
            present_mode, alpha_mode: caps.alpha_modes[0],
            view_formats: vec![], desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let egui_ctx = egui::Context::default();
        let mut visuals = egui::Visuals::dark();
        // Subtle rounded corners on all widgets
        let r3 = egui::CornerRadius::same(3);
        let r6 = egui::CornerRadius::same(6);
        visuals.window_corner_radius = r6;
        visuals.menu_corner_radius = egui::CornerRadius::same(4);
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

        Some(Self { device, queue, surface, config, egui_ctx, egui_state, egui_renderer, pointer_gone_needed: false })
    }

    fn render(&mut self, window: &Window, panes: &mut Vec<Chart>, active_pane: &mut usize, layout: &mut Layout, watchlist: &mut Watchlist, toasts: &[(String, f32, std::time::Instant, bool)], conn_panel_open: &mut bool, rx: &mpsc::Receiver<ChartCommand>) {
        crate::monitoring::frame_begin();
        crate::foundation::frame_profiler::frame_begin();
        // Bump shadow pipeline frame counter for texture pool recycling.
        crate::ui_kit::widgets::shadow_pipeline::next_frame();

        // Mirror the user's font_idx into TextEngine so PolishedLabel
        // (which passes Family::SansSerif as a sentinel) shapes with
        // the matching primary font.
        crate::ui_kit::widgets::text_engine::set_active_font_idx(watchlist.font_idx);

        // Phase 1: Acquire surface texture
        let t0 = std::time::Instant::now();
        let output = match self.surface.get_current_texture() {
            Ok(t) => t, Err(_) => { self.surface.configure(&self.device, &self.config); return; }
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
        let full_output = self.egui_ctx.run(raw_input, |ctx| { draw_chart(ctx, panes, active_pane, layout, watchlist, toasts, conn_panel_open, rx); });
        self.egui_state.handle_platform_output(window, full_output.platform_output);
        let layout_us = t1.elapsed().as_micros() as u64;

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

        // Phase 5: Render pass
        let t4 = std::time::Instant::now();
        let mut enc2 = self.device.create_command_encoder(&Default::default());
        let mut pass = enc2.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view, resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
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
        let attrs = WindowAttributes::default()
            .with_title("Apex Terminal")
            .with_inner_size(PhysicalSize::new(self.iw, self.ih))
            .with_min_inner_size(PhysicalSize::new(960, 540))
            .with_decorations(false)
            .with_window_icon(make_window_icon())
            .with_active(true)
            .with_maximized(true);

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
                                // would un-maximize.
                                ShowWindow(hwnd, 3);                       // SW_SHOWMAXIMIZED

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
        // Apply persisted global settings
        wl.font_scale = loaded_settings.font_scale;
        wl.font_idx = loaded_settings.font_idx;
        // Re-init fonts if the loaded font differs from default
        if wl.font_idx != 0 { crate::ui_kit::icons::init_fonts(&gpu.egui_ctx, wl.font_idx); }
        wl.compact_mode = loaded_settings.compact_mode;
        wl.pane_header_size = loaded_settings.pane_header_size;
        wl.toolbar_auto_hide = loaded_settings.toolbar_auto_hide;
        wl.show_x_axis = loaded_settings.show_x_axis;
        wl.show_y_axis = loaded_settings.show_y_axis;
        wl.shared_x_axis = loaded_settings.shared_x_axis;
        wl.shared_y_axis = loaded_settings.shared_y_axis;
        if let Some(favs) = loaded_settings.draw_favorites.clone() { wl.draw_favorites = favs; }
        wl.style_idx = loaded_settings.style_idx;
        wl.pane_split_h = loaded_settings.pane_split_h;
        wl.pane_split_v = loaded_settings.pane_split_v;
        wl.pane_split_h2 = loaded_settings.pane_split_h2;
        wl.pane_split_v2 = loaded_settings.pane_split_v2;
        // Load persisted hotkeys (override defaults)
        load_hotkeys(&mut wl.hotkeys);
        // Load persisted templates
        wl.pane_templates = load_templates();
        // Load persisted alerts
        let (wl_alerts, pane_alerts_map) = load_alerts();
        wl.alerts = wl_alerts;
        if !wl.alerts.is_empty() {
            wl.next_alert_id = wl.alerts.iter().map(|a| a.id).max().unwrap_or(0) + 1;
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
                save_state(&cw.panes, cw.layout, &cw.watchlist);
                cw.watchlist.persist();
                self.windows.retain(|w| w.id != wid);
            }
            WindowEvent::Resized(s) => {
                if s.width>0&&s.height>0 {
                    cw.gpu.config.width=s.width; cw.gpu.config.height=s.height;
                    cw.gpu.surface.configure(&cw.gpu.device, &cw.gpu.config);
                    // User-driven (window resize) — must be immediate so the
                    // surface reconfigure is reflected before the next paint.
                    crate::foundation::frame_profiler::note_repaint(
                        concat!(file!(), ":", line!(), " resize"),
                    );
                    cw.win.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                // Drain watchlist price updates before render
                // (these come via the broadcast channel from fetch_watchlist_prices)
                let mut cmds_to_requeue = Vec::new();
                while let Ok(cmd) = cw.rx.try_recv() {
                    match cmd {
                        ChartCommand::WatchlistPrice { ref symbol, price, prev_close } => {
                            cw.watchlist.set_price(symbol, price);
                            cw.watchlist.set_prev_close(symbol, prev_close);
                        }
                        ChartCommand::ScannerPrice { ref symbol, price, prev_close, volume } => {
                            if let Some(r) = cw.watchlist.scanner_results.iter_mut().find(|r| r.symbol == *symbol) {
                                r.price = price;
                                r.volume = volume;
                                r.change_pct = if prev_close > 0.0 { (price - prev_close) / prev_close * 100.0 } else { 0.0 };
                            } else {
                                let change_pct = if prev_close > 0.0 { (price - prev_close) / prev_close * 100.0 } else { 0.0 };
                                cw.watchlist.scanner_results.push(ScanResult {
                                    symbol: symbol.clone(), price, change_pct, volume,
                                });
                            }
                        }
                        ChartCommand::TapeEntry { ref symbol, price, qty, time, is_buy } => {
                            cw.watchlist.tape_entries.push(TapeRow {
                                symbol: symbol.clone(), price, qty, time, is_buy,
                            });
                            if cw.watchlist.tape_entries.len() > 500 {
                                cw.watchlist.tape_entries.drain(..cw.watchlist.tape_entries.len() - 500);
                            }
                        }
                        ChartCommand::ChainData { ref symbol, dte, underlying_price, ref calls, ref puts } => {
                            if *symbol == cw.watchlist.chain_symbol {
                                let to_rows = |data: &[(f32,f32,f32,f32,i32,i32,f32,bool,String)]| -> Vec<OptionRow> {
                                    data.iter().map(|(strike,last,bid,ask,vol,oi,iv,itm,contract)| OptionRow {
                                        strike: *strike, last: *last, bid: *bid, ask: *ask,
                                        volume: *vol, oi: *oi, iv: *iv, itm: *itm, contract: contract.clone(),
                                    }).collect()
                                };
                                if dte == 0 {
                                    cw.watchlist.chain_0dte = (to_rows(calls), to_rows(puts));
                                } else {
                                    cw.watchlist.chain_far = (to_rows(calls), to_rows(puts));
                                }
                                cw.watchlist.chain_loading = false;
                            }
                        }
                        ChartCommand::SearchResults { ref query, ref results, ref source } => {
                            // Only apply if query still matches current search
                            if source == "watchlist" && !query.is_empty()
                                && cw.watchlist.search_query.to_lowercase().starts_with(&query.to_lowercase()) {
                                // Merge: keep static results and append API results that aren't already present
                                for (sym, name) in results {
                                    if !cw.watchlist.search_results.iter().any(|(s, _)| s == sym) {
                                        cw.watchlist.search_results.push((sym.clone(), name.clone()));
                                    }
                                }
                            } else if source == "chain" && !query.is_empty()
                                && cw.watchlist.chain_sym_input.to_lowercase().starts_with(&query.to_lowercase()) {
                                for (sym, name) in results {
                                    if !cw.watchlist.search_results.iter().any(|(s, _)| s == sym) {
                                        cw.watchlist.search_results.push((sym.clone(), name.clone()));
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
                        save_state(&cw.panes, cw.layout, &cw.watchlist);
                        cw.last_save = Some(now);
                    }
                }
                // Process pending workspace load
                if let Some(ws_name) = cw.watchlist.pending_workspace_load.take() {
                    let path = workspace_dir().join(format!("{}.json", ws_name));
                    if let Ok(data) = std::fs::read_to_string(&path) {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                            let (new_panes, new_layout) = {
                                let layout = match json.get("layout").and_then(|v| v.as_str()).unwrap_or("1") {
                                    "2" => Layout::Two, "2H" => Layout::TwoH, "3" => Layout::Three, "3L" => Layout::ThreeL,
                                    "4" => Layout::Four, "4L" => Layout::FourL,
                                    "5C" => Layout::FiveC, "5L" => Layout::FiveL, "5W" => Layout::FiveW, "5R" => Layout::FiveR,
                                    "6" => Layout::Six, "6H" => Layout::SixH, "6L" => Layout::SixL,
                                    "7" => Layout::Seven, "8H" => Layout::EightH, "9" => Layout::Nine, _ => Layout::One,
                                };
                                let theme_idx = json.get("theme_idx").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
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
                                        chart.renko_brick_size = p.get("renko_brick_size").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                        chart.range_bar_size = p.get("range_bar_size").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                        chart.tick_bar_count = p.get("tick_bar_count").and_then(|v| v.as_u64()).unwrap_or(500) as u32;
                                        chart.alt_bars_dirty = true;
                                        chart.vp_mode = match p.get("vp_mode").and_then(|v| v.as_str()).unwrap_or("off") {
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
                                                chart.indicators.push(ind);
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
                        }
                    }
                }
                // Process pending alerts from context menu
                if let Some((sym, price, above)) = PENDING_ALERT.with(|a| a.borrow_mut().take()) {
                    let id = cw.watchlist.next_alert_id; cw.watchlist.next_alert_id += 1;
                    cw.watchlist.alerts.push(Alert { id, symbol: sym, price, above, triggered: false, message: String::new() });
                }
                // Collect order execution toasts
                let new_toasts = PENDING_TOASTS.with(|ts| {
                    let mut v = ts.borrow_mut();
                    let r = v.drain(..).collect::<Vec<_>>();
                    r
                });
                for (msg, price, is_buy) in new_toasts {
                    cw.toasts.push((msg, price, std::time::Instant::now(), is_buy));
                }
                // Remove expired toasts (>5 seconds)
                cw.toasts.retain(|(_, _, created, _)| created.elapsed().as_secs() < 5);
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
                // OS-driven (DPI change) — must be immediate.
                crate::foundation::frame_profiler::note_repaint(
                    concat!(file!(), ":", line!(), " dpi_change"),
                );
                cw.win.request_redraw();
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        // Check for new window spawn requests
        while let Ok(req) = self.spawn_rx.try_recv() {
            self.spawn_window(el, req.rx, Some(req.initial_cmd));
        }

        // Remove windows that requested close
        self.windows.retain(|w| !w.close_requested);

        // Handle symbol/timeframe changes + frame rate for ALL windows
        for cw in &mut self.windows {
            for pane in &mut cw.panes {
                let sym_change = pane.pending_symbol_change.take();
                let tf_change = pane.pending_timeframe_change.take();
                if sym_change.is_some() || tf_change.is_some() {
                    // Stash the OUTGOING (sym, tf)'s bars/ts in the tab cache
                    // before swapping, so re-entry restores instantly.
                    if !pane.symbol.is_empty() && !pane.bars.is_empty() {
                        pane.tab_cache.insert(
                            (pane.symbol.clone(), pane.timeframe.clone()),
                            (pane.bars.clone(), pane.timestamps.clone(), std::time::Instant::now()),
                        );
                        // Cap to 10 entries — evict the oldest by Instant. Each
                        // entry holds bar data (~120 KB at 5000 bars), so an
                        // unbounded cache leaks memory across long sessions.
                        const MAX: usize = 10;
                        while pane.tab_cache.len() > MAX {
                            if let Some((evict_key, _)) = pane.tab_cache.iter()
                                .min_by_key(|(_, (_, _, ts))| *ts)
                                .map(|(k, v)| (k.clone(), v.clone()))
                            {
                                pane.tab_cache.remove(&evict_key);
                            } else { break; }
                        }
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
                                pane.symbol_history_idx = pane.symbol_history.len();
                            }
                        }
                        pane.symbol_nav_in_progress = false;
                        pane.symbol = sym.clone();
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
                    pane.indicators.clear();
                    pane.drawings.clear(); // cleared here, reloaded when LoadBars arrives
                    pane.drawings_requested = false; // allow re-fetch for new timeframe
                    pane.history_loading = false;
                    pane.history_exhausted = false;
                    pane.sim_price = 0.0;
                    pane.last_candle_time = std::time::Instant::now();

                    if let Some(handle) = &self.app_handle {
                        use tauri::Emitter;
                        let _ = handle.emit("native-chart-load", serde_json::json!({
                            "symbol": sym, "timeframe": tf,
                        }));
                    }

                    if pane.is_option && !pane.option_contract.is_empty() {
                        fetch_option_bars_background(pane.option_contract.clone(), sym, tf, pane.bar_source_mark);
                    } else {
                        fetch_bars_background(sym, tf);
                    }
                }
            }

            // ── Linked pane groups: propagate symbol changes across linked panes ──
            // Detect which panes just changed symbol (had pending_symbol_change processed above)
            // by checking which panes have empty bars + link_group > 0
            let mut link_changes: Vec<(u8, String)> = Vec::new();
            for pane in &cw.panes {
                if pane.link_group > 0 && pane.bars.is_empty() && !pane.symbol.is_empty() {
                    let already = link_changes.iter().any(|(g, _)| *g == pane.link_group);
                    if !already {
                        link_changes.push((pane.link_group, pane.symbol.clone()));
                    }
                }
            }
            // For linked panes: ONLY change symbol + fetch bars. Preserve timeframe, indicators, drawings.
            for (group, sym) in &link_changes {
                for pane in &mut cw.panes {
                    if pane.link_group == *group && pane.symbol != *sym && !pane.bars.is_empty() {
                        let tf = pane.timeframe.clone();
                        pane.symbol = sym.clone();
                        pane.bars.clear();
                        pane.timestamps.clear();
                        pane.indicator_bar_count = 0; // force indicator recompute with new bars
                        pane.vol_analytics_computed = 0;
                        pane.history_loading = false;
                        pane.history_exhausted = false;
                        pane.drawings_requested = false;
                        pane.drawings.clear();
                        // DO NOT clear indicators, timeframe, or other pane settings
                        fetch_bars_background(sym.clone(), tf);
                    }
                }
            }

            // Request redraw — Fifo vsync naturally caps at display refresh rate.
            // Data-driven path: collapses to a single paint per ~16 ms window
            // because winit coalesces redundant `request_redraw` calls until
            // RedrawRequested fires.
            crate::foundation::frame_profiler::note_repaint(
                concat!(file!(), ":", line!(), " about_to_wait_tick"),
            );
            cw.win.request_redraw();
        }

        // Frame-pacing control flow:
        //   • When idle (no input + no animations for ≥30 frames) we drop to
        //     WaitUntil(now + 16ms) so background-driven repaints (data ticks,
        //     drawing-db saves) collapse to ≤60 Hz instead of spinning the
        //     event loop. winit will still wake immediately on any input event
        //     or explicit request_redraw, so latency for user actions is
        //     unaffected.
        //   • Otherwise: Poll, exactly as before — Fifo present blocks in
        //     get_current_texture() so the loop is naturally vsync-bounded.
        if crate::foundation::frame_profiler::is_idle() {
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(16);
            el.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(deadline));
        } else {
            el.set_control_flow(winit::event_loop::ControlFlow::Poll);
        }
    }
}

// ─── State persistence ───────────────────────────────────────────────────────

fn state_path() -> std::path::PathBuf {
    let mut p = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("apex-terminal");
    let _ = std::fs::create_dir_all(&p);
    p.push("native-chart-state.json");
    p
}

fn workspace_dir() -> std::path::PathBuf {
    let mut p = state_path(); p.pop(); p.push("workspaces"); let _ = std::fs::create_dir_all(&p); p
}

fn workspace_to_json(panes: &[Chart], layout: Layout) -> String {
    let pane_data: Vec<serde_json::Value> = panes.iter().map(|p| {
        let indicators: Vec<serde_json::Value> = p.indicators.iter().map(|ind| serde_json::json!({
            "kind": ind.kind.label(), "period": ind.period, "color": ind.color,
            "visible": ind.visible, "thickness": ind.thickness,
            "param2": ind.param2, "param3": ind.param3, "param4": ind.param4,
            "source": ind.source, "offset": ind.offset,
            "ob_level": ind.ob_level, "os_level": ind.os_level,
            "source_tf": ind.source_tf,
            "line_style": match ind.line_style { LineStyle::Solid => "solid", LineStyle::Dashed => "dashed", LineStyle::Dotted => "dotted" },
        })).collect();
        serde_json::json!({
            "symbol": p.symbol, "timeframe": p.timeframe,
            "show_volume": p.show_volume, "show_oscillators": p.show_oscillators,
            "ohlc_tooltip": p.ohlc_tooltip, "magnet": p.magnet, "log_scale": p.log_scale,
            "show_vwap_bands": p.show_vwap_bands, "show_cvd": p.show_cvd,
            "show_delta_volume": p.show_delta_volume, "show_rvol": p.show_rvol,
            "show_ma_ribbon": p.show_ma_ribbon, "show_prev_close": p.show_prev_close,
            "show_auto_sr": p.show_auto_sr, "show_auto_fib": p.show_auto_fib, "swing_leg_mode": p.swing_leg_mode, "show_footprint": p.show_footprint,
            "show_gamma": p.show_gamma, "show_darkpool": p.show_darkpool, "show_events": p.show_events, "hit_highlight": p.hit_highlight,
            "show_pnl_curve": p.show_pnl_curve, "show_pattern_labels": p.show_pattern_labels,
            "link_group": p.link_group,
            "session_shading": p.session_shading,
            "rth_start_minutes": p.rth_start_minutes,
            "rth_end_minutes": p.rth_end_minutes,
            "eth_bar_opacity": p.eth_bar_opacity,
            "session_bg_tint": p.session_bg_tint,
            "session_bg_color": p.session_bg_color,
            "session_bg_opacity": p.session_bg_opacity,
            "session_break_lines": p.session_break_lines,
            "candle_mode": match p.candle_mode {
                CandleMode::Standard => "std", CandleMode::Violin => "vln",
                CandleMode::Gradient => "grd", CandleMode::ViolinGradient => "vg",
                CandleMode::HeikinAshi => "ha", CandleMode::Line => "line", CandleMode::Area => "area",
                    CandleMode::Renko => "rnk", CandleMode::RangeBar => "rng", CandleMode::TickBar => "tck",
            },
            "renko_brick_size": p.renko_brick_size,
            "range_bar_size": p.range_bar_size,
            "tick_bar_count": p.tick_bar_count,
            "vp_mode": match p.vp_mode {
                VolumeProfileMode::Off => "off", VolumeProfileMode::Classic => "classic",
                VolumeProfileMode::Heatmap => "heatmap", VolumeProfileMode::Strip => "strip",
                VolumeProfileMode::Clean => "clean",
            },
            "indicators": indicators,
        })
    }).collect();
    let state = serde_json::json!({
        "version": 2,
        "layout": layout.label(),
        "theme_idx": panes.first().map(|p| p.theme_idx).unwrap_or(5),
        "panes": pane_data,
        "recent_symbols": panes.first().map(|p| &p.recent_symbols).cloned().unwrap_or_default(),
    });
    serde_json::to_string_pretty(&state).unwrap_or_default()
}

pub(crate) fn save_workspace(name: &str, panes: &[Chart], layout: Layout) {
    let json = workspace_to_json(panes, layout);
    let safe_name: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' }).collect();
    let path = workspace_dir().join(format!("{}.json", safe_name));
    let _ = std::fs::write(path, json);
}

pub(crate) fn list_workspaces() -> Vec<String> {
    let dir = workspace_dir();
    let mut names: Vec<String> = std::fs::read_dir(dir).ok().map(|entries| {
        entries.filter_map(|e| {
            let name = e.ok()?.file_name().to_string_lossy().to_string();
            if name.ends_with(".json") { Some(name.trim_end_matches(".json").to_string()) } else { None }
        }).collect()
    }).unwrap_or_default();
    names.sort();
    names
}

pub(crate) fn save_state(panes: &[Chart], layout: Layout, watchlist: &Watchlist) {
    let pane_data: Vec<serde_json::Value> = panes.iter().map(|p| {
        // Serialize indicators — include ALL styling fields
        let indicators: Vec<serde_json::Value> = p.indicators.iter().map(|ind| serde_json::json!({
            "kind": ind.kind.label(), "period": ind.period, "color": ind.color,
            "visible": ind.visible, "thickness": ind.thickness,
            "param2": ind.param2, "param3": ind.param3, "param4": ind.param4,
            "source": ind.source, "offset": ind.offset,
            "ob_level": ind.ob_level, "os_level": ind.os_level,
            "source_tf": ind.source_tf,
            "line_style": match ind.line_style { LineStyle::Solid => "solid", LineStyle::Dashed => "dashed", LineStyle::Dotted => "dotted" },
            // Band styling (BB, Keltner, etc.)
            "upper_color": ind.upper_color, "lower_color": ind.lower_color,
            "fill_color_hex": ind.fill_color_hex,
            "upper_thickness": ind.upper_thickness, "lower_thickness": ind.lower_thickness,
        })).collect();
        serde_json::json!({
            "symbol": p.symbol, "timeframe": p.timeframe,
            // Toggles
            "show_volume": p.show_volume, "show_oscillators": p.show_oscillators,
            "ohlc_tooltip": p.ohlc_tooltip, "magnet": p.magnet, "log_scale": p.log_scale,
            "show_vwap_bands": p.show_vwap_bands, "show_cvd": p.show_cvd,
            "show_delta_volume": p.show_delta_volume, "show_rvol": p.show_rvol,
            "show_ma_ribbon": p.show_ma_ribbon, "show_prev_close": p.show_prev_close,
            "show_auto_sr": p.show_auto_sr, "show_auto_fib": p.show_auto_fib, "swing_leg_mode": p.swing_leg_mode, "show_footprint": p.show_footprint,
            "show_gamma": p.show_gamma, "show_darkpool": p.show_darkpool, "show_events": p.show_events, "hit_highlight": p.hit_highlight,
            "show_pnl_curve": p.show_pnl_curve, "show_pattern_labels": p.show_pattern_labels,
            "link_group": p.link_group,
            // Session shading
            "session_shading": p.session_shading,
            "rth_start_minutes": p.rth_start_minutes,
            "rth_end_minutes": p.rth_end_minutes,
            "eth_bar_opacity": p.eth_bar_opacity,
            "session_bg_tint": p.session_bg_tint,
            "session_bg_color": p.session_bg_color,
            "session_bg_opacity": p.session_bg_opacity,
            "session_break_lines": p.session_break_lines,
            // Modes
            "candle_mode": match p.candle_mode {
                CandleMode::Standard => "std", CandleMode::Violin => "vln",
                CandleMode::Gradient => "grd", CandleMode::ViolinGradient => "vg",
                CandleMode::HeikinAshi => "ha", CandleMode::Line => "line", CandleMode::Area => "area",
                    CandleMode::Renko => "rnk", CandleMode::RangeBar => "rng", CandleMode::TickBar => "tck",
            },
            "renko_brick_size": p.renko_brick_size,
            "range_bar_size": p.range_bar_size,
            "tick_bar_count": p.tick_bar_count,
            "vp_mode": match p.vp_mode {
                VolumeProfileMode::Off => "off", VolumeProfileMode::Classic => "classic",
                VolumeProfileMode::Heatmap => "heatmap", VolumeProfileMode::Strip => "strip",
                VolumeProfileMode::Clean => "clean",
            },
            // Indicators
            "indicators": indicators,
            // Chart widgets
            "chart_widgets": serde_json::to_value(&p.chart_widgets).unwrap_or_default(),
            // Option-pane state (preserved across sessions so option charts
            // restore as option charts, not as broken stock fetches).
            "is_option": p.is_option,
            "option_contract": p.option_contract,
            "option_strike": p.option_strike,
            "option_type": p.option_type,
            "option_expiry": p.option_expiry,
            "underlying": p.underlying,
            // MARK_BARS_PROTOCOL — persist Last/Mark choice per chart.
            "bar_source": if p.bar_source_mark { "mark" } else { "last" },
        })
    }).collect();
    // Global settings from Watchlist
    let phs = match watchlist.pane_header_size {
        crate::chart_renderer::PaneHeaderSize::Compact => "compact",
        crate::chart_renderer::PaneHeaderSize::Normal => "normal",
        crate::chart_renderer::PaneHeaderSize::Expanded => "expanded",
    };
    let state = serde_json::json!({
        "version": 3,
        "layout": layout.label(),
        "theme_idx": panes.first().map(|p| p.theme_idx).unwrap_or(5),
        "panes": pane_data,
        "recent_symbols": panes.first().map(|p| &p.recent_symbols).cloned().unwrap_or_default(),
        "draw_favorites": watchlist.draw_favorites,
        "style_idx": watchlist.style_idx,
        "settings": {
            "font_scale": watchlist.font_scale,
            "font_idx": watchlist.font_idx,
            "compact_mode": watchlist.compact_mode,
            "pane_header_size": phs,
            "toolbar_auto_hide": watchlist.toolbar_auto_hide,
            "show_x_axis": watchlist.show_x_axis,
            "show_y_axis": watchlist.show_y_axis,
            "shared_x_axis": watchlist.shared_x_axis,
            "shared_y_axis": watchlist.shared_y_axis,
            "pane_split_h": watchlist.pane_split_h,
            "pane_split_v": watchlist.pane_split_v,
            "pane_split_h2": watchlist.pane_split_h2,
            "pane_split_v2": watchlist.pane_split_v2,
        },
    });
    let _ = std::fs::write(state_path(), serde_json::to_string_pretty(&state).unwrap_or_default());

    // ── Persist alerts ──
    save_alerts(watchlist, panes);
    // ── Persist hotkeys ──
    save_hotkeys(watchlist);
    // ── Persist templates ──
    save_templates(&watchlist.pane_templates);
}

/// Loaded global settings (applied to Watchlist after load)
struct LoadedSettings {
    font_scale: f32,
    font_idx: usize,
    compact_mode: bool,
    pane_header_size: crate::chart_renderer::PaneHeaderSize,
    toolbar_auto_hide: bool,
    show_x_axis: bool, show_y_axis: bool,
    shared_x_axis: bool, shared_y_axis: bool,
    pane_split_h: f32, pane_split_v: f32, pane_split_h2: f32, pane_split_v2: f32,
    draw_favorites: Option<Vec<String>>,
    style_idx: usize,
}
impl Default for LoadedSettings { fn default() -> Self { Self {
    font_scale: 1.6, font_idx: 0, compact_mode: false,
    pane_header_size: crate::chart_renderer::PaneHeaderSize::Compact,
    toolbar_auto_hide: false,
    show_x_axis: true, show_y_axis: true,
    shared_x_axis: false, shared_y_axis: false,
    pane_split_h: 0.5, pane_split_v: 0.5, pane_split_h2: 0.5, pane_split_v2: 0.5,
    draw_favorites: None,
    style_idx: 0,
}}}

fn load_state() -> (Vec<Chart>, Layout, LoadedSettings) {
    let path = state_path();
    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return (vec![Chart::new()], Layout::One, LoadedSettings::default()),
    };
    let json: serde_json::Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(_) => return (vec![Chart::new()], Layout::One, LoadedSettings::default()),
    };

    let layout = match json.get("layout").and_then(|v| v.as_str()).unwrap_or("1") {
        "2" => Layout::Two, "2H" => Layout::TwoH, "3" => Layout::Three, "4" => Layout::Four,
        "6" => Layout::Six, "6H" => Layout::SixH, "9" => Layout::Nine, _ => Layout::One,
    };
    let theme_idx = json.get("theme_idx").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    let recents: Vec<(String, String)> = json.get("recent_symbols").and_then(|v| v.as_array()).map(|arr| {
        arr.iter().filter_map(|v| {
            let a = v.as_array()?;
            Some((a.first()?.as_str()?.to_string(), a.get(1)?.as_str()?.to_string()))
        }).collect()
    }).unwrap_or_default();

    let pane_arr = json.get("panes").and_then(|v| v.as_array());
    let mut panes = Vec::new();
    if let Some(arr) = pane_arr {
        for p in arr {
            let sym = p.get("symbol").and_then(|v| v.as_str()).unwrap_or("AAPL");
            let tf = p.get("timeframe").and_then(|v| v.as_str()).unwrap_or("5m");
            let mut chart = Chart::new_with(sym, tf);
            chart.theme_idx = theme_idx;
            chart.recent_symbols = recents.clone();
            chart.pending_symbol_change = Some(sym.to_string());

            // Restore toggle states
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

            // Restore session shading settings
            chart.session_shading = gb("session_shading", false);
            chart.rth_start_minutes = p.get("rth_start_minutes").and_then(|v| v.as_u64()).unwrap_or(570) as u16;
            chart.rth_end_minutes = p.get("rth_end_minutes").and_then(|v| v.as_u64()).unwrap_or(960) as u16;
            chart.eth_bar_opacity = p.get("eth_bar_opacity").and_then(|v| v.as_f64()).unwrap_or(0.35) as f32;
            chart.session_bg_tint = gb("session_bg_tint", false);
            chart.session_bg_color = p.get("session_bg_color").and_then(|v| v.as_str()).unwrap_or("#1a1a2e").to_string();
            chart.session_bg_opacity = p.get("session_bg_opacity").and_then(|v| v.as_f64()).unwrap_or(0.15) as f32;
            chart.session_break_lines = gb("session_break_lines", true);

            // Restore candle mode
            chart.candle_mode = match p.get("candle_mode").and_then(|v| v.as_str()).unwrap_or("std") {
                "vln" => CandleMode::Violin, "grd" => CandleMode::Gradient, "vg" => CandleMode::ViolinGradient,
                "ha" => CandleMode::HeikinAshi, "line" => CandleMode::Line, "area" => CandleMode::Area,
                    "rnk" => CandleMode::Renko, "rng" => CandleMode::RangeBar, "tck" => CandleMode::TickBar,
                _ => CandleMode::Standard,
            };
            chart.renko_brick_size = p.get("renko_brick_size").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            chart.range_bar_size = p.get("range_bar_size").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            chart.tick_bar_count = p.get("tick_bar_count").and_then(|v| v.as_u64()).unwrap_or(500) as u32;
            chart.alt_bars_dirty = true; // force recompute on load
            // Restore volume profile mode
            chart.vp_mode = match p.get("vp_mode").and_then(|v| v.as_str()).unwrap_or("off") {
                "classic" => VolumeProfileMode::Classic, "heatmap" => VolumeProfileMode::Heatmap,
                "strip" => VolumeProfileMode::Strip, "clean" => VolumeProfileMode::Clean,
                _ => VolumeProfileMode::Off,
            };

            // Restore indicators
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
                    ind.visible = visible;
                    ind.thickness = thickness;
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
                    // Band styling (BB, Keltner, etc.)
                    ind.upper_color = ind_json.get("upper_color").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    ind.lower_color = ind_json.get("lower_color").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    ind.fill_color_hex = ind_json.get("fill_color_hex").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    ind.upper_thickness = ind_json.get("upper_thickness").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    ind.lower_thickness = ind_json.get("lower_thickness").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    chart.indicators.push(ind);
                }
            }

            // Restore chart widgets
            if let Some(wv) = p.get("chart_widgets") {
                if let Ok(widgets) = serde_json::from_value::<Vec<super::ChartWidget>>(wv.clone()) {
                    chart.chart_widgets = widgets;
                    // Reset animation state (transient, not meaningful from disk)
                    for w in &mut chart.chart_widgets { w.anim_init = false; }
                }
            }

            // Option-pane state — restore the contract metadata so the pane
            // re-fetches via fetch_option_bars_background instead of trying
            // to load the (non-existent) display label as a stock symbol.
            chart.is_option = gb("is_option", false);
            chart.option_contract = p.get("option_contract").and_then(|v| v.as_str()).unwrap_or("").to_string();
            chart.option_strike   = p.get("option_strike").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            chart.option_type     = p.get("option_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
            chart.option_expiry   = p.get("option_expiry").and_then(|v| v.as_str()).unwrap_or("").to_string();
            chart.underlying      = p.get("underlying").and_then(|v| v.as_str()).unwrap_or("").to_string();
            // MARK_BARS_PROTOCOL — default to "last" on missing.
            chart.bar_source_mark = p.get("bar_source").and_then(|v| v.as_str()).unwrap_or("last") == "mark";

            panes.push(chart);
        }
    }
    if panes.is_empty() { panes.push(Chart::new()); }
    // Trim excess panes to match layout capacity
    let max = layout.max_panes();
    panes.truncate(max);

    // Restore global settings (version 3+)
    let mut settings = LoadedSettings::default();
    if let Some(s) = json.get("settings") {
        settings.font_scale = s.get("font_scale").and_then(|v| v.as_f64()).unwrap_or(1.6) as f32;
        settings.font_idx = s.get("font_idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        settings.compact_mode = s.get("compact_mode").and_then(|v| v.as_bool()).unwrap_or(false);
        settings.pane_header_size = match s.get("pane_header_size").and_then(|v| v.as_str()).unwrap_or("compact") {
            "normal" => crate::chart_renderer::PaneHeaderSize::Normal,
            "expanded" => crate::chart_renderer::PaneHeaderSize::Expanded,
            _ => crate::chart_renderer::PaneHeaderSize::Compact,
        };
        settings.toolbar_auto_hide = s.get("toolbar_auto_hide").and_then(|v| v.as_bool()).unwrap_or(false);
        settings.show_x_axis = s.get("show_x_axis").and_then(|v| v.as_bool()).unwrap_or(true);
        settings.show_y_axis = s.get("show_y_axis").and_then(|v| v.as_bool()).unwrap_or(true);
        settings.shared_x_axis = s.get("shared_x_axis").and_then(|v| v.as_bool()).unwrap_or(false);
        settings.shared_y_axis = s.get("shared_y_axis").and_then(|v| v.as_bool()).unwrap_or(false);
        settings.pane_split_h = s.get("pane_split_h").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
        settings.pane_split_v = s.get("pane_split_v").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
        settings.pane_split_h2 = s.get("pane_split_h2").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
        settings.pane_split_v2 = s.get("pane_split_v2").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
    }
    // Drawing-tool favorites — top-level key (added independently of settings).
    if let Some(arr) = json.get("draw_favorites").and_then(|v| v.as_array()) {
        let favs: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
        if !favs.is_empty() { settings.draw_favorites = Some(favs); }
    }
    // Style index — top-level key, clamped to known list.
    if let Some(s) = json.get("style_idx").and_then(|v| v.as_u64()) {
        settings.style_idx = (s as usize).min(STYLE_NAMES.len().saturating_sub(1));
    }

    (panes, layout, settings)
}

// ─── Alerts persistence ──────────────────────────────────────────────────────

fn alerts_path() -> std::path::PathBuf {
    let mut p = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("apex-terminal"); let _ = std::fs::create_dir_all(&p);
    p.push("alerts.json"); p
}

fn save_alerts(watchlist: &Watchlist, panes: &[Chart]) {
    use crate::chart_renderer::trading::PriceAlert;
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
    let _ = std::fs::write(alerts_path(), serde_json::to_string_pretty(&json).unwrap_or_default());
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

fn save_hotkeys(watchlist: &Watchlist) {
    let arr: Vec<serde_json::Value> = watchlist.hotkeys.iter().map(|hk| serde_json::json!({
        "action": hk.action, "key_name": hk.key_name,
        "ctrl": hk.ctrl, "shift": hk.shift, "alt": hk.alt,
    })).collect();
    let _ = std::fs::write(hotkeys_path(), serde_json::to_string_pretty(&serde_json::Value::Array(arr)).unwrap_or_default());
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
    // Remove existing files first (in case templates were deleted)
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            if e.path().extension().map_or(false, |x| x == "json") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    for (name, data) in templates {
        let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' }).collect();
        let path = dir.join(format!("{}.json", safe));
        let _ = std::fs::write(path, serde_json::to_string_pretty(data).unwrap_or_default());
    }
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
    let _ = std::fs::write(watchlists_path(), serde_json::to_string_pretty(&state).unwrap_or_default());
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
                                symbol, price: 0.0, prev_close: 0.0, loaded: false,
                                is_option, underlying, option_type, strike, expiry, bid, ask,
                                pinned: false, tags: vec![], rvol: rvol_seed, atr: 0.0,
                                high_52wk: 0.0, low_52wk: 0.0, day_high: 0.0, day_low: 0.0,
                                avg_daily_range: 2.0, earnings_days: -1, alert_triggered: false, price_history: vec![],
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
                symbol: s.into(), price: 0.0, prev_close: 0.0, loaded: false,
                is_option: false, underlying: String::new(), option_type: String::new(), strike: 0.0, expiry: String::new(), bid: 0.0, ask: 0.0,
                pinned: false, tags: vec![], rvol: rvol_seed, atr: 0.0,
                high_52wk: 0.0, low_52wk: 0.0, day_high: 0.0, day_low: 0.0,
                avg_daily_range: 2.0, earnings_days: -1, alert_triggered: false, price_history: vec![],
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

/// Called from Tauri command thread to open a new native chart window.
/// First call starts the render thread; subsequent calls send spawn requests.
pub fn open_window(rx: mpsc::Receiver<ChartCommand>, initial_cmd: ChartCommand, app_handle: Option<tauri::AppHandle>) {
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

    let handle = app_handle.clone();
    std::thread::spawn(move || {
        #[cfg(target_os = "windows")]
        let el = {
            use winit::platform::windows::EventLoopBuilderExtWindows;
            EventLoop::builder().with_any_thread(true).build().unwrap()
        };
        #[cfg(not(target_os = "windows"))]
        let el = EventLoop::builder().build().unwrap();
        let mut app = App {
            app_handle: handle, iw: 1920, ih: 1080,
            windows: Vec::new(), spawn_rx,
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
pub fn open_window_blocking(rx: mpsc::Receiver<ChartCommand>, initial_cmd: ChartCommand, app_handle: Option<tauri::AppHandle>) {
    use winit::platform::macos::EventLoopBuilderExtMacOS;

    let spawn_tx_lock = SPAWN_TX.get_or_init(|| Mutex::new(None));
    let (spawn_tx, spawn_rx) = mpsc::channel::<SpawnRequest>();
    let _ = spawn_tx.send(SpawnRequest { rx, initial_cmd });
    *spawn_tx_lock.lock().unwrap() = Some(spawn_tx);

    let el = EventLoop::builder()
        .with_activate_ignoring_other_apps(true)
        .build()
        .unwrap();
    let mut app = App { app_handle, iw: 1920, ih: 1080, windows: Vec::new(), spawn_rx };
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

