//! Single source of truth for hover / focus / pressed / disabled / selected
//! visual treatment.
//!
//! Every shell calls `apply_interaction(..)` to derive its painted appearance
//! so hover treatment never has to be re-implemented.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Rect, Stroke};
use super::super::super::style::*;

/// Composable interaction flags. Shells set bits as they observe state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InteractionState {
    pub hover: bool,
    pub pressed: bool,
    pub focused: bool,
    pub selected: bool,
    pub disabled: bool,
}

impl InteractionState {
    pub fn new() -> Self { Self::default() }
    pub fn hovered(mut self, v: bool)  -> Self { self.hover = v; self }
    pub fn pressed(mut self, v: bool)  -> Self { self.pressed = v; self }
    pub fn focused(mut self, v: bool)  -> Self { self.focused = v; self }
    pub fn selected(mut self, v: bool) -> Self { self.selected = v; self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
}

/// How the hover state should be visualized. Different widget families call
/// for different paint strategies — buttons want a white veil, trade buttons
/// want a brightened bull/bear, ghost buttons want an accent tint.
#[derive(Clone, Copy, Debug)]
pub enum HoverTreatment {
    /// Tint with `base_color` at `tokens.hover_bg_alpha`.
    AccentTint,
    /// Paint a white veil over the rect at the given alpha (0..=255).
    WhiteVeil(u8),
    /// Brighten `base_color` by the given factor (>1.0 = brighter).
    BrightenColor(f32),
    /// Use this exact color as the hover overlay fill.
    Custom(Color32),
}

impl Default for HoverTreatment {
    fn default() -> Self { HoverTreatment::AccentTint }
}

/// Numerical knobs that drive the interaction layer. Values resolve through
/// existing `style::alpha_*` helpers so theming a token cascades everywhere.
#[derive(Clone, Copy, Debug)]
pub struct InteractionTokens {
    pub hover_bg_alpha: u8,
    pub hover_border_alpha: u8,
    pub focus_ring_width: f32,
    pub focus_ring_alpha: u8,
    pub pressed_scale: f32,
    pub disabled_opacity: f32,
    pub selected_bg_alpha: u8,
    pub selected_border_alpha: u8,
    pub hover_treatment: HoverTreatment,
}

impl Default for InteractionTokens {
    fn default() -> Self {
        // Read live StyleSettings so hover/focus/disabled knobs in the inspector
        // propagate to all shells that use InteractionTokens::default().
        let st = current();
        Self {
            hover_bg_alpha:        st.hover_bg_alpha,
            hover_border_alpha:    alpha_muted(),
            focus_ring_width:      st.focus_ring_width,
            focus_ring_alpha:      st.focus_ring_alpha,
            pressed_scale:         0.97,
            disabled_opacity:      st.disabled_opacity,
            selected_bg_alpha:     alpha_tint(),
            selected_border_alpha: alpha_active(),
            hover_treatment:       HoverTreatment::AccentTint,
        }
    }
}

/// Painted appearance derived from an `InteractionState`.
#[derive(Clone, Copy, Debug)]
pub struct Visuals {
    pub fill: Color32,
    pub stroke: Stroke,
    /// Multiplier callers should apply to text/icon color to convey disabled.
    pub fg_modifier: f32,
}

/// Fold an `InteractionState` over a base color, returning paint-ready visuals.
///
/// `base_color` is treated as the "accent" tint for this element — hover bg
/// and selected bg are derived as alpha-tinted versions of it.
pub fn apply_interaction(
    _rect: Rect,
    state: InteractionState,
    base_color: Color32,
    tokens: &InteractionTokens,
) -> Visuals {
    let mut fill = Color32::TRANSPARENT;
    let mut stroke = Stroke::NONE;
    let mut fg_modifier = 1.0;

    if state.selected {
        fill = color_alpha(base_color, tokens.selected_bg_alpha);
        stroke = Stroke::new(stroke_thin(), color_alpha(base_color, tokens.selected_border_alpha));
    } else if state.hover {
        fill = match tokens.hover_treatment {
            HoverTreatment::AccentTint        => color_alpha(base_color, tokens.hover_bg_alpha),
            HoverTreatment::WhiteVeil(a)      => Color32::from_white_alpha(a),
            HoverTreatment::BrightenColor(f)  => brighten_color(base_color, f),
            HoverTreatment::Custom(c)         => c,
        };
        stroke = Stroke::new(stroke_thin(), color_alpha(base_color, tokens.hover_border_alpha));
    }

    if state.pressed {
        // active_bg_alpha from StyleSettings conveys press intensity.
        let active_alpha = current().active_bg_alpha;
        fill = color_alpha(base_color, active_alpha);
    }

    if state.focused {
        stroke = Stroke::new(
            tokens.focus_ring_width,
            color_alpha(base_color, tokens.focus_ring_alpha),
        );
    }

    if state.disabled {
        fg_modifier = tokens.disabled_opacity;
    }

    Visuals { fill, stroke, fg_modifier }
}

/// Multiply RGB channels by `factor`, clamped to 255. Used by
/// `HoverTreatment::BrightenColor`.
fn brighten_color(c: Color32, factor: f32) -> Color32 {
    let f = factor.max(0.0);
    let r = ((c.r() as f32 * f).min(255.0)) as u8;
    let g = ((c.g() as f32 * f).min(255.0)) as u8;
    let b = ((c.b() as f32 * f).min(255.0)) as u8;
    Color32::from_rgba_premultiplied(r, g, b, c.a())
}
