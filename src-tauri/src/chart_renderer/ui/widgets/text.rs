//! Builder + impl Widget primitives — text family.
//! See ui/widgets/mod.rs for the rationale.
//!
//! Each struct wraps an existing legacy helper from `components.rs` /
//! `components_extra.rs`. The legacy helpers still coexist — no call sites
//! are migrated here.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, RichText, Ui, Widget};
use super::super::style::*;
use super::foundation::text_style::TextStyle;

// Re-export size enums so callers only need to import from this module.
pub use super::super::components::{MonoSize, NumericSize};

// ─── PaneTitle ────────────────────────────────────────────────────────────────

/// Builder for a pane heading. Replaces `components::pane_title(ui, text, color)`.
///
/// ```ignore
/// ui.add(PaneTitle::new("Watchlist").color(theme.fg));
/// ```
#[must_use = "PaneTitle must be added with `ui.add(...)` to render"]
pub struct PaneTitle<'a> {
    text: &'a str,
    color: Color32,
}

impl<'a> PaneTitle<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: Color32::from_rgb(200, 200, 210) }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
}

impl<'a> Widget for PaneTitle<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.label(TextStyle::HeadingMd.as_rich(self.text, self.color))
    }
}

// ─── Subheader ────────────────────────────────────────────────────────────────

/// Builder for a sub-section heading. Replaces `components::subheader(ui, text, color)`.
///
/// ```ignore
/// ui.add(Subheader::new("Greeks").color(theme.dim));
/// ```
#[must_use = "Subheader must be added with `ui.add(...)` to render"]
pub struct Subheader<'a> {
    text: &'a str,
    color: Color32,
}

impl<'a> Subheader<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: Color32::from_rgb(150, 150, 160) }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
}

impl<'a> Widget for Subheader<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.label(TextStyle::Eyebrow.as_rich(&style_label_case(self.text), self.color))
    }
}

// ─── BodyLabel ────────────────────────────────────────────────────────────────

/// Builder for default body text. Replaces `components::body_label(ui, text, color)`.
///
/// ```ignore
/// ui.add(BodyLabel::new("No positions open.").color(theme.fg));
/// ```
#[must_use = "BodyLabel must be added with `ui.add(...)` to render"]
pub struct BodyLabel<'a> {
    text: &'a str,
    color: Color32,
}

impl<'a> BodyLabel<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: Color32::from_rgb(200, 200, 210) }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
}

impl<'a> Widget for BodyLabel<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.label(TextStyle::Body.as_rich(self.text, self.color))
    }
}

// ─── MutedLabel ───────────────────────────────────────────────────────────────

/// Builder for secondary / dim text. Replaces `components::muted_label(ui, text, base_color)`.
///
/// `base_color` has `alpha_muted()` applied internally, matching the legacy helper.
///
/// ```ignore
/// ui.add(MutedLabel::new("Last updated 3m ago").color(theme.dim));
/// ```
#[must_use = "MutedLabel must be added with `ui.add(...)` to render"]
pub struct MutedLabel<'a> {
    text: &'a str,
    base_color: Color32,
}

impl<'a> MutedLabel<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, base_color: Color32::from_rgb(150, 150, 160) }
    }
    pub fn color(mut self, c: Color32) -> Self { self.base_color = c; self }
}

impl<'a> Widget for MutedLabel<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let c = color_alpha(self.base_color, alpha_muted());
        ui.label(TextStyle::BodySm.as_rich(self.text, c))
    }
}

// ─── CaptionLabel ─────────────────────────────────────────────────────────────

/// Builder for secondary caption text. Replaces `components_extra::caption_label(ui, text, dim)`.
///
/// Applies `alpha_dim()` internally, matching the legacy helper.
///
/// ```ignore
/// ui.add(CaptionLabel::new("https://example.com").color(theme.dim));
/// ```
#[must_use = "CaptionLabel must be added with `ui.add(...)` to render"]
pub struct CaptionLabel<'a> {
    text: &'a str,
    dim: Color32,
}

impl<'a> CaptionLabel<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, dim: Color32::from_rgb(120, 120, 130) }
    }
    pub fn color(mut self, c: Color32) -> Self { self.dim = c; self }
}

impl<'a> Widget for CaptionLabel<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.label(TextStyle::Caption.as_rich(self.text, color_alpha(self.dim, alpha_dim())))
    }
}

// ─── MonospaceCode ────────────────────────────────────────────────────────────

/// Builder for monospace code / ticker / price text at a chosen size.
/// Replaces `components::monospace_code(ui, text, size, color)`.
///
/// Convenience shortcuts: `.xs()` / `.sm()` / `.md()`.
///
/// ```ignore
/// ui.add(MonospaceCode::new("AAPL").sm().color(theme.accent));
/// ui.add(MonospaceCode::new("0.00").size(MonoSize::Xs).color(theme.dim));
/// ```
#[must_use = "MonospaceCode must be added with `ui.add(...)` to render"]
pub struct MonospaceCode<'a> {
    text: &'a str,
    color: Color32,
    size: MonoSize,
}

impl<'a> MonospaceCode<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: Color32::from_rgb(200, 200, 210), size: MonoSize::Sm }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
    pub fn size(mut self, s: MonoSize) -> Self { self.size = s; self }
    pub fn xs(mut self) -> Self { self.size = MonoSize::Xs; self }
    pub fn sm(mut self) -> Self { self.size = MonoSize::Sm; self }
    pub fn md(mut self) -> Self { self.size = MonoSize::Md; self }
}

impl<'a> Widget for MonospaceCode<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let style = match self.size {
            MonoSize::Xs => TextStyle::MonoSm,  // font_sm — Xs widget intent maps to small mono
            MonoSize::Sm => TextStyle::MonoSm,
            MonoSize::Md => TextStyle::Mono,
        };
        // For MonoSize::Xs preserve original font_xs by overriding size.
        let mut rt = style.as_rich(self.text, self.color);
        if matches!(self.size, MonoSize::Xs) {
            rt = RichText::new(self.text).monospace().size(font_xs()).color(self.color);
        }
        ui.label(rt)
    }
}

// ─── NumericDisplay ───────────────────────────────────────────────────────────

/// Builder for large numeric readouts (price, P&L, account values).
/// Replaces `components::numeric_display(ui, text, size, color)`.
///
/// Convenience shortcuts: `.lg()` / `.xl()` / `.hero()`.
///
/// ```ignore
/// ui.add(NumericDisplay::new("+$1,234.56").hero().color(theme.bull));
/// ```
#[must_use = "NumericDisplay must be added with `ui.add(...)` to render"]
pub struct NumericDisplay<'a> {
    text: &'a str,
    color: Color32,
    size: NumericSize,
}

impl<'a> NumericDisplay<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: Color32::from_rgb(200, 200, 210), size: NumericSize::Lg }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
    pub fn size(mut self, s: NumericSize) -> Self { self.size = s; self }
    pub fn lg(mut self) -> Self { self.size = NumericSize::Lg; self }
    pub fn xl(mut self) -> Self { self.size = NumericSize::Xl; self }
    pub fn hero(mut self) -> Self { self.size = NumericSize::Hero; self }
}

impl<'a> Widget for NumericDisplay<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let style = match self.size {
            NumericSize::Lg   => TextStyle::Numeric,
            NumericSize::Xl   => TextStyle::NumericLg,
            NumericSize::Hero => TextStyle::NumericHero,
        };
        // Lg variant rendered legacy at font_lg(); Numeric uses font_md(). Preserve.
        let mut rt = style.as_rich(self.text, self.color);
        if matches!(self.size, NumericSize::Lg) {
            rt = RichText::new(self.text).monospace().size(font_lg()).strong().color(self.color);
        }
        ui.label(rt)
    }
}

// ─── SectionLabel ─────────────────────────────────────────────────────────────

/// Size variant for [`SectionLabel`].
pub enum SectionLabelSize {
    /// `7.0` — matches legacy `style::section_label` (sub-`font_xs`).
    Tiny,
    /// `font_xs()` — tightest, for dense column headers.
    Xs,
    /// `font_sm()` — default (matches `section_label_widget`).
    Sm,
    /// `font_md()` — slightly emphasised group title.
    Md,
    /// `font_lg()` — large section divider.
    Lg,
}

/// Builder for a section label. Replaces `components::section_label_widget` and its
/// `_xs` / `_md` / `_lg` variants.
///
/// Applies `style_label_case()` (uppercase under Meridien) and `monospace().strong()`,
/// matching the legacy helpers exactly.
///
/// ```ignore
/// ui.add(SectionLabel::new("Greeks").color(theme.dim));
/// ui.add(SectionLabel::new("Positions").lg().color(theme.fg));
/// ```
#[must_use = "SectionLabel must be added with `ui.add(...)` to render"]
pub struct SectionLabel<'a> {
    text: &'a str,
    color: Color32,
    size: SectionLabelSize,
}

impl<'a> SectionLabel<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: Color32::from_rgb(150, 150, 160), size: SectionLabelSize::Sm }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
    pub fn size(mut self, s: SectionLabelSize) -> Self { self.size = s; self }
    pub fn tiny(mut self) -> Self { self.size = SectionLabelSize::Tiny; self }
    pub fn xs(mut self) -> Self { self.size = SectionLabelSize::Xs; self }
    pub fn md(mut self) -> Self { self.size = SectionLabelSize::Md; self }
    pub fn lg(mut self) -> Self { self.size = SectionLabelSize::Lg; self }
}

impl<'a> Widget for SectionLabel<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let s = style_label_case(self.text);
        // Sm maps to TextStyle::Label (font_sm + strong + non-monospace per mapping).
        // Other sizes preserve legacy monospace look via explicit RichText.
        let rt = match self.size {
            SectionLabelSize::Sm => TextStyle::Label.as_rich(&s, self.color),
            SectionLabelSize::Tiny => RichText::new(s).monospace().size(7.0).strong().color(self.color),
            SectionLabelSize::Xs   => RichText::new(s).monospace().size(font_xs()).strong().color(self.color),
            SectionLabelSize::Md   => RichText::new(s).monospace().size(font_md()).strong().color(self.color),
            SectionLabelSize::Lg   => RichText::new(s).monospace().size(font_lg()).strong().color(self.color),
        };
        ui.label(rt)
    }
}

// ─── section_label adapter ────────────────────────────────────────────────────

/// Free-fn adapter for the legacy `style::section_label(ui, text, color)`.
/// Calls `ui.add(SectionLabel::new(text).tiny().color(color))` — the legacy
/// helper renders at literal size `7.0` (sub-`font_xs`), so this uses the
/// `Tiny` variant to match byte-for-byte.
#[inline]
pub fn section_label(ui: &mut Ui, text: &str, color: Color32) -> Response {
    ui.add(SectionLabel::new(text).tiny().color(color))
}

// ─── DimLabel ─────────────────────────────────────────────────────────────────

/// Builder for a dim info label. Replaces `style::dim_label(ui, text, color)`.
///
/// Renders `RichText::new(text).monospace().size(font_sm()).color(color)` —
/// no alpha applied (the caller passes the dim color directly).
///
/// ```ignore
/// ui.add(DimLabel::new("Last updated").color(theme.dim));
/// ```
#[must_use = "DimLabel must be added with `ui.add(...)` to render"]
pub struct DimLabel<'a> {
    text: &'a str,
    color: Color32,
}

impl<'a> DimLabel<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: Color32::from_rgb(150, 150, 160) }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
}

impl<'a> Widget for DimLabel<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.label(TextStyle::BodySm.as_rich(self.text, self.color))
    }
}
