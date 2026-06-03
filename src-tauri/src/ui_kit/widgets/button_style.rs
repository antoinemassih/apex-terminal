//! Per-widget `StyleSheet` trait for [`super::button::Button`].
//!
//! ## Why this pattern?
//!
//! `ComponentTheme` is a fat trait with ~20 semantic color methods shared
//! by every widget. The `ButtonStyle` trait is a narrow contract: it declares
//! *exactly* the three colors a Button needs (`fg`, `bg`, `border`) and the
//! two axes that change them (`Variant`, `ButtonState`). This is the iced
//! `widget::button::StyleSheet` idea applied to our stack.
//!
//! New call sites can implement `ButtonStyle` directly to fully customize one
//! button without touching `ComponentTheme`. Existing call sites keep using
//! `Button::show(ui, theme)` unchanged — under the hood, `show` constructs a
//! `DefaultButtonStyle` adapter and routes through `show_styled`. Zero
//! migration cost; full customization headroom for new code.

use egui::Color32;

use super::theme::ComponentTheme;
use super::tokens::Variant;
use crate::ui_kit::tokens as st;
use crate::ui_kit::sx::{palette_ct, Shade, Tone};

// ── ButtonState ───────────────────────────────────────────────────────────────

/// The interactive state the Button is currently in.
///
/// Passed to every `ButtonStyle` method so an implementor can return different
/// colors for each state (e.g., darker background on `Pressed`, muted on
/// `Disabled`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonState {
    /// Resting, no pointer interaction.
    Idle,
    /// Pointer is hovering over the button.
    Hover,
    /// Button is marked `.active(true)` (e.g., a toggle that is "on").
    Active,
    /// Pointer button is currently down (mid-click).
    Pressed,
    /// Button is non-interactive (`.disabled(true)`).
    Disabled,
    /// Button is awaiting an async result (`.loading(true)`). ButtonStyle
    /// implementors can use this to color the inline spinner cohesively
    /// with the rest of the variant (P6.3 — was previously a `bool` flag
    /// on the widget that ButtonStyle impls couldn't see).
    Loading,
}

// ── ButtonStyle trait ─────────────────────────────────────────────────────────

/// Per-widget style contract for [`super::button::Button`].
///
/// Implement this trait on your own type to fully control a button's colors
/// without touching `ComponentTheme`. Pass the implementor to
/// `Button::new("X").show_styled(ui, &my_style)`.
pub trait ButtonStyle {
    /// Foreground (text + icon glyph) color.
    fn fg(&self, variant: Variant, state: ButtonState) -> Color32;

    /// Background fill color.
    fn bg(&self, variant: Variant, state: ButtonState) -> Color32;

    /// Border/stroke color. Return `Color32::TRANSPARENT` to suppress the border.
    fn border(&self, variant: Variant, state: ButtonState) -> Color32;
}

// ── DefaultButtonStyle adapter ────────────────────────────────────────────────

/// Bridges any `ComponentTheme` into `ButtonStyle` so existing themes get
/// a `ButtonStyle` implementation for free — no migration needed at call sites.
///
/// `Button::show(ui, theme)` constructs one of these internally and calls
/// `show_styled`, keeping the two code paths in sync.
pub struct DefaultButtonStyle<'a> {
    theme: &'a dyn ComponentTheme,
}

impl<'a> DefaultButtonStyle<'a> {
    /// Wrap a `ComponentTheme` reference.
    pub fn new(theme: &'a dyn ComponentTheme) -> Self {
        Self { theme }
    }
}

impl<'a> ButtonStyle for DefaultButtonStyle<'a> {
    fn fg(&self, variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        let muted = |c: Color32| -> Color32 {
            Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 178)
        };
        let half = |c: Color32| -> Color32 { st::color_half(c) };
        let disabled_alpha = |c: Color32| -> Color32 {
            Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(),
                (c.a() as f32 * 0.5).round() as u8)
        };

        let base = match variant {
            Variant::Primary | Variant::Danger => {
                // White/contrast fg on filled buttons.
                st::contrast_fg(
                    if matches!(variant, Variant::Danger) { t.bear() } else { t.accent() }
                )
            }
            // DynamicTint: fg defaults to t.text() but the Button widget
            // honours `.tint(color)` as a stronger override at the call site —
            // see `resolve_tint()` in button.rs which is consulted before the
            // ButtonStyle fg is applied. So DynamicTint shares the Ghost
            // default here and gets its colour from the tint pipeline.
            Variant::Ghost | Variant::Chrome | Variant::TextOnly
            | Variant::DynamicTint => t.text(),
            Variant::Secondary => t.text(),
            Variant::Link => match state {
                // Slightly lighter accent on hover for link variant.
                ButtonState::Hover => st::color_alpha(t.accent(), 230),
                _ => t.accent(),
            },
            Variant::Chip | Variant::Toggle => match state {
                ButtonState::Idle => half(t.text()),
                ButtonState::Hover => t.text(),
                ButtonState::Active | ButtonState::Pressed => t.accent(),
                ButtonState::Disabled | ButtonState::Loading => half(t.dim()),
            },
            Variant::Tab => match state {
                ButtonState::Active => t.text(),
                _ => muted(t.dim()),
            },
            Variant::InlineClose => match state {
                ButtonState::Hover | ButtonState::Pressed => t.text(),
                _ => st::color_subtle(t.dim()),
            },
            Variant::MutedIcon => match state {
                ButtonState::Hover | ButtonState::Pressed => t.text(),
                _ => half(t.dim()),
            },
            // P5b — NeutralAction fg: contrast_fg against the variant's fill so the
            // text reads on any theme. Was hardcoded BLACK which became invisible
            // on dark palettes.
            Variant::NeutralAction => st::contrast_fg(t.surface()),
        };

        if matches!(state, ButtonState::Disabled | ButtonState::Loading) {
            disabled_alpha(base)
        } else {
            base
        }
    }

    fn bg(&self, variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        // Unified Sx palette — filled variants step along the tone's shade ramp
        // (S500 idle → S400 hover → S600 press) instead of ad-hoc lighten/darken
        // magic numbers, so the whole button system reads from one ramp source.
        let pal = palette_ct(t);
        let transparent = Color32::TRANSPARENT;
        let disabled_alpha = |c: Color32| -> Color32 {
            Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(),
                (c.a() as f32 * 0.5).round() as u8)
        };
        // Surface-lift helper (Secondary's idle→hover lighten — not a ramp step
        // because it nudges the neutral surface, not a semantic tone).
        let lighten = |c: Color32, amt: f32| -> Color32 {
            let lerp = |x: u8| -> u8 {
                let v = x as f32 + (255.0 - x as f32) * amt.clamp(0.0, 1.0);
                v.round().clamp(0.0, 255.0) as u8
            };
            Color32::from_rgba_premultiplied(lerp(c.r()), lerp(c.g()), lerp(c.b()), c.a())
        };

        let base = match variant {
            Variant::Primary => match state {
                ButtonState::Idle   => pal.base(Tone::Accent),
                ButtonState::Hover  => pal.shade(Tone::Accent, Shade::S400),
                ButtonState::Active | ButtonState::Pressed => pal.shade(Tone::Accent, Shade::S600),
                ButtonState::Disabled | ButtonState::Loading => pal.base(Tone::Accent),
            },
            Variant::Secondary => match state {
                ButtonState::Idle  => t.surface(),
                ButtonState::Hover => lighten(t.surface(), 0.08),
                ButtonState::Active | ButtonState::Pressed =>
                    st::color_alpha(t.accent(), st::alpha_tint()),
                ButtonState::Disabled | ButtonState::Loading => t.surface(),
            },
            Variant::Ghost => match state {
                ButtonState::Idle  => transparent,
                ButtonState::Hover => st::color_alpha(t.text(), 18),
                ButtonState::Active | ButtonState::Pressed =>
                    st::color_alpha(t.accent(), st::alpha_soft()),
                ButtonState::Disabled | ButtonState::Loading => transparent,
            },
            Variant::Danger => match state {
                ButtonState::Idle   => pal.base(Tone::Bear),
                ButtonState::Hover  => pal.shade(Tone::Bear, Shade::S400),
                ButtonState::Active | ButtonState::Pressed => pal.shade(Tone::Bear, Shade::S600),
                ButtonState::Disabled | ButtonState::Loading => pal.base(Tone::Bear),
            },
            Variant::Link | Variant::Tab | Variant::TextOnly |
            Variant::Chrome | Variant::InlineClose => transparent,
            // DynamicTint: transparent idle; on hover/active a low-alpha tint
            // overlay using the caller-provided color (via .tint()) — the
            // Button widget pipes the tint via `bg_override` so this default
            // only fires when no tint is set.
            Variant::DynamicTint => transparent,
            Variant::MutedIcon => match state {
                ButtonState::Hover | ButtonState::Pressed => st::color_alpha(t.text(), 18),
                _ => transparent,
            },
            Variant::Chip | Variant::Toggle => match state {
                ButtonState::Idle  => transparent,
                ButtonState::Hover => st::color_alpha(t.text(), 18),
                ButtonState::Active | ButtonState::Pressed =>
                    st::color_alpha(t.accent(), st::alpha_tint()),
                ButtonState::Disabled | ButtonState::Loading => transparent,
            },
            // P5b — NeutralAction bg: derive from theme surface with a slight
            // lift so the button reads as "neutral but present" on any palette.
            // Was hardcoded gray-170/190/150 which became a foreign object on
            // any non-default palette.
            Variant::NeutralAction => match state {
                ButtonState::Idle    => st::color_alpha(t.text(), 28),
                ButtonState::Hover   => st::color_alpha(t.text(), 44),
                ButtonState::Active | ButtonState::Pressed => st::color_alpha(t.text(), 18),
                ButtonState::Disabled | ButtonState::Loading => st::color_alpha(t.text(), 18),
            },
        };

        if matches!(state, ButtonState::Disabled | ButtonState::Loading) {
            disabled_alpha(base)
        } else {
            base
        }
    }

    fn border(&self, variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        let transparent = Color32::TRANSPARENT;

        match variant {
            Variant::Secondary | Variant::Toggle => match state {
                ButtonState::Active | ButtonState::Pressed =>
                    st::color_alpha(t.accent(), st::alpha_active()),
                _ => t.border(),
            },
            Variant::NeutralAction => t.border(),
            _ => transparent,
        }
    }
}

// ── OutlinedButtonStyle ───────────────────────────────────────────────────────

/// Material Outlined Button look — no fill, colored border, colored text.
///
/// Every variant renders with a transparent background and a 1px border
/// drawn in the variant's semantic color (accent for Primary, bear for
/// Danger, etc.). On hover the border brightens via alpha; on press a
/// faint tinted fill appears so the interaction registers without fully
/// "filling" the button. Ideal for secondary CTAs and filter rows where
/// fills would create too much visual noise.
pub struct OutlinedButtonStyle<'a> {
    theme: &'a dyn ComponentTheme,
}

impl<'a> OutlinedButtonStyle<'a> {
    /// Wrap a `ComponentTheme` reference.
    pub fn new(theme: &'a dyn ComponentTheme) -> Self {
        Self { theme }
    }
}

impl<'a> ButtonStyle for OutlinedButtonStyle<'a> {
    fn fg(&self, variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        let base = match variant {
            Variant::Danger => t.bear(),
            Variant::Ghost | Variant::Secondary => t.dim(),
            _ => t.accent(),
        };
        if matches!(state, ButtonState::Disabled | ButtonState::Loading) {
            st::color_alpha(base, 90)
        } else {
            base
        }
    }

    fn bg(&self, variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        match state {
            ButtonState::Hover => {
                let tint = match variant {
                    Variant::Danger => t.bear(),
                    _ => t.accent(),
                };
                st::color_alpha(tint, st::alpha_faint())
            }
            ButtonState::Active | ButtonState::Pressed => {
                let tint = match variant {
                    Variant::Danger => t.bear(),
                    _ => t.accent(),
                };
                st::color_alpha(tint, st::alpha_ghost())
            }
            _ => Color32::TRANSPARENT,
        }
    }

    fn border(&self, variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        let base = match variant {
            Variant::Danger => t.bear(),
            Variant::Ghost | Variant::Secondary => t.border(),
            _ => t.accent(),
        };
        if matches!(state, ButtonState::Disabled | ButtonState::Loading) {
            st::color_alpha(base, 60)
        } else if state == ButtonState::Hover {
            st::color_alpha(base, 200)
        } else {
            base
        }
    }
}

// ── SoftButtonStyle ───────────────────────────────────────────────────────────

/// Friendly modern tinted fill — no border, soft alpha background.
///
/// Fills the button with the variant's semantic color at `alpha_soft` (~20/255)
/// so it reads as "tinted" rather than "solid". The foreground uses the
/// variant color at high opacity for clear legibility over the pastel fill.
/// No border — the fill itself defines the button boundary. On hover the
/// tint deepens slightly; on press it reaches `alpha_tint`. Suitable for
/// dashboard action rows, tagging UIs, and any context where a solid fill
/// would be too heavy.
pub struct SoftButtonStyle<'a> {
    theme: &'a dyn ComponentTheme,
}

impl<'a> SoftButtonStyle<'a> {
    /// Wrap a `ComponentTheme` reference.
    pub fn new(theme: &'a dyn ComponentTheme) -> Self {
        Self { theme }
    }
}

impl<'a> ButtonStyle for SoftButtonStyle<'a> {
    fn fg(&self, variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        let base = match variant {
            Variant::Danger => t.bear(),
            Variant::Ghost | Variant::Secondary => t.text(),
            _ => t.accent(),
        };
        if matches!(state, ButtonState::Disabled | ButtonState::Loading) {
            st::color_alpha(base, 100)
        } else {
            base
        }
    }

    fn bg(&self, variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        let tint = match variant {
            Variant::Danger => t.bear(),
            Variant::Ghost | Variant::Secondary => t.text(),
            _ => t.accent(),
        };
        let alpha = match state {
            ButtonState::Disabled | ButtonState::Loading => st::alpha_faint(),
            ButtonState::Idle     => st::alpha_soft(),
            ButtonState::Hover    => st::alpha_subtle(),
            ButtonState::Active | ButtonState::Pressed => st::alpha_tint(),
        };
        st::color_alpha(tint, alpha)
    }

    fn border(&self, _variant: Variant, _state: ButtonState) -> Color32 {
        Color32::TRANSPARENT
    }
}

// ── ElevatedButtonStyle ───────────────────────────────────────────────────────

/// Card-style elevated button — ghost fill + accent border, "raised" feel.
///
/// Combines a surface-lifted background (`alpha_ghost` fill from the variant
/// color) with a visible border so the button reads as an interactive card.
/// On hover the fill steps up to `alpha_soft` and the border brightens, giving
/// a smooth elevation feel without a drop shadow. Best for primary CTAs inside
/// card layouts, settings panels, and feature grids where the button needs
/// visual weight without a flat filled pill look.
pub struct ElevatedButtonStyle<'a> {
    theme: &'a dyn ComponentTheme,
}

impl<'a> ElevatedButtonStyle<'a> {
    /// Wrap a `ComponentTheme` reference.
    pub fn new(theme: &'a dyn ComponentTheme) -> Self {
        Self { theme }
    }
}

impl<'a> ButtonStyle for ElevatedButtonStyle<'a> {
    fn fg(&self, variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        let base = match variant {
            Variant::Danger => t.bear(),
            Variant::Ghost | Variant::Secondary => t.text(),
            _ => t.accent(),
        };
        if matches!(state, ButtonState::Disabled | ButtonState::Loading) {
            st::color_alpha(base, 90)
        } else {
            base
        }
    }

    fn bg(&self, variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        let tint = match variant {
            Variant::Danger => t.bear(),
            Variant::Ghost | Variant::Secondary => t.surface(),
            _ => t.accent(),
        };
        let alpha = match state {
            ButtonState::Disabled | ButtonState::Loading => st::alpha_faint(),
            ButtonState::Idle     => st::alpha_ghost(),
            ButtonState::Hover    => st::alpha_soft(),
            ButtonState::Active | ButtonState::Pressed => st::alpha_subtle(),
        };
        match variant {
            Variant::Ghost | Variant::Secondary => {
                // Surface-lifted fill — use the surface color directly, not tinted.
                let lift: i16 = match state {
                    ButtonState::Idle     => 12,
                    ButtonState::Hover    => 22,
                    ButtonState::Active | ButtonState::Pressed => 32,
                    ButtonState::Disabled | ButtonState::Loading => 0,
                };
                let clamp = |c: i16| c.clamp(0, 255) as u8;
                Color32::from_rgb(
                    clamp(tint.r() as i16 + lift),
                    clamp(tint.g() as i16 + lift),
                    clamp(tint.b() as i16 + lift),
                )
            }
            _ => st::color_alpha(tint, alpha),
        }
    }

    fn border(&self, variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        let base = match variant {
            Variant::Danger => t.bear(),
            Variant::Ghost | Variant::Secondary => t.border_variant(),
            _ => t.accent(),
        };
        let alpha = match state {
            ButtonState::Disabled | ButtonState::Loading => st::alpha_soft(),
            ButtonState::Idle     => st::alpha_muted(),
            ButtonState::Hover    => st::alpha_strong(),
            ButtonState::Active | ButtonState::Pressed => 255,
        };
        st::color_alpha(base, alpha)
    }
}

// ── MutedButtonStyle ──────────────────────────────────────────────────────────

/// Deliberately understated — all variants render in the theme's dim palette.
///
/// Foreground uses `theme.dim()`, background is transparent (ghost fill on
/// hover only), border uses `theme.border()`. The visual result is a button
/// that reads as "inactive-looking but still clickable" — ideal for secondary
/// list actions (expand, details, copy) that should not compete with the
/// primary action nearby. All semantic variants (Primary, Danger, Ghost)
/// collapse to the same muted visual; no color differentiation is intentional.
pub struct MutedButtonStyle<'a> {
    theme: &'a dyn ComponentTheme,
}

impl<'a> MutedButtonStyle<'a> {
    /// Wrap a `ComponentTheme` reference.
    pub fn new(theme: &'a dyn ComponentTheme) -> Self {
        Self { theme }
    }
}

impl<'a> ButtonStyle for MutedButtonStyle<'a> {
    fn fg(&self, _variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        match state {
            ButtonState::Disabled | ButtonState::Loading => st::color_alpha(t.dim(), 80),
            ButtonState::Hover | ButtonState::Active | ButtonState::Pressed => t.text(),
            ButtonState::Idle => t.dim(),
        }
    }

    fn bg(&self, _variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        match state {
            ButtonState::Hover    => st::color_alpha(t.text(), 12),
            ButtonState::Active | ButtonState::Pressed => st::color_alpha(t.text(), 20),
            _ => Color32::TRANSPARENT,
        }
    }

    fn border(&self, _variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        match state {
            ButtonState::Disabled | ButtonState::Loading => st::color_alpha(t.border(), 80),
            ButtonState::Active | ButtonState::Pressed => t.border_variant(),
            _ => t.border(),
        }
    }
}

// ── AccentEmphasisStyle ───────────────────────────────────────────────────────

/// Every variant renders as accent — creates a focused, cohesive button group.
///
/// Ignores the per-button `Variant` entirely and renders all buttons with the
/// theme's accent color: solid accent fill, contrast foreground, no border.
/// Useful when you have a horizontal row of buttons (e.g. quick-action strips,
/// onboarding step CTAs, tutorial nav) that all belong to the same focused
/// group and should visually read as a unified set. On hover the fill
/// lightens slightly; on press it darkens so the click still registers.
pub struct AccentEmphasisStyle<'a> {
    theme: &'a dyn ComponentTheme,
}

impl<'a> AccentEmphasisStyle<'a> {
    /// Wrap a `ComponentTheme` reference.
    pub fn new(theme: &'a dyn ComponentTheme) -> Self {
        Self { theme }
    }
}

impl<'a> ButtonStyle for AccentEmphasisStyle<'a> {
    fn fg(&self, _variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        let base = st::contrast_fg(t.accent());
        if matches!(state, ButtonState::Disabled | ButtonState::Loading) {
            st::color_alpha(base, 120)
        } else {
            base
        }
    }

    fn bg(&self, _variant: Variant, state: ButtonState) -> Color32 {
        // Same Sx accent ramp as DefaultButtonStyle's filled variants — one
        // shared source of "lighter on hover, darker on press".
        let pal = palette_ct(self.theme);
        match state {
            ButtonState::Disabled | ButtonState::Loading => st::color_alpha(pal.base(Tone::Accent), 90),
            ButtonState::Idle     => pal.base(Tone::Accent),
            ButtonState::Hover    => pal.shade(Tone::Accent, Shade::S400),
            ButtonState::Active | ButtonState::Pressed => pal.shade(Tone::Accent, Shade::S600),
        }
    }

    fn border(&self, _variant: Variant, _state: ButtonState) -> Color32 {
        Color32::TRANSPARENT
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_kit::widgets::theme::PortableTheme;

    #[test]
    fn default_button_style_primary_idle_fg_is_contrast() {
        let theme = PortableTheme::dark();
        let style = DefaultButtonStyle::new(&theme);
        // Primary/Idle fg must be readable over the accent fill — should be
        // white or near-white on a dark accent.
        let fg = style.fg(Variant::Primary, ButtonState::Idle);
        // Contrast fg over a blue accent in dark mode must be light.
        assert!(
            fg.r() as u16 + fg.g() as u16 + fg.b() as u16 > 400,
            "Primary/Idle fg should be light over dark accent, got {:?}", fg
        );
    }

    #[test]
    fn default_button_style_ghost_idle_bg_is_transparent() {
        let theme = PortableTheme::dark();
        let style = DefaultButtonStyle::new(&theme);
        let bg = style.bg(Variant::Ghost, ButtonState::Idle);
        assert_eq!(bg.a(), 0, "Ghost/Idle bg must be fully transparent");
    }

    #[test]
    fn default_button_style_ghost_hover_bg_is_visible() {
        let theme = PortableTheme::dark();
        let style = DefaultButtonStyle::new(&theme);
        let bg = style.bg(Variant::Ghost, ButtonState::Hover);
        assert!(bg.a() > 0, "Ghost/Hover bg must have some alpha");
    }

    #[test]
    fn default_button_style_secondary_has_border_idle() {
        let theme = PortableTheme::dark();
        let style = DefaultButtonStyle::new(&theme);
        let border = style.border(Variant::Secondary, ButtonState::Idle);
        assert!(border.a() > 0, "Secondary/Idle border should be visible");
    }

    #[test]
    fn default_button_style_primary_border_is_transparent() {
        let theme = PortableTheme::dark();
        let style = DefaultButtonStyle::new(&theme);
        let border = style.border(Variant::Primary, ButtonState::Idle);
        assert_eq!(border.a(), 0, "Primary/Idle border should be transparent");
    }

    #[test]
    fn button_state_eq() {
        assert_eq!(ButtonState::Idle, ButtonState::Idle);
        assert_ne!(ButtonState::Idle, ButtonState::Hover);
    }

    /// Sanity: DefaultButtonStyle::new compiles for a concrete PortableTheme ref.
    #[test]
    fn adapter_compiles_for_portable_theme() {
        let theme = PortableTheme::light();
        let style = DefaultButtonStyle::new(&theme);
        // Exercise all three methods for all states to catch exhaustiveness gaps.
        for state in [
            ButtonState::Idle,
            ButtonState::Hover,
            ButtonState::Active,
            ButtonState::Pressed,
            ButtonState::Disabled,
            ButtonState::Loading,
        ] {
            let _ = style.fg(Variant::Primary, state);
            let _ = style.bg(Variant::Primary, state);
            let _ = style.border(Variant::Primary, state);
        }
    }
}
