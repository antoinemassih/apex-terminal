//! `SemanticLabel` — design-system label builder.
//!
//! Encapsulates the repeated `RichText::new(x).monospace().size(sz).strong().color(c)` chain
//! that appeared across tabs, menus, and headers. Callers choose a [`LabelVariant`] that
//! encodes the correct defaults, then override individual knobs as needed.
//!
//! Two emission paths:
//! - [`SemanticLabel::show`]           — direct `ui.label(...)` call, returns `Response`.
//! - [`SemanticLabel::into_rich_text`] — produces a `RichText` for embedding inside
//!                                       `Button::new(...)`, `selectable_label`, etc.

#![allow(dead_code)]

use egui::{Color32, Response, RichText, Ui};
use super::super::style::{font_sm, font_md, font_lg};

// ─── Theme alias ─────────────────────────────────────────────────────────────

type Theme = crate::chart_renderer::gpu::Theme;

// ─── LabelVariant ────────────────────────────────────────────────────────────

/// Semantic role of a label — encodes default size, weight, and monospace flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelVariant {
    /// Tab button label: monospace + `font_sm` + strong.
    Tab,
    /// Menu item / trigger label: monospace + `font_sm`, not strong by default.
    MenuItem,
    /// Panel / dialog header title: monospace + `font_md` + strong.
    Header,
    /// Large header (dialog titles using `font_lg`): monospace + `font_lg` + strong.
    HeaderLg,
}

impl LabelVariant {
    fn default_size(self) -> f32 {
        match self {
            LabelVariant::Tab      => font_sm(),
            LabelVariant::MenuItem => font_sm(),
            LabelVariant::Header   => font_md(),
            LabelVariant::HeaderLg => font_lg(),
        }
    }
    fn default_strong(self) -> bool {
        match self {
            LabelVariant::Tab      => true,
            LabelVariant::MenuItem => false,
            LabelVariant::Header   => true,
            LabelVariant::HeaderLg => true,
        }
    }
    fn default_monospace(self) -> bool {
        true // all variants are monospace by default
    }
}

// ─── SemanticLabel ───────────────────────────────────────────────────────────

/// Builder for a design-system text label.
///
/// ```ignore
/// // Emit as Button content:
/// egui::Button::new(SemanticLabel::new("Orders", LabelVariant::Tab).color(fg).into_rich_text())
///
/// // Emit directly:
/// SemanticLabel::new("Positions", LabelVariant::Header).color(accent).show(ui);
/// ```
pub struct SemanticLabel {
    text:      String,
    variant:   LabelVariant,
    color:     Option<Color32>,
    size:      Option<f32>,
    strong:    Option<bool>,
    monospace: Option<bool>,
}

impl SemanticLabel {
    /// Construct a new label with the given text and variant.
    pub fn new(text: impl Into<String>, variant: LabelVariant) -> Self {
        Self {
            text: text.into(),
            variant,
            color:     None,
            size:      None,
            strong:    None,
            monospace: None,
        }
    }

    /// Override the label color (otherwise caller must supply via `into_rich_text` / `show`).
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self }

    /// Override the font size (default comes from the variant).
    pub fn size(mut self, sz: f32) -> Self { self.size = Some(sz); self }

    /// Override the bold/strong flag (default comes from the variant).
    pub fn strong(mut self, s: bool) -> Self { self.strong = Some(s); self }

    /// Override the monospace flag (default: true for all variants).
    pub fn monospace(mut self, m: bool) -> Self { self.monospace = Some(m); self }

    // ─── Internal helpers ────────────────────────────────────────────────────

    fn resolved_size(&self) -> f32 {
        self.size.unwrap_or_else(|| self.variant.default_size())
    }
    fn resolved_strong(&self) -> bool {
        self.strong.unwrap_or_else(|| self.variant.default_strong())
    }
    fn resolved_monospace(&self) -> bool {
        self.monospace.unwrap_or_else(|| self.variant.default_monospace())
    }

    // ─── Emission ────────────────────────────────────────────────────────────

    /// Produce a `RichText` ready to embed in `Button::new(...)`, `selectable_label`, etc.
    /// The `color` field must already be set (via `.color(...)`) before calling this,
    /// or provide `fallback` as the colour to use when none was set.
    pub fn into_rich_text_with_fallback(self, fallback: Color32) -> RichText {
        let color = self.color.unwrap_or(fallback);
        let size  = self.resolved_size();
        let strong = self.resolved_strong();
        let mono  = self.resolved_monospace();
        let rt = RichText::new(self.text).size(size).color(color);
        let rt = if mono  { rt.monospace() } else { rt };
        let rt = if strong { rt.strong()   } else { rt };
        rt
    }

    /// Produce a `RichText` using the color already set via `.color(...)`.
    /// Panics (debug) / uses `Color32::WHITE` (release) if no color was set.
    pub fn into_rich_text(self) -> RichText {
        let fallback = self.color.unwrap_or(Color32::WHITE);
        self.into_rich_text_with_fallback(fallback)
    }

    /// Render the label directly via `ui.label(...)`. Returns the `Response`.
    /// The `color` field must be set before calling this.
    pub fn show(self, ui: &mut Ui) -> Response {
        let rt = self.into_rich_text();
        ui.label(rt)
    }
}
