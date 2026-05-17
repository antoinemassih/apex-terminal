//! Shared widget tokens. Used by every component for variant + size
//! consistency. If a widget needs a value outside these enums, raise a
//! flag — the answer is almost always "use a Variant/Size we already
//! have."

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Variant {
    #[default]
    Primary,    // accent-filled, strong CTA
    Secondary,  // border-only or surface-filled
    Ghost,      // transparent until hover
    Danger,     // bear-colored, destructive
    Link,       // text-only, underline on hover
    Chrome,     // fully overridable; caller sets fill/stroke/min_size/corner_radius
    // ── 2026-05 additions to absorb the 75 Chrome escape-hatch sites ─────────
    /// Toggle chip — small selectable pill. Active = accent fg + soft accent
    /// fill; inactive = dim*0.5 fg + transparent fill. Used in segmented
    /// controls, filter pills, preset selectors. Set `.active(bool)`.
    Chip,
    /// Tab — frameless, transparent fill, accent fg when `.active(true)`,
    /// `dim*0.5` otherwise. For nav strips like Stocks/Options, panel tabs.
    Tab,
    /// Inline close — small icon-only `Icon::X` style with dim*0.7 glyph,
    /// frameless, 18px square. For modal headers, chip dismiss, floating panes.
    InlineClose,
    /// Muted icon — Ghost variant with `dim*0.5` glyph color. For secondary
    /// row actions (edit, delete) that should not steal focus.
    MutedIcon,
    /// Neutral action — Secondary with a gray fill (170,170,170) and BLACK fg.
    /// For utility actions like FLATTEN that aren't destructive but aren't
    /// primary either.
    NeutralAction,
    /// Text-only — Chrome + transparent fill + frameless. For inline link-like
    /// affordances that shouldn't render as a button. Caller sets `.fg()`.
    TextOnly,
    /// Toggle chip — one of a row of mutually-exclusive (or independently)
    /// selectable chips. Inactive = transparent bg + dim outline + text-color
    /// fg (alpha-soft); Active = accent-tinted bg + accent fg + accent border
    /// (alpha-active). Hover blends toward active styling. Used for style /
    /// font-scale / session-tint preset chips in settings panels. Construct
    /// via [`super::button::Button::toggle`].
    Toggle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Size {
    Xs,
    Sm,
    #[default]
    Md,
    Lg,
}

impl Size {
    /// Maps to the typography scale (font_xs/sm/md/lg from style.rs).
    pub fn font_size(&self) -> f32 {
        match self {
            Size::Xs => crate::chart_renderer::ui::style::font_xs(),
            Size::Sm => crate::chart_renderer::ui::style::font_sm(),
            // Md uses sm typography by default — buttons aren't titles.
            Size::Md => crate::chart_renderer::ui::style::font_sm(),
            Size::Lg => crate::chart_renderer::ui::style::font_md(),
        }
    }

    /// Maps to the spacing grid (gap_2xs/xs/sm/md from style.rs).
    pub fn padding_x(&self) -> f32 {
        match self {
            Size::Xs => crate::chart_renderer::ui::style::gap_2xs(),
            Size::Sm => crate::chart_renderer::ui::style::gap_xs(),
            Size::Md => crate::chart_renderer::ui::style::gap_sm(),
            Size::Lg => crate::chart_renderer::ui::style::gap_md(),
        }
    }

    pub fn height(&self) -> f32 {
        match self {
            Size::Xs => 18.0,
            Size::Sm => 22.0,
            Size::Md => 28.0,
            Size::Lg => 34.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct State {
    pub hovered: bool,
    pub active: bool,    // toggled-on state (tabs, switches, pinned items)
    pub pressed: bool,
    pub disabled: bool,
    pub focused: bool,
}
