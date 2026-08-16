//! UI Kit — Design system for the native GPU chart.
//! Provides consistent theming, icons, and reusable widgets.
//! Some items are reserved for future use.

#[allow(dead_code)]
pub mod icons;
#[allow(dead_code)]
pub mod widgets;
pub mod symbols;
pub mod tokens;
pub mod style;
pub mod text_style;
/// M3.3: THE interaction-state table — hover / pressed / focus / selected /
/// disabled → paint-ready `Visuals`. Owned here (was chart-side foundation)
/// so widgets can reach it; the chart layer re-exports it via
/// `chart::renderer::ui::foundation::interaction`.
pub mod interaction;
/// Taffy-backed flexbox layout (geometry only; styling stays in the tokens).
pub mod layout;
/// The declarative cascading layer: parent-to-child inheritance plus an
/// element tree that compiles down to egui. An ORGANIZING system, not a
/// renderer — no VDOM, no diffing.
///
/// The blanket `#[allow(dead_code)]` this carried while the module had no
/// consumers is GONE, deliberately. It was load-bearing for about a day and
/// then became a place for unused API to hide: the whole risk with an
/// additive layer is that it is written, admired, and never called, and an
/// allow at the module root is exactly what makes that invisible. Now that
/// widgets, the pane chrome and the tab strip go through it, the compiler
/// telling us a constructor is unreachable is information we want.
pub mod cascade;
/// Typed design scales (Space/Radius/Weight/Level) — the constraint layer.
pub mod scale;
pub mod cursor;
#[allow(dead_code)]
pub mod sx;
/// Bug-Inspect anchor registry. Portable (egui + std only) — owned here so
/// widgets no longer reach into `chart_renderer::bug_anchor` to instrument
/// themselves. The chart-app's `bug_anchor` re-exports these items.
#[allow(dead_code)]
pub mod inspect;
/// Tests only — asserts the `chart_renderer -> ui_kit` dependency direction
/// never flips back. See `KNOWN_VIOLATIONS` there for what is still pending.
#[cfg(test)]
mod layer_guard;

// S7: Themable assets — fonts, icons, imagery
#[allow(dead_code)]
pub mod fonts;
#[allow(dead_code)]
pub mod assets;

/// Line-style enum — used by drawing widgets and renderers. Portable
/// primitive (no theme/state coupling); lives here so `ui_kit` doesn't have
/// to import it back from `chart_renderer`. The chart-app re-exports this
/// at `crate::chart_renderer::LineStyle` for back-compat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineStyle { Solid, Dashed, Dotted }
