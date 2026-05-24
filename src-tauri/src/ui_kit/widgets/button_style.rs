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
            Variant::Ghost | Variant::Chrome | Variant::TextOnly => t.text(),
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
                ButtonState::Disabled => half(t.dim()),
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
            Variant::NeutralAction => Color32::BLACK,
        };

        if state == ButtonState::Disabled {
            disabled_alpha(base)
        } else {
            base
        }
    }

    fn bg(&self, variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        let transparent = Color32::TRANSPARENT;
        let disabled_alpha = |c: Color32| -> Color32 {
            Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(),
                (c.a() as f32 * 0.5).round() as u8)
        };
        // Inline helpers to avoid reaching outside this crate boundary.
        let lighten = |c: Color32, amt: f32| -> Color32 {
            let lerp = |x: u8| -> u8 {
                let v = x as f32 + (255.0 - x as f32) * amt.clamp(0.0, 1.0);
                v.round().clamp(0.0, 255.0) as u8
            };
            Color32::from_rgba_premultiplied(lerp(c.r()), lerp(c.g()), lerp(c.b()), c.a())
        };
        let darken = |c: Color32, amt: f32| -> Color32 {
            let f = (1.0 - amt).clamp(0.0, 1.0);
            Color32::from_rgba_premultiplied(
                (c.r() as f32 * f) as u8,
                (c.g() as f32 * f) as u8,
                (c.b() as f32 * f) as u8,
                c.a(),
            )
        };

        let base = match variant {
            Variant::Primary => match state {
                ButtonState::Idle   => t.accent(),
                ButtonState::Hover  => lighten(t.accent(), 0.10),
                ButtonState::Active | ButtonState::Pressed => darken(t.accent(), 0.08),
                ButtonState::Disabled => t.accent(),
            },
            Variant::Secondary => match state {
                ButtonState::Idle  => t.surface(),
                ButtonState::Hover => lighten(t.surface(), 0.08),
                ButtonState::Active | ButtonState::Pressed =>
                    st::color_alpha(t.accent(), st::alpha_tint()),
                ButtonState::Disabled => t.surface(),
            },
            Variant::Ghost => match state {
                ButtonState::Idle  => transparent,
                ButtonState::Hover => st::color_alpha(t.text(), 18),
                ButtonState::Active | ButtonState::Pressed =>
                    st::color_alpha(t.accent(), st::alpha_soft()),
                ButtonState::Disabled => transparent,
            },
            Variant::Danger => match state {
                ButtonState::Idle   => t.bear(),
                ButtonState::Hover  => lighten(t.bear(), 0.10),
                ButtonState::Active | ButtonState::Pressed => darken(t.bear(), 0.08),
                ButtonState::Disabled => t.bear(),
            },
            Variant::Link | Variant::Tab | Variant::TextOnly |
            Variant::Chrome | Variant::InlineClose => transparent,
            Variant::MutedIcon => match state {
                ButtonState::Hover | ButtonState::Pressed => st::color_alpha(t.text(), 18),
                _ => transparent,
            },
            Variant::Chip | Variant::Toggle => match state {
                ButtonState::Idle  => transparent,
                ButtonState::Hover => st::color_alpha(t.text(), 18),
                ButtonState::Active | ButtonState::Pressed =>
                    st::color_alpha(t.accent(), st::alpha_tint()),
                ButtonState::Disabled => transparent,
            },
            Variant::NeutralAction => match state {
                ButtonState::Idle    => Color32::from_gray(170),
                ButtonState::Hover   => Color32::from_gray(190),
                ButtonState::Active | ButtonState::Pressed => Color32::from_gray(150),
                ButtonState::Disabled => Color32::from_gray(170),
            },
        };

        if state == ButtonState::Disabled {
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
        ] {
            let _ = style.fg(Variant::Primary, state);
            let _ = style.bg(Variant::Primary, state);
            let _ = style.border(Variant::Primary, state);
        }
    }
}
