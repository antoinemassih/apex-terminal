//! Theme contract for ui_kit widgets.
//!
//! Widgets take `&dyn ComponentTheme` instead of `&chart_renderer::gpu::Theme`
//! so the kit could extract as a standalone crate. The trait exposes only
//! what widgets actually need — the 6-color palette + a few derived
//! getters. Add fields cautiously; every new field couples the kit to a
//! specific theme shape.

use egui::Color32;

// Re-export the trading-app's concrete `Theme` so ui_kit widgets that take
// `&Theme` (legacy signatures, mostly panel-body primitives) keep compiling.
// `Watchlist` / `SplitSection` / `pane_tabs_header_h` / `live_theme_count` /
// `get_theme` were removed from this bridge after the side_panel_shell and
// split_section_panel widgets moved out of ui_kit (into chart_renderer::ui::panels).
//
// When `ui_kit` extracts to a workspace crate, this single `pub(crate) use`
// is what disappears — widgets switch to `&dyn ComponentTheme` or take
// `PortableTheme` directly.
#[allow(unused_imports)]
pub(crate) use crate::chart_renderer::gpu::Theme;

pub trait ComponentTheme {
    // Core 6-color palette (matches the discipline established in item 4).
    fn accent(&self) -> Color32;
    fn bull(&self) -> Color32;
    fn bear(&self) -> Color32;
    fn text(&self) -> Color32;
    fn dim(&self) -> Color32;
    fn border(&self) -> Color32;
    fn border_variant(&self) -> Color32;
    fn warn(&self) -> Color32;

    // Surface tokens (background fills).
    fn bg(&self) -> Color32;
    fn surface(&self) -> Color32; // raised surface, e.g. toolbar_bg

    /// L2 surface — one layer up from `surface()`. Used for inputs,
    /// cards, sub-section bodies. Direction-aware: lighter on dark
    /// themes, darker on light themes. Default impl uses the standard
    /// `color_layer_up(t, 1)` 7%-step heuristic.
    fn surface_raised(&self) -> Color32 {
        // Heuristic: detect dark vs light by `bg()` luminance, shift
        // surface() toward text() by ~7%. Matches `style::color_layer_up`.
        let base = self.surface();
        let target = self.text();
        let bg = self.bg();
        let is_dark = (bg.r() as i16 + bg.g() as i16 + bg.b() as i16) < 384;
        let _ = target;
        let shift: i16 = if is_dark { 18 } else { -18 };
        let clamp = |c: i16| -> u8 { c.clamp(0, 255) as u8 };
        Color32::from_rgb(
            clamp(base.r() as i16 + shift),
            clamp(base.g() as i16 + shift),
            clamp(base.b() as i16 + shift),
        )
    }

    // Element state alpha overlays (Zed-derived). Applied OVER an element's
    // idle background to signal hover/active/selected/disabled without
    // switching colors. Pre-computed in the theme preset.
    fn element_hover(&self) -> Color32;
    fn element_active(&self) -> Color32;
    fn element_selected(&self) -> Color32;
    fn element_disabled(&self) -> Color32;
    fn ghost_hover(&self) -> Color32;
    fn ghost_active(&self) -> Color32;

    // Icon color ramp — decoupled from text hierarchy.
    fn icon(&self) -> Color32;
    fn icon_muted(&self) -> Color32;
    fn icon_disabled(&self) -> Color32;
    fn icon_accent(&self) -> Color32;

    /// Shadow tint. Themes pick a near-black for dark palettes and a soft
    /// gray for light palettes so drops don't read as a hard black smudge
    /// on Bauhaus / Peach / Ivory / Newsprint. The default impl returns
    /// `Color32::BLACK` as a safe fallback; concrete themes override.
    fn shadow_color(&self) -> Color32 { Color32::BLACK }

    /// Semantic "success" colour — green-ish. Default impl delegates to
    /// `bull()` so trading themes keep their existing palette; non-trading
    /// themes (e.g. a doc app) override to provide a non-bull semantic.
    fn success(&self) -> Color32 { self.bull() }

    /// Semantic "danger" colour — red-ish. Default impl delegates to
    /// `bear()`. Used by form validation, error indicators, destructive
    /// actions — anywhere the meaning is "warning/wrong/destroy" rather
    /// than "the market went down".
    fn danger(&self) -> Color32 { self.bear() }

    // ── Semantic surface tokens (Phase 2c — portability) ─────────────────────
    // These previously lived in chart_renderer::ui::style as free functions
    // taking `&Theme` (concrete). Moved onto the trait as defaults so widgets
    // can compute them from `&dyn ComponentTheme` without a chart-app dep.

    /// Border colour for a framed surface. Defaults to `border()`. Override
    /// only if a theme wants a separate "raised-surface" border tone.
    fn surface_border(&self) -> Color32 { self.border() }

    /// Header band background — one elevation step over `bg()`.
    fn header_surface(&self) -> Color32 { self.bg().gamma_multiply(0.95) }

    /// Section-header band background — two elevation steps over `bg()`.
    fn section_header_surface(&self) -> Color32 { self.bg().gamma_multiply(0.88) }

    /// Panel body background — three elevation steps over `bg()`.
    fn panel_surface(&self) -> Color32 { self.bg().gamma_multiply(0.85) }

    /// Header divider/border colour. 38α over `text()`.
    fn header_border(&self) -> Color32 {
        let t = self.text();
        Color32::from_rgba_unmultiplied(t.r(), t.g(), t.b(), 38)
    }

    /// Theme-aware card shadow. Default: `[0, 2]` offset, blur 4, spread 0,
    /// alpha 60 over `shadow_color()`. Mirrors `style::shadow_card_themed`.
    /// Themes / apps that need a different shadow override this method.
    fn shadow_card(&self) -> egui::epaint::Shadow {
        let s = self.shadow_color();
        egui::epaint::Shadow {
            offset: [0, 2],
            blur: 4,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(s.r(), s.g(), s.b(), 60),
        }
    }

    /// Layered surface lift over `surface()`. Used for cards / sub-sections /
    /// active-tab bodies that nest visibly above the panel base. `n` is the
    /// lift step (0..=5); 7% per step (≈18/255). Direction-aware: lighter on
    /// dark themes, darker on light themes. Mirrors the chart-app's
    /// `style::color_layer_up(t, n)` helper so widgets get a portable path
    /// to the same effect.
    fn color_layer_up(&self, n: u8) -> Color32 {
        let base = self.surface();
        let bg = self.bg();
        let is_dark = (bg.r() as i16 + bg.g() as i16 + bg.b() as i16) < 384;
        let steps = n.min(5) as i16;
        let shift: i16 = if is_dark { 18 * steps } else { -18 * steps };
        let clamp = |c: i16| -> u8 { c.clamp(0, 255) as u8 };
        Color32::from_rgb(
            clamp(base.r() as i16 + shift),
            clamp(base.g() as i16 + shift),
            clamp(base.b() as i16 + shift),
        )
    }
}

// `impl ComponentTheme for crate::chart_renderer::gpu::Theme` is the
// chart-app's bridge to this trait and lives in `chart_renderer::theme_impl`
// (correct dependency direction: chart_renderer -> ui_kit, not the reverse).
// Re-exported here for back-compat with widgets that import
// `super::theme::active_theme`.
pub use crate::chart_renderer::theme_impl::active_theme;

// ── PortableTheme — the standalone Theme for non-trading apps ────────────────
//
// A plain struct with the semantic colour tokens `ComponentTheme` exposes.
// No bull/bear (those default-impl to `accent()`). No trading-specific
// fields. A doc app, settings dialog, or any embedder of `ui_kit` can:
//
//     let theme = PortableTheme::dark();           // or `::light()` / `::default()`
//     // pass to widgets that take `&dyn ComponentTheme`:
//     MyButton::new("Save").show(ui, &theme);
//
// The trading app keeps `chart_renderer::gpu::Theme` with bull/bear, etc.;
// both types satisfy `ComponentTheme` so widgets work with either.
//
// Goal of this type: prove `ComponentTheme` is implementable without
// reaching into `chart_renderer` for any field. (When `ui_kit` extracts to
// a workspace crate, this is the only `Theme` it ships.)

#[derive(Clone, Debug)]
pub struct PortableTheme {
    pub accent: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub border: Color32,
    pub border_variant: Color32,
    pub warn: Color32,
    pub bg: Color32,
    pub surface: Color32,

    // Element-state alpha overlays.
    pub element_hover: Color32,
    pub element_active: Color32,
    pub element_selected: Color32,
    pub element_disabled: Color32,
    pub ghost_hover: Color32,
    pub ghost_active: Color32,

    // Icon ramp.
    pub icon: Color32,
    pub icon_muted: Color32,
    pub icon_disabled: Color32,
    pub icon_accent: Color32,

    pub shadow_color: Color32,
}

impl PortableTheme {
    /// Reasonable dark-theme defaults — neutral grays, blue accent, soft
    /// black shadow. Good enough to bring up a new app's UI and iterate.
    pub fn dark() -> Self {
        Self {
            accent:           Color32::from_rgb( 70, 130, 220),
            text:             Color32::from_rgb(220, 220, 222),
            dim:              Color32::from_rgb(140, 140, 145),
            border:           Color32::from_rgb( 56,  56,  60),
            border_variant:   Color32::from_rgb( 76,  76,  80),
            warn:             Color32::from_rgb(220, 160,  40),
            bg:               Color32::from_rgb( 22,  22,  26),
            surface:          Color32::from_rgb( 30,  30,  34),
            element_hover:    Color32::from_rgba_unmultiplied(255, 255, 255, 14),
            element_active:   Color32::from_rgba_unmultiplied(255, 255, 255, 28),
            element_selected: Color32::from_rgba_unmultiplied( 70, 130, 220, 40),
            element_disabled: Color32::from_rgba_unmultiplied(255, 255, 255,  8),
            ghost_hover:      Color32::from_rgba_unmultiplied(255, 255, 255, 10),
            ghost_active:     Color32::from_rgba_unmultiplied(255, 255, 255, 22),
            icon:             Color32::from_rgb(200, 200, 204),
            icon_muted:       Color32::from_rgb(140, 140, 145),
            icon_disabled:    Color32::from_rgb( 88,  88,  92),
            icon_accent:      Color32::from_rgb( 70, 130, 220),
            shadow_color:     Color32::BLACK,
        }
    }

    /// Light-theme defaults. Use as a starting point; tune for brand.
    pub fn light() -> Self {
        Self {
            accent:           Color32::from_rgb( 30,  90, 180),
            text:             Color32::from_rgb( 28,  28,  32),
            dim:              Color32::from_rgb(110, 110, 116),
            border:           Color32::from_rgb(216, 216, 220),
            border_variant:   Color32::from_rgb(200, 200, 204),
            warn:             Color32::from_rgb(192, 120,   0),
            bg:               Color32::from_rgb(250, 250, 252),
            surface:          Color32::from_rgb(240, 240, 244),
            element_hover:    Color32::from_rgba_unmultiplied(  0,   0,   0, 14),
            element_active:   Color32::from_rgba_unmultiplied(  0,   0,   0, 28),
            element_selected: Color32::from_rgba_unmultiplied( 30,  90, 180, 40),
            element_disabled: Color32::from_rgba_unmultiplied(  0,   0,   0,  8),
            ghost_hover:      Color32::from_rgba_unmultiplied(  0,   0,   0, 10),
            ghost_active:     Color32::from_rgba_unmultiplied(  0,   0,   0, 22),
            icon:             Color32::from_rgb( 60,  60,  64),
            icon_muted:       Color32::from_rgb(120, 120, 124),
            icon_disabled:    Color32::from_rgb(180, 180, 184),
            icon_accent:      Color32::from_rgb( 30,  90, 180),
            shadow_color:     Color32::from_rgb(120, 120, 124),
        }
    }
}

impl Default for PortableTheme {
    fn default() -> Self { Self::dark() }
}

impl ComponentTheme for PortableTheme {
    fn accent(&self) -> Color32 { self.accent }
    // bull/bear use the trait defaults (delegate to accent via success/danger
    // path — actually default to accent directly for portable themes).
    fn bull(&self) -> Color32 { self.accent }
    fn bear(&self) -> Color32 { self.warn }
    fn text(&self) -> Color32 { self.text }
    fn dim(&self) -> Color32 { self.dim }
    fn border(&self) -> Color32 { self.border }
    fn border_variant(&self) -> Color32 { self.border_variant }
    fn warn(&self) -> Color32 { self.warn }
    fn bg(&self) -> Color32 { self.bg }
    fn surface(&self) -> Color32 { self.surface }
    fn element_hover(&self) -> Color32 { self.element_hover }
    fn element_active(&self) -> Color32 { self.element_active }
    fn element_selected(&self) -> Color32 { self.element_selected }
    fn element_disabled(&self) -> Color32 { self.element_disabled }
    fn ghost_hover(&self) -> Color32 { self.ghost_hover }
    fn ghost_active(&self) -> Color32 { self.ghost_active }
    fn icon(&self) -> Color32 { self.icon }
    fn icon_muted(&self) -> Color32 { self.icon_muted }
    fn icon_disabled(&self) -> Color32 { self.icon_disabled }
    fn icon_accent(&self) -> Color32 { self.icon_accent }
    fn shadow_color(&self) -> Color32 { self.shadow_color }
}

/// Read the active theme index stashed in egui memory by the render loop.
/// Falls back to 0 (Midnight) if nothing was stashed. Portable — no
/// chart-app dependency.
pub fn active_theme_idx(ctx: &egui::Context) -> usize {
    ctx.data(|d| d.get_temp::<usize>(egui::Id::new("apex_active_theme_idx")))
       .unwrap_or(0)
}

// ── Ambient theme (UI extraction, item 2) ────────────────────────────────────
//
// Hosts (the chart app, or any other app embedding ui_kit) stash the active
// `Theme` in egui memory once per frame. ui_kit widgets that have no
// theme arg (e.g. `Widget` impls returning `Response`) read it back via
// `get_ambient_theme(ctx)` instead of reaching into the chart-app's live
// theme registry. This severs `active_theme()`'s hard dependency on
// `chart_renderer::gpu::get_theme(idx)` — the registry is now a fallback.

const AMBIENT_KEY: &str = "apex_ambient_theme";

/// Stash the current `Theme` in egui memory so ui_kit's parameter-less
/// widgets can find it. Call this once per frame from the host app's
/// render loop, before any UI is built. Cheap — one cloned `Theme`
/// insertion per frame.
pub fn set_ambient_theme(ctx: &egui::Context, theme: Theme) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(AMBIENT_KEY), theme));
}

/// Read the ambient `Theme` set by [`set_ambient_theme`]. Returns `None`
/// if the host hasn't set one this frame — callers should fall back to
/// whatever's appropriate (the chart-app version of `active_theme()`
/// falls back to the live theme registry by index).
pub fn get_ambient_theme(ctx: &egui::Context) -> Option<Theme> {
    ctx.data(|d| d.get_temp::<Theme>(egui::Id::new(AMBIENT_KEY)))
}

impl<T: ComponentTheme + ?Sized> ComponentTheme for &T {
    fn accent(&self) -> Color32 { (**self).accent() }
    fn bull(&self) -> Color32 { (**self).bull() }
    fn bear(&self) -> Color32 { (**self).bear() }
    fn text(&self) -> Color32 { (**self).text() }
    fn dim(&self) -> Color32 { (**self).dim() }
    fn border(&self) -> Color32 { (**self).border() }
    fn border_variant(&self) -> Color32 { (**self).border_variant() }
    fn warn(&self) -> Color32 { (**self).warn() }
    fn bg(&self) -> Color32 { (**self).bg() }
    fn surface(&self) -> Color32 { (**self).surface() }
    fn element_hover(&self) -> Color32 { (**self).element_hover() }
    fn element_active(&self) -> Color32 { (**self).element_active() }
    fn element_selected(&self) -> Color32 { (**self).element_selected() }
    fn element_disabled(&self) -> Color32 { (**self).element_disabled() }
    fn ghost_hover(&self) -> Color32 { (**self).ghost_hover() }
    fn ghost_active(&self) -> Color32 { (**self).ghost_active() }
    fn icon(&self) -> Color32 { (**self).icon() }
    fn icon_muted(&self) -> Color32 { (**self).icon_muted() }
    fn icon_disabled(&self) -> Color32 { (**self).icon_disabled() }
    fn icon_accent(&self) -> Color32 { (**self).icon_accent() }
    fn shadow_color(&self) -> Color32 { (**self).shadow_color() }
}
