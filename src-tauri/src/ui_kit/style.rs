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
    // Font sizes (proportional).
    pub font_2xs:    f32,
    pub font_xs:     f32,
    pub font_sm:     f32,
    pub font_md:     f32,
    pub font_lg:     f32,
    pub font_xl:     f32,
    // Spacing.
    pub gap_xs:     f32,
    pub gap_xs_mid: f32,
    pub gap_sm:     f32,
    pub gap_md:     f32,
    pub gap_lg:     f32,
    pub gap_xl:     f32,
    pub gap_2xl:    f32,
    pub gap_3xl:    f32,
    // Radii.
    pub radius_xs: f32,
    pub radius_sm: f32,
    pub radius_md: f32,
    pub radius_lg: f32,
    // Strokes.
    pub stroke_hair:   f32,
    pub stroke_thin:   f32,
    pub stroke_medium: f32,
    pub stroke_std:    f32,
    pub stroke_bold:   f32,
    pub stroke_thick:  f32,
    // Alphas.
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
    // Shadows.
    pub shadow_offset: f32,
    pub shadow_alpha:  u8,
    pub shadow_spread: f32,
}

/// Compile-time defaults — match every token fn's non-design-mode constant
/// so the first frame (before any host calls `set_frame_tokens`) returns
/// identical values.
pub const DEFAULT_TOKEN_SNAPSHOT: TokenSnapshot = TokenSnapshot {
    // Fonts.
    font_2xs: 8.0, font_xs: 9.0, font_sm: 11.0, font_md: 13.0, font_lg: 16.0, font_xl: 22.0,
    // Spacing.
    gap_xs: 4.0, gap_xs_mid: 6.0, gap_sm: 8.0, gap_md: 12.0,
    gap_lg: 16.0, gap_xl: 20.0, gap_2xl: 24.0, gap_3xl: 32.0,
    // Radii.
    radius_xs: 2.0, radius_sm: 4.0, radius_md: 6.0, radius_lg: 12.0,
    // Strokes.
    stroke_hair: 0.3, stroke_thin: 0.5, stroke_medium: 0.8,
    stroke_std: 1.0, stroke_bold: 1.5, stroke_thick: 2.0,
    // Alphas.
    alpha_faint: 10, alpha_ghost: 15, alpha_soft: 20, alpha_subtle: 40,
    alpha_tint: 48, alpha_muted: 60, alpha_dim: 60, alpha_line: 80,
    alpha_strong: 80, alpha_active: 100, alpha_heavy: 120, alpha_solid: 200,
    // Shadows.
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
// Large hero numbers in chart-widget bodies (KPIs, countdown digits). Wired
// through the design_tokens macro so the display tier is themable (P3.1).
// FontTokens.xxl / .display / .display_lg are the corresponding TOML fields
// (with naming inherited from an earlier scheme — kept for back-compat).
#[inline] pub fn font_display_sm() -> f32 { crate::dt_f32!(font.xxl,        28.0) }
#[inline] pub fn font_display_md() -> f32 { crate::dt_f32!(font.display,    32.0) }
#[inline] pub fn font_display_lg() -> f32 { crate::dt_f32!(font.display_lg, 42.0) }
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

// ─── Uppercase alpha constants (compile-time fallbacks) ──────────────────────
pub const ALPHA_FAINT:   u8 =  10;
pub const ALPHA_GHOST:   u8 =  15;
pub const ALPHA_SOFT:    u8 =  20;
pub const ALPHA_WHISPER: u8 =  25;
pub const ALPHA_HINT:    u8 =  30;
pub const ALPHA_SUBTLE:  u8 =  40;
pub const ALPHA_TINT:    u8 =  48;
pub const ALPHA_MUTED:   u8 =  60;
pub const ALPHA_DIM:     u8 =  60;
pub const ALPHA_LINE:    u8 =  80;
pub const ALPHA_STRONG:  u8 =  80;
pub const ALPHA_ACTIVE:  u8 = 100;
pub const ALPHA_HEAVY:   u8 = 120;
pub const ALPHA_SOLID:   u8 = 200;

// ─── Uppercase stroke constants ──────────────────────────────────────────────
pub const STROKE_HAIR:        f32 = 0.3;
pub const STROKE_THIN:        f32 = 0.5;
pub const STROKE_MEDIUM:      f32 = 0.8;
pub const STROKE_STD:         f32 = 1.0;
pub const STROKE_BOLD:        f32 = 1.5;
pub const STROKE_THICK:       f32 = 2.0;
pub const STROKE_EXTRA_THICK: f32 = 2.5;
pub const STROKE_HEAVY:       f32 = 3.0;

// ─── CornerRadius helpers — `r_xs/sm/md/lg_cr()` returning egui::CornerRadius
//     from the per-frame radius tokens. Saves callers a cast at every site.
#[inline] pub fn r_xs_cr() -> egui::CornerRadius { egui::CornerRadius::same(radius_xs() as u8) }
#[inline] pub fn r_sm_cr() -> egui::CornerRadius { egui::CornerRadius::same(radius_sm() as u8) }
#[inline] pub fn r_md_cr() -> egui::CornerRadius { egui::CornerRadius::same(radius_md() as u8) }
#[inline] pub fn r_lg_cr() -> egui::CornerRadius { egui::CornerRadius::same(radius_lg() as u8) }

// ─── Shadow geometry helpers ─────────────────────────────────────────────────
#[inline] pub fn shadow_offset() -> f32 { frame_tokens().shadow_offset }
#[inline] pub fn shadow_alpha()  -> u8  { frame_tokens().shadow_alpha }
#[inline] pub fn shadow_spread() -> f32 { frame_tokens().shadow_spread }

/// `color_alpha` over an explicit color — convenience for shadow paints that
/// already have the shadow color resolved (e.g. from `theme.shadow_color()`).
/// Mirrors the chart-app's free `shadow_color_alpha(t, a)`; this version takes
/// the resolved color directly so the body is portable.
#[inline]
pub fn shadow_color_alpha_of(shadow_color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(shadow_color.r(), shadow_color.g(), shadow_color.b(), alpha)
}



// ─── Font sizes (px) ─────────────────────────────────────────────────────────
// `font_2xs/xs/sm/md/lg/xl` read from the per-frame `TokenSnapshot` so
// design-mode inspector edits to `font.*` propagate live (chart-app's
// `begin_frame` syncs DesignTokens → snapshot once per frame). `font_4xs`
// / `font_3xs` / `font_xs_plus` / `font_md_plus` stay as constants — the
// DesignTokens font set doesn't expose those tiers (yet).

#[inline] pub fn font_4xs()    -> f32 { 6.0 }
#[inline] pub fn font_3xs()    -> f32 { 7.0 }
#[inline] pub fn font_2xs()    -> f32 { frame_tokens().font_2xs }
#[inline] pub fn font_xs()     -> f32 { frame_tokens().font_xs }
#[inline] pub fn font_xs_plus() -> f32 { 10.0 }
#[inline] pub fn font_sm()     -> f32 { frame_tokens().font_sm }
#[inline] pub fn font_md()     -> f32 { frame_tokens().font_md }
#[inline] pub fn font_md_plus() -> f32 { 14.0 }
#[inline] pub fn font_lg()     -> f32 { frame_tokens().font_lg }
#[inline] pub fn font_xl()     -> f32 { frame_tokens().font_xl }

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

// ─── Spacing (px) ────────────────────────────────────────────────────────────

// `gap_2xs` stays a constant (no DesignTokens equivalent — used only for
// icon-internal padding, ~2px). The rest read from the per-frame snapshot.
#[inline] pub fn gap_2xs() -> f32 { 2.0 }
#[inline] pub fn gap_xs()  -> f32 { frame_tokens().gap_xs }
#[inline] pub fn gap_sm()  -> f32 { frame_tokens().gap_sm }
#[inline] pub fn gap_md()  -> f32 { frame_tokens().gap_md }
#[inline] pub fn gap_lg()  -> f32 { frame_tokens().gap_lg }
#[inline] pub fn gap_xl()  -> f32 { frame_tokens().gap_xl }
#[inline] pub fn gap_2xl() -> f32 { frame_tokens().gap_2xl }
#[inline] pub fn gap_3xl() -> f32 { frame_tokens().gap_3xl }

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

// ─── Line-height multipliers (P2.5) ─────────────────────────────────────────
//
// Multipliers applied to font size to derive line height. Replace the bare
// floats scattered through TextSpec in chart/renderer/ui/foundation/text_style.rs
// and any per-call `RichText::new(..).size(s).line_height(s * 1.3)` patterns.

/// 1.2 — display / hero text, tight stack.
#[inline] pub fn line_tight()   -> f32 { 1.2  }
/// 1.25 — large heading.
#[inline] pub fn line_heading() -> f32 { 1.25 }
/// 1.3 — caption / label / mono.
#[inline] pub fn line_dense()   -> f32 { 1.3  }
/// 1.35 — small body / small mono.
#[inline] pub fn line_compact() -> f32 { 1.35 }
/// 1.4 — body / readable copy. Default for paragraph text.
#[inline] pub fn line_normal()  -> f32 { 1.4  }
/// 1.5 — loose / generously-spaced paragraph.
#[inline] pub fn line_loose()   -> f32 { 1.5  }

// ─── Motion timing (P2.5) ────────────────────────────────────────────────────
//
// Standardised animation durations in milliseconds. Use these instead of
// inline magic numbers when adding animations.

/// 0 ms — no animation; snap instantly.
#[inline] pub fn motion_instant() -> u32 {   0 }
/// 80 ms — micro-interactions (hover state transitions).
#[inline] pub fn motion_fast()    -> u32 {  80 }
/// 160 ms — standard UI transitions (open/close, slide, fade).
#[inline] pub fn motion_std()     -> u32 { 160 }
/// 240 ms — comfortable / non-urgent transitions.
#[inline] pub fn motion_slow()    -> u32 { 240 }
/// 400 ms — emphasis / decorative animations (rarely used).
#[inline] pub fn motion_xslow()   -> u32 { 400 }

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
