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

// ── Declarative color vocabulary (the recipe layer) ───────────────────────────
//
// `Col` is the alphabet a button variant is *declared* in — each variant states
// its fg/bg/border per state as DATA (`Col`), and the single `resolve()` below
// turns a `Col` into a concrete `Color32` against the theme palette. This is the
// cva/shadcn idea adapted to immediate-mode: variants are declarations, not
// imperative color math. New treatments are new `Col` tables, not new structs.

/// A declarative color source, resolved against the palette at paint time.
#[derive(Clone, Copy)]
pub(crate) enum Col {
    /// Fully transparent.
    None,
    /// The tone's base (500) color.
    Base(Tone),
    /// A genuine ramp shade of the tone (lighter/darker step).
    Shade(Tone, Shade),
    /// The tone's base at an explicit alpha (the `color_alpha` tint).
    Alpha(Tone, u8),
    /// A readable foreground that contrasts against the tone's base fill.
    Contrast(Tone),
    /// `color_half` of the tone's base.
    Half(Tone),
    /// `color_subtle` of the tone's base.
    Subtle(Tone),
    /// The tone's base lifted toward white by `pct`/100 (neutral surface nudge).
    Lighten(Tone, u8),
}

/// Lift a color toward white by `amt` (0..1), preserving alpha. Shared by
/// `Col::Lighten` — the one non-ramp lightening (a neutral-surface nudge).
fn lighten(c: Color32, amt: f32) -> Color32 {
    let lerp = |x: u8| -> u8 {
        let v = x as f32 + (255.0 - x as f32) * amt.clamp(0.0, 1.0);
        v.round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgba_premultiplied(lerp(c.r()), lerp(c.g()), lerp(c.b()), c.a())
}

/// Halve a color's alpha (the disabled/loading dim). Applied to fg+bg, not border.
fn dim_disabled(c: Color32) -> Color32 {
    Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * 0.5).round() as u8)
}

/// Resolve a declarative [`Col`] to a concrete color against the theme palette.
pub(crate) fn resolve(col: Col, t: &dyn ComponentTheme) -> Color32 {
    let pal = palette_ct(t);
    match col {
        Col::None => Color32::TRANSPARENT,
        Col::Base(tone) => pal.base(tone),
        Col::Shade(tone, s) => pal.shade(tone, s),
        Col::Alpha(tone, a) => st::color_alpha(pal.base(tone), a),
        Col::Contrast(tone) => st::contrast_fg(pal.base(tone)),
        Col::Half(tone) => st::color_half(pal.base(tone)),
        Col::Subtle(tone) => st::color_subtle(pal.base(tone)),
        Col::Lighten(tone, pct) => lighten(pal.base(tone), pct as f32 / 100.0),
    }
}

// ── DefaultButtonStyle recipe ─────────────────────────────────────────────────
// Each function below is the declarative variant table for one channel. Read
// top-to-bottom it's the whole button design in `Col` terms — no color math.

use Col::*;
use ButtonState::{Idle, Hover, Active, Pressed, Disabled, Loading};
use Variant as V;

/// Foreground (text/icon) spec per variant × state.
fn default_fg(v: Variant, s: ButtonState) -> Col {
    match v {
        V::Primary => Contrast(Tone::Accent),
        V::Danger  => Contrast(Tone::Bear),
        // DynamicTint shares the Ghost default; its real color comes from the
        // call-site `.tint()` pipeline in button.rs.
        V::Ghost | V::Chrome | V::TextOnly | V::DynamicTint | V::Secondary => Base(Tone::Text),
        V::Link => match s { Hover => Alpha(Tone::Accent, 230), _ => Base(Tone::Accent) },
        V::Chip | V::Toggle => match s {
            Idle => Half(Tone::Text),
            Hover => Base(Tone::Text),
            Active | Pressed => Base(Tone::Accent),
            Disabled | Loading => Half(Tone::Dim),
        },
        V::Tab => match s { Active => Base(Tone::Text), _ => Alpha(Tone::Dim, 178) },
        V::InlineClose => match s { Hover | Pressed => Base(Tone::Text), _ => Subtle(Tone::Dim) },
        V::MutedIcon => match s { Hover | Pressed => Base(Tone::Text), _ => Half(Tone::Dim) },
        V::NeutralAction => Contrast(Tone::Surface),
    }
}

/// Background fill spec per variant × state. Filled variants step the ramp
/// (S500 idle → S400 hover → S600 press).
fn default_bg(v: Variant, s: ButtonState) -> Col {
    match v {
        V::Primary => match s {
            Hover => Shade(Tone::Accent, Shade::S400),
            Active | Pressed => Shade(Tone::Accent, Shade::S600),
            _ => Base(Tone::Accent),
        },
        V::Danger => match s {
            Hover => Shade(Tone::Bear, Shade::S400),
            Active | Pressed => Shade(Tone::Bear, Shade::S600),
            _ => Base(Tone::Bear),
        },
        V::Secondary => match s {
            Hover => Lighten(Tone::Surface, 8),
            Active | Pressed => Alpha(Tone::Accent, st::alpha_tint()),
            _ => Base(Tone::Surface),
        },
        V::Ghost => match s {
            Hover => Alpha(Tone::Text, 18),
            Active | Pressed => Alpha(Tone::Accent, st::alpha_soft()),
            _ => None,
        },
        V::Chip | V::Toggle => match s {
            Hover => Alpha(Tone::Text, 18),
            Active | Pressed => Alpha(Tone::Accent, st::alpha_tint()),
            _ => None,
        },
        V::MutedIcon => match s { Hover | Pressed => Alpha(Tone::Text, 18), _ => None },
        V::NeutralAction => match s {
            Idle => Alpha(Tone::Text, 28),
            Hover => Alpha(Tone::Text, 44),
            _ => Alpha(Tone::Text, 18), // Active|Pressed|Disabled|Loading
        },
        V::Link | V::Tab | V::TextOnly | V::Chrome | V::InlineClose | V::DynamicTint => None,
    }
}

/// Border spec per variant × state.
fn default_border(v: Variant, s: ButtonState) -> Col {
    match v {
        V::Secondary | V::Toggle => match s {
            Active | Pressed => Alpha(Tone::Accent, st::alpha_active()),
            _ => Base(Tone::Border),
        },
        V::NeutralAction => Base(Tone::Border),
        _ => None,
    }
}

impl<'a> ButtonStyle for DefaultButtonStyle<'a> {
    fn fg(&self, variant: Variant, state: ButtonState) -> Color32 {
        let c = resolve(default_fg(variant, state), self.theme);
        if matches!(state, Disabled | Loading) { dim_disabled(c) } else { c }
    }

    fn bg(&self, variant: Variant, state: ButtonState) -> Color32 {
        let c = resolve(default_bg(variant, state), self.theme);
        if matches!(state, Disabled | Loading) { dim_disabled(c) } else { c }
    }

    fn border(&self, variant: Variant, state: ButtonState) -> Color32 {
        resolve(default_border(variant, state), self.theme)
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
            Variant::Danger => palette_ct(t).base(Tone::Bear),
            Variant::Ghost | Variant::Secondary => palette_ct(t).base(Tone::Dim),
            _ => palette_ct(t).base(Tone::Accent),
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
                    Variant::Danger => palette_ct(t).base(Tone::Bear),
                    _ => palette_ct(t).base(Tone::Accent),
                };
                st::color_alpha(tint, st::alpha_faint())
            }
            ButtonState::Active | ButtonState::Pressed => {
                let tint = match variant {
                    Variant::Danger => palette_ct(t).base(Tone::Bear),
                    _ => palette_ct(t).base(Tone::Accent),
                };
                st::color_alpha(tint, st::alpha_ghost())
            }
            _ => Color32::TRANSPARENT,
        }
    }

    fn border(&self, variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        let base = match variant {
            Variant::Danger => palette_ct(t).base(Tone::Bear),
            Variant::Ghost | Variant::Secondary => palette_ct(t).base(Tone::Border),
            _ => palette_ct(t).base(Tone::Accent),
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
            Variant::Danger => palette_ct(t).base(Tone::Bear),
            Variant::Ghost | Variant::Secondary => palette_ct(t).base(Tone::Text),
            _ => palette_ct(t).base(Tone::Accent),
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
            Variant::Danger => palette_ct(t).base(Tone::Bear),
            Variant::Ghost | Variant::Secondary => palette_ct(t).base(Tone::Text),
            _ => palette_ct(t).base(Tone::Accent),
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
            Variant::Danger => palette_ct(t).base(Tone::Bear),
            Variant::Ghost | Variant::Secondary => palette_ct(t).base(Tone::Text),
            _ => palette_ct(t).base(Tone::Accent),
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
            Variant::Danger => palette_ct(t).base(Tone::Bear),
            Variant::Ghost | Variant::Secondary => palette_ct(t).base(Tone::Surface),
            _ => palette_ct(t).base(Tone::Accent),
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
            Variant::Danger => palette_ct(t).base(Tone::Bear),
            Variant::Ghost | Variant::Secondary => t.border_variant(),
            _ => palette_ct(t).base(Tone::Accent),
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
/// Foreground uses `palette_ct(theme).base(Tone::Dim)`, background is transparent (ghost fill on
/// hover only), border uses `palette_ct(theme).base(Tone::Border)`. The visual result is a button
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
            ButtonState::Disabled | ButtonState::Loading => st::color_alpha(palette_ct(t).base(Tone::Dim), 80),
            ButtonState::Hover | ButtonState::Active | ButtonState::Pressed => palette_ct(t).base(Tone::Text),
            ButtonState::Idle => palette_ct(t).base(Tone::Dim),
        }
    }

    fn bg(&self, _variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        match state {
            ButtonState::Hover    => st::color_alpha(palette_ct(t).base(Tone::Text), 12),
            ButtonState::Active | ButtonState::Pressed => st::color_alpha(palette_ct(t).base(Tone::Text), 20),
            _ => Color32::TRANSPARENT,
        }
    }

    fn border(&self, _variant: Variant, state: ButtonState) -> Color32 {
        let t = self.theme;
        match state {
            ButtonState::Disabled | ButtonState::Loading => st::color_alpha(palette_ct(t).base(Tone::Border), 80),
            ButtonState::Active | ButtonState::Pressed => t.border_variant(),
            _ => palette_ct(t).base(Tone::Border),
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
        let base = st::contrast_fg(palette_ct(t).base(Tone::Accent));
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

    /// Characterization digest — pins the EXACT fg/bg/border bytes of every
    /// ButtonStyle across all 14 variants × 6 states on both dark and light
    /// themes (≈3000 channel values folded with FNV-1a). Any change to button
    /// styling output flips the digest.
    ///
    /// This is the byte-identity net for the DS#1 recipe refactor: build the
    /// declarative recipe, route the styles through it, and keep this green to
    /// prove the look is unchanged across every variant/state/theme.
    #[test]
    fn button_styles_characterization_digest() {
        use crate::ui_kit::widgets::tokens::Variant::*;
        let variants = [
            Primary, Secondary, Ghost, Danger, Link, Chrome, Chip, Tab,
            InlineClose, MutedIcon, NeutralAction, TextOnly, Toggle, DynamicTint,
        ];
        let states = [
            ButtonState::Idle, ButtonState::Hover, ButtonState::Active,
            ButtonState::Pressed, ButtonState::Disabled, ButtonState::Loading,
        ];
        let fold = |mut acc: u64, c: Color32| -> u64 {
            for b in [c.r(), c.g(), c.b(), c.a()] {
                acc = (acc ^ b as u64).wrapping_mul(1099511628211);
            }
            acc
        };
        let mut digest: u64 = 1469598103934665603; // FNV offset basis
        for theme in [PortableTheme::dark(), PortableTheme::light()] {
            let d = DefaultButtonStyle::new(&theme);
            let o = OutlinedButtonStyle::new(&theme);
            let s = SoftButtonStyle::new(&theme);
            let e = ElevatedButtonStyle::new(&theme);
            let m = MutedButtonStyle::new(&theme);
            let a = AccentEmphasisStyle::new(&theme);
            let styles: [&dyn ButtonStyle; 6] = [&d, &o, &s, &e, &m, &a];
            for style in styles {
                for &v in &variants {
                    for &st in &states {
                        digest = fold(digest, style.fg(v, st));
                        digest = fold(digest, style.bg(v, st));
                        digest = fold(digest, style.border(v, st));
                    }
                }
            }
        }
        assert_eq!(
            digest, BUTTON_STYLE_DIGEST,
            "button styling output changed (digest {digest}). If intentional, \
             update BUTTON_STYLE_DIGEST; if this is the DS#1 recipe refactor, \
             the recipe is NOT byte-identical yet."
        );
    }

    // Pinned digest of all ButtonStyle output. Update only on an intentional
    // visual change to buttons.
    const BUTTON_STYLE_DIGEST: u64 = 14977099151003970929;
}
