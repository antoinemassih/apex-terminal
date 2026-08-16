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
use crate::chart::renderer::ui::foundation::text_style::TextStyle;
#[inline(always)]
fn ambient(ctx: &egui::Context) -> super::super::super::gpu::Theme {
    crate::chart_renderer::theme_impl::active_theme(ctx)
}

// ─── Cascade plumbing ────────────────────────────────────────────────────────
//
// These widgets used to bake a `font_*()` constant into every `RichText`. They
// now take their size from the tier table that `TextStyle::install` writes into
// `Style::text_styles` each frame, so a subtree can retune a whole family with
//   ui.style_mut().text_styles.insert(TextStyle::Body.egui(), smaller);
// and these ~235 call sites follow without being touched.

/// Cascade-aware `RichText` for `tier`, safe on hosts without the table.
///
/// `TextStyle::as_rich_cascading` reaches for `RichText::text_style(..)`, and
/// egui's `TextStyle::resolve` **panics** when the `Name` key is missing. Any
/// host that spins up a bare `egui::Context` and never runs `setup_theme`
/// (`chart/renderer/inspector_window.rs`, `foundation/design_inspector.rs`)
/// has no `apex.*` keys at all, so probe the inherited table first — exactly
/// what `TextStyle::font_id_in` does — and bake a `FontId` only as the
/// fallback.
fn tier_rich(tier: TextStyle, ui: &Ui, text: &str, color: Color32) -> RichText {
    let spec = tier.spec();
    let key = tier.egui();
    let mut rt = RichText::new(text).color(color);
    rt = if ui.style().text_styles.contains_key(&key) {
        rt.text_style(key)                 // true cascade: resolved at layout time
    } else {
        rt.font(tier.font_id())            // no table installed — fall back safely
    };
    if spec.line_height_factor > 0.0 {
        rt = rt.line_height(Some(spec.size * spec.line_height_factor));
    }
    if spec.strong { rt = rt.strong(); }
    rt
}

/// [`tier_rich`] with the monospace family forced, keeping the tier's size.
///
/// `RichText::monospace()` cannot be used for this: in egui it is literally
/// `text_style(TextStyle::Monospace)`, i.e. it *replaces* the tier and drops the
/// cascade. `family()` only overrides the family, leaving the resolved size in
/// place.
fn tier_rich_mono(tier: TextStyle, ui: &Ui, text: &str, color: Color32) -> RichText {
    tier_rich(tier, ui, text, color).family(FontFamily::Monospace)
}

// ─── Size enums (moved from labels.rs) ───────────────────────────────────────

/// Size variants for [`MonospaceCode`].
///
/// Each variant names a tier, not a pixel size — see the `Widget` impl.
pub enum MonoSize {
    /// [`TextStyle::Caption`] (mono) — column headers, supplemental info.
    Xs,
    /// [`TextStyle::MonoSm`] — default mono text.
    Sm,
    /// [`TextStyle::BodyLg`] (mono) — emphasized mono.
    Md,
}

/// Size variants for numeric readouts.
pub enum NumericSize {
    /// `font_lg()` — compact price / change readout.
    Lg,
    /// `font_xl()` — secondary headline.
    Xl,
    /// 30 px — hero display (portfolio total, primary price).
    Hero,
}

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
            // `RichText::monospace()` is `text_style(TextStyle::Monospace)` in
            // egui — it would replace the tier and throw away the cascaded
            // size. `family()` overrides only the family, so an explicit
            // `.monospace(true)` from a call site still inherits its size.
            rt = rt.family(if m { FontFamily::Monospace } else { FontFamily::Proportional });
        }
        if let Some(g) = self.gamma     { color = color.gamma_multiply(g); }
        (rt, color)
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
    color: Option<Color32>,
    overrides: Overrides,
}

impl<'a> Subheader<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: None, overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self }
    pub fn size(mut self, px: f32) -> Self { self.overrides.size = Some(px); self }
    pub fn strong(mut self, v: bool) -> Self { self.overrides.strong = Some(v); self }
    pub fn italics(mut self, v: bool) -> Self { self.overrides.italics = Some(v); self }
    pub fn monospace(mut self, v: bool) -> Self { self.overrides.monospace = Some(v); self }
    pub fn gamma(mut self, g: f32) -> Self { self.overrides.gamma = Some(g); self }
}

impl<'a> Widget for Subheader<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let resolved = self.color.unwrap_or_else(|| {
            // No colour at this call site: ask the ancestors before
            // falling back to the tier's own tone. An explicit
            // `.color()` above still wins — see the precedence tests
            // in `ui_kit/widgets/label.rs`.
            crate::ui_kit::cascade::resolved().color_or(ambient(ui.ctx()).dim)
        });
        let rt = tier_rich(TextStyle::Eyebrow, ui, &style_label_case(self.text), resolved);
        let (rt, color) = self.overrides.apply(rt, resolved);
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
    color: Option<Color32>,
    overrides: Overrides,
}

impl<'a> BodyLabel<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: None, overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self }
    pub fn size(mut self, px: f32) -> Self { self.overrides.size = Some(px); self }
    pub fn strong(mut self, v: bool) -> Self { self.overrides.strong = Some(v); self }
    pub fn italics(mut self, v: bool) -> Self { self.overrides.italics = Some(v); self }
    pub fn monospace(mut self, v: bool) -> Self { self.overrides.monospace = Some(v); self }
    pub fn gamma(mut self, g: f32) -> Self { self.overrides.gamma = Some(g); self }
}

impl<'a> Widget for BodyLabel<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let resolved = self.color.unwrap_or_else(|| {
            // No colour at this call site: ask the ancestors before
            // falling back to the tier's own tone. An explicit
            // `.color()` above still wins — see the precedence tests
            // in `ui_kit/widgets/label.rs`.
            crate::ui_kit::cascade::resolved().color_or(ambient(ui.ctx()).text)
        });
        let rt = tier_rich(TextStyle::Body, ui, self.text, resolved);
        let (rt, color) = self.overrides.apply(rt, resolved);
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
    dim: Option<Color32>,
    overrides: Overrides,
}

impl<'a> CaptionLabel<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, dim: None, overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.dim = Some(c); self }
    pub fn size(mut self, px: f32) -> Self { self.overrides.size = Some(px); self }
    pub fn strong(mut self, v: bool) -> Self { self.overrides.strong = Some(v); self }
    pub fn italics(mut self, v: bool) -> Self { self.overrides.italics = Some(v); self }
    pub fn monospace(mut self, v: bool) -> Self { self.overrides.monospace = Some(v); self }
    pub fn gamma(mut self, g: f32) -> Self { self.overrides.gamma = Some(g); self }
}

impl<'a> Widget for CaptionLabel<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let base = self.dim.unwrap_or(ambient(ui.ctx()).dim);
        let c = color_alpha(base, alpha_dim());
        let rt = tier_rich(TextStyle::Caption, ui, self.text, c);
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
    color: Option<Color32>,
    size: MonoSize,
    overrides: Overrides,
}

impl<'a> MonospaceCode<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: None, size: MonoSize::Sm, overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self }
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
        let resolved = self.color.unwrap_or_else(|| {
            // No colour at this call site: ask the ancestors before
            // falling back to the tier's own tone. An explicit
            // `.color()` above still wins — see the precedence tests
            // in `ui_kit/widgets/label.rs`.
            crate::ui_kit::cascade::resolved().color_or(ambient(ui.ctx()).text)
        });
        // Sizes come from the tier table; the family is forced monospace because
        // Caption/BodyLg are proportional tiers borrowed purely for their step
        // on the scale. The three tiers are strictly ordered under every
        // StyleSystem (font_caption ≤ 9 < font_sm 12 < font_lg 16), unlike
        // Mono (= st.font_body, 10–13) which can overtake MonoSm and invert the
        // xs/sm/md ladder.
        let tier = match self.size {
            MonoSize::Xs => TextStyle::Caption,  // legacy font_xs 10 → font_caption 9
            MonoSize::Sm => TextStyle::MonoSm,   // legacy font_sm 12 → font_sm 12
            MonoSize::Md => TextStyle::BodyLg,   // legacy font_md 14 → font_lg 16
        };
        let rt = tier_rich_mono(tier, ui, self.text, resolved);
        let (rt, color) = self.overrides.apply(rt, resolved);
        ui.label(rt.color(color))
    }
}

// ─── SectionLabel ─────────────────────────────────────────────────────────────

/// Size variant for [`SectionLabel`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionLabelSize {
    /// [`TextStyle::Caption`] — matches legacy `style::section_label` (`font_2xs`).
    Tiny,
    /// [`TextStyle::Caption`] — tightest, for dense column headers.
    Xs,
    /// [`TextStyle::Label`] — default (matches `section_label_widget`).
    Sm,
    /// [`TextStyle::MonoSm`] — slightly emphasised group title.
    Md,
    /// [`TextStyle::BodyLg`] — large section divider.
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
    color: Option<Color32>,
    size: SectionLabelSize,
    overrides: Overrides,
}

impl<'a> SectionLabel<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: None, size: SectionLabelSize::Sm, overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self }
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
        let resolved = self.color.unwrap_or_else(|| {
            // No colour at this call site: ask the ancestors before
            // falling back to the tier's own tone. An explicit
            // `.color()` above still wins — see the precedence tests
            // in `ui_kit/widgets/label.rs`.
            crate::ui_kit::cascade::resolved().color_or(ambient(ui.ctx()).dim)
        });
        let s = style_label_case(self.text);
        // Every variant now takes its size from a tier; the monospace family and
        // `.strong()` (the legacy look) are re-applied on top, since only the
        // Label tier is `strong` and none of these tiers is monospace.
        // Resulting ladder is monotonic under every StyleSystem:
        //   Caption(8–9) ≤ Label(8–11) ≤ MonoSm(12) < BodyLg(16).
        let tier = match self.size {
            SectionLabelSize::Tiny => TextStyle::Caption,  // legacy font_2xs 9 → font_caption 9
            SectionLabelSize::Xs   => TextStyle::Caption,  // legacy font_xs 10 → font_caption 9
            SectionLabelSize::Sm   => TextStyle::Label,    // legacy font_sm 12 → font_section_label 10
            SectionLabelSize::Md   => TextStyle::MonoSm,   // legacy font_md 14 → font_sm 12
            SectionLabelSize::Lg   => TextStyle::BodyLg,   // legacy font_lg 16 → font_lg 16
        };
        let rt = tier_rich_mono(tier, ui, &s, resolved).strong();
        let (rt, color) = self.overrides.apply(rt, resolved);
        ui.label(rt.color(color))
    }
}

// ─── section_label adapter ────────────────────────────────────────────────────

/// Free-fn adapter for the legacy `style::section_label(ui, text, color)`.
/// Calls `ui.add(SectionLabel::new(text).tiny().color(color))` — the legacy
/// helper renders at `font_2xs()`, which the `Tiny` variant now sources from
/// the [`TextStyle::Caption`] tier.
#[inline]
pub fn section_label(ui: &mut Ui, text: &str, color: Color32) -> Response {
    ui.add(SectionLabel::new(text).tiny().color(color))
}

// ─── DimLabel ─────────────────────────────────────────────────────────────────

/// Builder for a dim info label. Replaces `style::dim_label(ui, text, color)`.
///
/// Monospace at the [`TextStyle::MonoSm`] tier (= `font_sm()`, the legacy
/// size) — no alpha applied (the caller passes the dim color directly).
///
/// ```ignore
/// ui.add(DimLabel::new("Last updated").color(theme.dim));
/// ```
#[must_use = "DimLabel must be added with `ui.add(...)` to render"]
pub struct DimLabel<'a> {
    text: &'a str,
    color: Option<Color32>,
    overrides: Overrides,
}

impl<'a> DimLabel<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, color: None, overrides: Overrides::default() }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self }
    pub fn size(mut self, px: f32) -> Self { self.overrides.size = Some(px); self }
    pub fn strong(mut self, v: bool) -> Self { self.overrides.strong = Some(v); self }
    pub fn italics(mut self, v: bool) -> Self { self.overrides.italics = Some(v); self }
    pub fn monospace(mut self, v: bool) -> Self { self.overrides.monospace = Some(v); self }
    pub fn gamma(mut self, g: f32) -> Self { self.overrides.gamma = Some(g); self }
}

impl<'a> Widget for DimLabel<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let resolved = self.color.unwrap_or_else(|| {
            // No colour at this call site: ask the ancestors before
            // falling back to the tier's own tone. An explicit
            // `.color()` above still wins — see the precedence tests
            // in `ui_kit/widgets/label.rs`.
            crate::ui_kit::cascade::resolved().color_or(ambient(ui.ctx()).dim)
        });
        // MonoSm, not BodySm: both resolve to `font_sm()` so the size is
        // unchanged, but MonoSm restores the monospace family this widget
        // documents (and that `style::dim_label` renders with).
        let rt = tier_rich(TextStyle::MonoSm, ui, self.text, resolved);
        let (rt, color) = self.overrides.apply(rt, resolved);
        ui.label(rt.color(color))
    }
}

