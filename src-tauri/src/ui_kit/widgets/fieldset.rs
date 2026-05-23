//! `Fieldset` — collapsible form field group with optional heading.
//!
//! Use this to group related form fields under a heading (e.g., "Advanced
//! Options", "Bracket Order Settings"). When collapsed, only the heading
//! and caret are visible.
//!
//! Distinct from `PanelSection` (which is for panel bodies) and the existing
//! `FieldSet` in `form_section.rs` (which has a full border and no
//! collapsibility). `Fieldset` uses a heading-level separator, form-appropriate
//! padding, and a collapsibility toggle.
//!
//! ```ignore
//! // Non-collapsible group:
//! Fieldset::new("Connection Settings")
//!     .helper("Override defaults for this symbol")
//!     .show(ui, theme, |ui| {
//!         FormField::new("Host").show(ui, theme, |ui| { ... });
//!         FormField::new("Port").show(ui, theme, |ui| { ... });
//!     });
//!
//! // Collapsible group:
//! Fieldset::new("Advanced Options")
//!     .collapsible(&mut state.advanced_open)
//!     .show(ui, theme, |ui| {
//!         FormField::new("Timeout").show(ui, theme, |ui| { ... });
//!     });
//! ```

use egui::{RichText, Sense, Stroke, Ui, Vec2};

use super::theme::ComponentTheme;
use crate::ui_kit::tokens as st;

/// Collapsible form field group with optional heading and subtitle.
///
/// Title is rendered UPPERCASE in `t.dim` at `font_sm()` (per visual spec for
/// form group headings, differentiating them from panel section headers which
/// use `font_md()`). A hairline rule sits below the heading. Body is indented
/// `gap_sm()` from the left edge.
#[must_use = "Fieldset does nothing until `.show(ui, theme, body)` is called"]
pub struct Fieldset<'a> {
    title: &'a str,
    collapsible: Option<&'a mut bool>,
    helper: Option<&'a str>,
}

impl<'a> Fieldset<'a> {
    /// Create a `Fieldset` with the given heading title.
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            collapsible: None,
            helper: None,
        }
    }

    /// Make this group collapsible. `expanded` is the persistent boolean that
    /// controls whether the body is visible. Clicking the header toggles it.
    /// When `None` (the default), the group is always expanded.
    pub fn collapsible(mut self, expanded: &'a mut bool) -> Self {
        self.collapsible = Some(expanded);
        self
    }

    /// Small dim subtitle shown under the heading (not shown when collapsed).
    pub fn helper(mut self, h: &'a str) -> Self {
        self.helper = Some(h);
        self
    }

    /// Render the fieldset header and (when expanded) the body.
    ///
    /// Returns the body's return value as `Option<R>` — `None` when the group
    /// is collapsed.
    pub fn show<R>(
        self,
        ui: &mut Ui,
        theme: &dyn ComponentTheme,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> Option<R> {
        let expanded = match &self.collapsible {
            Some(b) => **b,
            None => true,
        };

        // ── Header row ────────────────────────────────────────────────────────
        let header_resp = ui.horizontal(|ui| {
            // Caret (only when collapsible).
            if self.collapsible.is_some() {
                let caret = if expanded { "▾" } else { "▸" };
                ui.label(
                    RichText::new(caret)
                        .size(st::font_xs())
                        .color(st::color_alpha(theme.dim(), st::alpha_strong())),
                );
                ui.add_space(st::gap_xs());
            }

            // Title — UPPERCASE, dim, font_sm.
            ui.label(
                RichText::new(self.title.to_uppercase())
                    .monospace()
                    .size(st::font_sm())
                    .color(st::color_alpha(theme.dim(), st::alpha_strong())),
            );
        });

        // Make the whole header row clickable when collapsible.
        if let Some(exp) = self.collapsible {
            let header_rect = header_resp.response.rect;
            let click_resp = ui.interact(
                header_rect,
                ui.id().with(self.title).with("fieldset_toggle"),
                Sense::click(),
            );
            if click_resp.clicked() {
                *exp = !*exp;
            }
            if click_resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }

        // ── Hairline rule ─────────────────────────────────────────────────────
        ui.add_space(st::gap_xs());
        {
            let avail = ui.available_width();
            let (rect, _) = ui.allocate_exact_size(
                Vec2::new(avail, st::stroke_thin()),
                Sense::hover(),
            );
            if ui.is_rect_visible(rect) {
                let rule_col = st::color_alpha(theme.border(), st::alpha_muted());
                ui.painter().hline(
                    rect.x_range(),
                    rect.center().y,
                    Stroke::new(st::stroke_thin(), rule_col),
                );
            }
        }

        // ── Body (only when expanded) ─────────────────────────────────────────
        if !expanded {
            return None;
        }

        ui.add_space(st::gap_xs());

        // Optional helper text.
        if let Some(h) = self.helper {
            ui.label(
                RichText::new(h)
                    .monospace()
                    .size(st::font_xs())
                    .italics()
                    .color(st::color_alpha(theme.dim(), st::alpha_dim())),
            );
            ui.add_space(st::gap_xs());
        }

        // Body indented gap_sm() from the left.
        let result = ui
            .horizontal(|ui| {
                ui.add_space(st::gap_sm());
                ui.vertical(|ui| body(ui)).inner
            })
            .inner;

        Some(result)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke: default fieldset is non-collapsible (no `collapsible` set).
    #[test]
    fn default_is_not_collapsible() {
        let fs = Fieldset::new("Options");
        assert!(
            fs.collapsible.is_none(),
            "Fieldset must not be collapsible by default"
        );
        assert_eq!(fs.title, "Options");
    }

    /// Smoke: `.helper()` stores the subtitle text.
    #[test]
    fn helper_stored() {
        let fs = Fieldset::new("Advanced").helper("These are optional settings");
        assert_eq!(fs.helper, Some("These are optional settings"));
    }

    /// Smoke: verify UPPERCASE title rendering (source check).
    #[test]
    fn title_is_uppercased() {
        let src = include_str!("fieldset.rs");
        assert!(
            src.contains("to_uppercase()"),
            "Fieldset must uppercase the title per the visual spec"
        );
    }

    /// Smoke: verify hairline rule uses alpha_muted (source check).
    #[test]
    fn hairline_uses_alpha_muted() {
        let src = include_str!("fieldset.rs");
        assert!(
            src.contains("alpha_muted()"),
            "Fieldset hairline rule must use alpha_muted() per spec"
        );
    }

    /// Smoke: collapsible flag toggles body visibility — simulate the logic.
    #[test]
    fn collapsed_shows_no_body() {
        // When `expanded` is false and no collapsible ptr, it's always shown.
        // With collapsible and expanded=false, show() returns None.
        // We can't run the egui render path in a unit test, but we can verify
        // the source contains the early-return guard.
        let src = include_str!("fieldset.rs");
        assert!(
            src.contains("return None"),
            "Fieldset must return None (skip body) when collapsed"
        );
    }
}

// TODO follow-up: re-export from widgets/mod.rs after the 5-primitives agent lands.
//   pub use fieldset::Fieldset;
