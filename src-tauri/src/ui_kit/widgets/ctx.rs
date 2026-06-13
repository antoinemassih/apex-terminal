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
//!    `RecipeSet` reference.  The existing `widget.show(ui, theme)` entry
//!    point builds one of these internally and calls `show_ctx` — so every
//!    existing caller keeps compiling unchanged.
//!
//! 2. Call sites that want per-call-site overrides construct
//!    `StyleCtx::new(theme, tokens, recipes)` and call `widget.show_ctx(ui, &ctx)`.
//!
//! # Example
//!
//! ```ignore
//! // Existing call (unchanged):
//! Button::new("Save").show(ui, &theme);
//!
//! // New opt-in path with custom tokens:
//! let ctx = StyleCtx::new(&theme, compact_tokens, &recipes);
//! Button::new("Save").show_ctx(ui, &ctx);
//!
//! // Two independent contexts in one frame — gallery proof:
//! let ctx_a = StyleCtx::new(&theme_a, tokens_a, &recipes_a);
//! let ctx_b = StyleCtx::new(&theme_b, tokens_b, &recipes_b);
//! // Both can coexist because ctx carries everything; no global mutation.
//! ```

use crate::design_system::recipes::RecipeSet;
use crate::ui_kit::style::{frame_tokens, TokenSnapshot};
use super::theme::ComponentTheme;

// ── Static empty RecipeSet ────────────────────────────────────────────────────
//
// `StyleCtx::from_theme` needs to return a `StyleCtx<'static>` recipe
// reference without heap-allocating each call.  A `static` empty set is
// `Default`-constructed once and never mutated.

static EMPTY_RECIPE_SET: std::sync::OnceLock<RecipeSet> = std::sync::OnceLock::new();

fn empty_recipes() -> &'static RecipeSet {
    EMPTY_RECIPE_SET.get_or_init(RecipeSet::new)
}

// ── StyleCtx ─────────────────────────────────────────────────────────────────

/// Threaded style context — bundles colour theme, dimension tokens, and
/// component recipes so they all travel together through a widget call.
///
/// Construct via [`StyleCtx::from_theme`] (the shim used inside
/// `show` wrappers) or [`StyleCtx::new`] for explicit control.
///
/// Lifetime `'a` is tied to the borrow of the `ComponentTheme` and
/// `RecipeSet` so this type is zero-allocation and suitable for stack use.
pub struct StyleCtx<'a> {
    theme:   &'a dyn ComponentTheme,
    tokens:  TokenSnapshot,
    recipes: &'a RecipeSet,
}

impl<'a> StyleCtx<'a> {
    /// Explicit constructor — caller supplies all three components.
    /// Use this when you need per-call-site token overrides or
    /// a non-default `RecipeSet`.
    pub fn new(
        theme:   &'a dyn ComponentTheme,
        tokens:  TokenSnapshot,
        recipes: &'a RecipeSet,
    ) -> Self {
        Self { theme, tokens, recipes }
    }

    /// Shim constructor — builds a `StyleCtx` from a theme reference
    /// using the current frame's `TokenSnapshot` and an empty `RecipeSet`.
    ///
    /// This is the constructor used inside `show(ui, theme)` wrappers so
    /// existing callers are never touched.
    pub fn from_theme(theme: &'a dyn ComponentTheme) -> Self {
        Self {
            theme,
            tokens: frame_tokens(),
            recipes: empty_recipes(),
        }
    }

    // ── Theme colour accessors ────────────────────────────────────────────

    /// Pass-through to the underlying theme so code with a `&StyleCtx` can
    /// call `ctx.theme()` and get a reference to the `ComponentTheme`.
    #[inline] pub fn theme(&self) -> &dyn ComponentTheme { self.theme }

    /// The full `RecipeSet` (for widgets that need to resolve recipe keys).
    #[inline] pub fn recipes(&self) -> &RecipeSet { self.recipes }

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
