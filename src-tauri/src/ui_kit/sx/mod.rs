//! `sx` — a Tailwind-like utility-style + variant-recipe system for the UI kit.
//!
//! Three layers, all immediate-mode-native and allocation-free on the hot path:
//!   - [`color`]  — semantic [`Tone`]s expanded into precomputed shade [`Palette`]s.
//!   - [`style`]  — [`Sx`] composable utility values with hover/active/disabled
//!                  state variants, painted via a reserved background shape.
//!   - [`recipe`] — [`Recipe`] variant resolution (cva), built once per component.
//!
//! See `color::palette` for the per-theme ramp cache and the module docs in each
//! file for the performance contract.

pub mod color;
pub mod style;
pub mod recipe;
pub mod recipe_spec;
pub mod recipes;

pub use color::{palette, palette_ct, Palette, Shade, Tone};
pub use style::{BorderSpec, Fill, Sx, SxDelta, StyleState};
pub use recipe::Recipe;
pub use recipe_spec::{RecipeDelta, RecipeKey, RecipeSpec};
