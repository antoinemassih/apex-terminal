//! Sized variant tokens used by foundation shells.
//!
//! `Size`, `Density`, `Radius` resolve through existing `style::*` token
//! helpers — never duplicate raw values here.

#![allow(dead_code, unused_imports)]

use egui::{CornerRadius, Margin};
use super::super::style::*;

/// Size scale used by every shell (button, row, card, input, chip).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size { Xs, Sm, Md, Lg, Xl }

impl Size {
    /// Recommended height in logical px. Md/Lg pull from StyleSettings so the
    /// `button_height_px` knob applies to the primary button sizes.
    pub fn height(self) -> f32 {
        let st = current();
        match self {
            Size::Xs => 16.0,
            Size::Sm => btn_small_height(),       // 22.0 (fixed compact)
            Size::Md => st.button_height_px,      // driven by style knob
            Size::Lg => st.button_height_px + 4.0, // slightly taller than Md
            Size::Xl => st.button_height_px + 8.0,
        }
    }

    /// Inner padding for the shell. Md/Lg x-padding reads from `button_padding_x`.
    pub fn padding(self) -> Margin {
        let st = current();
        let (x, y) = match self {
            Size::Xs => (gap_sm(), gap_xs()),
            Size::Sm => (gap_md(), gap_xs()),
            Size::Md => (st.button_padding_x, gap_sm()),
            Size::Lg => (st.button_padding_x + 2.0, gap_md()),
            Size::Xl => (gap_2xl(), gap_lg()),
        };
        Margin { left: x as i8, right: x as i8, top: y as i8, bottom: y as i8 }
    }

    /// Default font size for text living inside a shell of this size.
    pub fn font(self) -> f32 {
        match self {
            Size::Xs => font_xs(),
            Size::Sm => font_sm(),
            Size::Md => font_md(),
            Size::Lg => font_lg(),
            Size::Xl => font_xl(),
        }
    }
}

/// Vertical density mode applied across rows / lists / cards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density { Compact, Default, Comfortable }

impl Density {
    /// Scale factor applied to vertical padding. 1.0 == Default.
    pub fn vscale(self) -> f32 {
        match self {
            Density::Compact => 0.65,
            Density::Default => 1.0,
            Density::Comfortable => 1.4,
        }
    }
}

/// Radius scale. Pill is fully rounded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Radius { None, Xs, Sm, Md, Lg, Pill }

impl Radius {
    pub fn corner(self) -> CornerRadius {
        // Read from StyleSettings so radius switching works across style presets (#1).
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
