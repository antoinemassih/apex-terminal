//! Foundation-shell tokens.
//!
//! `Size` was unified into `ui_kit::widgets::tokens::Size` (5-tier, P2.4
//! consolidation, 2026-05). Importers should use that one.
//!
//! `Radius` stays here because its `Pill` variant reads `StyleSettings.r_pill`
//! which varies per style preset (e.g. Meridien r_pill = 0) — the ui_kit
//! equivalent `radius_pill()` is a fixed 999.0 constant with no preset awareness.
//! Unifying requires the style-axis decision deferred to Phase 5.

#![allow(dead_code, unused_imports)]

use egui::CornerRadius;
use super::super::style::*;

/// Radius scale. Pill reads the per-preset `r_pill` knob.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Radius { None, Xs, Sm, Md, Lg, Pill }

impl Radius {
    pub fn corner(self) -> CornerRadius {
        // Read from StyleSettings so radius switching works across style presets.
        let st = current();
        match self {
            Radius::None => CornerRadius::ZERO,
            Radius::Xs   => CornerRadius::same(st.r_xs),
            Radius::Sm   => CornerRadius::same(st.r_sm),
            Radius::Md   => CornerRadius::same(st.r_md),
            Radius::Lg   => CornerRadius::same(st.r_lg),
            Radius::Pill => CornerRadius::same(st.r_pill),
        }
    }
}
