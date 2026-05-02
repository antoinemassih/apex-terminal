//! Builder + impl Widget primitives — text family.
//! See ui/widgets/mod.rs for the rationale.
//!
//! Each struct wraps an existing legacy helper from `components.rs` /
//! `components_extra.rs`. The legacy helpers still coexist — no call sites
//! are migrated here.
//!
//! # Remaining call-site sweep candidates
//! The following patterns appear 5+ times and should be migrated in a later pass:
//!
//! **`SectionLabel::new(…).md().color(t.accent)`** — 17+ raw
//!   `RichText::new(…).monospace().size(font_md()).strong().color(t.accent)` usages.
//!   Known files: `plays_panel.rs`, `options_panel.rs`, `scanner_panel.rs`,
//!   `trade_plan_panel.rs`, `intel_panel.rs`, `risk_panel.rs`.
//!
//! **`CategoryHeader::new(…).color(t.dim)`** — nav/tree headers.
//!   Known files: `watchlist_panel.rs`, `scanner_panel.rs`, `intel_panel.rs`.
//!
//! **`border_stroke(t)` / `border_stroke_thin(t)`** — raw `Stroke::new(stroke_std(), t.toolbar_border)`.
//!   Known files: `watchlist_panel.rs`, `dom_action.rs`, `overlay_manager.rs`.

#![allow(dead_code, unused_imports)]

use egui::{Color32, FontFamily, Response, RichText, Ui, Widget};
use super::super::style::*;
use super::foundation::text_style::TextStyle;

// Re-export size enums so callers only need to import from this module.
pub use super::super::components::{MonoSize, NumericSize};

// ─── Escape-hatch overrides ───────────────────────────────────────────────────

/// Per-widget overrides that let any raw `RichText` pattern be expressed via
/// the typed widget builders. All fields are `None` by default (= no override).
#[derive(Default, Clone, Copy)]
struct Overrides {
    size:      Option<f32>,
    strong:    Option<bool>,
    italics:   Option<bool>,
    monospace: Option<bool>,
    gamma:     Option<f32>,
}

impl Overrides {
    /// Apply accumulated overrides to a `RichText` + resolved color.
    fn apply(self, mut rt: RichText, mut color: Color32) -> (RichText, Color32) {
        if let Some(s) = self.size      { rt = rt.size(s); }
        if let Some(b) = self.strong    { if b { rt = rt.strong(); } }
        if let Some(i) = self.italics   { if i { rt = rt.italics(); } }
        if let Some(m) = self.monospace {
            if m { rt = rt.monospace(); }
            else { rt = rt.family(FontFamily::Proportional); }
        }
        if let Some(g) = self.gamma     { color = color.gamma_multiply(g); }
        (rt, color)
    }
}

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
    overrides: Overrides,
}

impl<'a> PaneTitle<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: Color32::from_rgb(200, 200, 210), overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
    pub fn size(mut self, px: f32) -> Self { self.overrides.size = Some(px); self }
    pub fn strong(mut self, v: bool) -> Self { self.overrides.strong = Some(v); self }
    pub fn italics(mut self, v: bool) -> Self { self.overrides.italics = Some(v); self }
    pub fn monospace(mut self, v: bool) -> Self { self.overrides.monospace = Some(v); self }
    pub fn gamma(mut self, g: f32) -> Self { self.overrides.gamma = Some(g); self }
}

impl<'a> Widget for PaneTitle<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let rt = TextStyle::HeadingMd.as_rich(self.text, self.color);
        let (rt, color) = self.overrides.apply(rt, self.color);
        ui.label(rt.color(color))
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
    overrides: Overrides,
}

impl<'a> Subheader<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: Color32::from_rgb(150, 150, 160), overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
    pub fn size(mut self, px: f32) -> Self { self.overrides.size = Some(px); self }
    pub fn strong(mut self, v: bool) -> Self { self.overrides.strong = Some(v); self }
    pub fn italics(mut self, v: bool) -> Self { self.overrides.italics = Some(v); self }
    pub fn monospace(mut self, v: bool) -> Self { self.overrides.monospace = Some(v); self }
    pub fn gamma(mut self, g: f32) -> Self { self.overrides.gamma = Some(g); self }
}

impl<'a> Widget for Subheader<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let rt = TextStyle::Eyebrow.as_rich(&style_label_case(self.text), self.color);
        let (rt, color) = self.overrides.apply(rt, self.color);
        ui.label(rt.color(color))
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
    overrides: Overrides,
}

impl<'a> BodyLabel<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: Color32::from_rgb(200, 200, 210), overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
    pub fn size(mut self, px: f32) -> Self { self.overrides.size = Some(px); self }
    pub fn strong(mut self, v: bool) -> Self { self.overrides.strong = Some(v); self }
    pub fn italics(mut self, v: bool) -> Self { self.overrides.italics = Some(v); self }
    pub fn monospace(mut self, v: bool) -> Self { self.overrides.monospace = Some(v); self }
    pub fn gamma(mut self, g: f32) -> Self { self.overrides.gamma = Some(g); self }
}

impl<'a> Widget for BodyLabel<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let rt = TextStyle::Body.as_rich(self.text, self.color);
        let (rt, color) = self.overrides.apply(rt, self.color);
        ui.label(rt.color(color))
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
    overrides: Overrides,
}

impl<'a> MutedLabel<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, base_color: Color32::from_rgb(150, 150, 160), overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.base_color = c; self }
    pub fn size(mut self, px: f32) -> Self { self.overrides.size = Some(px); self }
    pub fn strong(mut self, v: bool) -> Self { self.overrides.strong = Some(v); self }
    pub fn italics(mut self, v: bool) -> Self { self.overrides.italics = Some(v); self }
    pub fn monospace(mut self, v: bool) -> Self { self.overrides.monospace = Some(v); self }
    pub fn gamma(mut self, g: f32) -> Self { self.overrides.gamma = Some(g); self }
}

impl<'a> Widget for MutedLabel<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let c = color_alpha(self.base_color, alpha_muted());
        let rt = TextStyle::BodySm.as_rich(self.text, c);
        let (rt, color) = self.overrides.apply(rt, c);
        ui.label(rt.color(color))
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
    overrides: Overrides,
}

impl<'a> CaptionLabel<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, dim: Color32::from_rgb(120, 120, 130), overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.dim = c; self }
    pub fn size(mut self, px: f32) -> Self { self.overrides.size = Some(px); self }
    pub fn strong(mut self, v: bool) -> Self { self.overrides.strong = Some(v); self }
    pub fn italics(mut self, v: bool) -> Self { self.overrides.italics = Some(v); self }
    pub fn monospace(mut self, v: bool) -> Self { self.overrides.monospace = Some(v); self }
    pub fn gamma(mut self, g: f32) -> Self { self.overrides.gamma = Some(g); self }
}

impl<'a> Widget for CaptionLabel<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let c = color_alpha(self.dim, alpha_dim());
        let rt = TextStyle::Caption.as_rich(self.text, c);
        let (rt, color) = self.overrides.apply(rt, c);
        ui.label(rt.color(color))
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
    overrides: Overrides,
}

impl<'a> MonospaceCode<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: Color32::from_rgb(200, 200, 210), size: MonoSize::Sm, overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
    pub fn size(mut self, s: MonoSize) -> Self { self.size = s; self }
    pub fn xs(mut self) -> Self { self.size = MonoSize::Xs; self }
    pub fn sm(mut self) -> Self { self.size = MonoSize::Sm; self }
    pub fn md(mut self) -> Self { self.size = MonoSize::Md; self }
    /// Override pixel size (escape-hatch; use `.size(MonoSize::*)` for semantic sizes).
    pub fn size_px(mut self, px: f32) -> Self { self.overrides.size = Some(px); self }
    pub fn strong(mut self, v: bool) -> Self { self.overrides.strong = Some(v); self }
    pub fn italics(mut self, v: bool) -> Self { self.overrides.italics = Some(v); self }
    pub fn monospace(mut self, v: bool) -> Self { self.overrides.monospace = Some(v); self }
    pub fn gamma(mut self, g: f32) -> Self { self.overrides.gamma = Some(g); self }
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
        let (rt, color) = self.overrides.apply(rt, self.color);
        ui.label(rt.color(color))
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
    overrides: Overrides,
}

impl<'a> NumericDisplay<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: Color32::from_rgb(200, 200, 210), size: NumericSize::Lg, overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
    pub fn size(mut self, s: NumericSize) -> Self { self.size = s; self }
    pub fn lg(mut self) -> Self { self.size = NumericSize::Lg; self }
    pub fn xl(mut self) -> Self { self.size = NumericSize::Xl; self }
    pub fn hero(mut self) -> Self { self.size = NumericSize::Hero; self }
    /// Override pixel size (escape-hatch; use `.size(NumericSize::*)` for semantic sizes).
    pub fn size_px(mut self, px: f32) -> Self { self.overrides.size = Some(px); self }
    pub fn strong(mut self, v: bool) -> Self { self.overrides.strong = Some(v); self }
    pub fn italics(mut self, v: bool) -> Self { self.overrides.italics = Some(v); self }
    pub fn monospace(mut self, v: bool) -> Self { self.overrides.monospace = Some(v); self }
    pub fn gamma(mut self, g: f32) -> Self { self.overrides.gamma = Some(g); self }
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
        let (rt, color) = self.overrides.apply(rt, self.color);
        ui.label(rt.color(color))
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
    overrides: Overrides,
}

impl<'a> SectionLabel<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: Color32::from_rgb(150, 150, 160), size: SectionLabelSize::Sm, overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
    pub fn size(mut self, s: SectionLabelSize) -> Self { self.size = s; self }
    pub fn tiny(mut self) -> Self { self.size = SectionLabelSize::Tiny; self }
    pub fn xs(mut self) -> Self { self.size = SectionLabelSize::Xs; self }
    pub fn md(mut self) -> Self { self.size = SectionLabelSize::Md; self }
    pub fn lg(mut self) -> Self { self.size = SectionLabelSize::Lg; self }
    /// Override pixel size (escape-hatch; use `.size(SectionLabelSize::*)` for semantic sizes).
    pub fn size_px(mut self, px: f32) -> Self { self.overrides.size = Some(px); self }
    pub fn strong(mut self, v: bool) -> Self { self.overrides.strong = Some(v); self }
    pub fn italics(mut self, v: bool) -> Self { self.overrides.italics = Some(v); self }
    pub fn monospace(mut self, v: bool) -> Self { self.overrides.monospace = Some(v); self }
    pub fn gamma(mut self, g: f32) -> Self { self.overrides.gamma = Some(g); self }
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
        let (rt, color) = self.overrides.apply(rt, self.color);
        ui.label(rt.color(color))
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
    overrides: Overrides,
}

impl<'a> DimLabel<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: Color32::from_rgb(150, 150, 160), overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
    pub fn size(mut self, px: f32) -> Self { self.overrides.size = Some(px); self }
    pub fn strong(mut self, v: bool) -> Self { self.overrides.strong = Some(v); self }
    pub fn italics(mut self, v: bool) -> Self { self.overrides.italics = Some(v); self }
    pub fn monospace(mut self, v: bool) -> Self { self.overrides.monospace = Some(v); self }
    pub fn gamma(mut self, g: f32) -> Self { self.overrides.gamma = Some(g); self }
}

impl<'a> Widget for DimLabel<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let rt = TextStyle::BodySm.as_rich(self.text, self.color);
        let (rt, color) = self.overrides.apply(rt, self.color);
        ui.label(rt.color(color))
    }
}

// ─── CategoryHeader ───────────────────────────────────────────────────────────

/// Builder for a nav/tree categorical section header.
///
/// Renders `RichText::new(text).monospace().size(font_xs()).color(color)` — the standard
/// dim divider label used in watchlist groups, scanner categories, and tree nav headers.
///
/// ```ignore
/// ui.add(CategoryHeader::new("WATCHLIST").color(t.dim));
/// ui.add(CategoryHeader::new("TECHNICALS").color(t.accent));
/// ```
#[must_use = "CategoryHeader must be added with `ui.add(...)` to render"]
pub struct CategoryHeader<'a> {
    text: &'a str,
    color: Color32,
    overrides: Overrides,
}

impl<'a> CategoryHeader<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: Color32::from_rgb(120, 120, 130), overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
    pub fn size(mut self, px: f32) -> Self { self.overrides.size = Some(px); self }
    pub fn strong(mut self, v: bool) -> Self { self.overrides.strong = Some(v); self }
    pub fn italics(mut self, v: bool) -> Self { self.overrides.italics = Some(v); self }
    pub fn monospace(mut self, v: bool) -> Self { self.overrides.monospace = Some(v); self }
    pub fn gamma(mut self, g: f32) -> Self { self.overrides.gamma = Some(g); self }
}

impl<'a> Widget for CategoryHeader<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let rt = RichText::new(self.text).monospace().size(font_xs()).color(self.color);
        let (rt, color) = self.overrides.apply(rt, self.color);
        ui.label(rt.color(color))
    }
}
