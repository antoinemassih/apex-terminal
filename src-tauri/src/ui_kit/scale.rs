//! Typed design scales — the constraint layer.
//!
//! ## Why enums and not `f32`
//!
//! This codebase already had tokens (`gap_sm()`, `radius_md()`, …) before this
//! module existed, and it still drifted: the 2026-07-31 audit found 903
//! off-token primitives, ~70% of the app's text at 9-11px, and 12 distinct row
//! heights. Tokens alone were not enough, because every API that consumed them
//! took a plain `f32` — `Flex::gap(8.0)` is as valid as `Flex::gap(gap_sm())`,
//! and at 2am the literal wins.
//!
//! Tailwind's real discipline is not that it has a spacing scale; it is that
//! `p-4` is the ONLY convenient way to express padding. The scale is *closed*.
//! These enums do the same thing for Rust: an API that takes [`Space`] cannot
//! be handed `13.7`, so the design system stops competing with the shortcut and
//! simply becomes the shortcut.
//!
//! ## What they resolve to
//!
//! Nothing here invents values. Every variant maps onto the EXISTING token
//! functions, which in turn read the per-frame `TokenSnapshot` pushed by the
//! host — so live token editing, per-style presets and the 21 palettes all keep
//! working exactly as before. This is a typed front door onto the system that
//! already exists, not a second source of truth.
//!
//! ## Escape hatch
//!
//! [`Space::Exact`] / [`Radius::Exact`] exist because a handful of places
//! genuinely need a measured or caller-derived value (a galley width, a
//! half-height pill). They are deliberately verbose to type and easy to grep,
//! so they read as the exception they are.

use crate::ui_kit::style as st;

/// Spacing scale — gaps, padding, margins.
///
/// Resolves to the `gap_*` tokens. Ordered smallest → largest so
/// `Space::Sm < Space::Md` is meaningful.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub enum Space {
    /// No space at all.
    #[default]
    None,
    /// 2px-ish — hairline separation, icon-to-glyph.
    X2s,
    /// 4px-ish — the tightest real gutter.
    Xs,
    /// 6px-ish — between Xs and Sm; chip padding.
    XsMid,
    /// 8px-ish — the default gutter between related controls.
    Sm,
    /// 12px-ish — between groups within a section.
    Md,
    /// 16px-ish — panel inset, section padding.
    Lg,
    /// 20px-ish.
    Xl,
    /// 24px-ish — between major blocks.
    X2l,
    /// 32px-ish — page-level separation.
    X3l,
    /// Escape hatch: an exact pixel value. Use ONLY for measured/derived
    /// quantities (galley widths, half-heights). Anything you could have
    /// expressed on the scale should be on the scale.
    Exact(OrderedF32),
}

impl Space {
    /// Resolve to pixels via the live token snapshot.
    pub fn px(self) -> f32 {
        match self {
            Space::None => 0.0,
            Space::X2s => st::gap_2xs(),
            Space::Xs => st::gap_xs(),
            Space::XsMid => st::gap_xs_mid(),
            Space::Sm => st::gap_sm(),
            Space::Md => st::gap_md(),
            Space::Lg => st::gap_lg(),
            Space::Xl => st::gap_xl(),
            Space::X2l => st::gap_2xl(),
            Space::X3l => st::gap_3xl(),
            Space::Exact(v) => v.0,
        }
    }

    /// Escape hatch constructor — see [`Space::Exact`].
    pub fn exact(px: f32) -> Self {
        Space::Exact(OrderedF32(px))
    }
}

/// Corner-radius scale.
///
/// [`Radius::Style`] is the important one: it resolves to the ACTIVE STYLE's
/// region radius, which is how a single spec renders rounded on Aperture/Cadence
/// and sharp on Meridien/Mariner without the call site knowing anything about
/// styles. Prefer it for panel/card/region chrome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub enum Radius {
    /// Square corners.
    #[default]
    None,
    Xs,
    Sm,
    Md,
    Lg,
    /// Fully rounded (capsule). Per-style: some styles square this off.
    Pill,
    /// The active style's region/card radius — rounded on tiled styles, 0 on
    /// flush ones. The right default for panel and card chrome.
    Style,
    /// Escape hatch — measured/derived radii only (e.g. `h * 0.5`).
    Exact(OrderedF32),
}

impl Radius {
    pub fn px(self) -> f32 {
        match self {
            Radius::None => 0.0,
            Radius::Xs => st::radius_xs(),
            Radius::Sm => st::radius_sm(),
            Radius::Md => st::radius_md(),
            Radius::Lg => st::radius_lg(),
            Radius::Pill => st::radius_pill(),
            // Per-style region radius lives in the frame snapshot as radius_lg
            // for tiled styles and 0 for flush ones; the chart layer overrides
            // this via the ComponentTheme bridge where it needs the exact
            // `region_radius`. Portable hosts get the lg rung.
            Radius::Style => st::radius_lg(),
            Radius::Exact(v) => v.0,
        }
    }

    pub fn exact(px: f32) -> Self {
        Radius::Exact(OrderedF32(px))
    }

    pub fn cr(self) -> egui::CornerRadius {
        egui::CornerRadius::same(self.px().round().clamp(0.0, 255.0) as u8)
    }
}

/// Stroke-weight scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub enum Weight {
    /// No stroke.
    #[default]
    None,
    /// Sub-pixel hairline.
    Hair,
    /// The standard 0.5px divider.
    Thin,
    Medium,
    /// 1px.
    Std,
    Bold,
    Thick,
}

impl Weight {
    pub fn px(self) -> f32 {
        match self {
            Weight::None => 0.0,
            Weight::Hair => st::stroke_hair(),
            Weight::Thin => st::stroke_thin(),
            Weight::Medium => st::stroke_medium(),
            Weight::Std => st::stroke_std(),
            Weight::Bold => st::stroke_bold(),
            Weight::Thick => st::stroke_thick(),
        }
    }
}

/// Elevation scale — which surface a container paints.
///
/// Resolves through the `ComponentTheme` surface ladder, so it follows the
/// active palette (including the 5 light ones) instead of a frozen colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub enum Level {
    /// Paint nothing — inherit whatever is behind.
    #[default]
    None,
    /// The app canvas.
    Canvas,
    /// A side-panel / card body.
    Panel,
    /// A section band inside a panel.
    Section,
    /// A header band.
    Header,
    /// One step above the surrounding surface.
    Raised,
}

impl Level {
    pub fn color<T: crate::ui_kit::widgets::theme::ComponentTheme + ?Sized>(
        self,
        t: &T,
    ) -> Option<egui::Color32> {
        match self {
            Level::None => None,
            Level::Canvas => Some(t.bg()),
            Level::Panel => Some(t.panel_surface()),
            Level::Section => Some(t.section_header_surface()),
            Level::Header => Some(t.header_surface()),
            Level::Raised => Some(t.surface_raised()),
        }
    }
}

/// A hashable/orderable `f32` wrapper so the scale enums can derive `Eq`/`Ord`
/// and be used as map keys. Design values are never NaN; a NaN is treated as 0
/// rather than panicking, since a layout value is not worth crashing a trading
/// terminal over.
#[derive(Clone, Copy, Debug)]
pub struct OrderedF32(pub f32);

impl OrderedF32 {
    fn key(self) -> i64 {
        if self.0.is_nan() { 0 } else { (self.0 * 1000.0).round() as i64 }
    }
}
impl PartialEq for OrderedF32 {
    fn eq(&self, o: &Self) -> bool { self.key() == o.key() }
}
impl Eq for OrderedF32 {}
impl PartialOrd for OrderedF32 {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(o)) }
}
impl Ord for OrderedF32 {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering { self.key().cmp(&o.key()) }
}
impl std::hash::Hash for OrderedF32 {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) { self.key().hash(h) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_scale_is_monotonic() {
        let steps = [
            Space::None, Space::X2s, Space::Xs, Space::XsMid,
            Space::Sm, Space::Md, Space::Lg, Space::Xl, Space::X2l, Space::X3l,
        ];
        for w in steps.windows(2) {
            assert!(
                w[0].px() <= w[1].px(),
                "{:?} ({}) must not exceed {:?} ({})",
                w[0], w[0].px(), w[1], w[1].px()
            );
        }
    }

    #[test]
    fn radius_scale_is_monotonic() {
        let steps = [Radius::None, Radius::Xs, Radius::Sm, Radius::Md, Radius::Lg];
        for w in steps.windows(2) {
            assert!(w[0].px() <= w[1].px(), "{:?} must not exceed {:?}", w[0], w[1]);
        }
        assert!(Radius::Pill.px() >= Radius::Lg.px(), "Pill is the roundest rung");
    }

    #[test]
    fn weight_scale_is_monotonic() {
        let steps = [
            Weight::None, Weight::Hair, Weight::Thin,
            Weight::Medium, Weight::Std, Weight::Bold, Weight::Thick,
        ];
        for w in steps.windows(2) {
            assert!(w[0].px() <= w[1].px(), "{:?} must not exceed {:?}", w[0], w[1]);
        }
    }

    #[test]
    fn none_is_always_zero() {
        assert_eq!(Space::None.px(), 0.0);
        assert_eq!(Radius::None.px(), 0.0);
        assert_eq!(Weight::None.px(), 0.0);
    }

    #[test]
    fn exact_escape_hatch_round_trips() {
        assert_eq!(Space::exact(13.5).px(), 13.5);
        assert_eq!(Radius::exact(7.25).px(), 7.25);
    }

    /// A NaN must not panic — a bad layout number is not worth crashing a
    /// trading terminal for.
    #[test]
    fn nan_is_tolerated_not_panicking() {
        let s = Space::exact(f32::NAN);
        assert!(s.px().is_nan() || s.px() == 0.0);
        let _ = s == Space::exact(f32::NAN); // Eq must not panic
    }

    #[test]
    fn radius_converts_to_corner_radius() {
        assert_eq!(Radius::None.cr(), egui::CornerRadius::same(0));
        assert!(Radius::Lg.cr().nw > 0);
    }
}
