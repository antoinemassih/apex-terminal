//! Foundation variant enums + per-variant color resolution.
//!
//! Shells never resolve theme colors inline — they call
//! `variant.fill_color(theme)` / `.fg_color(theme)` / `.border_color(theme)`.
//!
//! Theme color access is restricted to the real fields on
//! `chart_renderer::gpu::Theme`: `text`, `dim`, `accent`, `bull`, `bear`, `bg`,
//! `toolbar_bg`, `toolbar_border`.

#![allow(dead_code, unused_imports)]

use egui::Color32;
use super::super::style::*;

type Theme = super::super::super::gpu::Theme;

// ─── ButtonVariant ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonVariant { Primary, Secondary, Ghost, Destructive, Subtle, Brand }

impl ButtonVariant {
    pub fn fill_color(self, t: &Theme) -> Color32 {
        match self {
            ButtonVariant::Primary     => color_alpha(t.accent, alpha_muted()),
            ButtonVariant::Secondary   => color_alpha(t.toolbar_border, alpha_soft()),
            ButtonVariant::Ghost       => Color32::TRANSPARENT,
            ButtonVariant::Destructive => color_alpha(t.bear, alpha_muted()),
            ButtonVariant::Subtle      => color_alpha(t.toolbar_border, alpha_ghost()),
            ButtonVariant::Brand       => t.accent,
        }
    }
    pub fn fg_color(self, t: &Theme) -> Color32 {
        match self {
            ButtonVariant::Primary     => t.accent,
            ButtonVariant::Secondary   => t.text,
            ButtonVariant::Ghost       => t.dim,
            ButtonVariant::Destructive => t.bear,
            ButtonVariant::Subtle      => t.dim,
            ButtonVariant::Brand       => contrast_fg(t.accent),
        }
    }
    pub fn border_color(self, t: &Theme) -> Color32 {
        match self {
            ButtonVariant::Primary     => color_alpha(t.accent, alpha_active()),
            ButtonVariant::Secondary   => color_alpha(t.toolbar_border, alpha_dim()),
            ButtonVariant::Ghost       => Color32::TRANSPARENT,
            ButtonVariant::Destructive => color_alpha(t.bear, alpha_active()),
            ButtonVariant::Subtle      => color_alpha(t.toolbar_border, alpha_muted()),
            ButtonVariant::Brand       => color_alpha(t.accent, alpha_active()),
        }
    }
}

// ─── CardVariant ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardVariant { Bordered, Elevated, Ghost, Filled }

impl CardVariant {
    pub fn fill_color(self, t: &Theme) -> Color32 {
        match self {
            CardVariant::Bordered => t.toolbar_bg,
            CardVariant::Elevated => t.toolbar_bg,
            CardVariant::Ghost    => Color32::TRANSPARENT,
            CardVariant::Filled   => color_alpha(t.toolbar_border, alpha_soft()),
        }
    }
    pub fn fg_color(self, t: &Theme) -> Color32 { t.text }
    pub fn border_color(self, t: &Theme) -> Color32 {
        match self {
            CardVariant::Bordered => color_alpha(t.toolbar_border, alpha_strong()),
            CardVariant::Elevated => color_alpha(t.toolbar_border, alpha_dim()),
            CardVariant::Ghost    => Color32::TRANSPARENT,
            CardVariant::Filled   => color_alpha(t.toolbar_border, alpha_muted()),
        }
    }
}

// ─── ChipVariant ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipVariant { Solid, Outline, Subtle, Removable }

impl ChipVariant {
    pub fn fill_color(self, t: &Theme) -> Color32 {
        match self {
            ChipVariant::Solid     => color_alpha(t.accent, alpha_tint()),
            ChipVariant::Outline   => Color32::TRANSPARENT,
            ChipVariant::Subtle    => color_alpha(t.toolbar_border, alpha_ghost()),
            ChipVariant::Removable => color_alpha(t.accent, alpha_soft()),
        }
    }
    pub fn fg_color(self, t: &Theme) -> Color32 {
        match self {
            ChipVariant::Solid | ChipVariant::Removable => t.accent,
            ChipVariant::Outline => t.text,
            ChipVariant::Subtle  => t.dim,
        }
    }
    pub fn border_color(self, t: &Theme) -> Color32 {
        match self {
            ChipVariant::Solid     => color_alpha(t.accent, alpha_active()),
            ChipVariant::Outline   => color_alpha(t.toolbar_border, alpha_dim()),
            ChipVariant::Subtle    => color_alpha(t.toolbar_border, alpha_muted()),
            ChipVariant::Removable => color_alpha(t.accent, alpha_strong()),
        }
    }
}

// ─── RowVariant ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowVariant { Default, Compact, Comfortable, Header, Subheader }

impl RowVariant {
    pub fn fill_color(self, t: &Theme) -> Color32 {
        match self {
            RowVariant::Header    => color_alpha(t.toolbar_border, alpha_soft()),
            RowVariant::Subheader => color_alpha(t.toolbar_border, alpha_ghost()),
            _                     => Color32::TRANSPARENT,
        }
    }
    pub fn fg_color(self, t: &Theme) -> Color32 {
        match self {
            RowVariant::Header | RowVariant::Subheader => t.dim,
            _                                          => t.text,
        }
    }
    pub fn border_color(self, t: &Theme) -> Color32 {
        color_alpha(t.toolbar_border, alpha_muted())
    }
}

// ─── InputVariant ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputVariant { Default, Search, Numeric, Password, Inline }

impl InputVariant {
    pub fn fill_color(self, t: &Theme) -> Color32 {
        match self {
            InputVariant::Inline => Color32::TRANSPARENT,
            _                    => color_alpha(t.bg, alpha_active()),
        }
    }
    pub fn fg_color(self, t: &Theme) -> Color32 { t.text }
    pub fn border_color(self, t: &Theme) -> Color32 {
        match self {
            InputVariant::Inline => Color32::TRANSPARENT,
            _                    => color_alpha(t.toolbar_border, alpha_dim()),
        }
    }
}
