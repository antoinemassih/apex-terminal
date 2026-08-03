//! M3.3: the interaction-state system moved DOWN into `ui_kit`
//! (`crate::ui_kit::interaction`) so the design-system widgets can finally
//! derive hover / pressed / focus / selected / disabled visuals from it —
//! the dependency direction (`chart` -> `ui_kit`) had fenced all 75 widget
//! files out, which is why the designed system had ~2 call sites while ~196
//! hand-rolled `if response.hovered() { .. }` blocks picked their own colours.
//! This shim keeps every existing chart-side path
//! (`foundation::interaction::InteractionState`, `foundation::InteractionState`,
//! `apply_interaction`, `HoverTreatment`, `InteractionTokens`) compiling
//! unchanged.

pub use crate::ui_kit::interaction::*;
