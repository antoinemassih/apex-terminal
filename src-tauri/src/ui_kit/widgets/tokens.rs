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
