//! Foundation variant enums + per-variant color resolution.
//!
//! Relocated from `chart/renderer/ui/foundation/variants.rs` — behaviour preserved verbatim.
//!
//! Shells never resolve theme colors inline — they call
//! `variant.fill_color(theme)` / `.fg_color(theme)` / `.border_color(theme)`.
//!
//! Theme color access is restricted to the real fields on
//! `chart_renderer::gpu::Theme`: `text`, `dim`, `accent`, `bull`, `bear`, `bg`,
//! `toolbar_bg`, `toolbar_border`.


use egui::Color32;
use crate::ui_kit::tokens::*;
use crate::ui_kit::widgets::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};

// ─── ButtonVariant ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonVariant { Primary, Secondary, Ghost, Destructive, Subtle, Brand }

impl ButtonVariant {
    pub fn fill_color(self, t: &dyn ComponentTheme) -> Color32 {
        match self {
            ButtonVariant::Primary     => color_alpha(palette_ct(t).base(Tone::Accent), alpha_muted()),
            ButtonVariant::Secondary   => color_alpha(t.surface_border(), alpha_soft()),
            ButtonVariant::Ghost       => Color32::TRANSPARENT,
            ButtonVariant::Destructive => color_alpha(palette_ct(t).base(Tone::Bear), alpha_muted()),
            ButtonVariant::Subtle      => color_alpha(t.surface_border(), alpha_ghost()),
            ButtonVariant::Brand       => palette_ct(t).base(Tone::Accent),
        }
    }
    pub fn fg_color(self, t: &dyn ComponentTheme) -> Color32 {
        match self {
            ButtonVariant::Primary     => palette_ct(t).base(Tone::Accent),
            ButtonVariant::Secondary   => palette_ct(t).base(Tone::Text),
            ButtonVariant::Ghost       => palette_ct(t).base(Tone::Dim),
            ButtonVariant::Destructive => palette_ct(t).base(Tone::Bear),
            ButtonVariant::Subtle      => palette_ct(t).base(Tone::Dim),
            ButtonVariant::Brand       => contrast_fg(palette_ct(t).base(Tone::Accent)),
        }
    }
    pub fn border_color(self, t: &dyn ComponentTheme) -> Color32 {
        match self {
            ButtonVariant::Primary     => color_alpha(palette_ct(t).base(Tone::Accent), alpha_active()),
            ButtonVariant::Secondary   => color_alpha(t.surface_border(), alpha_dim()),
            ButtonVariant::Ghost       => Color32::TRANSPARENT,
            ButtonVariant::Destructive => color_alpha(palette_ct(t).base(Tone::Bear), alpha_active()),
            ButtonVariant::Subtle      => color_alpha(t.surface_border(), alpha_muted()),
            ButtonVariant::Brand       => color_alpha(palette_ct(t).base(Tone::Accent), alpha_active()),
        }
    }
}

// ─── CardVariant ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardVariant { Bordered, Elevated, Ghost, Filled }

impl CardVariant {
    pub fn fill_color(self, t: &dyn ComponentTheme) -> Color32 {
        match self {
            CardVariant::Bordered => palette_ct(t).base(Tone::Surface),
            CardVariant::Elevated => palette_ct(t).base(Tone::Surface),
            CardVariant::Ghost    => Color32::TRANSPARENT,
            CardVariant::Filled   => color_alpha(t.surface_border(), alpha_soft()),
        }
    }
    pub fn fg_color(self, t: &dyn ComponentTheme) -> Color32 { palette_ct(t).base(Tone::Text) }
    pub fn border_color(self, t: &dyn ComponentTheme) -> Color32 {
        match self {
            CardVariant::Bordered => color_alpha(t.surface_border(), alpha_strong()),
            CardVariant::Elevated => color_alpha(t.surface_border(), alpha_dim()),
            CardVariant::Ghost    => Color32::TRANSPARENT,
            CardVariant::Filled   => color_alpha(t.surface_border(), alpha_muted()),
        }
    }
}

// ─── ChipVariant ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipVariant { Solid, Outline, Subtle, Removable }

impl ChipVariant {
    pub fn fill_color(self, t: &dyn ComponentTheme) -> Color32 {
        match self {
            ChipVariant::Solid     => color_alpha(palette_ct(t).base(Tone::Accent), alpha_tint()),
            ChipVariant::Outline   => Color32::TRANSPARENT,
            ChipVariant::Subtle    => color_alpha(t.surface_border(), alpha_ghost()),
            ChipVariant::Removable => color_alpha(palette_ct(t).base(Tone::Accent), alpha_soft()),
        }
    }
    pub fn fg_color(self, t: &dyn ComponentTheme) -> Color32 {
        match self {
            ChipVariant::Solid | ChipVariant::Removable => palette_ct(t).base(Tone::Accent),
            ChipVariant::Outline => palette_ct(t).base(Tone::Text),
            ChipVariant::Subtle  => palette_ct(t).base(Tone::Dim),
        }
    }
    pub fn border_color(self, t: &dyn ComponentTheme) -> Color32 {
        match self {
            ChipVariant::Solid     => color_alpha(palette_ct(t).base(Tone::Accent), alpha_active()),
            ChipVariant::Outline   => color_alpha(t.surface_border(), alpha_dim()),
            ChipVariant::Subtle    => color_alpha(t.surface_border(), alpha_muted()),
            ChipVariant::Removable => color_alpha(palette_ct(t).base(Tone::Accent), alpha_strong()),
        }
    }
}

// ─── RowVariant ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowVariant { Default, Compact, Comfortable, Header, Subheader }

impl RowVariant {
    pub fn fill_color(self, t: &dyn ComponentTheme) -> Color32 {
        match self {
            RowVariant::Header    => color_alpha(t.surface_border(), alpha_soft()),
            RowVariant::Subheader => color_alpha(t.surface_border(), alpha_ghost()),
            _                     => Color32::TRANSPARENT,
        }
    }
    pub fn fg_color(self, t: &dyn ComponentTheme) -> Color32 {
        match self {
            RowVariant::Header | RowVariant::Subheader => palette_ct(t).base(Tone::Dim),
            _                                          => palette_ct(t).base(Tone::Text),
        }
    }
    pub fn border_color(self, t: &dyn ComponentTheme) -> Color32 {
        color_alpha(t.surface_border(), alpha_muted())
    }
}

// ─── InputVariant ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputVariant { Default, Search, Numeric, Password, Inline }

impl InputVariant {
    pub fn fill_color(self, t: &dyn ComponentTheme) -> Color32 {
        match self {
            InputVariant::Inline => Color32::TRANSPARENT,
            _                    => color_alpha(palette_ct(t).base(Tone::Bg), alpha_active()),
        }
    }
    pub fn fg_color(self, t: &dyn ComponentTheme) -> Color32 { palette_ct(t).base(Tone::Text) }
    pub fn border_color(self, t: &dyn ComponentTheme) -> Color32 {
        match self {
            InputVariant::Inline => Color32::TRANSPARENT,
            _                    => color_alpha(t.surface_border(), alpha_dim()),
        }
    }
}
