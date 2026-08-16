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
    /// Dynamic tint — Ghost-like (transparent until hover) but uses the
    /// caller-supplied `.tint(color)` for fg, hover overlay, and border.
    /// Designed to absorb the ~15 `Variant::Chrome + .fg(dynamic_color)`
    /// escape-hatch sites identified by the variant-completeness audit
    /// (option-type colour, link-group colour, connection-state colour
    /// buttons). Behaves like Ghost when no `.tint()` is set.
    ///
    /// ## UNUSED — and the migration it was built for does not work
    ///
    /// Zero call sites, and that is not neglect: the premise is wrong. The
    /// audit assumed those sites needed only a dynamic FOREGROUND. Every one
    /// actually sets a dynamic `.fill()` too — they are filled badges (the
    /// option-type chip in the watchlist, the hotkey key cap, the session
    /// colour swatch), not transparent-until-hover affordances. Migrating them
    /// here would erase their background.
    ///
    /// `Tag` does not take them either: it is toned semantically
    /// (`TagTone::Normal | Muted | Success | Warning`) and these carry
    /// arbitrary per-item colours.
    ///
    /// So `Variant::Chrome + .fg() + .fill()` is the CORRECT call there, not an
    /// escape hatch to be closed. The real gap is that no widget expresses
    /// "badge with an arbitrary colour"; until one exists, leave those sites
    /// alone. Recorded here so the migration is not attempted a third time.
    DynamicTint,
}

/// Canonical 5-tier size scale used across the design system.
///
/// `Xl` was added 2026-05 (P2.4 consolidation) to absorb the chart-shell
/// `foundation::tokens::Size::Xl` which was the only difference between the
/// two parallel enums. Use this enum for every widget AND every shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Size {
    Xs,
    Sm,
    #[default]
    Md,
    Lg,
    Xl,
}

impl Size {
    /// Maps to the typography scale (font_xs/sm/md/lg/xl from style.rs).
    pub fn font_size(&self) -> f32 {
        match self {
            Size::Xs => crate::ui_kit::tokens::font_xs(),
            Size::Sm => crate::ui_kit::tokens::font_sm(),
            // Md uses sm typography by default — buttons aren't titles.
            Size::Md => crate::ui_kit::tokens::font_sm(),
            Size::Lg => crate::ui_kit::tokens::font_md(),
            Size::Xl => crate::ui_kit::tokens::font_xl(),
        }
    }

    /// Maps to the spacing grid (gap_2xs/xs/sm/md/lg from style.rs).
    pub fn padding_x(&self) -> f32 {
        match self {
            Size::Xs => crate::ui_kit::tokens::gap_2xs(),
            Size::Sm => crate::ui_kit::tokens::gap_xs(),
            Size::Md => crate::ui_kit::tokens::gap_sm(),
            Size::Lg => crate::ui_kit::tokens::gap_md(),
            Size::Xl => crate::ui_kit::tokens::gap_2xl(),
        }
    }

    /// Symmetric padding as an `egui::Margin`. Used by shells that need
    /// both x and y inner padding (e.g. RowShell, CardShell).
    pub fn padding(&self) -> egui::Margin {
        let (x, y) = match self {
            Size::Xs => (crate::ui_kit::tokens::gap_sm(), crate::ui_kit::tokens::gap_xs()),
            Size::Sm => (crate::ui_kit::tokens::gap_md(), crate::ui_kit::tokens::gap_xs()),
            Size::Md => (crate::ui_kit::tokens::gap_md(), crate::ui_kit::tokens::gap_sm()),
            Size::Lg => (crate::ui_kit::tokens::gap_lg(), crate::ui_kit::tokens::gap_md()),
            Size::Xl => (crate::ui_kit::tokens::gap_2xl(), crate::ui_kit::tokens::gap_lg()),
        };
        egui::Margin { left: x as i8, right: x as i8, top: y as i8, bottom: y as i8 }
    }

    /// Stable slug for audit ids, so a check can grade a control against the
    /// rung its caller actually ASKED for rather than one global minimum.
    pub fn slug(&self) -> &'static str {
        match self {
            Size::Xs => "xs",
            Size::Sm => "sm",
            Size::Md => "md",
            Size::Lg => "lg",
            Size::Xl => "xl",
        }
    }

    /// Maps to the control-height ladder (`Density.control_xs..control_xl`).
    ///
    /// This returned frozen literals — `18/22/28/34/40` — while `font_size()`,
    /// `padding_x()` and `padding()` directly above all read live tokens. One
    /// struct, two philosophies: a theme could change a control's type and its
    /// padding but never its HEIGHT, which is the axis the eye actually reads
    /// as consistency. It was also the largest remaining frozen surface in the
    /// design system — every `Size`-based control in ~74 primitives.
    ///
    /// The ladder's defaults are exactly these former literals, so nothing
    /// moves until a style authors otherwise.
    pub fn height(&self) -> f32 {
        match self {
            Size::Xs => crate::ui_kit::style::control_h_xs(),
            Size::Sm => crate::ui_kit::style::control_h_sm(),
            Size::Md => crate::ui_kit::style::control_h_md(),
            Size::Lg => crate::ui_kit::style::control_h_lg(),
            Size::Xl => crate::ui_kit::style::control_h_xl(),
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

/// Per-style-preset button visual treatment. Selects which of 5 styling
/// strategies the legacy `simple_btn()` path applies.
///
/// Owned by ui_kit (was in chart_renderer::ui::style; moved in P5b extraction
/// Step 1 — the audit's HARD blocker). The chart-app's StyleSettings still
/// has a `button_treatment: ButtonTreatment` field but the enum definition
/// is now portable.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ButtonTreatment {
    #[default]
    SoftPill,
    OutlineAccent,
    UnderlineActive,
    RaisedActive,
    BlackFillActive,
}
