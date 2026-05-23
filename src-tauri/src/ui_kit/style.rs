//! Pure design-token primitives — owned by `ui_kit`.
//!
//! Part of the UI extraction (see `docs/UI_EXTRACTION.md` item 1 —
//! "Physical token move"). This module contains the **stateless** token
//! helpers: font sizes, spacing, stroke widths, alphas, radii, elevation
//! factors, plus `color_alpha` / `color_dim` color utilities. All values
//! are pure constants or constant expressions — no `FRAME_TOKENS`
//! thread-local read, no `crate::chart_renderer` reference.
//!
//! Stateful style machinery (`FRAME_TOKENS`, `STYLE_STORE`, `ACTIVE_STYLE`,
//! the `Theme`-taking helpers like `header_surface(t)`) stays in
//! `chart::renderer::ui::style` — those depend on the chart-app's style
//! preset system. When `ui_kit` extracts to its own crate, this file
//! comes with it; the stateful pieces stay in the trading app.
//!
//! `ui_kit::tokens` re-exports both modules so widgets get a single
//! import surface (`use crate::ui_kit::tokens::{font_sm, gap_xs, ...}`).

use egui::Color32;
use std::cell::Cell;

// ─── Per-frame token snapshot (UI extraction, item F) ────────────────────────
//
// Lock-free thread_local holding a Copy struct of every design-token value.
// Hosts (the chart app, or any embedder of ui_kit) refresh this once per frame
// via `set_frame_tokens(snap)` before any UI is built. Token-reading helpers
// (`gap_xs_mid`, `radius_xs..lg`, `stroke_hair..thick`, `alpha_faint..solid`)
// resolve through `frame_tokens()` and get the per-frame value with zero
// allocation. First frame (before any setter call) returns
// `DEFAULT_TOKEN_SNAPSHOT` — the values match the function-body constants for
// the unstyled defaults so visuals are identical until a host pushes its
// snapshot.

#[derive(Clone, Copy, Debug)]
pub struct TokenSnapshot {
    pub gap_xs_mid: f32,
    pub radius_xs: f32,
    pub radius_sm: f32,
    pub radius_md: f32,
    pub radius_lg: f32,
    pub stroke_hair:   f32,
    pub stroke_thin:   f32,
    pub stroke_medium: f32,
    pub stroke_std:    f32,
    pub stroke_bold:   f32,
    pub stroke_thick:  f32,
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
    pub shadow_offset: f32,
    pub shadow_alpha:  u8,
    pub shadow_spread: f32,
}

/// Compile-time defaults — match every token fn's non-design-mode constant
/// so the first frame (before any host calls `set_frame_tokens`) returns
/// identical values.
pub const DEFAULT_TOKEN_SNAPSHOT: TokenSnapshot = TokenSnapshot {
    gap_xs_mid: 6.0,
    radius_xs: 2.0, radius_sm: 4.0, radius_md: 6.0, radius_lg: 12.0,
    stroke_hair: 0.3, stroke_thin: 0.5, stroke_medium: 0.8,
    stroke_std: 1.0, stroke_bold: 1.5, stroke_thick: 2.0,
    alpha_faint: 10, alpha_ghost: 15, alpha_soft: 20, alpha_subtle: 40,
    alpha_tint: 48, alpha_muted: 60, alpha_dim: 60, alpha_line: 80,
    alpha_strong: 80, alpha_active: 100, alpha_heavy: 120, alpha_solid: 200,
    shadow_offset: 2.0, shadow_alpha: 60, shadow_spread: 4.0,
};

thread_local! {
    static FRAME_TOKENS_LOCAL: Cell<TokenSnapshot> = Cell::new(DEFAULT_TOKEN_SNAPSHOT);
}

/// Host-side: stash this frame's `TokenSnapshot`. Call once per frame from
/// the render loop, before any UI is built. Cheap — one Cell write.
#[inline]
pub fn set_frame_tokens(snap: TokenSnapshot) {
    FRAME_TOKENS_LOCAL.with(|c| c.set(snap));
}

/// Widget-side: read the current frame's `TokenSnapshot`. Returns
/// `DEFAULT_TOKEN_SNAPSHOT` if no host has pushed one this frame.
#[inline]
pub fn frame_tokens() -> TokenSnapshot {
    FRAME_TOKENS_LOCAL.with(|c| c.get())
}

// ─── FRAME_TOKENS-backed token helpers ───────────────────────────────────────
// Thin accessors over `frame_tokens().field`. Hosts that don't push a
// snapshot get the `DEFAULT_TOKEN_SNAPSHOT` values, which match the
// stand-alone constants below.

#[inline] pub fn gap_xs_mid() -> f32 { frame_tokens().gap_xs_mid }
#[inline] pub fn radius_xs()  -> f32 { frame_tokens().radius_xs }
#[inline] pub fn radius_sm()  -> f32 { frame_tokens().radius_sm }
#[inline] pub fn radius_md()  -> f32 { frame_tokens().radius_md }
#[inline] pub fn radius_lg()  -> f32 { frame_tokens().radius_lg }

#[inline] pub fn stroke_hair()   -> f32 { frame_tokens().stroke_hair }
#[inline] pub fn stroke_thin()   -> f32 { frame_tokens().stroke_thin }
#[inline] pub fn stroke_medium() -> f32 { frame_tokens().stroke_medium }
#[inline] pub fn stroke_std()    -> f32 { frame_tokens().stroke_std }
#[inline] pub fn stroke_bold()   -> f32 { frame_tokens().stroke_bold }
#[inline] pub fn stroke_thick()  -> f32 { frame_tokens().stroke_thick }

#[inline] pub fn alpha_faint()   -> u8 { frame_tokens().alpha_faint }
#[inline] pub fn alpha_ghost()   -> u8 { frame_tokens().alpha_ghost }
#[inline] pub fn alpha_soft()    -> u8 { frame_tokens().alpha_soft }
#[inline] pub fn alpha_subtle()  -> u8 { frame_tokens().alpha_subtle }
#[inline] pub fn alpha_tint()    -> u8 { frame_tokens().alpha_tint }
#[inline] pub fn alpha_muted()   -> u8 { frame_tokens().alpha_muted }
#[inline] pub fn alpha_dim()     -> u8 { frame_tokens().alpha_dim }
#[inline] pub fn alpha_line()    -> u8 { frame_tokens().alpha_line }
#[inline] pub fn alpha_strong()  -> u8 { frame_tokens().alpha_strong }
#[inline] pub fn alpha_active()  -> u8 { frame_tokens().alpha_active }
#[inline] pub fn alpha_heavy()   -> u8 { frame_tokens().alpha_heavy }
#[inline] pub fn alpha_solid()   -> u8 { frame_tokens().alpha_solid }

// ─── Monospace font helpers ──────────────────────────────────────────────────
// Tabular financial data (prices, qty, OCC tickers, kbd labels). Returns
// FontId so the family is explicit at the call site.
#[inline] pub fn mono_4xs() -> egui::FontId { egui::FontId::new(font_4xs(), egui::FontFamily::Monospace) }
#[inline] pub fn mono_3xs() -> egui::FontId { egui::FontId::new(font_3xs(), egui::FontFamily::Monospace) }
#[inline] pub fn mono_2xs() -> egui::FontId { egui::FontId::new(font_2xs(), egui::FontFamily::Monospace) }
#[inline] pub fn mono_xs()  -> egui::FontId { egui::FontId::new(font_xs(),  egui::FontFamily::Monospace) }
#[inline] pub fn mono_xs_plus() -> egui::FontId { egui::FontId::new(font_xs_plus(), egui::FontFamily::Monospace) }
#[inline] pub fn mono_sm()  -> egui::FontId { egui::FontId::new(font_sm(),  egui::FontFamily::Monospace) }
#[inline] pub fn mono_md()  -> egui::FontId { egui::FontId::new(font_md(),  egui::FontFamily::Monospace) }
#[inline] pub fn mono_md_plus() -> egui::FontId { egui::FontId::new(font_md_plus(), egui::FontFamily::Monospace) }
#[inline] pub fn mono_lg()  -> egui::FontId { egui::FontId::new(font_lg(),  egui::FontFamily::Monospace) }

// ─── Contrast / readability helpers ──────────────────────────────────────────

/// Pick BLACK or WHITE foreground for the given background based on perceived
/// luminance. Used by chips, badges, status pills — anywhere a swatch needs a
/// readable label without the caller hand-picking the fg.
#[inline]
pub fn contrast_fg(bg: Color32) -> Color32 {
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if lum > 140.0 { Color32::BLACK } else { Color32::WHITE }
}

// ─── Display-tier proportional fonts ─────────────────────────────────────────
// Large hero numbers in chart-widget bodies (KPIs, countdown digits).
#[inline] pub fn font_display_sm() -> f32 { 28.0 }
#[inline] pub fn font_display_md() -> f32 { 32.0 }
#[inline] pub fn font_display_lg() -> f32 { 42.0 }
#[inline] pub fn font_display_xl() -> f32 { 56.0 }

// ─── Icon control sizes ──────────────────────────────────────────────────────
#[inline] pub fn icon_xs() -> f32 { 14.0 }
#[inline] pub fn icon_sm() -> f32 { 16.0 }
#[inline] pub fn icon_md() -> f32 { 18.0 }
#[inline] pub fn icon_lg() -> f32 { 20.0 }

// ─── Row heights ─────────────────────────────────────────────────────────────
#[inline] pub fn row_height_dense()     -> f32 { 18.0 }
#[inline] pub fn row_height_compact()   -> f32 { 20.0 }
#[inline] pub fn row_height_default()   -> f32 { 22.0 }
#[inline] pub fn row_height_spacious()  -> f32 { 24.0 }
#[inline] pub fn row_height_tall()      -> f32 { 30.0 }

// ─── Card padding ────────────────────────────────────────────────────────────
#[inline] pub fn card_padding_compact()  -> f32 { 8.0 }
#[inline] pub fn card_padding_default()  -> f32 { 12.0 }
#[inline] pub fn card_padding_spacious() -> f32 { 16.0 }

// ─── Divider insets ──────────────────────────────────────────────────────────
#[inline] pub fn divider_inset_xs() -> f32 { 1.0 }
#[inline] pub fn divider_inset_sm() -> f32 { 2.0 }
#[inline] pub fn divider_inset_md() -> f32 { 3.0 }
#[inline] pub fn divider_inset_lg() -> f32 { 5.0 }



// ─── Font sizes (px) ─────────────────────────────────────────────────────────

pub fn font_4xs()    -> f32 { 6.0 }
pub fn font_3xs()    -> f32 { 7.0 }
pub fn font_2xs()    -> f32 { 8.0 }
pub fn font_xs()     -> f32 { 9.0 }
pub fn font_xs_plus() -> f32 { 10.0 }
pub fn font_sm()     -> f32 { 11.0 }
pub fn font_md()     -> f32 { 13.0 }
pub fn font_md_plus() -> f32 { 14.0 }
pub fn font_lg()     -> f32 { 16.0 }
pub fn font_xl()     -> f32 { 22.0 }

pub const FONT_DISPLAY_SM: f32 = 28.0;
pub const FONT_DISPLAY_MD: f32 = 32.0;
pub const FONT_DISPLAY_LG: f32 = 42.0;
pub const FONT_DISPLAY_XL: f32 = 56.0;

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

// ─── Spacing (px) ────────────────────────────────────────────────────────────

pub fn gap_2xs() -> f32 { 2.0 }
pub fn gap_xs()  -> f32 { 4.0 }
pub fn gap_sm()  -> f32 { 8.0 }
pub fn gap_md()  -> f32 { 12.0 }
pub fn gap_lg()  -> f32 { 16.0 }
pub fn gap_xl()  -> f32 { 20.0 }
pub fn gap_2xl() -> f32 { 24.0 }
pub fn gap_3xl() -> f32 { 32.0 }

pub const GAP_2XS:    f32 =  2.0;
pub const GAP_XS:     f32 =  4.0;
pub const GAP_SM:     f32 =  8.0;
pub const GAP_MD:     f32 = 12.0;
pub const GAP_LG:     f32 = 16.0;
pub const GAP_XL:     f32 = 20.0;
pub const GAP_2XL:    f32 = 24.0;
pub const GAP_3XL:    f32 = 32.0;

// ─── Stroke widths (px) — pure constants ─────────────────────────────────────

pub fn stroke_extra_thick() -> f32 { 2.5 }
pub fn stroke_heavy()       -> f32 { 3.0 }

// ─── Radii (px) — pure constants ─────────────────────────────────────────────

pub fn radius_pill() -> f32 { 999.0 }

// ─── Alpha (0..=255) — pure constants ────────────────────────────────────────

pub fn alpha_whisper() -> u8 { 25 }
pub fn alpha_hint()    -> u8 { 30 }

// ─── Elevation factors (gamma multipliers over `bg()`) ───────────────────────
//
// Used by `ComponentTheme::header_surface()` / `section_header_surface()` /
// `panel_surface()` default impls so the elevation ramp lives in one place
// and is portable across themes.

pub const ELEVATION_1_FACTOR: f32 = 0.95;
pub const ELEVATION_2_FACTOR: f32 = 0.88;
pub const ELEVATION_3_FACTOR: f32 = 0.85;

// ─── Color utilities — pure egui math, no theme/state ────────────────────────

/// Return `c` with its alpha replaced by `a`. Convenience wrapper around
/// `Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)`.
#[inline]
pub fn color_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Linearly multiply the alpha channel by `factor` (clamped 0..=1).
#[inline]
pub fn color_alpha_mul(c: Color32, factor: f32) -> Color32 {
    let new_a = ((c.a() as f32) * factor.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), new_a)
}

// ─── Color dimming helpers ───────────────────────────────────────────────────
// Mirror the chart-app's named multipliers (subtle / muted / half / etc.) so
// widgets get a portable path to the same dim effect without reaching into
// `chart_renderer::ui::style`.

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
