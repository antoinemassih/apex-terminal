//! `StyleCtx` — threaded style context for ui_kit widgets (Stream S5).
//!
//! # Purpose
//!
//! Widgets that previously read dimension tokens (`gap_xs()`, `radius_sm()`, …)
//! via free global functions suffered a "split-brain": the colour came from
//! `&dyn ComponentTheme` (correctly threaded), but dimensions came from a
//! thread-local (`frame_tokens()`) that cannot be overridden per call-site.
//! `StyleCtx` bundles theme, tokens, and recipes into a single value so that
//! two independent `StyleCtx` instances coexisting in the same frame can carry
//! different dimension scales (e.g. a compact density preview next to a
//! standard one).
//!
//! # Migration path
//!
//! 1. `StyleCtx::from_theme(theme)` is the zero-cost shim constructor.
//!    It reads the current frame's `TokenSnapshot` and carries an empty
//!    `RecipeSet` (via a static `Arc`).  The existing `widget.show(ui, theme)`
//!    entry point builds one of these internally and calls `show_ctx` — so
//!    every existing caller keeps compiling unchanged.
//!
//! 2. `StyleCtx::from_ctx(theme, egui_ctx)` is the ambient-aware constructor.
//!    It pulls the `RecipeSet` that the host stashed via
//!    `set_ambient_recipes(ctx, ...)` so widgets automatically pick up active
//!    theme-pack overrides.  The `show(ui, theme)` path should be upgraded to
//!    this form once widgets are ready to adopt recipes.
//!
//! 3. Call sites that want per-call-site overrides construct
//!    `StyleCtx::new(theme, tokens, Arc::new(recipes))` and call
//!    `widget.show_ctx(ui, &ctx)`.
//!
//! # Example
//!
//! ```ignore
//! // Existing call (unchanged):
//! Button::new("Save").show(ui, &theme);
//!
//! // Ambient-aware path (picks up set_ambient_recipes automatically):
//! let ctx = StyleCtx::from_ctx(&theme, ui.ctx());
//! Button::new("Save").show_ctx(ui, &ctx);
//!
//! // Explicit override with custom tokens + recipe set:
//! let ctx = StyleCtx::new(&theme, compact_tokens, Arc::new(recipes));
//! Button::new("Save").show_ctx(ui, &ctx);
//!
//! // Two independent contexts in one frame — gallery proof:
//! let ctx_a = StyleCtx::from_ctx(&theme_a, ui.ctx());
//! let ctx_b = StyleCtx::new(&theme_b, tokens_b, Arc::new(recipes_b));
//! // Both can coexist because ctx carries everything; no global mutation.
//! ```

use std::sync::Arc;

use crate::design_system::recipes::RecipeSet;
use crate::ui_kit::style::{frame_tokens, TokenSnapshot};
use super::theme::ComponentTheme;

// ── StyleCtx ─────────────────────────────────────────────────────────────────

/// Threaded style context — bundles colour theme, dimension tokens, and
/// component recipes so they all travel together through a widget call.
///
/// Construct via:
/// - [`StyleCtx::from_theme`] — shim for `show(ui, theme)` call sites; empty
///   RecipeSet (no visual change).
/// - [`StyleCtx::from_ctx`] — ambient-aware; pulls the `RecipeSet` stashed by
///   the host via [`super::theme::set_ambient_recipes`].
/// - [`StyleCtx::new`] — explicit control over all three components.
///
/// Lifetime `'a` is tied only to the `ComponentTheme` borrow. The `RecipeSet`
/// is ref-counted (`Arc`) so the context is cheap to build — no recipe-map copy.
pub struct StyleCtx<'a> {
    theme:   &'a dyn ComponentTheme,
    tokens:  TokenSnapshot,
    /// The active `RecipeSet`. Wrapped in `Arc` so `from_ctx` can clone the
    /// ambient arc in O(1) without copying the map. `from_theme` uses a static
    /// empty arc — effectively free.
    recipes: Arc<RecipeSet>,
}

impl<'a> StyleCtx<'a> {
    /// Explicit constructor — caller supplies all three components.
    ///
    /// Use this when you need per-call-site token overrides or an explicit
    /// `RecipeSet` (e.g. a preview widget rendering against a non-ambient set).
    pub fn new(
        theme:   &'a dyn ComponentTheme,
        tokens:  TokenSnapshot,
        recipes: Arc<RecipeSet>,
    ) -> Self {
        Self { theme, tokens, recipes }
    }

    /// Ambient-aware constructor — pulls the active [`RecipeSet`] that the host
    /// stashed via [`super::theme::set_ambient_recipes`].
    ///
    /// This is the preferred upgrade path for `show(ui, theme)` wrappers that
    /// want their widget to adopt recipe overrides without the caller having to
    /// thread a `RecipeSet` explicitly. Falls back to an empty static set when
    /// no host has called `set_ambient_recipes` this frame (unit tests, etc.) —
    /// zero visual change.
    pub fn from_ctx(theme: &'a dyn ComponentTheme, ctx: &egui::Context) -> Self {
        Self {
            theme,
            tokens: frame_tokens(),
            recipes: super::theme::get_ambient_recipes(ctx),
        }
    }

    /// Shim constructor — builds a `StyleCtx` from a theme reference using the
    /// current frame's `TokenSnapshot` and a static empty `RecipeSet`.
    ///
    /// This is the constructor used inside `show(ui, theme)` wrappers that have
    /// not yet been upgraded to `from_ctx`. Existing callers are never touched.
    ///
    /// **Upgrade path:** replace `StyleCtx::from_theme(theme)` with
    /// `StyleCtx::from_ctx(theme, ui.ctx())` in `show` wrappers once the widget
    /// is ready to participate in recipe-driven restyling.
    pub fn from_theme(theme: &'a dyn ComponentTheme) -> Self {
        Self {
            theme,
            tokens: frame_tokens(),
            recipes: super::theme::empty_recipe_arc(),
        }
    }

    // ── Theme colour accessors ────────────────────────────────────────────

    /// Pass-through to the underlying theme so code with a `&StyleCtx` can
    /// call `ctx.theme()` and get a reference to the `ComponentTheme`.
    #[inline] pub fn theme(&self) -> &dyn ComponentTheme { self.theme }

    /// The full `RecipeSet` (for widgets that need to resolve recipe keys).
    #[inline] pub fn recipes(&self) -> &RecipeSet { &self.recipes }

    /// The raw `TokenSnapshot` (for widgets that need direct field access).
    #[inline] pub fn tokens(&self) -> &TokenSnapshot { &self.tokens }

    // ── Dimension accessors ───────────────────────────────────────────────
    //
    // These are the split-brain fix: widgets that previously called
    // `crate::ui_kit::style::gap_xs()` (global thread-local) now call
    // `ctx.gap_xs()` — they receive whatever tokens were threaded into this
    // `StyleCtx`, not the global default.
    //
    // Note: the radius helpers on `ComponentTheme` (trait defaults) still
    // call the global `crate::ui_kit::style::radius_xs()` which applies the
    // `corner_scale_override()` multiplier.  The `StyleCtx` helpers below
    // apply the same multiplier so they remain consistent.

    #[inline] pub fn font_2xs(&self) -> f32 { self.tokens.font_2xs }
    #[inline] pub fn font_xs(&self)  -> f32 { self.tokens.font_xs  }
    #[inline] pub fn font_sm(&self)  -> f32 { self.tokens.font_sm  }
    #[inline] pub fn font_md(&self)  -> f32 { self.tokens.font_md  }
    #[inline] pub fn font_lg(&self)  -> f32 { self.tokens.font_lg  }
    #[inline] pub fn font_xl(&self)  -> f32 { self.tokens.font_xl  }

    #[inline] pub fn gap_xs(&self)     -> f32 { self.tokens.gap_xs     }
    #[inline] pub fn gap_xs_mid(&self) -> f32 { self.tokens.gap_xs_mid }
    #[inline] pub fn gap_sm(&self)     -> f32 { self.tokens.gap_sm     }
    #[inline] pub fn gap_md(&self)     -> f32 { self.tokens.gap_md     }
    #[inline] pub fn gap_lg(&self)     -> f32 { self.tokens.gap_lg     }
    #[inline] pub fn gap_xl(&self)     -> f32 { self.tokens.gap_xl     }
    #[inline] pub fn gap_2xl(&self)    -> f32 { self.tokens.gap_2xl    }
    #[inline] pub fn gap_3xl(&self)    -> f32 { self.tokens.gap_3xl    }

    // Radii apply the corner_scale_override multiplier (same as the global
    // `radius_xs()` … `radius_lg()` helpers) so button/tab corner treatment
    // respects the user's Sharp/Round override even when using a custom ctx.
    #[inline] pub fn radius_xs(&self) -> f32 {
        self.tokens.radius_xs * crate::ui_kit::style::corner_scale_override().scale()
    }
    #[inline] pub fn radius_sm(&self) -> f32 {
        self.tokens.radius_sm * crate::ui_kit::style::corner_scale_override().scale()
    }
    #[inline] pub fn radius_md(&self) -> f32 {
        self.tokens.radius_md * crate::ui_kit::style::corner_scale_override().scale()
    }
    #[inline] pub fn radius_lg(&self) -> f32 {
        self.tokens.radius_lg * crate::ui_kit::style::corner_scale_override().scale()
    }

    // Strokes apply the border_weight_override multiplier.
    #[inline] pub fn stroke_hair(&self) -> f32 {
        self.tokens.stroke_hair * crate::ui_kit::style::border_weight_override().scale()
    }
    #[inline] pub fn stroke_thin(&self) -> f32 {
        self.tokens.stroke_thin * crate::ui_kit::style::border_weight_override().scale()
    }
    #[inline] pub fn stroke_medium(&self) -> f32 {
        self.tokens.stroke_medium * crate::ui_kit::style::border_weight_override().scale()
    }
    #[inline] pub fn stroke_std(&self) -> f32 {
        self.tokens.stroke_std * crate::ui_kit::style::border_weight_override().scale()
    }
    #[inline] pub fn stroke_bold(&self) -> f32 {
        self.tokens.stroke_bold * crate::ui_kit::style::border_weight_override().scale()
    }
    #[inline] pub fn stroke_thick(&self) -> f32 {
        self.tokens.stroke_thick * crate::ui_kit::style::border_weight_override().scale()
    }

    // ── Alpha passthrough (direct, no multiplier needed) ─────────────────

    #[inline] pub fn alpha_faint(&self)  -> u8 { self.tokens.alpha_faint  }
    #[inline] pub fn alpha_ghost(&self)  -> u8 { self.tokens.alpha_ghost  }
    #[inline] pub fn alpha_soft(&self)   -> u8 { self.tokens.alpha_soft   }
    #[inline] pub fn alpha_muted(&self)  -> u8 { self.tokens.alpha_muted  }
    #[inline] pub fn alpha_dim(&self)    -> u8 { self.tokens.alpha_dim    }
    #[inline] pub fn alpha_strong(&self) -> u8 { self.tokens.alpha_strong }
    #[inline] pub fn alpha_active(&self) -> u8 { self.tokens.alpha_active }
    #[inline] pub fn alpha_heavy(&self)  -> u8 { self.tokens.alpha_heavy  }
    #[inline] pub fn alpha_solid(&self)  -> u8 { self.tokens.alpha_solid  }
}
