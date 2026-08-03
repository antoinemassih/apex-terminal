//! Single source of truth for hover / focus / pressed / disabled / selected
//! visual treatment.
//!
//! M3.3: promoted DOWN into `ui_kit` from
//! `chart/renderer/ui/foundation/interaction.rs` (which is now a re-export
//! shim). The designed system had ~2 call sites because the dependency
//! direction (`chart` -> `ui_kit`) fenced all 75 widget files out of it —
//! exactly the same trap `TextStyle` was in before M2.1. Living here, the
//! design-system widgets can finally derive their interaction visuals from
//! ONE table instead of hand-rolling `if response.hovered() { .. }`.
//!
//! Usage:
//!
//! ```ignore
//! let st = InteractionState::new()
//!     .hovered(resp.hovered())
//!     .pressed(resp.is_pointer_button_down_on())
//!     .selected(is_active);
//! let v = apply_interaction(rect, st, theme.accent, &InteractionTokens::default());
//! if v.fill != Color32::TRANSPARENT { painter.rect_filled(rect, cr, v.fill); }
//! ```
//!
//! ### Tokens
//! `InteractionTokens::default()` resolves through
//! [`crate::ui_kit::style::frame_tokens`], so a host that pushes a
//! `TokenSnapshot` each frame drives every interaction visual in the app.
//!
//! Three knobs the chart-app's `StyleSettings` carries
//! (`hover_bg_alpha`, `active_bg_alpha`, `disabled_opacity`) have **no
//! `TokenSnapshot` field yet**, so they resolve to the nearest existing
//! alpha token here — see [`hover_bg_alpha_token`] /
//! [`pressed_bg_alpha_token`] / [`DISABLED_OPACITY_FALLBACK`]. The values
//! match the default (Aperture) preset exactly; when those fields land on
//! `TokenSnapshot`, swap the two fns and delete this note.

use egui::{Color32, Rect, Stroke};
use crate::ui_kit::style::{
    alpha_active, alpha_ghost, alpha_muted, alpha_tint, alpha_whisper, color_alpha,
    frame_tokens, stroke_thin,
};

/// Hover overlay alpha. Mirrors `StyleSettings::hover_bg_alpha` (15 on the
/// default Aperture preset) via the `alpha_ghost` token, which carries the
/// same value in `DEFAULT_TOKEN_SNAPSHOT`.
#[inline]
pub fn hover_bg_alpha_token() -> u8 { alpha_ghost() }

/// Pressed/active overlay alpha. Mirrors `StyleSettings::active_bg_alpha`
/// (25 on Aperture) via the `alpha_whisper` token (also 25).
#[inline]
pub fn pressed_bg_alpha_token() -> u8 { alpha_whisper() }

/// Foreground opacity multiplier for disabled elements. Mirrors
/// `StyleSettings::disabled_opacity` on the default preset.
pub const DISABLED_OPACITY_FALLBACK: f32 = 0.5;

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
#[derive(Clone, Copy, Debug, PartialEq)]
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
/// the `ui_kit::style` alpha/stroke helpers so theming a token cascades
/// everywhere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InteractionTokens {
    pub hover_bg_alpha: u8,
    pub hover_border_alpha: u8,
    /// Pressed/active overlay alpha (was read from `StyleSettings` inline).
    pub pressed_bg_alpha: u8,
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
        // Read the live frame snapshot so hover/focus/disabled knobs pushed by
        // the host propagate to every shell using `InteractionTokens::default()`.
        let snap = frame_tokens();
        Self {
            hover_bg_alpha:        hover_bg_alpha_token(),
            hover_border_alpha:    alpha_muted(),
            pressed_bg_alpha:      pressed_bg_alpha_token(),
            focus_ring_width:      snap.focus_ring_width,
            focus_ring_alpha:      snap.focus_ring_alpha,
            pressed_scale:         0.97,
            disabled_opacity:      DISABLED_OPACITY_FALLBACK,
            selected_bg_alpha:     alpha_tint(),
            selected_border_alpha: alpha_active(),
            hover_treatment:       HoverTreatment::AccentTint,
        }
    }
}

impl InteractionTokens {
    /// A borderless variant — hover/selected paint a fill only, no outline.
    /// The common shape for list rows, section header strips and inline icon
    /// buttons, which never drew a hover border by hand.
    pub fn borderless() -> Self {
        Self { hover_border_alpha: 0, selected_border_alpha: 0, ..Self::default() }
    }

    /// Set the hover overlay alpha (`HoverTreatment::AccentTint` only).
    pub fn hover_alpha(mut self, a: u8) -> Self { self.hover_bg_alpha = a; self }

    /// Set the selected overlay alpha.
    pub fn selected_alpha(mut self, a: u8) -> Self { self.selected_bg_alpha = a; self }

    /// Swap the hover paint strategy.
    pub fn treatment(mut self, h: HoverTreatment) -> Self { self.hover_treatment = h; self }
}

/// Painted appearance derived from an `InteractionState`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Visuals {
    pub fill: Color32,
    pub stroke: Stroke,
    /// Multiplier callers should apply to text/icon color to convey disabled.
    pub fg_modifier: f32,
}

impl Visuals {
    /// `true` when nothing needs painting — lets callers skip the rect entirely.
    #[inline]
    pub fn is_idle(&self) -> bool {
        self.fill == Color32::TRANSPARENT && self.stroke.width <= 0.0
    }

    /// `base` with the disabled foreground multiplier applied. Callers use this
    /// for text / glyph colors so disabled reads as dimmed without every widget
    /// re-deriving the multiplier.
    #[inline]
    pub fn fg(&self, base: Color32) -> Color32 {
        if self.fg_modifier >= 1.0 { base } else { base.gamma_multiply(self.fg_modifier) }
    }
}

/// Fold an `InteractionState` over a base color, returning paint-ready visuals.
///
/// `base_color` is treated as the "accent" tint for this element — hover bg
/// and selected bg are derived as alpha-tinted versions of it.
///
/// **Disabled dominates.** A disabled element cannot be hovered, pressed or
/// focused in any meaningful sense, so those affordances are suppressed and
/// only `fg_modifier` is returned (plus `selected`, which is a data state
/// rather than an interaction affordance).
pub fn apply_interaction(
    _rect: Rect,
    state: InteractionState,
    base_color: Color32,
    tokens: &InteractionTokens,
) -> Visuals {
    let mut fill = Color32::TRANSPARENT;
    let mut stroke = Stroke::NONE;

    if state.disabled {
        // Interaction affordances are suppressed; `selected` still reads.
        if state.selected {
            fill = color_alpha(base_color, tokens.selected_bg_alpha);
            if tokens.selected_border_alpha > 0 {
                stroke = Stroke::new(
                    stroke_thin(),
                    color_alpha(base_color, tokens.selected_border_alpha),
                );
            }
        }
        return Visuals { fill, stroke, fg_modifier: tokens.disabled_opacity };
    }

    if state.selected {
        fill = color_alpha(base_color, tokens.selected_bg_alpha);
        if tokens.selected_border_alpha > 0 {
            stroke = Stroke::new(
                stroke_thin(),
                color_alpha(base_color, tokens.selected_border_alpha),
            );
        }
    } else if state.hover {
        fill = match tokens.hover_treatment {
            HoverTreatment::AccentTint        => color_alpha(base_color, tokens.hover_bg_alpha),
            HoverTreatment::WhiteVeil(a)      => Color32::from_white_alpha(a),
            HoverTreatment::BrightenColor(f)  => brighten_color(base_color, f),
            HoverTreatment::Custom(c)         => c,
        };
        if tokens.hover_border_alpha > 0 {
            stroke = Stroke::new(
                stroke_thin(),
                color_alpha(base_color, tokens.hover_border_alpha),
            );
        }
    }

    if state.pressed {
        fill = color_alpha(base_color, tokens.pressed_bg_alpha);
    }

    if state.focused {
        stroke = Stroke::new(
            tokens.focus_ring_width,
            color_alpha(base_color, tokens.focus_ring_alpha),
        );
    }

    Visuals { fill, stroke, fg_modifier: 1.0 }
}

/// Multiply RGB channels by `factor`, clamped to 255. Used by
/// `HoverTreatment::BrightenColor`.
pub fn brighten_color(c: Color32, factor: f32) -> Color32 {
    let f = factor.max(0.0);
    let r = ((c.r() as f32 * f).min(255.0)) as u8;
    let g = ((c.g() as f32 * f).min(255.0)) as u8;
    let b = ((c.b() as f32 * f).min(255.0)) as u8;
    Color32::from_rgba_premultiplied(r, g, b, c.a())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: Color32 = Color32::from_rgb(80, 160, 240);

    fn r() -> Rect { Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 20.0)) }

    fn v(state: InteractionState) -> Visuals {
        apply_interaction(r(), state, BASE, &InteractionTokens::default())
    }

    fn idle() -> Visuals { v(InteractionState::new()) }

    /// Idle is the "paint nothing" case — every other state must differ from it.
    #[test]
    fn idle_paints_nothing() {
        let i = idle();
        assert!(i.is_idle(), "idle must be paint-free, got {i:?}");
        assert_eq!(i.fg_modifier, 1.0);
    }

    #[test]
    fn hover_differs_from_idle() {
        let h = v(InteractionState::new().hovered(true));
        assert_ne!(h, idle(), "hover must produce a different visual than idle");
        assert_ne!(h.fill, Color32::TRANSPARENT, "hover must paint a fill");
    }

    #[test]
    fn pressed_differs_from_idle_and_hover() {
        let p = v(InteractionState::new().pressed(true));
        let h = v(InteractionState::new().hovered(true));
        assert_ne!(p, idle(), "pressed must differ from idle");
        assert_ne!(p.fill, h.fill, "pressed must read stronger than hover");
    }

    #[test]
    fn selected_differs_from_idle_and_hover() {
        let s = v(InteractionState::new().selected(true));
        let h = v(InteractionState::new().hovered(true));
        assert_ne!(s, idle(), "selected must differ from idle");
        assert_ne!(s.fill, h.fill, "selected must differ from hover");
    }

    #[test]
    fn focused_differs_from_idle() {
        let f = v(InteractionState::new().focused(true));
        assert_ne!(f, idle(), "focus ring must differ from idle");
        assert!(f.stroke.width > 0.0, "focus must paint a ring");
    }

    #[test]
    fn disabled_differs_from_idle() {
        let d = v(InteractionState::new().disabled(true));
        assert_ne!(d, idle(), "disabled must differ from idle");
        assert!(d.fg_modifier < 1.0, "disabled must dim the foreground");
    }

    /// The composition rule: disabled DOMINATES the interaction affordances.
    /// A widget that forgets to gate its hover check on `!disabled` still gets
    /// the right visual, because the table refuses to hover a dead control.
    #[test]
    fn disabled_composes_over_hover_pressed_focus() {
        let d = v(InteractionState::new().disabled(true));
        for st in [
            InteractionState::new().disabled(true).hovered(true),
            InteractionState::new().disabled(true).pressed(true),
            InteractionState::new().disabled(true).focused(true),
            InteractionState::new().disabled(true).hovered(true).pressed(true).focused(true),
        ] {
            assert_eq!(v(st), d, "disabled must absorb {st:?}");
        }
    }

    /// `selected` is a DATA state, not an interaction affordance, so it
    /// survives disabling (a disabled-but-selected row still reads selected).
    #[test]
    fn disabled_preserves_selected_fill() {
        let ds = v(InteractionState::new().disabled(true).selected(true));
        let s  = v(InteractionState::new().selected(true));
        assert_eq!(ds.fill, s.fill, "selected fill must survive disabling");
        assert!(ds.fg_modifier < 1.0, "…but the foreground still dims");
    }

    #[test]
    fn hover_treatments_are_distinct() {
        let t = |h: HoverTreatment| {
            apply_interaction(
                r(),
                InteractionState::new().hovered(true),
                BASE,
                &InteractionTokens::default().treatment(h),
            )
            .fill
        };
        let tint   = t(HoverTreatment::AccentTint);
        let veil   = t(HoverTreatment::WhiteVeil(20));
        let bright = t(HoverTreatment::BrightenColor(1.3));
        let custom = t(HoverTreatment::Custom(Color32::from_rgb(9, 9, 9)));
        assert_ne!(tint, veil);
        assert_ne!(tint, bright);
        assert_ne!(tint, custom);
    }

    #[test]
    fn borderless_tokens_drop_the_outline() {
        let h = apply_interaction(
            r(),
            InteractionState::new().hovered(true),
            BASE,
            &InteractionTokens::borderless(),
        );
        assert_eq!(h.stroke, Stroke::NONE, "borderless hover must not outline");
        assert_ne!(h.fill, Color32::TRANSPARENT, "…but must still fill");
    }

    #[test]
    fn fg_helper_dims_only_when_disabled() {
        let base = Color32::from_rgb(200, 200, 200);
        assert_eq!(idle().fg(base), base);
        let d = v(InteractionState::new().disabled(true));
        assert_ne!(d.fg(base), base);
    }
}
