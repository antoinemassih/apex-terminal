//! Shared styling helpers — single source of truth for all UI style decisions.
//!
//! # Changing the look in one place
//! - Font sizes   → `FONT_*` constants
//! - Spacing      → `GAP_*` constants
//! - Corner radii → `RADIUS_*` constants
//! - Stroke widths → `STROKE_*` constants
//! - Alpha tiers  → `ALPHA_*` constants
//! - Drop shadows → `SHADOW_*` constants
//! - Fixed colors → `TEXT_*` constants
//!
//! All helpers below use these constants internally, so a single change propagates everywhere.

use egui::{self, Color32, RichText, Stroke};
use std::cell::Cell;

/// Register an element hit for inspect mode. No-op when design-mode is off.
#[inline(always)]
fn hit(r: &egui::Rect, family: &'static str, category: &'static str) {
    crate::design_tokens::register_hit(
        [r.min.x, r.min.y, r.width(), r.height()], family, category);
}

// ─── Per-frame token snapshot (spec §5 Rule 2) ────────────────────────────────
//
// `FRAME_TOKENS` is a lock-free `thread_local` holding a flat `Copy` struct of
// every design-token value that was previously computed via `dt_f32!`/`dt_u8!`
// on each call site.  `begin_frame()` refreshes it once per frame (wired inside
// `set_active_style`) so all token reads within that frame are a single `Cell`
// get — ~1 ns, no lock, no map lookup (spec §5 Rule 3).
//
// Hardcoded-constant token fns (font_sm = 11.0, gap_sm = 8.0, …) are NOT
// routed through this struct — they are already free compile-time constants and
// adding them would gain nothing while breaking the "leave unchanged if already
// guaranteed identical" contract.

/// Flat resolved values for the design tokens that were previously backed by
/// `dt_f32!` / `dt_u8!` lookups.  All fields are `f32` or `u8` (Copy primitives).
#[derive(Clone, Copy, Debug)]
pub struct TokenSnapshot {
    // ── Spacing ─────────────────────────────────────────────────────────────
    pub gap_xs_mid: f32,

    // ── Radii ───────────────────────────────────────────────────────────────
    pub radius_xs: f32,
    pub radius_sm: f32,
    pub radius_md: f32,
    pub radius_lg: f32,

    // ── Stroke widths ────────────────────────────────────────────────────────
    pub stroke_hair:   f32,
    pub stroke_thin:   f32,
    pub stroke_medium: f32,
    pub stroke_std:    f32,
    pub stroke_bold:   f32,
    pub stroke_thick:  f32,

    // ── Alpha tiers (u8, 0-255) ──────────────────────────────────────────────
    pub alpha_faint:  u8,
    pub alpha_ghost:  u8,
    pub alpha_soft:   u8,
    pub alpha_subtle: u8,
    pub alpha_tint:   u8,
    pub alpha_muted:  u8,
    pub alpha_dim:    u8,
    pub alpha_line:   u8,
    pub alpha_strong: u8,
    pub alpha_active: u8,
    pub alpha_heavy:  u8,
    pub alpha_solid:  u8,

    // ── Shadow primitives ───────────────────────────────────────────────────
    pub shadow_offset: f32,
    pub shadow_alpha:  u8,
    pub shadow_spread: f32,
}

/// Compile-time default — matches every token fn's non-design-mode constant
/// so the first frame (before `begin_frame` fires) returns identical values.
const DEFAULT_TOKEN_SNAPSHOT: TokenSnapshot = TokenSnapshot {
    gap_xs_mid:    6.0,
    radius_xs:     2.0,
    radius_sm:     4.0,
    radius_md:     6.0,
    radius_lg:    12.0,
    stroke_hair:   0.3,
    stroke_thin:   0.5,
    stroke_medium: 0.8,
    stroke_std:    1.0,
    stroke_bold:   1.5,
    stroke_thick:  2.0,
    alpha_faint:   10,
    alpha_ghost:   15,
    alpha_soft:    20,
    alpha_subtle:  40,
    alpha_tint:    48,
    alpha_muted:   60,
    alpha_dim:     60,
    alpha_line:    80,
    alpha_strong:  80,
    alpha_active: 100,
    alpha_heavy:  120,
    alpha_solid:  200,
    shadow_offset: 2.0,
    shadow_alpha:   60,
    shadow_spread:  4.0,
};

thread_local! {
    static FRAME_TOKENS: Cell<TokenSnapshot> = Cell::new(DEFAULT_TOKEN_SNAPSHOT);
}

/// Refresh the per-frame token snapshot from the current active style /
/// design-mode settings.  Call once per frame, after `set_active_style`.
///
/// This is wired inside `set_active_style` so callers in `core.rs` need no
/// changes — they already call `set_active_style` at frame start.
///
/// **Source-swap (Phase B)**: per-style dimension tokens (radii, stroke
/// widths) are sourced LIVE from the active style's `StyleSettings` via
/// `current()` — selecting a different style now changes them in shipping
/// builds, not just design-mode.  The default style (Meridien) is defined to
/// hold the pre-swap values, so the default look is preserved.  Global tokens
/// (alpha tiers, `gap_xs_mid`, `stroke_medium`, shadow geometry) have no
/// per-style backing and stay on the `dt_f32!`/`dt_u8!` path.
#[inline]
pub fn begin_frame() {
    // ── Hot-reload override (one RwLock::read per frame — ~20–50 ns) ──────────
    // If a background watcher has installed a live StyleSystem override, source
    // the per-style dimension tokens (radii, strokes) from it instead of from
    // the active `StyleSettings`.  When no override is present (`None`) the
    // existing `current()` path is used unchanged.
    let override_style = crate::design_system::active_override();

    // Active style's StyleSettings — the live per-style dimension source.
    let st = current();

    // Resolve radii from override when present, otherwise from StyleSettings.
    let (r_xs, r_sm, r_md, r_lg) = if let Some(ref ov) = override_style {
        (
            ov.radii.xs,
            ov.radii.sm,
            ov.radii.md,
            ov.radii.lg,
        )
    } else {
        (
            st.r_xs as f32,
            st.r_sm as f32,
            st.r_md as f32,
            st.r_lg as f32,
        )
    };

    // Resolve strokes from override when present, otherwise from StyleSettings.
    let (stroke_hair, stroke_thin, stroke_std, stroke_bold, stroke_thick) =
        if let Some(ref ov) = override_style {
            (
                ov.strokes.thin,   // thin  → hair (sub-pixel hairline)
                ov.strokes.std,    // std   → thin (1 px standard)
                ov.strokes.std,    // std   → std  (same level)
                ov.strokes.md,     // md    → bold (1.5 px emphasis)
                ov.strokes.heavy,  // heavy → thick (2 px)
            )
        } else {
            (
                st.stroke_hair,
                st.stroke_thin,
                st.stroke_std,
                st.stroke_bold,
                st.stroke_thick,
            )
        };

    let snap = TokenSnapshot {
        gap_xs_mid:    crate::dt_f32!(spacing.xs_mid, 6.0),
        radius_xs:     r_xs,
        radius_sm:     r_sm,
        radius_md:     r_md,
        radius_lg:     r_lg,
        stroke_hair,
        stroke_thin,
        stroke_medium: crate::dt_f32!(stroke.medium, 0.8),
        stroke_std,
        stroke_bold,
        stroke_thick,
        alpha_faint:   crate::dt_u8!(alpha.faint,   10),
        alpha_ghost:   crate::dt_u8!(alpha.ghost,   15),
        alpha_soft:    crate::dt_u8!(alpha.soft,    20),
        alpha_subtle:  crate::dt_u8!(alpha.subtle,  40),
        alpha_tint:    crate::dt_u8!(alpha.tint,    48),
        alpha_muted:   crate::dt_u8!(alpha.muted,   60),
        alpha_dim:     crate::dt_u8!(alpha.dim,     60),
        alpha_line:    crate::dt_u8!(alpha.line,    80),
        alpha_strong:  crate::dt_u8!(alpha.strong,  80),
        alpha_active:  crate::dt_u8!(alpha.active, 100),
        alpha_heavy:   crate::dt_u8!(alpha.heavy,  120),
        alpha_solid:   crate::dt_u8!(alpha.solid,  200),
        shadow_offset: crate::dt_f32!(shadow.offset, 2.0),
        shadow_alpha:  crate::dt_u8!(shadow.alpha,   60),
        shadow_spread: crate::dt_f32!(shadow.spread,  4.0),
    };
    FRAME_TOKENS.with(|c| c.set(snap));
}

// ─── Typography scale ─────────────────────────────────────────────────────────
// Typography scale — 4 sizes, monospace pinned for financial data.
//
// Density-first scale anchored at 9px for high-density financial UIs
// (price ladders, options chains, watchlists). Hierarchy comes through
// size + color, not new fonts.
//
// Size scale (use ONLY these 4):
//   font_xs()  =  9.0   — micro-labels, dropdown items, badge text
//   font_sm()  = 11.0   — default body, list rows, tab labels, nav buttons
//   font_md()  = 13.0   — emphasized body, panel titles
//   font_lg()  = 16.0   — section headers, modal titles
//
// Anything outside this scale is a bug. If you need 10px or 12px, you
// probably want font_sm. If you need 14-15px, you probably want font_md
// or font_lg.
//
// Weight is currently single-axis (Medium for sans, Regular for mono).
// Multi-weight support is a future phase; for now achieve hierarchy
// through size and color (use t.dim for secondary, t.text for primary).
//
// Monospace is ALWAYS JetBrains Mono regardless of the font picker.
// Use mono_xs/sm/md/lg for prices, quantities, OCC tickers, anything
// tabular. The font picker controls proportional UI chrome only.

// Below `font_xs` are the *micro* tier — chart annotations, badge counts on
// glyphs. They exist because chart_widgets / pane.rs need labels at 6–8px to
// fit dense overlays without overlapping ticks. New UI code should NOT use
// these; they are the floor for chart rendering only.
/// 6.0 — chart micro-overlay text only (RSI zones, market phase). Avoid in UI chrome.
pub fn font_4xs() -> f32 { 6.0 }
/// 7.0 — chart annotations (volume ratios, trade entries). Avoid in UI chrome.
pub fn font_3xs() -> f32 { 7.0 }
/// 8.0 — small badges and overlay tags (price-axis order labels).
pub fn font_2xs() -> f32 { 8.0 }
/// 9.0 — micro-labels, dropdown items, badge text.
pub fn font_xs() -> f32 { 9.0 }
/// 10.0 — between xs and sm (compact column headers, condensed body).
pub fn font_xs_plus() -> f32 { 10.0 }
/// 11.0 — default body, list rows, tab labels, nav buttons.
pub fn font_sm() -> f32 { 11.0 }
/// 13.0 — emphasized body, panel titles.
pub fn font_md() -> f32 { 13.0 }
/// 14.0 — between md and lg (large chart annotations, hero stats).
pub fn font_md_plus() -> f32 { 14.0 }
/// 16.0 — section headers, modal titles.
pub fn font_lg() -> f32 { 16.0 }
/// 22.0 — hero numbers, modal hero titles.
pub fn font_xl() -> f32 { 22.0 }

// ─── Display-font tier (proportional, large numerics only) ───────────────────
// For infographic/hero KPI numbers rendered with `FontId::proportional(...)`.
// These are NOT for UI chrome — use them only in chart widget body paint calls
// where a large display number is the primary focal point of the widget.
//
//   font_display_sm()  = 28.0 — compact hero (single KPI in a narrow widget)
//   font_display_md()  = 32.0 — standard hero number
//   font_display_lg()  = 42.0 — prominent hero / dual-KPI banner
//   font_display_xl()  = 56.0 — primary focal number (full-width banner widget)

/// 28.0 — compact display hero (narrow widget KPI, countdown digits).
#[inline] pub fn font_display_sm() -> f32 { 28.0 }
/// 32.0 — standard display hero number (primary gauge focal point).
#[inline] pub fn font_display_md() -> f32 { 32.0 }
/// 42.0 — prominent display hero (dual-KPI or large widget focal number).
#[inline] pub fn font_display_lg() -> f32 { 42.0 }
/// 56.0 — maximum display focal number (full-width banner widget).
#[inline] pub fn font_display_xl() -> f32 { 56.0 }

pub const FONT_DISPLAY_SM: f32 = 28.0;
pub const FONT_DISPLAY_MD: f32 = 32.0;
pub const FONT_DISPLAY_LG: f32 = 42.0;
pub const FONT_DISPLAY_XL: f32 = 56.0;

// ─── Monospace helpers (JetBrains Mono, pinned) ───────────────────────────────
// Use these for tabular financial data: prices, quantities, OCC tickers.
// Returns FontId so the family is explicit at the call site.
#[inline] pub fn mono_4xs() -> egui::FontId { egui::FontId::new(font_4xs(), egui::FontFamily::Monospace) }
#[inline] pub fn mono_3xs() -> egui::FontId { egui::FontId::new(font_3xs(), egui::FontFamily::Monospace) }
#[inline] pub fn mono_2xs() -> egui::FontId { egui::FontId::new(font_2xs(), egui::FontFamily::Monospace) }
#[inline] pub fn mono_xs()  -> egui::FontId { egui::FontId::new(font_xs(),  egui::FontFamily::Monospace) }
#[inline] pub fn mono_xs_plus() -> egui::FontId { egui::FontId::new(font_xs_plus(), egui::FontFamily::Monospace) }
#[inline] pub fn mono_sm()  -> egui::FontId { egui::FontId::new(font_sm(),  egui::FontFamily::Monospace) }
#[inline] pub fn mono_md()  -> egui::FontId { egui::FontId::new(font_md(),  egui::FontFamily::Monospace) }
#[inline] pub fn mono_md_plus() -> egui::FontId { egui::FontId::new(font_md_plus(), egui::FontFamily::Monospace) }
#[inline] pub fn mono_lg()  -> egui::FontId { egui::FontId::new(font_lg(),  egui::FontFamily::Monospace) }

// ─── Legacy aliases (DEPRECATED) ──────────────────────────────────────────────
// Kept compiling existing call sites; new code must use the named tier above.
#[doc(hidden)] pub fn font_sm_tight() -> f32 { font_xs() }
#[doc(hidden)] pub fn font_2xl()      -> f32 { font_lg() }

// Const aliases — kept so any const-context call sites compile. Values match
// the active scale (4xs=6, 3xs=7, 2xs=8, xs=9, xs+=10, sm=11, md=13, md+=14, lg=16, xl=22).
pub const FONT_4XS:     f32 = 6.0;
pub const FONT_3XS:     f32 = 7.0;
pub const FONT_2XS:     f32 = 8.0;
pub const FONT_XS:      f32 = 9.0;
pub const FONT_XS_PLUS: f32 = 10.0;
pub const FONT_SM:      f32 = 11.0;
pub const FONT_MD:      f32 = 13.0;
pub const FONT_MD_PLUS: f32 = 14.0;
pub const FONT_LG:      f32 = 16.0;
pub const FONT_XL:      f32 = 22.0;
pub const FONT_2XL:     f32 = 22.0;

// ─── Spacing tokens ───────────────────────────────────────────────────────────
// Spacing scale — density-first chrome, anchored on a ~4px grid.
//
// Tier summary (reach for these in order of specificity):
//   gap_2xs()     =  2.0  — icon-internal padding, [icon][badge] overlays
//   gap_xs()      =  4.0  — minimum intra-cluster gap (adjacent buttons)
//   gap_xs_mid()  =  6.0  — micro-gap between xs and sm; for icon-label
//                           pairs and compact chip rows where 4px is too
//                           tight and 8px is too loose (DS-IMPL-3)
//   gap_sm()      =  8.0  — default inter-element gap
//   gap_md()      = 12.0  — section padding, list-row vertical
//   gap_lg()      = 16.0  — panel inner margin
//   gap_xl()      = 20.0  — between sections in a panel
//   gap_2xl()     = 24.0  — between distinct panel groups
//   gap_3xl()     = 32.0  — page-level breaks (rare)
//
// If you are unsure, prefer gap_sm (8). Reach for gap_xs_mid only when you
// have a specific tight composition that needs exactly 6px.
//
// 2026-05: gap_2xs was previously aliased to gap_xs (both 4.0). It is now a
// real 2.0 token for icon-internal padding and tightly-packed compositions
// (the gap inside `[icon][badge]` overlays, etc.).
// 2026-05 DS-IMPL-3: gap_xs_mid (6.0) added as a micro-gap tier.
pub fn gap_2xs() -> f32 { 2.0 }
pub fn gap_xs()  -> f32 { 4.0 }
/// 6.0 — micro-gap tier between `gap_xs` (4.0) and `gap_sm` (8.0).
/// Use for icon-label pairs and compact chip rows. Backed by
/// `spacing.xs_mid` design token (DS-IMPL-3).
pub fn gap_xs_mid() -> f32 { FRAME_TOKENS.with(|c| c.get().gap_xs_mid) }
pub fn gap_sm()  -> f32 { 8.0 }
pub fn gap_md()  -> f32 { 12.0 }
pub fn gap_lg()  -> f32 { 16.0 }
pub fn gap_xl()  -> f32 { 20.0 }
pub fn gap_2xl() -> f32 { 24.0 }
pub fn gap_3xl() -> f32 { 32.0 }

pub const GAP_2XS:    f32 =  2.0;
pub const GAP_XS:     f32 =  4.0;
/// Compile-time fallback for `gap_xs_mid()`. Prefer the function when
/// a design-token override is needed at runtime.
pub const GAP_XS_MID: f32 =  6.0;
pub const GAP_SM:     f32 =  8.0;
pub const GAP_MD:  f32 = 12.0;
pub const GAP_LG:  f32 = 16.0;
pub const GAP_XL:  f32 = 20.0;
pub const GAP_2XL: f32 = 24.0;
pub const GAP_3XL: f32 = 32.0;

// ─── Icon control sizes ──────────────────────────────────────────────────────
// Standard square sizes for icon-only controls (toggle pills, trailing buttons,
// inline icon-only buttons). Replaces hand-rolled vec2(14, 14) / vec2(16, 16) etc.
#[inline] pub fn icon_xs() -> f32 { 14.0 }
#[inline] pub fn icon_sm() -> f32 { 16.0 }
#[inline] pub fn icon_md() -> f32 { 18.0 }
#[inline] pub fn icon_lg() -> f32 { 20.0 }

// ─── Row heights ─────────────────────────────────────────────────────────────
// Canonical list/table row heights. PanelListRow defaults to row_height_default
// (22) for dense lists and row_height_spacious (24) for breathable ones.
#[inline] pub fn row_height_dense()     -> f32 { 18.0 }
#[inline] pub fn row_height_compact()   -> f32 { 20.0 }
#[inline] pub fn row_height_default()   -> f32 { 22.0 }
#[inline] pub fn row_height_spacious()  -> f32 { 24.0 }
#[inline] pub fn row_height_tall()      -> f32 { 30.0 }

// ─── Card padding ────────────────────────────────────────────────────────────
// Symmetric inner_margin presets for PanelCard / hand-rolled card bodies.
#[inline] pub fn card_padding_compact()  -> f32 { 8.0 }
#[inline] pub fn card_padding_default()  -> f32 { 12.0 }
#[inline] pub fn card_padding_spacious() -> f32 { 16.0 }

// ─── Divider insets ──────────────────────────────────────────────────────────
// Vertical inset for hairline dividers (typically applied to top + bottom
// of the dividing line so the rule doesn't kiss adjacent content).
#[inline] pub fn divider_inset_xs() -> f32 { 1.0 }
#[inline] pub fn divider_inset_sm() -> f32 { 2.0 }
#[inline] pub fn divider_inset_md() -> f32 { 3.0 }
#[inline] pub fn divider_inset_lg() -> f32 { 5.0 }

// ─── Corner radius tokens ─────────────────────────────────────────────────────
// 2026-05: function fallbacks reconciled with the const values (was 3/4/8).
pub fn radius_xs() -> f32 { FRAME_TOKENS.with(|c| c.get().radius_xs) }
pub fn radius_sm() -> f32 { FRAME_TOKENS.with(|c| c.get().radius_sm) }
pub fn radius_md() -> f32 { FRAME_TOKENS.with(|c| c.get().radius_md) }
pub fn radius_lg() -> f32 { FRAME_TOKENS.with(|c| c.get().radius_lg) }
/// Pill (full-rounded). For toggle pills, status badges, etc.
pub fn radius_pill() -> f32 { 999.0 }

pub const RADIUS_XS: f32 = 2.0;
pub const RADIUS_SM: f32 = 4.0;
pub const RADIUS_MD: f32 = 6.0;
pub const RADIUS_LG: f32 = 12.0;
pub const RADIUS_PILL: f32 = 999.0;

// ─── Cursor tokens ───────────────────────────────────────────────────────────
//
// Centralized cursor policy. Every interactive surface in the app should
// route its cursor through one of these helpers. They:
//   1. Check `resp.hovered()` — no cursor leak when the pointer moves away.
//   2. Honor `is_inspect_mode()` — the design inspector owns the cursor
//      while it's active, so widgets must not override it.
//   3. Implement state machines where relevant (draggable: Grab on hover,
//      Grabbing while a drag is in progress).
//
// Add a new helper here rather than inlining `set_cursor_icon` at a call
// site. Inlined sites drift: they forget the inspect guard, they don't
// handle drag-state transitions, and they make role audits impossible.
pub mod cursor {
    use egui::{CursorIcon, Response, Ui};

    #[inline]
    fn inspect_mode() -> bool {
        crate::design_tokens::is_inspect_mode()
    }

    /// PointingHand on hover — for any surface that responds to click
    /// (buttons, chips, links, list rows, menu items, status indicators).
    #[inline]
    pub fn clickable(ui: &Ui, resp: &Response) {
        if resp.hovered() && !inspect_mode() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
    }

    /// Grab on hover, Grabbing while the user is actively dragging.
    /// For chart pan, drawing handles, draggable lines, tab reorder.
    #[inline]
    pub fn draggable(ui: &Ui, resp: &Response) {
        if inspect_mode() { return; }
        if resp.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        } else if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }
    }

    /// Horizontal resize cursor — vertical dividers between panes/columns.
    /// Stays sticky during a drag so the cursor doesn't flicker back to the
    /// default arrow when the pointer briefly leaves the narrow drag rect.
    #[inline]
    pub fn resize_h(ui: &Ui, resp: &Response) {
        if inspect_mode() { return; }
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
    }

    /// Vertical resize cursor — horizontal dividers between rows/panes.
    /// Stays sticky during a drag.
    #[inline]
    pub fn resize_v(ui: &Ui, resp: &Response) {
        if inspect_mode() { return; }
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
        }
    }

    /// Diagonal NW–SE resize — corner grabbers on resizable panels.
    #[inline]
    pub fn resize_nwse(ui: &Ui, resp: &Response) {
        if resp.hovered() && !inspect_mode() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe);
        }
    }

    /// Text I-beam — text input fields, editable cells.
    #[inline]
    pub fn text_input(ui: &Ui, resp: &Response) {
        if resp.hovered() && !inspect_mode() {
            ui.ctx().set_cursor_icon(CursorIcon::Text);
        }
    }

    /// Crosshair — chart measurement tool, precision picking.
    #[inline]
    pub fn crosshair(ui: &Ui, resp: &Response) {
        if resp.hovered() && !inspect_mode() {
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        }
    }

    /// ZoomIn — zoom-box tool while idle (Grabbing takes over once the
    /// drag starts, paint that one yourself with `set_cursor_icon` since
    /// the zoom drag is modal and doesn't go through Response).
    #[inline]
    pub fn zoom_in(ui: &Ui, resp: &Response) {
        if resp.hovered() && !inspect_mode() {
            ui.ctx().set_cursor_icon(CursorIcon::ZoomIn);
        }
    }

    /// Modal cursor: set whenever a tool mode is active (measure, zoom,
    /// crosshair, drawing). Doesn't read from a Response — caller already
    /// knows the mode is on. Still honors inspect mode so the design
    /// inspector retains control.
    #[inline]
    pub fn modal(ui: &Ui, icon: CursorIcon) {
        if !inspect_mode() {
            ui.ctx().set_cursor_icon(icon);
        }
    }

    /// `ui.add(widget)` + `clickable(...)` in one call. Use this anywhere
    /// you're about to write `ui.add(egui::Button::new(...))` — the
    /// resulting `Response` already has PointingHand wired up on hover.
    /// egui 0.31 has no global "interactive cursor" style, so call sites
    /// either route through this helper, `ui_kit::Button` (which sets
    /// PointingHand internally), or set the cursor manually.
    #[inline]
    pub fn click_widget<W: egui::Widget>(ui: &mut Ui, widget: W) -> Response {
        let r = ui.add(widget);
        clickable(ui, &r);
        r
    }

    /// Paint a keyboard-focus ring around `resp.rect` when the widget has
    /// focus. No-op when the widget is not focused, so callers add this
    /// unconditionally after building their response.
    ///
    /// The ring is painted *outside* the widget's rect (2 px expansion) so it
    /// doesn't clip or overlap inner chrome. Accent color at `focus_ring_alpha`
    /// opacity keeps it visible but subtle — consistent with the design system's
    /// focus treatment across Button, Input, and Select.
    ///
    /// `accent` — the accent Color32 for this widget (from its theme). Pull
    /// it with `theme.accent()` (ComponentTheme) or `t.accent` (Theme).
    #[inline]
    pub fn focus_ring(ui: &Ui, resp: &Response, accent: egui::Color32) {
        use egui::{CornerRadius, Stroke, StrokeKind};
        if !resp.has_focus() {
            return;
        }
        let st = super::current();
        let color = super::color_alpha(accent, st.focus_ring_alpha);
        let radius = CornerRadius::same((super::radius_sm() as u8).saturating_add(1));
        ui.painter().rect_stroke(
            resp.rect.expand(2.0),
            radius,
            Stroke::new(st.focus_ring_width, color),
            StrokeKind::Outside,
        );
    }
}

// ─── Stroke width tokens ─────────────────────────────────────────────────────
// Stroke scale — sub-pixel to multi-pixel hairlines for borders and rules.
//
// Tier summary:
//   stroke_hair()   = 0.3  — sub-pixel separator; nearly invisible hairline
//   stroke_thin()   = 0.5  — light border, table column divider
//   stroke_medium() = 0.8  — mid-weight border; between thin and std
//                            (DS-IMPL-3: now backed by `stroke.medium` token)
//   stroke_std()    = 1.0  — default UI border (buttons, inputs, panels)
//   stroke_bold()   = 1.5  — emphasis border (active selection, focus ring)
//   stroke_thick()  = 2.0  — strong visual separator
//   stroke_heavy()  = 3.0  — decorative / accent rule
//
// Use `stroke_medium()` when `stroke_thin()` feels too ghost-like and
// `stroke_std()` is heavier than desired for the context.
pub fn stroke_hair()        -> f32 { FRAME_TOKENS.with(|c| c.get().stroke_hair) }
pub fn stroke_thin()        -> f32 { FRAME_TOKENS.with(|c| c.get().stroke_thin) }
/// 0.8 — mid-weight border tier between `stroke_thin` (0.5) and
/// `stroke_std` (1.0). Backed by `stroke.medium` design token (DS-IMPL-3).
pub fn stroke_medium()      -> f32 { FRAME_TOKENS.with(|c| c.get().stroke_medium) }
pub fn stroke_std()         -> f32 { FRAME_TOKENS.with(|c| c.get().stroke_std) }
pub fn stroke_bold()        -> f32 { FRAME_TOKENS.with(|c| c.get().stroke_bold) }
pub fn stroke_thick()       -> f32 { FRAME_TOKENS.with(|c| c.get().stroke_thick) }
pub fn stroke_extra_thick() -> f32 { 2.5 }
pub fn stroke_heavy()       -> f32 { 3.0 }

pub const STROKE_HAIR:        f32 = 0.3;
pub const STROKE_THIN:        f32 = 0.5;
pub const STROKE_MEDIUM:      f32 = 0.8;
pub const STROKE_STD:         f32 = 1.0;
pub const STROKE_BOLD:        f32 = 1.5;
pub const STROKE_THICK:       f32 = 2.0;
pub const STROKE_EXTRA_THICK: f32 = 2.5;
pub const STROKE_HEAVY:       f32 = 3.0;

// ─── Semantic alpha tokens ────────────────────────────────────────────────────
// 2026-05: tier expanded with intermediate values to absorb hardcoded literals
// (25, 30, 140, 180, 230). Existing tiers (faint=10, subtle=40, muted=60,
// line=80, active=100, heavy=120, solid=200) keep their values so visuals
// don't shift. Note: `alpha_muted == alpha_dim` (both 60) and
// `alpha_line == alpha_strong` (both 80) by design — same value, different
// semantic intent (muted/strong = chrome; dim/line = borders).
pub fn alpha_faint()       -> u8 { FRAME_TOKENS.with(|c| c.get().alpha_faint) }
pub fn alpha_ghost()       -> u8 { FRAME_TOKENS.with(|c| c.get().alpha_ghost) }
pub fn alpha_soft()        -> u8 { FRAME_TOKENS.with(|c| c.get().alpha_soft) }
pub fn alpha_whisper()     -> u8 { 25 }
pub fn alpha_hint()        -> u8 { 30 }
pub fn alpha_subtle()      -> u8 { FRAME_TOKENS.with(|c| c.get().alpha_subtle) }
pub fn alpha_tint()        -> u8 { FRAME_TOKENS.with(|c| c.get().alpha_tint) }
pub fn alpha_muted()       -> u8 { FRAME_TOKENS.with(|c| c.get().alpha_muted) }
pub fn alpha_dim()         -> u8 { FRAME_TOKENS.with(|c| c.get().alpha_dim) }
pub fn alpha_line()        -> u8 { FRAME_TOKENS.with(|c| c.get().alpha_line) }
pub fn alpha_strong()      -> u8 { FRAME_TOKENS.with(|c| c.get().alpha_strong) }
pub fn alpha_active()      -> u8 { FRAME_TOKENS.with(|c| c.get().alpha_active) }
pub fn alpha_heavy()       -> u8 { FRAME_TOKENS.with(|c| c.get().alpha_heavy) }
pub fn alpha_intense()     -> u8 { 140 }
pub fn alpha_prominent()   -> u8 { 180 }
pub fn alpha_solid()       -> u8 { FRAME_TOKENS.with(|c| c.get().alpha_solid) }
pub fn alpha_near_opaque() -> u8 { 230 }

/// Use with `color_alpha(color, ALPHA_*)` for consistent opacity tiers.
pub const ALPHA_FAINT:       u8 = 10;
pub const ALPHA_GHOST:       u8 = 15;
pub const ALPHA_SOFT:        u8 = 20;
pub const ALPHA_WHISPER:     u8 = 25;
pub const ALPHA_HINT:        u8 = 30;
pub const ALPHA_SUBTLE:      u8 = 40;
pub const ALPHA_TINT:        u8 = 48;
pub const ALPHA_MUTED:       u8 = 60;
pub const ALPHA_DIM:         u8 = 60;
pub const ALPHA_LINE:        u8 = 80;
pub const ALPHA_STRONG:      u8 = 80;
pub const ALPHA_ACTIVE:      u8 = 100;
pub const ALPHA_HEAVY:       u8 = 120;
pub const ALPHA_INTENSE:     u8 = 140;
pub const ALPHA_PROMINENT:   u8 = 180;
pub const ALPHA_SOLID:       u8 = 200;
pub const ALPHA_NEAR_OPAQUE: u8 = 230;

// ─── Drop shadow tokens ───────────────────────────────────────────────────────
pub fn shadow_offset() -> f32 { FRAME_TOKENS.with(|c| c.get().shadow_offset) }
pub fn shadow_alpha()  -> u8  { FRAME_TOKENS.with(|c| c.get().shadow_alpha) }
pub fn shadow_spread() -> f32 { FRAME_TOKENS.with(|c| c.get().shadow_spread) }

pub const SHADOW_OFFSET: f32 = 2.0;
pub const SHADOW_ALPHA:  u8  = 60;
pub const SHADOW_SPREAD: f32 = 4.0;

// ─── Shadow preset accessors (semantic depth scale) ──────────────────────────
// Returns egui::epaint::Shadow ready for `Frame::shadow(...)`. Backed by the
// `shadow_preset` design-token sub-struct when design-mode is on, else hard-
// coded defaults that match the original inline values they replace.
//
// LEGACY: `shadow_card / _modal / _tooltip / _dropdown` (no theme arg) use
// black as the shadow color — fine on dark themes, but on light themes
// (Bauhaus, Peach, Ivory, Newsprint) they paint as a hard black smudge.
// Prefer the `_themed` variants — they pull `t.shadow_color` so light
// themes get a soft gray drop. The legacy variants are kept compiling for
// the ~30 call sites that don't currently have a theme handle.
#[inline]
fn shadow_from_preset(offset: [i8; 2], blur: u8, spread: u8, alpha: u8) -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset, blur, spread,
        color: Color32::from_black_alpha(alpha),
    }
}

#[inline]
fn shadow_from_preset_themed(
    t: &super::super::gpu::Theme,
    offset: [i8; 2], blur: u8, spread: u8, alpha: u8,
) -> egui::epaint::Shadow {
    let s = t.shadow_color;
    egui::epaint::Shadow {
        offset, blur, spread,
        color: Color32::from_rgba_unmultiplied(s.r(), s.g(), s.b(), alpha),
    }
}

/// Card / panel — subtle resting lift. Defaults: offset (0,2), blur 4, alpha 60.
pub fn shadow_card() -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(t) = crate::design_tokens::get() {
        let p = t.shadow_preset.card;
        return shadow_from_preset(p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset([0, 2], 4, 0, 60)
}

/// Card / panel — theme-aware. Use this in new code.
pub fn shadow_card_themed(t: &super::super::gpu::Theme) -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(dt) = crate::design_tokens::get() {
        let p = dt.shadow_preset.card;
        return shadow_from_preset_themed(t, p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset_themed(t, [0, 2], 4, 0, 60)
}

/// Modal dialog — tall, soft. Defaults: offset (0,8), blur 28, spread 2, alpha 80.
pub fn shadow_modal() -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(t) = crate::design_tokens::get() {
        let p = t.shadow_preset.modal;
        return shadow_from_preset(p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset([0, 8], 28, 2, 80)
}

/// Modal dialog — theme-aware. Use this in new code.
pub fn shadow_modal_themed(t: &super::super::gpu::Theme) -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(dt) = crate::design_tokens::get() {
        let p = dt.shadow_preset.modal;
        return shadow_from_preset_themed(t, p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset_themed(t, [0, 8], 28, 2, 80)
}

/// Tooltip — small, crisp. Used for hover bubbles.
pub fn shadow_tooltip() -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(t) = crate::design_tokens::get() {
        let p = t.shadow_preset.tooltip;
        return shadow_from_preset(p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset([0, 2], 0, 0, 60)
}

/// Tooltip — theme-aware. Use this in new code.
pub fn shadow_tooltip_themed(t: &super::super::gpu::Theme) -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(dt) = crate::design_tokens::get() {
        let p = dt.shadow_preset.tooltip;
        return shadow_from_preset_themed(t, p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset_themed(t, [0, 2], 0, 0, 60)
}

/// Dropdown / popover. Defaults: offset (0,8), blur 24, spread 1, alpha 40.
pub fn shadow_dropdown() -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(t) = crate::design_tokens::get() {
        let p = t.shadow_preset.dropdown;
        return shadow_from_preset(p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset([0, 8], 24, 1, 40)
}

/// Dropdown / popover — theme-aware. Use this in new code.
pub fn shadow_dropdown_themed(t: &super::super::gpu::Theme) -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(dt) = crate::design_tokens::get() {
        let p = dt.shadow_preset.dropdown;
        return shadow_from_preset_themed(t, p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset_themed(t, [0, 8], 24, 1, 40)
}

// ─── Elevation tints ─────────────────────────────────────────────────────────
// Surface elevation scale — three tints for layered dark-UI backgrounds.
//
// Elevation tints slightly brighten `theme.bg` to communicate Z-depth.
// Three levels are intentionally perceptual constants chosen for dark themes:
//
//   elevation_1() = bg × 0.95  — resting card / panel surface (subtle lift)
//   elevation_2() = bg × 0.88  — raised panel, popover body, inline editor
//   elevation_3() = bg × 0.85  — modal / dialog surface (highest layer)
//
// The gamma multipliers are DARK-THEME perceptual constants. Light themes
// would need a different strategy (additive tint or a separate lookup) because
// gamma_multiply(< 1.0) darkens on light backgrounds, inverting the intended
// depth cue.
//
// TODO: When light-theme elevation support is added, split on `theme.is_light()`
// (or equivalent flag) and use `gamma_multiply(1.05 | 1.10 | 1.15)` for light
// surfaces so the depth direction stays consistent across all 15 themes.
//
// These do NOT use design tokens — the gamma values are perceptual constants,
// not tweakable style decisions. If you find yourself wanting to override them,
// consider whether a dedicated surface color token would be more appropriate.

// Elevation gamma factors — the single source of truth for the perceptual
// depth ramp applied to `theme.bg`. DARK-THEME constants (see the light-theme
// TODO above). Phase B3 promotes these to `StyleSystem.elevation` so a style
// system can override the ramp; until then a `const` is the correct home for
// a perceptual constant (vs the magic literal repeated across call sites).
pub const ELEVATION_1_FACTOR: f32 = 0.95;
pub const ELEVATION_2_FACTOR: f32 = 0.88;
pub const ELEVATION_3_FACTOR: f32 = 0.85;

/// Elevation 1 — resting card / panel surface. Subtle lift above the base bg.
/// `theme.bg` darkened/lightened by gamma × 0.95 for dark themes.
#[inline]
pub fn elevation_1(theme: &super::super::gpu::Theme) -> Color32 {
    theme.bg.gamma_multiply(ELEVATION_1_FACTOR)
}

/// Elevation 2 — raised panel, popover body, inline editor surface.
/// `theme.bg` × 0.88 for dark themes.
#[inline]
pub fn elevation_2(theme: &super::super::gpu::Theme) -> Color32 {
    theme.bg.gamma_multiply(ELEVATION_2_FACTOR)
}

/// Elevation 3 — modal / dialog surface (highest Z-layer).
/// `theme.bg` × 0.85 for dark themes.
#[inline]
pub fn elevation_3(theme: &super::super::gpu::Theme) -> Color32 {
    theme.bg.gamma_multiply(ELEVATION_3_FACTOR)
}

// ─── Semantic color accessors ────────────────────────────────────────────────
// Intent colors that don't fit the brand palette: hover/focus/disabled, order
// status (cancel pastel), sentiment, and third-party brand colors.
/// Soft white overlay for hover-tint on dark surfaces.
pub fn hover_tint() -> Color32 {
    crate::dt_rgba!(semantic.hover_tint, [255, 255, 255, 16])
}
/// Focus ring — keyboard-focus halo color.
pub fn focus_ring() -> Color32 {
    crate::dt_rgba!(semantic.focus_ring, [100, 200, 255, 200])
}
/// Foreground color for disabled controls.
pub fn disabled_fg() -> Color32 {
    crate::dt_rgba!(semantic.disabled_fg, [140, 140, 150, 160])
}
/// Background color for disabled controls.
pub fn disabled_bg() -> Color32 {
    crate::dt_rgba!(semantic.disabled_bg, [40, 40, 46, 200])
}
/// Order cancel button background — pastel red.
pub fn order_cancel_bg() -> Color32 {
    crate::dt_rgba!(semantic.order_cancel_bg, [232, 156, 156, 255])
}
/// Order cancel button foreground — dark red on pastel.
pub fn order_cancel_fg() -> Color32 {
    crate::dt_rgba!(semantic.order_cancel_fg, [70, 25, 25, 255])
}
/// Sentiment — positive (bullish, good news, upvote).
pub fn sentiment_positive() -> Color32 {
    crate::dt_rgba!(semantic.sentiment_positive, [80, 200, 120, 255])
}
/// Sentiment — neutral (informational).
pub fn sentiment_neutral() -> Color32 {
    crate::dt_rgba!(semantic.sentiment_neutral, [180, 180, 195, 255])
}
/// Chat author-distinction palette — returns the i-th color, modulo'd by the
/// palette length so callers can pass a raw author-name hash directly. Used
/// by the Discord chat panel to give each speaker a stable, distinct color.
///
/// Note: `crate::design_tokens::get()` returns a cloned snapshot, so we only
/// expose an indexed accessor (no `&'static` slice variant).
pub fn chat_author_color(idx: usize) -> Color32 {
    // Hard-coded fallback mirrors ChatAuthorPalette::default() — must stay in sync.
    const FALLBACK: [[u8; 4]; 8] = [
        [ 74, 158, 255, 255], [ 46, 204, 113, 255], [243, 156,  18, 255], [155,  89, 182, 255],
        [231,  76,  60, 255], [ 26, 188, 156, 255], [241, 196,  15, 255], [ 52, 152, 219, 255],
    ];
    #[cfg(feature = "design-mode")]
    if let Some(t) = crate::design_tokens::get() {
        let pal = &t.chat_author_palette.colors;
        let c = pal[idx % pal.len()];
        return Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
    }
    let c = FALLBACK[idx % FALLBACK.len()];
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// Sentiment — negative (bearish, bad news, downvote).
pub fn sentiment_negative() -> Color32 {
    crate::dt_rgba!(semantic.sentiment_negative, [224, 85, 96, 255])
}
/// Discord brand color — "Blurple" (#5865F2).
pub fn discord_blurple() -> Color32 {
    crate::dt_rgba!(semantic.discord_blurple, [88, 101, 242, 255])
}
/// Order ledger badge — RECON state (lavender/purple).
pub fn order_state_recon() -> Color32 {
    crate::dt_rgba!(semantic.order_state_recon, [167, 139, 250, 255])
}
/// Order ledger badge — CTRL state (red).
pub fn order_state_ctrl() -> Color32 {
    crate::dt_rgba!(semantic.order_state_ctrl, [255, 100, 100, 255])
}

// ─── Fixed text colors (fallback for code without Theme access) ──────────────
// Prefer `t.text` when Theme is in scope — these are dark-theme defaults.
pub static TEXT_PRIMARY: Color32 = Color32::from_rgb(220, 220, 230);
pub static TEXT_SECONDARY: Color32 = Color32::from_rgb(200, 200, 210);

// ─── Status color tokens ─────────────────────────────────────────────────────
/// Green — active / live / filled (status_ok).
pub fn status_ok()    -> Color32 { crate::dt_rgba!(status.ok,    [120, 180, 120, 255]) }
/// Orange — warning / pending (status_warn).
pub fn status_warn()  -> Color32 { crate::dt_rgba!(status.warn,  [255, 165,   0, 255]) }
/// Red — error / rejected (status_error).
pub fn status_error() -> Color32 { crate::dt_rgba!(status.error, [224,  85,  96, 255]) }
/// Blue/purple — informational (status_info).
pub fn status_info()  -> Color32 { crate::dt_rgba!(status.info,  [100, 200, 255, 255]) }

// ─── Drawing palette tokens ──────────────────────────────────────────────────
/// Four link-group identity colors: blue, green, orange, purple.
pub fn drawing_palette() -> [Color32; 4] {
    #[cfg(feature = "design-mode")]
    if let Some(t) = crate::design_tokens::get() {
        let p = t.drawing.palette;
        return p.map(|[r, g, b, a]| Color32::from_rgba_unmultiplied(r, g, b, a));
    }
    [
        Color32::from_rgb( 70, 130, 255),
        Color32::from_rgb( 80, 200, 120),
        Color32::from_rgb(255, 160,  60),
        Color32::from_rgb(180, 100, 255),
    ]
}

// ─── Semantic accent colors (design-system tokens) ───────────────────────────
/// Amber — used for "Active" status, R:R ≥ 1 indicator, and warning states.
pub const COLOR_AMBER: Color32 = Color32::from_rgb(255, 191, 0);
/// Teal — T2 target label color (second exit level).
pub const COLOR_T2: Color32 = Color32::from_rgb(26, 188, 156);
/// Blue — T3 target label color (third exit level).
pub const COLOR_T3: Color32 = Color32::from_rgb(52, 152, 219);
/// Cyan/blue informational color — used for "info" status, link-like
/// non-accent emphasis. RGB: (74,158,255). Theme-invariant; if you need a
/// theme-following blue, use `t.accent` instead.
pub const COLOR_INFO_CYAN:    Color32 = Color32::from_rgb( 74, 158, 255);
/// Pastel green — sentiment/profit signal independent of theme bull color.
/// Use sparingly; prefer `t.bull` for trade direction.
pub const COLOR_PROFIT_GREEN: Color32 = Color32::from_rgb( 46, 204, 113);
/// Pastel red — sentiment/loss signal independent of theme bear color.
pub const COLOR_LOSS_RED:     Color32 = Color32::from_rgb(231,  76,  60);
/// Purple accent for special-tier indicators / category-specific tints.
pub const COLOR_PURPLE:       Color32 = Color32::from_rgb(180, 100, 255);

// ─── Raw text helpers ─────────────────────────────────────────────────────────

#[inline]
pub fn mono(text: &str, size: f32, color: Color32) -> RichText {
    RichText::new(text).monospace().size(size).color(color)
}

#[inline]
pub fn mono_bold(text: &str, size: f32, color: Color32) -> RichText {
    RichText::new(text).monospace().size(size).strong().color(color)
}

// ─── Toolbar button ───────────────────────────────────────────────────────────

/// Toolbar button — FONT_LG, RADIUS_MD, themed, pointer cursor.
/// Active state: accent fill + accent border + soft glow halo + bottom underline.
/// Hover state: subtle bg tint + accent border.
/// True when every char in `s` is in a Phosphor / icon private-use codepoint
/// range. Lets us detect "icon-only" toolbar buttons so we can render their
/// glyphs ~50% larger than text labels without breaking text-button sizing.
fn label_is_icon_only(s: &str) -> bool {
    if s.is_empty() { return false; }
    s.chars().all(|c| {
        let cp = c as u32;
        // Private Use Area (U+E000–U+F8FF) — where Phosphor glyphs live.
        // Allow ASCII whitespace as a separator (e.g., "{ICON} {count}").
        (0xE000..=0xF8FF).contains(&cp)
            || (0xF0000..=0x10FFFF).contains(&cp)
            || c.is_ascii_whitespace()
            || c.is_ascii_digit()
    })
}

#[deprecated(note = "use `ui_kit::Button::toolbar(label).active(b).show(ui, theme)` (which is exactly what the `toolbar_btn(ui, label, active, t)` helper in components/toolbar/mod.rs already does)")]
pub fn tb_btn(ui: &mut egui::Ui, label: &str, active: bool, accent: Color32, dim: Color32, toolbar_bg: Color32, toolbar_border: Color32) -> egui::Response {
    let st = current();
    // Apply uppercase transform per active style (#5).
    let raw_label = style_label_case(label);
    // Icon-only buttons render at font_md (13) so the glyph stays visually
    // weighted next to font_sm (11) text labels — same proportional bump as
    // body→title text. Older code used font_md*1.5 which produced ~19.5px
    // icons that towered over 11px nav text and felt off-scale.
    let label_size = if label_is_icon_only(label) { font_md() } else { font_sm() };
    // Apply nav letter-spacing approximation via thin-spaces (U+2009).
    let nav_sp = st.nav_letter_spacing_px;
    let display_label = if nav_sp < 0.5 {
        raw_label
    } else {
        let sep = if nav_sp > 1.5 { "\u{2009}\u{2009}" } else { "\u{2009}" };
        raw_label.chars().map(|c| c.to_string()).collect::<Vec<_>>().join(sep)
    };
    let corner_r = st.r_sm as f32;

    // Derive active fill/text from the invert-active discriminant (§3.2).
    // invert_active_fill → palette inversion: fill=theme.text, text=theme.bg.
    let theme = crate::ui_kit::widgets::theme::active_theme(ui.ctx());
    let active_fill = if st.invert_active_fill { theme.text } else { color_alpha(accent, alpha_tint()) };
    let active_text = if st.invert_active_fill { theme.bg } else { accent };

    // Button treatment dispatch (#18).
    let (bg, fg, border) = match st.button_treatment {
        ButtonTreatment::UnderlineActive => {
            // Transparent idle; active uses derived fill/text.
            let fg = if active { active_text } else { dim };
            (Color32::TRANSPARENT, fg, Color32::TRANSPARENT)
        }
        _ => {
            let bg = if active {
                if st.invert_active_fill { active_fill } else { color_alpha(accent, alpha_tint()) }
            } else { color_alpha(toolbar_border, alpha_ghost()) };
            let fg = if active { active_text } else { dim };
            let border = if active { color_alpha(accent, alpha_active()) } else { color_alpha(toolbar_border, alpha_muted()) };
            (bg, fg, border)
        }
    };

    // For UnderlineActive (Meridien), paint the column tint BEFORE the button
    // via the Background layer so the button's text/fill renders on top.
    if matches!(st.button_treatment, ButtonTreatment::UnderlineActive) {
        // We need the button rect first. Allocate exact size, paint bg, then add button inside.
        let btn_width = {
            let galley = ui.fonts(|f| f.layout_no_wrap(
                display_label.clone(),
                egui::FontId::monospace(label_size),
                Color32::WHITE, // layout-only: color discarded, only width is read below
            ));
            galley.rect.width() + 16.0 // approx button padding
        };
        let btn_size = egui::vec2(btn_width.max(0.0), 24.0);
        let (btn_rect, _btn_sense) = ui.allocate_exact_size(btn_size, egui::Sense::hover());
        let tb = toolbar_rect();
        let col_rect = egui::Rect::from_min_max(
            egui::pos2(btn_rect.left(), tb.top()),
            egui::pos2(btn_rect.right(), tb.bottom()),
        );
        // Paint column tint in Background layer so button draws on top.
        // nav_active_col_alpha controls the column tint alpha for the active nav button.
        let bg_painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("tb_btn_col_bg")));
        use crate::chart::renderer::ui::components::motion;
        let active_target = color_alpha(toolbar_border, st.nav_active_col_alpha.max(alpha_ghost()));
        let hover_target  = color_alpha(dim, alpha_ghost());
        let hovered = ui.rect_contains_pointer(btn_rect);
        // Animate active and hover tints independently so they fade in/out smoothly.
        let active_id = ui.id().with(("tb_btn_active", label));
        let hover_id  = ui.id().with(("tb_btn_hover", label));
        let active_t  = motion::ease_bool(ui.ctx(), active_id, active, motion::MED);
        let hover_t   = motion::ease_bool(ui.ctx(), hover_id,  hovered && !active, motion::FAST);
        // Compose: start transparent, lerp in hover, then lerp toward active (active wins).
        let mut col_tint = motion::lerp_color(Color32::TRANSPARENT, hover_target, hover_t);
        col_tint = motion::lerp_color(col_tint, active_target, active_t);
        if col_tint.a() > 0 {
            bg_painter.rect_filled(col_rect, 0.0, col_tint);
        }
        if active_t > 0.001 {
            let ul_thickness = if st.tab_underline_thickness > 0.0 { st.tab_underline_thickness } else { st.stroke_bold };
            let underline_y = tb.bottom() - 1.0;
            let ul_color = motion::fade_in(active_fill, active_t);
            bg_painter.line_segment(
                [egui::pos2(btn_rect.left(), underline_y), egui::pos2(btn_rect.right(), underline_y)],
                Stroke::new(ul_thickness, ul_color));
        }
        // Place the actual button in the already-allocated rect via put().
        let resp = ui.put(btn_rect, egui::Button::new(RichText::new(display_label).monospace().size(label_size).color(fg))
            .wrap_mode(egui::TextWrapMode::Extend)
            .fill(Color32::TRANSPARENT).stroke(Stroke::new(0.0, Color32::TRANSPARENT)).corner_radius(corner_r));
        hit(&resp.rect, "TOOLBAR_BTN", "Toolbar");
        if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let _ = toolbar_bg;
        return resp;
    }

    use crate::chart::renderer::ui::components::motion;
    // Active fill target (what egui would have snapped to when active).
    let active_bg_target = if st.invert_active_fill { active_fill } else { color_alpha(accent, alpha_tint()) };
    let idle_bg = color_alpha(toolbar_border, alpha_ghost());
    // Animate active state for the button bg.
    let active_id = ui.id().with(("tb_btn_std_active", label));
    let active_t = motion::ease_bool(ui.ctx(), active_id, active, motion::MED);
    let animated_bg = motion::lerp_color(idle_bg, active_bg_target, active_t);
    let animated_border = motion::lerp_color(
        color_alpha(toolbar_border, alpha_muted()),
        color_alpha(accent, alpha_active()),
        active_t,
    );
    let _ = bg; let _ = border;

    let resp = ui.add(egui::Button::new(RichText::new(display_label).monospace().size(label_size).color(fg))
        .wrap_mode(egui::TextWrapMode::Extend)
        .fill(animated_bg).stroke(Stroke::new(stroke_thin(), animated_border)).corner_radius(corner_r)
        .min_size(egui::vec2(0.0, row_height_spacious())));
    hit(&resp.rect, "TOOLBAR_BTN", "Toolbar");

    // Hover bevel highlight — animate fade-in/out so it doesn't snap.
    let hover_id = ui.id().with(("tb_btn_std_hover", label));
    let hover_t = motion::ease_bool(ui.ctx(), hover_id, resp.hovered() && !active, motion::FAST);
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if active_t > 0.001 {
        let r = resp.rect;
        // stroke_bold drives the active-state accent underline for non-Meridien styles.
        let ul_color = motion::fade_in(color_alpha(accent, alpha_dim()), active_t);
        ui.painter().line_segment(
            [egui::pos2(r.left() + 4.0, r.bottom() + 0.5),
             egui::pos2(r.right() - 4.0, r.bottom() + 0.5)],
            Stroke::new(st.stroke_bold, ul_color));
    }
    if hover_t > 0.001 && !active {
        let r = resp.rect;
        let bevel = motion::fade_in(Color32::from_rgba_unmultiplied(255, 255, 255, 10), hover_t);
        ui.painter().rect_filled(
            egui::Rect::from_min_max(r.min, egui::pos2(r.right(), r.top() + 1.0)),
            egui::CornerRadius { nw: corner_r as u8, ne: corner_r as u8, sw: 0, se: 0 },
            bevel);
    }
    let _ = toolbar_bg; // may be used for future hover tint
    resp
}

// ─── Dialog / popup windows ───────────────────────────────────────────────────

/// Standard popup window frame — dark background, no title bar.
pub fn popup_frame(ctx: &egui::Context, id: &str, pos: egui::Pos2, width: f32, fill: Color32, border_color: Option<Color32>) -> egui::Window<'static> {
    let mut frame = egui::Frame::popup(&ctx.style()).fill(fill).inner_margin(gap_lg());
    if let Some(bc) = border_color {
        frame = frame.stroke(Stroke::new(stroke_std(), bc));
    }
    egui::Window::new(id.to_string())
        .fixed_pos(pos).fixed_size(egui::vec2(width, 0.0))
        .title_bar(false).frame(frame)
}

/// Application-quality dialog window — zero inner padding, RADIUS_LG corners.
///
/// Fill and border now resolve from the active theme so light themes receive
/// appropriate surface colors instead of the former hardcoded dark values.
pub fn dialog_window(ctx: &egui::Context, id: &str, pos: egui::Pos2, width: f32, border_color: Option<Color32>) -> egui::Window<'static> {
    let t = crate::ui_kit::widgets::theme::active_theme(ctx);
    let fill = t.toolbar_bg;
    let border = border_color.unwrap_or(color_alpha(t.toolbar_border, 80));
    egui::Window::new(id.to_string())
        .fixed_pos(pos).fixed_size(egui::vec2(width, 0.0))
        .title_bar(false)
        .frame(egui::Frame::popup(&ctx.style()).fill(fill).inner_margin(0.0)
            .stroke(Stroke::new(stroke_std(), border)).corner_radius(radius_lg()))
}

/// Theme-aware dialog window — rich shadow when shadows_enabled, flat hairline when not (#16).
pub fn dialog_window_themed(ctx: &egui::Context, id: &str, pos: egui::Pos2, width: f32, toolbar_bg: Color32, toolbar_border: Color32, border_color: Option<Color32>) -> egui::Window<'static> {
    let st = current();
    let t = crate::ui_kit::widgets::theme::active_theme(ctx);
    let border = border_color.unwrap_or(color_alpha(toolbar_border, alpha_strong()));
    let corner_r = if st.r_lg == 0 { 0.0 } else { radius_lg() };
    let shadow = if st.shadows_enabled {
        egui::epaint::Shadow {
            offset: [0, 8],
            blur: 28,
            spread: 2,
            color: shadow_color_alpha(&t, 80),
        }
    } else if st.card_floating_shadow {
        egui::epaint::Shadow {
            offset: [0, 3],
            blur: 8,
            spread: 0,
            color: shadow_color_alpha(&t, st.card_floating_shadow_alpha),
        }
    } else {
        egui::epaint::Shadow::NONE
    };
    egui::Window::new(id.to_string())
        .fixed_pos(pos).fixed_size(egui::vec2(width, 0.0))
        .title_bar(false)
        .frame(egui::Frame::popup(&ctx.style())
            .fill(toolbar_bg)
            .inner_margin(0.0)
            .stroke(Stroke::new(st.stroke_std, border))
            .corner_radius(corner_r)
            .shadow(shadow))
}

/// Dialog header bar — auto-darkened bg, FONT_LG title, X close. Returns true if closed.
pub fn dialog_header(ui: &mut egui::Ui, title: &str, dim: Color32) -> bool {
    dialog_header_colored(ui, title, dim, None)
}

/// Dialog header bar with explicit header background.
pub fn dialog_header_colored(ui: &mut egui::Ui, title: &str, dim: Color32, header_bg: Option<Color32>) -> bool {
    let _ = dim; // previously used to tint the X glyph; now handled by Button::close placement
    let darken = crate::dt_u8!(dialog.header_darken, 8);
    let fill = header_bg.unwrap_or_else(|| {
        let bg = ui.visuals().window_fill();
        Color32::from_rgb(bg.r().saturating_sub(darken), bg.g().saturating_sub(darken), bg.b().saturating_sub(darken))
    });
    let mut closed = false;
    let rlg = current().r_lg;
    egui::Frame::NONE.fill(fill)
        .inner_margin(egui::Margin { left: gap_lg() as i8, right: gap_lg() as i8, top: gap_lg() as i8, bottom: gap_lg() as i8 })
        .corner_radius(egui::CornerRadius { nw: rlg, ne: rlg, sw: 0, se: 0 })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let text_col = ui.style().visuals.override_text_color.unwrap_or(TEXT_PRIMARY);
                ui.label(RichText::new(title).monospace().size(font_lg()).strong().color(text_col));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let t = crate::ui_kit::widgets::theme::active_theme(ui.ctx());
                    if crate::ui_kit::widgets::Button::close().show(ui, &t).clicked() {
                        closed = true;
                    }
                });
            });
        });
    closed
}

// ─── Separators ───────────────────────────────────────────────────────────────

/// Full-width horizontal separator.
/// Uses `stroke_hair` when the active style has hairline_borders, otherwise `stroke_thin` —
/// giving Meridien its characteristic super-thin dividers.
#[inline]
pub fn separator(ui: &mut egui::Ui, color: Color32) {
    let st = current();
    let sw = if st.hairline_borders { st.stroke_hair } else { stroke_thin() };
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [egui::pos2(rect.left(), ui.cursor().min.y), egui::pos2(rect.right(), ui.cursor().min.y)],
        Stroke::new(sw, color));
    ui.add_space(crate::dt_f32!(separator.after_space, 1.0));
}

/// Inset separator with margins on both sides.
/// Uses `stroke_hair` when the active style has hairline_borders, otherwise `stroke_thin`.
pub fn dialog_separator(ui: &mut egui::Ui, margin: f32, color: Color32) {
    let st = current();
    let sw = if st.hairline_borders { st.stroke_hair } else { stroke_thin() };
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [egui::pos2(rect.left() + margin, ui.cursor().min.y),
         egui::pos2(rect.right() - margin, ui.cursor().min.y)],
        Stroke::new(sw, color));
    ui.add_space(crate::dt_f32!(separator.after_space, 1.0));
}

/// Inset separator + soft gradient shadow below (3 fading lines).
/// Uses `stroke_thick` for the main divider line so bold-separator sites are style-driven.
///
/// Resolves the active theme from the UI context so light themes get a
/// soft gray gradient instead of the former hardcoded black.
pub fn dialog_separator_shadow(ui: &mut egui::Ui, margin: f32, color: Color32) {
    let t = crate::ui_kit::widgets::theme::active_theme(ui.ctx());
    dialog_separator_shadow_impl(ui, margin, color, t.shadow_color);
}

/// Inset separator + soft gradient shadow below — theme-aware.
pub fn dialog_separator_shadow_themed(
    ui: &mut egui::Ui,
    margin: f32,
    color: Color32,
    t: &super::super::gpu::Theme,
) {
    dialog_separator_shadow_impl(ui, margin, color, t.shadow_color);
}

#[inline]
fn dialog_separator_shadow_impl(ui: &mut egui::Ui, margin: f32, color: Color32, shadow_tint: Color32) {
    let rect = ui.available_rect_before_wrap();
    let y = ui.cursor().min.y;
    let left = rect.left() + margin;
    let right = rect.right() - margin;
    ui.painter().line_segment([egui::pos2(left, y), egui::pos2(right, y)], Stroke::new(current().stroke_thick, color));
    // Fading shadow gradient: 3 strokes at decreasing alpha (themed tint).
    #[cfg(feature = "design-mode")]
    let shadow_alphas = {
        if let Some(t) = crate::design_tokens::get() { t.shadow.gradient } else { [20u8, 12, 4] }
    };
    #[cfg(not(feature = "design-mode"))]
    let shadow_alphas = [20u8, 12, 4];
    for (i, &a) in shadow_alphas.iter().enumerate() {
        ui.painter().line_segment(
            [egui::pos2(left, y + (i + 1) as f32), egui::pos2(right, y + (i + 1) as f32)],
            Stroke::new(
                stroke_thin(),
                Color32::from_rgba_unmultiplied(shadow_tint.r(), shadow_tint.g(), shadow_tint.b(), a),
            ),
        );
    }
    ui.add_space(crate::dt_f32!(separator.shadow_space, 4.0));
}

/// Indented section label with left margin — used inside dialogs.
pub fn dialog_section(ui: &mut egui::Ui, text: &str, margin: f32, color: Color32) {
    ui.horizontal(|ui| {
        ui.add_space(margin);
        ui.label(RichText::new(text).monospace().size(font_sm()).strong().color(color));
    });
    ui.add_space(gap_xs() + 1.0);
}

// ─── Labels ───────────────────────────────────────────────────────────────────

/// Section header — FONT_SM bold. Uppercases label when the active style requires it (#12).
/// Adds `section_label_padding_top` space before and `section_label_padding_bottom` after.
#[inline]
pub fn section_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    let st = current();
    if st.section_label_padding_top > 0.0 { ui.add_space(st.section_label_padding_top); }
    let label = style_label_case(text);
    ui.label(RichText::new(label).monospace().size(font_2xs()).strong().color(color));
    if st.section_label_padding_bottom > 0.0 { ui.add_space(st.section_label_padding_bottom); }
}

/// Extra-small section label — dim monospace at 6 pt, uppercase when style requires (#12).
#[inline]
pub fn section_label_xs(ui: &mut egui::Ui, text: &str, color: Color32) {
    let label = style_label_case(text);
    ui.label(RichText::new(label).monospace().size(6.0).color(color));
}

/// Dim info label — FONT_SM regular.
#[inline]
pub fn dim_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).monospace().size(font_sm()).color(color));
}

/// Column header cell — FONT_XS dim monospace, fixed width.
/// `right_align = true` for numeric columns (PRICE, SIZE), false for text (SYMBOL, TIME).
pub fn col_header(ui: &mut egui::Ui, text: &str, width: f32, color: Color32, right_align: bool) {
    let layout = if right_align {
        egui::Layout::right_to_left(egui::Align::Center)
    } else {
        egui::Layout::left_to_right(egui::Align::Center)
    };
    ui.allocate_ui_with_layout(egui::vec2(width, crate::dt_f32!(table.header_height, 12.0)), layout, |ui| {
        ui.label(RichText::new(text).monospace().size(font_xs()).color(color));
    });
}

// ─── Segmented control ───────────────────────────────────────────────────────

/// Pill group of buttons with a sunken inset trough. Returns `Some(index)` of the clicked
/// segment, `None` if nothing clicked. Caller updates state on `Some(i)`.
///
/// Uses a painter-reservation approach: buttons are rendered in the normal horizontal flow
/// (so `horizontal_centered` can center them correctly), and the trough background is
/// painted behind them via a reserved painter slot — avoiding Frame centering issues.
pub fn segmented_control(
    ui: &mut egui::Ui,
    active_idx: usize,
    labels: &[&str],
    toolbar_bg: Color32,
    toolbar_border: Color32,
    accent: Color32,
    dim: Color32,
) -> Option<usize> {
    let mut clicked = None;

    let td = crate::dt_u8!(segmented.trough_darken, 12);
    let trough = Color32::from_rgb(
        toolbar_bg.r().saturating_sub(td),
        toolbar_bg.g().saturating_sub(td),
        toolbar_bg.b().saturating_sub(td),
    );
    let border_col = color_alpha(toolbar_border, alpha_strong());

    let bg_slot = ui.painter().add(egui::Shape::Noop);

    let prev_spacing = ui.spacing().item_spacing.x;
    ui.spacing_mut().item_spacing.x = gap_xs();

    let mut union_rect: Option<egui::Rect> = None;
    let n = labels.len();
    let rsm = radius_sm() as u8;
    // Match Size::Sm button height (22px) so segmented controls don't read
    // 2px shorter than adjacent toolbar buttons.
    let seg_btn_h = 22.0;
    let seg_pad_x = 5.0;

    for (i, label) in labels.iter().enumerate() {
        let active = i == active_idx;
        let fg = if active { accent } else { dim };
        let bg = if active { color_alpha(accent, alpha_tint() + 5) } else { Color32::TRANSPARENT };
        let cr = match (i, n) {
            (0, 1) => egui::CornerRadius::same(rsm),
            (0, _) => egui::CornerRadius { nw: rsm, sw: rsm, ne: 0, se: 0 },
            (x, n) if x == n - 1 => egui::CornerRadius { nw: 0, sw: 0, ne: rsm, se: rsm },
            _ => egui::CornerRadius::ZERO,
        };
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(seg_pad_x, prev_pad.y);
        let resp = ui.add(
            egui::Button::new(RichText::new(*label).monospace().size(font_md()).strong().color(fg))
                .fill(bg).stroke(Stroke::NONE).corner_radius(cr)
                .min_size(egui::vec2(0.0, seg_btn_h))
        );
        ui.spacing_mut().button_padding = prev_pad;
        union_rect = Some(union_rect.map_or(resp.rect, |r: egui::Rect| r.union(resp.rect)));
        cursor::clickable(ui, &resp);
        if resp.clicked() { clicked = Some(i); }
    }

    ui.spacing_mut().item_spacing.x = prev_spacing;

    if let Some(ur) = union_rect {
        let trough_expand = crate::dt_f32!(segmented.trough_expand_x, 4.0);
        let trough_rect = ur.expand2(egui::vec2(trough_expand, 0.0));
        let r = radius_md() + 1.0;
        ui.painter().set(bg_slot, egui::Shape::rect_filled(trough_rect, r, trough));
        ui.painter().rect_stroke(trough_rect, r, Stroke::new(stroke_thin(), border_col), egui::StrokeKind::Outside);
    }

    clicked
}

// ─── Panel chrome ─────────────────────────────────────────────────────────────
// icon_btn removed — use `ui_kit::Button::icon(icon).variant(Variant::Ghost).placement(p).show(ui, t)`.

/// Close button (X icon) — standard panel close.
/// Kept for ABI compatibility (imported in core.rs which is sacred).
#[deprecated(note = "use `ui_kit::Button::close().show(ui, theme).clicked()`")]
#[inline]
pub fn close_button(ui: &mut egui::Ui, _dim: Color32) -> bool {
    let t = crate::ui_kit::widgets::theme::active_theme(ui.ctx());
    crate::ui_kit::widgets::Button::close().show(ui, &t).clicked()
}

// panel_header / panel_header_sub removed — use ui_kit::widgets::PanelHeader instead.

/// Horizontal tab bar — 2px underline on active tab. Renders inline; wrap in `ui.horizontal`.
pub fn tab_bar<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    current: &mut T,
    tabs: &[(T, &str)],
    accent: Color32,
    dim: Color32,
) {
    let tab_ul = crate::dt_f32!(tab.underline_thickness, 2.0);
    for (tab, label) in tabs {
        let active = *current == *tab;
        let color = if active { accent } else { dim };
        let resp = ui.add(
            egui::Button::new(RichText::new(*label).monospace().size(font_lg()).strong().color(color))
                .frame(false)
        );
        if resp.clicked() { *current = *tab; }
        if active && crate::chart_renderer::ui::style::current().show_active_tab_underline {
            let r = resp.rect;
            ui.painter().rect_filled(
                egui::Rect::from_min_max(egui::pos2(r.left(), r.max.y - tab_ul), egui::pos2(r.right(), r.max.y)),
                0.0, accent);
        }
    }
}

// ─── Tooltip infrastructure ───────────────────────────────────────────────────

/// Standard tooltip `egui::Frame` — use with `resp.on_hover_ui(|ui| { tooltip_frame(...).show(ui, |ui| { ... }) })`.
/// Matches the watchlist deferred tooltip style.
pub fn tooltip_frame(toolbar_bg: Color32, toolbar_border: Color32) -> egui::Frame {
    egui::Frame::NONE
        .fill(toolbar_bg)
        .stroke(Stroke::new(stroke_thin(), color_alpha(toolbar_border, alpha_strong())))
        .inner_margin(crate::dt_f32!(tooltip.padding, 8.0))
        .corner_radius(crate::dt_f32!(tooltip.corner_radius, 8.0))
}

/// Single stat row inside a tooltip — label left, value right.
pub fn stat_row(ui: &mut egui::Ui, label: &str, value: &str, label_color: Color32, value_color: Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).monospace().size(crate::dt_f32!(tooltip.stat_label_size, 8.0)).color(label_color));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).monospace().size(crate::dt_f32!(tooltip.stat_value_size, 10.0)).strong().color(value_color));
        });
    });
}

/// Paint a drop shadow behind a painter-based tooltip rect (call BEFORE painting the bg).
///
/// Resolves the active theme from the painter's context so light themes get a
/// soft gray drop shadow instead of the former hardcoded black.
pub fn paint_tooltip_shadow(painter: &egui::Painter, rect: egui::Rect, radius: f32) {
    let t = crate::ui_kit::widgets::theme::active_theme(painter.ctx());
    let shadow_rect = rect.translate(egui::vec2(shadow_offset(), shadow_offset()));
    painter.rect_filled(shadow_rect, radius, shadow_color_alpha(&t, shadow_alpha()));
}

/// Theme-aware drop shadow behind a painter-based tooltip rect.
pub fn paint_tooltip_shadow_themed(
    painter: &egui::Painter,
    rect: egui::Rect,
    radius: f32,
    t: &super::super::gpu::Theme,
) {
    let shadow_rect = rect.translate(egui::vec2(shadow_offset(), shadow_offset()));
    painter.rect_filled(shadow_rect, radius, shadow_color_alpha(t, shadow_alpha()));
}

// ─── Utility ──────────────────────────────────────────────────────────────────

/// Convert hex color string to Color32 with opacity.
pub fn hex_to_color(hex: &str, opacity: f32) -> Color32 {
    let h = hex.trim_start_matches('#');
    let r = u8::from_str_radix(h.get(0..2).unwrap_or("80"), 16).unwrap_or(128);
    let g = u8::from_str_radix(h.get(2..4).unwrap_or("80"), 16).unwrap_or(128);
    let b = u8::from_str_radix(h.get(4..6).unwrap_or("80"), 16).unwrap_or(128);
    Color32::from_rgba_unmultiplied(r, g, b, (opacity * 255.0) as u8)
}

/// Color with alpha — shorthand for `Color32::from_rgba_unmultiplied(r, g, b, alpha)`.
#[inline]
pub fn color_alpha(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

// ─── Color dimming helpers ───────────────────────────────────────────────────
// Replace ad-hoc `color.gamma_multiply(0.X)` chains with these named helpers.
// Pick by intent, not by number — see UI_AUDIT.md for the histogram of
// usages each multiplier covers.
//
// `subtle`     — secondary text/icons that still read clearly
// `muted`      — disabled-leaning, but still visible
// `dim`        — clearly de-emphasised (placeholder text, etc.)
// `very_dim`   — barely visible (decorative chart rules, watermarks)

/// 0.7× — secondary text/icons that still read clearly.
#[inline] pub fn color_subtle(c: Color32) -> Color32 { c.gamma_multiply(0.7) }
/// 0.6× — muted UI element (visible but not interactive-feeling).
#[inline] pub fn color_muted(c: Color32) -> Color32 { c.gamma_multiply(0.6) }
/// 0.5× — half-strength.
#[inline] pub fn color_half(c: Color32) -> Color32 { c.gamma_multiply(0.5) }
/// 0.4× — clearly de-emphasised (placeholder text, inactive states).
#[inline] pub fn color_dim(c: Color32) -> Color32 { c.gamma_multiply(0.4) }
/// 0.3× — barely visible (decorative chart rules, watermarks).
#[inline] pub fn color_very_dim(c: Color32) -> Color32 { c.gamma_multiply(0.3) }

// ─── Lighten / darken primitives ─────────────────────────────────────────────
// Linear RGB lerp toward white / black. Used to derive hover/pressed states
// from a base fill color.

/// Lighten a color toward white by `amount` (0.0–1.0). Preserves alpha.
#[inline]
pub fn lighten(c: Color32, amount: f32) -> Color32 {
    let amt = amount.clamp(0.0, 1.0);
    let r = c.r() as f32 + (255.0 - c.r() as f32) * amt;
    let g = c.g() as f32 + (255.0 - c.g() as f32) * amt;
    let b = c.b() as f32 + (255.0 - c.b() as f32) * amt;
    Color32::from_rgba_premultiplied(r as u8, g as u8, b as u8, c.a())
}

/// Darken a color toward black by `amount` (0.0–1.0). Preserves alpha.
#[inline]
pub fn darken(c: Color32, amount: f32) -> Color32 {
    let amt = (1.0 - amount).clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied(
        (c.r() as f32 * amt) as u8,
        (c.g() as f32 * amt) as u8,
        (c.b() as f32 * amt) as u8,
        c.a(),
    )
}

// ─── Semantic interaction-state colors ───────────────────────────────────────
// Canonical hover / pressed / active / divider / disabled tones built on the
// primitives above. Call-sites should reach for these instead of inlining
// `lighten(c, 0.10)` / `color_alpha(t.toolbar_border, 36)` etc.

/// Brighten a color by 10% — canonical hover treatment for filled surfaces.
#[inline] pub fn color_hover(c: Color32) -> Color32 { lighten(c, 0.10) }

/// Darken a color by 8% — canonical pressed/active state for filled surfaces.
#[inline] pub fn color_pressed(c: Color32) -> Color32 { darken(c, 0.08) }

/// Subtle text-color hover tint for rows/cells. Roughly matches PanelListRow's
/// HOVER_BG_ALPHA constant — gives ~7% text alpha overlay.
#[inline]
pub fn hover_tint_text(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    color_alpha(t.text, 18)
}

/// Subtle accent fill for active chips/toggles. Use when a toggleable
/// surface needs a "yes I'm on" visual that's quieter than a full accent.
#[inline]
pub fn active_chip_fill(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    color_alpha(t.accent, alpha_soft())
}

/// Standard hairline divider color. Wraps the toolbar_border + alpha 36 pair
/// that's been hand-written across ~5 files for section dividers.
#[inline]
pub fn divider_color(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    color_alpha(t.toolbar_border, 36)
}

/// Disabled overlay — soft dim wash to apply over content that's not interactive.
#[inline]
pub fn disabled_overlay(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    color_alpha(t.dim, alpha_dim())
}

// ─── L2 surface helper (panel sub-section / card layer) ──────────────────────
//
// The design system uses four surface layers:
//   L0: t.bg              — app canvas
//   L1: t.toolbar_bg      — panel body
//   L2: `color_layer_up`  — sub-section / card / active tab body
//   L3: hover/selected    — color_alpha(t.text, 8) or color_alpha(t.accent, 24)
//
// Direction (lighten vs darken) is derived from the theme's `bg` brightness so
// the lift reads the same on dark + light themes.

/// Returns the panel surface one layer up from `t.toolbar_bg`. Used for
/// cards, sub-sections, the active tab body — anywhere the design system
/// asks for a subtle L2 surface that contrasts gently with the panel body.
///
/// `n` is the number of 4% steps to lift (clamped to keep things subtle).
/// `n=1` is the canonical L2; larger values move toward an L3-ish accent.
/// Direction (lighten vs darken) follows whether the active theme is dark or
/// light — detected from `t.bg` brightness, same heuristic the gpu hairline
/// helpers use.
#[inline]
pub(crate) fn color_layer_up(t: &crate::chart_renderer::gpu::Theme, n: u8) -> Color32 {
    let base = t.toolbar_bg;
    let bg = t.bg;
    // Match `gpu::hairline_border`'s dark-vs-light heuristic.
    let is_dark = (bg.r() as i16 + bg.g() as i16 + bg.b() as i16) < 384;
    // 7% per step (≈18/255), capped at 5 steps. Calibrated so an L2
    // subsection visibly nests above L1 without looking like a separate
    // surface; was 4% which read as nothing. Stays under the threshold
    // where the lifted bg starts feeling like a separate card.
    let steps = n.min(5) as i16;
    let shift: i16 = if is_dark { 18 * steps } else { -18 * steps };
    let clamp = |c: i16| -> u8 { c.clamp(0, 255) as u8 };
    Color32::from_rgb(
        clamp(base.r() as i16 + shift),
        clamp(base.g() as i16 + shift),
        clamp(base.b() as i16 + shift),
    )
}

/// Top-level panel header surface — closest to `t.bg` (chart pane
/// background). Used by SidePanelShell so the topmost panel chrome
/// reads as adjacent to the chart pane.
#[inline]
pub(crate) fn header_surface(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    t.bg.gamma_multiply(ELEVATION_1_FACTOR)
}

/// Section header surface — one shade darker than `header_surface` so
/// PanelSection headers sit visually below the SidePanelShell header
/// above them. Creates the depth ramp: SidePanelShell (lightest) →
/// PanelSection → PanelSubSection → panel body (darkest).
#[inline]
pub(crate) fn section_header_surface(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    t.bg.gamma_multiply(ELEVATION_2_FACTOR)
}

/// Panel body surface — darker than `t.bg` so the side panel body
/// recedes visually below the chart and below its own header.
/// The pattern is: header (lighter, near `t.bg`) → body (darker,
/// recessed) — readable depth without high-contrast slabs.
#[inline]
pub(crate) fn panel_surface(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    t.bg.gamma_multiply(ELEVATION_3_FACTOR)
}

/// Header border — matches the chart pane header's perimeter hairline:
/// `color_alpha(t.text, 38)` at `stroke_thin()`. Use for every panel
/// header bottom rule, accordion rule, and side-panel header rule so
/// the entire chrome family reads as one bordered system.
#[inline]
pub(crate) fn header_border(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    color_alpha(t.text, 38)
}

/// Inverse of `color_layer_up` — moves the surface DOWN one or more
/// layers (toward `t.bg`, away from `t.text`). Use for elements that
/// should read as RECESSED rather than RAISED — section header bands,
/// trough surfaces, inset wells. Direction-aware: darker on dark themes,
/// lighter on light themes (always away from the foreground).
#[inline]
pub(crate) fn color_layer_down(t: &crate::chart_renderer::gpu::Theme, n: u8) -> Color32 {
    let base = t.toolbar_bg;
    let bg = t.bg;
    let is_dark = (bg.r() as i16 + bg.g() as i16 + bg.b() as i16) < 384;
    let steps = n.min(5) as i16;
    // OPPOSITE direction from color_layer_up — dark themes go darker,
    // light themes go lighter (move toward t.bg).
    let shift: i16 = if is_dark { -18 * steps } else { 18 * steps };
    let clamp = |c: i16| -> u8 {
        if c < 0 { 0 } else if c > 255 { 255 } else { c as u8 }
    };
    Color32::from_rgb(
        clamp(base.r() as i16 + shift),
        clamp(base.g() as i16 + shift),
        clamp(base.b() as i16 + shift),
    )
}

// ─── Theme-aware shadow color ────────────────────────────────────────────────
// Light themes set `t.shadow_color` to a dark-gray (not black) so shadows on
// cream/peach backgrounds blend instead of looking like hole-punched silhouettes.
// Use this helper instead of hardcoding `Color32::from_rgba_unmultiplied(0,0,0,X)`.

/// Shadow color from the theme at the given alpha. Replaces hardcoded
/// `Color32::from_rgba_unmultiplied(0, 0, 0, X)` calls — those break light themes.
#[inline]
pub fn shadow_color_alpha(t: &super::super::gpu::Theme, alpha: u8) -> Color32 {
    let c = t.shadow_color;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

// ─── Form layout ──────────────────────────────────────────────────────────────

/// Form row: right-aligned fixed-width label + content widget.
pub fn form_row(ui: &mut egui::Ui, label: &str, label_width: f32, dim: Color32, add_content: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.allocate_ui(egui::vec2(label_width, crate::dt_f32!(form.row_height, 18.0)), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(gap_sm());
                ui.label(RichText::new(label).monospace().size(font_sm()).color(dim));
            });
        });
        add_content(ui);
    });
}

// ─── Cards / badges ───────────────────────────────────────────────────────────

/// Status badge — small tinted pill (e.g. "DRAFT", "PLACED", "TRIGGERED").
pub fn status_badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    // r_chip overrides r_sm for badges/chips when non-zero; allows pill chips alongside
    // square buttons on the same style.
    let st = current();
    let chip_r = if st.r_chip > 0 { st.r_chip as f32 } else { radius_sm() };
    let resp = ui.add(egui::Button::new(RichText::new(text).monospace().size(crate::dt_f32!(badge.font_size, 8.0)).strong().color(color))
        .fill(color_alpha(color, alpha_subtle()))
        .stroke(Stroke::new(stroke_thin(), color_alpha(color, alpha_dim())))
        .corner_radius(chip_r)
        .min_size(egui::vec2(0.0, crate::dt_f32!(badge.height, 16.0))));
    hit(&resp.rect, "BADGE", "Badges");
}

/// Order card — left accent stripe + subtle bg. Returns true if the card area was clicked.
pub fn order_card(ui: &mut egui::Ui, accent: Color32, bg: Color32, add_content: impl FnOnce(&mut egui::Ui)) -> bool {
    let ml = crate::dt_i8!(card.margin_left, 9);
    let mr = crate::dt_i8!(card.margin_right, 6);
    let my = crate::dt_i8!(card.margin_y, 5);
    let cr = crate::dt_f32!(card.radius, 4.0);
    let available_w = ui.available_width();
    let resp = egui::Frame::NONE
        .fill(bg)
        .inner_margin(egui::Margin { left: ml, right: mr, top: my, bottom: my })
        .corner_radius(cr)
        .show(ui, |ui| {
            ui.set_min_width(available_w - 15.0);
            let outer = ui.min_rect();
            let stripe = egui::Rect::from_min_max(
                egui::pos2(outer.left() - ml as f32, outer.top() - my as f32),
                egui::pos2(outer.left() - ml as f32 + crate::dt_f32!(card.stripe_width, 3.0), outer.bottom() + my as f32));
            let stripe_col = color_alpha(accent, current().card_stripe_alpha);
            ui.painter().rect_filled(stripe, egui::CornerRadius { nw: cr as u8, sw: cr as u8, ne: 0, se: 0 }, stripe_col);
            add_content(ui);
        });
    let card_rect = resp.response.rect;
    let click_resp = ui.interact(card_rect, ui.id().with(("card_click", card_rect.min.x as i32, card_rect.min.y as i32)), egui::Sense::click());
    if click_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    cursor::focus_ring(ui, &click_resp, accent);
    ui.add_space(gap_sm());
    click_resp.clicked()
}

// ─── Buttons ──────────────────────────────────────────────────────────────────

// action_btn / trade_btn / cta_btn / small_action_btn / simple_btn removed —
// zero call sites. Use the canonical ui_kit::Button presets:
//   ui_kit::Button::action / ::trade / ::cta / ::small_action / ::simple

// ─── Drawing helpers ──────────────────────────────────────────────────────────

/// Draw a dashed or dotted line between two points.
pub fn dashed_line(painter: &egui::Painter, a: egui::Pos2, b: egui::Pos2, stroke: Stroke, style: super::super::LineStyle) {
    use super::super::LineStyle;
    let dir = b - a;
    let len = dir.length();
    if len < 1.0 || !len.is_finite() || len > 20000.0 { return; }
    match style {
        LineStyle::Solid => { painter.line_segment([a, b], stroke); }
        LineStyle::Dashed | LineStyle::Dotted => {
            let (dash, gap) = if style == LineStyle::Dashed { (6.0, 3.0) } else { (2.0, 2.0) };
            let norm = dir / len;
            let mut d = 0.0;
            while d < len {
                let p0 = a + norm * d;
                let p1 = a + norm * (d + dash).min(len);
                painter.line_segment([p0, p1], stroke);
                d += dash + gap;
            }
        }
    }
}

/// Draw a thick line into an RGBA buffer (for icon generation).
pub fn draw_line_rgba(rgba: &mut [u8], width: u32, x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32, color: [u8; 4]) {
    let len = ((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0)).sqrt();
    let steps = (len * 3.0) as i32;
    let w = thickness as i32;
    for i in 0..=steps {
        let t = i as f32 / steps.max(1) as f32;
        let px = (x0 + (x1 - x0) * t) as i32;
        let py = (y0 + (y1 - y0) * t) as i32;
        for dy in -w..=w {
            for dx in -w..=w {
                let ix = px + dx;
                let iy = py + dy;
                if ix >= 0 && ix < width as i32 && iy >= 0 && iy < width as i32 {
                    let idx = ((iy as u32 * width + ix as u32) * 4) as usize;
                    if idx + 3 < rgba.len() { rgba[idx..idx + 4].copy_from_slice(&color); }
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Split-section sidebar helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Draggable divider between two split sections. Returns vertical drag delta.
pub fn split_divider(ui: &mut egui::Ui, _id_salt: &str, dim: Color32) -> f32 {
    let div_h = crate::dt_f32!(split_divider.height, 6.0);
    let inset = crate::dt_f32!(split_divider.inset, 8.0);
    let dot_r = crate::dt_f32!(split_divider.dot_radius, 1.5);
    let dot_sp = crate::dt_f32!(split_divider.dot_spacing, 8.0);
    let active_sw = crate::dt_f32!(split_divider.active_stroke, 2.0);
    let inactive_sw = crate::dt_f32!(split_divider.inactive_stroke, 1.0);

    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), div_h), egui::Sense::drag());
    let p = ui.painter();

    let active = resp.hovered() || resp.dragged();
    let st_dh = current();
    let handle_alpha_mult = if active { 1.0 } else { st_dh.drag_handle_alpha };
    let color = if active { dim.gamma_multiply(0.6) } else {
        color_alpha(dim, (alpha_faint() as f32 * (handle_alpha_mult / 0.5).min(1.0)) as u8)
    };

    // Active drag handle uses stroke_thick from the style preset for a prominent feel.
    let effective_active_sw = st_dh.stroke_thick.max(active_sw);
    p.line_segment(
        [egui::pos2(rect.left() + inset, rect.center().y),
         egui::pos2(rect.right() - inset, rect.center().y)],
        Stroke::new(if active { effective_active_sw } else { inactive_sw }, color));

    if active {
        let cy = rect.center().y;
        let cx = rect.center().x;
        let scaled_dot_r = dot_r * st_dh.drag_handle_dot_scale;
        for dx in [-dot_sp, 0.0, dot_sp] {
            p.circle_filled(egui::pos2(cx + dx, cy), scaled_dot_r, dim.gamma_multiply(0.4));
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }

    if resp.dragged() { resp.drag_delta().y } else { 0.0 }
}

// ─── Compatibility shims for in-session widget builders ───────────────────────
// These were introduced alongside the new widgets/* design-system primitives.
// They centralize per-style overrides; for now they return reasonable defaults.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonTreatment {
    SoftPill,
    OutlineAccent,
    UnderlineActive,
    RaisedActive,
    BlackFillActive,
}

#[derive(Clone, PartialEq, Debug)]
pub struct StyleSettings {
    pub r_xs: u8,
    pub r_sm: u8,
    pub r_md: u8,
    pub r_lg: u8,
    pub r_pill: u8,
    pub serif_headlines: bool,
    pub button_treatment: ButtonTreatment,
    pub hairline_borders: bool,
    pub stroke_hair: f32,
    pub stroke_thin: f32,
    pub stroke_std: f32,
    /// Bold stroke weight — Meridien collapses to 1 px, Relay/Aperture use 1.5.
    pub stroke_bold: f32,
    pub stroke_thick: f32,
    pub shadows_enabled: bool,
    pub solid_active_fills: bool,
    /// True only for the style that paints active elements as a solid inverted
    /// slab (fill = palette text, text = palette bg). Replaces the removed
    /// `active_fill_color` / `active_text_color` `Option<Color32>` overrides
    /// (Phase 0b): a dimension-axis boolean, not a colour. Distinct from
    /// `solid_active_fills` — that flag is broader and true for more styles.
    pub invert_active_fill: bool,
    pub uppercase_section_labels: bool,
    /// Letter spacing approximation (px) applied to tracked-out section labels.
    pub label_letter_spacing_px: f32,
    /// Multiplier applied when scaling toolbar height (1.0 = baseline, 1.4 = Meridien tall).
    pub toolbar_height_scale: f32,
    /// Multiplier applied when scaling pane header height (1.0 = baseline, 1.1 = Meridien).
    pub header_height_scale: f32,
    /// Hero numeric font size in pt (22 for Relay, 36 for Meridien).
    pub font_hero: f32,
    /// Paint full-height vertical divider lines between toolbar button clusters.
    pub vertical_group_dividers: bool,
    /// Show active-tab accent underline in tab bars.
    pub show_active_tab_underline: bool,
    /// Active pane header fill multiplier (1.2 = brighter for Relay, 0.95 = near-transparent for Meridien).
    pub active_header_fill_multiply: f32,
    /// Inactive pane header fill multiplier — applied when there are multiple
    /// visible panes and this pane is not active. Lower = more recessed.
    pub inactive_header_fill_multiply: f32,
    /// Paint a distinct fill for inactive pane headers.
    pub inactive_header_fill: bool,
    /// Alpha of the hairline outer border drawn around inactive pane headers
    /// (the contrast color is derived from theme luminance).
    pub header_outer_border_alpha: u8,
    /// Stroke width of the pane-header outer border.
    pub header_outer_border_width: f32,
    /// Alpha of the inter-section vertical divider lines inside the pane header
    /// (between nav cluster, indicator chips, and right-side icon buttons).
    pub header_divider_alpha: u8,
    /// Account strip panel height in logical px.
    pub account_strip_height: f32,

    // ── Layout & spacing ──────────────────────────────────────────────────
    /// Pane border outline thickness in logical px.
    pub pane_border_width: f32,
    /// Gap between adjacent panes in px.
    pub pane_gap: f32,
    /// Card vertical inner padding in px.
    pub card_padding_y: f32,
    /// Card horizontal inner padding in px.
    pub card_padding_x: f32,
    /// Base list-row height in px.
    pub row_height_px: f32,
    /// Base button height in px.
    pub button_height_px: f32,
    /// Button horizontal padding in px.
    pub button_padding_x: f32,
    /// Tab strip height in px.
    pub tab_height: f32,

    // ── Typography overrides ──────────────────────────────────────────────
    /// Section/eyebrow label font size in pt.
    pub font_section_label: f32,
    /// Body text font size in pt.
    pub font_body: f32,
    /// Caption font size in pt.
    pub font_caption: f32,

    // ── Interaction tokens ────────────────────────────────────────────────
    /// Alpha for hover overlay (0-255).
    pub hover_bg_alpha: u8,
    /// Alpha for active/pressed state (0-255).
    pub active_bg_alpha: u8,
    /// Focus ring stroke width.
    pub focus_ring_width: f32,
    /// Focus ring alpha (0-255).
    pub focus_ring_alpha: u8,
    /// Opacity multiplier for disabled widgets (0.0-1.0).
    pub disabled_opacity: f32,

    // ── Drop shadow ───────────────────────────────────────────────────────
    /// Shadow blur radius in px.
    pub shadow_blur: f32,
    /// Shadow vertical offset in px.
    pub shadow_offset_y: f32,
    /// Shadow alpha (0-255).
    pub shadow_alpha: u8,

    // ── Density & accent ─────────────────────────────────────────────────
    /// Global density: 0=compact, 1=normal, 2=roomy. Drives row/tab/button
    /// height multipliers when explicit overrides are not set.
    pub density: u8,
    /// Saturation/brightness multiplier for accent on active elements.
    pub accent_emphasis: f32,

    // ── Reference-match fields (Newsprint/editorial style) ────────────────
    /// Letter-spacing added between glyphs in toolbar nav buttons (px).
    /// Meridien: 1.5, others: 0.
    pub nav_letter_spacing_px: f32,
    /// Drop icon glyphs from right-side toolbar nav buttons (label-only).
    /// Meridien: true, others: false.
    pub nav_buttons_label_only: bool,
    /// Render right-side toolbar nav button labels in ALL CAPS.
    /// Meridien: true, others: false.
    pub nav_buttons_uppercase_labels: bool,
    /// Thickness of the tab-active underline in pane headers.
    /// Meridien: 2.0, Aperture: 0.0 (hidden), Octave: 1.0.
    pub tab_underline_thickness: f32,
    /// When true, draw the underline directly under active tab text (not at header bottom).
    /// Meridien: true, others: false.
    pub tab_underline_under_text: bool,
    /// Show a subtle floating shadow on card windows even when `shadows_enabled` is false.
    /// Meridien: true, Aperture: covered by shadows_enabled, Octave: false.
    pub card_floating_shadow: bool,
    /// Alpha for the card floating shadow (0-255). Meridien: 25, others: 0.
    pub card_floating_shadow_alpha: u8,
    /// Height for the primary CTA button in px. Meridien: 36, Aperture: 40, Octave: 32.
    pub cta_height_px: f32,
    /// Horizontal padding for the primary CTA button in px. Meridien: 16, others: 12.
    pub cta_padding_x: f32,

    // ── New knobs added in design-pass 2 ─────────────────────────────────────

    /// Pane gap fill color alpha (0-255). 0 = transparent (gap shows bg).
    /// Controls the visible color of the gutter between panes.
    /// Meridien: 0 (flush), Aperture: 30, Octave: 15.
    pub pane_gap_alpha: u8,
    /// Pane active indicator: 0=none, 1=top border line, 2=header fill, 3=both.
    /// Meridien: 1, Aperture: 2, Octave: 3.
    pub pane_active_indicator: u8,
    /// Toolbar nav background alpha for active button column tint.
    /// Meridien: 18, Aperture: 0 (none), Octave: 25.
    pub nav_active_col_alpha: u8,
    /// Alpha for the dialog/popup backdrop overlay (0-255). 0 = no backdrop.
    pub dialog_backdrop_alpha: u8,
    /// Tab inactive text alpha multiplier (0.0-1.0). 0.5 = dimmed, 1.0 = full.
    pub tab_inactive_alpha: f32,
    /// Tab hover background alpha (0-255). Applied when hovering an inactive tab.
    pub tab_hover_bg_alpha: u8,
    /// Section label top padding in px (space above eyebrow labels).
    pub section_label_padding_top: f32,
    /// Section label bottom padding in px (space below eyebrow labels before content).
    pub section_label_padding_bottom: f32,
    /// Pane gap (gutter) fill color override. None = use toolbar_border at pane_gap_alpha.
    pub pane_gap_color: Option<Color32>,
    /// Drag handle (split divider) color alpha multiplier (0.0-1.0).
    pub drag_handle_alpha: f32,
    /// Drag handle dot size multiplier (0.5-2.0). 1.0 = default.
    pub drag_handle_dot_scale: f32,
    /// Toast / status-bar background alpha (0-255).
    pub toast_bg_alpha: u8,
    /// Stripe/accent-banner fill alpha for order/alert cards (0-255).
    pub card_stripe_alpha: u8,
    /// Pill / chip border radius separate from r_sm. 0 = use r_sm.
    /// When non-zero, overrides r_sm for badge/chip corners specifically.
    pub r_chip: u8,

    // ── Accessibility ─────────────────────────────────────────────────────
    /// When false, all motion::ease_bool / ease_value calls snap immediately
    /// to their target value, honoring the system "reduce motion" preference.
    /// Default: true.
    pub animations_enabled: bool,
}

// Active style selection — set once at the top of each draw_chart frame
// from `gpu::style_id(watchlist)`. 0 = Meridien (editorial), 1 = Aperture
// (modern, soft), 2 = Octave (dense). All other indices alias to Meridien.
static ACTIVE_STYLE: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(0);

pub fn set_active_style(id: u8) {
    ACTIVE_STYLE.store(id, std::sync::atomic::Ordering::Relaxed);
    // Refresh the per-frame token snapshot (spec §5 Rule 2).  Called here so
    // the one existing `set_active_style` call-site in `draw_chart` (core.rs)
    // needs no modification — `core.rs` is sacred and must not be touched.
    begin_frame();
}

// Toolbar rect — set once at the start of each toolbar frame so tb_btn can
// read it for full-height hover/active column overlays (Meridien only, #18).
// Encoded as four f32 bits packed into four AtomicU32 cells (min_x, min_y, max_x, max_y).
static TB_RECT: [std::sync::atomic::AtomicU32; 4] = [
    std::sync::atomic::AtomicU32::new(0),
    std::sync::atomic::AtomicU32::new(0),
    std::sync::atomic::AtomicU32::new(0),
    std::sync::atomic::AtomicU32::new(0),
];

/// Set the toolbar rect at the start of the toolbar panel (gpu.rs ~line 3700).
pub fn set_toolbar_rect(r: egui::Rect) {
    TB_RECT[0].store(r.min.x.to_bits(), std::sync::atomic::Ordering::Relaxed);
    TB_RECT[1].store(r.min.y.to_bits(), std::sync::atomic::Ordering::Relaxed);
    TB_RECT[2].store(r.max.x.to_bits(), std::sync::atomic::Ordering::Relaxed);
    TB_RECT[3].store(r.max.y.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

/// Read the stored toolbar rect. Returns a zero-sized rect if not yet set.
pub fn toolbar_rect() -> egui::Rect {
    let min_x = f32::from_bits(TB_RECT[0].load(std::sync::atomic::Ordering::Relaxed));
    let min_y = f32::from_bits(TB_RECT[1].load(std::sync::atomic::Ordering::Relaxed));
    let max_x = f32::from_bits(TB_RECT[2].load(std::sync::atomic::Ordering::Relaxed));
    let max_y = f32::from_bits(TB_RECT[3].load(std::sync::atomic::Ordering::Relaxed));
    egui::Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y))
}

// ─── Live-editable style storage ─────────────────────────────────────────────
// Three RwLock<StyleSettings> initialised once from the hardcoded defaults.
// `current()` clones the active one; `set_style_settings` overwrites it.

// ┌─ STYLE_DEFAULTS_BEGIN ─────────────────────────────────────────────────────
fn style_defaults(id: u8) -> StyleSettings {
    match id {
        1 => StyleSettings {
            r_xs: 4, r_sm: 6, r_md: 8, r_lg: 12, r_pill: 99,
            serif_headlines: false,
            button_treatment: ButtonTreatment::SoftPill,
            hairline_borders: false,
            stroke_hair: 0.5, stroke_thin: 1.0, stroke_std: 1.5,
            stroke_bold: 1.5, stroke_thick: 2.0,
            shadows_enabled: true, solid_active_fills: false, invert_active_fill: false,
            uppercase_section_labels: false, label_letter_spacing_px: 0.0,
            toolbar_height_scale: 1.0, header_height_scale: 1.0,
            font_hero: 22.0, vertical_group_dividers: false,
            show_active_tab_underline: true,
            active_header_fill_multiply: 0.7, inactive_header_fill_multiply: 1.08,
            inactive_header_fill: true,
            header_outer_border_alpha: 38, header_outer_border_width: 0.5,
            header_divider_alpha: 50,
            account_strip_height: 26.0,
            pane_border_width: 1.0, pane_gap: 8.0,
            card_padding_y: 12.0, card_padding_x: 14.0,
            row_height_px: 26.0, button_height_px: 28.0, button_padding_x: 14.0,
            tab_height: 32.0,
            font_section_label: 10.0, font_body: 11.0, font_caption: 9.0,
            hover_bg_alpha: 15, active_bg_alpha: 25,
            focus_ring_width: 2.0, focus_ring_alpha: 90, disabled_opacity: 0.5,
            shadow_blur: 24.0, shadow_offset_y: 8.0, shadow_alpha: 40,
            density: 2, accent_emphasis: 1.1,
            nav_letter_spacing_px: 0.0, tab_underline_thickness: 0.0,
            nav_buttons_label_only: false, nav_buttons_uppercase_labels: false,
            tab_underline_under_text: false, card_floating_shadow: false,
            card_floating_shadow_alpha: 0,
            cta_height_px: 40.0, cta_padding_x: 12.0,
            pane_gap_alpha: 30, pane_active_indicator: 2,
            nav_active_col_alpha: 0, dialog_backdrop_alpha: 0,
            tab_inactive_alpha: 0.55, tab_hover_bg_alpha: 18,
            section_label_padding_top: 6.0, section_label_padding_bottom: 2.0,
            pane_gap_color: None,
            drag_handle_alpha: 0.7, drag_handle_dot_scale: 1.0,
            toast_bg_alpha: 200, card_stripe_alpha: 255,
            r_chip: 0,
            animations_enabled: true,
        },
        2 => StyleSettings {
            r_xs: 1, r_sm: 2, r_md: 3, r_lg: 4, r_pill: 99,
            serif_headlines: false,
            button_treatment: ButtonTreatment::RaisedActive,
            hairline_borders: true,
            stroke_hair: 0.4, stroke_thin: 0.6, stroke_std: 1.0,
            stroke_bold: 1.0, stroke_thick: 1.4,
            shadows_enabled: false, solid_active_fills: true, invert_active_fill: false,
            uppercase_section_labels: true, label_letter_spacing_px: 0.0,
            toolbar_height_scale: 1.0, header_height_scale: 1.0,
            font_hero: 22.0, vertical_group_dividers: false,
            show_active_tab_underline: true,
            active_header_fill_multiply: 0.7, inactive_header_fill_multiply: 1.08,
            inactive_header_fill: true,
            header_outer_border_alpha: 38, header_outer_border_width: 0.5,
            header_divider_alpha: 50,
            account_strip_height: 26.0,
            pane_border_width: 1.0, pane_gap: 2.0,
            card_padding_y: 6.0, card_padding_x: 8.0,
            row_height_px: 20.0, button_height_px: 22.0, button_padding_x: 8.0,
            tab_height: 26.0,
            font_section_label: 8.0, font_body: 10.0, font_caption: 8.0,
            hover_bg_alpha: 18, active_bg_alpha: 30,
            focus_ring_width: 1.5, focus_ring_alpha: 110, disabled_opacity: 0.45,
            shadow_blur: 8.0, shadow_offset_y: 4.0, shadow_alpha: 20,
            density: 0, accent_emphasis: 0.95,
            nav_letter_spacing_px: 0.0, tab_underline_thickness: 1.0,
            nav_buttons_label_only: false, nav_buttons_uppercase_labels: false,
            tab_underline_under_text: false, card_floating_shadow: false,
            card_floating_shadow_alpha: 0,
            cta_height_px: 32.0, cta_padding_x: 12.0,
            pane_gap_alpha: 15, pane_active_indicator: 3,
            nav_active_col_alpha: 25, dialog_backdrop_alpha: 0,
            tab_inactive_alpha: 0.5, tab_hover_bg_alpha: 20,
            section_label_padding_top: 3.0, section_label_padding_bottom: 1.0,
            pane_gap_color: None,
            drag_handle_alpha: 0.6, drag_handle_dot_scale: 0.85,
            toast_bg_alpha: 220, card_stripe_alpha: 255,
            r_chip: 0,
            animations_enabled: true,
        },
        _ => StyleSettings {
            // Phase B source-swap: Meridien (the default style) is redefined
            // to hold the values the app actually rendered before the swap —
            // the graduated dt_f32! token scale that ~75% of call sites used.
            // This makes the source-swap a no-op for the default look.
            r_xs: 2, r_sm: 4, r_md: 6, r_lg: 12, r_pill: 0,
            serif_headlines: true,
            button_treatment: ButtonTreatment::UnderlineActive,
            hairline_borders: true,
            stroke_hair: 0.3, stroke_thin: 0.5, stroke_std: 1.0,
            stroke_bold: 1.5, stroke_thick: 2.0,
            shadows_enabled: true, solid_active_fills: true, invert_active_fill: true,
            uppercase_section_labels: true, label_letter_spacing_px: 0.0,
            toolbar_height_scale: 1.40, header_height_scale: 1.10,
            font_hero: 36.0, vertical_group_dividers: true,
            show_active_tab_underline: true,
            active_header_fill_multiply: 0.7, inactive_header_fill_multiply: 1.08,
            inactive_header_fill: true,
            header_outer_border_alpha: 38, header_outer_border_width: 0.5,
            header_divider_alpha: 50,
            account_strip_height: 36.0,
            pane_border_width: 1.0, pane_gap: 0.0,
            card_padding_y: 8.0, card_padding_x: 10.0,
            row_height_px: 22.0, button_height_px: 24.0, button_padding_x: 10.0,
            tab_height: 28.0,
            font_section_label: 8.0, font_body: 10.0, font_caption: 8.0,
            hover_bg_alpha: 20, active_bg_alpha: 35,
            focus_ring_width: 1.0, focus_ring_alpha: 120, disabled_opacity: 0.4,
            shadow_blur: 0.0, shadow_offset_y: 0.0, shadow_alpha: 0,
            density: 1, accent_emphasis: 1.0,
            nav_letter_spacing_px: 0.0, tab_underline_thickness: 2.0,
            nav_buttons_label_only: true, nav_buttons_uppercase_labels: true,
            tab_underline_under_text: true, card_floating_shadow: true,
            card_floating_shadow_alpha: 25,
            cta_height_px: 36.0, cta_padding_x: 16.0,
            pane_gap_alpha: 0, pane_active_indicator: 1,
            nav_active_col_alpha: 18, dialog_backdrop_alpha: 0,
            tab_inactive_alpha: 0.6, tab_hover_bg_alpha: 12,
            section_label_padding_top: 4.0, section_label_padding_bottom: 2.0,
            pane_gap_color: None,
            drag_handle_alpha: 0.5, drag_handle_dot_scale: 1.0,
            toast_bg_alpha: 230, card_stripe_alpha: 255,
            r_chip: 0,
            animations_enabled: true,
        },
    }
}
// └─ STYLE_DEFAULTS_END ───────────────────────────────────────────────────────

/// Public test accessor for `style_defaults`.
/// Maps: 0 → Meridien (default `_` arm), 1 → Aperture, 2 → Octave.
/// Available only in `#[cfg(test)]` so it does not bloat the release binary.
#[cfg(test)]
pub fn style_defaults_pub(id: u8) -> StyleSettings {
    style_defaults(id)
}

// ─── Design-system → StyleSettings adapter ───────────────────────────────────
//
// Converts a `design_system::StyleSystem` to a `StyleSettings` using
// `style_defaults(base_id)` as the base value (via struct-update syntax)
// and then overrides every field that `StyleSystem` cleanly carries.
//
// Fields with a clean `StyleSystem` source (12 groups, ~20 fields):
//   radii:      r_xs/r_sm/r_md/r_lg from ss.radii.xs/sm/md/lg  (cast f32→u8)
//               r_pill from ss.radii.full  (capped at 255)
//   strokes:    stroke_hair/thin/std/bold/thick from ss.strokes.hair/thin/std/bold/thick
//   treatments: hairline_borders/solid_active_fills/uppercase_section_labels
//               from ss.treatments.*
//   spacing:    cta_height_px from ss.spacing.cta_height
//               card_padding_y / card_padding_x from ss.spacing.md / lg
//   typography: font_section_label / font_caption from ss.typography.size_xs
//               font_body from ss.typography.size_sm
//               font_hero from ss.typography.size_xl
//   density:    density from ss.density.factor (0.8→0, 1.0→1, ≥1.2→2)
//               row_height_px from ss.density.row_height_dense
//   shadows:    shadow_blur/shadow_offset_y from ss.shadows.card.blur/offset_y
//               shadow_alpha from (ss.shadows.card.alpha * 255) as u8
//               shadows_enabled from ss.shadows.card.blur > 0.0
//
// All other StyleSettings fields keep the `style_defaults(base_id)` value
// through the struct-update spread.
pub fn style_system_to_style_settings(
    ss: &crate::design_system::StyleSystem,
    base_id: u8,
) -> StyleSettings {
    let base = style_defaults(base_id);

    // Density: 0.8 → compact (0), 1.2+ → roomy (2), anything else → normal (1).
    let density = if (ss.density.factor - 0.8_f32).abs() < 0.05 {
        0u8
    } else if ss.density.factor >= 1.15 {
        2u8
    } else {
        1u8
    };

    StyleSettings {
        // ── Radii ────────────────────────────────────────────────────────────
        // r_xs/r_sm/r_md/r_lg have a clean 1-to-1 source in StyleSystem.
        // r_pill does NOT: StyleSystem stores 9999.0 (conceptual "full round")
        // for all three styles, but style_defaults uses 0 (Meridien, sharp pill)
        // and 99 (Aperture/Octave). No consistent cap exists, so r_pill inherits
        // from style_defaults(base_id) via the ..base spread below.
        r_xs: ss.radii.xs as u8,
        r_sm: ss.radii.sm as u8,
        r_md: ss.radii.md as u8,
        r_lg: ss.radii.lg as u8,

        // ── Strokes ──────────────────────────────────────────────────────────
        // stroke_bold and stroke_thick map 1-to-1 for all three styles.
        // stroke_hair/thin/std do NOT map consistently: Meridien uses
        // strokes.hair/thin/std directly, but Aperture/Octave require a
        // one-tier shift (strokes.thin→stroke_hair, strokes.std→stroke_thin,
        // strokes.bold→stroke_std). Since no consistent cross-style mapping
        // exists, stroke_hair/thin/std inherit from style_defaults(base_id).
        stroke_bold:  ss.strokes.bold,
        stroke_thick: ss.strokes.thick,

        // ── Treatments ───────────────────────────────────────────────────────
        hairline_borders:         ss.treatments.hairline_borders,
        solid_active_fills:       ss.treatments.solid_active_fills,
        uppercase_section_labels: ss.treatments.uppercase_section_labels,

        // ── Spacing ──────────────────────────────────────────────────────────
        cta_height_px:  ss.spacing.cta_height,
        card_padding_y: ss.spacing.md,
        card_padding_x: ss.spacing.lg,

        // ── Typography ───────────────────────────────────────────────────────
        // font_caption maps to size_xs for all three styles (confirmed field-exact).
        // font_section_label does NOT: Aperture size_xs=9 but style_defaults(1)
        // font_section_label=10; it inherits from style_defaults(base_id).
        font_caption: ss.typography.size_xs,
        font_body:    ss.typography.size_sm,
        font_hero:    ss.typography.size_xl,

        // ── Density ──────────────────────────────────────────────────────────
        density,
        row_height_px: ss.density.row_height_dense,

        // ── Shadows ──────────────────────────────────────────────────────────
        // shadow_blur/offset_y/alpha have a clean source.
        // shadows_enabled cannot be derived from blur > 0: Meridien has blur=0
        // but style_defaults(0).shadows_enabled=true; Octave has blur > 0 but
        // style_defaults(2).shadows_enabled=false. Inherits from base.
        shadow_blur:     ss.shadows.card.blur,
        shadow_offset_y: ss.shadows.card.offset_y,
        shadow_alpha:    (ss.shadows.card.alpha * 255.0).round() as u8,

        // All remaining fields (r_pill, stroke_hair/thin/std, shadows_enabled,
        // font_section_label, and all other StyleSettings fields not carried
        // cleanly by StyleSystem) inherit from style_defaults(base_id).
        ..base
    }
}

// ─── Dynamic style preset store ──────────────────────────────────────────────
// Vec of (name, settings) pairs. Ids 0/1/2 are the canonical three styles
// (Meridien/Aperture/Octave) and cannot be deleted. User-added presets append
// beyond index 2 and survive only for the session (in-memory, no source write).

static STYLE_STORE: std::sync::OnceLock<std::sync::RwLock<Vec<(String, StyleSettings)>>> =
    std::sync::OnceLock::new();

fn style_store() -> &'static std::sync::RwLock<Vec<(String, StyleSettings)>> {
    STYLE_STORE.get_or_init(|| {
        // Source the 3 canonical styles from design_system::builtin_style_systems()
        // mapped through the adapter. This mirrors the colour-axis flip in gpu.rs
        // (live_themes → builtin_color_schemes). The adapter output is field-exact
        // with style_defaults(i) for all StyleSystem-carried fields (equivalence test
        // guarantees this); fields not in StyleSystem keep the style_defaults base.
        use crate::design_system::builtin_style_systems;
        let systems = builtin_style_systems();
        debug_assert_eq!(
            systems.len(), 3,
            "builtin_style_systems() must return exactly 3 entries (Meridien/Aperture/Octave)"
        );
        let mut v: Vec<(String, StyleSettings)> = systems
            .iter()
            .enumerate()
            .map(|(i, ss)| {
                (ss.meta.name.clone(), style_system_to_style_settings(ss, i as u8))
            })
            .collect();
        // Alias the remaining STYLE_NAMES (indices 3-9) to Meridien's settings
        // so existing style_idx values don't out-of-range on first lookup.
        let meridien = style_system_to_style_settings(&systems[0], 0);
        let alias_names = ["Cadence", "Chord", "Lattice", "Tangent", "Tempo", "Contour", "Relay"];
        for name in alias_names {
            v.push((name.to_string(), meridien.clone()));
        }
        std::sync::RwLock::new(v)
    })
}

/// Get a clone of the settings for style `id`. Falls back to 0 (Meridien) if out of range.
pub fn get_style_settings(id: u8) -> StyleSettings {
    let store = style_store().read().unwrap();
    let idx = id as usize;
    if idx < store.len() { store[idx].1.clone() } else { store[0].1.clone() }
}

/// Overwrite the settings for style `id` — takes effect on the next frame.
/// Silently ignored if `id` is out of range.
pub fn set_style_settings(id: u8, settings: StyleSettings) {
    let mut store = style_store().write().unwrap();
    let idx = id as usize;
    if idx < store.len() { store[idx].1 = settings; }
}

/// Add a new named preset cloned from an existing style. Returns the new id.
pub fn add_style_preset(name: &str, settings: StyleSettings) -> u8 {
    let mut store = style_store().write().unwrap();
    let id = store.len() as u8;
    store.push((name.to_string(), settings));
    id
}

/// Delete a user preset. Ids 0/1/2 are protected (no-op). All ids above the
/// deleted slot are shifted down — callers should re-read `list_style_presets`
/// and update any stored `style_idx` values accordingly.
pub fn delete_style_preset(id: u8) {
    if id < 3 { return; }
    let mut store = style_store().write().unwrap();
    let idx = id as usize;
    if idx < store.len() { store.remove(idx); }
}

/// Rename a preset in-place. No-op if `id` is out of range.
pub fn rename_style_preset(id: u8, new_name: String) {
    let mut store = style_store().write().unwrap();
    let idx = id as usize;
    if idx < store.len() { store[idx].0 = new_name; }
}

/// Returns `(id, name)` pairs for all registered presets — use for dropdowns.
pub fn list_style_presets() -> Vec<(u8, String)> {
    style_store().read().unwrap()
        .iter().enumerate()
        .map(|(i, (name, _))| (i as u8, name.clone()))
        .collect()
}

pub fn current() -> StyleSettings {
    let id = ACTIVE_STYLE.load(std::sync::atomic::Ordering::Relaxed);
    get_style_settings(id)
}

// Style-aware corner radius helpers — route through `current()` so corners
// flip when the active style changes (Meridien 0/0/0/0/0, Aperture 4/6/8/12/99,
// Octave 1/2/3/4/99). Previously these used static tokens which broke the
// style cascade — a popup using r_lg_cr() always got 8px regardless of style.
pub fn r_xs() -> egui::CornerRadius { egui::CornerRadius::same(current().r_xs) }
pub fn r_sm_cr() -> egui::CornerRadius { egui::CornerRadius::same(current().r_sm) }
pub fn r_md_cr() -> egui::CornerRadius { egui::CornerRadius::same(current().r_md) }
pub fn r_lg_cr() -> egui::CornerRadius { egui::CornerRadius::same(current().r_lg) }
pub fn r_pill() -> egui::CornerRadius { egui::CornerRadius::same(current().r_pill) }

pub fn btn_compact_height() -> f32 { 22.0 }
pub fn btn_simple_height() -> f32 { 24.0 }
pub fn btn_small_height() -> f32 { 22.0 }
pub fn btn_trade_height() -> f32 { 28.0 }

// ── New style-setting helpers ────────────────────────────────────────────────
/// Density-aware row height. Reads `row_height_px` then scales by density vscale.
pub fn style_row_height() -> f32 {
    let st = current();
    let scale = match st.density { 0 => 0.85, 2 => 1.15, _ => 1.0 };
    st.row_height_px * scale
}
/// Density-aware button height. Reads `button_height_px` then scales by density vscale.
pub fn style_button_height() -> f32 {
    let st = current();
    let scale = match st.density { 0 => 0.85, 2 => 1.15, _ => 1.0 };
    st.button_height_px * scale
}
/// Density-aware tab height. Reads `tab_height` then scales by density vscale.
pub fn style_tab_height() -> f32 {
    let st = current();
    let scale = match st.density { 0 => 0.85, 2 => 1.15, _ => 1.0 };
    st.tab_height * scale
}
/// Accent color with emphasis multiplier applied (brightness boost for active elements).
pub fn accent_emphasised(color: egui::Color32) -> egui::Color32 {
    color.gamma_multiply(current().accent_emphasis)
}

pub fn contrast_fg(bg: egui::Color32) -> egui::Color32 {
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if lum > 140.0 { egui::Color32::BLACK } else { egui::Color32::WHITE }
}

pub fn rule_stroke_for(_bg: egui::Color32, border: egui::Color32) -> egui::Stroke {
    // Use pane_border_width so Meridien hairlines honour the style knob.
    egui::Stroke::new(current().pane_border_width, border)
}

/// Paint a full-height inter-cluster vertical divider line in the toolbar (#4).
/// Call between button groups when `current().vertical_group_dividers` is true.
/// `panel_rect` should be the full toolbar panel rect for correct top/bottom span.
pub fn tb_group_break(ui: &mut egui::Ui, panel_rect: egui::Rect, border: egui::Color32) {
    if !current().vertical_group_dividers { return; }
    ui.add_space(gap_md());
    let x = ui.cursor().left();
    // Use alpha_heavy (120) for clearly visible dividers even on dim toolbar_border colors.
    let color = color_alpha(border, alpha_heavy());
    ui.painter().line_segment(
        [egui::pos2(x, panel_rect.top() + 2.0), egui::pos2(x, panel_rect.bottom() - 2.0)],
        egui::Stroke::new(stroke_std(), color),
    );
    ui.add_space(gap_md());
}

/// Returns `s` uppercased (and letter-spaced) for active styles that request it (#5, #12).
///
/// # Letter-spacing limitation
/// egui does not support CSS `letter-spacing`. We approximate it by inserting Unicode
/// thin-spaces (U+2009) between characters. Threshold:
///   < 0.5 px  → no spacing
///   0.5–1.5 px → single thin-space between each char
///   > 1.5 px  → double thin-space between each char
/// This is a visual approximation; the effective gap depends on font rendering.
pub fn style_label_case(s: &str) -> String {
    let st = current();
    let base = if st.uppercase_section_labels { s.to_uppercase() } else { s.to_string() };
    // Apply letter-spacing approximation via Unicode thin-spaces (U+2009).
    let sp = st.label_letter_spacing_px;
    if sp < 0.5 {
        base
    } else {
        let sep = if sp > 1.5 { "\u{2009}\u{2009}" } else { "\u{2009}" };
        base.chars().map(|c| c.to_string()).collect::<Vec<_>>().join(sep)
    }
}

/// Returns a `FontId` appropriate for hero numerics — serif when the active
/// style requests it, monospace otherwise (#14).
pub fn hero_font_id(size: f32) -> egui::FontId {
    if current().serif_headlines {
        egui::FontId::new(size, egui::FontFamily::Name("serif".into()))
    } else {
        egui::FontId::monospace(size)
    }
}

/// Builds a `RichText` for large numeric displays using the hero font (#14).
pub fn hero_text(text: &str, color: egui::Color32) -> egui::RichText {
    let size = current().font_hero;
    egui::RichText::new(text).font(hero_font_id(size)).color(color)
}

/// Apply per-style egui::Style overrides (widget visuals, spacing, shadows)
/// to the given context. Call once per frame after `set_active_style` (#3).
///
/// This is intentionally a *supplement* to the rich visual block already
/// applied in `setup_theme`; it only overrides the fields that differ
/// between styles so that non-Meridien themes remain visually unchanged.
pub fn apply_ui_style(ctx: &egui::Context, settings: &StyleSettings, toolbar_border: egui::Color32, toolbar_bg: egui::Color32, accent: egui::Color32) {
    let mut style = (*ctx.style()).clone();
    let is_meridien = settings.hairline_borders && settings.serif_headlines;

    if is_meridien {
        // Meridien widget fills: transparent inactive, flat hairline borders
        let inact = &mut style.visuals.widgets.inactive;
        inact.bg_fill      = egui::Color32::TRANSPARENT;
        inact.weak_bg_fill = egui::Color32::TRANSPARENT;
        inact.bg_stroke    = egui::Stroke::new(stroke_std(), color_alpha(toolbar_border, 70));
        inact.corner_radius = egui::CornerRadius::ZERO;

        let hov = &mut style.visuals.widgets.hovered;
        hov.bg_fill      = color_alpha(toolbar_border, 18);
        hov.corner_radius = egui::CornerRadius::ZERO;

        let act = &mut style.visuals.widgets.active;
        act.corner_radius = egui::CornerRadius::ZERO;

        let open = &mut style.visuals.widgets.open;
        open.corner_radius = egui::CornerRadius::ZERO;

        // Shadows: keep the values applied in setup_theme — Meridien previously
        // zeroed these (#16) but that left ComboBox/menus with no depth cue.
        // The setup_theme values are already subtle and theme-appropriate.
        style.visuals.window_stroke = egui::Stroke::new(settings.stroke_std, toolbar_border);
        style.visuals.window_corner_radius = egui::CornerRadius::ZERO;
        style.visuals.menu_corner_radius   = egui::CornerRadius::ZERO;

        // Denser editorial spacing
        style.spacing.button_padding   = egui::vec2(gap_xl(), gap_xs());
        style.spacing.menu_margin      = egui::Margin { left: gap_md() as i8, right: gap_md() as i8, top: gap_sm() as i8, bottom: gap_sm() as i8 };
        style.spacing.interact_size.y  = 22.0;
        style.spacing.item_spacing     = egui::vec2(gap_sm(), gap_xs());
    }

    // input_focus_color: derived from accent (§3.2 — no per-style override).
    style.visuals.selection.stroke = egui::Stroke::new(settings.focus_ring_width, accent);

    ctx.set_style(style);
    let _ = (toolbar_bg,); // may be used in future for popup fill overrides
}

// ─── #19 chrome_tile_btn ──────────────────────────────────────────────────────

/// State passed to [`paint_chrome_tile_button`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ChromeTileState { Idle, Hovered, Active }

/// Paint the small square chrome tile button used for "+Tab" and template/star
/// buttons in pane headers. Uses `current().r_md` (0 for Meridien, rounded
/// otherwise) and `current().stroke_thin`.
///
/// Returns nothing — the caller owns the `Response` and acts on clicks.
///
/// # Example
/// ```ignore
/// let resp = ui.allocate_rect(rect, egui::Sense::click());
/// let state = if resp.hovered() { ChromeTileState::Hovered } else { ChromeTileState::Idle };
/// paint_chrome_tile_button(&ui.painter().with_clip_rect(rect), rect, state, t);
/// ```
pub fn paint_chrome_tile_button(
    painter: &egui::Painter,
    rect: egui::Rect,
    state: ChromeTileState,
    t: &crate::chart_renderer::gpu::Theme,
) {
    let cr = egui::CornerRadius::same(current().r_md);
    let sw = current().stroke_thin;
    let (bg, border) = match state {
        ChromeTileState::Active  => (
            color_alpha(t.accent, 38),
            color_alpha(t.accent, alpha_active()),
        ),
        ChromeTileState::Hovered => (
            color_alpha(t.toolbar_border, alpha_subtle()),
            color_alpha(t.accent, alpha_line()),
        ),
        ChromeTileState::Idle    => (
            color_alpha(t.toolbar_border, 18),
            color_alpha(t.toolbar_border, alpha_muted()),
        ),
    };
    painter.rect_filled(rect, cr, bg);
    painter.rect_stroke(rect, cr, egui::Stroke::new(sw, border),
        egui::StrokeKind::Outside);
}

// ─── Border stroke shorthands ─────────────────────────────────────────────────

/// Standard 1px border stroke using `t.toolbar_border`. Covers 90% of separator / divider use.
#[inline]
pub fn border_stroke(t: &crate::chart_renderer::gpu::Theme) -> Stroke {
    Stroke::new(stroke_std(), t.toolbar_border)
}

/// Hair-width border stroke for dense / compact UI regions.
#[inline]
pub fn border_stroke_thin(t: &crate::chart_renderer::gpu::Theme) -> Stroke {
    Stroke::new(stroke_thin(), t.toolbar_border)
}

// ─── Icon button size tokens ──────────────────────────────────────────────────

/// 16×16 — small icon button (close, delete, inline action).
pub const BTN_ICON_SM: egui::Vec2 = egui::vec2(16.0, 16.0);
/// 24×24 — standard icon button (toolbar action, panel header icon).
pub const BTN_ICON_MD: egui::Vec2 = egui::vec2(24.0, 24.0);
/// 32×24 — wide icon button (split actions, nav arrows with extra hit area).
pub const BTN_ICON_LG: egui::Vec2 = egui::vec2(32.0, 24.0);

/// Foreground color for a [`ChromeTileState`] — pair with [`paint_chrome_tile_button`].
pub fn chrome_tile_fg(state: ChromeTileState, t: &crate::chart_renderer::gpu::Theme) -> egui::Color32 {
    match state {
        ChromeTileState::Active  => t.accent,
        ChromeTileState::Hovered => t.text,
        ChromeTileState::Idle    => t.dim.gamma_multiply(0.8),
    }
}

// ─── DS-IMPL-3 token tests ───────────────────────────────────────────────────

#[cfg(test)]
mod ds_impl_3_tests {
    use super::*;

    /// gap_xs_mid() must return 6.0 (sits between gap_xs=4.0 and gap_sm=8.0).
    #[test]
    fn gap_xs_mid_literal() {
        // design-mode is off in tests by default, so the dt_f32! macro falls
        // through to its compile-time default.
        assert_eq!(gap_xs_mid(), 6.0_f32);
    }

    /// GAP_XS_MID const must match the function's fallback value.
    #[test]
    fn gap_xs_mid_const_matches_fn() {
        assert_eq!(GAP_XS_MID, gap_xs_mid());
    }

    /// stroke_medium() must return 0.8 — the promoted design-token value.
    #[test]
    fn stroke_medium_literal() {
        assert_eq!(stroke_medium(), 0.8_f32);
    }

    /// STROKE_MEDIUM const must match the function fallback.
    #[test]
    fn stroke_medium_const_matches_fn() {
        assert_eq!(STROKE_MEDIUM, stroke_medium());
    }

    /// elevation_1/2/3 must return theme.bg gamma-multiplied by the documented
    /// perceptual constants (0.95 / 0.88 / 0.85).
    #[test]
    fn elevation_tints_use_correct_gamma() {
        let t = crate::chart_renderer::gpu::THEMES
            .iter().find(|t| t.name == "Midnight")
            .expect("Midnight theme must exist");
        let expected_1 = t.bg.gamma_multiply(0.95);
        let expected_2 = t.bg.gamma_multiply(0.88);
        let expected_3 = t.bg.gamma_multiply(0.85);
        assert_eq!(elevation_1(t), expected_1, "elevation_1 gamma constant");
        assert_eq!(elevation_2(t), expected_2, "elevation_2 gamma constant");
        assert_eq!(elevation_3(t), expected_3, "elevation_3 gamma constant");
    }

    /// elevation_1 must be brighter (higher in RGBA sum) than elevation_2,
    /// which must be brighter than elevation_3, for a typical dark background.
    #[test]
    fn elevation_depth_order_is_monotonic() {
        let t = crate::chart_renderer::gpu::THEMES
            .iter().find(|t| t.name == "Midnight")
            .expect("Midnight theme must exist");
        // Sum RGB channels as a proxy for luminance.
        let lum = |c: egui::Color32| c.r() as u32 + c.g() as u32 + c.b() as u32;
        // On dark bg, gamma_multiply < 1 darkens — so lum(e1) >= lum(e2) >= lum(e3).
        // (They may be equal if bg is black, but that's not the case for real themes.)
        assert!(lum(elevation_1(t)) >= lum(elevation_2(t)),
            "elevation_1 should be >= elevation_2 in luminance");
        assert!(lum(elevation_2(t)) >= lum(elevation_3(t)),
            "elevation_2 should be >= elevation_3 in luminance");
    }
}
