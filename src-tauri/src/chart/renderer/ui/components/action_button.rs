//! Big action button (canonical builder + legacy helper), side-pane action,
//! brand CTA. Defines `ActionTier`, `ActionSize`, `ActionButton`.

use super::super::style::*;
use egui::{self, Color32, Response, RichText, Stroke, Ui};
use crate::ui_kit::widgets::Button as KitButton;
use crate::ui_kit::widgets::tokens::{Variant as KitVariant, Size as KitSize};

// ─── Helper: luminance-aware contrast color ──────────────────────────────────

#[inline]
fn ds_contrast_fg(bg: Color32) -> Color32 {
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if lum > 140.0 { Color32::from_rgb(20, 20, 24) } else { Color32::from_rgb(240, 240, 244) }
}

// ─── BigActionButton ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionTier {
    Primary,
    Destructive,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionSize { Small, Medium, Large }

/// Legacy positional-arg helper for the big action button.
pub fn big_action_btn(
    ui: &mut Ui,
    label: &str,
    tier: ActionTier,
    size: ActionSize,
    accent: Color32,
    bear: Color32,
    dim: Color32,
    disabled: bool,
) -> Response {
    let height: f32 = match size { ActionSize::Small => 24.0, ActionSize::Medium => 32.0, ActionSize::Large => 40.0 };
    let kit_size = match size { ActionSize::Small => KitSize::Sm, ActionSize::Medium => KitSize::Md, ActionSize::Large => KitSize::Lg };
    let theme = crate::ui_kit::widgets::theme::active_theme(ui.ctx());
    let variant = match tier {
        ActionTier::Primary => KitVariant::Primary,
        ActionTier::Destructive => KitVariant::Danger,
        ActionTier::Secondary => KitVariant::Secondary,
    };
    KitButton::new(label).variant(variant).size(kit_size)
        .tint(if matches!(tier, ActionTier::Primary) { accent } else if matches!(tier, ActionTier::Destructive) { bear } else { accent })
        .disabled(disabled)
        .min_size(egui::vec2(0.0, height))
        .show(ui, &theme)
}

// ─── SidePaneActionButton ────────────────────────────────────────────────────

#[allow(unused_variables)]
pub fn side_pane_action_btn(
    ui: &mut Ui,
    icon: Option<&str>,
    label: &str,
    accent: Color32,
    dim: Color32,
) -> Response {
    let display = match icon {
        Some(ic) => format!("{} {}", ic, label),
        None => label.to_owned(),
    };
    let theme = crate::ui_kit::widgets::theme::active_theme(ui.ctx());
    KitButton::new(display.as_str()).variant(KitVariant::Secondary).size(KitSize::Sm)
        .tint(accent).min_size(egui::vec2(0.0, row_height_default()))
        .show(ui, &theme)
}

// ─── Brand CTA ────────────────────────────────────────────────────────────────

/// Brand-color CTA — like `big_action_btn` but with an explicit brand color
/// (e.g. Discord blurple from `palette.discord`). Uses the same height,
/// padding, font, radius, and border as `big_action_btn` so brand CTAs feel
/// like first-class action buttons in the same family.
pub fn brand_cta_button(
    ui: &mut Ui,
    label: &str,
    brand_color: Color32,
    fg_color: Color32,
    size: ActionSize,
    disabled: bool,
) -> Response {
    let height: f32 = match size { ActionSize::Small => 24.0, ActionSize::Medium => 32.0, ActionSize::Large => 40.0 };
    let kit_size = match size { ActionSize::Small => KitSize::Sm, ActionSize::Medium => KitSize::Md, ActionSize::Large => KitSize::Lg };
    let theme = crate::ui_kit::widgets::theme::active_theme(ui.ctx());
    // Chrome variant: brand_color is a custom fill (e.g. Discord blurple) that doesn't map to Primary/Danger.
    KitButton::new(label).variant(KitVariant::Chrome).size(kit_size)
        .fill(brand_color).fg(fg_color)
        .stroke(Stroke::new(stroke_thin(), color_alpha(brand_color, alpha_active())))
        .disabled(disabled).min_size(egui::vec2(0.0, height))
        .show(ui, &theme)
}
