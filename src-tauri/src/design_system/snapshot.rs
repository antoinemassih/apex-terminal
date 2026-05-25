//! `DesignSnapshot` — the per-frame flat resolved token struct.
//!
//! ## Purpose
//!
//! `style.rs` exposes stable token functions (`font_sm()`, `gap_md()`, …) whose
//! signatures must never change (Rule 2 of the spec — `core.rs` calls them
//! directly).  In Phase B2 those functions will be backed by a `thread_local`
//! holding a `DesignSnapshot` refreshed once per frame.
//!
//! The snapshot is a **flat `Copy` struct** of resolved `f32` / `u8` primitives
//! — no `String`, no `Vec`, no heap allocation.  A `thread_local` read is ~1 ns
//! and lock-free (uses `Cell<DesignSnapshot>`).
//!
//! ## Constraints (spec §5)
//! - `#[derive(Clone, Copy)]` — must be `Copy`.
//! - Only primitive types: `f32`, `u8`, `bool`.
//! - `DEFAULT_SNAPSHOT` is a `const` so the `thread_local` initializer is free.
//!
//! ## Phase note
//! In Phase B1 this struct is defined and its resolver is wired, but the
//! `thread_local` pump and `style.rs` rewrites happen in Phase B2.

use super::{color_scheme::ColorScheme, style_system::StyleSystem};

// ── DesignSnapshot ────────────────────────────────────────────────────────────

/// Flat resolved token values for one frame.
///
/// Produced by [`snapshot`] from a `(StyleSystem, ColorScheme)` pair.
/// Consumed in O(1) by token read sites (see spec §5 Rule 2).
///
/// Field naming mirrors the existing `style.rs` public function names so
/// Phase B2 can do a mechanical rename of bodies without touching call sites.
///
/// This struct is a **superset** of `TokenSnapshot` in `style.rs` — it carries
/// every token that `TokenSnapshot` does plus the design-system-only fields.
#[derive(Clone, Copy, Debug)]
pub struct DesignSnapshot {
    // ── Typography ──────────────────────────────────────────────────────────
    pub size_xs: f32,
    pub size_sm: f32,
    pub size_md: f32,
    pub size_lg: f32,
    pub size_xl: f32,
    pub mono_sm: f32,
    pub mono_md: f32,
    pub mono_lg: f32,

    // ── Spacing ─────────────────────────────────────────────────────────────
    pub gap_xs:    f32,
    /// 6.0 — micro-gap between `gap_xs` (4) and `gap_sm` (8). Mirrors
    /// `TokenSnapshot::gap_xs_mid` / `spacing.xs_mid` (DS-IMPL-3).
    pub gap_xs_mid: f32,
    pub gap_sm:    f32,
    pub gap_md:    f32,
    pub gap_lg:    f32,
    pub gap_xl:    f32,
    pub gap_xxl:   f32,
    pub gmd:       f32,
    pub cta_height: f32,

    // ── Radii ───────────────────────────────────────────────────────────────
    pub radius_none: f32,
    pub radius_xs:   f32,
    pub radius_sm:   f32,
    pub radius_md:   f32,
    pub radius_lg:   f32,

    // ── Strokes ─────────────────────────────────────────────────────────────
    /// Sub-pixel hairline. Mirrors `TokenSnapshot::stroke_hair`.
    pub stroke_hair:   f32,
    pub stroke_thin:   f32,
    /// Mid-weight border tier. Mirrors `TokenSnapshot::stroke_medium` (DS-IMPL-3).
    pub stroke_medium: f32,
    pub stroke_std:    f32,
    /// Bold emphasis stroke. Mirrors `TokenSnapshot::stroke_bold`.
    pub stroke_bold:   f32,
    /// Thick stroke. Mirrors `TokenSnapshot::stroke_thick`.
    pub stroke_thick:  f32,
    pub stroke_md:     f32,
    pub stroke_heavy:  f32,

    // ── Alpha tiers (u8, 0-255) — mirror TokenSnapshot ────────────────────
    /// Near-invisible overlay. Mirrors `TokenSnapshot::alpha_faint` = 10.
    pub alpha_faint:  u8,
    /// Ghost alpha. Mirrors `TokenSnapshot::alpha_ghost` = 15.
    pub alpha_ghost:  u8,
    pub alpha_soft_u8: u8,
    pub alpha_subtle_u8: u8,
    /// Tint alpha. Mirrors `TokenSnapshot::alpha_tint` = 48.
    pub alpha_tint:   u8,
    pub alpha_muted_u8: u8,
    /// Dim alpha. Mirrors `TokenSnapshot::alpha_dim` = 60.
    pub alpha_dim:    u8,
    /// Line alpha. Mirrors `TokenSnapshot::alpha_line` = 80.
    pub alpha_line:   u8,
    pub alpha_strong_u8: u8,
    /// Active alpha. Mirrors `TokenSnapshot::alpha_active` = 100.
    pub alpha_active: u8,
    /// Heavy alpha. Mirrors `TokenSnapshot::alpha_heavy` = 120.
    pub alpha_heavy_u8: u8,
    /// Scrim alpha (140). Between heavy (120) and solid (200) — for
    /// command-palette / modal-backdrop scrims that want to dim but not blank.
    pub alpha_scrim: u8,
    /// Solid alpha. Mirrors `TokenSnapshot::alpha_solid` = 200.
    pub alpha_solid:  u8,

    // ── Alpha multipliers (f32 0.0-1.0) — design-system composites ────────
    pub alpha_subtle:        f32,
    pub alpha_soft:          f32,
    pub alpha_muted:         f32,
    pub alpha_mid:           f32,
    pub alpha_strong:        f32,
    pub alpha_header_border: f32,

    // ── Shadow primitives — mirror TokenSnapshot ────────────────────────────
    /// Shadow offset. Mirrors `TokenSnapshot::shadow_offset` = 2.0.
    pub shadow_offset: f32,
    /// Shadow alpha (u8). Mirrors `TokenSnapshot::shadow_alpha` = 60.
    pub shadow_alpha_u8: u8,
    /// Shadow spread. Mirrors `TokenSnapshot::shadow_spread` = 4.0.
    pub shadow_spread: f32,

    // ── Elevation factors ───────────────────────────────────────────────────
    pub elevation_l1: f32,
    pub elevation_l2: f32,
    pub elevation_l3: f32,

    // ── Density ─────────────────────────────────────────────────────────────
    pub density_factor:              f32,
    pub row_height_dense:            f32,
    pub row_height_comfortable:      f32,

    // ── Shadow geometry (preset slots) ──────────────────────────────────────
    pub shadow_card_blur:      f32,
    pub shadow_card_offset_y:  f32,
    pub shadow_card_alpha:     f32,
    pub shadow_modal_blur:     f32,
    pub shadow_modal_offset_y: f32,
    pub shadow_modal_alpha:    f32,
    pub shadow_tooltip_blur:   f32,
    pub shadow_tooltip_alpha:  f32,

    // ── Treatments ──────────────────────────────────────────────────────────
    pub solid_active_fills:       bool,
    pub hairline_borders:         bool,
    pub uppercase_section_labels: bool,

    // ── Resolved colour primitives (from ColorScheme) ───────────────────────
    /// `[r, g, b, a]` of `colors.bg`
    pub bg:     [u8; 4],
    pub surface: [u8; 4],
    pub text:    [u8; 4],
    pub dim:     [u8; 4],
    pub border:  [u8; 4],
    pub accent:  [u8; 4],
    pub bull:    [u8; 4],
    pub bear:    [u8; 4],
    pub warn:    [u8; 4],
    pub shadow:  [u8; 4],

    /// `true` if the active color scheme is dark.
    pub is_dark: bool,
}

// ── Default / const snapshot ──────────────────────────────────────────────────

/// Compile-time default snapshot (used to initialise the `thread_local` before
/// the first `begin_frame` call).  Values match the `Default` impl of each
/// sub-struct so there is no visible mismatch on the first frame.
pub const DEFAULT_SNAPSHOT: DesignSnapshot = DesignSnapshot {
    // Typography
    size_xs: 10.0, size_sm: 11.0, size_md: 13.0, size_lg: 15.0, size_xl: 18.0,
    mono_sm: 11.0, mono_md: 13.0, mono_lg: 15.0,
    // Spacing — gap_xs_mid matches DEFAULT_TOKEN_SNAPSHOT in style.rs (6.0)
    gap_xs: 2.0, gap_xs_mid: 6.0, gap_sm: 4.0, gap_md: 8.0,
    gap_lg: 12.0, gap_xl: 16.0, gap_xxl: 24.0,
    gmd: 8.0, cta_height: 28.0,
    // Radii — match DEFAULT_TOKEN_SNAPSHOT in style.rs
    radius_none: 0.0, radius_xs: 2.0, radius_sm: 4.0, radius_md: 6.0, radius_lg: 12.0,
    // Strokes — match DEFAULT_TOKEN_SNAPSHOT in style.rs
    stroke_hair: 0.3, stroke_thin: 0.5, stroke_medium: 0.8,
    stroke_std: 1.0, stroke_bold: 1.5, stroke_thick: 2.0,
    stroke_md: 1.5, stroke_heavy: 2.0,
    // Alpha tiers (u8) — match DEFAULT_TOKEN_SNAPSHOT in style.rs
    alpha_faint: 10, alpha_ghost: 15, alpha_soft_u8: 20, alpha_subtle_u8: 40,
    alpha_tint: 48, alpha_muted_u8: 60, alpha_dim: 60, alpha_line: 80,
    alpha_strong_u8: 80, alpha_active: 100, alpha_heavy_u8: 120, alpha_scrim: 140, alpha_solid: 200,
    // Alpha multipliers (f32)
    alpha_subtle: 0.04, alpha_soft: 0.12, alpha_muted: 0.24, alpha_mid: 0.48,
    alpha_strong: 0.72, alpha_header_border: 0.18,
    // Shadow primitives — match DEFAULT_TOKEN_SNAPSHOT in style.rs
    shadow_offset: 2.0, shadow_alpha_u8: 60, shadow_spread: 4.0,
    // Elevation
    elevation_l1: 1.05, elevation_l2: 0.95, elevation_l3: 0.88,
    // Density
    density_factor: 1.0, row_height_dense: 22.0, row_height_comfortable: 32.0,
    // Shadow geometry presets
    shadow_card_blur: 8.0, shadow_card_offset_y: 2.0, shadow_card_alpha: 0.3,
    shadow_modal_blur: 24.0, shadow_modal_offset_y: 8.0, shadow_modal_alpha: 0.5,
    shadow_tooltip_blur: 6.0, shadow_tooltip_alpha: 0.4,
    // Treatments
    solid_active_fills: false, hairline_borders: false, uppercase_section_labels: false,
    // Colours — neutral dark defaults
    bg:      [18,  18,  18, 255],
    surface: [28,  28,  28, 255],
    text:    [220, 220, 220, 255],
    dim:     [120, 120, 120, 255],
    border:  [55,  55,  55, 255],
    accent:  [99,  102, 241, 255],
    bull:    [52,  211, 153, 255],
    bear:    [248, 113, 113, 255],
    warn:    [251, 191,  36, 255],
    shadow:  [0,   0,   0,  180],
    is_dark: true,
};

// ── Resolver ──────────────────────────────────────────────────────────────────

/// Resolve a `(StyleSystem, ColorScheme)` pair into a flat `DesignSnapshot`.
///
/// Called once per frame (from `begin_frame`) and stored in the `thread_local`
/// so all token reads within that frame are simple field accesses on the `Copy`
/// struct.
pub fn snapshot(style: &StyleSystem, colors: &ColorScheme) -> DesignSnapshot {
    let t = &style.typography;
    let sp = &style.spacing;
    let r = &style.radii;
    let st = &style.strokes;
    let al = &style.alphas;
    let el = &style.elevation;
    let d = &style.density;
    let sh = &style.shadows;
    let tr = &style.treatments;

    DesignSnapshot {
        // Typography
        size_xs: t.size_xs, size_sm: t.size_sm, size_md: t.size_md,
        size_lg: t.size_lg, size_xl: t.size_xl,
        mono_sm: t.mono_sm, mono_md: t.mono_md, mono_lg: t.mono_lg,
        // Spacing
        gap_xs: sp.xs, gap_xs_mid: sp.xs_mid, gap_sm: sp.sm, gap_md: sp.md,
        gap_lg: sp.lg, gap_xl: sp.xl, gap_xxl: sp.xxl,
        gmd: sp.gmd, cta_height: sp.cta_height,
        // Radii
        radius_none: r.none, radius_xs: r.xs, radius_sm: r.sm,
        radius_md: r.md, radius_lg: r.lg,
        // Strokes — full set mirroring TokenSnapshot
        stroke_hair: st.hair, stroke_thin: st.thin, stroke_medium: st.medium,
        stroke_std: st.std, stroke_bold: st.bold, stroke_thick: st.thick,
        stroke_md: st.md, stroke_heavy: st.heavy,
        // Alpha tiers (u8) — mirror TokenSnapshot values
        alpha_faint:     al.faint,
        alpha_ghost:     al.ghost,
        alpha_soft_u8:   al.soft_u8,
        alpha_subtle_u8: al.subtle_u8,
        alpha_tint:      al.tint,
        alpha_muted_u8:  al.muted_u8,
        alpha_dim:       al.dim,
        alpha_line:      al.line,
        alpha_strong_u8: al.strong_u8,
        alpha_active:    al.active,
        alpha_heavy_u8:  al.heavy_u8,
        alpha_scrim:     al.scrim,
        alpha_solid:     al.solid,
        // Alpha multipliers (f32)
        alpha_subtle: al.subtle, alpha_soft: al.soft, alpha_muted: al.muted,
        alpha_mid: al.mid, alpha_strong: al.strong, alpha_header_border: al.header_border,
        // Shadow primitives — mirror TokenSnapshot
        shadow_offset:   sh.card.offset_y,  // primary offset from card shadow
        shadow_alpha_u8: (sh.card.alpha * 255.0) as u8,
        shadow_spread:   sh.card.spread,
        // Elevation
        elevation_l1: el.l1, elevation_l2: el.l2, elevation_l3: el.l3,
        // Density
        density_factor: d.factor, row_height_dense: d.row_height_dense,
        row_height_comfortable: d.row_height_comfortable,
        // Shadow geometry presets
        shadow_card_blur:      sh.card.blur,
        shadow_card_offset_y:  sh.card.offset_y,
        shadow_card_alpha:     sh.card.alpha,
        shadow_modal_blur:     sh.modal.blur,
        shadow_modal_offset_y: sh.modal.offset_y,
        shadow_modal_alpha:    sh.modal.alpha,
        shadow_tooltip_blur:   sh.tooltip.blur,
        shadow_tooltip_alpha:  sh.tooltip.alpha,
        // Treatments
        solid_active_fills:       tr.solid_active_fills,
        hairline_borders:         tr.hairline_borders,
        uppercase_section_labels: tr.uppercase_section_labels,
        // Colours
        bg:      colors.bg,
        surface: colors.surface,
        text:    colors.text,
        dim:     colors.dim,
        border:  colors.border,
        accent:  colors.accent,
        bull:    colors.bull,
        bear:    colors.bear,
        warn:    colors.warn,
        shadow:  colors.shadow,
        is_dark: colors.meta.is_dark,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design_system::{
        color_scheme::builtin_dark,
        style_system::StyleSystem,
    };

    #[test]
    fn snapshot_smoke_default_values() {
        let style = StyleSystem::builtin_default();
        let colors = builtin_dark();
        let snap = snapshot(&style, &colors);

        // Font sizes match the Typography defaults
        assert_eq!(snap.size_sm, 11.0);
        assert_eq!(snap.size_md, 13.0);
        assert_eq!(snap.mono_md, 13.0);

        // Spacing
        assert_eq!(snap.gap_md, 8.0);
        assert_eq!(snap.gmd, 8.0);

        // Radii
        assert_eq!(snap.radius_sm, 4.0);

        // Alphas
        assert!((snap.alpha_muted - 0.24).abs() < f32::EPSILON);

        // Colours match the palette
        assert_eq!(snap.accent, colors.accent);
        assert_eq!(snap.bull,   colors.bull);
        assert!(snap.is_dark);
    }

    #[test]
    fn snapshot_meridien_treatments() {
        let style = StyleSystem::meridien();
        let colors = builtin_dark();
        let snap = snapshot(&style, &colors);
        assert!(snap.solid_active_fills);
        assert!(snap.hairline_borders);
        assert!(snap.uppercase_section_labels);
        // Meridien uses hairline stroke as std
        assert_eq!(snap.stroke_std, 0.5);
    }

    #[test]
    fn default_snapshot_is_const() {
        // Ensure the compile-time constant stays in sync with at least one
        // field from the default style + dark palette combination.
        let style = StyleSystem::default();
        let colors = builtin_dark();
        let live = snapshot(&style, &colors);
        assert_eq!(live.size_sm, DEFAULT_SNAPSHOT.size_sm);
        assert_eq!(live.gap_md,  DEFAULT_SNAPSHOT.gap_md);
    }
}
