//! Theme contract for ui_kit widgets.
//!
//! Widgets take `&dyn ComponentTheme` instead of `&chart_renderer::gpu::Theme`
//! so the kit could extract as a standalone crate. The trait exposes only
//! what widgets actually need — the 6-color palette + a few derived
//! getters. Add fields cautiously; every new field couples the kit to a
//! specific theme shape.

use egui::Color32;

// The Theme bridge has been removed — all ui_kit widgets are now generic-T
// (`T: ComponentTheme = PortableTheme`) or take `&dyn ComponentTheme`.
// The trading-app `Theme` flows through inference at call sites; no ui_kit
// code references the concrete type directly. This file is now free of
// `crate::chart_renderer::gpu::Theme`.

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

    /// Header band background — most-lifted panel surface. Additive luminance
    /// shift (raised on dark, inset on light) so it survives near-black bg —
    /// see `crate::ui_kit::style::elevate`.
    fn header_surface(&self) -> Color32 {
        crate::ui_kit::style::elevate(self.bg(), crate::ui_kit::style::ELEVATE_PANEL_HEADER)
    }

    /// Section-header band background — mid elevation step.
    fn section_header_surface(&self) -> Color32 {
        crate::ui_kit::style::elevate(self.bg(), crate::ui_kit::style::ELEVATE_PANEL_SECTION)
    }

    /// Panel body background — the side-panel card that lifts off the canvas.
    fn panel_surface(&self) -> Color32 {
        crate::ui_kit::style::elevate(self.bg(), crate::ui_kit::style::ELEVATE_PANEL_BODY)
    }

    /// Header divider/border colour. 38α over `text()`.
    fn header_border(&self) -> Color32 {
        let t = self.text();
        Color32::from_rgba_unmultiplied(t.r(), t.g(), t.b(), 38)
    }

    /// Compose `shadow_color()` with an explicit alpha. Mirrors
    /// `style::shadow_color_alpha(t, alpha)` so widgets get a portable
    /// path.
    fn shadow_color_alpha(&self, alpha: u8) -> Color32 {
        let s = self.shadow_color();
        Color32::from_rgba_unmultiplied(s.r(), s.g(), s.b(), alpha)
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
    // ── Dimension tokens (P4.2) ──────────────────────────────────────────────
    // Default impls delegate to the per-frame TokenSnapshot via the token
    // helper functions in `ui_kit::style`. Themes / hosts don't need to
    // implement these — they pick up the active StyleSystem automatically.
    // When ui_kit extracts to its own crate, these become the single trait-
    // boundary path for both colours and dimensions.

    fn font_xs(&self) -> f32   { crate::ui_kit::style::font_xs() }
    fn font_sm(&self) -> f32   { crate::ui_kit::style::font_sm() }
    fn font_md(&self) -> f32   { crate::ui_kit::style::font_md() }
    fn font_lg(&self) -> f32   { crate::ui_kit::style::font_lg() }

    fn gap_xs(&self) -> f32    { crate::ui_kit::style::gap_xs() }
    fn gap_sm(&self) -> f32    { crate::ui_kit::style::gap_sm() }
    fn gap_md(&self) -> f32    { crate::ui_kit::style::gap_md() }
    fn gap_lg(&self) -> f32    { crate::ui_kit::style::gap_lg() }

    fn radius_xs(&self) -> f32 { crate::ui_kit::style::radius_xs() }
    fn radius_sm(&self) -> f32 { crate::ui_kit::style::radius_sm() }
    fn radius_md(&self) -> f32 { crate::ui_kit::style::radius_md() }
    fn radius_lg(&self) -> f32 { crate::ui_kit::style::radius_lg() }

    fn stroke_thin(&self) -> f32 { crate::ui_kit::style::stroke_thin() }
    fn stroke_std(&self)  -> f32 { crate::ui_kit::style::stroke_std() }
    fn stroke_bold(&self) -> f32 { crate::ui_kit::style::stroke_bold() }

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
// P5b extraction Step 3: ui_kit now OWNS its own `active_theme()` so it
// no longer re-exports from `chart_renderer::theme_impl`. The portable
// implementation reads the ambient PortableTheme stashed by the host once
// per frame (chart-app does this in `gpu::setup_theme` via the
// `theme_impl::theme_to_portable` bridge). Falls back to the default
// PortableTheme if no host stashed one — which is the right behaviour
// when ui_kit ships as a standalone crate with no chart_renderer to
// resolve a live palette index.
//
// The ~40 ui_kit widget callers (`super::theme::active_theme(ui.ctx())`)
// continue to compile because PortableTheme implements `ComponentTheme`
// and every widget signature takes `&dyn ComponentTheme` or generic
// `T: ComponentTheme` (no concrete `gpu::Theme` reach-ins).
pub fn active_theme(ctx: &egui::Context) -> PortableTheme {
    get_ambient_theme::<PortableTheme>(ctx).unwrap_or_default()
}

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
    pub bull: Color32,
    pub bear: Color32,
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
            bull:             Color32::from_rgb( 52, 168, 110),
            bear:             Color32::from_rgb(220,  80,  90),
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
            bull:             Color32::from_rgb( 22, 130,  72),
            bear:             Color32::from_rgb(190,  50,  60),
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
    // P5b: bull/bear now have dedicated fields so the ambient-stashed
    // PortableTheme can carry the chart-app's actual bull/bear values
    // (previously these collapsed to accent/warn, which made widgets like
    // RiskRewardBar and MetricRow show wrong colors under extraction).
    fn bull(&self) -> Color32 { self.bull }
    fn bear(&self) -> Color32 { self.bear }
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

/// Stash the current theme in egui memory so ui_kit's parameter-less
/// widgets can find it. Generic over any `ComponentTheme` implementor —
/// the host pushes its concrete theme type once per frame. Cheap (one
/// cloned theme insertion per frame).
pub fn set_ambient_theme<T: ComponentTheme + Clone + Send + Sync + 'static>(
    ctx: &egui::Context,
    theme: T,
) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(AMBIENT_KEY), theme));
}

/// Read the ambient theme set by [`set_ambient_theme`]. Caller specifies
/// the expected concrete type via turbofish: `get_ambient_theme::<MyTheme>(ctx)`.
/// Returns `None` if the host hasn't set one this frame OR if the stashed
/// type doesn't match `T`.
pub fn get_ambient_theme<T: ComponentTheme + Clone + Send + Sync + 'static>(
    ctx: &egui::Context,
) -> Option<T> {
    ctx.data(|d| d.get_temp::<T>(egui::Id::new(AMBIENT_KEY)))
}

// ── Ambient RecipeSet (Stream S5 — ADOPTION) ─────────────────────────────────
//
// Hosts stash an `Arc<RecipeSet>` once per frame (next to `set_ambient_theme`)
// so widgets that call `StyleCtx::from_theme` get the active recipe overrides
// automatically. The `Arc` wrapper means clone is O(1) — every `StyleCtx` built
// from the ambient set pays only a refcount bump, not a full map copy.
//
// When no host has set one (unit tests, standalone ui_kit embedders) the
// get function returns an empty static set — zero-cost, zero visual change.

const AMBIENT_RECIPES_KEY: &str = "apex_ambient_recipes";

/// Stash the active [`RecipeSet`] in egui memory so [`StyleCtx::from_theme`]
/// picks it up automatically. Call once per frame next to [`set_ambient_theme`].
///
/// The set is wrapped in `Arc` so the clone stored in egui memory is O(1) and
/// subsequent reads via [`get_ambient_recipes`] are also O(1) refcount bumps.
///
/// When no recipes are loaded (the common case during development) pass
/// `Arc::new(RecipeSet::new())` — all widgets fall through to their built-in
/// defaults and nothing changes visually.
pub fn set_ambient_recipes(
    ctx: &egui::Context,
    recipes: std::sync::Arc<crate::design_system::recipes::RecipeSet>,
) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(AMBIENT_RECIPES_KEY), recipes));
}

/// Read the ambient [`RecipeSet`] set by [`set_ambient_recipes`].
///
/// Returns an `Arc` so the caller can hold a cheap reference without
/// re-entering egui's data lock on every widget call. Falls back to an empty
/// static set when no host has set one this frame.
pub fn get_ambient_recipes(
    ctx: &egui::Context,
) -> std::sync::Arc<crate::design_system::recipes::RecipeSet> {
    ctx.data(|d| {
        d.get_temp::<std::sync::Arc<crate::design_system::recipes::RecipeSet>>(
            egui::Id::new(AMBIENT_RECIPES_KEY)
        )
    })
    .unwrap_or_else(empty_recipe_arc)
}

/// Returns the static empty-`RecipeSet` `Arc`. Returned by [`get_ambient_recipes`]
/// when no host has called [`set_ambient_recipes`]. Also used directly by
/// `ctx::StyleCtx::from_theme` and the host wiring in `gpu.rs`. The `Arc` is
/// created once (via `OnceLock`) and cloned on subsequent calls — no heap
/// allocation per widget, no egui-memory lock.
pub fn empty_recipe_arc() -> std::sync::Arc<crate::design_system::recipes::RecipeSet> {
    use std::sync::{Arc, OnceLock};
    static EMPTY: OnceLock<Arc<crate::design_system::recipes::RecipeSet>> = OnceLock::new();
    Arc::clone(EMPTY.get_or_init(|| Arc::new(crate::design_system::recipes::RecipeSet::new())))
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
