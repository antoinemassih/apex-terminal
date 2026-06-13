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

use super::{color_scheme::ColorScheme, style_system::{BevelStyle, GroupEnclosure, StyleSystem}};

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
    /// Section / eyebrow label font size. Mirrors `Typography::size_section_label`.
    pub size_section_label: f32,
    /// Letter-spacing (px) for general tracked-out labels. Mirrors `Typography::label_tracking`.
    pub label_tracking: f32,
    /// Letter-spacing (px) for toolbar nav button text. Mirrors `Typography::nav_tracking`.
    pub nav_tracking: f32,
    /// Letter-spacing (px) for section/eyebrow headers. Mirrors `Typography::section_tracking`.
    pub section_tracking: f32,
    // NOTE: font family Strings (family_ui, family_mono, family_display) are intentionally
    // deferred — DesignSnapshot must stay Copy. String fields will land in S7's lazy resolver.

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
    /// Primary CTA button horizontal padding. Mirrors `Spacing::cta_padding_x`.
    pub cta_padding_x: f32,
    /// Standard button height. Mirrors `Spacing::button_height`.
    pub button_height: f32,
    /// Standard button horizontal padding. Mirrors `Spacing::button_padding_x`.
    pub button_padding_x: f32,
    /// Tab strip height. Mirrors `Spacing::tab_height`.
    pub tab_height: f32,

    // ── Radii ───────────────────────────────────────────────────────────────
    pub radius_none: f32,
    pub radius_xs:   f32,
    pub radius_sm:   f32,
    pub radius_md:   f32,
    pub radius_lg:   f32,
    /// Full pill / circular radius (conceptually max round). Mirrors `Radii::full`.
    pub radius_full: f32,
    /// Pill radius runtime value (0 = sharp pill, 99 = rounded pill). Mirrors `Radii::pill`.
    pub radius_pill: f32,
    /// Chip/badge corner radius. 0 = use `radius_sm`. Mirrors `Radii::chip`.
    pub radius_chip: f32,

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
    pub shadow_card_blur:          f32,
    pub shadow_card_spread:        f32,
    pub shadow_card_offset_y:      f32,
    pub shadow_card_alpha:         f32,
    pub shadow_modal_blur:         f32,
    pub shadow_modal_spread:       f32,
    pub shadow_modal_offset_y:     f32,
    pub shadow_modal_alpha:        f32,
    pub shadow_tooltip_blur:       f32,
    pub shadow_tooltip_spread:     f32,
    pub shadow_tooltip_alpha:      f32,
    pub shadow_dropdown_blur:      f32,
    pub shadow_dropdown_spread:    f32,
    pub shadow_dropdown_offset_y:  f32,
    pub shadow_dropdown_alpha:     f32,

    // ── Treatments ──────────────────────────────────────────────────────────
    pub solid_active_fills:       bool,
    pub hairline_borders:         bool,
    pub uppercase_section_labels: bool,
    pub segmented_filled_idle:    bool,
    /// Focus ring style as u8: 0=None, 1=Outline, 2=Glow. Flattened from
    /// `FocusRingStyle` (not Copy) for snapshot compatibility.
    pub focus_ring_style: u8,
    /// Surface bevel mode. `BevelStyle` is `Copy`.
    pub surface_bevel:            BevelStyle,
    pub bevel_highlight_alpha:    u8,
    pub bevel_shadow_alpha:       u8,
    pub wl_row_side_margin:       f32,
    pub wl_row_corner_radius:     u8,
    pub wl_row_divider_alpha:     u8,
    pub section_header_mono:      bool,
    pub wl_symbol_mono:           bool,
    /// Default tab treatment index (0=Line, 1=Segmented, 2=Filled, 3=Card, 4=Pane).
    pub panel_tab_treatment:      u8,
    pub pane_active_fill_accent:  bool,
    pub serif_headlines:          bool,
    /// Active-state button treatment index (0=SoftPill, 1=OutlineAccent, …).
    pub button_treatment:         u8,
    pub invert_active_fill:       bool,
    pub vertical_group_dividers:  bool,
    pub show_active_tab_underline: bool,
    pub inactive_header_fill:     bool,
    pub nav_buttons_label_only:   bool,
    pub nav_buttons_uppercase_labels: bool,
    pub tab_underline_under_text: bool,
    pub card_floating_shadow:     bool,
    pub shadows_enabled:          bool,
    pub animations_enabled:       bool,

    // ── Chrome ──────────────────────────────────────────────────────────────
    pub toolbar_height_scale:          f32,
    pub header_height_scale:           f32,
    pub account_strip_height:          f32,
    pub pane_border_width:             f32,
    pub pane_gap:                      f32,
    pub pane_gap_alpha:                u8,
    /// Active pane indicator index (0=none, 1=top stripe, 2=header fill, 3=both).
    pub pane_active_indicator:         u8,
    pub active_header_fill_multiply:   f32,
    pub inactive_header_fill_multiply: f32,
    pub header_outer_border_alpha:     u8,
    pub header_outer_border_width:     f32,
    pub header_divider_alpha:          u8,
    pub nav_active_col_alpha:          u8,
    pub dialog_backdrop_alpha:         u8,
    pub tab_inactive_alpha:            f32,
    pub tab_hover_bg_alpha:            u8,
    pub tab_underline_thickness:       f32,
    pub section_label_padding_top:     f32,
    pub section_label_padding_bottom:  f32,
    pub drag_handle_alpha:             f32,
    pub drag_handle_dot_scale:         f32,
    pub toast_bg_alpha:                u8,
    pub card_stripe_alpha:             u8,
    pub card_floating_shadow_alpha:    u8,
    pub accent_emphasis:               f32,
    pub disabled_opacity:              f32,
    pub focus_ring_width:              f32,
    pub focus_ring_alpha:              u8,
    pub hover_bg_alpha:                u8,
    pub active_bg_alpha:               u8,
    pub region_gap:                    f32,
    pub region_radius:                 f32,
    pub region_border_alpha:           u8,
    pub nav_cluster_radius:            f32,
    pub nav_cluster_fill_alpha:        u8,
    pub nav_cluster_padding:           f32,
    /// Button group enclosure style. `GroupEnclosure` is `Copy`.
    pub button_group:                  GroupEnclosure,
    pub toolnav_height:                f32,
    pub footer_default_open:           bool,
    /// Panel header tab treatment index (0=Line, 1=Segmented, 2=Filled, 3=Card, 4=Pane).
    pub panel_header_treatment:        u8,
    pub panel_section_fill_alpha:      u8,
    pub panel_footer_card:             bool,
    pub panel_footer_radius:           f32,

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
    // Typography — P2.2 aligned to DEFAULT_TOKEN_SNAPSHOT in ui_kit/style.rs
    size_xs:  9.0, size_sm: 11.0, size_md: 13.0, size_lg: 16.0, size_xl: 22.0,
    mono_sm: 11.0, mono_md: 13.0, mono_lg: 16.0,
    // Typography — new S2 fields (match Typography::default())
    size_section_label: 9.0,
    label_tracking: 0.0, nav_tracking: 0.0, section_tracking: 0.0,
    // Spacing — P2.2 aligned to DEFAULT_TOKEN_SNAPSHOT in ui_kit/style.rs
    gap_xs: 4.0, gap_xs_mid: 6.0, gap_sm: 8.0, gap_md: 12.0,
    gap_lg: 16.0, gap_xl: 20.0, gap_xxl: 24.0,
    gmd: 8.0, cta_height: 28.0,
    // Spacing — new S2 fields (match Spacing::default())
    cta_padding_x: 12.0, button_height: 24.0, button_padding_x: 10.0, tab_height: 28.0,
    // Radii — match DEFAULT_TOKEN_SNAPSHOT in style.rs
    radius_none: 0.0, radius_xs: 2.0, radius_sm: 4.0, radius_md: 6.0, radius_lg: 12.0,
    // Radii — new S2 fields (match Radii::default())
    radius_full: 9999.0, radius_pill: 99.0, radius_chip: 0.0,
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
    // Shadow geometry presets (match Shadows::default())
    shadow_card_blur: 8.0, shadow_card_spread: 0.0, shadow_card_offset_y: 2.0, shadow_card_alpha: 0.3,
    shadow_modal_blur: 24.0, shadow_modal_spread: 0.0, shadow_modal_offset_y: 8.0, shadow_modal_alpha: 0.5,
    shadow_tooltip_blur: 6.0, shadow_tooltip_spread: 0.0, shadow_tooltip_alpha: 0.4,
    shadow_dropdown_blur: 12.0, shadow_dropdown_spread: 0.0, shadow_dropdown_offset_y: 4.0, shadow_dropdown_alpha: 0.4,
    // Treatments (match Treatments::default())
    solid_active_fills: false, hairline_borders: false, uppercase_section_labels: false,
    segmented_filled_idle: false,
    focus_ring_style: 1, // FocusRingStyle::Outline
    surface_bevel: BevelStyle::None,
    bevel_highlight_alpha: 0, bevel_shadow_alpha: 0,
    wl_row_side_margin: 0.0, wl_row_corner_radius: 0, wl_row_divider_alpha: 0,
    section_header_mono: false, wl_symbol_mono: false,
    panel_tab_treatment: 0,
    pane_active_fill_accent: false,
    serif_headlines: false, button_treatment: 0, invert_active_fill: false,
    vertical_group_dividers: false, show_active_tab_underline: true, inactive_header_fill: true,
    nav_buttons_label_only: false, nav_buttons_uppercase_labels: false,
    tab_underline_under_text: false, card_floating_shadow: false,
    shadows_enabled: true, animations_enabled: true,
    // Chrome (match Chrome::default())
    toolbar_height_scale: 1.0, header_height_scale: 1.0,
    account_strip_height: 26.0, pane_border_width: 1.0, pane_gap: 0.0, pane_gap_alpha: 0,
    pane_active_indicator: 2,
    active_header_fill_multiply: 0.7, inactive_header_fill_multiply: 1.08,
    header_outer_border_alpha: 38, header_outer_border_width: 0.5,
    header_divider_alpha: 50, nav_active_col_alpha: 0, dialog_backdrop_alpha: 0,
    tab_inactive_alpha: 0.55, tab_hover_bg_alpha: 18, tab_underline_thickness: 2.0,
    section_label_padding_top: 4.0, section_label_padding_bottom: 2.0,
    drag_handle_alpha: 0.6, drag_handle_dot_scale: 1.0,
    toast_bg_alpha: 220, card_stripe_alpha: 255, card_floating_shadow_alpha: 0,
    accent_emphasis: 1.0, disabled_opacity: 0.5,
    focus_ring_width: 1.5, focus_ring_alpha: 110, hover_bg_alpha: 18, active_bg_alpha: 30,
    region_gap: 0.0, region_radius: 12.0, region_border_alpha: 40,
    nav_cluster_radius: 8.0, nav_cluster_fill_alpha: 0, nav_cluster_padding: 6.0,
    button_group: GroupEnclosure::None,
    toolnav_height: 0.0, footer_default_open: false,
    panel_header_treatment: 0, panel_section_fill_alpha: 0,
    panel_footer_card: false, panel_footer_radius: 10.0,
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
    use super::style_system::FocusRingStyle;

    let t = &style.typography;
    let sp = &style.spacing;
    let r = &style.radii;
    let st = &style.strokes;
    let al = &style.alphas;
    let el = &style.elevation;
    let d = &style.density;
    let sh = &style.shadows;
    let tr = &style.treatments;
    let ch = &style.chrome;

    DesignSnapshot {
        // Typography
        size_xs: t.size_xs, size_sm: t.size_sm, size_md: t.size_md,
        size_lg: t.size_lg, size_xl: t.size_xl,
        mono_sm: t.mono_sm, mono_md: t.mono_md, mono_lg: t.mono_lg,
        // Typography — new S2 fields
        size_section_label: t.size_section_label,
        label_tracking: t.label_tracking,
        nav_tracking:   t.nav_tracking,
        section_tracking: t.section_tracking,
        // Spacing
        gap_xs: sp.xs, gap_xs_mid: sp.xs_mid, gap_sm: sp.sm, gap_md: sp.md,
        gap_lg: sp.lg, gap_xl: sp.xl, gap_xxl: sp.xxl,
        gmd: sp.gmd, cta_height: sp.cta_height,
        // Spacing — new S2 fields
        cta_padding_x:   sp.cta_padding_x,
        button_height:   sp.button_height,
        button_padding_x: sp.button_padding_x,
        tab_height:      sp.tab_height,
        // Radii
        radius_none: r.none, radius_xs: r.xs, radius_sm: r.sm,
        radius_md: r.md, radius_lg: r.lg,
        // Radii — new S2 fields
        radius_full: r.full,
        radius_pill: r.pill,
        radius_chip: r.chip,
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
        // Shadow geometry presets — full ShadowSpec fields
        shadow_card_blur:         sh.card.blur,
        shadow_card_spread:       sh.card.spread,
        shadow_card_offset_y:     sh.card.offset_y,
        shadow_card_alpha:        sh.card.alpha,
        shadow_modal_blur:        sh.modal.blur,
        shadow_modal_spread:      sh.modal.spread,
        shadow_modal_offset_y:    sh.modal.offset_y,
        shadow_modal_alpha:       sh.modal.alpha,
        shadow_tooltip_blur:      sh.tooltip.blur,
        shadow_tooltip_spread:    sh.tooltip.spread,
        shadow_tooltip_alpha:     sh.tooltip.alpha,
        shadow_dropdown_blur:     sh.dropdown.blur,
        shadow_dropdown_spread:   sh.dropdown.spread,
        shadow_dropdown_offset_y: sh.dropdown.offset_y,
        shadow_dropdown_alpha:    sh.dropdown.alpha,
        // Treatments
        solid_active_fills:       tr.solid_active_fills,
        hairline_borders:         tr.hairline_borders,
        uppercase_section_labels: tr.uppercase_section_labels,
        segmented_filled_idle:    tr.segmented_filled_idle,
        focus_ring_style: match tr.focus_ring {
            FocusRingStyle::None    => 0,
            FocusRingStyle::Outline => 1,
            FocusRingStyle::Glow    => 2,
        },
        surface_bevel:            tr.surface_bevel,
        bevel_highlight_alpha:    tr.bevel_highlight_alpha,
        bevel_shadow_alpha:       tr.bevel_shadow_alpha,
        wl_row_side_margin:       tr.wl_row_side_margin,
        wl_row_corner_radius:     tr.wl_row_corner_radius,
        wl_row_divider_alpha:     tr.wl_row_divider_alpha,
        section_header_mono:      tr.section_header_mono,
        wl_symbol_mono:           tr.wl_symbol_mono,
        panel_tab_treatment:      tr.panel_tab_treatment,
        pane_active_fill_accent:  tr.pane_active_fill_accent,
        serif_headlines:          tr.serif_headlines,
        button_treatment:         tr.button_treatment,
        invert_active_fill:       tr.invert_active_fill,
        vertical_group_dividers:  tr.vertical_group_dividers,
        show_active_tab_underline: tr.show_active_tab_underline,
        inactive_header_fill:     tr.inactive_header_fill,
        nav_buttons_label_only:   tr.nav_buttons_label_only,
        nav_buttons_uppercase_labels: tr.nav_buttons_uppercase_labels,
        tab_underline_under_text: tr.tab_underline_under_text,
        card_floating_shadow:     tr.card_floating_shadow,
        shadows_enabled:          tr.shadows_enabled,
        animations_enabled:       tr.animations_enabled,
        // Chrome
        toolbar_height_scale:          ch.toolbar_height_scale,
        header_height_scale:           ch.header_height_scale,
        account_strip_height:          ch.account_strip_height,
        pane_border_width:             ch.pane_border_width,
        pane_gap:                      ch.pane_gap,
        pane_gap_alpha:                ch.pane_gap_alpha,
        pane_active_indicator:         ch.pane_active_indicator,
        active_header_fill_multiply:   ch.active_header_fill_multiply,
        inactive_header_fill_multiply: ch.inactive_header_fill_multiply,
        header_outer_border_alpha:     ch.header_outer_border_alpha,
        header_outer_border_width:     ch.header_outer_border_width,
        header_divider_alpha:          ch.header_divider_alpha,
        nav_active_col_alpha:          ch.nav_active_col_alpha,
        dialog_backdrop_alpha:         ch.dialog_backdrop_alpha,
        tab_inactive_alpha:            ch.tab_inactive_alpha,
        tab_hover_bg_alpha:            ch.tab_hover_bg_alpha,
        tab_underline_thickness:       ch.tab_underline_thickness,
        section_label_padding_top:     ch.section_label_padding_top,
        section_label_padding_bottom:  ch.section_label_padding_bottom,
        drag_handle_alpha:             ch.drag_handle_alpha,
        drag_handle_dot_scale:         ch.drag_handle_dot_scale,
        toast_bg_alpha:                ch.toast_bg_alpha,
        card_stripe_alpha:             ch.card_stripe_alpha,
        card_floating_shadow_alpha:    ch.card_floating_shadow_alpha,
        accent_emphasis:               ch.accent_emphasis,
        disabled_opacity:              ch.disabled_opacity,
        focus_ring_width:              ch.focus_ring_width,
        focus_ring_alpha:              ch.focus_ring_alpha,
        hover_bg_alpha:                ch.hover_bg_alpha,
        active_bg_alpha:               ch.active_bg_alpha,
        region_gap:                    ch.region_gap,
        region_radius:                 ch.region_radius,
        region_border_alpha:           ch.region_border_alpha,
        nav_cluster_radius:            ch.nav_cluster_radius,
        nav_cluster_fill_alpha:        ch.nav_cluster_fill_alpha,
        nav_cluster_padding:           ch.nav_cluster_padding,
        button_group:                  ch.button_group,
        toolnav_height:                ch.toolnav_height,
        footer_default_open:           ch.footer_default_open,
        panel_header_treatment:        ch.panel_header_treatment,
        panel_section_fill_alpha:      ch.panel_section_fill_alpha,
        panel_footer_card:             ch.panel_footer_card,
        panel_footer_radius:           ch.panel_footer_radius,
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

        // Spacing — P2.2 aligned: gap_md now 12.0 (matches TokenSnapshot)
        assert_eq!(snap.gap_md, 12.0);
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

    #[test]
    fn snapshot_new_typography_fields() {
        let style = StyleSystem::builtin_default();
        let colors = builtin_dark();
        let snap = snapshot(&style, &colors);
        // size_section_label defaults to 9.0 (matches Typography::default_section_label)
        assert_eq!(snap.size_section_label, 9.0);
        // tracking fields default to 0.0
        assert_eq!(snap.label_tracking, 0.0);
        assert_eq!(snap.nav_tracking, 0.0);
        assert_eq!(snap.section_tracking, 0.0);
        // DEFAULT_SNAPSHOT const must match
        assert_eq!(snap.size_section_label, DEFAULT_SNAPSHOT.size_section_label);
        assert_eq!(snap.label_tracking,     DEFAULT_SNAPSHOT.label_tracking);
    }

    #[test]
    fn snapshot_new_spacing_fields() {
        let style = StyleSystem::builtin_default();
        let colors = builtin_dark();
        let snap = snapshot(&style, &colors);
        assert_eq!(snap.cta_padding_x,  12.0);
        assert_eq!(snap.button_height,  24.0);
        assert_eq!(snap.button_padding_x, 10.0);
        assert_eq!(snap.tab_height,     28.0);
        assert_eq!(snap.cta_padding_x,  DEFAULT_SNAPSHOT.cta_padding_x);
        assert_eq!(snap.button_height,  DEFAULT_SNAPSHOT.button_height);
    }

    #[test]
    fn snapshot_new_radii_fields() {
        let style = StyleSystem::builtin_default();
        let colors = builtin_dark();
        let snap = snapshot(&style, &colors);
        assert_eq!(snap.radius_full, 9999.0);
        assert_eq!(snap.radius_pill,   99.0);
        assert_eq!(snap.radius_chip,    0.0);
        assert_eq!(snap.radius_full, DEFAULT_SNAPSHOT.radius_full);
        assert_eq!(snap.radius_pill, DEFAULT_SNAPSHOT.radius_pill);
    }

    #[test]
    fn snapshot_shadow_geometry_extended() {
        let style = StyleSystem::builtin_default();
        let colors = builtin_dark();
        let snap = snapshot(&style, &colors);
        // Spread fields (all 0.0 in defaults)
        assert_eq!(snap.shadow_card_spread,     0.0);
        assert_eq!(snap.shadow_modal_spread,    0.0);
        assert_eq!(snap.shadow_tooltip_spread,  0.0);
        // Dropdown shadow (previously absent)
        assert_eq!(snap.shadow_dropdown_blur,    12.0);
        assert_eq!(snap.shadow_dropdown_offset_y, 4.0);
        assert_eq!(snap.shadow_dropdown_alpha,    0.4);
        assert!((snap.shadow_dropdown_alpha - DEFAULT_SNAPSHOT.shadow_dropdown_alpha).abs() < f32::EPSILON);
    }

    #[test]
    fn snapshot_treatments_extended() {
        let style = StyleSystem::builtin_default();
        let colors = builtin_dark();
        let snap = snapshot(&style, &colors);
        // New treatment booleans — all default false except show_active_tab_underline,
        // inactive_header_fill, shadows_enabled, animations_enabled
        assert!(!snap.segmented_filled_idle);
        assert_eq!(snap.focus_ring_style, 1); // Outline
        assert!(!snap.pane_active_fill_accent);
        assert!(snap.show_active_tab_underline);
        assert!(snap.inactive_header_fill);
        assert!(snap.shadows_enabled);
        assert!(snap.animations_enabled);
        // Bevel defaults: None / 0 / 0
        assert_eq!(snap.bevel_highlight_alpha, 0);
        assert_eq!(snap.bevel_shadow_alpha, 0);
        // Watchlist row defaults
        assert_eq!(snap.wl_row_side_margin, 0.0);
        assert_eq!(snap.wl_row_corner_radius, 0);
        assert_eq!(snap.wl_row_divider_alpha, 0);
        // Meridien overrides divider alpha
        let meridien = StyleSystem::meridien();
        let msnap = snapshot(&meridien, &colors);
        assert_eq!(msnap.wl_row_divider_alpha, 30);
    }

    #[test]
    fn snapshot_chrome_fields() {
        let style = StyleSystem::builtin_default();
        let colors = builtin_dark();
        let snap = snapshot(&style, &colors);
        assert_eq!(snap.toolbar_height_scale, 1.0);
        assert_eq!(snap.header_height_scale,  1.0);
        assert_eq!(snap.account_strip_height, 26.0);
        assert_eq!(snap.pane_active_indicator, 2);
        assert_eq!(snap.tab_underline_thickness, 2.0);
        assert!((snap.tab_inactive_alpha - 0.55).abs() < f32::EPSILON);
        assert!(!snap.footer_default_open);
        assert!(!snap.panel_footer_card);
        assert_eq!(snap.panel_footer_radius, 10.0);
        // Const must match live
        assert_eq!(snap.pane_active_indicator,   DEFAULT_SNAPSHOT.pane_active_indicator);
        assert_eq!(snap.toolbar_height_scale,     DEFAULT_SNAPSHOT.toolbar_height_scale);
        assert_eq!(snap.panel_footer_radius,      DEFAULT_SNAPSHOT.panel_footer_radius);
    }
}
