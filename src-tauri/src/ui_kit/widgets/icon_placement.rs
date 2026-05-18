//! Placement-driven icon-button sizing and visibility resolution.
//!
//! Every icon button in the app picks a placement that determines its glyph size,
//! hit-target rect, hover treatment, and click-affordance. This kills the long-
//! standing drift between `icon_btn()` (one size scale), `Button::close()` (another),
//! `header_action_btn()` (a third), and the hand-painted painter.text() rows.
//!
//! Rule of thumb:
//! - Toolbar / Modal       — biggest, most visible, primary chrome
//! - PanelHeader / ChartChrome — panel-level affordances
//! - ListRow                — per-row delete/lock/visibility
//! - TabClose               — tightest, color-snap (no bg fill) due to tab spacing
//! - InlineLabel            — inside a labeled Button, follows the Button's size
//! - StatusIndicator        — non-interactive decoration, NOT a button
//!
//! Visibility rule (single source of truth for icon glyph color):
//! - Neutral idle:   t.dim (full opacity)
//! - Neutral hover:  t.text
//! - Neutral active: t.accent
//! - Destructive idle:  t.bear at alpha_soft()
//! - Destructive hover: t.bear (full)
//! - Affirmative idle:  t.bull at alpha_soft()
//! - Affirmative hover: t.bull (full)
//! - Disabled:          t.dim at alpha_muted(), no hover

// TODO follow-up wave: re-export from widgets/mod.rs after the 5-primitives
// agent (a2ab82f3...) lands — it's currently editing that file. Then:
//   pub use icon_placement::{IconPlacement, IconTone, IconState, icon_glyph_color, icon_hover_bg};

use egui::Color32;
use crate::chart::renderer::ui::style::{alpha_soft, alpha_muted, alpha_ghost, color_alpha};
use super::theme::ComponentTheme;

/// Placement context for an icon button. Determines glyph size, hit-target,
/// hover treatment, and interactivity semantics.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IconPlacement {
    /// Primary chrome: toolbars, top nav. 14px glyph / 24×24 hit-target, bg fill on hover.
    Toolbar,
    /// Panel-level: panel headers. 13px glyph / 20×20 hit-target, bg fill on hover.
    PanelHeader,
    /// Chart overlay affordances: zoom, crosshair, etc. 13px glyph / 20×20, bg fill on hover.
    ChartChrome,
    /// Modal / dialog chrome: modal close, sheet close. 14px glyph / 24×24, bg fill on hover.
    Modal,
    /// Per-row inline icons: delete, lock, visibility. 12px glyph / 16×16, bg fill on hover.
    ListRow,
    /// Tab close button. 11px glyph / 14×14, color-snap only — NO bg fill.
    TabClose,
    /// Icon embedded inside a labeled `Button`. 12px glyph / 0px hit (follows parent).
    InlineLabel,
    /// Non-interactive decoration. 12px glyph / 12×12, NOT interactive.
    StatusIndicator,
}

/// Semantic tone for an icon glyph. Drives idle / hover color resolution.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum IconTone {
    /// Standard chrome: `t.dim` → `t.text` → `t.accent`.
    #[default]
    Neutral,
    /// Delete / danger: `t.bear` at reduced alpha idle → full on hover.
    Destructive,
    /// Confirm / add: `t.bull` at reduced alpha idle → full on hover.
    Affirmative,
}

/// Interaction state for an icon glyph.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum IconState {
    /// Normal, pointer not over.
    #[default]
    Idle,
    /// Pointer is hovering over the hit-target.
    Hover,
    /// Pointer is pressed (click in progress).
    Active,
    /// Non-interactive: gates interaction, dims glyph.
    Disabled,
}

impl IconPlacement {
    /// Glyph point size (egui font size, in logical pixels).
    ///
    /// | Placement       | px |
    /// |-----------------|----|
    /// | Toolbar         | 14 |
    /// | PanelHeader     | 13 |
    /// | ChartChrome     | 13 |
    /// | Modal           | 14 |
    /// | ListRow         | 12 |
    /// | TabClose        | 11 |
    /// | InlineLabel     | 12 |
    /// | StatusIndicator | 12 |
    pub fn glyph_px(self) -> f32 {
        match self {
            Self::Toolbar         => 14.0,
            Self::PanelHeader     => 13.0,
            Self::ChartChrome     => 13.0,
            Self::Modal           => 14.0,
            Self::ListRow         => 12.0,
            Self::TabClose        => 11.0,
            Self::InlineLabel     => 12.0,
            Self::StatusIndicator => 12.0,
        }
    }

    /// Hit-target square side (logical pixels).
    ///
    /// `InlineLabel` returns `0.0` — the caller should follow the parent button's rect.
    ///
    /// | Placement       | px |
    /// |-----------------|----|
    /// | Toolbar         | 24 |
    /// | PanelHeader     | 20 |
    /// | ChartChrome     | 20 |
    /// | Modal           | 24 |
    /// | ListRow         | 16 |
    /// | TabClose        | 14 |
    /// | InlineLabel     |  0 |
    /// | StatusIndicator | 12 |
    pub fn hit_px(self) -> f32 {
        match self {
            Self::Toolbar         => 24.0,
            Self::PanelHeader     => 20.0,
            Self::ChartChrome     => 20.0,
            Self::Modal           => 24.0,
            Self::ListRow         => 16.0,
            Self::TabClose        => 14.0,
            Self::InlineLabel     =>  0.0,
            Self::StatusIndicator => 12.0,
        }
    }

    /// Whether this placement renders a background fill rect on hover.
    ///
    /// `TabClose` uses color-snap only (tight tab spacing means a bg fill is
    /// visually noisy). `StatusIndicator` and `InlineLabel` are not interactive
    /// buttons so they never show a bg.
    pub fn hover_bg(self) -> bool {
        !matches!(self, Self::TabClose | Self::StatusIndicator | Self::InlineLabel)
    }

    /// Whether this placement is interactive (responds to pointer events).
    ///
    /// Only `StatusIndicator` is non-interactive.
    pub fn interactive(self) -> bool {
        !matches!(self, Self::StatusIndicator)
    }
}

/// Single source of truth for icon glyph color resolution.
///
/// All icon-button widgets MUST funnel through this function.
/// No ad-hoc `gamma_multiply` / `color_subtle` calls in icon-button paint code.
///
/// # Arguments
/// * `t`     — active theme (from `&dyn ComponentTheme`)
/// * `tone`  — semantic role of the icon
/// * `state` — current interaction state
pub fn icon_glyph_color(t: &dyn ComponentTheme, tone: IconTone, state: IconState) -> Color32 {
    use IconTone::*;
    use IconState::*;
    match (tone, state) {
        (Neutral,     Idle)     => t.dim(),
        (Neutral,     Hover)    => t.text(),
        (Neutral,     Active)   => t.accent(),
        (Neutral,     Disabled) => color_alpha(t.dim(), alpha_muted()),
        (Destructive, Idle)     => color_alpha(t.bear(), alpha_soft()),
        (Destructive, Hover)    => t.bear(),
        (Destructive, Active)   => t.bear(),
        (Destructive, Disabled) => color_alpha(t.bear(), alpha_muted()),
        (Affirmative, Idle)     => color_alpha(t.bull(), alpha_soft()),
        (Affirmative, Hover)    => t.bull(),
        (Affirmative, Active)   => t.bull(),
        (Affirmative, Disabled) => color_alpha(t.bull(), alpha_muted()),
    }
}

/// Hover background color for icon buttons that opt in to bg-fill-on-hover.
///
/// Call only when `IconPlacement::hover_bg()` returns `true`. `TabClose` and
/// non-interactive placements skip this entirely.
pub fn icon_hover_bg(t: &dyn ComponentTheme) -> Color32 {
    color_alpha(t.text(), alpha_ghost())
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Color32;

    /// Minimal test double for ComponentTheme.
    struct TestTheme;

    impl ComponentTheme for TestTheme {
        fn accent(&self) -> Color32 { Color32::from_rgb(100, 150, 255) }
        fn bull(&self)   -> Color32 { Color32::from_rgb(0, 200, 100) }
        fn bear(&self)   -> Color32 { Color32::from_rgb(230, 60, 60) }
        fn text(&self)   -> Color32 { Color32::from_rgb(230, 230, 230) }
        fn dim(&self)    -> Color32 { Color32::from_rgb(120, 120, 130) }
        fn border(&self) -> Color32 { Color32::from_rgb(60, 60, 70) }
        fn border_variant(&self) -> Color32 { Color32::from_rgb(70, 70, 80) }
        fn warn(&self)   -> Color32 { Color32::from_rgb(255, 180, 0) }
        fn bg(&self)     -> Color32 { Color32::from_rgb(18, 18, 22) }
        fn surface(&self)-> Color32 { Color32::from_rgb(28, 28, 36) }
        fn element_hover(&self)    -> Color32 { Color32::from_rgba_unmultiplied(230, 230, 230, 12) }
        fn element_active(&self)   -> Color32 { Color32::from_rgba_unmultiplied(230, 230, 230, 20) }
        fn element_selected(&self) -> Color32 { Color32::from_rgba_unmultiplied(230, 230, 230, 16) }
        fn element_disabled(&self) -> Color32 { Color32::from_rgba_unmultiplied(230, 230, 230, 6) }
        fn ghost_hover(&self)      -> Color32 { Color32::from_rgba_unmultiplied(230, 230, 230, 15) }
        fn ghost_active(&self)     -> Color32 { Color32::from_rgba_unmultiplied(230, 230, 230, 24) }
        fn icon(&self)             -> Color32 { Color32::from_rgb(180, 180, 190) }
        fn icon_muted(&self)       -> Color32 { Color32::from_rgb(100, 100, 110) }
        fn icon_disabled(&self)    -> Color32 { Color32::from_rgba_unmultiplied(180, 180, 190, 80) }
        fn icon_accent(&self)      -> Color32 { Color32::from_rgb(100, 150, 255) }
    }

    fn t() -> TestTheme { TestTheme }

    /// Every (tone, state) combination must return a non-fully-transparent color.
    /// We check alpha > 0 — a zero-alpha result would mean the icon is invisible.
    #[test]
    fn all_tone_state_combinations_have_nonzero_alpha() {
        let tones  = [IconTone::Neutral, IconTone::Destructive, IconTone::Affirmative];
        let states = [IconState::Idle, IconState::Hover, IconState::Active, IconState::Disabled];
        for tone in tones {
            for state in states {
                let c = icon_glyph_color(&t(), tone, state);
                assert!(
                    c.a() > 0,
                    "icon_glyph_color({tone:?}, {state:?}) returned fully transparent",
                );
            }
        }
    }

    /// Hover glyph must be more visible (higher alpha or brighter) than idle.
    /// For Neutral: hover = t.text (opaque), idle = t.dim (opaque).
    #[test]
    fn neutral_hover_brighter_than_idle() {
        let idle  = icon_glyph_color(&t(), IconTone::Neutral, IconState::Idle);
        let hover = icon_glyph_color(&t(), IconTone::Neutral, IconState::Hover);
        // text is brighter / more saturated than dim — compare luminance proxy.
        let lum = |c: Color32| c.r() as u32 + c.g() as u32 + c.b() as u32;
        assert!(lum(hover) >= lum(idle), "hover should be >= idle brightness");
    }

    /// Destructive idle must be softer than destructive hover.
    #[test]
    fn destructive_idle_softer_than_hover() {
        let idle  = icon_glyph_color(&t(), IconTone::Destructive, IconState::Idle);
        let hover = icon_glyph_color(&t(), IconTone::Destructive, IconState::Hover);
        assert!(idle.a() < hover.a(), "destructive idle alpha should be < hover alpha");
    }

    /// Affirmative idle must be softer than affirmative hover.
    #[test]
    fn affirmative_idle_softer_than_hover() {
        let idle  = icon_glyph_color(&t(), IconTone::Affirmative, IconState::Idle);
        let hover = icon_glyph_color(&t(), IconTone::Affirmative, IconState::Hover);
        assert!(idle.a() < hover.a(), "affirmative idle alpha should be < hover alpha");
    }

    /// Active state for Neutral should use the accent color.
    #[test]
    fn neutral_active_uses_accent() {
        let active = icon_glyph_color(&t(), IconTone::Neutral, IconState::Active);
        assert_eq!(active, t().accent());
    }

    /// Disabled state for all tones must be less visible than idle.
    #[test]
    fn disabled_less_visible_than_idle_for_all_tones() {
        let tones = [IconTone::Neutral, IconTone::Destructive, IconTone::Affirmative];
        for tone in tones {
            let idle     = icon_glyph_color(&t(), tone, IconState::Idle);
            let disabled = icon_glyph_color(&t(), tone, IconState::Disabled);
            assert!(
                disabled.a() <= idle.a(),
                "{tone:?} disabled.a()={} should be <= idle.a()={}",
                disabled.a(), idle.a(),
            );
        }
    }

    /// hover_bg must be non-fully-transparent.
    #[test]
    fn hover_bg_has_nonzero_alpha() {
        let bg = icon_hover_bg(&t());
        assert!(bg.a() > 0, "icon_hover_bg returned fully transparent");
    }

    // ── IconPlacement property tests ──────────────────────────────────────────

    /// StatusIndicator is the only non-interactive placement.
    #[test]
    fn only_status_indicator_is_non_interactive() {
        let non_interactive: Vec<_> = [
            IconPlacement::Toolbar,
            IconPlacement::PanelHeader,
            IconPlacement::ChartChrome,
            IconPlacement::Modal,
            IconPlacement::ListRow,
            IconPlacement::TabClose,
            IconPlacement::InlineLabel,
            IconPlacement::StatusIndicator,
        ]
        .iter()
        .filter(|p| !p.interactive())
        .collect();
        assert_eq!(non_interactive.len(), 1);
        assert_eq!(*non_interactive[0], IconPlacement::StatusIndicator);
    }

    /// TabClose, StatusIndicator, and InlineLabel must not show bg fill.
    #[test]
    fn tab_close_status_inline_have_no_hover_bg() {
        assert!(!IconPlacement::TabClose.hover_bg());
        assert!(!IconPlacement::StatusIndicator.hover_bg());
        assert!(!IconPlacement::InlineLabel.hover_bg());
    }

    /// All interactive placements except TabClose/InlineLabel must show hover bg.
    #[test]
    fn toolbar_panelheader_chartchrome_modal_listrow_have_hover_bg() {
        assert!(IconPlacement::Toolbar.hover_bg());
        assert!(IconPlacement::PanelHeader.hover_bg());
        assert!(IconPlacement::ChartChrome.hover_bg());
        assert!(IconPlacement::Modal.hover_bg());
        assert!(IconPlacement::ListRow.hover_bg());
    }

    /// InlineLabel must have hit_px == 0 (caller follows parent).
    #[test]
    fn inline_label_has_zero_hit_px() {
        assert_eq!(IconPlacement::InlineLabel.hit_px(), 0.0);
    }

    /// Glyph sizes must be positive for all placements.
    #[test]
    fn all_glyph_sizes_are_positive() {
        let placements = [
            IconPlacement::Toolbar,
            IconPlacement::PanelHeader,
            IconPlacement::ChartChrome,
            IconPlacement::Modal,
            IconPlacement::ListRow,
            IconPlacement::TabClose,
            IconPlacement::InlineLabel,
            IconPlacement::StatusIndicator,
        ];
        for p in placements {
            assert!(p.glyph_px() > 0.0, "{p:?}.glyph_px() must be > 0");
        }
    }

    /// Hit targets must be non-negative (InlineLabel is 0.0, others are > 0).
    #[test]
    fn all_hit_targets_are_nonnegative() {
        let placements = [
            IconPlacement::Toolbar,
            IconPlacement::PanelHeader,
            IconPlacement::ChartChrome,
            IconPlacement::Modal,
            IconPlacement::ListRow,
            IconPlacement::TabClose,
            IconPlacement::InlineLabel,
            IconPlacement::StatusIndicator,
        ];
        for p in placements {
            assert!(p.hit_px() >= 0.0, "{p:?}.hit_px() must be >= 0");
        }
    }
}
