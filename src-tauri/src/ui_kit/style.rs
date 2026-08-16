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
    /// Pill/fully-round radius — PER-STYLE (Aperture 99, Meridien 0 sharp).
    pub radius_pill: f32,
    // Strokes.
    pub stroke_hair:   f32,
    pub stroke_thin:   f32,
    pub stroke_medium: f32,
    pub stroke_std:    f32,
    pub stroke_bold:   f32,
    pub stroke_thick:  f32,
    /// Extra-thick 2.5 px stroke. Was a hardcoded `2.5` in the "pure
    /// constants" block with 5 live call sites, while the design inspector
    /// carried a `stroke.heavy (2.5)` slider that nothing read — the same
    /// number, authored in one place and consumed from another.
    /// Icon glyph sizes. Were four hardcoded literals, so no theme could
    /// change icon scale — a primary axis of a UI's density and character.
    /// Display type scale + the three in-between UI rungs. Seven more values
    /// that were bare literals, so a style could author its whole UI ladder and
    /// still not touch the sizes that set a screen's voice.
    pub font_display_sm: f32,
    pub font_display_md: f32,
    pub font_display_lg: f32,
    pub font_display_xl: f32,
    pub font_4xs: f32,
    pub font_xs_plus: f32,
    pub font_md_plus: f32,
    /// The 2px base of the gap ladder — `gap_2xs()` applied the spacing
    /// override to a hardcoded 2.0, so the rung scaled but could not be
    /// re-pitched by a style.
    pub gap_2xs: f32,
    /// The two alpha rungs between `ghost` (15) and `subtle` (40).
    pub alpha_whisper: u8,
    pub alpha_hint: u8,
    pub icon_xs: f32,
    pub icon_sm: f32,
    pub icon_md: f32,
    pub icon_lg: f32,
    /// Line-height (leading) multipliers. Also hardcoded before this. Leading
    /// is the biggest single lever on whether a dense trading UI reads as
    /// tight/technical or open/editorial — the Meridien vs Aperture axis — and
    /// it was the one thing a theme could not touch.
    pub line_tight: f32,
    pub line_heading: f32,
    pub line_dense: f32,
    pub line_compact: f32,
    pub line_normal: f32,
    pub line_loose: f32,
    pub stroke_extra_thick: f32,
    /// Decorative / accent rule, 3 px.
    ///
    /// Named `rule`, not `heavy`, on purpose. There were already three
    /// unrelated "heavy"s with three different values: `DesignTokens.stroke.
    /// heavy` = 2.5, `StyleSystem.Strokes.heavy` = 2.0 (a legacy alias for
    /// `thick`), and `ui_kit::style::stroke_heavy()` = 3.0. Reusing the name a
    /// fourth time is how that happened in the first place.
    pub stroke_rule: f32,
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
    /// Scrim alpha (140). Between heavy (120) and solid (200) — for
    /// command-palette / modal-backdrop scrims (dim but don't blank).
    /// The elevation ladder, carried whole. Each rung is `{radius, offset_y,
    /// alpha}` — `radius` is Gaussian sigma, not a corner radius.
    ///
    /// These are a `shadow_tier`-shaped group deliberately named `elev_*`: the
    /// `shadow_*` fields above traverse the DesignTokens cascade and these do
    /// not (yet), and `cascade_gate.py` reads a split group as an unfinished
    /// migration. Keeping them a separate, uniformly-direct group states the
    /// truth — theme-authorable, not yet inspector-tunable — instead of hiding
    /// a split inside one name.
    pub elev_sm: crate::design_system::style_system::ShadowTier,
    pub elev_md: crate::design_system::style_system::ShadowTier,
    pub elev_lg: crate::design_system::style_system::ShadowTier,
    pub elev_xl: crate::design_system::style_system::ShadowTier,
    pub alpha_scrim:  u8,
    /// Borders and fills at full presence, one rung below `near_solid`.
    pub alpha_dense: u8,
    /// Secondary label text and disabled accents. Read AS TEXT, so it stops
    /// short of `solid` — the ladder's only rung tuned for legibility.
    pub alpha_near_solid: u8,
    pub alpha_solid:  u8,
    // Shadows.
    pub shadow_offset: f32,
    pub shadow_alpha:  u8,
    pub shadow_spread: f32,

    // ── P5b extraction fields (style-preset knobs read by ui_kit widgets) ──
    // These previously required ui_kit widgets to call
    // `chart_renderer::ui::style::current()` (the audit's HARD blocker).
    // Now the chart-app's begin_frame() populates them into the snapshot
    // and widgets read via `frame_tokens()` — fully portable.
    pub focus_ring_alpha: u8,    // input focus ring alpha (0..=255)
    pub focus_ring_width: f32,   // input focus ring stroke width (px)
    pub toast_bg_alpha:   u8,    // toast bg fill alpha (glassmorphic toast)
    pub button_treatment: super::widgets::tokens::ButtonTreatment, // legacy simple_btn variant
    /// Watchlist / list row side margin (px). 0 = flush; >0 = pill-float inset.
    pub wl_row_side_margin: f32,
    /// Watchlist / list row corner radius (px). 0 = no rounding; 99 = pill.
    pub wl_row_corner_radius: u8,
    /// Watchlist / list row hairline divider alpha. 0 = no divider.
    pub wl_row_divider_alpha: u8,

    /// Default tab treatment for ui-kit Tabs widgets (0=Line, 1=Segmented,
    /// 2=Filled, 3=Card, 4=Pane). Populated by begin_frame from StyleSettings.
    /// Kept as u8 so the chart-side `begin_frame` struct literal needs no change.
    /// Read via `style_tab_treatment()`, which is the accessor that is actually
    /// consumed. AUDIT 2026-08: this doc used to point at
    /// `panel_tab_treatment_typed()` and `style_panel_header_treatment()` —
    /// both had ZERO consumers, and the latter returned a hardcoded Default
    /// rather than reading the snapshot at all. Both are deleted; panel-header
    /// treatment needs its own snapshot field before it can mean anything.
    pub panel_tab_treatment: u8,

    // ── S3: `pane_active_indicator` and `panel_header_treatment` ─────────────
    // These two fields are intentionally NOT added to this struct yet because
    // the chart-side `begin_frame()` constructs `TokenSnapshot` as a full
    // struct literal — adding required fields here would break it without a
    // simultaneous chart/ edit (which is out of scope for S3). Instead, the
    // typed accessors `style_pane_active_indicator()` /
    // `style_panel_header_treatment()` are provided below, currently returning
    // the enum Defaults. Chart/ will populate the new fields in the next round
    // that covers begin_frame changes (can use `..DEFAULT_TOKEN_SNAPSHOT` struct
    // update syntax to add fields one-by-one without breaking the literal).

    // Surface bevel — ported from the React ApexTerminalThemes mockup's
    // inset box-shadow faces (Alto/Mariner raised, Cadence elevated cards).
    // Populated by chart-side begin_frame() from StyleSettings.surface_bevel.
    pub surface_bevel: crate::design_system::style_system::BevelStyle,
    /// How a focused control is ringed. Authored per style (Aperture asks for
    /// `Glow`, the others `Outline`) and, until now, honoured nowhere: the
    /// painter drew one fixed outline whatever the style said, so
    /// `FocusRingStyle::None` did not remove the ring and `Glow` did not glow.
    pub focus_ring: crate::design_system::style_system::FocusRingStyle,
    pub bevel_highlight_alpha: u8, // white inner top-edge alpha (0 = no highlight)
    pub bevel_shadow_alpha:    u8, // black inner bottom-edge alpha (0 = no shadow)
    /// M1 Change C: authored bevel tint (RGB; alpha from the knobs above).
    /// Defaults WHITE/BLACK — the pre-M1 hardcoded look.
    pub bevel_highlight_tint: Color32,
    pub bevel_shadow_tint: Color32,
    /// M2.1: per-style semantic font sizes (previously read from chart-side
    /// `StyleSettings::current()` inside text_style.rs — which fenced the
    /// entire 16-tier cascade OUT of ui_kit by dependency direction).
    pub font_body: f32,
    pub font_caption: f32,
    pub font_section_label: f32,
    // M4.5: structural proportions (per-style; were hard literals).
    pub row_dense: f32,
    pub row_compact: f32,
    pub row_default: f32,
    pub row_spacious: f32,
    pub row_tall: f32,
    pub splitter_width: f32,
    /// Gutter between panes in the mosaic.
    ///
    /// Was readable only as `chart_renderer::ui::style::current().pane_gap` —
    /// the bottom layer of the cascade — so the gutter ignored every layer
    /// above it. Editing it in the F12 inspector or a hot-reloaded theme JSON
    /// did nothing, while the panes it separates restyled normally.
    pub pane_gap: f32,
    /// Control-height ladder backing `Size::height()` (M-refine).
    pub control_xs: f32,
    pub control_sm: f32,
    pub control_md: f32,
    pub control_lg: f32,
    pub control_xl: f32,
    pub rail_narrow: f32,
    pub rail_medium: f32,
    pub rail_wide: f32,
}

/// Compile-time defaults — match every token fn's non-design-mode constant
/// so the first frame (before any host calls `set_frame_tokens`) returns
/// identical values.
pub const DEFAULT_TOKEN_SNAPSHOT: TokenSnapshot = TokenSnapshot {
    // Fonts.
    // Type scale lift — see the matching comment in chart/renderer/ui/style.rs.
    // These are the fallbacks used when no host pushes tokens; they must track
    // the chart-layer values or a headless/portable host renders a different
    // (smaller) scale than the app.
    font_2xs: 9.0, font_xs: 10.0, font_sm: 12.0, font_md: 14.0, font_lg: 16.0, font_xl: 22.0,
    // Spacing.
    gap_xs: 4.0, gap_xs_mid: 6.0, gap_sm: 8.0, gap_md: 12.0,
    gap_lg: 16.0, gap_xl: 20.0, gap_2xl: 24.0, gap_3xl: 32.0,
    // Radii.
    radius_xs: 2.0, radius_sm: 4.0, radius_md: 6.0, radius_lg: 12.0, radius_pill: 999.0,
    // Strokes.
    stroke_hair: 0.3, stroke_thin: 0.5, stroke_medium: 0.8,
    stroke_std: 1.0, stroke_bold: 1.5, stroke_thick: 2.0,
    stroke_extra_thick: 2.5, stroke_rule: 3.0,
    font_display_sm: 28.0, font_display_md: 32.0,
    font_display_lg: 42.0, font_display_xl: 56.0,
    font_4xs: 6.0, font_xs_plus: 10.0, font_md_plus: 14.0,
    gap_2xs: 2.0, alpha_whisper: 25, alpha_hint: 30,
    icon_xs: 14.0, icon_sm: 16.0, icon_md: 18.0, icon_lg: 20.0,
    line_tight: 1.20, line_heading: 1.25, line_dense: 1.30,
    line_compact: 1.35, line_normal: 1.40, line_loose: 1.50,
    // Alphas.
    alpha_faint: 10, alpha_ghost: 15, alpha_soft: 20, alpha_subtle: 40,
    alpha_tint: 48, alpha_muted: 60, alpha_dim: 60, alpha_line: 80,
    alpha_strong: 80, alpha_active: 100, alpha_heavy: 120, elev_sm: crate::design_system::style_system::ShadowTier { radius:  8.0, offset_y:  2.0, alpha:  64 },
    elev_md: crate::design_system::style_system::ShadowTier { radius: 16.0, offset_y:  4.0, alpha:  77 },
    elev_lg: crate::design_system::style_system::ShadowTier { radius: 24.0, offset_y:  8.0, alpha:  89 },
    elev_xl: crate::design_system::style_system::ShadowTier { radius: 32.0, offset_y: 12.0, alpha: 102 },
    alpha_scrim: 140, alpha_dense: 160, alpha_near_solid: 180, alpha_solid: 200,
    // Shadows.
    shadow_offset: 2.0, shadow_alpha: 60, shadow_spread: 4.0,
    // Style-preset knobs (P5b): defaults match Aperture (the default preset).
    // The chart-app overrides each frame via set_frame_tokens().
    focus_ring_alpha: 160,
    focus_ring_width: 1.5,
    toast_bg_alpha:   235,
    button_treatment: crate::ui_kit::widgets::tokens::ButtonTreatment::SoftPill,
    wl_row_side_margin: 0.0, wl_row_corner_radius: 0, wl_row_divider_alpha: 0,
    panel_tab_treatment: 0, // Line (PanelHeaderTreatment::Line)
    // Bevel defaults: flat/none (no bevel until the chart-app pushes a themed preset).
    surface_bevel:         crate::design_system::style_system::BevelStyle::None,
    focus_ring:            crate::design_system::style_system::FocusRingStyle::Outline,
    bevel_highlight_alpha: 0,
    bevel_shadow_alpha:    0,
    bevel_highlight_tint: Color32::WHITE,
    bevel_shadow_tint:    Color32::BLACK,
    font_body: 11.0,
    font_caption: 9.0,
    font_section_label: 9.0,
    row_dense:      18.0,
    row_compact:    20.0,
    row_default:    22.0,
    row_spacious:   24.0,
    row_tall:       30.0,
    splitter_width:  8.0,
    pane_gap:        8.0,
    control_xs: 18.0, control_sm: 22.0, control_md: 28.0, control_lg: 34.0, control_xl: 40.0,
    rail_narrow:   240.0,
    rail_medium:   300.0,
    rail_wide:     400.0,
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

/// M1 Change C: patch the ACTIVE PANE's authored bevel tints into the frame
/// snapshot. Called from `setup_theme` (which knows the palette) after the
/// dimension-side `begin_frame` has pushed the snapshot — geometry from the
/// style axis, tint from the colour axis, joined here once per frame.
/// Default bevel tints when a palette authors none — the classic pre-M1
/// look. Named tokens so call sites never hardcode WHITE/BLACK.
pub const BEVEL_TINT_DEFAULT_HIGHLIGHT: Color32 = Color32::WHITE;
pub const BEVEL_TINT_DEFAULT_SHADOW:    Color32 = Color32::BLACK;

// ── M1 Change E: per-frame shadow stacks (non-Copy → own thread-local) ──────
thread_local! {
    static CARD_SHADOW_LAYERS: std::cell::RefCell<
        std::sync::Arc<Vec<crate::design_system::style_system::ShadowLayer>>
    > = std::cell::RefCell::new(std::sync::Arc::new(Vec::new()));
    static MODAL_SHADOW_LAYERS: std::cell::RefCell<
        std::sync::Arc<Vec<crate::design_system::style_system::ShadowLayer>>
    > = std::cell::RefCell::new(std::sync::Arc::new(Vec::new()));
}

thread_local! {
    static NUMERAL_TIER: std::cell::Cell<
        Option<crate::design_system::style_system::NumeralTier>
    > = std::cell::Cell::new(None);
}

/// M1 Change D setters/getters (pushed by `begin_frame`).
pub fn set_numeral_tier(nt: Option<crate::design_system::style_system::NumeralTier>) {
    NUMERAL_TIER.with(|c| c.set(nt));
}
/// The authored display-numeral treatment (None = classic mono).
pub fn numeral_tier() -> Option<crate::design_system::style_system::NumeralTier> {
    NUMERAL_TIER.with(|c| c.get())
}

// ── M2.3: SCOPED token overrides ────────────────────────────────────────────
//
// `FRAME_TOKENS_LOCAL` is per-frame global: one snapshot for the whole tree, so
// a subtree could never be denser/roomier than its siblings (the documented
// `Density` contract in CLAUDE.md promised a per-component knob that did not
// exist). `TokenScope` gives the snapshot the same push/pop discipline
// `ThemeScope` gives the palette.
//
// ```ignore
// let _dense = TokenScope::with(|t| { t.gap_md *= 0.85; t.font_sm -= 1.0; });
// render_instrument_panel(ui);   // tighter, siblings unaffected
// ```
#[must_use = "the scope restores the previous tokens on drop; bind it to a variable"]
pub struct TokenScope {
    prev: TokenSnapshot,
}

impl TokenScope {
    /// Push a modified copy of the current frame tokens for this scope.
    pub fn with(f: impl FnOnce(&mut TokenSnapshot)) -> Self {
        let prev = frame_tokens();
        let mut next = prev;
        f(&mut next);
        set_frame_tokens(next);
        Self { prev }
    }

    /// Push a whole snapshot (e.g. a preview host rendering another style).
    pub fn push(snap: TokenSnapshot) -> Self {
        let prev = frame_tokens();
        set_frame_tokens(snap);
        Self { prev }
    }

    /// Scale the density-bearing tokens (gaps + row height) by `factor`.
    /// Mariner's "10% tighter than Alto" as a SCOPED property rather than a
    /// process-global mutation.
    pub fn density(factor: f32) -> Self {
        Self::with(|t| {
            t.gap_xs     *= factor;
            t.gap_xs_mid *= factor;
            t.gap_sm     *= factor;
            t.gap_md     *= factor;
            t.gap_lg     *= factor;
            t.gap_xl     *= factor;
        })
    }
}

impl Drop for TokenScope {
    fn drop(&mut self) {
        set_frame_tokens(self.prev);
    }
}

/// Push the active style's authored shadow stacks (called from `begin_frame`).
pub fn set_card_shadow_layers(
    card: Vec<crate::design_system::style_system::ShadowLayer>,
    modal: Vec<crate::design_system::style_system::ShadowLayer>,
) {
    CARD_SHADOW_LAYERS.with(|c| *c.borrow_mut() = std::sync::Arc::new(card));
    MODAL_SHADOW_LAYERS.with(|c| *c.borrow_mut() = std::sync::Arc::new(modal));
}

/// The authored card shadow stack for this frame (empty = use the legacy
/// single-spec `shadow_card_themed` path).
pub fn card_shadow_layers()
-> std::sync::Arc<Vec<crate::design_system::style_system::ShadowLayer>> {
    CARD_SHADOW_LAYERS.with(|c| c.borrow().clone())
}

/// The authored modal shadow stack for this frame.
pub fn modal_shadow_layers()
-> std::sync::Arc<Vec<crate::design_system::style_system::ShadowLayer>> {
    MODAL_SHADOW_LAYERS.with(|c| c.borrow().clone())
}

/// Resolve a layer tint against the active palette snapshot.
/// `Shadow` → the caller-supplied palette shadow colour; `Highlight` → the
/// frame's bevel-highlight tint (authored per palette, WHITE default).
pub fn resolve_shadow_tint(
    tint: crate::design_system::style_system::ShadowTint,
    palette_shadow: Color32,
) -> Color32 {
    use crate::design_system::style_system::ShadowTint as T;
    match tint {
        T::Shadow    => palette_shadow,
        T::Highlight => frame_tokens().bevel_highlight_tint,
        T::Custom(c) => Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]),
    }
}

/// Paint an authored multi-layer card shadow stack around/inside `rect`.
/// OUTER layers approximate the CSS drop with egui's epaint Shadow tessellation
/// per layer; INSET layers (all blur==0 in the six DS specs) paint as 1px edge
/// strokes clipped to the rect — no blur pass. Returns true when it painted
/// (caller then SKIPS the legacy single-shadow path).
pub fn paint_shadow_stack(
    painter: &egui::Painter,
    rect: egui::Rect,
    radius: egui::CornerRadius,
    layers: &[crate::design_system::style_system::ShadowLayer],
    palette_shadow: Color32,
) -> bool {
    if layers.is_empty() || !rect.is_finite() { return false; }
    for l in layers {
        let base = resolve_shadow_tint(l.tint, palette_shadow);
        let col = Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), l.alpha);
        if l.inset {
            // Inset: top edge (positive offset_y) or bottom edge (negative),
            // 1px stroke inside the rect — matches the CSS inset hairlines.
            let y = if l.offset_y >= 0.0 { rect.top() + 0.5 + l.offset_y - 1.0 }
                    else { rect.bottom() - 0.5 + l.offset_y + 1.0 };
            let r = radius.nw.max(radius.ne) as f32;
            let inset_x = (r * 0.5).clamp(0.0, 3.0);
            painter.line_segment(
                [egui::pos2(rect.left() + inset_x, y), egui::pos2(rect.right() - inset_x, y)],
                egui::Stroke::new(1.0, col),
            );
        } else {
            let sh = egui::epaint::Shadow {
                offset: [l.offset_x as i8, l.offset_y as i8],
                blur:   l.blur as u8,
                spread: l.spread.max(0.0) as u8, // egui Shadow has no negative spread; clamp (CSS -16px approximated by blur)
                color:  col,
            };
            painter.add(sh.as_shape(rect, radius));
        }
    }
    true
}

pub fn set_frame_bevel_tints(highlight: Color32, shadow: Color32) {
    FRAME_TOKENS_LOCAL.with(|c| {
        let mut snap = c.get();
        snap.bevel_highlight_tint = highlight;
        snap.bevel_shadow_tint = shadow;
        c.set(snap);
    });
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

// `gap_xs_mid` lives with the rest of the spacing ladder further down, beside
// gap_2xs..gap_3xl. It used to be declared here, ~240 lines from its siblings,
// and was the only rung that did not apply `spacing_scale_override()` — which
// is exactly the kind of thing distance hides. See the ladder for the defect
// that caused.
// Radius helpers apply the user's CornerScale override multiplier (Sharp/Subtle
// /Standard/Round). Standard = 1.0× is the no-op; Sharp = 0.0× returns zero for
// every tier (square-corner Meridien aesthetic). Override defaults to Standard.
#[inline] pub fn radius_xs()  -> f32 { frame_tokens().radius_xs * corner_scale_override().scale() }
#[inline] pub fn radius_sm()  -> f32 { frame_tokens().radius_sm * corner_scale_override().scale() }
#[inline] pub fn radius_md()  -> f32 { frame_tokens().radius_md * corner_scale_override().scale() }
#[inline] pub fn radius_lg()  -> f32 { frame_tokens().radius_lg * corner_scale_override().scale() }

// Stroke helpers apply the user's BorderWeight override (Hairline 0.5× /
// Standard 1.0× / Bold 1.5×). Standard = no-op; Hairline minimises every
// border across the app, Bold thickens them. Override defaults to Standard.
#[inline] pub fn stroke_hair()   -> f32 { frame_tokens().stroke_hair   * border_weight_override().scale() }
#[inline] pub fn stroke_thin()   -> f32 { frame_tokens().stroke_thin   * border_weight_override().scale() }
#[inline] pub fn stroke_medium() -> f32 { frame_tokens().stroke_medium * border_weight_override().scale() }
#[inline] pub fn stroke_std()    -> f32 { frame_tokens().stroke_std    * border_weight_override().scale() }
#[inline] pub fn stroke_bold()   -> f32 { frame_tokens().stroke_bold   * border_weight_override().scale() }
#[inline] pub fn stroke_thick()  -> f32 { frame_tokens().stroke_thick  * border_weight_override().scale() }
#[inline] pub fn stroke_extra_thick() -> f32 { frame_tokens().stroke_extra_thick * border_weight_override().scale() }
/// Decorative / accent rule (3 px). Formerly `stroke_heavy()`, renamed to stop
/// colliding with the two other unrelated "heavy" stroke values.
#[inline] pub fn stroke_rule()   -> f32 { frame_tokens().stroke_rule   * border_weight_override().scale() }

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
#[inline] pub fn alpha_scrim()   -> u8 { frame_tokens().alpha_scrim }
#[inline] pub fn alpha_dense()      -> u8 { frame_tokens().alpha_dense }

/// The elevation ladder. `ui_kit::widgets::shadow`'s tier constructors held
/// these as bare literals, so no theme could author its own depth.
#[inline] pub fn elev_sm() -> crate::design_system::style_system::ShadowTier { frame_tokens().elev_sm }
#[inline] pub fn elev_md() -> crate::design_system::style_system::ShadowTier { frame_tokens().elev_md }
#[inline] pub fn elev_lg() -> crate::design_system::style_system::ShadowTier { frame_tokens().elev_lg }
#[inline] pub fn elev_xl() -> crate::design_system::style_system::ShadowTier { frame_tokens().elev_xl }
#[inline] pub fn alpha_near_solid() -> u8 { frame_tokens().alpha_near_solid }
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
#[inline] pub fn mono_xl()  -> egui::FontId { egui::FontId::new(font_xl(),  egui::FontFamily::Monospace) }

/// Monospace at an arbitrary (computed) size — for the handful of call sites
/// that derive a size from a caller-supplied base (`base - 1.0`, `base * 1.1`)
/// rather than picking a rung on the ladder. Prefer the named tiers above.
#[inline] pub fn mono_at(size: f32) -> egui::FontId { egui::FontId::new(size, egui::FontFamily::Monospace) }

// ─── Proportional font helpers ───────────────────────────────────────────────
// The mirror of the `mono_*` family for UI chrome (labels, buttons, headings,
// hero numbers). Returns FontId so the family is explicit at the call site and
// the SIZE routes through the live token ladder instead of a frozen literal.
#[inline] pub fn prop_4xs() -> egui::FontId { egui::FontId::new(font_4xs(), egui::FontFamily::Proportional) }
#[inline] pub fn prop_3xs() -> egui::FontId { egui::FontId::new(font_3xs(), egui::FontFamily::Proportional) }
#[inline] pub fn prop_2xs() -> egui::FontId { egui::FontId::new(font_2xs(), egui::FontFamily::Proportional) }
#[inline] pub fn prop_xs()  -> egui::FontId { egui::FontId::new(font_xs(),  egui::FontFamily::Proportional) }
#[inline] pub fn prop_xs_plus() -> egui::FontId { egui::FontId::new(font_xs_plus(), egui::FontFamily::Proportional) }
#[inline] pub fn prop_sm()  -> egui::FontId { egui::FontId::new(font_sm(),  egui::FontFamily::Proportional) }
#[inline] pub fn prop_md()  -> egui::FontId { egui::FontId::new(font_md(),  egui::FontFamily::Proportional) }
#[inline] pub fn prop_md_plus() -> egui::FontId { egui::FontId::new(font_md_plus(), egui::FontFamily::Proportional) }
#[inline] pub fn prop_lg()  -> egui::FontId { egui::FontId::new(font_lg(),  egui::FontFamily::Proportional) }
#[inline] pub fn prop_xl()  -> egui::FontId { egui::FontId::new(font_xl(),  egui::FontFamily::Proportional) }

// Display tier — hero numbers / KPI digits.
#[inline] pub fn prop_display_sm() -> egui::FontId { egui::FontId::new(font_display_sm(), egui::FontFamily::Proportional) }
#[inline] pub fn prop_display_md() -> egui::FontId { egui::FontId::new(font_display_md(), egui::FontFamily::Proportional) }
#[inline] pub fn prop_display_lg() -> egui::FontId { egui::FontId::new(font_display_lg(), egui::FontFamily::Proportional) }
#[inline] pub fn prop_display_xl() -> egui::FontId { egui::FontId::new(font_display_xl(), egui::FontFamily::Proportional) }

/// Proportional at an arbitrary (computed) size — see `mono_at`.
#[inline] pub fn prop_at(size: f32) -> egui::FontId { egui::FontId::new(size, egui::FontFamily::Proportional) }

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
// Large hero numbers in chart-widget bodies (KPIs, countdown digits). These
// are pure constants so `ui_kit/style.rs` has zero `crate::dt_*!` macro
// dependencies — important for the workspace-crate extraction. The previous
// `dt_f32!(font.xxl, 28.0)` versions defaulted to these literals anyway when
// `design-mode` was off (shipping builds), and the TOML override path is
// preserved via `TokenSnapshot` — hosts that push a TokenSnapshot with the
// display tier extended can drive these from outside.
/// Largest UI heading rung (Display/HeadingLg tiers). Mirrors the chart-side
/// helper so the moved text_style.rs needs no chart import.
#[inline] pub fn font_2xl() -> f32 { font_xl() + 6.0 }
/// Per-style semantic sizes (M2.1 — pushed by begin_frame from StyleSettings).
#[inline] pub fn font_body() -> f32 { frame_tokens().font_body }
#[inline] pub fn font_caption() -> f32 { frame_tokens().font_caption }
#[inline] pub fn font_section_label() -> f32 { frame_tokens().font_section_label }
#[inline] pub fn font_display_sm() -> f32 { frame_tokens().font_display_sm }
#[inline] pub fn font_display_md() -> f32 { frame_tokens().font_display_md }
#[inline] pub fn font_display_lg() -> f32 { frame_tokens().font_display_lg }
#[inline] pub fn font_display_xl() -> f32 { frame_tokens().font_display_xl }

// ─── Icon control sizes ──────────────────────────────────────────────────────
#[inline] pub fn icon_xs() -> f32 { frame_tokens().icon_xs }
#[inline] pub fn icon_sm() -> f32 { frame_tokens().icon_sm }
#[inline] pub fn icon_md() -> f32 { frame_tokens().icon_md }
#[inline] pub fn icon_lg() -> f32 { frame_tokens().icon_lg }

// ─── Row heights ─────────────────────────────────────────────────────────────
// M4.5: per-style now — sourced from the frame snapshot (which begin_frame
// fills from the active StyleSystem's Density ladder) instead of hard
// literals, so a theme can author its own PROPORTIONS. Defaults are the
// former literals, so unauthored styles are byte-identical.
#[inline] pub fn row_height_dense()     -> f32 { frame_tokens().row_dense }
#[inline] pub fn row_height_compact()   -> f32 { frame_tokens().row_compact }
#[inline] pub fn row_height_default()   -> f32 { frame_tokens().row_default }
#[inline] pub fn row_height_spacious()  -> f32 { frame_tokens().row_spacious }
#[inline] pub fn row_height_tall()      -> f32 { frame_tokens().row_tall }
/// Splitter / drag-handle thickness (per-style).
#[inline] pub fn splitter_width()       -> f32 { frame_tokens().splitter_width }
/// Mosaic pane gutter (per-style). Prefer this over reading
/// `chart_renderer::ui::style::current().pane_gap` at paint time: that is the
/// bottom of the cascade and skips the inspector / hot-reload layers above it.
#[inline] pub fn pane_gap()             -> f32 { frame_tokens().pane_gap }
/// Side-rail width presets (per-style).
/// Smallest side any interactive control may present, in px.
///
/// AUDIT 2026-08 — ONE number, two users. The dev-inspector's `/design-audit`
/// hardcoded `28.0` in its touch-target check while `toolbar_control_h()` had
/// no floor at all, so a style could author a 24px control height and the app
/// would render it and then report itself non-compliant every frame. An
/// enforcement threshold and the layout it governs cannot be two separate
/// literals: whoever changes one silently puts the app in violation of the
/// other.
///
/// Applied as a FLOOR only, the same shape as `toolnav_min_height()`: a style
/// asking for a TALLER control keeps exactly the height it authored (Aperture's
/// 32 is untouched); a style asking for a shorter one is raised to this.
pub const MIN_TOUCH_TARGET_PX: f32 = 28.0;

/// Smallest side for controls inside a PANE HEADER, in px.
///
/// The pane header is 28px tall, so a chip meeting `MIN_TOUCH_TARGET_PX` fills
/// it edge to edge with no padding. Meeting the primary minimum there requires
/// a taller header — more chrome, less chart, on every pane of a mosaic — which
/// is the opposite of what this chrome is for.
///
/// Declared HERE, next to the number it relaxes, rather than as a magic
/// exemption inside the audit. Two thresholds is one more than ideal; two
/// thresholds a reader can see side by side is much better than one threshold
/// plus a list of widgets that quietly do not have to meet it.
///
/// This is a mouse-driven desktop terminal: 24px is a comfortable target under
/// Fitts's law at pointer speed. If this app ever ships a touch surface, this
/// constant is the thing to delete, and the pane header has to grow.
pub const MIN_PANE_CHROME_TARGET_PX: f32 = 24.0;

#[inline] pub fn control_h_xs()         -> f32 { frame_tokens().control_xs }
#[inline] pub fn control_h_sm()         -> f32 { frame_tokens().control_sm }
#[inline] pub fn control_h_md()         -> f32 { frame_tokens().control_md }
#[inline] pub fn control_h_lg()         -> f32 { frame_tokens().control_lg }
#[inline] pub fn control_h_xl()         -> f32 { frame_tokens().control_xl }
#[inline] pub fn rail_width_narrow()    -> f32 { frame_tokens().rail_narrow }
#[inline] pub fn rail_width_medium()    -> f32 { frame_tokens().rail_medium }
#[inline] pub fn rail_width_wide()      -> f32 { frame_tokens().rail_wide }

// ─── Card padding ────────────────────────────────────────────────────────────

// ─── Divider insets ──────────────────────────────────────────────────────────

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

#[inline] pub fn font_4xs()    -> f32 { frame_tokens().font_4xs }
// 3xs sits one step under 2xs. It was pinned at 8.0 while `font_2xs` was ALSO
// 8.0, so the two tiers were indistinguishable (a duplicate rung on the ladder).
// Now derived: always exactly 1px under 2xs, with an 8px legibility floor.
#[inline] pub fn font_3xs()    -> f32 { (font_2xs() - 1.0).max(8.0) }
#[inline] pub fn font_2xs()    -> f32 { frame_tokens().font_2xs }
#[inline] pub fn font_xs()     -> f32 { frame_tokens().font_xs }
#[inline] pub fn font_xs_plus() -> f32 { frame_tokens().font_xs_plus }
#[inline] pub fn font_sm()     -> f32 { frame_tokens().font_sm }
#[inline] pub fn font_md()     -> f32 { frame_tokens().font_md }
#[inline] pub fn font_md_plus() -> f32 { frame_tokens().font_md_plus }
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
// Gap helpers apply the user's SpacingScale override (Tight 0.75× / Standard
// 1.0× / Loose 1.25×). Standard = no-op; Tight condenses every gap, Loose
// spreads them. Override defaults to Standard.
#[inline] pub fn gap_2xs() -> f32 { frame_tokens().gap_2xs * spacing_scale_override().scale() }
#[inline] pub fn gap_xs()  -> f32 { frame_tokens().gap_xs  * spacing_scale_override().scale() }
// The 6.0 rung between gap_xs and gap_sm. It was declared ~240 lines above the
// rest of the ladder and was the ONLY rung without the scale multiplier, so
// the ladder collapsed at Tight (0.75x): gap_xs 3.0, gap_xs_mid 6.0 (frozen),
// gap_sm 6.0 — the mid rung landed exactly on the rung above it and the
// distinction between them disappeared. Byte-identical at Standard (1.0x),
// which is why nothing caught it.
#[inline] pub fn gap_xs_mid() -> f32 { frame_tokens().gap_xs_mid * spacing_scale_override().scale() }
#[inline] pub fn gap_sm()  -> f32 { frame_tokens().gap_sm  * spacing_scale_override().scale() }
#[inline] pub fn gap_md()  -> f32 { frame_tokens().gap_md  * spacing_scale_override().scale() }
#[inline] pub fn gap_lg()  -> f32 { frame_tokens().gap_lg  * spacing_scale_override().scale() }
#[inline] pub fn gap_xl()  -> f32 { frame_tokens().gap_xl  * spacing_scale_override().scale() }
#[inline] pub fn gap_2xl() -> f32 { frame_tokens().gap_2xl * spacing_scale_override().scale() }
#[inline] pub fn gap_3xl() -> f32 { frame_tokens().gap_3xl * spacing_scale_override().scale() }

pub const GAP_2XS:    f32 =  2.0;
pub const GAP_XS:     f32 =  4.0;
pub const GAP_SM:     f32 =  8.0;
pub const GAP_MD:     f32 = 12.0;
pub const GAP_LG:     f32 = 16.0;
pub const GAP_XL:     f32 = 20.0;
pub const GAP_2XL:    f32 = 24.0;
pub const GAP_3XL:    f32 = 32.0;

// ─── Stroke widths (px) — pure constants ─────────────────────────────────────

// (`stroke_extra_thick` / `stroke_rule` are token-backed accessors now and
// live with the rest of the stroke ladder above.)

// ─── Radii (px) — pure constants ─────────────────────────────────────────────

/// AUDIT 2026-08 (M0.3 accept criterion): every sibling radius accessor
/// multiplies by `corner_scale_override()` — this one did not. Under
/// `CornerScale::Sharp` (0.0x) every tier squared EXCEPT pills, which stayed at
/// 99, so the plan's own acceptance test ("Sharp squares everything uniformly",
/// exit criterion P-9) could never pass. One missing multiplier.
pub fn radius_pill() -> f32 { frame_tokens().radius_pill * corner_scale_override().scale() }

// ─── Alpha (0..=255) — pure constants ────────────────────────────────────────

pub fn alpha_whisper() -> u8 { frame_tokens().alpha_whisper }
pub fn alpha_hint()    -> u8 { frame_tokens().alpha_hint }

// ─── Elevation factors (gamma multipliers over `bg()`) ───────────────────────
//
// Used by `ComponentTheme::header_surface()` / `section_header_surface()` /
// `panel_surface()` default impls so the elevation ramp lives in one place
// and is portable across themes.

pub const ELEVATION_1_FACTOR: f32 = 0.95;
pub const ELEVATION_2_FACTOR: f32 = 0.88;
pub const ELEVATION_3_FACTOR: f32 = 0.85;

// ─── Elevation surface shift (2026-07-30 fix) ────────────────────────────────
//
// The gamma-multiplier ladder above (0.95/0.88/0.85) only ever DARKENS `bg`.
// On a near-black theme like Aperture (bg ≈ #000) that collapses every surface
// back to black — zero visible depth, which is exactly why dark themes rendered
// flat vs the ApexTerminalThemes mockup (panels lift off the canvas there).
//
// `elevate()` replaces the multiply with an additive luminance shift toward the
// contrast direction: dark bg → lighter (raised card), light bg → darker (inset
// panel). This mirrors `hairline_border`'s dark/light-aware philosophy and makes
// the depth ramp visible on ALL palettes, near-black included. Amounts are 0-255
// channel deltas; larger = more lift. Light themes take a gentler shift so cream
// editorial palettes get readable panels without going muddy.
#[inline]
pub fn elevate(bg: egui::Color32, amount: i16) -> egui::Color32 {
    let (r, g, b) = (bg.r() as i16, bg.g() as i16, bg.b() as i16);
    let is_dark = (r + g + b) < 384;
    // Dark: lighten by `amount`. Light: darken, but gentler (3/5) so light
    // palettes keep a soft inset rather than a heavy slab.
    let s: i16 = if is_dark { amount } else { -((amount * 3) / 5) };
    let c = |v: i16| (v + s).clamp(0, 255) as u8;
    egui::Color32::from_rgb(c(r), c(g), c(b))
}

/// Per-role elevation shift amounts (channel deltas passed to [`elevate`]).
/// Ordering preserves the legacy ramp — panel header is the most-lifted surface,
/// body the least — just made additive so it survives near-black backgrounds.
// Amounts tuned to the ApexTerminalThemes mockup ladder (Aperture: bg #000 →
// panel #141311 ≈ +19 → surface #1a1816 ≈ +26 → elevated #1f1d1a ≈ +31).
pub const ELEVATE_PANEL_HEADER:  i16 = 30; // SidePanelShell / chart pane header band
pub const ELEVATE_PANEL_SECTION: i16 = 26; // PanelSection / sub-section header
pub const ELEVATE_PANEL_BODY:    i16 = 20; // side-panel body card (lifts off canvas)
pub const ELEVATE_CARD:          i16 = 22; // resting card / tile
pub const ELEVATE_RAISED:        i16 = 30; // popover / inline editor
pub const ELEVATE_MODAL:         i16 = 38; // modal / dialog (highest Z)

// ─── Line-height multipliers (P2.5) ─────────────────────────────────────────
//
// Multipliers applied to font size to derive line height. Replace the bare
// floats scattered through TextSpec in chart/renderer/ui/foundation/text_style.rs
// and any per-call `RichText::new(..).size(s).line_height(s * 1.3)` patterns.

/// 1.2 — display / hero text, tight stack.
#[inline] pub fn line_tight()   -> f32 { frame_tokens().line_tight }
/// 1.25 — large heading.
#[inline] pub fn line_heading() -> f32 { frame_tokens().line_heading }
/// 1.3 — caption / label / mono.
#[inline] pub fn line_dense()   -> f32 { frame_tokens().line_dense }
/// 1.35 — small body / small mono.
#[inline] pub fn line_compact() -> f32 { frame_tokens().line_compact }
/// 1.4 — body / readable copy. Default for paragraph text.
#[inline] pub fn line_normal()  -> f32 { frame_tokens().line_normal }
/// 1.5 — loose / generously-spaced paragraph.
#[inline] pub fn line_loose()   -> f32 { frame_tokens().line_loose }

// ─── Motion timing (P2.5) ────────────────────────────────────────────────────
//
// Standardised animation durations in milliseconds. Use these instead of
// inline magic numbers when adding animations.

// Motion helpers apply the user's MotionSpeed override (Off 0× / Fast 0.5× /
// Standard 1.0× / Slow 1.5×). Off makes every animation snap instant (good
// for accessibility / over-RDP / power-user mode); Slow gives a comfortable
// pace for demos. Override defaults to Standard. Reads on the hot path go
// through one atomic load + multiplication.

#[inline]
fn motion_scale() -> f32 { motion_speed_override().scale() }

// The millisecond motion ladder (`motion_instant/fast/std/slow/xslow`) lived
// here and is REMOVED (audit 2026-08). It was a second, parallel motion scale:
// milliseconds and override-aware, against `ui_kit::widgets::motion`'s
// `FAST/MED/SLOW` in seconds, which is what every animation actually uses (52
// call sites between FAST and MED). It had zero consumers.
//
// Two ladders for one axis is the failure; which one survives is a detail. The
// seconds one won because it is the one the app calls, and it is now
// override-aware too — `motion::scaled()` applies `motion_speed_override()` at
// the `ease_bool` / `ease_value` chokepoint, so the user's Motion setting acts
// on all 52 sites rather than on the zero that read this ladder.

// ─── Density (P4.3) ──────────────────────────────────────────────────────────
//
// Typed enum replacing the legacy `StyleSettings.density: u8` (0/1/2). Carries
// the multiplicative scale applied to row heights, button heights, and tab
// heights, exposed as a single source of truth so the chart-app's three
// density-aware helpers (`style_row_height`, `style_button_height`,
// `style_tab_height`) and any future ui_kit consumers compute the same value.
//
// **Future per-user override**: a `Watchlist.density_override: Option<DensityMode>`
// field can let users pick Compact/Standard/Spacious independently of the
// active style preset; today the density value is preset-baked
// (`StyleSettings.density` set per Aperture/Octave/Meridien). The scaffolding
// to add that override is straightforward: extend `DensityMode::from_u8`
// callers to consult the override first, then fall back to the preset value.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum DensityMode {
    /// 0.85× — compact rows for power users / dense data displays.
    Compact,
    #[default]
    /// 1.0× — standard density (the default for most styles).
    Standard,
    /// 1.15× — spacious / touch-friendly / accessibility-leaning.
    Spacious,
}

// ─── Generic 5-tier token scale (P5) ─────────────────────────────────────────
//
// `BorderWeight`, `CornerScale`, `SpacingScale`, `MotionSpeed` and `ElevationLevel`
// all expose the same shape: a typed enum carrying a `scale()` multiplier applied
// to a token tier at the read site, plus from_u8/as_u8/label/all helpers so the
// settings panel can render them as toggle-button rows identical to the
// DensityMode picker.

impl DensityMode {
    /// Multiplier applied to height tokens (rows, buttons, tabs).
    #[inline]
    pub fn scale(self) -> f32 {
        match self {
            DensityMode::Compact  => 0.85,
            DensityMode::Standard => 1.0,
            DensityMode::Spacious => 1.15,
        }
    }

    /// Decode from the legacy `StyleSettings.density: u8` field (0/1/2).
    /// Any out-of-range value falls back to Standard.
    #[inline]
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => DensityMode::Compact,
            2 => DensityMode::Spacious,
            _ => DensityMode::Standard,
        }
    }

    /// Encode to the legacy `StyleSettings.density: u8` field (0/1/2).
    #[inline]
    pub fn as_u8(self) -> u8 {
        match self {
            DensityMode::Compact  => 0,
            DensityMode::Standard => 1,
            DensityMode::Spacious => 2,
        }
    }

    /// Display label for UI pickers.
    #[inline]
    pub fn label(self) -> &'static str {
        match self {
            DensityMode::Compact  => "Compact",
            DensityMode::Standard => "Standard",
            DensityMode::Spacious => "Spacious",
        }
    }

    /// All variants, ordered for picker rendering.
    #[inline]
    pub fn all() -> &'static [DensityMode] {
        &[DensityMode::Compact, DensityMode::Standard, DensityMode::Spacious]
    }
}

// ─── BorderWeight ────────────────────────────────────────────────────────────
//
// Multiplier applied to every `stroke_*()` token. Hairline = 0.5× makes every
// border render at half thickness (the Meridien aesthetic); Bold = 1.5× makes
// every border heavier (good for high-density chart annotations).

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum BorderWeight {
    Hairline, // 0.5× — minimal chrome
    #[default]
    Standard, // 1.0× — preset default
    Bold,     // 1.5× — heavier borders
}

impl BorderWeight {
    #[inline]
    pub fn scale(self) -> f32 {
        match self {
            BorderWeight::Hairline => 0.5,
            BorderWeight::Standard => 1.0,
            BorderWeight::Bold     => 1.5,
        }
    }
    #[inline]
    pub fn from_u8(v: u8) -> Self {
        match v { 0 => Self::Hairline, 2 => Self::Bold, _ => Self::Standard }
    }
    #[inline]
    pub fn as_u8(self) -> u8 {
        match self { Self::Hairline => 0, Self::Standard => 1, Self::Bold => 2 }
    }
    #[inline]
    pub fn label(self) -> &'static str {
        match self { Self::Hairline => "Hairline", Self::Standard => "Standard", Self::Bold => "Bold" }
    }
    #[inline]
    pub fn all() -> &'static [BorderWeight] {
        &[BorderWeight::Hairline, BorderWeight::Standard, BorderWeight::Bold]
    }
}

// ─── CornerScale ─────────────────────────────────────────────────────────────
//
// Multiplier applied to every `radius_*()` token. Sharp = 0× (zero rounding —
// the Meridien square-corner aesthetic); Round = 1.5× (juicier rounding).

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum CornerScale {
    Sharp,    // 0×    — all corners flat
    Subtle,   // 0.5×  — barely rounded
    #[default]
    Standard, // 1.0×  — preset default
    Round,    // 1.5×  — generous rounding
}

impl CornerScale {
    #[inline]
    pub fn scale(self) -> f32 {
        match self {
            CornerScale::Sharp    => 0.0,
            CornerScale::Subtle   => 0.5,
            CornerScale::Standard => 1.0,
            CornerScale::Round    => 1.5,
        }
    }
    #[inline]
    pub fn from_u8(v: u8) -> Self {
        match v { 0 => Self::Sharp, 1 => Self::Subtle, 3 => Self::Round, _ => Self::Standard }
    }
    #[inline]
    pub fn as_u8(self) -> u8 {
        match self { Self::Sharp => 0, Self::Subtle => 1, Self::Standard => 2, Self::Round => 3 }
    }
    #[inline]
    pub fn label(self) -> &'static str {
        match self {
            Self::Sharp    => "Sharp",
            Self::Subtle   => "Subtle",
            Self::Standard => "Standard",
            Self::Round    => "Round",
        }
    }
    #[inline]
    pub fn all() -> &'static [CornerScale] {
        &[CornerScale::Sharp, CornerScale::Subtle, CornerScale::Standard, CornerScale::Round]
    }
}

// ─── SpacingScale ────────────────────────────────────────────────────────────
//
// Multiplier applied to every `gap_*()` token. Tight = 0.75× condenses
// padding/gutters; Loose = 1.25× spreads them.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SpacingScale {
    Tight,    // 0.75×
    #[default]
    Standard, // 1.0×
    Loose,    // 1.25×
}

impl SpacingScale {
    #[inline]
    pub fn scale(self) -> f32 {
        match self {
            SpacingScale::Tight    => 0.75,
            SpacingScale::Standard => 1.0,
            SpacingScale::Loose    => 1.25,
        }
    }
    #[inline]
    pub fn from_u8(v: u8) -> Self {
        match v { 0 => Self::Tight, 2 => Self::Loose, _ => Self::Standard }
    }
    #[inline]
    pub fn as_u8(self) -> u8 {
        match self { Self::Tight => 0, Self::Standard => 1, Self::Loose => 2 }
    }
    #[inline]
    pub fn label(self) -> &'static str {
        match self { Self::Tight => "Tight", Self::Standard => "Standard", Self::Loose => "Loose" }
    }
    #[inline]
    pub fn all() -> &'static [SpacingScale] {
        &[SpacingScale::Tight, SpacingScale::Standard, SpacingScale::Loose]
    }
}

// ─── MotionSpeed ─────────────────────────────────────────────────────────────
//
// Multiplier applied to every `motion_*()` duration token. Off = 0× (skip all
// animations, snap instantly — accessibility / power-user mode); Fast = 0.5×;
// Slow = 1.5×.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum MotionSpeed {
    Off,      // 0×    — instant, no animation
    Fast,     // 0.5×  — half-duration
    #[default]
    Standard, // 1.0×  — default timings
    Slow,     // 1.5×  — relaxed
}

impl MotionSpeed {
    #[inline]
    pub fn scale(self) -> f32 {
        match self {
            MotionSpeed::Off      => 0.0,
            MotionSpeed::Fast     => 0.5,
            MotionSpeed::Standard => 1.0,
            MotionSpeed::Slow     => 1.5,
        }
    }
    #[inline]
    pub fn from_u8(v: u8) -> Self {
        match v { 0 => Self::Off, 1 => Self::Fast, 3 => Self::Slow, _ => Self::Standard }
    }
    #[inline]
    pub fn as_u8(self) -> u8 {
        match self { Self::Off => 0, Self::Fast => 1, Self::Standard => 2, Self::Slow => 3 }
    }
    #[inline]
    pub fn label(self) -> &'static str {
        match self {
            Self::Off      => "Off",
            Self::Fast     => "Fast",
            Self::Standard => "Standard",
            Self::Slow     => "Slow",
        }
    }
    #[inline]
    pub fn all() -> &'static [MotionSpeed] {
        &[MotionSpeed::Off, MotionSpeed::Fast, MotionSpeed::Standard, MotionSpeed::Slow]
    }
}

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

// ─── Style-driven tab treatment ──────────────────────────────────────────────

/// Returns the default `TabTreatment` for the active style preset, read from
/// the per-frame `TokenSnapshot`. Call sites that DON'T chain `.treatment()`
/// will pick this up automatically when constructors use it as their default.
/// Falls back to `TabTreatment::Line` if the frame hasn't been initialized.
#[inline]
pub fn style_tab_treatment() -> crate::ui_kit::widgets::TabTreatment {
    use crate::ui_kit::widgets::TabTreatment;
    match frame_tokens().panel_tab_treatment {
        1 => TabTreatment::Segmented,
        2 => TabTreatment::Filled,
        3 => TabTreatment::Card,
        4 => TabTreatment::Pane,
        _ => TabTreatment::Line,
    }
}

/// Returns the active-pane indicator style.
///
/// **S3 status**: the `TokenSnapshot` does not yet carry a `pane_active_indicator`
/// field because adding a required field would break the chart-side `begin_frame`
/// struct literal (out of scope for S3). This accessor returns the typed enum
/// Default (`PaneActiveIndicator::TopStripe`) until a later round adds the field
/// and wires `begin_frame` to populate it.
///
/// Once the field is added, this body becomes:
/// ```ignore
/// PaneActiveIndicator::from_u8(frame_tokens().pane_active_indicator)
/// ```
#[inline]
pub fn style_pane_active_indicator() -> crate::design_system::style_system::PaneActiveIndicator {
    crate::design_system::style_system::PaneActiveIndicator::default()
}


// ─── Surface bevel (portable) ────────────────────────────────────────────────
//
// Portable analogue of the React ApexTerminalThemes `box-shadow: inset …`
// faces used by Alto/Mariner/Cadence. Reads from the per-frame TokenSnapshot
// so the chart-app's themed `surface_bevel` knob drives it — no chart-side
// import required. Guards against degenerate rects (NaN / sub-pixel) so it
// is safe to call unconditionally at every button/header paint site.

/// Paint a 1px inner highlight on the top edge + 1px inner shadow on the
/// bottom edge of `rect`. No-op when the active style's bevel is `None`, or
/// the rect is degenerate. Tints are palette-independent (white/black with
/// theme-tuned alphas), matching the React `rgba(255…)/rgba(0…)` CSS values.
pub fn paint_bevel_portable(painter: &egui::Painter, rect: egui::Rect, radius: egui::CornerRadius) {
    use crate::design_system::style_system::BevelStyle;
    let snap = frame_tokens();
    // M1 Change C: tint is authorable per palette (WHITE/BLACK when unauthored).
    let ht = snap.bevel_highlight_tint;
    let stn = snap.bevel_shadow_tint;
    let hi = Color32::from_rgba_unmultiplied(ht.r(), ht.g(), ht.b(), snap.bevel_highlight_alpha);
    let sh = Color32::from_rgba_unmultiplied(stn.r(), stn.g(), stn.b(), snap.bevel_shadow_alpha);
    let (top_col, bot_col) = match snap.surface_bevel {
        BevelStyle::None  => return,
        BevelStyle::Raised => (hi, sh),
        BevelStyle::Inset  => (sh, hi),
    };
    if !rect.is_finite() || rect.width() < 1.0 || rect.height() < 1.0 { return; }
    let r = radius.nw.max(radius.ne).max(radius.sw).max(radius.se) as f32;
    let inset = (r * 0.5).clamp(0.0, 3.0);
    let y_top = rect.top() + 0.5;
    let y_bot = rect.bottom() - 0.5;
    painter.line_segment(
        [egui::pos2(rect.left() + inset, y_top), egui::pos2(rect.right() - inset, y_top)],
        egui::Stroke::new(1.0, top_col),
    );
    painter.line_segment(
        [egui::pos2(rect.left() + inset, y_bot), egui::pos2(rect.right() - inset, y_bot)],
        egui::Stroke::new(1.0, bot_col),
    );
}

// ─── User token-scale overrides (P5) ─────────────────────────────────────────
//
// Four global AtomicI8 slots store the user's BorderWeight / CornerScale /
// SpacingScale / MotionSpeed choices. Negative = no override (use the
// preset/default 1.0× scale); 0..=N maps to the enum's `from_u8`. The token
// reader helpers (radius_*/stroke_*/gap_*/motion_*) consult these atomics on
// every read — single relaxed atomic load, single multiplication. No lock
// contention; the values are written infrequently (only when the user clicks
// a picker in the settings panel).

use std::sync::atomic::{AtomicI8, Ordering};

static BORDER_WEIGHT_OVERRIDE:  AtomicI8 = AtomicI8::new(-1);
static CORNER_SCALE_OVERRIDE:   AtomicI8 = AtomicI8::new(-1);
static SPACING_SCALE_OVERRIDE:  AtomicI8 = AtomicI8::new(-1);
static MOTION_SPEED_OVERRIDE:   AtomicI8 = AtomicI8::new(-1);

/// Host-side: set the BorderWeight override. `None` clears it (use preset).
pub fn set_border_weight_override(mode: Option<BorderWeight>) {
    BORDER_WEIGHT_OVERRIDE.store(mode.map(|m| m.as_u8() as i8).unwrap_or(-1), Ordering::Release);
}
/// Read the override (or `Standard` if unset).
#[inline]
pub fn border_weight_override() -> BorderWeight {
    let v = BORDER_WEIGHT_OVERRIDE.load(Ordering::Acquire);
    if v < 0 { BorderWeight::Standard } else { BorderWeight::from_u8(v as u8) }
}
/// Read the override slot directly (None if not set).
#[inline]
pub fn border_weight_override_opt() -> Option<BorderWeight> {
    let v = BORDER_WEIGHT_OVERRIDE.load(Ordering::Acquire);
    if v < 0 { None } else { Some(BorderWeight::from_u8(v as u8)) }
}

pub fn set_corner_scale_override(mode: Option<CornerScale>) {
    CORNER_SCALE_OVERRIDE.store(mode.map(|m| m.as_u8() as i8).unwrap_or(-1), Ordering::Release);
}
#[inline]
pub fn corner_scale_override() -> CornerScale {
    let v = CORNER_SCALE_OVERRIDE.load(Ordering::Acquire);
    if v < 0 { CornerScale::Standard } else { CornerScale::from_u8(v as u8) }
}
#[inline]
pub fn corner_scale_override_opt() -> Option<CornerScale> {
    let v = CORNER_SCALE_OVERRIDE.load(Ordering::Acquire);
    if v < 0 { None } else { Some(CornerScale::from_u8(v as u8)) }
}

pub fn set_spacing_scale_override(mode: Option<SpacingScale>) {
    SPACING_SCALE_OVERRIDE.store(mode.map(|m| m.as_u8() as i8).unwrap_or(-1), Ordering::Release);
}
#[inline]
pub fn spacing_scale_override() -> SpacingScale {
    let v = SPACING_SCALE_OVERRIDE.load(Ordering::Acquire);
    if v < 0 { SpacingScale::Standard } else { SpacingScale::from_u8(v as u8) }
}
#[inline]
pub fn spacing_scale_override_opt() -> Option<SpacingScale> {
    let v = SPACING_SCALE_OVERRIDE.load(Ordering::Acquire);
    if v < 0 { None } else { Some(SpacingScale::from_u8(v as u8)) }
}

pub fn set_motion_speed_override(mode: Option<MotionSpeed>) {
    MOTION_SPEED_OVERRIDE.store(mode.map(|m| m.as_u8() as i8).unwrap_or(-1), Ordering::Release);
}
#[inline]
pub fn motion_speed_override() -> MotionSpeed {
    let v = MOTION_SPEED_OVERRIDE.load(Ordering::Acquire);
    if v < 0 { MotionSpeed::Standard } else { MotionSpeed::from_u8(v as u8) }
}
#[inline]
pub fn motion_speed_override_opt() -> Option<MotionSpeed> {
    let v = MOTION_SPEED_OVERRIDE.load(Ordering::Acquire);
    if v < 0 { None } else { Some(MotionSpeed::from_u8(v as u8)) }
}

// ─── Text measurement (for flex/painter layout) ──────────────────────────────
//
// Flex layout needs intrinsic text sizes: a title or count chip is an
// `Item::fixed(measured_width)` because a Taffy leaf has no measure function
// and would otherwise resolve to zero. Every widget doing that was building its
// own `FontId` and passing a throwaway colour to `layout_no_wrap`, which is
// (a) duplicated, (b) easy to get out of sync with the FontId actually painted,
// and (c) indistinguishable from real off-token drift to the design-system
// ratchet. Centralised here so callers pass a SIZE TOKEN and never construct a
// FontId or a placeholder colour.
//
// Sizes are rounded UP: solved rects can land a fraction narrower than the
// galley, which clips the final glyph.

/// Intrinsic size of `text` at `size` px in the proportional family.
/// Pass a token (`font_sm()`, `font_md()`, …), never a literal.
pub fn measure_prop(ui: &egui::Ui, text: &str, size: f32) -> egui::Vec2 {
    measure_with(ui, text, egui::FontId::proportional(size))
}

/// Intrinsic size of `text` at `size` px in the monospace family.
pub fn measure_mono(ui: &egui::Ui, text: &str, size: f32) -> egui::Vec2 {
    measure_with(ui, text, egui::FontId::monospace(size))
}

/// Intrinsic size of `text` in an explicit `FontId` — for callers that already
/// hold the exact font they will paint with (keeps measure and paint in sync).
pub fn measure_with(ui: &egui::Ui, text: &str, font: egui::FontId) -> egui::Vec2 {
    // The colour is irrelevant: this galley is measured, never painted.
    ui.fonts(|f| f.layout_no_wrap(text.to_string(), font, egui::Color32::PLACEHOLDER))
        .size()
        .ceil()
}

/// `FontId` at an explicit size and family — for the per-STYLE cases where the
/// family itself is a token (e.g. section headers are monospace on editorial
/// styles, proportional elsewhere). Keeps widgets from constructing `FontId`
/// directly, so the design-system ratchet stays meaningful.
#[inline]
pub fn font_at(size: f32, family: egui::FontFamily) -> egui::FontId {
    egui::FontId::new(size, family)
}

// ── Ladder-ordering invariants ───────────────────────────────────────────────
//
// AUDIT 2026-08. Every other design-system check asks whether a value is a
// token. None of them could see a ladder whose rungs are all legal tokens but
// no longer in order.
//
// `gap_xs_mid` was declared ~240 lines from the rest of the spacing ladder and
// was the only rung that did not apply `spacing_scale_override()`. At Standard
// (1.0x) it was byte-identical to correct, so nothing caught it — but at Tight
// (0.75x) the ladder read 3.0 / 6.0 / 6.0: the mid rung landed exactly on the
// rung above it and the two became indistinguishable.
//
// The bug is not the missing multiplier, it is that a ladder had no stated
// invariant. These tests give all three ladders one, checked at EVERY override
// setting rather than only the default that hides the problem.
#[cfg(test)]
mod ladder_ordering_tests {
    use super::*;

    /// Serialises the process-global override atomics these tests write.
    static LADDER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn assert_non_decreasing(label: &str, setting: &str, rungs: &[(&str, f32)]) {
        for w in rungs.windows(2) {
            let ((n0, v0), (n1, v1)) = (w[0], w[1]);
            assert!(
                v1 >= v0,
                "{label} ladder INVERTED at {setting}: {n0}={v0} but {n1}={v1} \
                 — a smaller rung must never outrank a larger one",
            );
        }
    }

    fn assert_strictly_increasing_at(label: &str, setting: &str, rungs: &[(&str, f32)]) {
        for w in rungs.windows(2) {
            let ((n0, v0), (n1, v1)) = (w[0], w[1]);
            assert!(
                v1 > v0,
                "{label} ladder COLLAPSED at {setting}: {n0}={v0} and {n1}={v1} \
                 are indistinguishable, so the two rungs render identically",
            );
        }
    }

    #[test]
    fn spacing_ladder_stays_ordered_at_every_scale() {
        let _g = LADDER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = spacing_scale_override();

        for (name, mode) in [
            ("Tight", SpacingScale::Tight),
            ("Standard", SpacingScale::Standard),
            ("Loose", SpacingScale::Loose),
        ] {
            set_spacing_scale_override(Some(mode));
            let rungs = [
                ("gap_2xs", gap_2xs()),
                ("gap_xs", gap_xs()),
                ("gap_xs_mid", gap_xs_mid()),
                ("gap_sm", gap_sm()),
                ("gap_md", gap_md()),
                ("gap_lg", gap_lg()),
                ("gap_xl", gap_xl()),
                ("gap_2xl", gap_2xl()),
                ("gap_3xl", gap_3xl()),
            ];
            // STRICT at every scale, not just Standard. The first draft
            // asserted strictness only at Standard and non-decreasing
            // elsewhere — which could not see the very defect that prompted
            // it, because a frozen rung COLLAPSES onto its neighbour
            // (gap_xs_mid 6.0 == gap_sm 6.0 at Tight) rather than inverting.
            // Reverting the gap_xs_mid fix left that draft green. Spacing has
            // no legitimate collapse case, so equality is always a bug here.
            assert_strictly_increasing_at("spacing", name, &rungs);
        }
        set_spacing_scale_override(Some(prev));
    }

    #[test]
    fn radius_ladder_stays_ordered_at_every_scale() {
        let _g = LADDER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = corner_scale_override();

        // Sharp is 0.0x — every rung collapses to zero BY DESIGN (the
        // square-corner aesthetic), so this ladder is only ever asserted
        // non-decreasing. Strict ordering is checked at Standard alone.
        for (name, mode) in [
            ("Sharp", CornerScale::Sharp),
            ("Standard", CornerScale::Standard),
        ] {
            set_corner_scale_override(Some(mode));
            let rungs = [
                ("radius_xs", radius_xs()),
                ("radius_sm", radius_sm()),
                ("radius_md", radius_md()),
                ("radius_lg", radius_lg()),
                ("radius_pill", radius_pill()),
            ];
            assert_non_decreasing("radius", name, &rungs);
            if matches!(mode, CornerScale::Standard) {
                assert_strictly_increasing_at("radius", name, &rungs);
            }
        }
        set_corner_scale_override(Some(prev));
    }

    #[test]
    fn stroke_ladder_stays_ordered_at_every_weight() {
        let _g = LADDER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = border_weight_override();

        for (name, mode) in [
            ("Hairline", BorderWeight::Hairline),
            ("Standard", BorderWeight::Standard),
        ] {
            set_border_weight_override(Some(mode));
            // NOTE THE ORDER: `stroke_medium` (0.8) sits BELOW `stroke_std`
            // (1.0), not above it. The first draft of this test assumed the
            // name ranked it above std and reported an inversion — the test
            // was wrong, the ladder was not. Recorded here because the naming
            // invites the same mistake: hair/thin/medium are the sub-1px tier
            // and std/bold/thick are the >=1px tier, so "medium" means medium
            // HAIRLINE, not medium weight overall.
            let rungs = [
                ("stroke_hair", stroke_hair()),
                ("stroke_thin", stroke_thin()),
                ("stroke_medium", stroke_medium()),
                ("stroke_std", stroke_std()),
                ("stroke_bold", stroke_bold()),
                ("stroke_thick", stroke_thick()),
                ("stroke_extra_thick", stroke_extra_thick()),
                ("stroke_rule", stroke_rule()),
            ];
            assert_strictly_increasing_at("stroke", name, &rungs);
        }
        set_border_weight_override(Some(prev));
    }
}

// ── M2.3 token-scope tests ───────────────────────────────────────────────────
#[cfg(test)]
mod m23_token_scope_tests {
    use super::*;

    /// Scoped tokens restore on drop — the enabler for two densities in one
    /// frame (the `Density` per-component knob CLAUDE.md promised but which
    /// only existed as a process-global `DensityMode`).
    #[test]
    fn token_scope_restores_previous() {
        let before = frame_tokens().gap_md;
        {
            let _s = TokenScope::with(|t| t.gap_md = 99.0);
            assert_eq!(frame_tokens().gap_md, 99.0, "scope must win inside");
        }
        assert_eq!(frame_tokens().gap_md, before, "previous tokens must be restored");
    }

    /// Mariner's "~10% tighter than Alto" as a SCOPED property.
    #[test]
    fn density_scope_scales_gaps_only_inside() {
        let base = frame_tokens();
        {
            let _s = TokenScope::density(0.9);
            let inner = frame_tokens();
            assert!((inner.gap_md - base.gap_md * 0.9).abs() < 0.001);
            assert!((inner.gap_sm - base.gap_sm * 0.9).abs() < 0.001);
            // font ladder untouched — density is spacing, not type
            assert_eq!(inner.font_sm, base.font_sm);
        }
        assert_eq!(frame_tokens().gap_md, base.gap_md);
    }

    /// Nesting composes multiplicatively and unwinds in order.
    #[test]
    fn token_scopes_nest() {
        let base = frame_tokens().gap_md;
        {
            let _a = TokenScope::density(0.5);
            {
                let _b = TokenScope::density(0.5);
                assert!((frame_tokens().gap_md - base * 0.25).abs() < 0.001);
            }
            assert!((frame_tokens().gap_md - base * 0.5).abs() < 0.001);
        }
        assert_eq!(frame_tokens().gap_md, base);
    }
}

#[cfg(test)]
mod corner_scale_tests {
    use super::*;

    /// The radius TOKENS honour the user's corner-scale preference; the raw
    /// `StyleSettings` fields do not.
    ///
    /// `radius_md()` is `frame_tokens().radius_md * corner_scale_override()`,
    /// while `current().r_md` is the unscaled preset value. Around thirty call
    /// sites read the raw field directly, so setting Corner Scale to Sharp
    /// flattened some surfaces and left others rounded — the preference
    /// appeared half-broken rather than off.
    ///
    /// This test exists so the distinction stays deliberate: if the scale ever
    /// stops applying, the reason ~30 sites were converted disappears with it.
    #[test]
    fn corner_scale_override_actually_scales_the_radius_tokens() {
        let base = radius_md();

        set_corner_scale_override(Some(CornerScale::Sharp));
        assert_eq!(radius_md(), 0.0, "Sharp must flatten every token-driven corner");

        set_corner_scale_override(Some(CornerScale::Round));
        let round = radius_md();

        set_corner_scale_override(Some(CornerScale::Standard));
        let standard = radius_md();

        assert!(round > standard, "Round ({round}) must exceed Standard ({standard})");

        // Restore so the shared thread-local does not leak into other tests.
        set_corner_scale_override(None);
        assert_eq!(radius_md(), base, "clearing the override must restore the preset value");
    }

    /// AUDIT 2026-08: the test above only ever checked `radius_md()`, which is
    /// exactly how `radius_pill()` shipped without the `corner_scale_override()`
    /// multiplier its four siblings all have. Under Sharp everything squared
    /// except pills, which stayed at 99 — so the plan's acceptance criterion
    /// "Sharp squares everything uniformly" (exit criterion P-9) was false, and
    /// nothing failed.
    ///
    /// Covering EVERY tier is the point: a per-tier accessor that forgets the
    /// scale is invisible unless the test enumerates them.
    #[test]
    fn sharp_squares_every_radius_tier_including_pill() {
        set_corner_scale_override(Some(CornerScale::Sharp));

        let tiers: [(&str, f32); 5] = [
            ("radius_xs",   radius_xs()),
            ("radius_sm",   radius_sm()),
            ("radius_md",   radius_md()),
            ("radius_lg",   radius_lg()),
            ("radius_pill", radius_pill()),
        ];

        set_corner_scale_override(None);

        let unsquared: Vec<&str> = tiers.iter()
            .filter(|(_, v)| *v != 0.0)
            .map(|(n, _)| *n)
            .collect();

        assert!(
            unsquared.is_empty(),
            "CornerScale::Sharp must flatten EVERY tier; these ignored it: {unsquared:?} \
             — an accessor missing its corner_scale_override() multiplier"
        );
    }
}

#[cfg(test)]
mod two_scale_invariant_tests {
    use super::*;

    /// Every `Size` rung must still fit its own glyphs at the smallest density.
    ///
    /// The app exposes two scales the user sets separately and that persist
    /// separately on the workspace:
    ///
    /// * `SpacingScale` — 0.75 / 1.0 / 1.25 — multiplies every `gap_*` rung.
    /// * `DensityMode`  — 0.85 / 1.0 / 1.15 — multiplies every `control_*` rung.
    ///
    /// Type scales with neither, so shrinking a control does not shrink the
    /// text inside it. That is the containment question, and `control_xs` at
    /// Compact has only 1.3 px of headroom over its own line box — thin enough
    /// that one nudge to the type scale would clip it, which is exactly how
    /// AT-148 happened.
    ///
    /// **Spacing scale is deliberately absent from the assertion.** Two earlier
    /// versions of this test included it, on the assumption that a control's
    /// height had to cover `text + 2 × vertical padding`. It does not:
    /// `Size::padding()` has no vertical consumer, `Size::height()` is an exact
    /// target and the text is centred inside it. Both versions "found" a defect
    /// — the second in 40 of 45 pairings, including Standard × Standard, in an
    /// app that plainly renders fine at its defaults. A model that indicts the
    /// default configuration is describing itself, not the app.
    #[test]
    fn every_size_rung_fits_its_glyphs_at_the_smallest_density() {
        let t = &DEFAULT_TOKEN_SNAPSHOT;
        // (name, control height, font) — mirrors Size::height() / font_size().
        let rungs: &[(&str, f32, f32)] = &[
            ("Xs", t.control_xs, t.font_xs),
            ("Sm", t.control_sm, t.font_sm),
            ("Md", t.control_md, t.font_sm),
            ("Lg", t.control_lg, t.font_md),
            ("Xl", t.control_xl, t.font_xl),
        ];

        let mut failures = Vec::new();
        for dm in [DensityMode::Compact, DensityMode::Standard, DensityMode::Spacious] {
            let d = dm.scale();
            for (name, h, font) in rungs {
                let box_h = h * d;
                let line = font * t.line_normal;
                if box_h + 0.01 < line {
                    failures.push(format!(
                        "{name} at density {dm:?}: box {box_h:.2} < line box {line:.2}"
                    ));
                }
            }
        }
        assert!(failures.is_empty(),
            "control heights clip their own text:
  {}", failures.join("
  "));
    }
}
