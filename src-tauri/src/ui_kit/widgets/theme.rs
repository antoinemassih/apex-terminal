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

    /// Whether section headers render monospace (per-style: editorial styles like
    /// Mariner/Alto/Relay opt in). Default proportional so ui_kit widgets pick
    /// the right family without hardcoding a global choice. The chart Theme
    /// overrides this from the active StyleSystem's `section_header_mono` token.
    fn section_header_mono(&self) -> bool { false }

    /// Whether cards should FLOAT (rounded + soft drop shadow) vs sit flush
    /// (flat, minimal). Per-style: tiled styles (Aperture/Cadence/Glass — those
    /// with a region gap) float; editorial styles (Meridien/Mariner) stay flat.
    /// The chart Theme overrides from `region_gap > 0`. Default true.
    fn cards_float(&self) -> bool { true }

    // ── Per-style ROW treatment ─────────────────────────────────────────────
    // Mirror the chart-layer StyleSystem `wl_row_*` tokens through the trait
    // boundary (same pattern as `cards_float`) so the generic `PanelListRow`
    // renders rows the SAME way the RowShell-based `WatchlistRow` does on each
    // style — pill-inset capsules on tiled styles (Aperture/Glass), a per-row
    // hairline on editorial styles (Alto/Mariner/Relay/Lucid). Defaults are the
    // flush/no-inset values so a bare PortableTheme is unchanged; the chart
    // Theme overrides all three from the active StyleSystem.

    /// Horizontal inset (px) applied to a row's interactive background so it
    /// reads as an inset capsule rather than a full-bleed band. 0 = flush.
    fn row_side_margin(&self) -> f32 { 0.0 }

    /// Corner radius (px) of a row's interactive background. 0 = sharp (falls
    /// back to the caller's recipe radius).
    fn row_corner_radius(&self) -> u8 { 0 }

    /// Alpha (over `surface_border()`) of a per-row bottom hairline drawn under
    /// EVERY row on editorial "ledger" styles. 0 = no per-style divider (the
    /// opt-in `.divided(true)` path still works independently).
    fn row_divider_alpha(&self) -> u8 { 0 }

    /// Standard list-row height (px) for this style — the SAME value the
    /// watchlist/option-chain rows use, so a generic `PanelListRow` is as tall
    /// and readable as those (not scrunched). Default 26; the chart Theme
    /// overrides from the active StyleSystem's density-scaled `row_height_px`.
    fn row_height(&self) -> f32 { 26.0 }

    /// Header divider/border colour. 38α over `text()`.
    fn header_border(&self) -> Color32 {
        let t = self.text();
        crate::ui_kit::style::color_alpha(t, 38)
    }

    /// Compose `shadow_color()` with an explicit alpha. Mirrors
    /// `style::shadow_color_alpha(t, alpha)` so widgets get a portable
    /// path.
    fn shadow_color_alpha(&self, alpha: u8) -> Color32 {
        let s = self.shadow_color();
        crate::ui_kit::style::color_alpha(s, alpha)
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
            color: crate::ui_kit::style::color_alpha(s, 60),
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

    // M0.4: per-style behavioural flags carried as REAL fields. Previously
    // these fell back to the `ComponentTheme` trait defaults (false / true)
    // while the full `Theme`'s impl read `StyleSettings::current()` live —
    // so `PanelSection` headers and `PanelCard` floating differed depending
    // on WHICH ambient object type a widget happened to resolve. One theme,
    // one answer.
    pub section_header_mono: bool,
    pub cards_float: bool,
}

impl PortableTheme {
    /// Snapshot any [`ComponentTheme`] into an OWNED, fully-featured theme.
    ///
    /// For deferred-render builders (`ContextMenu` and friends) that are
    /// constructed with a theme and shown later, where holding a
    /// `&dyn ComponentTheme` across the gap is awkward.
    ///
    /// The alternative such builders reached for was a hand-rolled struct of
    /// the four or five colours they happened to need — `MenuTheme` was one.
    /// That works until the widget wants anything else: the projection is
    /// lossy, so everything downstream of it is cut off from the palette AND
    /// from the recipe layer, and cannot resolve a key even in principle.
    /// Snapshotting the WHOLE theme costs a struct copy and keeps the widget
    /// inside the design system.
    pub fn snapshot(t: &dyn ComponentTheme) -> Self {
        Self {
            accent: t.accent(),
            bull: t.bull(),
            bear: t.bear(),
            text: t.text(),
            dim: t.dim(),
            border: t.border(),
            border_variant: t.border_variant(),
            warn: t.warn(),
            bg: t.bg(),
            surface: t.surface(),
            element_hover: t.element_hover(),
            element_active: t.element_active(),
            element_selected: t.element_selected(),
            element_disabled: t.element_disabled(),
            ghost_hover: t.ghost_hover(),
            ghost_active: t.ghost_active(),
            icon: t.icon(),
            icon_muted: t.icon_muted(),
            icon_disabled: t.icon_disabled(),
            icon_accent: t.icon_accent(),
            shadow_color: t.shadow_color(),
            section_header_mono: t.section_header_mono(),
            cards_float: t.cards_float(),
        }
    }

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
            section_header_mono: false,
            cards_float:      true,
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
            section_header_mono: false,
            cards_float:      true,
        }
    }
}

impl Default for PortableTheme {
    fn default() -> Self { Self::dark() }
}

impl ComponentTheme for PortableTheme {
    fn accent(&self) -> Color32 { self.accent }
    // M0.4: carry the per-style flags instead of inheriting trait defaults.
    fn section_header_mono(&self) -> bool { self.section_header_mono }
    fn cards_float(&self) -> bool { self.cards_float }
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
    // ── Per-style row treatment, sourced from the per-frame TokenSnapshot ────
    // `PortableTheme` is what the render loop ambient-stashes, so the ~40
    // parameter-less ui_kit widgets (Button, Checkbox, Switch, Slider, Tag,
    // Toast, Progress, Separator, …) resolve their theme through THIS impl.
    // It previously inherited the trait defaults for every per-style method,
    // which made those widgets structurally blind to `style_idx` — they always
    // rendered as if the style were flush/unrounded. The chart layer already
    // pushes the `wl_row_*` treatment into `TokenSnapshot` each frame
    // (ui_kit/style.rs), so read it here. Hosts that never push tokens get the
    // snapshot defaults (0/0/0), i.e. the previous behaviour — still portable.
    fn row_side_margin(&self) -> f32 { crate::ui_kit::style::frame_tokens().wl_row_side_margin }
    fn row_corner_radius(&self) -> u8 { crate::ui_kit::style::frame_tokens().wl_row_corner_radius }
    fn row_divider_alpha(&self) -> u8 { crate::ui_kit::style::frame_tokens().wl_row_divider_alpha }
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

// ── M2.3: SCOPED ambient theme (push/pop) ────────────────────────────────────
//
// The stash above is a single global key, so the LAST writer wins for the whole
// frame. That is why (a) an inactive pane rendered its chrome with the ACTIVE
// pane's palette, (b) Theme Studio had to hand-roll set-preview/restore around
// its catalogue, and (c) two densities could never coexist in one frame.
//
// `ThemeScope` makes the stash a STACK discipline: on construction it swaps in
// the scoped theme and remembers the previous value; on drop it restores it —
// exactly the CSS-subtree semantics ui_kit widgets already assume when they
// call `active_theme(ctx)`. RAII means an early return or a `?` inside the
// scope cannot leak the override.
//
// ```ignore
// let _scope = ThemeScope::push(ctx, pane_theme);   // this pane's palette
// render_pane_chrome(ui);                            // ui_kit sees pane_theme
// // dropped here -> previous ambient restored
// ```
#[must_use = "the scope restores the previous theme on drop; bind it to a variable"]
pub struct ThemeScope<'a> {
    ctx: &'a egui::Context,
    prev: Option<PortableTheme>,
}

impl<'a> ThemeScope<'a> {
    /// Push `theme` as the ambient theme for the lifetime of the returned
    /// guard. Any `ComponentTheme` works; it is converted to the portable
    /// shape that parameter-less ui_kit widgets read.
    pub fn push(ctx: &'a egui::Context, theme: PortableTheme) -> Self {
        let prev = get_ambient_theme::<PortableTheme>(ctx);
        set_ambient_theme(ctx, theme);
        Self { ctx, prev }
    }
}

impl Drop for ThemeScope<'_> {
    fn drop(&mut self) {
        match self.prev.take() {
            Some(prev) => set_ambient_theme(self.ctx, prev),
            // Nothing was stashed before us — clear rather than leave ours in
            // place, so a scope never outlives its block.
            None => self.ctx.data_mut(|d| {
                d.remove::<PortableTheme>(egui::Id::new(AMBIENT_KEY));
            }),
        }
    }
}

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
    // ── Defaulted methods MUST be forwarded too ─────────────────────────────
    // Without these, any generic instantiation where `T = &Theme` silently
    // reverts the app's per-style overrides to the ui_kit trait DEFAULTS —
    // with no compile-time signal (the blanket impl just inherits the
    // defaults). That made per-style row/card/header treatment vanish for
    // double-referenced themes. Forward every overridable method.
    fn section_header_mono(&self) -> bool { (**self).section_header_mono() }
    fn cards_float(&self) -> bool { (**self).cards_float() }
    fn row_side_margin(&self) -> f32 { (**self).row_side_margin() }
    fn row_corner_radius(&self) -> u8 { (**self).row_corner_radius() }
    fn row_divider_alpha(&self) -> u8 { (**self).row_divider_alpha() }
    fn row_height(&self) -> f32 { (**self).row_height() }
    fn surface_raised(&self) -> Color32 { (**self).surface_raised() }
    fn shadow_card(&self) -> egui::Shadow { (**self).shadow_card() }
    fn header_surface(&self) -> Color32 { (**self).header_surface() }
    fn section_header_surface(&self) -> Color32 { (**self).section_header_surface() }
    fn panel_surface(&self) -> Color32 { (**self).panel_surface() }
    fn header_border(&self) -> Color32 { (**self).header_border() }
    fn surface_border(&self) -> Color32 { (**self).surface_border() }
    fn color_layer_up(&self, n: u8) -> Color32 { (**self).color_layer_up(n) }
    fn shadow_color_alpha(&self, a: u8) -> Color32 { (**self).shadow_color_alpha(a) }
}

// ── M2.3 scope-guard tests ───────────────────────────────────────────────────
#[cfg(test)]
mod m23_scope_tests {
    use super::*;

    /// The core CSS-subtree guarantee: inside a scope widgets see the scoped
    /// palette; after it, the previous one is back. Before M2.3 the stash was
    /// a single global key, so the last writer won for the whole frame — which
    /// is why an inactive pane rendered its chrome in the ACTIVE pane's colours.
    #[test]
    fn theme_scope_restores_previous() {
        let ctx = egui::Context::default();
        let mut outer = PortableTheme::dark();
        outer.accent = Color32::from_rgb(1, 2, 3);
        let mut inner = PortableTheme::light();
        inner.accent = Color32::from_rgb(9, 9, 9);

        set_ambient_theme(&ctx, outer.clone());
        assert_eq!(active_theme(&ctx).accent, outer.accent);
        {
            let _scope = ThemeScope::push(&ctx, inner.clone());
            assert_eq!(active_theme(&ctx).accent, inner.accent, "scope must win inside");
        }
        assert_eq!(active_theme(&ctx).accent, outer.accent, "previous must be restored");
    }

    /// Nesting must unwind in order (pane inside preview inside app).
    #[test]
    fn theme_scopes_nest() {
        let ctx = egui::Context::default();
        let mk = |r: u8| { let mut t = PortableTheme::dark(); t.accent = Color32::from_rgb(r, 0, 0); t };
        set_ambient_theme(&ctx, mk(1));
        {
            let _a = ThemeScope::push(&ctx, mk(2));
            {
                let _b = ThemeScope::push(&ctx, mk(3));
                assert_eq!(active_theme(&ctx).accent.r(), 3);
            }
            assert_eq!(active_theme(&ctx).accent.r(), 2);
        }
        assert_eq!(active_theme(&ctx).accent.r(), 1);
    }

    /// A scope pushed with nothing underneath must not leak past its block.
    #[test]
    fn theme_scope_clears_when_nothing_was_stashed() {
        let ctx = egui::Context::default();
        {
            let _scope = ThemeScope::push(&ctx, PortableTheme::light());
            assert!(get_ambient_theme::<PortableTheme>(&ctx).is_some());
        }
        assert!(get_ambient_theme::<PortableTheme>(&ctx).is_none(), "must not outlive its block");
    }
}

/// Resolve a control's chrome (radius / fill / border) through the ambient
/// [`RecipeSet`], falling back to the values the widget already computed.
///
/// `default_*` encode today's look, so a style that does not author `key` gets
/// a byte-identical result — conversions are zero-visual-change until someone
/// opts in. That property is what makes it safe to wire the whole widget set.
pub(crate) fn resolve_control_chrome(
    ctx: &egui::Context,
    theme: &dyn ComponentTheme,
    key: &str,
    default_radius: f32,
    default_fill: egui::Color32,
    default_border: egui::Color32,
    default_border_w: f32,
) -> (egui::CornerRadius, egui::Color32, egui::Stroke) {
    use crate::ui_kit::sx::{Sx, StyleState};
    let recipes = get_ambient_recipes(ctx);
    let default_sx = Sx::new()
        .rounded(default_radius)
        .bg_color(default_fill)
        .border_color(default_border, default_border_w);
    let d = recipes.resolve(key, default_sx, theme).resolved(StyleState::Normal);
    let pal = crate::ui_kit::sx::palette_ct(theme);
    let radius = d.radius.unwrap_or(default_radius);
    let fill = d.fill_color(&pal).unwrap_or(default_fill);
    let (bw, bc) = match d.border_spec() {
        Some(b) => (b.width, d.resolved_border_color(&pal).unwrap_or(default_border)),
        None => (default_border_w, default_border),
    };
    (
        egui::CornerRadius::same(radius.clamp(0.0, 255.0).round() as u8),
        fill,
        egui::Stroke::new(bw, bc),
    )
}

/// Resolve a widget-declared [`Sx`] through the ambient [`RecipeSet`].
///
/// The companion to [`resolve_control_chrome`], for widgets that already
/// DECLARE their box as an `Sx` and paint it in one call (`Alert`, `Badge`,
/// `Kbd`). Passing the widget's own `Sx` as the default keeps the
/// zero-visual-change property: an unauthored key returns it untouched.
pub(crate) fn resolve_sx(
    ctx: &egui::Context,
    theme: &dyn ComponentTheme,
    key: &str,
    default_sx: crate::ui_kit::sx::Sx,
) -> crate::ui_kit::sx::Sx {
    get_ambient_recipes(ctx).resolve(key, default_sx, theme)
}
